//! Blocking of transactions to known malicious destinations.

use super::{Assessment, Checker};
use crate::engine::{CheckContext, MetaTransaction, Proposal, RuleId};
use alloy::primitives::Address;
use std::collections::HashSet;

/// Denies transactions to a configured destination.
pub struct BlocklistChecker(HashSet<Address>);

impl BlocklistChecker {
    /// Creates a checker with the destinations to deny.
    pub fn new(blocklist: impl IntoIterator<Item = Address>) -> Self {
        Self(blocklist.into_iter().collect())
    }

    /// Whether any of `calls`' destinations is blocklisted — not just a
    /// batch's top-level container. Not corpus-testable: the integration
    /// script hard-codes an empty blocklist (see the batched-meta-
    /// transactions epic's G4), so this is covered by a unit test instead.
    fn any_call_blocklisted(&self, calls: &[MetaTransaction]) -> bool {
        calls.iter().any(|call| self.0.contains(&call.to))
    }
}

#[async_trait::async_trait]
impl Checker for BlocklistChecker {
    fn name(&self) -> &'static str {
        "blocklist"
    }

    async fn check(&self, proposal: &Proposal, _context: &CheckContext) -> Assessment {
        if self.any_call_blocklisted(&proposal.calls) {
            Assessment::Insecure {
                rule: RuleId::R4_6KnownMaliciousTarget,
            }
        } else {
            Assessment::Abstain
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::SafeTransaction;

    const A1: Address = Address::new([1u8; 20]);
    const A2: Address = Address::new([2u8; 20]);
    const A3: Address = Address::new([3u8; 20]);

    fn transaction(to: Address) -> Proposal {
        Proposal::from(SafeTransaction {
            to,
            ..Default::default()
        })
    }

    #[tokio::test]
    async fn denies_when_blocklisted() {
        let checker = BlocklistChecker::new([A1, A2]);

        for address in [A1, A2] {
            assert_eq!(
                checker
                    .check(&transaction(address), &CheckContext::default())
                    .await,
                Assessment::Insecure {
                    rule: RuleId::R4_6KnownMaliciousTarget,
                }
            );
        }
    }

    #[tokio::test]
    async fn abstains_with_empty_blocklist() {
        let checker = BlocklistChecker::new([]);

        assert_eq!(
            checker
                .check(&transaction(A1), &CheckContext::default())
                .await,
            Assessment::Abstain
        );
    }

    #[tokio::test]
    async fn abstains_when_not_blocklisted() {
        let checker = BlocklistChecker::new([A1, A2]);

        assert_eq!(
            checker
                .check(&transaction(A3), &CheckContext::default())
                .await,
            Assessment::Abstain
        );
    }

    /// Closes the batch gap: a blocklisted destination among several calls
    /// (as a flattened MultiSend batch produces) is caught, not just a
    /// single top-level `to`. Not corpus-testable (see [`any_call_blocklisted`]'s
    /// docs), so pinned directly on the helper instead of through `check()`.
    #[test]
    fn any_call_blocklisted_checks_every_call_not_just_the_first() {
        let checker = BlocklistChecker::new([A2]);
        let calls = [
            MetaTransaction {
                to: A1,
                ..Default::default()
            },
            MetaTransaction {
                to: A2,
                ..Default::default()
            },
        ];

        assert!(checker.any_call_blocklisted(&calls));
    }
}
