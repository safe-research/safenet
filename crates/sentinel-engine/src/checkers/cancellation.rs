//! Recognition of Safe nonce-cancellation transactions.

use super::{Assessment, Checker};
use crate::engine::{AspectSet, CheckContext, Proposal, SafeTransaction};

/// Considers an empty call from a Safe to itself secure.
pub struct CancellationChecker;

#[async_trait::async_trait]
impl Checker for CancellationChecker {
    fn name(&self) -> &'static str {
        "cancellation"
    }

    /// Claims every aspect of the transaction's single call, plus the
    /// refund leg: `cancellation`'s zeroed template pins `data` empty, which
    /// means `transaction` can never have decoded as a recognized MultiSend
    /// batch (a batch's top-level `data` is the packed payload, never
    /// empty), so `proposal.calls` is always exactly the one call this
    /// template-matches against.
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
                coverage: proposal
                    .checked(AspectSet::all())
                    .union(proposal.refund_checked()),
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

        let proposal = Proposal::from(transaction);
        assert_eq!(
            CancellationChecker
                .check(&proposal, &CheckContext::default())
                .await,
            Assessment::Secure {
                coverage: proposal
                    .checked(AspectSet::all())
                    .union(proposal.refund_checked())
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
