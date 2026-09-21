//! Check for the Safe's own gas-refund mechanism: a nonzero `gasPrice` has
//! the Safe reimburse the relayer for `gasPrice * gasUsed` (up to
//! `safeTxGas`/`baseGas`) in `gasToken` to `refundReceiver`. That payment is
//! itself a transfer out of the Safe, and `refundReceiver` is just as
//! attacker-controllable as any other transfer recipient — so it gets the
//! same [`AddressPoisoningChecker`] scrutiny as the transaction's primary
//! ERC-20 transfer, by resynthesizing the refund as a `transfer` call and
//! delegating to it.
//!
//! Runs late in the engine's checker chain, alongside
//! [`AddressPoisoningChecker`]'s own primary-transfer check: both are RPC-
//! backed, so cheaper local checkers get a chance to reach a verdict first.
//! A genuine prior interaction with `refundReceiver` is evidence about the
//! refund leg alone, never the transaction's primary effect, so
//! [`RefundChecker::check`] reinterprets a delegated [`Assessment::Secure`]
//! as claiming only [`Coverage::REFUND`] — the engine still needs another
//! check to cover the rest of the transaction before it can answer `Secure`
//! overall.

use super::{AddressPoisoningChecker, Assessment, CheckContext, Checker};
use crate::{
    contracts::bindings::erc20::transferCall,
    engine::{Coverage, MetaTransaction, Proposal, SafeTransaction},
};
use alloy::sol_types::SolCall as _;
use std::sync::Arc;

/// Treats a Safe transaction's own gas refund as a transfer and runs it
/// through [`AddressPoisoningChecker`]. Takes the checker as a shared `Arc`
/// so the engine can also run it directly against transactions' primary
/// transfers, rather than needing a second, independent instance.
pub struct RefundChecker(Arc<AddressPoisoningChecker>);

impl RefundChecker {
    pub fn new(address_poisoning: Arc<AddressPoisoningChecker>) -> Self {
        Self(address_poisoning)
    }
}

#[async_trait::async_trait]
impl Checker for RefundChecker {
    fn name(&self) -> &'static str {
        "refund"
    }

    /// Resynthesizes `transaction`'s own gas refund as an ERC-20 `transfer`
    /// from the Safe to `refundReceiver` and defers to
    /// [`AddressPoisoningChecker`], as the sole call of a `Proposal` that
    /// otherwise keeps `proposal.transaction` as-is — so the delegate still
    /// sees the real `chain_id` and `safe`, which the synthesized call has
    /// none of its own. Abstains outright when there's no refund to
    /// resynthesize (see [`refund_transfer`]). A delegated `Secure` is
    /// reinterpreted as covering only [`Coverage::REFUND`] — the recipient's
    /// prior history says nothing about the rest of the transaction.
    async fn check(&self, proposal: &Proposal, context: &CheckContext) -> Assessment {
        let Some(refund_call) = refund_transfer(&proposal.transaction) else {
            return Assessment::Abstain;
        };
        let refund_proposal = Proposal {
            transaction: proposal.transaction.clone(),
            calls: vec![refund_call],
        };
        match self.0.check(&refund_proposal, context).await {
            Assessment::Secure { .. } => Assessment::Secure {
                coverage: Coverage::REFUND,
            },
            assessment => assessment,
        }
    }
}

/// Builds the ERC-20 `transfer` call `transaction`'s own gas refund amounts
/// to, or `None` when there's nothing to check:
///
/// - `gasPrice` zero — no refund is paid at all.
/// - `gasToken` zero — the refund is paid in native currency, which
///   [`AddressPoisoningChecker`] doesn't decode (it only recognizes ERC-20
///   calldata).
///
///   TODO(follow-up): this checker abstaining no longer lets another check's
///   `Secure` stand in for the refund leg — an uncovered `Refund` now forces
///   the engine to `Abstain` on the whole transaction. What's still missing
///   is a way to *affirm* a native-currency refund at all. See F6 (native-
///   value target check).
/// - `refundReceiver` zero — Safe.sol then pays `tx.origin` instead, an
///   address this checker has no way to learn ahead of execution.
///
///   TODO(follow-up): same hole as the native-currency case — abstaining
///   here now costs the engine coverage rather than being silently masked by
///   another check's `Secure`. Closing it needs an amount policy, since an
///   unset `refundReceiver` paying a reasonable fee to an unknown relayer
///   isn't itself a violation. See F7 (refund amount policy).
fn refund_transfer(transaction: &SafeTransaction) -> Option<MetaTransaction> {
    if transaction.gas_price.is_zero()
        || transaction.gas_token.is_zero()
        || transaction.refund_receiver.is_zero()
    {
        return None;
    }

    Some(MetaTransaction {
        to: transaction.gas_token,
        data: transferCall {
            to: transaction.refund_receiver,
            amount: transaction
                .gas_price
                .saturating_mul(transaction.safe_tx_gas.saturating_add(transaction.base_gas)),
        }
        .abi_encode()
        .into(),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Address, U256};

    const SAFE: Address = Address::new([1u8; 20]);
    const GAS_TOKEN: Address = Address::new([2u8; 20]);
    const REFUND_RECEIVER: Address = Address::new([3u8; 20]);

    fn relayed_tx() -> SafeTransaction {
        SafeTransaction {
            safe: SAFE,
            gas_price: U256::from(1u64),
            safe_tx_gas: U256::from(100_000u64),
            base_gas: U256::from(21_000u64),
            gas_token: GAS_TOKEN,
            refund_receiver: REFUND_RECEIVER,
            ..Default::default()
        }
    }

    #[test]
    fn none_when_gas_price_is_zero() {
        let transaction = SafeTransaction {
            gas_price: U256::ZERO,
            ..relayed_tx()
        };

        assert_eq!(refund_transfer(&transaction), None);
    }

    #[test]
    fn none_on_a_native_currency_refund() {
        let transaction = SafeTransaction {
            gas_token: Address::ZERO,
            ..relayed_tx()
        };

        assert_eq!(refund_transfer(&transaction), None);
    }

    #[test]
    fn none_when_the_refund_receiver_is_unset() {
        let transaction = SafeTransaction {
            refund_receiver: Address::ZERO,
            ..relayed_tx()
        };

        assert_eq!(refund_transfer(&transaction), None);
    }

    #[test]
    fn builds_an_erc20_transfer_to_the_refund_receiver() {
        let refund = refund_transfer(&relayed_tx()).expect("a refund to check");

        assert_eq!(refund.to, GAS_TOKEN);
        assert_eq!(
            refund.data,
            transferCall {
                to: REFUND_RECEIVER,
                amount: U256::from(1u64) * U256::from(121_000u64),
            }
            .abi_encode()
        );
    }
}
