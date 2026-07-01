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
use safenet_core::{Service, index::EventLog, state::StateTransition, tx::Transaction};

/// The sentinel service: drives the request FSM (`preparing -> pending ->
/// committed -> finalized`) from `SentinelOracle`/`Consensus` events and maps
/// its actions to encoded transactions.
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
    detector: Detector,
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
            detector,
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
    fn handle_block_advance(&self, state: State, _block: u64) -> (State, Vec<SentinelAction>) {
        // TODO: move past-deadline `Committed` requests to `Finalized` with
        // a `Finalize` action; drop stale `Preparing`/`Pending`/`Finalized`
        // requests.
        (state, Vec::new())
    }

    fn encode_action(&self, kind: SentinelActionKind) -> Transaction {
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

    async fn new_block(&mut self, state: State, block: u64) -> (State, Vec<Self::Action>) {
        self.handle_block_advance(state, block)
    }

    async fn event(
        &mut self,
        state: State,
        event: EventLog<Self::Event>,
    ) -> (State, Vec<Self::Action>) {
        let block = event.block;
        match event.data {
            SentinelEvents::Consensus(Consensus::ConsensusEvents::OracleTransactionProposed(
                event,
            )) => self.handle_oracle_transaction_proposed(state, block, event),
            SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::NewRequest(event)) => {
                self.handle_new_request(state, block, event)
            }
            SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::Committed(event)) => {
                self.handle_committed(state, event)
            }
            SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::OracleResult(event)) => {
                self.handle_resolved(state, event)
            }
        }
    }
}

impl Service for SentinelService {
    type State = State;

    /// Maps each action to a `(Transaction, expires_at)` pair for the queue.
    ///
    /// The `expires_at` is the request's voting deadline, forwarded from the
    /// action so the `TransactionQueue` can drop it if it goes unsubmitted
    /// past that block.
    fn encode_actions(&self, actions: Vec<SentinelAction>) -> Vec<(Transaction, u64)> {
        actions
            .into_iter()
            .map(|SentinelAction { kind, expires_at }| (self.encode_action(kind), expires_at))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::consensus::{Operation, SafeTransaction};
    use alloy::primitives::{B256, address, uint};

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
        let mut svc = service();
        let safe_tx_hash = B256::repeat_byte(0x01);
        let id = request_id(safe_tx_hash, 7, ORACLE);
        let event =
            SentinelEvents::Consensus(Consensus::ConsensusEvents::OracleTransactionProposed(
                proposed_event(ORACLE, safe_tx_hash),
            ));

        let (state, actions) = svc.event(State::default(), log(5, event)).await;

        assert!(actions.is_empty());
        let request = &state.0[&id];
        assert_eq!(request.status, RequestStatus::Preparing);
        assert!(request.approve);
        assert_eq!(request.deadline, 5 + VOTING_WINDOW);
    }

    #[tokio::test]
    async fn oracle_transaction_proposed_denies_blocklisted_destination() {
        let mut svc = service_with_blocklist(vec![TO]);
        let safe_tx_hash = B256::repeat_byte(0x02);
        let id = request_id(safe_tx_hash, 7, ORACLE);
        let event =
            SentinelEvents::Consensus(Consensus::ConsensusEvents::OracleTransactionProposed(
                proposed_event(ORACLE, safe_tx_hash),
            ));

        let (state, _) = svc.event(State::default(), log(5, event)).await;

        assert!(!state.0[&id].approve);
    }

    #[tokio::test]
    async fn oracle_transaction_proposed_ignores_other_oracles() {
        let mut svc = service();
        let other_oracle = address!("6666666666666666666666666666666666666666");
        let event =
            SentinelEvents::Consensus(Consensus::ConsensusEvents::OracleTransactionProposed(
                proposed_event(other_oracle, B256::repeat_byte(0x03)),
            ));

        let (state, actions) = svc.event(State::default(), log(5, event)).await;

        assert!(state.0.is_empty());
        assert!(actions.is_empty());
    }

    #[tokio::test]
    async fn oracle_transaction_proposed_ignores_a_duplicate_for_an_existing_request() {
        let mut svc = service();
        let safe_tx_hash = B256::repeat_byte(0x0f);
        let id = request_id(safe_tx_hash, 7, ORACLE);
        let mut committed = request_state(RequestStatus::Preparing, 50, true);
        committed.status = RequestStatus::Committed;
        let state = with_request(id, committed.clone());
        let event =
            SentinelEvents::Consensus(Consensus::ConsensusEvents::OracleTransactionProposed(
                proposed_event(ORACLE, safe_tx_hash),
            ));

        let (state, actions) = svc.event(state, log(5, event)).await;

        assert_eq!(state.0[&id], committed);
        assert!(actions.is_empty());
    }

    #[tokio::test]
    async fn new_request_commits_approve_vote_for_a_preparing_request() {
        let mut svc = service();
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

        let (state, actions) = svc.event(state, log(40, event)).await;

        let request = &state.0[&id];
        assert_eq!(request.status, RequestStatus::Pending);
        assert_eq!(request.deadline, 50);
        assert_eq!(
            actions,
            vec![
                SentinelAction {
                    kind: SentinelActionKind::ApproveToken {
                        bond: U256::from(1_000u64)
                    },
                    expires_at: 50,
                },
                SentinelAction {
                    kind: SentinelActionKind::CommitApprove { id },
                    expires_at: 50,
                },
            ]
        );
    }

    #[tokio::test]
    async fn new_request_commits_deny_vote_when_denied() {
        let mut svc = service();
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

        let (state, actions) = svc.event(state, log(40, event)).await;

        let request = &state.0[&id];
        assert_eq!(request.status, RequestStatus::Pending);
        assert_eq!(request.deadline, 50);
        assert_eq!(
            actions,
            vec![
                SentinelAction {
                    kind: SentinelActionKind::ApproveToken {
                        bond: U256::from(1_000u64)
                    },
                    expires_at: 50,
                },
                SentinelAction {
                    kind: SentinelActionKind::CommitDeny { id },
                    expires_at: 50,
                },
            ]
        );
    }

    #[tokio::test]
    async fn new_request_ignores_unknown_or_non_preparing_requests() {
        let mut svc = service();
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

        let (state, actions) = svc.event(State::default(), log(1, event())).await;
        assert!(state.0.is_empty());
        assert!(actions.is_empty());

        let mut pending = with_request(id, request_state(RequestStatus::Preparing, 1, true));
        pending.0.get_mut(&id).unwrap().status = RequestStatus::Pending;
        let (state, actions) = svc.event(pending, log(1, event())).await;
        assert_eq!(state.0[&id].status, RequestStatus::Pending);
        assert!(actions.is_empty());
    }

    #[tokio::test]
    async fn committed_moves_a_pending_request_to_committed_for_our_account() {
        let mut svc = service();
        let id = B256::repeat_byte(0x07);
        let state = with_request(id, request_state(RequestStatus::Pending, 50, true));

        let (state, actions) = svc
            .event(state, log(10, committed_event(id, ACCOUNT)))
            .await;

        assert_eq!(state.0[&id].status, RequestStatus::Committed);
        assert!(actions.is_empty());
    }

    #[tokio::test]
    async fn committed_ignores_other_sentinels() {
        let mut svc = service();
        let id = B256::repeat_byte(0x08);
        let other = address!("8888888888888888888888888888888888888888");
        let state = with_request(id, request_state(RequestStatus::Pending, 50, true));

        let (state, actions) = svc.event(state, log(10, committed_event(id, other))).await;

        assert_eq!(state.0[&id].status, RequestStatus::Pending);
        assert!(actions.is_empty());
    }

    #[tokio::test]
    async fn committed_ignores_unknown_or_non_pending_requests() {
        let mut svc = service();
        let id = B256::repeat_byte(0x09);

        let (state, actions) = svc
            .event(State::default(), log(10, committed_event(id, ACCOUNT)))
            .await;
        assert!(state.0.is_empty());
        assert!(actions.is_empty());

        let state = with_request(id, request_state(RequestStatus::Preparing, 50, true));
        let (state, actions) = svc
            .event(state, log(10, committed_event(id, ACCOUNT)))
            .await;
        assert_eq!(state.0[&id].status, RequestStatus::Preparing);
        assert!(actions.is_empty());
    }

    #[tokio::test]
    async fn resolved_claims_and_drops_a_committed_request_when_our_vote_won() {
        let mut svc = service();
        let id = B256::repeat_byte(0x0a);
        let state = with_request(id, request_state(RequestStatus::Committed, 50, true));
        let event = resolved_event(id, true, ResolveReason::UNANIMOUS_APPROVE);

        let (state, actions) = svc.event(state, log(10, event)).await;

        assert!(!state.0.contains_key(&id));
        assert_eq!(
            actions,
            vec![SentinelAction {
                kind: SentinelActionKind::Claim { id },
                expires_at: u64::MAX,
            }]
        );
    }

    #[tokio::test]
    async fn resolved_claims_and_drops_a_finalized_request_when_our_vote_won() {
        let mut svc = service();
        let id = B256::repeat_byte(0x0b);
        let state = with_request(id, request_state(RequestStatus::Finalized, 60, false));
        let event = resolved_event(id, false, ResolveReason::UNANIMOUS_DENY);

        let (state, actions) = svc.event(state, log(10, event)).await;

        assert!(!state.0.contains_key(&id));
        assert_eq!(
            actions,
            vec![SentinelAction {
                kind: SentinelActionKind::Claim { id },
                expires_at: u64::MAX,
            }]
        );
    }

    #[tokio::test]
    async fn resolved_claims_on_timeout_even_when_our_vote_lost() {
        let mut svc = service();
        let id = B256::repeat_byte(0x0c);
        let state = with_request(id, request_state(RequestStatus::Committed, 50, false));
        let event = resolved_event(id, true, ResolveReason::TIMEOUT);

        let (state, actions) = svc.event(state, log(10, event)).await;

        assert!(!state.0.contains_key(&id));
        assert_eq!(
            actions,
            vec![SentinelAction {
                kind: SentinelActionKind::Claim { id },
                expires_at: u64::MAX,
            }]
        );
    }

    #[tokio::test]
    async fn resolved_drops_without_claiming_when_our_vote_lost() {
        let mut svc = service();
        let id = B256::repeat_byte(0x0d);
        let state = with_request(id, request_state(RequestStatus::Committed, 50, true));
        let event = resolved_event(id, false, ResolveReason::UNANIMOUS_DENY);

        let (state, actions) = svc.event(state, log(10, event)).await;

        assert!(!state.0.contains_key(&id));
        assert!(actions.is_empty());
    }

    #[tokio::test]
    async fn resolved_drops_without_claiming_a_request_we_never_committed_onchain() {
        let mut svc = service();
        let id = B256::repeat_byte(0x0e);
        let state = with_request(id, request_state(RequestStatus::Pending, 50, true));
        let event = resolved_event(id, true, ResolveReason::UNANIMOUS_APPROVE);

        let (state, actions) = svc.event(state, log(10, event)).await;

        assert!(!state.0.contains_key(&id));
        assert!(actions.is_empty());
    }

    #[tokio::test]
    async fn resolved_ignores_an_unknown_request() {
        let mut svc = service();
        let id = B256::repeat_byte(0x0f);
        let event = resolved_event(id, true, ResolveReason::UNANIMOUS_APPROVE);

        let (state, actions) = svc.event(State::default(), log(10, event)).await;

        assert!(state.0.is_empty());
        assert!(actions.is_empty());
    }

    #[tokio::test]
    async fn resolved_drops_without_panicking_on_a_malformed_result() {
        let mut svc = service();
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

        let (state, actions) = svc.event(state, log(10, event)).await;

        assert!(!state.0.contains_key(&id));
        assert!(actions.is_empty());
    }

    #[test]
    fn encodes_approve_token() {
        let bond = uint!(1_000_U256);
        let tx = service().encode_action(SentinelActionKind::ApproveToken { bond });

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
        let tx = service().encode_action(SentinelActionKind::CommitApprove { id });

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
        let tx = service().encode_action(SentinelActionKind::CommitDeny { id });

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
        let tx = service().encode_action(SentinelActionKind::Finalize { id });

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
        let tx = service().encode_action(SentinelActionKind::Claim { id });

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
        let encoded = service().encode_actions(vec![
            SentinelAction {
                kind: SentinelActionKind::ApproveToken { bond },
                expires_at: deadline,
            },
            SentinelAction {
                kind: SentinelActionKind::CommitApprove { id },
                expires_at: deadline,
            },
        ]);

        assert_eq!(encoded.len(), 2);
        assert_eq!(encoded[0].0.to, FEE_TOKEN);
        assert_eq!(encoded[1].0.to, ORACLE);
        assert_eq!(encoded[0].1, deadline);
        assert_eq!(encoded[1].1, deadline);
    }
}
