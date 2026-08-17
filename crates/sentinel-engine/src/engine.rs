//! Transaction-verification logic for the sentinel engine service.
//!
//! This module is independent of the HTTP transport in [`crate::api`]. The
//! API passes decoded Safe transactions to [`SentinelEngine`], which will own
//! the checker chain as checks are added in later phases.

use safe_tx::{SafeTransaction, rule::RuleId};
use serde::{Deserialize, Serialize};

/// The transaction-verification engine shared by API handlers.
pub struct SentinelEngine;

/// The engine's assessment of a proposed transaction.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase", tag = "verdict")]
pub enum Verdict {
    /// All configured checks consider the transaction secure.
    Secure,
    /// A check found that the transaction violates a Charter rule.
    Insecure {
        /// The rule violated by the transaction.
        rule: RuleId,
    },
    /// The engine abstains because it cannot give a trustworthy answer.
    Abstain,
}

impl SentinelEngine {
    /// Assesses a proposed Safe transaction using the configured checks.
    ///
    /// There are no checks in this phase, so the engine abstains from voting on
    /// every transaction.
    pub async fn security_check(&self, _transaction: SafeTransaction) -> Verdict {
        Verdict::Abstain
    }
}
