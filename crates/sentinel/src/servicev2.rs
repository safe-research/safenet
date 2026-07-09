use crate::{
    action::{SentinelActionKindV2 as SentinelActionKind, SentinelActionV2 as SentinelAction},
    bindings::{
        SentinelEventsV2 as SentinelEvents,
        consensus::Consensus,
        oracle::{ERC20, SentinelOracleV2 as SentinelOracle},
    },
    detector::Detector,
    hashing::{RevealSalt as _, commit_hash, oracle_tx_proposal_hash},
    state::{SentinelRequestStateV2 as RequestState, StateV2 as State},
};
use alloy::{
    primitives::{Address, B256, U256},
    sol_types::SolCall,
};
use safenet_core::{
    driver::{ActionEncoder, Service},
    state::{Command, Commands, Message, Pure, StateTransition},
    tx::{Signer, Transaction},
};
use std::convert::Infallible;

/// The sentinel service: drives the request FSM (mirroring
/// `SentinelOracleRequest.State`'s commit-reveal phases) from
/// `SentinelOracle`/`Consensus` events and maps its actions to encoded
/// transactions.
pub struct SentinelService {
    oracle: Address,
    fee_token: Address,
    consensus: Address,
    signer: Signer,
    chain_id: U256,
    voting_window: u64,
    detector: Detector,
}

/// Advances the request FSM in response to `SentinelOracle`/`Consensus`
/// events.
pub struct SentinelTransition {
    oracle: Address,
    /// The `Consensus` contract whose `OracleTransactionProposed` events are
    /// hashed into request ids.
    consensus: Address,
    /// Our own account, used to compute commitment hashes and identify votes
    /// we committed onchain.
    signer: Signer,
    /// The chain id of the EIP-712 domain used to derive request ids.
    chain_id: U256,
    /// The number of blocks a `WaitingForRequest` request is kept alive for
    /// before being cleaned up.
    voting_window: u64,
    detector: Detector,
}

/// Encodes [`SentinelAction`]s into the transactions that commit, reveal,
/// finalize and claim oracle requests.
pub struct SentinelEncoder {
    /// The `SentinelOracle` contract that commits, reveals, finalizations
    /// and claims are submitted to, and the spender approved to pull the
    /// bond.
    oracle: Address,
    /// The ERC-20 token that bonds are posted in.
    fee_token: Address,
}

#[expect(dead_code)]
impl SentinelService {
    pub fn new(
        oracle: Address,
        fee_token: Address,
        consensus: Address,
        signer: Signer,
        chain_id: U256,
        voting_window: u64,
        detector: Detector,
    ) -> Self {
        Self {
            oracle,
            fee_token,
            consensus,
            signer,
            chain_id,
            voting_window,
            detector,
        }
    }
}

impl SentinelTransition {
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
        // reset an already-tracked request (e.g. back to `WaitingForRequest`
        // after it has advanced further).
        if state.0.contains_key(&request_id) {
            return (state, Vec::new());
        }
        let approve = self.detector.approve(&event.transaction);
        state.0.insert(
            request_id,
            RequestState::WaitingForRequest {
                approve,
                deadline: block.saturating_add(self.voting_window),
            },
        );
        (state, Vec::new())
    }

    /// Locks a bond behind a blind commitment once a tracked request is
    /// opened for commits onchain.
    fn handle_new_request(
        &self,
        mut state: State,
        event: SentinelOracle::NewRequest,
    ) -> (State, Vec<SentinelAction>) {
        let Some(RequestState::WaitingForRequest { approve, .. }) = state.0.get(&event.requestId)
        else {
            return (state, Vec::new());
        };
        let approve = *approve;
        let commit_deadline = event.commitDeadline.saturating_to::<u64>();
        let reveal_deadline = event.revealDeadline.saturating_to::<u64>();
        let salt = self.signer.reveal_salt(event.requestId);
        let hash = commit_hash(self.signer.address(), event.requestId, approve, salt);
        state.0.insert(
            event.requestId,
            RequestState::CollectingCommitments {
                approve,
                commit_deadline,
                reveal_deadline,
                committed_count: 0,
                self_committed: false,
            },
        );
        let actions = vec![
            SentinelAction {
                kind: SentinelActionKind::ApproveToken {
                    bond: event.bondTarget,
                },
                expires_at: Some(commit_deadline),
            },
            SentinelAction {
                kind: SentinelActionKind::Commit {
                    id: event.requestId,
                    hash,
                },
                expires_at: Some(commit_deadline),
            },
        ];
        (state, actions)
    }

    /// Tallies a commitment landing onchain, from any sentinel, for a
    /// request we're still collecting commits for.
    fn handle_committed(
        &self,
        mut state: State,
        event: SentinelOracle::Committed,
    ) -> (State, Vec<SentinelAction>) {
        let Some(RequestState::CollectingCommitments {
            committed_count,
            self_committed,
            ..
        }) = state.0.get_mut(&event.requestId)
        else {
            return (state, Vec::new());
        };
        *committed_count += 1;
        if event.sentinel == self.signer.address() {
            *self_committed = true;
        }
        (state, Vec::new())
    }

    /// Tallies a reveal landing onchain, from any sentinel, and
    /// early-finalizes once every commit has been revealed.
    fn handle_revealed(
        &self,
        mut state: State,
        event: SentinelOracle::Revealed,
    ) -> (State, Vec<SentinelAction>) {
        let Some(RequestState::CollectingVotes {
            committed_count,
            revealed_count,
            approve_count,
            deny_count,
            self_revealed,
            ..
        }) = state.0.get_mut(&event.requestId)
        else {
            return (state, Vec::new());
        };
        *revealed_count += 1;
        if event.approved {
            *approve_count += 1;
        } else {
            *deny_count += 1;
        }
        if event.sentinel == self.signer.address() {
            *self_revealed = true;
        }
        if *revealed_count < *committed_count {
            return (state, Vec::new());
        }
        self.finalize(state, event.requestId)
    }

    /// Shared finalize step, reached from either the early-finalize check
    /// in [`Self::handle_revealed`] or the reveal-deadline branch in
    /// [`Self::handle_block_advance`]; always exits `CollectingVotes` in
    /// this same step, so a request's finalize step can only ever run once.
    ///
    /// There are the following cases when the `Finalize` action is emitted:
    /// - no one voted: a genuine timeout, where the bonds can be re-claimed
    /// - unanimous vote: it is possible to claim the bond and reward
    ///
    /// In other cases it doesn't make sense to trigger the finalization for
    /// this sentinel.
    fn finalize(&self, mut state: State, request_id: B256) -> (State, Vec<SentinelAction>) {
        let Some(RequestState::CollectingVotes {
            approve,
            revealed_count,
            approve_count,
            deny_count,
            self_revealed,
            ..
        }) = state.0.get(&request_id)
        else {
            return (state, Vec::new());
        };
        let approve = *approve;
        let dispute = *approve_count > 0 && *deny_count > 0;
        let timed_out = *revealed_count == 0;
        let self_revealed = *self_revealed;

        // Neither action has an onchain deadline past which it stops being
        // valid to submit, so both are kept alive indefinitely in the
        // `TransactionQueue`.
        let mut actions = vec![SentinelAction {
            kind: SentinelActionKind::Finalize { id: request_id },
            expires_at: None,
        }];

        if !self_revealed {
            state.0.remove(&request_id);
            if timed_out {
                return (state, actions);
            } else {
                return (state, Vec::new());
            };
        }

        if dispute {
            state.0.insert(
                request_id,
                RequestState::WaitingForDisputeResolution { approve },
            );
            return (state, actions);
        }

        // Unanimity plus our own counted vote guarantees we're on the
        // sole, winning side; no `OracleResult` round trip needed.
        actions.push(SentinelAction {
            kind: SentinelActionKind::Claim { id: request_id },
            expires_at: None,
        });
        state.0.remove(&request_id);
        (state, actions)
    }

    // TODO(sentinel commit-reveal, service FSM sub-task B3d): drops requests
    // we never got to commit on in time, reveals (or drops) requests past
    // their commit deadline, and finalizes requests past their reveal
    // deadline.
    fn handle_block_advance(&self, _state: State, _block: u64) -> (State, Vec<SentinelAction>) {
        todo!("service FSM sub-task B3d")
    }

    // TODO(sentinel commit-reveal, service FSM sub-task B3e): claims iff
    // our own revealed vote matches the oracle's outcome, dropping the
    // request either way.
    fn handle_resolved(
        &self,
        _state: State,
        _event: SentinelOracle::OracleResult,
    ) -> (State, Vec<SentinelAction>) {
        todo!("service FSM sub-task B3e")
    }
}

impl SentinelEncoder {
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
            // Measured onchain at ~196k gas for a request's first commit (fresh
            // storage slots for the request, the commitment and the ERC-20
            // allowance spend); 100k undershot this and ran out of gas. 250k
            // keeps headroom for `reveal`/`finalize`/`claim`'s own cold-storage
            // writes and the fee-token transfer.
            SentinelActionKind::Commit { id, hash } => Transaction {
                to: self.oracle,
                value: U256::ZERO,
                data: SentinelOracle::commitCall {
                    requestId: id,
                    commitHash: hash,
                }
                .abi_encode()
                .into(),
                gas: 250_000,
            },
            SentinelActionKind::Reveal { id, approve, salt } => Transaction {
                to: self.oracle,
                value: U256::ZERO,
                data: SentinelOracle::revealCall {
                    requestId: id,
                    approve,
                    salt,
                }
                .abi_encode()
                .into(),
                gas: 250_000,
            },
            SentinelActionKind::Finalize { id } => Transaction {
                to: self.oracle,
                value: U256::ZERO,
                data: SentinelOracle::finalizeCall { requestId: id }
                    .abi_encode()
                    .into(),
                gas: 250_000,
            },
            SentinelActionKind::Claim { id } => Transaction {
                to: self.oracle,
                value: U256::ZERO,
                data: SentinelOracle::claimCall { requestId: id }
                    .abi_encode()
                    .into(),
                gas: 250_000,
            },
        }
    }
}

impl StateTransition<State> for SentinelTransition {
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
                    SentinelEvents::Oracle(SentinelOracle::SentinelOracleV2Events::NewRequest(
                        event,
                    )) => self.handle_new_request(state, event),
                    SentinelEvents::Oracle(SentinelOracle::SentinelOracleV2Events::Committed(
                        event,
                    )) => self.handle_committed(state, event),
                    SentinelEvents::Oracle(SentinelOracle::SentinelOracleV2Events::Revealed(
                        event,
                    )) => self.handle_revealed(state, event),
                    SentinelEvents::Oracle(
                        SentinelOracle::SentinelOracleV2Events::OracleResult(event),
                    ) => self.handle_resolved(state, event),
                }
            }
            Message::Resume(result) => match result {},
        };
        let commands = actions.into_iter().map(Command::Action).collect();
        (state, commands)
    }
}

impl ActionEncoder<SentinelAction> for SentinelEncoder {
    fn encode_action(&self, action: SentinelAction) -> (Transaction, Option<u64>) {
        (self.encode_action_kind(action.kind), action.expires_at)
    }
}

impl Service for SentinelService {
    type State = State;
    type Event = SentinelEvents;

    type Transition = SentinelTransition;
    type Effects = Pure;
    type Actions = SentinelEncoder;

    fn components(self) -> (Self::Transition, Self::Effects, Self::Actions) {
        let SentinelService {
            oracle,
            fee_token,
            consensus,
            signer,
            chain_id,
            voting_window,
            detector,
        } = self;
        (
            SentinelTransition {
                oracle,
                consensus,
                signer,
                chain_id,
                voting_window,
                detector,
            },
            Pure,
            SentinelEncoder { oracle, fee_token },
        )
    }
}
