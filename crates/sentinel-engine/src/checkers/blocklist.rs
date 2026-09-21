//! Blocking of transactions to known malicious destinations.

use super::{Assessment, Checker};
use crate::engine::{CheckContext, Proposal, RuleId};
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

    async fn check(&self, proposal: &Proposal, _context: &CheckContext) -> Assessment {
        if self.0.contains(&proposal.transaction.to) {
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
}
