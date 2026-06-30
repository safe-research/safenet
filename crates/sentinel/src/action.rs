use alloy::primitives::{B256, U256};

/// An action emitted by the sentinel during a state transition.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(test), expect(dead_code))]
pub enum SentinelActionKind {
    /// Approve the fee token to be spent by the oracle (bond pre-authorisation).
    ApproveToken { bond: U256 },
    /// Cast an approve vote for the request with the given id.
    CommitApprove { id: B256 },
    /// Cast a deny vote for the request with the given id.
    CommitDeny { id: B256 },
    /// Finalise the committed vote for the request with the given id.
    Finalize { id: B256 },
    /// Claim the bond and reward for the request with the given id.
    Claim { id: B256 },
}

/// A sentinel action tagged with the voting-deadline block by which it is
/// no longer useful.
///
/// The deadline is forwarded to the `TransactionQueue` as the per-tx expiry
/// so the queue can drop it if it goes unsubmitted past that block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentinelAction {
    pub kind: SentinelActionKind,
    pub expires_at: u64,
}
