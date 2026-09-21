//! Transaction-verification logic for the sentinel engine service.
//!
//! This module is independent of the HTTP transport in [`crate::api`]. The
//! API passes decoded Safe transactions to [`SentinelEngine`], which owns the
//! configured checker chain.

mod coverage;
mod proposal;
mod rule;
mod transaction;

use self::proposal::ParseError;
// Test-only and not part of the engine's own public API — exposed so a
// checker's own tests can build a `Proposal` the same way `SentinelEngine`
// itself does, flattening a real batch, rather than `Proposal::from`'s
// unbatched identity wrap.
#[cfg(test)]
pub(crate) use self::proposal::parse;
pub use self::{
    coverage::{Coverage, CoverageLabel},
    proposal::Proposal,
    rule::RuleId,
    transaction::{MetaTransaction, Operation, SafeTransaction},
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
    /// `transaction` is first parsed into a [`Proposal`] — recognizing and
    /// flattening a MultiSend batch into its packed sub-calls (see
    /// [`Proposal`]'s docs). A batch nested deeper than the parser's
    /// recursion bound can't be turned into a usable view at all; the
    /// engine abstains without running any check in that case.
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
        let proposal = match proposal::parse(transaction) {
            Ok(proposal) => proposal,
            Err(ParseError(why)) => {
                tracing::trace!(why, "abstaining: transaction could not be parsed");
                return Verdict::Abstain;
            }
        };

        let mut covered = Coverage::empty();
        for checker in &self.0 {
            let assessment = checker.check(&proposal, &context).await;
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

        let required = Coverage::required_for(&proposal.transaction);
        let verdict = if covered.contains(required) {
            Verdict::Secure
        } else {
            let missing = covered.missing(required);
            tracing::trace!(%covered, %missing, "abstaining: incomplete coverage");
            for label in missing.labels() {
                crate::metrics::missing_coverage_total(label).increment(1);
            }
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

        async fn check(&self, _: &Proposal, _: &CheckContext) -> Assessment {
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
                coverage: Coverage::all(),
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
                coverage: Coverage::TO | Coverage::OPERATION,
            })),
            Box::new(StubChecker(Assessment::Secure {
                coverage: Coverage::VALUE | Coverage::DATA | Coverage::REFUND,
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
    async fn base_checker_never_abstains() {
        use crate::checkers::BaseChecker;

        let safe = alloy::primitives::Address::new([1u8; 20]);
        let other = alloy::primitives::Address::new([2u8; 20]);

        let allowed_call = SafeTransaction {
            safe,
            to: other,
            ..Default::default()
        };
        let denied_call = SafeTransaction {
            safe,
            to: safe,
            data: vec![0xde, 0xad, 0xbe, 0xef].into(),
            ..Default::default()
        };
        let allowed_delegatecall = SafeTransaction {
            safe,
            to: alloy::primitives::address!("526643F69b81B008F46d95CD5ced5eC0edFFDaC6"),
            data: alloy::primitives::bytes!("ed007fc6"),
            operation: Operation::DelegateCall,
            ..Default::default()
        };
        let denied_delegatecall = SafeTransaction {
            safe,
            to: other,
            operation: Operation::DelegateCall,
            ..Default::default()
        };

        for transaction in [
            allowed_call,
            denied_call,
            allowed_delegatecall,
            denied_delegatecall,
        ] {
            assert_ne!(
                BaseChecker
                    .check(&Proposal::from(transaction), &CheckContext::default())
                    .await,
                Assessment::Abstain
            );
        }
    }

    /// The epic's headline fix: a relayed nested `execTransaction` no longer
    /// affirms just because `NestedSafeChecker` doesn't inspect the refund
    /// leg. Not expressible as a corpus vector — see "Behavior changes not
    /// expressible as test vectors" in the verdict-composition epic.
    #[tokio::test]
    async fn relayed_nested_exec_transaction_abstains() {
        use crate::{checkers::NestedSafeChecker, contracts::bindings::safe};
        use alloy::{primitives::U256, sol_types::SolCall as _};

        let engine = SentinelEngine::new(vec![Box::new(NestedSafeChecker)]);
        let transaction = SafeTransaction {
            safe: alloy::primitives::Address::new([1u8; 20]),
            to: alloy::primitives::Address::new([2u8; 20]),
            gas_price: U256::from(1u64),
            data: safe::execTransactionCall {
                to: alloy::primitives::Address::ZERO,
                value: U256::ZERO,
                data: Default::default(),
                operation: 0,
                safeTxGas: U256::ZERO,
                baseGas: U256::ZERO,
                gasPrice: U256::ZERO,
                gasToken: alloy::primitives::Address::ZERO,
                refundReceiver: alloy::primitives::Address::ZERO,
                signatures: Default::default(),
            }
            .abi_encode()
            .into(),
            ..Default::default()
        };

        assert_eq!(
            engine
                .security_check(transaction, CheckContext::default())
                .await,
            Verdict::Abstain
        );
    }

    /// `NestedSafeChecker` doesn't inspect `value`, so it can't affirm a
    /// nested call that also carries native currency.
    #[tokio::test]
    async fn nested_exec_transaction_with_native_value_abstains() {
        use crate::{checkers::NestedSafeChecker, contracts::bindings::safe};
        use alloy::{primitives::U256, sol_types::SolCall as _};

        let engine = SentinelEngine::new(vec![Box::new(NestedSafeChecker)]);
        let transaction = SafeTransaction {
            safe: alloy::primitives::Address::new([1u8; 20]),
            to: alloy::primitives::Address::new([2u8; 20]),
            value: U256::from(1u64),
            data: safe::execTransactionCall {
                to: alloy::primitives::Address::ZERO,
                value: U256::ZERO,
                data: Default::default(),
                operation: 0,
                safeTxGas: U256::ZERO,
                baseGas: U256::ZERO,
                gasPrice: U256::ZERO,
                gasToken: alloy::primitives::Address::ZERO,
                refundReceiver: alloy::primitives::Address::ZERO,
                signatures: Default::default(),
            }
            .abi_encode()
            .into(),
            ..Default::default()
        };

        assert_eq!(
            engine
                .security_check(transaction, CheckContext::default())
                .await,
            Verdict::Abstain
        );
    }

    /// A relayed escape-hatch call is just as structurally safe as an
    /// unrelayed one, but `EscapeHatchChecker` can't vouch for the refund
    /// leg, so the engine now abstains rather than affirming on
    /// `Coverage::all()`. Not expressible as a corpus vector — see "Behavior
    /// changes not expressible as test vectors" in the verdict-composition
    /// epic.
    #[tokio::test]
    async fn relayed_escape_hatch_call_abstains() {
        use crate::{checkers::EscapeHatchChecker, contracts::bindings::safenet_guard};
        use alloy::{primitives::U256, sol_types::SolCall as _};

        let engine = SentinelEngine::new(vec![Box::new(EscapeHatchChecker)]);
        let transaction = SafeTransaction {
            safe: alloy::primitives::Address::new([1u8; 20]),
            to: alloy::primitives::Address::new([2u8; 20]),
            gas_price: U256::from(1u64),
            data: safenet_guard::cancelAnnouncementCall {
                announcementHash: Default::default(),
            }
            .abi_encode()
            .into(),
            ..Default::default()
        };

        assert_eq!(
            engine
                .security_check(transaction, CheckContext::default())
                .await,
            Verdict::Abstain
        );
    }

    #[tokio::test]
    async fn one_partial_claim_abstains() {
        let engine = SentinelEngine::new(vec![Box::new(StubChecker(Assessment::Secure {
            coverage: Coverage::TO | Coverage::OPERATION,
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
