use std::{convert::Infallible, sync::Arc};

use crate::{
    action::{SentinelAction, SentinelActionKind},
    bindings::{
        SentinelEvents,
        consensus::Consensus,
        oracle::{ERC20, ResolveReason, SentinelOracle},
    },
    detector::Detector,
    hashing::oracle_tx_proposal_hash,
    state::{RequestStatus, SentinelRequestState, State},
};
use alloy::{
    primitives::{Address, U256},
    sol_types::{SolCall, SolValue},
};
use safenet_core::{
    driver::{ActionEncoder, Service},
    state::{Command, Commands, Message, Pure, StateTransition},
    tx::Transaction,
};

/// The sentinel service: drives the request FSM (`preparing -> pending ->
/// committed -> finalized`) from `SentinelOracle`/`Consensus` events and maps
/// its actions to encoded transactions.
#[derive(Clone)]
pub struct SentinelService {
    oracle: Address,
    fee_token: Address,
    /// The `Consensus` contract whose `OracleTransactionProposed` events are
    /// hashed into request ids.
    consensus: Address,
    /// Our own address, used to identify votes we committed onchain.
    account: Address,
    /// The chain id of the EIP-712 domain used to derive request ids.
    chain_id: U256,
    /// The number of blocks a `Preparing` request is kept alive for before
    /// being cleaned up.
    voting_window: u64,
    detector: Arc<Detector>,
}

impl SentinelService {
    #[cfg_attr(not(test), expect(dead_code))]
    pub fn new(
        oracle: Address,
        fee_token: Address,
        consensus: Address,
        account: Address,
        chain_id: U256,
        voting_window: u64,
        detector: Detector,
    ) -> Self {
        Self {
            oracle,
            fee_token,
            consensus,
            account,
            chain_id,
            voting_window,
            detector: Arc::new(detector),
        }
    }

    /// Starts tracking a newly proposed oracle transaction, deciding whether
    /// we vote to approve or deny it.
    fn handle_oracle_transaction_proposed(
        &self,
        mut state: State,
        block: u64,
        event: Consensus::OracleTransactionProposed,
    ) -> (State, Vec<SentinelAction>) {
        if event.oracle != self.oracle {
            return (state, Vec::new());
        }
        let request_id = oracle_tx_proposal_hash(
            self.chain_id,
            self.consensus,
            event.epoch,
            event.oracle,
            event.safeTxHash,
        );
        // A duplicate or re-delivered proposal for the same request must not
        // reset an already-tracked request (e.g. back to `Preparing` after
        // it has advanced to `Pending`/`Committed`/`Finalized`).
        if state.0.contains_key(&request_id) {
            return (state, Vec::new());
        }
        let approve = self.detector.approve(&event.transaction);
        state.0.insert(
            request_id,
            SentinelRequestState {
                deadline: block.saturating_add(self.voting_window),
                approve,
                status: RequestStatus::Preparing,
            },
        );
        (state, Vec::new())
    }

    /// Casts our vote once a tracked request is opened for voting onchain.
    fn handle_new_request(
        &self,
        mut state: State,
        block: u64,
        event: SentinelOracle::NewRequest,
    ) -> (State, Vec<SentinelAction>) {
        let Some(existing) = state.0.get_mut(&event.requestId) else {
            return (state, Vec::new());
        };
        if existing.status != RequestStatus::Preparing {
            return (state, Vec::new());
        }
        let approve = existing.approve;
        let deadline = block.saturating_add(self.voting_window);
        existing.deadline = deadline;
        existing.status = RequestStatus::Pending;
        let vote = if approve {
            SentinelActionKind::CommitApprove {
                id: event.requestId,
            }
        } else {
            SentinelActionKind::CommitDeny {
                id: event.requestId,
            }
        };
        let actions = vec![
            SentinelAction {
                kind: SentinelActionKind::ApproveToken {
                    bond: event.bondTarget,
                },
                expires_at: deadline,
            },
            SentinelAction {
                kind: vote,
                expires_at: deadline,
            },
        ];
        (state, actions)
    }

    /// Records that our vote has been committed onchain.
    fn handle_committed(
        &self,
        mut state: State,
        event: SentinelOracle::Committed,
    ) -> (State, Vec<SentinelAction>) {
        if event.sentinel != self.account {
            return (state, Vec::new());
        }
        let Some(existing) = state.0.get_mut(&event.requestId) else {
            return (state, Vec::new());
        };
        if existing.status != RequestStatus::Pending {
            return (state, Vec::new());
        }
        existing.status = RequestStatus::Committed;
        (state, Vec::new())
    }

    /// Claims the bond for a request we committed on, once its outcome is
    /// known.
    fn handle_resolved(
        &self,
        mut state: State,
        event: SentinelOracle::OracleResult,
    ) -> (State, Vec<SentinelAction>) {
        let Some(existing) = state.0.remove(&event.requestId) else {
            return (state, Vec::new());
        };
        // We only committed onchain if the request reached `Committed`/
        // `Finalized`; otherwise our commit tx may never have confirmed, so
        // drop the request without claiming.
        if existing.status != RequestStatus::Committed
            && existing.status != RequestStatus::Finalized
        {
            return (state, Vec::new());
        }
        let Ok(reason) = ResolveReason::abi_decode(&event.result) else {
            tracing::warn!(
                request_id = %event.requestId,
                result = %event.result,
                "OracleResult.result did not decode as ResolveReason; dropping request without claiming",
            );
            return (state, Vec::new());
        };
        let vote_won = reason == ResolveReason::TIMEOUT || event.approved == existing.approve;
        let actions = if vote_won {
            vec![SentinelAction {
                kind: SentinelActionKind::Claim {
                    id: event.requestId,
                },
                // Claiming has no onchain deadline, so the action must never
                // expire in the `TransactionQueue`.
                expires_at: u64::MAX,
            }]
        } else {
            Vec::new()
        };
        (state, actions)
    }

    /// Finalizes committed requests and cleans up stale ones once their
    /// voting deadline has passed.
    fn handle_block_advance(&self, mut state: State, block: u64) -> (State, Vec<SentinelAction>) {
        let mut actions = Vec::new();
        state.0.retain(|&id, request| {
            if block <= request.deadline {
                return true;
            }
            match request.status {
                // Never opened for voting or never committed in time; drop
                // rather than finalize onchain.
                RequestStatus::Preparing | RequestStatus::Pending => false,
                RequestStatus::Committed => {
                    // Extend the deadline so the freshly `Finalized` request
                    // survives long enough for its `OracleResult` to arrive
                    // before we treat it as stale.
                    request.status = RequestStatus::Finalized;
                    request.deadline = block.saturating_add(self.voting_window);
                    actions.push(SentinelAction {
                        kind: SentinelActionKind::Finalize { id },
                        expires_at: request.deadline,
                    });
                    true
                }
                // `OracleResult` never arrived to claim and remove it; give up.
                RequestStatus::Finalized => false,
            }
        });
        (state, actions)
    }

    fn encode_action_kind(&self, kind: SentinelActionKind) -> Transaction {
        match kind {
            SentinelActionKind::ApproveToken { bond } => Transaction {
                to: self.fee_token,
                value: U256::ZERO,
                data: ERC20::approveCall {
                    spender: self.oracle,
                    amount: bond,
                }
                .abi_encode()
                .into(),
                gas: 55_000,
            },
            SentinelActionKind::CommitApprove { id } => Transaction {
                to: self.oracle,
                value: U256::ZERO,
                data: SentinelOracle::commitApproveCall { requestId: id }
                    .abi_encode()
                    .into(),
                gas: 100_000,
            },
            SentinelActionKind::CommitDeny { id } => Transaction {
                to: self.oracle,
                value: U256::ZERO,
                data: SentinelOracle::commitDenyCall { requestId: id }
                    .abi_encode()
                    .into(),
                gas: 100_000,
            },
            SentinelActionKind::Finalize { id } => Transaction {
                to: self.oracle,
                value: U256::ZERO,
                data: SentinelOracle::finalizeCall { requestId: id }
                    .abi_encode()
                    .into(),
                gas: 100_000,
            },
            SentinelActionKind::Claim { id } => Transaction {
                to: self.oracle,
                value: U256::ZERO,
                data: SentinelOracle::claimCall { requestId: id }
                    .abi_encode()
                    .into(),
                gas: 100_000,
            },
        }
    }
}

impl StateTransition<State> for SentinelService {
    type Event = SentinelEvents;
    type Action = SentinelAction;
    type Effect = Infallible;
    type Resume = Infallible;

    fn apply_transition(
        &self,
        state: State,
        message: Message<Self::Event, Self::Resume>,
    ) -> (State, Commands<State, Self>) {
        let (state, actions) = match message {
            Message::NewBlock(block) => self.handle_block_advance(state, block),
            Message::Event(event) => {
                let block = event.block;
                match event.data {
                    SentinelEvents::Consensus(
                        Consensus::ConsensusEvents::OracleTransactionProposed(event),
                    ) => self.handle_oracle_transaction_proposed(state, block, event),
                    SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::NewRequest(
                        event,
                    )) => self.handle_new_request(state, block, event),
                    SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::Committed(
                        event,
                    )) => self.handle_committed(state, event),
                    SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::OracleResult(
                        event,
                    )) => self.handle_resolved(state, event),
                }
            }
            Message::Resume(result) => match result {},
        };
        let commands = actions.into_iter().map(Command::Action).collect();
        (state, commands)
    }
}

impl ActionEncoder<SentinelAction> for SentinelService {
    fn encode_action(&self, action: SentinelAction) -> (Transaction, u64) {
        (self.encode_action_kind(action.kind), action.expires_at)
    }
}

impl Service for SentinelService {
    type State = State;
    type Event = SentinelEvents;

    type Transition = Self;
    type Effects = Pure;
    type Actions = Self;

    fn components(self) -> (Self::Transition, Self::Effects, Self::Actions) {
        (self.clone(), Pure, self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::consensus::{Operation, SafeTransaction};
    use alloy::primitives::{B256, address, uint};
    use safenet_core::index::EventLog;

    const ORACLE: Address = address!("1111111111111111111111111111111111111111");
    const FEE_TOKEN: Address = address!("2222222222222222222222222222222222222222");
    const CONSENSUS: Address = address!("3333333333333333333333333333333333333333");
    const SAFE: Address = address!("4444444444444444444444444444444444444444");
    const TO: Address = address!("5555555555555555555555555555555555555555");
    const ACCOUNT: Address = address!("7777777777777777777777777777777777777777");
    const CHAIN_ID: u64 = 1;
    const VOTING_WINDOW: u64 = 10;

    fn service() -> SentinelService {
        service_with_blocklist(vec![])
    }

    fn service_with_blocklist(blocklist: Vec<Address>) -> SentinelService {
        SentinelService::new(
            ORACLE,
            FEE_TOKEN,
            CONSENSUS,
            ACCOUNT,
            U256::from(CHAIN_ID),
            VOTING_WINDOW,
            Detector::new(blocklist),
        )
    }

    fn safe_tx(to: Address) -> SafeTransaction {
        SafeTransaction {
            to,
            operation: Operation::CALL,
            ..Default::default()
        }
    }

    fn request_id(safe_tx_hash: B256, epoch: u64, oracle: Address) -> B256 {
        oracle_tx_proposal_hash(U256::from(CHAIN_ID), CONSENSUS, epoch, oracle, safe_tx_hash)
    }

    fn request_state(status: RequestStatus, deadline: u64, approve: bool) -> SentinelRequestState {
        SentinelRequestState {
            deadline,
            approve,
            status,
        }
    }

    fn proposed_event(oracle: Address, safe_tx_hash: B256) -> Consensus::OracleTransactionProposed {
        Consensus::OracleTransactionProposed {
            safeTxHash: safe_tx_hash,
            chainId: U256::from(CHAIN_ID),
            safe: SAFE,
            epoch: 7,
            oracle,
            transaction: safe_tx(TO),
        }
    }

    fn committed_event(id: B256, sentinel: Address) -> SentinelEvents {
        SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::Committed(
            SentinelOracle::Committed {
                requestId: id,
                sentinel,
                approved: true,
                bondAmount: U256::ZERO,
                position: U256::ZERO,
            },
        ))
    }

    fn resolved_event(id: B256, approved: bool, reason: ResolveReason) -> SentinelEvents {
        SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::OracleResult(
            SentinelOracle::OracleResult {
                requestId: id,
                proposer: SAFE,
                result: reason.abi_encode().into(),
                approved,
            },
        ))
    }

    fn with_request(id: B256, request: SentinelRequestState) -> State {
        let mut state = State::default();
        state.0.insert(id, request);
        state
    }

    fn log(block: u64, data: SentinelEvents) -> EventLog<SentinelEvents> {
        EventLog {
            block,
            index: 0,
            data,
        }
    }

    #[tokio::test]
    async fn oracle_transaction_proposed_tracks_a_preparing_request() {
        let svc = service();
        let safe_tx_hash = B256::repeat_byte(0x01);
        let id = request_id(safe_tx_hash, 7, ORACLE);
        let event =
            SentinelEvents::Consensus(Consensus::ConsensusEvents::OracleTransactionProposed(
                proposed_event(ORACLE, safe_tx_hash),
            ));

        let (state, commands) =
            svc.apply_transition(State::default(), Message::Event(log(5, event)));

        assert!(commands.is_empty());
        let request = &state.0[&id];
        assert_eq!(request.status, RequestStatus::Preparing);
        assert!(request.approve);
        assert_eq!(request.deadline, 5 + VOTING_WINDOW);
    }

    #[tokio::test]
    async fn oracle_transaction_proposed_denies_blocklisted_destination() {
        let svc = service_with_blocklist(vec![TO]);
        let safe_tx_hash = B256::repeat_byte(0x02);
        let id = request_id(safe_tx_hash, 7, ORACLE);
        let event =
            SentinelEvents::Consensus(Consensus::ConsensusEvents::OracleTransactionProposed(
                proposed_event(ORACLE, safe_tx_hash),
            ));

        let (state, _) = svc.apply_transition(State::default(), Message::Event(log(5, event)));

        assert!(!state.0[&id].approve);
    }

    #[tokio::test]
    async fn oracle_transaction_proposed_ignores_other_oracles() {
        let svc = service();
        let other_oracle = address!("6666666666666666666666666666666666666666");
        let event =
            SentinelEvents::Consensus(Consensus::ConsensusEvents::OracleTransactionProposed(
                proposed_event(other_oracle, B256::repeat_byte(0x03)),
            ));

        let (state, commands) =
            svc.apply_transition(State::default(), Message::Event(log(5, event)));

        assert!(state.0.is_empty());
        assert!(commands.is_empty());
    }

    #[tokio::test]
    async fn oracle_transaction_proposed_ignores_a_duplicate_for_an_existing_request() {
        let svc = service();
        let safe_tx_hash = B256::repeat_byte(0x0f);
        let id = request_id(safe_tx_hash, 7, ORACLE);
        let mut committed = request_state(RequestStatus::Preparing, 50, true);
        committed.status = RequestStatus::Committed;
        let state = with_request(id, committed.clone());
        let event =
            SentinelEvents::Consensus(Consensus::ConsensusEvents::OracleTransactionProposed(
                proposed_event(ORACLE, safe_tx_hash),
            ));

        let (state, commands) = svc.apply_transition(state, Message::Event(log(5, event)));

        assert_eq!(state.0[&id], committed);
        assert!(commands.is_empty());
    }

    #[tokio::test]
    async fn new_request_commits_approve_vote_for_a_preparing_request() {
        let svc = service();
        let id = B256::repeat_byte(0x04);
        let state = with_request(id, request_state(RequestStatus::Preparing, 1, true));
        let event = SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::NewRequest(
            SentinelOracle::NewRequest {
                requestId: id,
                proposer: SAFE,
                fee: U256::ZERO,
                bondTarget: U256::from(1_000u64),
                deadline: U256::ZERO,
            },
        ));

        let (state, commands) = svc.apply_transition(state, Message::Event(log(40, event)));

        let request = &state.0[&id];
        assert_eq!(request.status, RequestStatus::Pending);
        assert_eq!(request.deadline, 50);
        assert_eq!(
            commands,
            vec![
                Command::Action(SentinelAction {
                    kind: SentinelActionKind::ApproveToken {
                        bond: U256::from(1_000u64)
                    },
                    expires_at: 50,
                }),
                Command::Action(SentinelAction {
                    kind: SentinelActionKind::CommitApprove { id },
                    expires_at: 50,
                }),
            ]
        );
    }

    #[tokio::test]
    async fn new_request_commits_deny_vote_when_denied() {
        let svc = service();
        let id = B256::repeat_byte(0x05);
        let state = with_request(id, request_state(RequestStatus::Preparing, 1, false));
        let event = SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::NewRequest(
            SentinelOracle::NewRequest {
                requestId: id,
                proposer: SAFE,
                fee: U256::ZERO,
                bondTarget: U256::from(1_000u64),
                deadline: U256::ZERO,
            },
        ));

        let (state, commands) = svc.apply_transition(state, Message::Event(log(40, event)));

        let request = &state.0[&id];
        assert_eq!(request.status, RequestStatus::Pending);
        assert_eq!(request.deadline, 50);
        assert_eq!(
            commands,
            vec![
                Command::Action(SentinelAction {
                    kind: SentinelActionKind::ApproveToken {
                        bond: U256::from(1_000u64)
                    },
                    expires_at: 50,
                }),
                Command::Action(SentinelAction {
                    kind: SentinelActionKind::CommitDeny { id },
                    expires_at: 50,
                }),
            ]
        );
    }

    #[tokio::test]
    async fn new_request_ignores_unknown_or_non_preparing_requests() {
        let svc = service();
        let id = B256::repeat_byte(0x06);
        let event = || {
            SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::NewRequest(
                SentinelOracle::NewRequest {
                    requestId: id,
                    proposer: SAFE,
                    fee: U256::ZERO,
                    bondTarget: U256::from(1_000u64),
                    deadline: U256::ZERO,
                },
            ))
        };

        let (state, commands) =
            svc.apply_transition(State::default(), Message::Event(log(1, event())));
        assert!(state.0.is_empty());
        assert!(commands.is_empty());

        let mut pending = with_request(id, request_state(RequestStatus::Preparing, 1, true));
        pending.0.get_mut(&id).unwrap().status = RequestStatus::Pending;
        let (state, commands) = svc.apply_transition(pending, Message::Event(log(1, event())));
        assert_eq!(state.0[&id].status, RequestStatus::Pending);
        assert!(commands.is_empty());
    }

    #[tokio::test]
    async fn committed_moves_a_pending_request_to_committed_for_our_account() {
        let svc = service();
        let id = B256::repeat_byte(0x07);
        let state = with_request(id, request_state(RequestStatus::Pending, 50, true));

        let (state, commands) =
            svc.apply_transition(state, Message::Event(log(10, committed_event(id, ACCOUNT))));

        assert_eq!(state.0[&id].status, RequestStatus::Committed);
        assert!(commands.is_empty());
    }

    #[tokio::test]
    async fn committed_ignores_other_sentinels() {
        let svc = service();
        let id = B256::repeat_byte(0x08);
        let other = address!("8888888888888888888888888888888888888888");
        let state = with_request(id, request_state(RequestStatus::Pending, 50, true));

        let (state, commands) =
            svc.apply_transition(state, Message::Event(log(10, committed_event(id, other))));

        assert_eq!(state.0[&id].status, RequestStatus::Pending);
        assert!(commands.is_empty());
    }

    #[tokio::test]
    async fn committed_ignores_unknown_or_non_pending_requests() {
        let svc = service();
        let id = B256::repeat_byte(0x09);

        let (state, commands) = svc.apply_transition(
            State::default(),
            Message::Event(log(10, committed_event(id, ACCOUNT))),
        );
        assert!(state.0.is_empty());
        assert!(commands.is_empty());

        let state = with_request(id, request_state(RequestStatus::Preparing, 50, true));
        let (state, commands) =
            svc.apply_transition(state, Message::Event(log(10, committed_event(id, ACCOUNT))));
        assert_eq!(state.0[&id].status, RequestStatus::Preparing);
        assert!(commands.is_empty());
    }

    #[tokio::test]
    async fn resolved_claims_and_drops_a_committed_request_when_our_vote_won() {
        let svc = service();
        let id = B256::repeat_byte(0x0a);
        let state = with_request(id, request_state(RequestStatus::Committed, 50, true));
        let event = resolved_event(id, true, ResolveReason::UNANIMOUS_APPROVE);

        let (state, commands) = svc.apply_transition(state, Message::Event(log(10, event)));

        assert!(!state.0.contains_key(&id));
        assert_eq!(
            commands,
            vec![Command::Action(SentinelAction {
                kind: SentinelActionKind::Claim { id },
                expires_at: u64::MAX,
            })]
        );
    }

    #[tokio::test]
    async fn resolved_claims_and_drops_a_finalized_request_when_our_vote_won() {
        let svc = service();
        let id = B256::repeat_byte(0x0b);
        let state = with_request(id, request_state(RequestStatus::Finalized, 60, false));
        let event = resolved_event(id, false, ResolveReason::UNANIMOUS_DENY);

        let (state, commands) = svc.apply_transition(state, Message::Event(log(10, event)));

        assert!(!state.0.contains_key(&id));
        assert_eq!(
            commands,
            vec![Command::Action(SentinelAction {
                kind: SentinelActionKind::Claim { id },
                expires_at: u64::MAX,
            })]
        );
    }

    #[tokio::test]
    async fn resolved_claims_on_timeout_even_when_our_vote_lost() {
        let svc = service();
        let id = B256::repeat_byte(0x0c);
        let state = with_request(id, request_state(RequestStatus::Committed, 50, false));
        let event = resolved_event(id, true, ResolveReason::TIMEOUT);

        let (state, commands) = svc.apply_transition(state, Message::Event(log(10, event)));

        assert!(!state.0.contains_key(&id));
        assert_eq!(
            commands,
            vec![Command::Action(SentinelAction {
                kind: SentinelActionKind::Claim { id },
                expires_at: u64::MAX,
            })]
        );
    }

    #[tokio::test]
    async fn resolved_drops_without_claiming_when_our_vote_lost() {
        let svc = service();
        let id = B256::repeat_byte(0x0d);
        let state = with_request(id, request_state(RequestStatus::Committed, 50, true));
        let event = resolved_event(id, false, ResolveReason::UNANIMOUS_DENY);

        let (state, commands) = svc.apply_transition(state, Message::Event(log(10, event)));

        assert!(!state.0.contains_key(&id));
        assert!(commands.is_empty());
    }

    #[tokio::test]
    async fn resolved_drops_without_claiming_a_request_we_never_committed_onchain() {
        let svc = service();
        let id = B256::repeat_byte(0x0e);
        let state = with_request(id, request_state(RequestStatus::Pending, 50, true));
        let event = resolved_event(id, true, ResolveReason::UNANIMOUS_APPROVE);

        let (state, commands) = svc.apply_transition(state, Message::Event(log(10, event)));

        assert!(!state.0.contains_key(&id));
        assert!(commands.is_empty());
    }

    #[tokio::test]
    async fn resolved_ignores_an_unknown_request() {
        let svc = service();
        let id = B256::repeat_byte(0x0f);
        let event = resolved_event(id, true, ResolveReason::UNANIMOUS_APPROVE);

        let (state, commands) =
            svc.apply_transition(State::default(), Message::Event(log(10, event)));

        assert!(state.0.is_empty());
        assert!(commands.is_empty());
    }

    #[tokio::test]
    async fn resolved_drops_without_panicking_on_a_malformed_result() {
        let svc = service();
        let id = B256::repeat_byte(0x10);
        let state = with_request(id, request_state(RequestStatus::Committed, 50, true));
        let event = SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::OracleResult(
            SentinelOracle::OracleResult {
                requestId: id,
                proposer: SAFE,
                result: Vec::new().into(),
                approved: true,
            },
        ));

        let (state, commands) = svc.apply_transition(state, Message::Event(log(10, event)));

        assert!(!state.0.contains_key(&id));
        assert!(commands.is_empty());
    }

    #[tokio::test]
    async fn block_advance_finalizes_a_past_deadline_committed_request() {
        let svc = service();
        let id = B256::repeat_byte(0x10);
        let state = with_request(id, request_state(RequestStatus::Committed, 50, true));

        let (state, commands) = svc.apply_transition(state, Message::NewBlock(51));

        let request = &state.0[&id];
        assert_eq!(request.status, RequestStatus::Finalized);
        assert_eq!(request.deadline, 51 + VOTING_WINDOW);
        assert_eq!(
            commands,
            vec![Command::Action(SentinelAction {
                kind: SentinelActionKind::Finalize { id },
                expires_at: 51 + VOTING_WINDOW,
            })]
        );
    }

    #[tokio::test]
    async fn block_advance_drops_stale_requests() {
        let svc = service();
        let preparing_id = B256::repeat_byte(0x12);
        let pending_id = B256::repeat_byte(0x13);
        let finalized_id = B256::repeat_byte(0x16);
        let mut state = with_request(
            preparing_id,
            request_state(RequestStatus::Preparing, 50, true),
        );
        state
            .0
            .insert(pending_id, request_state(RequestStatus::Pending, 50, true));
        state.0.insert(
            finalized_id,
            request_state(RequestStatus::Finalized, 50, true),
        );

        let (state, commands) = svc.apply_transition(state, Message::NewBlock(51));

        assert!(!state.0.contains_key(&preparing_id));
        assert!(!state.0.contains_key(&pending_id));
        assert!(!state.0.contains_key(&finalized_id));
        assert!(commands.is_empty());
    }

    #[tokio::test]
    async fn block_advance_keeps_requests_before_their_deadline() {
        let svc = service();
        let comitted_id = B256::repeat_byte(0x11);
        let preparing_id = B256::repeat_byte(0x14);
        let pending_id = B256::repeat_byte(0x15);
        let finalized_id = B256::repeat_byte(0x16);
        let mut state = with_request(
            comitted_id,
            request_state(RequestStatus::Committed, 50, true),
        );
        state.0.insert(
            preparing_id,
            request_state(RequestStatus::Preparing, 50, true),
        );
        state
            .0
            .insert(pending_id, request_state(RequestStatus::Pending, 50, true));
        state.0.insert(
            finalized_id,
            request_state(RequestStatus::Finalized, 50, true),
        );

        let (state, commands) = svc.apply_transition(state, Message::NewBlock(50));

        assert!(state.0.contains_key(&comitted_id));
        assert!(state.0.contains_key(&preparing_id));
        assert!(state.0.contains_key(&pending_id));
        assert!(state.0.contains_key(&finalized_id));
        assert!(commands.is_empty());
    }

    #[test]
    fn encodes_approve_token() {
        let bond = uint!(1_000_U256);
        let tx = service().encode_action_kind(SentinelActionKind::ApproveToken { bond });

        assert_eq!(tx.to, FEE_TOKEN);
        assert_eq!(tx.value, U256::ZERO);
        assert_eq!(tx.gas, 55_000);
        assert_eq!(
            tx.data.as_ref(),
            ERC20::approveCall {
                spender: ORACLE,
                amount: bond
            }
            .abi_encode(),
        );
    }

    #[test]
    fn encodes_commit_approve() {
        let id = B256::repeat_byte(0x01);
        let tx = service().encode_action_kind(SentinelActionKind::CommitApprove { id });

        assert_eq!(tx.to, ORACLE);
        assert_eq!(tx.value, U256::ZERO);
        assert_eq!(tx.gas, 100_000);
        assert_eq!(
            tx.data.as_ref(),
            SentinelOracle::commitApproveCall { requestId: id }.abi_encode(),
        );
    }

    #[test]
    fn encodes_commit_deny() {
        let id = B256::repeat_byte(0x02);
        let tx = service().encode_action_kind(SentinelActionKind::CommitDeny { id });

        assert_eq!(tx.to, ORACLE);
        assert_eq!(tx.value, U256::ZERO);
        assert_eq!(tx.gas, 100_000);
        assert_eq!(
            tx.data.as_ref(),
            SentinelOracle::commitDenyCall { requestId: id }.abi_encode(),
        );
    }

    #[test]
    fn encodes_finalize() {
        let id = B256::repeat_byte(0x03);
        let tx = service().encode_action_kind(SentinelActionKind::Finalize { id });

        assert_eq!(tx.to, ORACLE);
        assert_eq!(tx.value, U256::ZERO);
        assert_eq!(tx.gas, 100_000);
        assert_eq!(
            tx.data.as_ref(),
            SentinelOracle::finalizeCall { requestId: id }.abi_encode(),
        );
    }

    #[test]
    fn encodes_claim() {
        let id = B256::repeat_byte(0x04);
        let tx = service().encode_action_kind(SentinelActionKind::Claim { id });

        assert_eq!(tx.to, ORACLE);
        assert_eq!(tx.value, U256::ZERO);
        assert_eq!(tx.gas, 100_000);
        assert_eq!(
            tx.data.as_ref(),
            SentinelOracle::claimCall { requestId: id }.abi_encode(),
        );
    }

    #[test]
    fn encode_actions_forwards_expiry() {
        let bond = uint!(500_U256);
        let id = B256::repeat_byte(0xab);
        let deadline = 999u64;
        for action in [
            SentinelAction {
                kind: SentinelActionKind::ApproveToken { bond },
                expires_at: deadline,
            },
            SentinelAction {
                kind: SentinelActionKind::CommitApprove { id },
                expires_at: deadline,
            },
        ] {
            let (_, encoded_deadline) = service().encode_action(action);
            assert_eq!(encoded_deadline, deadline);
        }
    }
}
