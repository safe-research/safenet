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
#[cfg_attr(not(test), expect(dead_code))]
pub struct State(pub HashMap<B256, SentinelRequestState>);

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
}
