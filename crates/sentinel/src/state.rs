use alloy::primitives::B256;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The four stages a request moves through in the sentinel FSM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RequestStatus {
    Preparing,
    Pending,
    Committed,
    Finalized,
}

/// Per-request state tracked by the sentinel FSM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SentinelRequestState {
    /// Block at which the voting window expires.
    pub deadline: u64,
    /// Whether the sentinel is voting to approve this request.
    pub approve: bool,
    pub status: RequestStatus,
}

/// Snapshot state: every in-flight request, keyed by request ID.
///
/// Serialized per block by the `StateMachine`; `B256` keys round-trip through
/// serde_json as their hex representation.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct State(pub HashMap<B256, SentinelRequestState>);

/// Per-request state tracked by the sentinel FSM.
///
/// TODO(sentinel commit-reveal, phase C2): `RequestStatus`/
/// `SentinelRequestState`/`State` above are deleted once `servicev2.rs`
/// replaces `service.rs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SentinelRequestStateV2 {
    /// Our vote intent is decided, but the oracle hasn't opened the request
    /// for voting yet. `deadline` is our own guessed cutoff, since the real
    /// `commitDeadline` isn't known until `NewRequest` arrives.
    WaitingForRequest { approve: bool, deadline: u64 },
    /// The request exists onchain and commits are being collected.
    /// `committed_count` tallies every `Committed` event, from any
    /// sentinel; `self_committed` tracks whether ours landed among them.
    CollectingCommitments {
        approve: bool,
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

/// Snapshot state for the V2 (commit-reveal) sentinel FSM, keyed by request
/// ID.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateV2(pub HashMap<B256, SentinelRequestStateV2>);

#[cfg(test)]
mod tests {
    use super::*;

    fn preparing(deadline: u64) -> SentinelRequestState {
        SentinelRequestState {
            deadline,
            approve: true,
            status: RequestStatus::Preparing,
        }
    }

    #[test]
    fn state_serde_roundtrip() {
        let id = B256::from([1u8; 32]);
        let mut state = State::default();
        state.0.insert(id, preparing(100));

        let json = serde_json::to_string(&state).unwrap();
        let restored: State = serde_json::from_str(&json).unwrap();

        let req = &restored.0[&id];
        assert_eq!(req.deadline, 100);
        assert!(req.approve);
        assert_eq!(req.status, RequestStatus::Preparing);
    }

    #[test]
    fn state_multiple_requests() {
        let mut state = State::default();
        for i in 0u8..3 {
            state
                .0
                .insert(B256::from([i; 32]), preparing(u64::from(i) * 10));
        }
        assert_eq!(state.0.len(), 3);
        let json = serde_json::to_string(&state).unwrap();
        let restored: State = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.0.len(), 3);
        for i in 0u8..3 {
            let req = &restored.0[&B256::from([i; 32])];
            assert_eq!(req.deadline, u64::from(i) * 10);
            assert!(req.approve);
            assert_eq!(req.status, RequestStatus::Preparing);
        }
    }

    fn collecting_commitments_v2(
        commit_deadline: u64,
        reveal_deadline: u64,
    ) -> SentinelRequestStateV2 {
        SentinelRequestStateV2::CollectingCommitments {
            approve: false,
            commit_deadline,
            reveal_deadline,
            committed_count: 2,
            self_committed: true,
        }
    }

    #[test]
    fn state_v2_serde_roundtrip() {
        let id = B256::from([2u8; 32]);
        let mut state = StateV2::default();
        state.0.insert(id, collecting_commitments_v2(100, 150));

        let json = serde_json::to_string(&state).unwrap();
        let restored: StateV2 = serde_json::from_str(&json).unwrap();

        assert_eq!(restored, state);
    }

    #[test]
    fn state_v2_multiple_requests_in_different_phases() {
        let waiting_id = B256::from([3u8; 32]);
        let collecting_id = B256::from([4u8; 32]);
        let mut state = StateV2::default();
        state.0.insert(
            waiting_id,
            SentinelRequestStateV2::WaitingForRequest {
                approve: true,
                deadline: 10,
            },
        );
        state
            .0
            .insert(collecting_id, collecting_commitments_v2(100, 150));

        let json = serde_json::to_string(&state).unwrap();
        let restored: StateV2 = serde_json::from_str(&json).unwrap();

        assert_eq!(restored, state);
    }
}
