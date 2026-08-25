//! Blocking of transactions to known malicious destinations.

use super::Checker;
use crate::engine::{RuleId, SafeTransaction, Verdict};
use alloy::primitives::Address;
use std::collections::HashSet;

/// Denies transactions to a configured destination.
pub struct BlocklistChecker(HashSet<Address>);

impl BlocklistChecker {
    /// Creates a checker with the destinations to deny.
    pub fn new(blocklist: impl IntoIterator<Item = Address>) -> Self {
        Self(blocklist.into_iter().collect())
    }
}

#[async_trait::async_trait]
impl Checker for BlocklistChecker {
    fn name(&self) -> &'static str {
        "blocklist"
    }

    async fn check(&self, transaction: &SafeTransaction) -> Verdict {
        if self.0.contains(&transaction.to) {
            Verdict::Insecure {
                rule: RuleId::R4_6KnownMaliciousTarget,
            }
        } else {
            Verdict::Abstain
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A1: Address = Address::new([1u8; 20]);
    const A2: Address = Address::new([2u8; 20]);
    const A3: Address = Address::new([3u8; 20]);

    fn transaction(to: Address) -> SafeTransaction {
        SafeTransaction {
            to,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn denies_when_blocklisted() {
        let checker = BlocklistChecker::new([A1, A2]);

        for address in [A1, A2] {
            assert_eq!(
                checker.check(&transaction(address)).await,
                Verdict::Insecure {
                    rule: RuleId::R4_6KnownMaliciousTarget,
                }
            );
        }
    }

    #[tokio::test]
    async fn abstains_with_empty_blocklist() {
        let checker = BlocklistChecker::new([]);

        assert_eq!(checker.check(&transaction(A1)).await, Verdict::Abstain);
    }

    #[tokio::test]
    async fn abstains_when_not_blocklisted() {
        let checker = BlocklistChecker::new([A1, A2]);

        assert_eq!(checker.check(&transaction(A3)).await, Verdict::Abstain);
    }
}
