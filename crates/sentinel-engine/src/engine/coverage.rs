//! The coverage vocabulary a check's affirming path claims, and the engine
//! composes, in place of "first non-abstaining verdict wins". See the
//! "Verdict composition" epic for the full rationale.

use super::{Operation, SafeTransaction};
use bitflags::bitflags;
use std::fmt;

bitflags! {
    /// The parts of a Safe transaction a check can vouch for, as a set. A
    /// single-aspect claim is just a `Coverage` with one bit set (e.g.
    /// `Coverage::TO`) — there is no separate "aspect" type.
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub struct Coverage: u8 {
        /// The destination the transaction calls.
        const TO = 1 << 0;
        /// Native currency leaving the Safe.
        const VALUE = 1 << 1;
        /// The calldata and the effects it encodes.
        const DATA = 1 << 2;
        /// `CALL` versus `DELEGATECALL`.
        const OPERATION = 1 << 3;
        /// The Safe's own gas-refund payment (`gasPrice`, `gasToken`,
        /// `safeTxGas`, `baseGas`, `refundReceiver`) — one indivisible aspect,
        /// since a claim about part of the payment says nothing actionable.
        const REFUND = 1 << 4;

        /// `TO | VALUE | DATA | OPERATION` — everything but the refund leg.
        const ACTION = Self::TO.bits() | Self::VALUE.bits() | Self::DATA.bits() | Self::OPERATION.bits();
        /// Every aspect.
        const ALL = Self::ACTION.bits() | Self::REFUND.bits();
    }
}

impl Coverage {
    /// `other`'s aspects that `self` lacks — what an `Abstain` is logged
    /// with.
    pub const fn missing(self, other: Self) -> Self {
        other.difference(self)
    }

    /// The aspects a `Secure` verdict for `transaction` requires vouchers
    /// for. An aspect the transaction cannot actually exercise is trivially
    /// covered and dropped from the requirement:
    ///
    /// - `value == 0`, or `operation == DelegateCall` (which takes no value
    ///   argument) — no native currency leaves the Safe, so `Value` needs no
    ///   voucher.
    /// - `gasPrice == 0` — `Safe.sol` only calls `handlePayment` `if
    ///   (gasPrice > 0)`, so nothing is paid and `Refund` needs no voucher.
    /// - `data` is empty — there is no calldata effect to vouch for.
    ///
    /// `To` and `Operation` are always required.
    pub fn required_for(transaction: &SafeTransaction) -> Self {
        let mut required = Self::ALL;
        if transaction.value.is_zero() || transaction.operation == Operation::DelegateCall {
            required = required.difference(Self::VALUE);
        }
        if transaction.gas_price.is_zero() {
            required = required.difference(Self::REFUND);
        }
        if transaction.data.is_empty() {
            required = required.difference(Self::DATA);
        }
        required
    }
}

impl fmt::Display for Coverage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names = self
            .iter_names()
            .map(|(name, _)| name.to_lowercase())
            .collect::<Vec<_>>();
        write!(f, "{}", names.join("|"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Address, Bytes, U256};

    fn transaction(
        value: U256,
        gas_price: U256,
        data: impl Into<Bytes>,
        operation: Operation,
    ) -> SafeTransaction {
        SafeTransaction {
            value,
            gas_price,
            data: data.into(),
            operation,
            ..Default::default()
        }
    }

    #[test]
    fn empty_contains_nothing() {
        for aspect in [
            Coverage::TO,
            Coverage::VALUE,
            Coverage::DATA,
            Coverage::OPERATION,
            Coverage::REFUND,
        ] {
            assert!(!Coverage::empty().contains(aspect));
        }
    }

    #[test]
    fn all_contains_every_aspect() {
        for aspect in [
            Coverage::TO,
            Coverage::VALUE,
            Coverage::DATA,
            Coverage::OPERATION,
            Coverage::REFUND,
        ] {
            assert!(Coverage::ALL.contains(aspect));
        }
    }

    #[test]
    fn action_is_all_but_refund() {
        assert!(Coverage::ACTION.contains(Coverage::TO));
        assert!(Coverage::ACTION.contains(Coverage::VALUE));
        assert!(Coverage::ACTION.contains(Coverage::DATA));
        assert!(Coverage::ACTION.contains(Coverage::OPERATION));
        assert!(!Coverage::ACTION.contains(Coverage::REFUND));
    }

    #[test]
    fn contains_checks_every_aspect_of_the_other_set() {
        let covered = Coverage::TO | Coverage::DATA | Coverage::OPERATION;
        assert!(covered.contains(Coverage::TO | Coverage::DATA));
        assert!(covered.contains(Coverage::empty()));
        assert!(!covered.contains(Coverage::VALUE));
    }

    #[test]
    fn union_combines_two_partial_claims() {
        let first = Coverage::TO | Coverage::OPERATION;
        let second = Coverage::DATA;
        assert_eq!(
            first.union(second),
            Coverage::TO | Coverage::DATA | Coverage::OPERATION
        );
    }

    #[test]
    fn missing_is_the_others_aspects_self_lacks() {
        let covered = Coverage::TO | Coverage::OPERATION;
        let required = Coverage::ACTION;
        assert_eq!(covered.missing(required), Coverage::VALUE | Coverage::DATA);
        assert_eq!(required.missing(covered), Coverage::empty());
    }

    #[test]
    fn required_for_is_all_for_an_ordinary_relayed_call_with_data() {
        let tx = transaction(
            U256::from(1u64),
            U256::from(1u64),
            vec![0xde, 0xad],
            Operation::Call,
        );
        assert_eq!(Coverage::required_for(&tx), Coverage::ALL);
    }

    #[test]
    fn required_for_drops_value_when_value_is_zero() {
        let tx = transaction(
            U256::ZERO,
            U256::from(1u64),
            vec![0xde, 0xad],
            Operation::Call,
        );
        assert!(!Coverage::required_for(&tx).contains(Coverage::VALUE));
    }

    #[test]
    fn required_for_drops_value_for_a_delegatecall_even_with_nonzero_value() {
        let tx = transaction(
            U256::from(1u64),
            U256::from(1u64),
            vec![0xde, 0xad],
            Operation::DelegateCall,
        );
        assert!(!Coverage::required_for(&tx).contains(Coverage::VALUE));
    }

    #[test]
    fn required_for_drops_refund_when_gas_price_is_zero() {
        let tx = transaction(
            U256::from(1u64),
            U256::ZERO,
            vec![0xde, 0xad],
            Operation::Call,
        );
        assert!(!Coverage::required_for(&tx).contains(Coverage::REFUND));
    }

    #[test]
    fn required_for_drops_data_when_data_is_empty() {
        let tx = transaction(
            U256::from(1u64),
            U256::from(1u64),
            Bytes::new(),
            Operation::Call,
        );
        assert!(!Coverage::required_for(&tx).contains(Coverage::DATA));
    }

    #[test]
    fn required_for_always_requires_to_and_operation() {
        let tx = SafeTransaction {
            to: Address::ZERO,
            ..Default::default()
        };
        let required = Coverage::required_for(&tx);
        assert!(required.contains(Coverage::TO));
        assert!(required.contains(Coverage::OPERATION));
    }
}
