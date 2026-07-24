use crate::effect::Effect;
use alloy::primitives::{B256, U256};
use safenet_core::state::Command;

/// An action emitted by the sentinel during a state transition, driving the
/// commit-reveal FSM (blind `Commit` followed by `Reveal`) rather than a
/// public vote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SentinelActionKind {
    /// Approve the fee token to be spent by the oracle (bond pre-authorisation).
    ApproveToken { bond: U256 },
    /// Lock a bond behind a blind commitment hash for the request with the given id.
    Commit { id: B256, hash: B256 },
    /// Reveal a previously committed vote and reasoning for the request with the given id.
    Reveal {
        id: B256,
        approve: bool,
        salt: B256,
        reason: String,
    },
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

impl From<SentinelAction> for Command<SentinelAction, Effect> {
    fn from(action: SentinelAction) -> Self {
        Command::Action(action)
    }
}
