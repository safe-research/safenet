//! The shared verdict type and trait every check plugged into
//! [`crate::effect::Handler`]'s dynamic-check chain implements: the built-in
//! [`crate::cow::CowChecker`] and
//! [`crate::address_poisoning::AddressPoisoningChecker`], and the
//! operator-configured [`crate::dynamic_checker::RemoteChecker`] alike.
//!
//! `Handler` runs its checkers in a fixed order, stopping at the first
//! non-[`CheckOutcome::Unknown`] result. If every checker in the chain comes
//! back `Unknown`, the whole dynamic check resolves to `Unknown` too — the
//! request is dropped unanswered rather than guessed at either way (see
//! `SentinelTransition::handle_dynamic_check_result`).
//!
//! This is an initial cut of the "array of checkers" shape (one, fixed,
//! hardcoded order; no way for a checker to run conditionally on an earlier
//! one's outcome beyond stop-on-first-answer) and is expected to be
//! iterated on.

use safe_tx::{rule::RuleId, types::SafeTransaction};

/// The result of running one checker in [`crate::effect::Handler`]'s chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckOutcome {
    /// No checker so far denied the transaction.
    Approved,
    /// This checker denied the transaction, citing this rule.
    Denied(RuleId),
    /// This checker has nothing conclusive to say — including a lookup
    /// failure (e.g. an unreachable RPC/HTTP endpoint), which can't be
    /// trusted as evidence either way. Move on to whichever checker runs
    /// next; if every checker in the chain resolves to this, the request is
    /// dropped unanswered rather than guessed at.
    Unknown,
}

/// One check in [`crate::effect::Handler`]'s dynamic-check chain.
#[async_trait::async_trait]
pub trait Checker: Send + Sync {
    async fn check(&self, transaction: &SafeTransaction) -> CheckOutcome;
}
