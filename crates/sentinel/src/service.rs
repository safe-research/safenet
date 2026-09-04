use crate::{
    action::{SentinelAction, SentinelActionKind},
    bindings::{
        SentinelEvents,
        consensus::Consensus,
        oracle::{ERC20, RequestState as OracleRequestState, SentinelOracle},
    },
    effect,
    engine::{CheckOutcome, EngineClient},
    hashing::{RevealSalt as _, commit_hash, oracle_tx_proposal_hash},
    metrics::ResolvedOutcome,
    state::{Request, SentinelRequestState as RequestState, State},
};
use alloy::{
    primitives::{Address, B256, U256},
    sol_types::SolCall,
};
use safenet_core::{
    driver::{ActionEncoder, Service},
    state::{Command, Commands, Message, StateTransition},
    tx::{Signer, Transaction},
};
use std::time::Duration;

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
    /// Checks proposed transactions.
    engine: EngineClient,
    /// Maximum time the sentinel engine has to answer a security check.
    engine_timeout: Duration,
}

/// Advances the request FSM in response to `SentinelOracle`/`Consensus`
/// events.
pub struct SentinelTransition {
    oracle: Address,
    /// The `Consensus` contract whose `TransactionProposed` events are
    /// hashed into request ids.
    consensus: Address,
    /// Our own account, used to compute commitment hashes and identify votes
    /// we committed onchain.
    signer: Signer,
    /// The chain id of the EIP-712 domain used to derive request ids.
    chain_id: U256,
    /// The number of blocks a request without an onchain commit deadline is
    /// kept alive for before being cleaned up.
    voting_window: u64,
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
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        oracle: Address,
        fee_token: Address,
        consensus: Address,
        signer: Signer,
        chain_id: U256,
        voting_window: u64,
        engine: EngineClient,
        engine_timeout: Duration,
    ) -> Self {
        Self {
            oracle,
            fee_token,
            consensus,
            signer,
            chain_id,
            voting_window,
            engine,
            engine_timeout,
        }
    }
}

impl SentinelTransition {
    /// Starts tracking a newly proposed oracle transaction and requests a
    /// verdict from the configured sentinel engine via
    /// [`effect::Effect::EngineCheck`].
    fn handle_oracle_transaction_proposed(
        &self,
        mut state: State,
        block: u64,
        event: Consensus::TransactionProposed,
    ) -> (State, Commands<State, Self>) {
        if event.oracle != self.oracle {
            return (state, Vec::new());
        }
        let request_id = oracle_tx_proposal_hash(
            self.chain_id,
            self.consensus,
            event.epoch,
            event.oracle,
            event.oracleData.clone(),
            event.safeTxHash,
        );
        // A duplicate or re-delivered proposal for the same request must not
        // reset an already-tracked request (e.g. back to `WaitingForRequest`
        // after it has advanced further).
        if let Some(entry) = state.0.get(&request_id) {
            tracing::warn!(
                %request_id,
                state = entry.name(),
                "ignoring duplicate oracle transaction proposal"
            );
            return (state, Vec::new());
        }
        let deadline = block.saturating_add(self.voting_window);
        state.0.insert(
            request_id,
            RequestState::WaitingForEngineCheck {
                deadline,
                request: None,
            },
        );
        crate::metrics::requests_proposed_total().increment(1);

        (
            state,
            vec![Command::Effect(effect::Effect::EngineCheck {
                request_id,
                transaction: event.transaction,
                block,
            })],
        )
    }

    /// Consumes a [`effect::Effect::EngineCheck`]'s resolved outcome for
    /// `request_id`. If the onchain request is already open, voting begins
    /// immediately; otherwise the resolved decision waits for `NewRequest`.
    fn handle_engine_check_result(
        &self,
        mut state: State,
        request_id: B256,
        outcome: CheckOutcome,
    ) -> (State, Commands<State, Self>) {
        let (deadline, request) = match state.0.remove(&request_id) {
            Some(RequestState::WaitingForEngineCheck { deadline, request }) => (deadline, request),
            Some(entry) => {
                tracing::warn!(
                    %request_id,
                    state = entry.name(),
                    "ignoring unexpected engine check result"
                );
                state.0.insert(request_id, entry);
                return (state, Vec::new());
            }
            None => {
                tracing::warn!(%request_id, "ignoring stale engine check result");
                return (state, Vec::new());
            }
        };

        let (approve, reason) = match outcome {
            CheckOutcome::Approved => (true, String::new()),
            CheckOutcome::Denied(rule) => (false, rule.to_string()),
            CheckOutcome::Unknown => {
                tracing::warn!(%request_id, "engine check failed; dropping request unanswered");
                return (state, Vec::new());
            }
        };
        if let Some(request) = request {
            return self.commit_vote(state, request_id, approve, reason, request);
        }

        state.0.insert(
            request_id,
            RequestState::WaitingForRequest {
                approve,
                reason,
                deadline,
            },
        );
        (state, Vec::new())
    }

    /// Starts voting on an open request by locking a bond behind a blind
    /// commitment.
    fn commit_vote(
        &self,
        mut state: State,
        request_id: B256,
        approve: bool,
        reason: String,
        request: Request,
    ) -> (State, Commands<State, Self>) {
        let Request {
            bond_target,
            slash_amount,
            commit_deadline,
            reveal_deadline,
        } = request;
        let salt = self.signer.reveal_salt(request_id);
        let hash = commit_hash(self.signer.address(), request_id, approve, salt, &reason);
        state.0.insert(
            request_id,
            RequestState::CollectingCommitments {
                approve,
                reason,
                slash_amount,
                commit_deadline,
                reveal_deadline,
                committed_count: 0,
                self_committed: false,
            },
        );
        let actions = vec![
            SentinelAction {
                kind: SentinelActionKind::ApproveToken {
                    bond: U256::from(bond_target),
                },
                expires_at: Some(commit_deadline),
            }
            .into(),
            SentinelAction {
                kind: SentinelActionKind::Commit {
                    id: request_id,
                    hash,
                },
                expires_at: Some(commit_deadline),
            }
            .into(),
        ];
        (state, actions)
    }

    /// Retains a newly opened request while its engine check is outstanding,
    /// or begins voting immediately if the decision is already available.
    fn handle_new_request(
        &self,
        mut state: State,
        event: SentinelOracle::NewRequest,
    ) -> (State, Commands<State, Self>) {
        let request_id = event.requestId;
        let request = Request {
            bond_target: event.bondTarget,
            slash_amount: event.slashAmount,
            commit_deadline: event.commitDeadline,
            reveal_deadline: event.revealDeadline,
        };
        match state.0.remove(&request_id) {
            Some(RequestState::WaitingForRequest {
                approve, reason, ..
            }) => self.commit_vote(state, request_id, approve, reason, request),
            Some(RequestState::WaitingForEngineCheck {
                deadline,
                request: None,
            }) => {
                state.0.insert(
                    request_id,
                    RequestState::WaitingForEngineCheck {
                        deadline,
                        request: Some(request),
                    },
                );
                (state, Vec::new())
            }
            Some(entry) => {
                tracing::warn!(
                    %request_id,
                    state = entry.name(),
                    "ignoring unexpected new request"
                );
                state.0.insert(request_id, entry);
                (state, Vec::new())
            }
            None => {
                tracing::debug!(%request_id, "ignoring new request for an untracked proposal");
                (state, Vec::new())
            }
        }
    }

    /// Tallies a commitment landing onchain, from any sentinel, for a
    /// request we're still collecting commits for.
    fn handle_committed(
        &self,
        mut state: State,
        event: SentinelOracle::Committed,
    ) -> (State, Commands<State, Self>) {
        let Some(entry) = state.0.get_mut(&event.requestId) else {
            tracing::debug!(
                request_id = %event.requestId,
                "ignoring commitment for an untracked request"
            );
            return (state, Vec::new());
        };
        let RequestState::CollectingCommitments {
            committed_count,
            self_committed,
            ..
        } = entry
        else {
            tracing::warn!(
                request_id = %event.requestId,
                state = entry.name(),
                "ignoring unexpected commitment"
            );
            return (state, Vec::new());
        };
        *committed_count += 1;
        // Guarded on the false-to-true edge (rather than just
        // `event.sentinel == self.signer.address()`) so a re-delivered
        // `Committed` log (e.g. replayed after a reorg restores this exact
        // block range) can't double-count our own participation.
        if !*self_committed && event.sentinel == self.signer.address() {
            *self_committed = true;
            crate::metrics::requests_participated_total().increment(1);
            crate::metrics::bond_amount().record(event.bondAmount.to::<u128>() as f64);
        }
        (state, Vec::new())
    }

    /// Tallies a reveal landing onchain, from any sentinel, and
    /// early-finalizes once every commit has been revealed.
    fn handle_revealed(
        &self,
        mut state: State,
        event: SentinelOracle::Revealed,
    ) -> (State, Commands<State, Self>) {
        let Some(entry) = state.0.get_mut(&event.requestId) else {
            tracing::debug!(
                request_id = %event.requestId,
                "ignoring reveal for an untracked request"
            );
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
            tracing::warn!(
                request_id = %event.requestId,
                state = entry.name(),
                "ignoring unexpected reveal"
            );
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
    fn handle_block_advance(&self, mut state: State, block: u64) -> (State, Commands<State, Self>) {
        let mut actions = Vec::new();

        state.0.retain(|id, entry| match entry {
            RequestState::WaitingForEngineCheck { deadline, request } => {
                block
                    <= request
                        .as_ref()
                        .map_or(*deadline, |request| request.commit_deadline)
            }
            RequestState::WaitingForRequest { deadline, .. } => block <= *deadline,
            RequestState::CollectingCommitments {
                approve,
                reason,
                slash_amount,
                commit_deadline,
                reveal_deadline,
                committed_count,
                self_committed,
            } => {
                if block <= *commit_deadline {
                    return true;
                }
                // Our own commit never landed onchain, so revealing would
                // just revert; drop the request instead.
                if !*self_committed {
                    return false;
                }
                let approve = *approve;
                let slash_amount = *slash_amount;
                let reveal_deadline = *reveal_deadline;
                let committed_count = *committed_count;
                // `CollectingVotes` has no `reason` field of its own, so this is the
                // last use of it — take it rather than cloning.
                let reason = std::mem::take(reason);
                let salt = self.signer.reveal_salt(*id);
                actions.push(
                    SentinelAction {
                        kind: SentinelActionKind::Reveal {
                            id: *id,
                            approve,
                            salt,
                            reason,
                        },
                        expires_at: Some(reveal_deadline),
                    }
                    .into(),
                );
                *entry = RequestState::CollectingVotes {
                    approve,
                    slash_amount,
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
                if block <= *reveal_deadline {
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
    /// `WaitingForDisputeResolution` — by always claiming, regardless of
    /// which side won: bond slashing is partial, so even a losing vote can
    /// leave an unslashed remainder (`bondTarget - slashAmount`) to reclaim,
    /// and `claim()` pays out `0` extra on the losing side without reverting.
    ///
    /// Also records whether this sentinel's own revealed vote matched the
    /// arbitrated `outcome` (`resolveDispute` only ever resolves to
    /// `RESOLVED_APPROVED`/`RESOLVED_DENIED`, never `TIMED_OUT`) and, if it
    /// lost, how much of its bond `slash_amount` says was slashed for it.
    fn handle_resolved(
        &self,
        mut state: State,
        event: SentinelOracle::DisputeResolved,
    ) -> (State, Commands<State, Self>) {
        let (approve, slash_amount) = match state.0.remove(&event.requestId) {
            Some(RequestState::WaitingForDisputeResolution {
                approve,
                slash_amount,
            }) => (approve, slash_amount),
            Some(entry) => {
                tracing::warn!(
                    request_id = %event.requestId,
                    state = entry.name(),
                    "ignoring unexpected dispute resolution"
                );
                state.0.insert(event.requestId, entry);
                return (state, Vec::new());
            }
            None => {
                tracing::debug!(
                    request_id = %event.requestId,
                    "ignoring dispute resolution for an untracked request"
                );
                return (state, Vec::new());
            }
        };

        let won = approve == (event.outcome == OracleRequestState::RESOLVED_APPROVED);
        if won {
            crate::metrics::requests_resolved_total(ResolvedOutcome::DisputeWon).increment(1);
        } else {
            crate::metrics::requests_resolved_total(ResolvedOutcome::DisputeLost).increment(1);
            crate::metrics::dispute_bond_slashed_amount().record(slash_amount.to::<u128>() as f64);
        }

        let actions = vec![
            SentinelAction {
                kind: SentinelActionKind::Claim {
                    id: event.requestId,
                },
                expires_at: None,
            }
            .into(),
        ];
        (state, actions)
    }

    /// A `FROZEN` request escaping arbitration without a ruling --
    /// `ArbitrationTimedOut` (permissionless, once nobody has ruled within
    /// `ARBITRATION_TIMEOUT`) or `DisputeOutOfScope` (the arbitrator
    /// declining outright). Both reuse the oracle's `TIMED_OUT` machinery
    /// (see the Solidity comments on `timeoutArbitration`/`markOutOfScope`):
    /// no side is established, so every committed bond returns in full via
    /// `claim()` -- unlike [`Self::handle_resolved`], there is no winning
    /// side and no slash to record.
    fn handle_arbitration_timeout(
        &self,
        mut state: State,
        request_id: B256,
    ) -> (State, Commands<State, Self>) {
        match state.0.remove(&request_id) {
            Some(RequestState::WaitingForDisputeResolution { .. }) => {}
            // Still claim: the oracle is authoritative that this request
            // timed out, and having any tracked entry at all means we most
            // likely posted a bond for it, even though our own local FSM
            // hadn't (yet) caught up to `WaitingForDisputeResolution`.
            Some(entry) => {
                tracing::warn!(
                    request_id = %request_id,
                    state = entry.name(),
                    "claiming despite unexpected state at arbitration timeout"
                );
            }
            None => {
                tracing::trace!(
                    request_id = %request_id,
                    "ignoring arbitration timeout for an untracked request"
                );
                return (state, Vec::new());
            }
        }

        crate::metrics::requests_resolved_total(ResolvedOutcome::Timeout).increment(1);

        let actions = vec![
            SentinelAction {
                kind: SentinelActionKind::Claim { id: request_id },
                expires_at: None,
            }
            .into(),
        ];
        (state, actions)
    }

    /// Records this sentinel's own fee reward once a `claim()` it (or
    /// anyone else) submitted lands onchain. No request state is touched:
    /// by the time a request is claimable, [`Self::finalize`]/
    /// [`Self::handle_resolved`] have already removed its tracked entry, so
    /// `feeReward` is read directly off the event rather than correlated
    /// against anything retained locally.
    fn handle_claimed(
        &self,
        state: State,
        event: SentinelOracle::Claimed,
    ) -> (State, Commands<State, Self>) {
        if event.sentinel == self.signer.address() && !event.feeReward.is_zero() {
            crate::metrics::fee_reward_amount().record(event.feeReward.to::<u128>() as f64);
        }
        (state, Vec::new())
    }

    /// Shared finalize step, reached from either the early-finalize check
    /// in [`Self::handle_revealed`] or the reveal-deadline branch in
    /// [`Self::handle_block_advance`]; always exits `CollectingVotes` in
    /// this same step, so a request's finalize step can only ever run once.
    ///
    /// There are the following cases when the `Finalize` action is emitted:
    /// - no one voted: a genuine timeout, where the bonds can be re-claimed
    /// - unanimous vote: it is possible to claim the bond and reward
    /// - a dispute: there is still a possibility to receive a reward
    ///
    /// In other cases it doesn't make sense to trigger the finalization for
    /// this sentinel.
    fn finalize(
        &self,
        state: &RequestState,
        request_id: B256,
    ) -> (Option<RequestState>, Commands<State, Self>) {
        let RequestState::CollectingVotes {
            approve,
            slash_amount,
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
        let slash_amount = *slash_amount;
        let dispute = *approve_count > 0 && *deny_count > 0;
        let timed_out = *revealed_count == 0;

        // If this sentinel did not participate and it was not a timeout
        // then no actions should be taken and the request should be dropped
        if !*self_revealed && !timed_out {
            return (None, Vec::new());
        }

        let mut actions = vec![
            SentinelAction {
                kind: SentinelActionKind::Finalize { id: request_id },
                expires_at: None,
            }
            .into(),
        ];

        // In case of a dispute it is not a timeout, so this sentinel participated.
        // Finalize the request and wait for a dispute resolution by the arbitrator;
        // `handle_resolved` records the win/loss metric once that arrives.
        if dispute {
            return (
                Some(RequestState::WaitingForDisputeResolution {
                    approve,
                    slash_amount,
                }),
                actions,
            );
        }

        // Unanimity plus our own counted vote guarantees this sentinel is on the
        // sole, winning side; no `DisputeResolved` round trip needed.
        let outcome_metric = if timed_out {
            ResolvedOutcome::Timeout
        } else {
            ResolvedOutcome::Unanimous
        };
        crate::metrics::requests_resolved_total(outcome_metric).increment(1);
        actions.push(
            SentinelAction {
                kind: SentinelActionKind::Claim { id: request_id },
                expires_at: None,
            }
            .into(),
        );
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
            SentinelActionKind::Reveal {
                id,
                approve,
                salt,
                reason,
            } => Transaction {
                to: self.oracle,
                value: U256::ZERO,
                data: SentinelOracle::revealCall {
                    requestId: id,
                    approve,
                    salt,
                    reason,
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
    type Effect = effect::Effect;
    type Resume = effect::Resume;

    fn apply_transition(
        &self,
        state: State,
        message: Message<Self::Event, Self::Resume>,
    ) -> (State, Commands<State, Self>) {
        match message {
            Message::NewBlock(block) => self.handle_block_advance(state, block),
            Message::Event(event) => {
                let block = event.block;
                match event.data {
                    SentinelEvents::Consensus(Consensus::ConsensusEvents::TransactionProposed(
                        event,
                    )) => self.handle_oracle_transaction_proposed(state, block, event),
                    SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::NewRequest(
                        event,
                    )) => self.handle_new_request(state, event),
                    SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::Committed(
                        event,
                    )) => self.handle_committed(state, event),
                    SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::Revealed(
                        event,
                    )) => self.handle_revealed(state, event),
                    SentinelEvents::Oracle(
                        SentinelOracle::SentinelOracleEvents::DisputeResolved(event),
                    ) => self.handle_resolved(state, event),
                    SentinelEvents::Oracle(
                        SentinelOracle::SentinelOracleEvents::ArbitrationTimedOut(event),
                    ) => self.handle_arbitration_timeout(state, event.requestId),
                    SentinelEvents::Oracle(
                        SentinelOracle::SentinelOracleEvents::DisputeOutOfScope(event),
                    ) => self.handle_arbitration_timeout(state, event.requestId),
                    SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::Claimed(
                        event,
                    )) => self.handle_claimed(state, event),
                }
            }
            Message::Resume(effect::Resume::EngineCheckResult {
                request_id,
                outcome,
            }) => self.handle_engine_check_result(state, request_id, outcome),
        }
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
    type Action = SentinelAction;
    type Effect = effect::Effect;
    type Resume = effect::Resume;

    type Transition = SentinelTransition;
    type Effects = effect::Handler;
    type Actions = SentinelEncoder;

    fn components(self) -> (Self::Transition, Self::Effects, Self::Actions) {
        let SentinelService {
            oracle,
            fee_token,
            consensus,
            signer,
            chain_id,
            voting_window,
            engine,
            engine_timeout,
        } = self;
        (
            SentinelTransition {
                oracle,
                consensus,
                signer,
                chain_id,
                voting_window,
            },
            effect::Handler::new(engine, engine_timeout),
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
    use crate::{
        bindings::{
            consensus::{Operation, SafeTransaction},
            oracle::RequestState as OnchainRequestState,
        },
        engine::RuleId,
    };
    use alloy::{
        primitives::{Bytes, Uint, address, aliases::U96, keccak256},
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
    const ENGINE_TIMEOUT: Duration = Duration::from_millis(7_500);
    /// The reason attached to an engine-approved transaction.
    const REASON: &str = "";

    fn self_signer() -> Signer {
        Signer::new(SigningKey::from_bytes(&keccak256("sentinel-flow-test-key").0.into()).unwrap())
    }

    fn self_address() -> Address {
        self_signer().address()
    }

    fn service() -> SentinelService {
        // These flow tests drive `Message::Resume` themselves (see
        // `resolve_engine_check`) rather than through the `Handler`'s real
        // `Effect::EngineCheck` resolution, so the configured engine is
        // never invoked.
        SentinelService::new(
            ORACLE,
            FEE_TOKEN,
            CONSENSUS,
            self_signer(),
            U256::from(CHAIN_ID),
            VOTING_WINDOW,
            // Configure an engine for an invalid URL, all checks come back
            // as `Unknown`.
            EngineClient::new("http://127.0.0.1:1".parse().unwrap()).unwrap(),
            ENGINE_TIMEOUT,
        )
    }

    fn transition() -> SentinelTransition {
        service().components().0
    }

    fn safe_tx(to: Address) -> SafeTransaction {
        SafeTransaction {
            safe: SAFE,
            to,
            operation: Operation::CALL,
            ..Default::default()
        }
    }

    fn request_id(safe_tx_hash: B256, epoch: u64, oracle: Address) -> B256 {
        oracle_tx_proposal_hash(
            U256::from(CHAIN_ID),
            CONSENSUS,
            epoch,
            oracle,
            Bytes::new(),
            safe_tx_hash,
        )
    }

    /// Mirrors `SafeId.create` in `contracts/src/libraries/SafeId.sol`: the chain ID occupies the
    /// upper 96 bits and the address the lower 160 bits of the resulting `bytes32`.
    fn safe_id(chain_id: u64, safe: Address) -> B256 {
        let value: U256 = U256::from(chain_id) << 160 | U256::from_be_bytes(safe.into_word().0);
        B256::from(value.to_be_bytes::<32>())
    }

    fn proposed_event(oracle: Address, safe_tx_hash: B256, to: Address) -> SentinelEvents {
        SentinelEvents::Consensus(Consensus::ConsensusEvents::TransactionProposed(
            Consensus::TransactionProposed {
                safeTxHash: safe_tx_hash,
                safeId: safe_id(CHAIN_ID, SAFE),
                oracle,
                epoch: 7,
                oracleData: Bytes::new(),
                transaction: safe_tx(to),
            },
        ))
    }

    /// The engine-check effect emitted for a proposal for `(id, to)` at
    /// `block`.
    fn engine_check_effect(
        id: B256,
        to: Address,
        block: u64,
    ) -> Command<SentinelAction, effect::Effect> {
        Command::Effect(effect::Effect::EngineCheck {
            request_id: id,
            transaction: safe_tx(to),
            block,
        })
    }

    /// Resolves the outstanding `Effect::EngineCheck` for `id` with
    /// `outcome`, exactly as `TransitionBatch` would perform and resume it
    /// inline within the same event. The flow tests below drive
    /// `SentinelTransition` directly rather than through the full
    /// `StateMachine`, so this simulates that resolution step for them.
    fn resolve_engine_check(
        svc: &SentinelTransition,
        state: State,
        id: B256,
        outcome: CheckOutcome,
    ) -> (State, Commands<State, SentinelTransition>) {
        svc.apply_transition(
            state,
            Message::Resume(effect::Resume::EngineCheckResult {
                request_id: id,
                outcome,
            }),
        )
    }

    fn new_request_event(
        id: B256,
        fee: U256,
        bond_target: U256,
        slash_amount: U256,
        commit_deadline: u64,
        reveal_deadline: u64,
    ) -> SentinelEvents {
        SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::NewRequest(
            SentinelOracle::NewRequest {
                requestId: id,
                sponsor: SAFE,
                fee: fee.to(),
                bondTarget: bond_target.to(),
                slashAmount: slash_amount.to(),
                commitDeadline: commit_deadline,
                revealDeadline: reveal_deadline,
            },
        ))
    }

    fn committed_event(id: B256, sentinel: Address, bond: u64) -> SentinelEvents {
        SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::Committed(
            SentinelOracle::Committed {
                requestId: id,
                sentinel,
                bondAmount: Uint::from(bond),
            },
        ))
    }

    fn revealed_event(id: B256, sentinel: Address, approved: bool, bond: u64) -> SentinelEvents {
        SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::Revealed(
            SentinelOracle::Revealed {
                requestId: id,
                sentinel,
                approved,
                bondAmount: Uint::from(bond),
                reason: String::new(),
            },
        ))
    }

    fn dispute_resolved_event(
        id: B256,
        outcome: OnchainRequestState,
        slashed: U256,
    ) -> SentinelEvents {
        SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::DisputeResolved(
            SentinelOracle::DisputeResolved {
                requestId: id,
                outcome,
                slashed: slashed.to(),
                context: String::new(),
            },
        ))
    }

    fn arbitration_timed_out_event(id: B256) -> SentinelEvents {
        SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::ArbitrationTimedOut(
            SentinelOracle::ArbitrationTimedOut { requestId: id },
        ))
    }

    fn dispute_out_of_scope_event(id: B256) -> SentinelEvents {
        SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::DisputeOutOfScope(
            SentinelOracle::DisputeOutOfScope {
                requestId: id,
                context: String::new(),
            },
        ))
    }

    fn claimed_event(
        id: B256,
        sentinel: Address,
        bond_return: u64,
        fee_reward: u64,
    ) -> SentinelEvents {
        SentinelEvents::Oracle(SentinelOracle::SentinelOracleEvents::Claimed(
            SentinelOracle::Claimed {
                requestId: id,
                sentinel,
                bondReturn: Uint::from(bond_return),
                feeReward: Uint::from(fee_reward),
            },
        ))
    }

    fn log(block: u64, data: SentinelEvents) -> EventLog<SentinelEvents> {
        EventLog {
            block,
            index: 0,
            address: Address::ZERO,
            data,
        }
    }

    fn assert_new_request_before_engine_check(
        safe_tx_hash: B256,
        outcome: CheckOutcome,
        approve: bool,
        reason: &str,
    ) {
        let svc = transition();
        let id = request_id(safe_tx_hash, 7, ORACLE);
        let bond_target = U96::from(500);

        let (state, commands) = svc.apply_transition(
            State::default(),
            Message::Event(log(1, proposed_event(ORACLE, safe_tx_hash, TO))),
        );
        assert_eq!(commands, vec![engine_check_effect(id, TO, 1)]);

        let (state, commands) = svc.apply_transition(
            state,
            Message::Event(log(
                2,
                new_request_event(
                    id,
                    U256::from(1_000u64),
                    U256::from(bond_target),
                    U256::from(bond_target),
                    20,
                    40,
                ),
            )),
        );
        assert!(commands.is_empty());
        assert_eq!(
            state.0[&id],
            RequestState::WaitingForEngineCheck {
                deadline: 1 + VOTING_WINDOW,
                request: Some(Request {
                    bond_target,
                    slash_amount: bond_target,
                    commit_deadline: 20,
                    reveal_deadline: 40,
                }),
            },
        );

        let (state, commands) = resolve_engine_check(&svc, state, id, outcome);
        assert_eq!(
            state.0[&id],
            RequestState::CollectingCommitments {
                approve,
                reason: reason.to_string(),
                slash_amount: bond_target,
                commit_deadline: 20,
                reveal_deadline: 40,
                committed_count: 0,
                self_committed: false,
            },
        );
        let hash = commit_hash(
            self_address(),
            id,
            approve,
            self_signer().reveal_salt(id),
            reason,
        );
        assert_eq!(
            commands,
            vec![
                SentinelAction {
                    kind: SentinelActionKind::ApproveToken {
                        bond: U256::from(bond_target),
                    },
                    expires_at: Some(20),
                }
                .into(),
                SentinelAction {
                    kind: SentinelActionKind::Commit { id, hash },
                    expires_at: Some(20),
                }
                .into(),
            ],
        );
    }

    /// Full happy path: propose, commit (from two sentinels), reveal (from
    /// two sentinels) — unanimously in favor — and finalize/claim as soon as
    /// the last reveal lands, without waiting out the reveal window.
    #[test]
    fn flow_unanimous_approve_finalizes_via_early_reveal_and_claims() {
        let svc = transition();
        let safe_tx_hash = B256::repeat_byte(0x01);
        let id = request_id(safe_tx_hash, 7, ORACLE);

        // The transaction is proposed onchain; the engine check is emitted
        // and its outstanding state is tracked explicitly.
        let (state, commands) = svc.apply_transition(
            State::default(),
            Message::Event(log(1, proposed_event(ORACLE, safe_tx_hash, TO))),
        );
        assert_eq!(commands, vec![engine_check_effect(id, TO, 1)]);
        assert_eq!(
            state.0[&id],
            RequestState::WaitingForEngineCheck {
                deadline: 1 + VOTING_WINDOW,
                request: None,
            },
        );

        // The engine check approves; the provisional decision becomes final.
        let (state, commands) = resolve_engine_check(&svc, state, id, CheckOutcome::Approved);
        assert_eq!(
            state.0[&id],
            RequestState::WaitingForRequest {
                approve: true,
                reason: REASON.to_string(),
                deadline: 1 + VOTING_WINDOW,
            },
        );
        assert_eq!(commands, []);

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
                reason: REASON.to_string(),
                deadline: 1 + VOTING_WINDOW,
            },
        );

        // The request is opened onchain: we lock a bond behind a blind
        // commitment hash.
        let (state, commands) = svc.apply_transition(
            state,
            Message::Event(log(
                5,
                new_request_event(
                    id,
                    U256::from(1_000u64),
                    U256::from(500u64),
                    U256::from(500u64),
                    20,
                    40,
                ),
            )),
        );
        let salt = self_signer().reveal_salt(id);
        let hash = commit_hash(self_address(), id, true, salt, REASON);
        assert_eq!(
            state.0[&id],
            RequestState::CollectingCommitments {
                approve: true,
                reason: REASON.to_string(),
                slash_amount: U96::from(500),
                commit_deadline: 20,
                reveal_deadline: 40,
                committed_count: 0,
                self_committed: false,
            },
        );
        assert_eq!(
            commands,
            vec![
                SentinelAction {
                    kind: SentinelActionKind::ApproveToken {
                        bond: U256::from(500u64)
                    },
                    expires_at: Some(20),
                }
                .into(),
                SentinelAction {
                    kind: SentinelActionKind::Commit { id, hash },
                    expires_at: Some(20),
                }
                .into(),
            ],
        );

        // Our own commit lands onchain, followed by the other sentinel's.
        let (state, commands) = svc.apply_transition(
            state,
            Message::Event(log(6, committed_event(id, self_address(), 500u64))),
        );
        assert!(commands.is_empty());
        let (state, commands) = svc.apply_transition(
            state,
            Message::Event(log(7, committed_event(id, OTHER, 500u64))),
        );
        assert!(commands.is_empty());
        assert_eq!(
            state.0[&id],
            RequestState::CollectingCommitments {
                approve: true,
                reason: REASON.to_string(),
                slash_amount: U96::from(500),
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
            vec![
                SentinelAction {
                    kind: SentinelActionKind::Reveal {
                        id,
                        approve: true,
                        salt,
                        reason: REASON.to_string(),
                    },
                    expires_at: Some(40),
                }
                .into(),
            ],
        );
        assert_eq!(
            state.0[&id],
            RequestState::CollectingVotes {
                approve: true,
                slash_amount: U96::from(500),
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
            Message::Event(log(22, revealed_event(id, OTHER, true, 500u64))),
        );
        assert!(commands.is_empty());
        assert_eq!(
            state.0[&id],
            RequestState::CollectingVotes {
                approve: true,
                slash_amount: U96::from(500),
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
            Message::Event(log(23, revealed_event(id, self_address(), true, 500u64))),
        );
        assert!(!state.0.contains_key(&id));
        assert_eq!(
            commands,
            vec![
                SentinelAction {
                    kind: SentinelActionKind::Finalize { id },
                    expires_at: None,
                }
                .into(),
                SentinelAction {
                    kind: SentinelActionKind::Claim { id },
                    expires_at: None,
                }
                .into(),
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
        let (state, _) = resolve_engine_check(&svc, state, id, CheckOutcome::Approved);
        let (state, _) = svc.apply_transition(
            state,
            Message::Event(log(
                5,
                new_request_event(
                    id,
                    U256::from(1_000u64),
                    U256::from(500u64),
                    U256::from(500u64),
                    20,
                    40,
                ),
            )),
        );
        let (state, _) = svc.apply_transition(
            state,
            Message::Event(log(6, committed_event(id, self_address(), 500u64))),
        );
        let (state, _) = svc.apply_transition(
            state,
            Message::Event(log(7, committed_event(id, OTHER, 500u64))),
        );
        let (state, _) = svc.apply_transition(state, Message::NewBlock(21));

        // The other sentinel reveals the opposite vote, and our own reveal
        // lands last: unanimity fails, so this is a genuine dispute.
        let (state, _) = svc.apply_transition(
            state,
            Message::Event(log(22, revealed_event(id, OTHER, false, 500u64))),
        );
        let (state, commands) = svc.apply_transition(
            state,
            Message::Event(log(23, revealed_event(id, self_address(), true, 500u64))),
        );

        assert_eq!(
            commands,
            vec![
                SentinelAction {
                    kind: SentinelActionKind::Finalize { id },
                    expires_at: None,
                }
                .into(),
            ],
        );
        assert_eq!(
            state.0[&id],
            RequestState::WaitingForDisputeResolution {
                approve: true,
                slash_amount: U96::from(500),
            }
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
            vec![
                SentinelAction {
                    kind: SentinelActionKind::Claim { id },
                    expires_at: None,
                }
                .into(),
            ],
        );
    }

    /// Bond slashing is partial, so a losing vote still leaves an unslashed
    /// remainder to reclaim — `handle_resolved` must claim unconditionally,
    /// not only when our own revealed vote matches the arbitration outcome.
    #[test]
    fn flow_dispute_still_claims_when_arbitration_contradicts_our_vote() {
        let (svc, id, state) = setup_dispute();
        let event = dispute_resolved_event(id, OnchainRequestState::RESOLVED_DENIED, U256::ZERO);

        let (state, commands) = svc.apply_transition(state, Message::Event(log(50, event)));

        assert!(!state.0.contains_key(&id));
        assert_eq!(
            commands,
            vec![
                SentinelAction {
                    kind: SentinelActionKind::Claim { id },
                    expires_at: None,
                }
                .into(),
            ],
        );
    }

    /// `timeoutArbitration()` -- nobody rules within `ARBITRATION_TIMEOUT` --
    /// reuses the oracle's `TIMED_OUT` machinery, so it must claim just like
    /// a genuine `DisputeResolved`, even though there's no winning side.
    #[test]
    fn flow_claims_on_arbitration_timeout() {
        let (svc, id, state) = setup_dispute();
        let event = arbitration_timed_out_event(id);

        let (state, commands) = svc.apply_transition(state, Message::Event(log(50, event)));

        assert!(!state.0.contains_key(&id));
        assert_eq!(
            commands,
            vec![
                SentinelAction {
                    kind: SentinelActionKind::Claim { id },
                    expires_at: None,
                }
                .into(),
            ],
        );
    }

    /// `markOutOfScope` -- the arbitrator declining outright -- reuses the
    /// same `TIMED_OUT` machinery as `timeoutArbitration`, so it must also
    /// claim unconditionally.
    #[test]
    fn flow_claims_on_dispute_out_of_scope() {
        let (svc, id, state) = setup_dispute();
        let event = dispute_out_of_scope_event(id);

        let (state, commands) = svc.apply_transition(state, Message::Event(log(50, event)));

        assert!(!state.0.contains_key(&id));
        assert_eq!(
            commands,
            vec![
                SentinelAction {
                    kind: SentinelActionKind::Claim { id },
                    expires_at: None,
                }
                .into(),
            ],
        );
    }

    /// `Claimed` carries no request state to correlate against (the tracked
    /// entry is already gone by the time a request is claimable), so it
    /// must be a pure pass-through regardless of whose claim it is.
    #[test]
    fn claimed_event_never_mutates_state() {
        let svc = transition();
        let id = B256::repeat_byte(0x0f);
        let mut state = State::default();
        state.0.insert(
            id,
            RequestState::WaitingForRequest {
                approve: true,
                reason: String::new(),
                deadline: 10,
            },
        );

        let (after_self, commands) = svc.apply_transition(
            state.clone(),
            Message::Event(log(1, claimed_event(id, self_address(), 500, 100))),
        );
        assert!(commands.is_empty());
        assert_eq!(after_self, state);

        let (after_other, commands) = svc.apply_transition(
            state.clone(),
            Message::Event(log(1, claimed_event(id, OTHER, 500, 100))),
        );
        assert!(commands.is_empty());
        assert_eq!(after_other, state);
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
        let (state, _) = resolve_engine_check(&svc, state, id, CheckOutcome::Approved);
        let (state, _) = svc.apply_transition(
            state,
            Message::Event(log(
                5,
                new_request_event(
                    id,
                    U256::from(1_000u64),
                    U256::from(500u64),
                    U256::from(500u64),
                    20,
                    40,
                ),
            )),
        );
        let (state, _) = svc.apply_transition(
            state,
            Message::Event(log(6, committed_event(id, self_address(), 500u64))),
        );

        let salt = self_signer().reveal_salt(id);
        let (state, commands) = svc.apply_transition(state, Message::NewBlock(21));
        assert_eq!(
            commands,
            vec![
                SentinelAction {
                    kind: SentinelActionKind::Reveal {
                        id,
                        approve: true,
                        salt,
                        reason: REASON.to_string(),
                    },
                    expires_at: Some(40),
                }
                .into(),
            ],
        );

        // Our own reveal transaction never confirms onchain, and neither
        // does anyone else's.
        let (state, commands) = svc.apply_transition(state, Message::NewBlock(41));

        assert!(!state.0.contains_key(&id));
        assert_eq!(
            commands,
            vec![
                SentinelAction {
                    kind: SentinelActionKind::Finalize { id },
                    expires_at: None,
                }
                .into(),
                SentinelAction {
                    kind: SentinelActionKind::Claim { id },
                    expires_at: None,
                }
                .into(),
            ],
        );
    }

    /// A resume without a matching outstanding engine check can arrive after
    /// a reorg restored a snapshot from before its proposal. It must not create
    /// state for the orphaned request.
    #[test]
    fn stale_engine_check_resume_after_reorg_is_ignored() {
        let svc = transition();
        let id = B256::repeat_byte(0x09);

        let (state, commands) =
            resolve_engine_check(&svc, State::default(), id, CheckOutcome::Approved);

        assert!(commands.is_empty());
        assert!(!state.0.contains_key(&id));
    }

    /// A denying engine check finalizes the provisional request into
    /// `WaitingForRequest` citing the remote outcome's `RuleId`.
    #[test]
    fn engine_check_denial_finalizes_with_the_engine_rule() {
        let svc = transition();
        let safe_tx_hash = B256::repeat_byte(0x06);
        let id = request_id(safe_tx_hash, 7, ORACLE);

        let (state, commands) = svc.apply_transition(
            State::default(),
            Message::Event(log(1, proposed_event(ORACLE, safe_tx_hash, TO))),
        );
        assert_eq!(commands, vec![engine_check_effect(id, TO, 1)]);

        let rule = RuleId::new(4, 6);
        let (state, commands) = resolve_engine_check(&svc, state, id, CheckOutcome::Denied(rule));
        assert_eq!(
            state.0[&id],
            RequestState::WaitingForRequest {
                approve: false,
                reason: rule.to_string(),
                deadline: 1 + VOTING_WINDOW,
            },
        );
        assert_eq!(commands, []);

        let (state, commands) = svc.apply_transition(
            state,
            Message::Event(log(
                5,
                new_request_event(
                    id,
                    U256::from(1_000u64),
                    U256::from(500u64),
                    U256::from(500u64),
                    20,
                    40,
                ),
            )),
        );
        let reason = RuleId::new(4, 6).to_string();
        assert_eq!(
            state.0[&id],
            RequestState::CollectingCommitments {
                approve: false,
                reason: reason.clone(),
                slash_amount: U96::from(500),
                commit_deadline: 20,
                reveal_deadline: 40,
                committed_count: 0,
                self_committed: false,
            },
        );
        let hash = commit_hash(
            self_address(),
            id,
            false,
            self_signer().reveal_salt(id),
            &reason,
        );
        assert_eq!(
            commands,
            vec![
                SentinelAction {
                    kind: SentinelActionKind::ApproveToken {
                        bond: U256::from(500u64),
                    },
                    expires_at: Some(20),
                }
                .into(),
                SentinelAction {
                    kind: SentinelActionKind::Commit { id, hash },
                    expires_at: Some(20),
                }
                .into(),
            ],
        );
    }

    #[test]
    fn new_request_before_engine_check_approval_starts_voting_on_resume() {
        assert_new_request_before_engine_check(
            B256::repeat_byte(0x0a),
            CheckOutcome::Approved,
            true,
            REASON,
        );
    }

    #[test]
    fn new_request_before_engine_check_denial_starts_voting_on_resume() {
        let rule = RuleId::new(42, 1337);
        let reason = rule.to_string();
        assert_new_request_before_engine_check(
            B256::repeat_byte(0x0b),
            CheckOutcome::Denied(rule),
            false,
            &reason,
        );
    }

    /// An unreachable/malfunctioning engine check must not be guessed at
    /// either way — the request is dropped unanswered instead of voting.
    #[test]
    fn engine_check_failure_drops_the_request_unanswered() {
        let svc = transition();
        let safe_tx_hash = B256::repeat_byte(0x07);
        let id = request_id(safe_tx_hash, 7, ORACLE);

        let (state, _) = svc.apply_transition(
            State::default(),
            Message::Event(log(1, proposed_event(ORACLE, safe_tx_hash, TO))),
        );
        assert_eq!(
            state.0[&id],
            RequestState::WaitingForEngineCheck {
                deadline: 1 + VOTING_WINDOW,
                request: None,
            },
        );

        let (state, _) = resolve_engine_check(&svc, state, id, CheckOutcome::Unknown);
        assert!(!state.0.contains_key(&id));
    }

    #[test]
    fn engine_check_failure_drops_a_stored_onchain_request() {
        let svc = transition();
        let safe_tx_hash = B256::repeat_byte(0x0c);
        let id = request_id(safe_tx_hash, 7, ORACLE);

        let (state, _) = svc.apply_transition(
            State::default(),
            Message::Event(log(1, proposed_event(ORACLE, safe_tx_hash, TO))),
        );
        let (state, commands) = svc.apply_transition(
            state,
            Message::Event(log(
                2,
                new_request_event(
                    id,
                    U256::from(1_000u64),
                    U256::from(500u64),
                    U256::from(500u64),
                    20,
                    40,
                ),
            )),
        );
        assert!(commands.is_empty());

        let (state, commands) = resolve_engine_check(&svc, state, id, CheckOutcome::Unknown);

        assert!(commands.is_empty());
        assert!(!state.0.contains_key(&id));
    }

    #[test]
    fn waiting_engine_checks_expire_using_the_available_deadline() {
        let svc = transition();
        let proposed_only = B256::repeat_byte(0x0d);
        let request_open = B256::repeat_byte(0x0e);
        let mut state = State::default();
        state.0.insert(
            proposed_only,
            RequestState::WaitingForEngineCheck {
                deadline: 10,
                request: None,
            },
        );
        state.0.insert(
            request_open,
            RequestState::WaitingForEngineCheck {
                deadline: 10,
                request: Some(Request {
                    bond_target: U96::from(500),
                    slash_amount: U96::from(500),
                    commit_deadline: 20,
                    reveal_deadline: 40,
                }),
            },
        );

        let (state, commands) = svc.apply_transition(state, Message::NewBlock(11));
        assert!(commands.is_empty());
        assert!(!state.0.contains_key(&proposed_only));
        assert!(state.0.contains_key(&request_open));

        let (state, commands) = svc.apply_transition(state, Message::NewBlock(21));
        assert!(commands.is_empty());
        assert!(!state.0.contains_key(&request_open));

        let (state, commands) =
            resolve_engine_check(&svc, state, request_open, CheckOutcome::Approved);
        assert!(commands.is_empty());
        assert!(!state.0.contains_key(&request_open));
    }

    /// A replayed resume for a request that already advanced past its engine
    /// check (e.g. the effect ran twice after a crash) is a no-op because it no
    /// longer has a matching `WaitingForEngineCheck` marker.
    #[test]
    fn stale_engine_check_resume_does_not_disturb_an_already_advanced_request() {
        let svc = transition();
        let safe_tx_hash = B256::repeat_byte(0x08);
        let id = request_id(safe_tx_hash, 7, ORACLE);

        let (state, _) = svc.apply_transition(
            State::default(),
            Message::Event(log(1, proposed_event(ORACLE, safe_tx_hash, TO))),
        );
        let (state, _) = resolve_engine_check(&svc, state, id, CheckOutcome::Approved);
        let advanced = state.0[&id].clone();

        let (state, _) =
            resolve_engine_check(&svc, state, id, CheckOutcome::Denied(RuleId::new(42, 1337)));
        assert_eq!(state.0[&id], advanced);
    }
}
