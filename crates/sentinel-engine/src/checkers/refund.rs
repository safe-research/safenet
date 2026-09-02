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
//! Also, for the same reason [`crate::checkers::CowChecker`] runs ahead of
//! `AddressPoisoningChecker`'s own bypass (see that module's docs), a
//! `Secure` verdict here must never stand in for the whole transaction: it
//! only means the refund's recipient has *some* prior history, which is weak
//! evidence for the refund leg alone, let alone the transaction's primary
//! effect. [`RefundChecker::check`] squashes it to [`Verdict::Abstain`]
//! accordingly — only a genuine [`Verdict::Insecure`] denial is allowed
//! through.

use super::{AddressPoisoningChecker, CheckContext, Checker};
use crate::{
    contracts::bindings::erc20::transferCall,
    engine::{SafeTransaction, Verdict},
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
    /// [`AddressPoisoningChecker`]; abstains outright when there's no refund
    /// to resynthesize (see [`refund_transfer`]).
    async fn check(&self, transaction: &SafeTransaction, context: &CheckContext) -> Verdict {
        let Some(refund) = refund_transfer(transaction) else {
            return Verdict::Abstain;
        };
        deny_or_abstain(self.0.check(&refund, context).await)
    }
}

/// Only lets a denial through. A poisoning check's `Secure` verdict is, at
/// best, evidence about the one leg it was run against — never grounds to
/// affirm the whole transaction, which is what returning it here would do:
/// this checker runs in a chain that stops at the first non-[`Verdict::Abstain`]
/// verdict, so any recipient with *some* public onchain history (trivial for
/// an attacker to pick) would otherwise make the engine answer `Secure`
/// without Blocklist, CoW, ExcessiveApproval, or the primary-transfer check
/// ever running.
fn deny_or_abstain(verdict: Verdict) -> Verdict {
    match verdict {
        Verdict::Secure => Verdict::Abstain,
        verdict => verdict,
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
///   TODO(follow-up): this checker abstaining doesn't mean anything else
///   inspects a native refund either. Since the engine-wide "abstain on any
///   nonzero `gasPrice`" guard that used to sit ahead of the whole checker
///   chain is gone, a transaction another checker calls `Secure` can now
///   drain unbounded native currency to `refundReceiver` uncommented on. A
///   native-value-aware check (or at least an amount cap) is needed before
///   this is safe to affirm.
/// - `refundReceiver` zero — Safe.sol then pays `tx.origin` instead, an
///   address this checker has no way to learn ahead of execution.
///
///   TODO(follow-up): same hole as the native-currency case — the refund
///   still goes out, to a fully unvetted relayer, on a transaction the rest
///   of the chain can still approve. Needs a policy once the engine can
///   observe or reason about the relayer, not just abstain on it.
fn refund_transfer(transaction: &SafeTransaction) -> Option<SafeTransaction> {
    if transaction.gas_price.is_zero()
        || transaction.gas_token.is_zero()
        || transaction.refund_receiver.is_zero()
    {
        return None;
    }

    Some(SafeTransaction {
        safe: transaction.safe,
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
    use crate::engine::RuleId;
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

        assert_eq!(refund.safe, SAFE);
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

    #[test]
    fn never_lets_a_secure_refund_leg_affirm_the_whole_transaction() {
        assert_eq!(deny_or_abstain(Verdict::Secure), Verdict::Abstain);
    }

    #[test]
    fn passes_through_a_denial() {
        let denial = Verdict::Insecure {
            rule: RuleId::R4_3ValueTarget,
        };

        assert_eq!(deny_or_abstain(denial), denial);
    }

    #[test]
    fn passes_through_an_abstention() {
        assert_eq!(deny_or_abstain(Verdict::Abstain), Verdict::Abstain);
    }
}
