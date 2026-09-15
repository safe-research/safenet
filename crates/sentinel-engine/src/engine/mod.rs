//! Transaction-verification logic for the sentinel engine service.
//!
//! This module is independent of the HTTP transport in [`crate::api`]. The
//! API passes decoded Safe transactions to [`SentinelEngine`], which owns the
//! configured checker chain.

mod coverage;
mod rule;
mod transaction;

// `Aspect` has no non-test caller yet — checkers start building `Coverage`
// values from individual aspects in the epic's next phase, which narrows
// `BaseChecker`'s claim off `Coverage::ALL`.
#[allow(unused_imports)]
pub use self::{
    coverage::{Aspect, Coverage},
    rule::RuleId,
    transaction::{Operation, SafeTransaction},
};
use crate::checkers::{Assessment, Checker};
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
    ///
    /// Any `Insecure` assessment is the engine's verdict — denials are never
    /// masked by an affirmation, and the first denial short-circuits the
    /// run. Otherwise, the engine answers `Secure` only once the union of
    /// every affirming check's claimed [`Coverage`] covers every aspect the
    /// transaction actually has, and `Abstain` otherwise.
    pub async fn security_check(
        &self,
        transaction: SafeTransaction,
        context: CheckContext,
    ) -> Verdict {
        let mut covered = Coverage::NONE;
        for checker in &self.0 {
            let assessment = checker.check(&transaction, &context).await;
            tracing::trace!(checker = checker.name(), ?assessment, "checker assessment");
            match assessment {
                Assessment::Insecure { rule } => {
                    let verdict = Verdict::Insecure { rule };
                    tracing::trace!(?verdict, "security check verdict");
                    return verdict;
                }
                Assessment::Secure { coverage } => covered = covered.union(coverage),
                Assessment::Abstain => {}
            }
        }

        let required = Coverage::required_for(&transaction);
        let verdict = if covered.contains_all(required) {
            Verdict::Secure
        } else {
            let missing = covered.missing(required);
            tracing::trace!(%covered, %missing, "abstaining: incomplete coverage");
            Verdict::Abstain
        };
        tracing::trace!(?verdict, "security check verdict");
        verdict
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubChecker(Assessment);

    #[async_trait::async_trait]
    impl Checker for StubChecker {
        fn name(&self) -> &'static str {
            "stub"
        }

        async fn check(&self, _: &SafeTransaction, _: &CheckContext) -> Assessment {
            self.0
        }
    }

    /// A transaction with no aspects to trivially cover, so composing to
    /// `Secure` requires an explicit voucher for every one of them.
    fn transaction_requiring_every_aspect() -> SafeTransaction {
        SafeTransaction {
            value: alloy::primitives::U256::from(1u64),
            data: vec![0xde, 0xad].into(),
            gas_price: alloy::primitives::U256::from(1u64),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn abstains_when_every_checker_abstains() {
        let engine = SentinelEngine::new(vec![Box::new(StubChecker(Assessment::Abstain))]);

        assert_eq!(
            engine
                .security_check(SafeTransaction::default(), CheckContext::default())
                .await,
            Verdict::Abstain
        );
    }

    #[tokio::test]
    async fn a_denial_dominates_a_preceding_secure() {
        let engine = SentinelEngine::new(vec![
            Box::new(StubChecker(Assessment::Secure {
                coverage: Coverage::ALL,
            })),
            Box::new(StubChecker(Assessment::Insecure {
                rule: RuleId::R4_3ValueTarget,
            })),
        ]);

        assert_eq!(
            engine
                .security_check(SafeTransaction::default(), CheckContext::default())
                .await,
            Verdict::Insecure {
                rule: RuleId::R4_3ValueTarget,
            }
        );
    }

    #[tokio::test]
    async fn two_partial_claims_compose_to_secure() {
        let engine = SentinelEngine::new(vec![
            Box::new(StubChecker(Assessment::Secure {
                coverage: Coverage::of(&[Aspect::To, Aspect::Operation]),
            })),
            Box::new(StubChecker(Assessment::Secure {
                coverage: Coverage::of(&[Aspect::Value, Aspect::Data, Aspect::Refund]),
            })),
        ]);

        assert_eq!(
            engine
                .security_check(
                    transaction_requiring_every_aspect(),
                    CheckContext::default()
                )
                .await,
            Verdict::Secure
        );
    }

    #[tokio::test]
    async fn one_partial_claim_abstains() {
        let engine = SentinelEngine::new(vec![Box::new(StubChecker(Assessment::Secure {
            coverage: Coverage::of(&[Aspect::To, Aspect::Operation]),
        }))]);

        assert_eq!(
            engine
                .security_check(
                    transaction_requiring_every_aspect(),
                    CheckContext::default()
                )
                .await,
            Verdict::Abstain
        );
    }
}
