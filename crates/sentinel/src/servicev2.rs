use crate::{
    action::{SentinelActionKindV2 as SentinelActionKind, SentinelActionV2 as SentinelAction},
    bindings::{
        SentinelEventsV2 as SentinelEvents,
        consensus::Consensus,
        oracle::{ERC20, RequestState as OnchainRequestState, SentinelOracleV2 as SentinelOracle},
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
        let Some(entry) = state.0.get_mut(&event.requestId) else {
            return (state, Vec::new());
        };
        let RequestState::CollectingVotes {
            committed_count,
            revealed_count,
            approve_count,
            deny_count,
            self_revealed,
            ..
        } = entry
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
        let (update, actions) = self.finalize(entry, event.requestId);
        match update {
            None => {
                state.0.remove(&event.requestId);
            }
            Some(entry) => {
                state.0.insert(event.requestId, entry);
            }
        }
        (state, actions)
    }

    /// Drops requests we never got to commit on in time, reveals (or drops)
    /// requests past their commit deadline, and finalizes requests past
    /// their reveal deadline.
    fn handle_block_advance(&self, mut state: State, block: u64) -> (State, Vec<SentinelAction>) {
        let mut actions = Vec::new();

        state.0.retain(|id, entry| match *entry {
            RequestState::WaitingForRequest { deadline, .. } => block <= deadline,
            RequestState::CollectingCommitments {
                approve,
                commit_deadline,
                reveal_deadline,
                committed_count,
                self_committed,
            } => {
                if block <= commit_deadline {
                    return true;
                }
                // Our own commit never landed onchain, so revealing would
                // just revert; drop the request instead.
                if !self_committed {
                    return false;
                }
                let salt = self.signer.reveal_salt(*id);
                actions.push(SentinelAction {
                    kind: SentinelActionKind::Reveal {
                        id: *id,
                        approve,
                        salt,
                    },
                    expires_at: Some(reveal_deadline),
                });
                *entry = RequestState::CollectingVotes {
                    approve,
                    reveal_deadline,
                    committed_count,
                    revealed_count: 0,
                    approve_count: 0,
                    deny_count: 0,
                    self_revealed: false,
                };
                true
            }
            RequestState::CollectingVotes {
                reveal_deadline, ..
            } => {
                if block <= reveal_deadline {
                    return true;
                }
                let (update, finalization) = self.finalize(entry, *id);
                actions.extend(finalization);
                match update {
                    None => false,
                    Some(new_state) => {
                        *entry = new_state;
                        true
                    }
                }
            }
            RequestState::WaitingForDisputeResolution { .. } => true,
        });

        (state, actions)
    }

    /// Resolves a genuine dispute — `DisputeResolved` is only ever emitted by
    /// `resolveDispute`, i.e. only for a request that reached
    /// `WaitingForDisputeResolution` — by claiming iff our own revealed vote
    /// matches the arbitrator's outcome; drops the request either way.
    fn handle_resolved(
        &self,
        mut state: State,
        event: SentinelOracle::DisputeResolved,
    ) -> (State, Vec<SentinelAction>) {
        let Some(RequestState::WaitingForDisputeResolution { approve }) =
            state.0.get(&event.requestId)
        else {
            return (state, Vec::new());
        };
        let approve = *approve;
        state.0.remove(&event.requestId);
        let approved = event.outcome == OnchainRequestState::RESOLVED_APPROVED;
        let actions = if approved == approve {
            vec![SentinelAction {
                kind: SentinelActionKind::Claim {
                    id: event.requestId,
                },
                expires_at: None,
            }]
        } else {
            Vec::new()
        };
        (state, actions)
    }

    /// Shared finalize step, reached from either the early-finalize check
    /// in [`Self::handle_revealed`] or the reveal-deadline branch in
    /// [`Self::handle_block_advance`]; always exits `CollectingVotes` in
    /// this same step, so a request's finalize step can only ever run once.
    ///
    /// There are the following cases when the `Finalize` action is emitted:
    /// - no one voted: a genuine timeout, where the bonds can be re-claimed
    /// - unanimous vote: it is possible to claim the bond and reward
    /// - a disbute: there is still a possibility to receive a reward
    ///
    /// In other cases it doesn't make sense to trigger the finalization for
    /// this sentinel.
    fn finalize(
        &self,
        state: &RequestState,
        request_id: B256,
    ) -> (Option<RequestState>, Vec<SentinelAction>) {
        let RequestState::CollectingVotes {
            approve,
            revealed_count,
            approve_count,
            deny_count,
            self_revealed,
            ..
        } = state
        else {
            return (None, Vec::new());
        };
        let approve = *approve;
        let dispute = *approve_count > 0 && *deny_count > 0;
        let timed_out = *revealed_count == 0;

        // If this sentinel did not participate and it was not a timeout
        // then no actions should be taken and the request should be dropped
        if !*self_revealed && !timed_out {
            return (None, Vec::new());
        }

        let mut actions = vec![SentinelAction {
            kind: SentinelActionKind::Finalize { id: request_id },
            expires_at: None,
        }];

        // In case of a disbute it is not a timeout, so this sentinal participated.
        // Finalize the request and wait for a disbute resolution by the arbitrator.
        if dispute {
            return (
                Some(RequestState::WaitingForDisputeResolution { approve }),
                actions,
            );
        }

        // Unanimity plus our own counted vote guarantees this sentinal is on the
        // sole, winning side; no `DisputeResolved` round trip needed.
        actions.push(SentinelAction {
            kind: SentinelActionKind::Claim { id: request_id },
            expires_at: None,
        });
        (None, actions)
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
                        SentinelOracle::SentinelOracleV2Events::DisputeResolved(event),
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

/// Flow tests drive `apply_transition` through a whole request lifecycle —
/// proposal, commit, reveal, finalize, claim/dispute — rather than exercising
/// each handler in isolation, since the interesting behavior (early
/// finalization, the timeout-only liveness branch, dispute vs. immediate
/// claim) only shows up across a sequence of transitions.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::consensus::{Operation, SafeTransaction};
    use alloy::{
        primitives::{address, keccak256},
        signers::k256::ecdsa::SigningKey,
    };
    use safenet_core::index::EventLog;

    const ORACLE: Address = address!("1111111111111111111111111111111111111111");
    const FEE_TOKEN: Address = address!("2222222222222222222222222222222222222222");
    const CONSENSUS: Address = address!("3333333333333333333333333333333333333333");
    const SAFE: Address = address!("4444444444444444444444444444444444444444");
    const TO: Address = address!("5555555555555555555555555555555555555555");
    const OTHER: Address = address!("8888888888888888888888888888888888888888");
    const CHAIN_ID: u64 = 1;
    const VOTING_WINDOW: u64 = 10;

    fn self_signer() -> Signer {
        Signer::new(
            SigningKey::from_bytes(&keccak256("sentinel-v2-flow-test-key").0.into()).unwrap(),
        )
    }

    fn self_address() -> Address {
        self_signer().address()
    }

    fn service_with_blocklist(blocklist: Vec<Address>) -> SentinelService {
        SentinelService::new(
            ORACLE,
            FEE_TOKEN,
            CONSENSUS,
            self_signer(),
            U256::from(CHAIN_ID),
            VOTING_WINDOW,
            Detector::new(blocklist),
        )
    }

    fn transition_with_blocklist(blocklist: Vec<Address>) -> SentinelTransition {
        service_with_blocklist(blocklist).components().0
    }

    fn transition() -> SentinelTransition {
        transition_with_blocklist(vec![])
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

    fn proposed_event(oracle: Address, safe_tx_hash: B256, to: Address) -> SentinelEvents {
        SentinelEvents::Consensus(Consensus::ConsensusEvents::OracleTransactionProposed(
            Consensus::OracleTransactionProposed {
                safeTxHash: safe_tx_hash,
                chainId: U256::from(CHAIN_ID),
                safe: SAFE,
                epoch: 7,
                oracle,
                transaction: safe_tx(to),
            },
        ))
    }

    fn new_request_event(
        id: B256,
        fee: U256,
        bond_target: U256,
        commit_deadline: u64,
        reveal_deadline: u64,
    ) -> SentinelEvents {
        SentinelEvents::Oracle(SentinelOracle::SentinelOracleV2Events::NewRequest(
            SentinelOracle::NewRequest {
                requestId: id,
                proposer: SAFE,
                fee,
                bondTarget: bond_target,
                commitDeadline: U256::from(commit_deadline),
                revealDeadline: U256::from(reveal_deadline),
            },
        ))
    }

    fn committed_event(id: B256, sentinel: Address, bond: U256) -> SentinelEvents {
        SentinelEvents::Oracle(SentinelOracle::SentinelOracleV2Events::Committed(
            SentinelOracle::Committed {
                requestId: id,
                sentinel,
                bondAmount: bond,
            },
        ))
    }

    fn revealed_event(id: B256, sentinel: Address, approved: bool, bond: U256) -> SentinelEvents {
        SentinelEvents::Oracle(SentinelOracle::SentinelOracleV2Events::Revealed(
            SentinelOracle::Revealed {
                requestId: id,
                sentinel,
                approved,
                bondAmount: bond,
            },
        ))
    }

    fn dispute_resolved_event(
        id: B256,
        outcome: OnchainRequestState,
        slashed: U256,
    ) -> SentinelEvents {
        SentinelEvents::Oracle(SentinelOracle::SentinelOracleV2Events::DisputeResolved(
            SentinelOracle::DisputeResolved {
                requestId: id,
                outcome,
                slashed,
            },
        ))
    }

    fn log(block: u64, data: SentinelEvents) -> EventLog<SentinelEvents> {
        EventLog {
            block,
            index: 0,
            data,
        }
    }

    /// Full happy path: propose, commit (from two sentinels), reveal (from
    /// two sentinels) — unanimously in favor — and finalize/claim as soon as
    /// the last reveal lands, without waiting out the reveal window.
    #[test]
    fn flow_unanimous_approve_finalizes_via_early_reveal_and_claims() {
        let svc = transition();
        let safe_tx_hash = B256::repeat_byte(0x01);
        let id = request_id(safe_tx_hash, 7, ORACLE);

        // The transaction is proposed onchain; we decide to approve it and
        // start tracking the request.
        let (state, commands) = svc.apply_transition(
            State::default(),
            Message::Event(log(1, proposed_event(ORACLE, safe_tx_hash, TO))),
        );
        assert!(commands.is_empty());
        assert_eq!(
            state.0[&id],
            RequestState::WaitingForRequest {
                approve: true,
                deadline: 1 + VOTING_WINDOW,
            },
        );

        // A duplicate/re-delivered proposal for the same request must not
        // reset progress.
        let (state, commands) = svc.apply_transition(
            state,
            Message::Event(log(2, proposed_event(ORACLE, safe_tx_hash, TO))),
        );
        assert!(commands.is_empty());
        assert_eq!(
            state.0[&id],
            RequestState::WaitingForRequest {
                approve: true,
                deadline: 1 + VOTING_WINDOW,
            },
        );

        // The request is opened onchain: we lock a bond behind a blind
        // commitment hash.
        let (state, commands) = svc.apply_transition(
            state,
            Message::Event(log(
                5,
                new_request_event(id, U256::from(1_000u64), U256::from(500u64), 20, 40),
            )),
        );
        let salt = self_signer().reveal_salt(id);
        let hash = commit_hash(self_address(), id, true, salt);
        assert_eq!(
            state.0[&id],
            RequestState::CollectingCommitments {
                approve: true,
                commit_deadline: 20,
                reveal_deadline: 40,
                committed_count: 0,
                self_committed: false,
            },
        );
        assert_eq!(
            commands,
            vec![
                Command::Action(SentinelAction {
                    kind: SentinelActionKind::ApproveToken {
                        bond: U256::from(500u64)
                    },
                    expires_at: Some(20),
                }),
                Command::Action(SentinelAction {
                    kind: SentinelActionKind::Commit { id, hash },
                    expires_at: Some(20),
                }),
            ],
        );

        // Our own commit lands onchain, followed by the other sentinel's.
        let (state, commands) = svc.apply_transition(
            state,
            Message::Event(log(
                6,
                committed_event(id, self_address(), U256::from(500u64)),
            )),
        );
        assert!(commands.is_empty());
        let (state, commands) = svc.apply_transition(
            state,
            Message::Event(log(7, committed_event(id, OTHER, U256::from(500u64)))),
        );
        assert!(commands.is_empty());
        assert_eq!(
            state.0[&id],
            RequestState::CollectingCommitments {
                approve: true,
                commit_deadline: 20,
                reveal_deadline: 40,
                committed_count: 2,
                self_committed: true,
            },
        );

        // Past the commit deadline, our own commit landed, so we reveal.
        let (state, commands) = svc.apply_transition(state, Message::NewBlock(21));
        assert_eq!(
            commands,
            vec![Command::Action(SentinelAction {
                kind: SentinelActionKind::Reveal {
                    id,
                    approve: true,
                    salt
                },
                expires_at: Some(40),
            })],
        );
        assert_eq!(
            state.0[&id],
            RequestState::CollectingVotes {
                approve: true,
                reveal_deadline: 40,
                committed_count: 2,
                revealed_count: 0,
                approve_count: 0,
                deny_count: 0,
                self_revealed: false,
            },
        );

        // The other sentinel reveals first; not enough to finalize yet.
        let (state, commands) = svc.apply_transition(
            state,
            Message::Event(log(22, revealed_event(id, OTHER, true, U256::from(500u64)))),
        );
        assert!(commands.is_empty());
        assert_eq!(
            state.0[&id],
            RequestState::CollectingVotes {
                approve: true,
                reveal_deadline: 40,
                committed_count: 2,
                revealed_count: 1,
                approve_count: 1,
                deny_count: 0,
                self_revealed: false,
            },
        );

        // Our own reveal lands; every commit is now revealed, unanimously in
        // favor, so we finalize and claim immediately instead of waiting out
        // the reveal window.
        let (state, commands) = svc.apply_transition(
            state,
            Message::Event(log(
                23,
                revealed_event(id, self_address(), true, U256::from(500u64)),
            )),
        );
        assert!(!state.0.contains_key(&id));
        assert_eq!(
            commands,
            vec![
                Command::Action(SentinelAction {
                    kind: SentinelActionKind::Finalize { id },
                    expires_at: None,
                }),
                Command::Action(SentinelAction {
                    kind: SentinelActionKind::Claim { id },
                    expires_at: None,
                }),
            ],
        );
    }

    /// Drives a request to `WaitingForDisputeResolution`: both sides
    /// revealed, so the local tally can't resolve it — an external
    /// `DisputeResolved` is needed. Shared by the two arbitration-outcome
    /// tests below.
    fn setup_dispute() -> (SentinelTransition, B256, State) {
        let svc = transition();
        let safe_tx_hash = B256::repeat_byte(0x03);
        let id = request_id(safe_tx_hash, 7, ORACLE);

        let (state, _) = svc.apply_transition(
            State::default(),
            Message::Event(log(1, proposed_event(ORACLE, safe_tx_hash, TO))),
        );
        let (state, _) = svc.apply_transition(
            state,
            Message::Event(log(
                5,
                new_request_event(id, U256::from(1_000u64), U256::from(500u64), 20, 40),
            )),
        );
        let (state, _) = svc.apply_transition(
            state,
            Message::Event(log(
                6,
                committed_event(id, self_address(), U256::from(500u64)),
            )),
        );
        let (state, _) = svc.apply_transition(
            state,
            Message::Event(log(7, committed_event(id, OTHER, U256::from(500u64)))),
        );
        let (state, _) = svc.apply_transition(state, Message::NewBlock(21));

        // The other sentinel reveals the opposite vote, and our own reveal
        // lands last: unanimity fails, so this is a genuine dispute.
        let (state, _) = svc.apply_transition(
            state,
            Message::Event(log(
                22,
                revealed_event(id, OTHER, false, U256::from(500u64)),
            )),
        );
        let (state, commands) = svc.apply_transition(
            state,
            Message::Event(log(
                23,
                revealed_event(id, self_address(), true, U256::from(500u64)),
            )),
        );

        assert_eq!(
            commands,
            vec![Command::Action(SentinelAction {
                kind: SentinelActionKind::Finalize { id },
                expires_at: None,
            })],
        );
        assert_eq!(
            state.0[&id],
            RequestState::WaitingForDisputeResolution { approve: true },
        );

        (svc, id, state)
    }

    #[test]
    fn flow_dispute_claims_when_arbitration_matches_our_vote() {
        let (svc, id, state) = setup_dispute();
        let event = dispute_resolved_event(id, OnchainRequestState::RESOLVED_APPROVED, U256::ZERO);

        let (state, commands) = svc.apply_transition(state, Message::Event(log(50, event)));

        assert!(!state.0.contains_key(&id));
        assert_eq!(
            commands,
            vec![Command::Action(SentinelAction {
                kind: SentinelActionKind::Claim { id },
                expires_at: None,
            })],
        );
    }

    #[test]
    fn flow_dispute_drops_without_claim_when_arbitration_contradicts_our_vote() {
        let (svc, id, state) = setup_dispute();
        let event = dispute_resolved_event(id, OnchainRequestState::RESOLVED_DENIED, U256::ZERO);

        let (state, commands) = svc.apply_transition(state, Message::Event(log(50, event)));

        assert!(!state.0.contains_key(&id));
        assert!(commands.is_empty());
    }

    /// Nobody reveals at all — a genuine timeout with no other sentinel's
    /// FSM around to finalize instead — so we finalize and claim our own
    /// still-`PENDING` (unslashed) commitment ourselves.
    #[test]
    fn flow_finalizes_and_claims_on_genuine_reveal_timeout() {
        let svc = transition();
        let safe_tx_hash = B256::repeat_byte(0x05);
        let id = request_id(safe_tx_hash, 7, ORACLE);

        let (state, _) = svc.apply_transition(
            State::default(),
            Message::Event(log(1, proposed_event(ORACLE, safe_tx_hash, TO))),
        );
        let (state, _) = svc.apply_transition(
            state,
            Message::Event(log(
                5,
                new_request_event(id, U256::from(1_000u64), U256::from(500u64), 20, 40),
            )),
        );
        let (state, _) = svc.apply_transition(
            state,
            Message::Event(log(
                6,
                committed_event(id, self_address(), U256::from(500u64)),
            )),
        );

        let salt = self_signer().reveal_salt(id);
        let (state, commands) = svc.apply_transition(state, Message::NewBlock(21));
        assert_eq!(
            commands,
            vec![Command::Action(SentinelAction {
                kind: SentinelActionKind::Reveal {
                    id,
                    approve: true,
                    salt
                },
                expires_at: Some(40),
            })],
        );

        // Our own reveal transaction never confirms onchain, and neither
        // does anyone else's.
        let (state, commands) = svc.apply_transition(state, Message::NewBlock(41));

        assert!(!state.0.contains_key(&id));
        assert_eq!(
            commands,
            vec![
                Command::Action(SentinelAction {
                    kind: SentinelActionKind::Finalize { id },
                    expires_at: None,
                }),
                Command::Action(SentinelAction {
                    kind: SentinelActionKind::Claim { id },
                    expires_at: None,
                }),
            ],
        );
    }
}
