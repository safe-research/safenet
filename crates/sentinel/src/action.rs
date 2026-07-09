use alloy::primitives::{B256, U256};

/// An action emitted by the sentinel during a state transition.
#[derive(Debug, Clone, PartialEq, Eq)]
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
/// so the queue can drop it if it goes unsubmitted past that block. `None`
/// means the action must never expire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentinelAction {
    pub kind: SentinelActionKind,
    pub expires_at: Option<u64>,
}

/// An action emitted by the V2 (commit-reveal) sentinel FSM during a state
/// transition, replacing the public-vote `CommitApprove`/`CommitDeny` pair
/// above with a blind `Commit` followed by a `Reveal`.
///
/// TODO(sentinel commit-reveal, phase C2): `SentinelActionKind`/
/// `SentinelAction` above are deleted once `servicev2.rs` replaces
/// `service.rs`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(dead_code)]
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
