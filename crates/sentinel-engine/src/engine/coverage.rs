//! The coverage vocabulary a check's affirming path claims, and the engine
//! composes, in place of "first non-abstaining verdict wins". See the
//! "Verdict composition" epic for the full rationale.
//!
//! This file also holds `AspectSet`/`CallCoverage` (phase 7a of the batched
//! meta-transactions epic) — the not-yet-wired-up replacement for `Coverage`
//! below. See the comment ahead of that section.

// Temporary, for `AspectSet`/`CallCoverage` below: not yet reachable from
// `main()` or exercised by any test — phase 7b wires them in and migrates
// `Coverage`'s tests below over to them. Goes away in phase 7c, along with
// `Coverage` above them.
#![allow(dead_code)]

use super::{MetaTransaction, Operation, Proposal, SafeTransaction};
use bitflags::bitflags;
use std::fmt;

bitflags! {
    /// The parts of a Safe transaction a check can vouch for, as a set. A
    /// single-aspect claim is just a `Coverage` with one bit set (e.g.
    /// `Coverage::TO`) — there is no separate "aspect" type.
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub struct Coverage: u8 {
        /// The destination the transaction calls.
        #[bitflags(flag_name = "to")]
        const TO = 1 << 0;
        /// Native currency leaving the Safe.
        #[bitflags(flag_name = "value")]
        const VALUE = 1 << 1;
        /// The calldata and the effects it encodes.
        #[bitflags(flag_name = "data")]
        const DATA = 1 << 2;
        /// `CALL` versus `DELEGATECALL`.
        #[bitflags(flag_name = "operation")]
        const OPERATION = 1 << 3;
        /// The Safe's own gas-refund payment (`gasPrice`, `gasToken`,
        /// `safeTxGas`, `baseGas`, `refundReceiver`) — one indivisible aspect,
        /// since a claim about part of the payment says nothing actionable.
        #[bitflags(flag_name = "refund")]
        const REFUND = 1 << 4;
    }
}

/// A single coverage aspect's metric-label spelling, as yielded by
/// [`Coverage::labels`] — distinct from `Coverage` itself, which may hold
/// more than one aspect at once and so has no single label of its own.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoverageLabel(&'static str);

impl CoverageLabel {
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for CoverageLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Coverage {
    /// `TO | VALUE | DATA | OPERATION` — everything but the refund leg.
    pub fn action() -> Self {
        Self::TO | Self::VALUE | Self::DATA | Self::OPERATION
    }

    /// `other`'s aspects that `self` lacks — what an `Abstain` is logged
    /// with.
    pub const fn missing(self, other: Self) -> Self {
        other.difference(self)
    }

    /// This coverage's contained aspects, one label per set flag, in
    /// declaration order — what `Display` joins with `|`, and what the
    /// missing-coverage metric increments one counter per.
    pub fn labels(self) -> impl Iterator<Item = CoverageLabel> {
        self.iter_names().map(|(name, _)| CoverageLabel(name))
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
        let mut required = Self::all();
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
        let names = self.labels().map(CoverageLabel::as_str).collect::<Vec<_>>();
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
            assert!(Coverage::all().contains(aspect));
        }
    }

    #[test]
    fn action_is_all_but_refund() {
        assert!(Coverage::action().contains(Coverage::TO));
        assert!(Coverage::action().contains(Coverage::VALUE));
        assert!(Coverage::action().contains(Coverage::DATA));
        assert!(Coverage::action().contains(Coverage::OPERATION));
        assert!(!Coverage::action().contains(Coverage::REFUND));
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
        let required = Coverage::action();
        assert_eq!(covered.missing(required), Coverage::VALUE | Coverage::DATA);
        assert_eq!(required.missing(covered), Coverage::empty());
    }

    #[test]
    fn labels_yields_exactly_the_contained_aspects_in_declaration_order() {
        let coverage = Coverage::OPERATION | Coverage::TO;
        assert_eq!(
            coverage
                .labels()
                .map(CoverageLabel::as_str)
                .collect::<Vec<_>>(),
            vec!["to", "operation"]
        );
    }

    #[test]
    fn labels_are_the_lowercase_flag_names() {
        for (coverage, name) in [
            (Coverage::TO, "to"),
            (Coverage::VALUE, "value"),
            (Coverage::DATA, "data"),
            (Coverage::OPERATION, "operation"),
            (Coverage::REFUND, "refund"),
        ] {
            assert_eq!(
                coverage
                    .labels()
                    .map(CoverageLabel::as_str)
                    .collect::<Vec<_>>(),
                vec![name]
            );
        }
    }

    #[test]
    fn required_for_is_all_for_an_ordinary_relayed_call_with_data() {
        let tx = transaction(
            U256::from(1u64),
            U256::from(1u64),
            vec![0xde, 0xad],
            Operation::Call,
        );
        assert_eq!(Coverage::required_for(&tx), Coverage::all());
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

bitflags! {
    /// The parts of a single call a check can vouch for, as a set. A
    /// single-aspect claim is just an `AspectSet` with one bit set (e.g.
    /// `AspectSet::TO`) — there is no separate "aspect" type.
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub struct AspectSet: u8 {
        /// The destination the call targets.
        #[bitflags(flag_name = "to")]
        const TO = 1 << 0;
        /// Native currency the call moves.
        #[bitflags(flag_name = "value")]
        const VALUE = 1 << 1;
        /// The calldata and the effects it encodes.
        #[bitflags(flag_name = "data")]
        const DATA = 1 << 2;
        /// `CALL` versus `DELEGATECALL`.
        #[bitflags(flag_name = "operation")]
        const OPERATION = 1 << 3;
    }
}

/// The refund leg's own label. Not one of `AspectSet`'s bits, since the
/// refund is a proposal-level claim rather than a per-call one — see
/// [`CallCoverage`].
const REFUND_LABEL: CoverageLabel = CoverageLabel("refund");

impl AspectSet {
    /// This set's contained aspects, one label per set flag, in declaration
    /// order — what `Display` joins with `|`.
    pub fn labels(self) -> impl Iterator<Item = CoverageLabel> {
        self.iter_names().map(|(name, _)| CoverageLabel(name))
    }

    /// The aspects a `Secure` verdict for `call` requires vouchers for. An
    /// aspect `call` cannot actually exercise is trivially covered and
    /// dropped from the requirement:
    ///
    /// - `value == 0`, or `operation == DelegateCall` (which takes no value
    ///   argument) — no native currency leaves the Safe on this call, so
    ///   `Value` needs no voucher.
    /// - `data` is empty — there is no calldata effect to vouch for.
    ///
    /// `To` and `Operation` are always required.
    pub fn required_for(call: &MetaTransaction) -> Self {
        let mut required = Self::all();
        if call.value.is_zero() || call.operation == Operation::DelegateCall {
            required = required.difference(Self::VALUE);
        }
        if call.data.is_empty() {
            required = required.difference(Self::DATA);
        }
        required
    }
}

impl fmt::Display for AspectSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names = self.labels().map(CoverageLabel::as_str).collect::<Vec<_>>();
        write!(f, "{}", names.join("|"))
    }
}

/// What a check's affirming path claims to have examined: a per-call
/// [`AspectSet`], indexed parallel to [`Proposal::calls`] and always exactly
/// as long as it, plus whether the Safe transaction's own gas-refund leg was
/// vouched for — one flag, not one per call, since a packed sub-call has no
/// refund leg of its own (see `engine/transaction.rs`'s `MetaTransaction`
/// docs).
///
/// Every constructor below takes the proposal's call count so this invariant
/// holds by construction; [`CallCoverage::union`], [`CallCoverage::contains`]
/// and [`CallCoverage::missing`] all assume it (debug-asserted) rather than
/// tolerating a mismatched width — two claims about calls that aren't the
/// same proposal's calls have nothing meaningful to combine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallCoverage {
    calls: Vec<AspectSet>,
    refund: bool,
}

impl CallCoverage {
    /// Claims nothing: no aspect of any of `total_calls` calls, and not the
    /// refund leg. The fold's starting point.
    pub fn none(total_calls: usize) -> Self {
        Self {
            calls: vec![AspectSet::empty(); total_calls],
            refund: false,
        }
    }

    /// Claims `aspects` for every one of `total_calls` calls. Every
    /// affirming check in this crate either vouches for a call the same way
    /// as every other call it examines, or abstains outright — a claim
    /// naming a proper subset of indices isn't honest until a check can
    /// actually tell which calls it did and didn't examine, which none does
    /// today (see the batched-meta-transactions epic's G2 follow-up).
    pub fn calls(total_calls: usize, aspects: AspectSet) -> Self {
        Self {
            calls: vec![aspects; total_calls],
            refund: false,
        }
    }

    /// Claims only the Safe transaction's own refund leg — no aspect of any
    /// of `total_calls` calls. `total_calls` still has to be the proposal's
    /// own call count (not, say, a synthesized single-call sub-proposal a
    /// check delegates to internally) so this claim's width matches every
    /// other claim it's unioned against.
    pub fn refund(total_calls: usize) -> Self {
        Self {
            calls: vec![AspectSet::empty(); total_calls],
            refund: true,
        }
    }

    /// Combines two partial claims into what both together vouch for.
    pub fn union(self, other: Self) -> Self {
        debug_assert_eq!(
            self.calls.len(),
            other.calls.len(),
            "CallCoverage::union: operands must be claims about the same proposal's calls"
        );
        let calls = self
            .calls
            .into_iter()
            .zip(other.calls)
            .map(|(a, b)| a.union(b))
            .collect();
        Self {
            calls,
            refund: self.refund || other.refund,
        }
    }

    /// Whether `self` covers every aspect `required` names, for every call
    /// index `required` names, and the refund leg if `required` claims it.
    pub fn contains(&self, required: &Self) -> bool {
        debug_assert_eq!(
            self.calls.len(),
            required.calls.len(),
            "CallCoverage::contains: operands must be claims about the same proposal's calls"
        );
        self.calls
            .iter()
            .zip(&required.calls)
            .all(|(&have, &req)| have.contains(req))
            && (!required.refund || self.refund)
    }

    /// `required`'s aspects that `self` lacks, call by call, plus whether
    /// the refund leg is missing — what an `Abstain` is logged with.
    pub fn missing(&self, required: &Self) -> Self {
        debug_assert_eq!(
            self.calls.len(),
            required.calls.len(),
            "CallCoverage::missing: operands must be claims about the same proposal's calls"
        );
        let calls = self
            .calls
            .iter()
            .zip(&required.calls)
            .map(|(&have, &req)| req.difference(have))
            .collect();
        Self {
            calls,
            refund: required.refund && !self.refund,
        }
    }

    /// The coverage a `Secure` verdict for `proposal` requires: each call's
    /// own [`AspectSet::required_for`], plus the refund leg exactly when
    /// `proposal.transaction.gas_price != 0` (`Safe.sol` only calls
    /// `handlePayment` `if (gasPrice > 0)`, so nothing is paid and no
    /// voucher is needed when it's zero).
    pub fn required_for(proposal: &Proposal) -> Self {
        let calls = proposal.calls.iter().map(AspectSet::required_for).collect();
        let refund = !proposal.transaction.gas_price.is_zero();
        Self { calls, refund }
    }

    /// Every aspect label that can appear in a claim — the four per-call
    /// aspects plus the refund leg — for registering the missing-coverage
    /// metric with every label present (at zero) from the start.
    pub fn all_labels() -> impl Iterator<Item = CoverageLabel> {
        AspectSet::all()
            .labels()
            .chain(std::iter::once(REFUND_LABEL))
    }

    /// This coverage's aspect kinds: the union of every call's `AspectSet`,
    /// plus `"refund"` if the refund leg is claimed — the vocabulary the
    /// missing-coverage metric counts by, which does not distinguish which
    /// call index an aspect came from.
    pub fn labels(&self) -> impl Iterator<Item = CoverageLabel> {
        let calls = self
            .calls
            .iter()
            .fold(AspectSet::empty(), |acc, &aspects| acc.union(aspects));
        calls.labels().chain(self.refund.then_some(REFUND_LABEL))
    }
}

impl fmt::Display for CallCoverage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = self
            .calls
            .iter()
            .enumerate()
            .filter(|(_, aspects)| !aspects.is_empty())
            .map(|(i, aspects)| format!("call{i}:{aspects}"))
            .collect::<Vec<_>>();
        if self.refund {
            parts.push("refund".to_string());
        }
        write!(f, "{}", parts.join(","))
    }
}
