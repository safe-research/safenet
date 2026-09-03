//! Transaction-verification logic for the sentinel engine service.
//!
//! This module is independent of the HTTP transport in [`crate::api`]. The
//! API passes decoded Safe transactions to [`SentinelEngine`], which owns the
//! configured checker chain.

mod rule;
mod transaction;

pub use self::{
    rule::RuleId,
    transaction::{Operation, SafeTransaction},
};
use crate::checkers::Checker;
use serde::{Deserialize, Serialize};

/// The transaction-verification engine shared by API handlers.
pub struct SentinelEngine(Vec<Box<dyn Checker>>);

/// Per-request context threaded to every [`Checker`] alongside the
/// transaction being assessed, carrying caller-supplied hints that aren't
/// part of the transaction itself.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CheckContext {
    /// The block number the caller (the sentinel) considers current — the
    /// most recent block it had synced past when it submitted this check,
    /// from the request's required `block` field. A check that reads
    /// RPC-derived state should evaluate against this rather than resolving
    /// "latest" itself, so it shares the same view of the chain the caller
    /// had rather than racing ahead of (or behind) it. A check is free to
    /// ignore this if it has no RPC-derived state to anchor.
    pub block: u64,
}

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
    /// Creates an engine that runs `checkers` in order.
    pub fn new(checkers: Vec<Box<dyn Checker>>) -> Self {
        Self(checkers)
    }

    /// Assesses a proposed Safe transaction using the configured checks.
    pub async fn security_check(
        &self,
        transaction: SafeTransaction,
        context: CheckContext,
    ) -> Verdict {
        let mut verdict = Verdict::Abstain;
        for checker in &self.0 {
            verdict = checker.check(&transaction, &context).await;
            tracing::trace!(checker = checker.name(), ?verdict, "checker verdict");
            if verdict != Verdict::Abstain {
                break;
            }
        }
        tracing::trace!(?verdict, "security check verdict");
        verdict
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubChecker(Verdict);

    #[async_trait::async_trait]
    impl Checker for StubChecker {
        fn name(&self) -> &'static str {
            "stub"
        }

        async fn check(&self, _: &SafeTransaction, _: &CheckContext) -> Verdict {
            self.0
        }
    }

    #[tokio::test]
    async fn abstains_when_every_checker_abstains() {
        let engine = SentinelEngine::new(vec![Box::new(StubChecker(Verdict::Abstain))]);

        assert_eq!(
            engine
                .security_check(SafeTransaction::default(), CheckContext::default())
                .await,
            Verdict::Abstain
        );
    }

    #[tokio::test]
    async fn stops_at_the_first_non_abstaining_verdict() {
        let engine = SentinelEngine::new(vec![
            Box::new(StubChecker(Verdict::Abstain)),
            Box::new(StubChecker(Verdict::Secure)),
            Box::new(StubChecker(Verdict::Insecure {
                rule: RuleId::R4_3ValueTarget,
            })),
        ]);

        assert_eq!(
            engine
                .security_check(SafeTransaction::default(), CheckContext::default())
                .await,
            Verdict::Secure
        );
    }
}
