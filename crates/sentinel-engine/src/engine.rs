//! Transaction-verification logic for the sentinel engine service.
//!
//! This module is independent of the HTTP transport in [`crate::api`]. The
//! API passes decoded Safe transactions to [`SentinelEngine`], which owns the
//! configured checker chain.

use crate::checkers::Checker;
use safe_tx::{SafeTransaction, rule::RuleId};
use serde::{Deserialize, Serialize};

/// The transaction-verification engine shared by API handlers.
pub struct SentinelEngine(Vec<Box<dyn Checker>>);

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
    pub async fn security_check(&self, transaction: SafeTransaction) -> Verdict {
        let mut verdict = Verdict::Abstain;
        for checker in &self.0 {
            verdict = checker.check(&transaction).await;
            if verdict != Verdict::Abstain {
                break;
            }
        }
        verdict
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubChecker(Verdict);

    #[async_trait::async_trait]
    impl Checker for StubChecker {
        async fn check(&self, _: &SafeTransaction) -> Verdict {
            self.0
        }
    }

    #[tokio::test]
    async fn abstains_when_every_checker_abstains() {
        let engine = SentinelEngine::new(vec![Box::new(StubChecker(Verdict::Abstain))]);

        assert_eq!(
            engine.security_check(SafeTransaction::default()).await,
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
            engine.security_check(SafeTransaction::default()).await,
            Verdict::Secure
        );
    }
}
