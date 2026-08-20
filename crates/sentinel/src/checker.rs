//! The sentinel's internal representation of a transaction-verification
//! outcome. The static checker produces it locally, while
//! [`crate::engine::EngineClient`] maps the sentinel engine's wire verdict to
//! it.

use crate::engine::RuleId;

/// The result of checking a proposed transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckOutcome {
    /// The transaction should receive an approving vote.
    Approved,
    /// The transaction should receive a denying vote citing this rule.
    Denied(RuleId),
    /// There is no trustworthy verdict, so the request is dropped unanswered
    /// rather than guessed at.
    Unknown,
}
