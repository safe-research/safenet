use alloy::primitives::{B256, aliases::U96};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Onchain voting data retained while an engine check is still outstanding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// Bond amount the oracle expects this sentinel to post -- `U96`, matching the onchain
    /// `uint96` field exactly, so `NewRequest.bondTarget` assigns here with no cast at all.
    pub bond_target: U96,
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
    /// `request` until the check resumes.
    WaitingForEngineCheck {
        deadline: u64,
        request: Option<Request>,
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
        reveal_deadline: u64,
        committed_count: u64,
        revealed_count: u64,
        approve_count: u64,
        deny_count: u64,
        self_revealed: bool,
    },
    /// The local tally showed both sides had revealed votes (a dispute);
    /// nothing further needs tracking here, since `handle_resolved` always
    /// claims once the arbitrator settles it, regardless of which side won.
    WaitingForDisputeResolution,
}

impl SentinelRequestState {
    /// Returns a compact state name for diagnostics.
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::WaitingForEngineCheck { .. } => "waiting_for_engine_check",
            Self::WaitingForRequest { .. } => "waiting_for_request",
            Self::CollectingCommitments { .. } => "collecting_commitments",
            Self::CollectingVotes { .. } => "collecting_votes",
            Self::WaitingForDisputeResolution => "waiting_for_dispute_resolution",
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
            commit_deadline,
            reveal_deadline,
            committed_count: 2,
            self_committed: true,
        }
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
            },
        );
        state.0.insert(
            requested_id,
            SentinelRequestState::WaitingForEngineCheck {
                deadline: 20,
                request: Some(Request {
                    bond_target: U96::from(1_000),
                    commit_deadline: 30,
                    reveal_deadline: 40,
                }),
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
