//! Recognition of Safe nonce-cancellation transactions.

use super::{Assessment, Checker};
use crate::engine::{CheckContext, Coverage, Proposal, SafeTransaction};

/// Considers an empty call from a Safe to itself secure.
pub struct CancellationChecker;

#[async_trait::async_trait]
impl Checker for CancellationChecker {
    fn name(&self) -> &'static str {
        "cancellation"
    }

    async fn check(&self, proposal: &Proposal, _context: &CheckContext) -> Assessment {
        let transaction = &proposal.transaction;
        let cancellation = SafeTransaction {
            chain_id: transaction.chain_id,
            safe: transaction.safe,
            to: transaction.safe,
            nonce: transaction.nonce,
            ..Default::default()
        };
        if transaction == &cancellation {
            Assessment::Secure {
                coverage: Coverage::all(),
            }
        } else {
            Assessment::Abstain
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Address, U256};

    #[tokio::test]
    async fn considers_an_empty_call_to_the_safe_secure() {
        let safe = Address::repeat_byte(0x11);
        let transaction = SafeTransaction {
            chain_id: U256::from(1),
            safe,
            to: safe,
            nonce: U256::from(42),
            ..Default::default()
        };

        assert_eq!(
            CancellationChecker
                .check(&Proposal::from(transaction), &CheckContext::default())
                .await,
            Assessment::Secure {
                coverage: Coverage::all()
            }
        );
    }

    #[tokio::test]
    async fn abstains_when_a_transaction_field_is_not_zeroed() {
        let safe = Address::repeat_byte(0x11);
        let transaction = SafeTransaction {
            safe,
            to: safe,
            value: U256::from(1),
            ..Default::default()
        };

        assert_eq!(
            CancellationChecker
                .check(&Proposal::from(transaction), &CheckContext::default())
                .await,
            Assessment::Abstain
        );
    }
}
