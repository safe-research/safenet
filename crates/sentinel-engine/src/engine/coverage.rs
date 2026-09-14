//! The coverage vocabulary a check's affirming path claims, and the engine
//! composes, in place of "first non-abstaining verdict wins". See the
//! "Verdict composition" epic for the full rationale.

use super::{Operation, SafeTransaction};
use std::fmt;

/// A part of a Safe transaction that a check can vouch for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Aspect {
    /// The destination the transaction calls.
    To = 0,
    /// Native currency leaving the Safe.
    Value = 1,
    /// The calldata and the effects it encodes.
    Data = 2,
    /// `CALL` versus `DELEGATECALL`.
    Operation = 3,
    /// The Safe's own gas-refund payment (`gasPrice`, `gasToken`,
    /// `safeTxGas`, `baseGas`, `refundReceiver`) — one indivisible aspect,
    /// since a claim about part of the payment says nothing actionable.
    Refund = 4,
}

impl Aspect {
    /// Every variant, in declaration order — the order `Coverage`'s
    /// `Display` impl lists aspects in.
    const ALL: [Self; 5] = [
        Self::To,
        Self::Value,
        Self::Data,
        Self::Operation,
        Self::Refund,
    ];

    const fn bit(self) -> u8 {
        1 << (self as u8)
    }

    const fn name(self) -> &'static str {
        match self {
            Self::To => "to",
            Self::Value => "value",
            Self::Data => "data",
            Self::Operation => "operation",
            Self::Refund => "refund",
        }
    }
}

/// A set of [`Aspect`]s. A hand-rolled bitset over five variants — no new
/// dependency, and `const` constructors so the common claims are constants.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Coverage(u8);

impl Coverage {
    /// No aspects covered.
    pub const NONE: Self = Self(0);
    /// `To | Value | Data | Operation` — everything but the refund leg.
    pub const ACTION: Self =
        Self(Aspect::To.bit() | Aspect::Value.bit() | Aspect::Data.bit() | Aspect::Operation.bit());
    /// Every aspect.
    pub const ALL: Self = Self(Self::ACTION.0 | Aspect::Refund.bit());

    /// Builds a `Coverage` from a list of aspects.
    ///
    /// No non-test caller yet — every check narrowed in this epic's next
    /// phase builds its claim with this.
    #[allow(dead_code)]
    pub const fn of(aspects: &[Aspect]) -> Self {
        let mut bits = 0u8;
        let mut i = 0;
        while i < aspects.len() {
            bits |= aspects[i].bit();
            i += 1;
        }
        Self(bits)
    }

    /// Whether `aspect` is in this set.
    pub const fn contains(self, aspect: Aspect) -> bool {
        self.0 & aspect.bit() != 0
    }

    /// Whether every aspect in `other` is also in `self`.
    pub fn contains_all(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// The union of `self` and `other`.
    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// `other`'s aspects that `self` lacks — what an `Abstain` is logged
    /// with.
    pub fn missing(self, other: Self) -> Self {
        Self(other.0 & !self.0)
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
            required = Self(required.0 & !Aspect::Value.bit());
        }
        if transaction.gas_price.is_zero() {
            required = Self(required.0 & !Aspect::Refund.bit());
        }
        if transaction.data.is_empty() {
            required = Self(required.0 & !Aspect::Data.bit());
        }
        required
    }
}

impl fmt::Display for Coverage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names = Aspect::ALL
            .into_iter()
            .filter(|&aspect| self.contains(aspect))
            .map(Aspect::name)
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
    fn none_contains_nothing() {
        for aspect in Aspect::ALL {
            assert!(!Coverage::NONE.contains(aspect));
        }
    }

    #[test]
    fn all_contains_every_aspect() {
        for aspect in Aspect::ALL {
            assert!(Coverage::ALL.contains(aspect));
        }
    }

    #[test]
    fn action_is_all_but_refund() {
        assert!(Coverage::ACTION.contains(Aspect::To));
        assert!(Coverage::ACTION.contains(Aspect::Value));
        assert!(Coverage::ACTION.contains(Aspect::Data));
        assert!(Coverage::ACTION.contains(Aspect::Operation));
        assert!(!Coverage::ACTION.contains(Aspect::Refund));
    }

    #[test]
    fn of_builds_the_given_set() {
        let coverage = Coverage::of(&[Aspect::To, Aspect::Data]);
        assert!(coverage.contains(Aspect::To));
        assert!(coverage.contains(Aspect::Data));
        assert!(!coverage.contains(Aspect::Value));
        assert!(!coverage.contains(Aspect::Operation));
        assert!(!coverage.contains(Aspect::Refund));
    }

    #[test]
    fn contains_all_checks_every_aspect_of_the_other_set() {
        let covered = Coverage::of(&[Aspect::To, Aspect::Data, Aspect::Operation]);
        assert!(covered.contains_all(Coverage::of(&[Aspect::To, Aspect::Data])));
        assert!(covered.contains_all(Coverage::NONE));
        assert!(!covered.contains_all(Coverage::of(&[Aspect::Value])));
    }

    #[test]
    fn union_combines_two_partial_claims() {
        let first = Coverage::of(&[Aspect::To, Aspect::Operation]);
        let second = Coverage::of(&[Aspect::Data]);
        assert_eq!(
            first.union(second),
            Coverage::of(&[Aspect::To, Aspect::Data, Aspect::Operation])
        );
    }

    #[test]
    fn missing_is_the_others_aspects_self_lacks() {
        let covered = Coverage::of(&[Aspect::To, Aspect::Operation]);
        let required = Coverage::ACTION;
        assert_eq!(
            covered.missing(required),
            Coverage::of(&[Aspect::Value, Aspect::Data])
        );
        assert_eq!(required.missing(covered), Coverage::NONE);
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
        assert!(!Coverage::required_for(&tx).contains(Aspect::Value));
    }

    #[test]
    fn required_for_drops_value_for_a_delegatecall_even_with_nonzero_value() {
        let tx = transaction(
            U256::from(1u64),
            U256::from(1u64),
            vec![0xde, 0xad],
            Operation::DelegateCall,
        );
        assert!(!Coverage::required_for(&tx).contains(Aspect::Value));
    }

    #[test]
    fn required_for_drops_refund_when_gas_price_is_zero() {
        let tx = transaction(
            U256::from(1u64),
            U256::ZERO,
            vec![0xde, 0xad],
            Operation::Call,
        );
        assert!(!Coverage::required_for(&tx).contains(Aspect::Refund));
    }

    #[test]
    fn required_for_drops_data_when_data_is_empty() {
        let tx = transaction(
            U256::from(1u64),
            U256::from(1u64),
            Bytes::new(),
            Operation::Call,
        );
        assert!(!Coverage::required_for(&tx).contains(Aspect::Data));
    }

    #[test]
    fn required_for_always_requires_to_and_operation() {
        let tx = SafeTransaction {
            to: Address::ZERO,
            ..Default::default()
        };
        let required = Coverage::required_for(&tx);
        assert!(required.contains(Aspect::To));
        assert!(required.contains(Aspect::Operation));
    }
}
