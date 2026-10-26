use alloy::primitives::{B256, aliases::U96};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Onchain voting data retained while an engine check is still outstanding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// Bond amount the oracle expects this sentinel to post -- `U96`, matching the onchain
    /// `uint96` field exactly, so `NewRequest.bondTarget` assigns here with no cast at all.
    pub bond_target: U96,
    /// The amount of this sentinel's own bond an arbitrated dispute loss
    /// would slash -- `NewRequest.slashAmount`, carried through to
    /// `WaitingForDisputeResolution` purely for the
    /// `safenet_sentinel_dispute_bond_slashed_amount` metric.
    pub slash_amount: U96,
    /// Last block in which a commitment can be submitted.
    pub commit_deadline: u64,
    /// Last block in which a committed vote can be revealed.
    pub reveal_deadline: u64,
}

/// Per-request state tracked by the sentinel FSM, mirroring
/// `SentinelOracleRequest.State`'s commit-reveal phases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SentinelRequestState {
    /// The proposal is waiting for its sentinel engine check to complete. If
    /// the request opens onchain first, its voting data is retained in
    /// `request` until the check resumes, and `committed_count` tallies the
    /// other sentinels' `Committed` events that land in the meantime, so
    /// `CollectingCommitments` starts from the onchain count. None of them
    /// can be ours: we only commit once the check has completed.
    WaitingForEngineCheck {
        deadline: u64,
        request: Option<Request>,
        #[serde(default)]
        committed_count: u64,
    },
    /// Our vote intent is decided, but the oracle hasn't opened the request
    /// for voting yet. `deadline` is our own guessed cutoff, since the real
    /// `commitDeadline` isn't known until `NewRequest` arrives. `reason` is
    /// carried unchanged from the engine verdict through to the `commit_hash`
    /// call and the eventual `reveal` — it must never be re-derived.
    WaitingForRequest {
        approve: bool,
        reason: String,
        deadline: u64,
    },
    /// The request exists onchain and commits are being collected.
    /// `committed_count` tallies every `Committed` event, from any
    /// sentinel; `self_committed` tracks whether ours landed among them.
    /// `reason` is the same value carried from `WaitingForRequest`.
    CollectingCommitments {
        approve: bool,
        reason: String,
        slash_amount: U96,
        commit_deadline: u64,
        reveal_deadline: u64,
        committed_count: u64,
        self_committed: bool,
    },
    /// The commit window has closed and reveals are being collected.
    /// `committed_count` is the snapshot carried over from the previous
    /// phase (no more commits are possible once this phase is entered);
    /// `revealed_count`/`approve_count`/`deny_count` tally every `Revealed`
    /// event the same way `committed_count` tallied `Committed`.
    CollectingVotes {
        approve: bool,
        slash_amount: U96,
        reveal_deadline: u64,
        committed_count: u64,
        revealed_count: u64,
        approve_count: u64,
        deny_count: u64,
        self_revealed: bool,
    },
    /// A `Finalize` action was submitted and is awaiting onchain
    /// confirmation. No deadline: unlike every other state, advancement out
    /// of here is tied to our own `finalize()` call landing, not a block
    /// window, so `handle_block_advance` never expires it. Which of
    /// `handle_dispute_triggered`/`handle_request_timed_out`/
    /// `handle_oracle_result` fires next -- and thus which outcome this
    /// request actually reached -- is decided by the oracle's emitted event,
    /// not predicted locally; `approve`/`slash_amount` are carried through
    /// for the `DisputeTriggered` case, where they seed the next state.
    ///
    /// `self_revealed` is carried over from `CollectingVotes`: `finalize()`
    /// is submitted whether or not our own reveal landed, since the
    /// unslashed remainder of our bond is only claimable once the request
    /// is finalized. It only picks the `handle_oracle_result` metric
    /// outcome. Snapshots written before it was tracked only reached this
    /// state after our own reveal, or from a total timeout that never reads
    /// it, so it defaults to `true`.
    WaitingForOutcome {
        approve: bool,
        slash_amount: U96,
        #[serde(default = "default_true")]
        self_revealed: bool,
    },
    /// `DisputeTriggered` confirmed both sides had revealed votes onchain.
    /// `handle_resolved` always claims once the arbitrator settles it,
    /// regardless of which side won, but `approve`/`slash_amount` are kept
    /// so it can also record whether *this* sentinel's vote matched the
    /// arbitrated outcome and, if not, how much of its bond was slashed.
    ///
    /// `arbitration_deadline` is `DisputeTriggered.deadline`: once it passes
    /// without a ruling, `handle_block_advance` submits the permissionless
    /// `timeoutArbitration()` together with `claim()` and drops the request,
    /// rather than leaving the bond locked until someone else times it out.
    WaitingForDisputeResolution {
        approve: bool,
        slash_amount: U96,
        arbitration_deadline: u64,
    },
}

fn default_true() -> bool {
    true
}

impl SentinelRequestState {
    /// Returns a compact state name for diagnostics.
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::WaitingForEngineCheck { .. } => "waiting_for_engine_check",
            Self::WaitingForRequest { .. } => "waiting_for_request",
            Self::CollectingCommitments { .. } => "collecting_commitments",
            Self::CollectingVotes { .. } => "collecting_votes",
            Self::WaitingForOutcome { .. } => "waiting_for_outcome",
            Self::WaitingForDisputeResolution { .. } => "waiting_for_dispute_resolution",
        }
    }

    /// `(approve, slash_amount)` recovery for a request found in an
    /// unexpected state when one of the oracle's terminal finalize events
    /// (`DisputeTriggered`/`RequestTimedOut`/`OracleResult`) arrives instead
    /// of the expected `WaitingForOutcome` -- see
    /// `handle_dispute_triggered`/`handle_request_timed_out`/
    /// `handle_oracle_result`. Returns `None` for `WaitingForEngineCheck`/
    /// `WaitingForRequest` (our own logic never posts a bond that early --
    /// voting hasn't opened yet, or we haven't committed) and for
    /// `CollectingCommitments` while `self_committed` is still `false` (the
    /// request is open and its terms are known, but our own `Commit` hasn't
    /// landed onchain yet) -- in every one of these there is nothing of
    /// ours to claim back regardless of which event this turns out to be.
    /// `CollectingVotes` never needs the same check: `handle_block_advance`
    /// already drops a request that reaches its commit deadline without
    /// `self_committed`, so reaching `CollectingVotes` at all guarantees a
    /// real commit.
    pub(crate) fn approve_and_slash_amount(&self) -> Option<(bool, U96)> {
        match self {
            Self::WaitingForEngineCheck { .. } | Self::WaitingForRequest { .. } => None,
            Self::CollectingCommitments {
                approve,
                slash_amount,
                self_committed,
                ..
            } => self_committed.then_some((*approve, *slash_amount)),
            Self::CollectingVotes {
                approve,
                slash_amount,
                ..
            }
            | Self::WaitingForOutcome {
                approve,
                slash_amount,
                ..
            }
            | Self::WaitingForDisputeResolution {
                approve,
                slash_amount,
                ..
            } => Some((*approve, *slash_amount)),
        }
    }

    /// Whether our own committed vote is known to have been revealed
    /// onchain -- used by `handle_oracle_result` to tell a genuine win from
    /// a bond that's merely being reclaimed after our own `Reveal` never
    /// landed (see `ResolvedOutcome::RevealMissed`).
    ///
    /// `CollectingCommitments` is always `false`: reaching this at all
    /// already required `self_committed` via `approve_and_slash_amount`,
    /// but revealing only starts once `CollectingVotes` is reached, and
    /// this state hasn't gotten there yet. `WaitingForOutcome` reports the
    /// value carried over from `CollectingVotes`, since `finalize()` is
    /// submitted whether or not our own reveal landed.
    /// `WaitingForDisputeResolution` defaults to `true`, though moot in
    /// practice -- the oracle never re-emits `OracleResult` for a request
    /// that's already `FROZEN`.
    pub(crate) fn self_revealed(&self) -> bool {
        match self {
            Self::WaitingForEngineCheck { .. }
            | Self::WaitingForRequest { .. }
            | Self::CollectingCommitments { .. } => false,
            Self::CollectingVotes { self_revealed, .. }
            | Self::WaitingForOutcome { self_revealed, .. } => *self_revealed,
            Self::WaitingForDisputeResolution { .. } => true,
        }
    }
}

/// Snapshot state: every in-flight request, keyed by request ID.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct State(pub HashMap<B256, SentinelRequestState>);

#[cfg(test)]
mod tests {
    use super::*;

    fn collecting_commitments(commit_deadline: u64, reveal_deadline: u64) -> SentinelRequestState {
        SentinelRequestState::CollectingCommitments {
            approve: false,
            reason: "destination is blocklisted".to_string(),
            slash_amount: U96::from(500),
            commit_deadline,
            reveal_deadline,
            committed_count: 2,
            self_committed: true,
        }
    }

    #[test]
    fn approve_and_slash_amount_is_none_before_any_bond_could_have_been_posted() {
        assert_eq!(
            SentinelRequestState::WaitingForEngineCheck {
                deadline: 10,
                request: None,
                committed_count: 0,
            }
            .approve_and_slash_amount(),
            None,
        );
        assert_eq!(
            SentinelRequestState::WaitingForRequest {
                approve: true,
                reason: String::new(),
                deadline: 10,
            }
            .approve_and_slash_amount(),
            None,
        );
        assert_eq!(
            SentinelRequestState::CollectingCommitments {
                approve: true,
                reason: String::new(),
                slash_amount: U96::from(500),
                commit_deadline: 20,
                reveal_deadline: 40,
                committed_count: 0,
                self_committed: false,
            }
            .approve_and_slash_amount(),
            None,
        );
    }

    #[test]
    fn approve_and_slash_amount_recovers_the_real_values_once_a_bond_could_exist() {
        assert_eq!(
            collecting_commitments(20, 40).approve_and_slash_amount(),
            Some((false, U96::from(500))),
        );
        assert_eq!(
            SentinelRequestState::CollectingVotes {
                approve: true,
                slash_amount: U96::from(500),
                reveal_deadline: 40,
                committed_count: 2,
                revealed_count: 1,
                approve_count: 1,
                deny_count: 0,
                self_revealed: true,
            }
            .approve_and_slash_amount(),
            Some((true, U96::from(500))),
        );
        assert_eq!(
            SentinelRequestState::WaitingForOutcome {
                approve: true,
                slash_amount: U96::from(500),
                self_revealed: false,
            }
            .approve_and_slash_amount(),
            Some((true, U96::from(500))),
        );
        assert_eq!(
            SentinelRequestState::WaitingForDisputeResolution {
                approve: false,
                slash_amount: U96::from(500),
                arbitration_deadline: 60,
            }
            .approve_and_slash_amount(),
            Some((false, U96::from(500))),
        );
    }

    #[test]
    fn self_revealed_is_false_before_our_own_reveal_could_have_landed() {
        assert!(
            !SentinelRequestState::WaitingForEngineCheck {
                deadline: 10,
                request: None,
                committed_count: 0,
            }
            .self_revealed()
        );
        assert!(
            !SentinelRequestState::WaitingForRequest {
                approve: true,
                reason: String::new(),
                deadline: 10,
            }
            .self_revealed()
        );
        assert!(!collecting_commitments(20, 40).self_revealed());
        assert!(
            !SentinelRequestState::WaitingForOutcome {
                approve: true,
                slash_amount: U96::from(500),
                self_revealed: false,
            }
            .self_revealed()
        );
        assert!(
            !SentinelRequestState::CollectingVotes {
                approve: true,
                slash_amount: U96::from(500),
                reveal_deadline: 40,
                committed_count: 2,
                revealed_count: 1,
                approve_count: 0,
                deny_count: 1,
                self_revealed: false,
            }
            .self_revealed()
        );
    }

    #[test]
    fn self_revealed_is_true_once_our_own_reveal_is_known_to_have_landed() {
        assert!(
            SentinelRequestState::CollectingVotes {
                approve: true,
                slash_amount: U96::from(500),
                reveal_deadline: 40,
                committed_count: 2,
                revealed_count: 1,
                approve_count: 1,
                deny_count: 0,
                self_revealed: true,
            }
            .self_revealed()
        );
        assert!(
            SentinelRequestState::WaitingForOutcome {
                approve: true,
                slash_amount: U96::from(500),
                self_revealed: true,
            }
            .self_revealed()
        );
    }

    #[test]
    fn state_serde_roundtrip() {
        let id = B256::from([2u8; 32]);
        let mut state = State::default();
        state.0.insert(id, collecting_commitments(100, 150));

        let json = serde_json::to_string(&state).unwrap();
        let restored: State = serde_json::from_str(&json).unwrap();

        assert_eq!(restored, state);
    }

    /// Snapshots written before `WaitingForEngineCheck` tracked
    /// `committed_count` still load, with no commits counted.
    #[test]
    fn waiting_for_engine_check_without_committed_count_deserializes() {
        let restored: SentinelRequestState =
            serde_json::from_str(r#"{"WaitingForEngineCheck":{"deadline":10,"request":null}}"#)
                .unwrap();

        assert_eq!(
            restored,
            SentinelRequestState::WaitingForEngineCheck {
                deadline: 10,
                request: None,
                committed_count: 0,
            },
        );
    }

    /// Snapshots written before `WaitingForOutcome` tracked `self_revealed`
    /// only reached it after our own reveal (or from a total timeout that
    /// never reads it), so they load as revealed.
    #[test]
    fn waiting_for_outcome_without_self_revealed_deserializes() {
        let restored: SentinelRequestState = serde_json::from_str(
            r#"{"WaitingForOutcome":{"approve":true,"slash_amount":"0x1f4"}}"#,
        )
        .unwrap();

        assert_eq!(
            restored,
            SentinelRequestState::WaitingForOutcome {
                approve: true,
                slash_amount: U96::from(500),
                self_revealed: true,
            },
        );
    }

    #[test]
    fn state_multiple_requests_in_different_phases() {
        let checking_id = B256::from([1u8; 32]);
        let requested_id = B256::from([2u8; 32]);
        let waiting_id = B256::from([3u8; 32]);
        let collecting_id = B256::from([4u8; 32]);
        let mut state = State::default();
        state.0.insert(
            checking_id,
            SentinelRequestState::WaitingForEngineCheck {
                deadline: 10,
                request: None,
                committed_count: 0,
            },
        );
        state.0.insert(
            requested_id,
            SentinelRequestState::WaitingForEngineCheck {
                deadline: 20,
                request: Some(Request {
                    bond_target: U96::from(1_000),
                    slash_amount: U96::from(1_000),
                    commit_deadline: 30,
                    reveal_deadline: 40,
                }),
                committed_count: 0,
            },
        );
        state.0.insert(
            waiting_id,
            SentinelRequestState::WaitingForRequest {
                approve: true,
                reason: "destination is not blocklisted".to_string(),
                deadline: 10,
            },
        );
        state
            .0
            .insert(collecting_id, collecting_commitments(100, 150));

        let json = serde_json::to_string(&state).unwrap();
        let restored: State = serde_json::from_str(&json).unwrap();

        assert_eq!(restored, state);
    }
}
