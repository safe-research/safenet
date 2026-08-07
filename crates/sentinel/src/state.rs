use alloy::primitives::{B256, U256};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Onchain voting data retained while a dynamic check is still outstanding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// Bond amount the oracle expects this sentinel to post.
    pub bond_target: U256,
    /// Last block in which a commitment can be submitted.
    pub commit_deadline: u64,
    /// Last block in which a committed vote can be revealed.
    pub reveal_deadline: u64,
}

/// Per-request state tracked by the sentinel FSM, mirroring
/// `SentinelOracleRequest.State`'s commit-reveal phases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SentinelRequestState {
    /// The proposal passed the static check and is waiting for its dynamic
    /// check to complete. If the request opens onchain first, its voting data
    /// is retained in `request` until the check resumes.
    WaitingForDynamicCheck {
        deadline: u64,
        request: Option<Request>,
    },
    /// Our vote intent is decided, but the oracle hasn't opened the request
    /// for voting yet. `deadline` is our own guessed cutoff, since the real
    /// `commitDeadline` isn't known until `NewRequest` arrives. `reason` is
    /// carried unchanged from `StaticChecker::check` through to the `commit_hash`
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
    /// only our own vote needs to survive, to compare against the eventual
    /// arbitration outcome.
    WaitingForDisputeResolution { approve: bool },
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
            SentinelRequestState::WaitingForDynamicCheck {
                deadline: 10,
                request: None,
            },
        );
        state.0.insert(
            requested_id,
            SentinelRequestState::WaitingForDynamicCheck {
                deadline: 20,
                request: Some(Request {
                    bond_target: U256::from(1_000),
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
