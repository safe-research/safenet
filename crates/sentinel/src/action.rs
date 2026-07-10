use alloy::primitives::{B256, U256};

/// An action emitted by the V2 (commit-reveal) sentinel FSM during a state
/// transition, replacing the public-vote `CommitApprove`/`CommitDeny` pair
/// with a blind `Commit` followed by a `Reveal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SentinelActionKindV2 {
    /// Approve the fee token to be spent by the oracle (bond pre-authorisation).
    ApproveToken { bond: U256 },
    /// Lock a bond behind a blind commitment hash for the request with the given id.
    Commit { id: B256, hash: B256 },
    /// Reveal a previously committed vote for the request with the given id.
    Reveal { id: B256, approve: bool, salt: B256 },
    /// Finalise the committed vote for the request with the given id.
    Finalize { id: B256 },
    /// Claim the bond and reward for the request with the given id.
    Claim { id: B256 },
}

/// A V2 sentinel action tagged with the block by which it is no longer
/// useful; see [`SentinelAction`]'s own docs for the expiry semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentinelActionV2 {
    pub kind: SentinelActionKindV2,
    pub expires_at: Option<u64>,
}
