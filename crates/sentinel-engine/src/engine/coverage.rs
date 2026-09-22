//! The coverage vocabulary a check's affirming path claims, and the engine
//! composes, in place of "first non-abstaining verdict wins". See the
//! "Verdict composition" epic for the full rationale, and the
//! "batched meta-transactions" epic's "Coverage becomes per call" for why
//! it's keyed the way it is here: a claim names the calls it examined, not
//! the transaction as a whole.

use super::{MetaTransaction, Operation, Proposal};
use bitflags::bitflags;
use std::fmt;

/// A single coverage aspect's metric-label spelling, as yielded by
/// [`AspectSet::labels`] and [`Coverage::labels`] — distinct from either,
/// since both may hold more than one aspect at once and so have no single
/// label of their own.
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
/// [`Coverage`].
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
/// holds by construction; [`Coverage::union`], [`Coverage::contains`]
/// and [`Coverage::missing`] all assume it (debug-asserted) rather than
/// tolerating a mismatched width — two claims about calls that aren't the
/// same proposal's calls have nothing meaningful to combine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Coverage {
    calls: Vec<AspectSet>,
    refund: bool,
}

impl Coverage {
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
            "Coverage::union: operands must be claims about the same proposal's calls"
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
            "Coverage::contains: operands must be claims about the same proposal's calls"
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
            "Coverage::missing: operands must be claims about the same proposal's calls"
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

impl fmt::Display for Coverage {
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Address, Bytes, U256};

    fn call(value: U256, data: impl Into<Bytes>, operation: Operation) -> MetaTransaction {
        MetaTransaction {
            value,
            data: data.into(),
            operation,
            ..Default::default()
        }
    }

    fn proposal(calls: Vec<MetaTransaction>, gas_price: U256) -> Proposal {
        Proposal {
            transaction: crate::engine::SafeTransaction {
                gas_price,
                ..Default::default()
            },
            calls,
        }
    }

    #[test]
    fn aspect_set_empty_contains_nothing() {
        for aspect in [
            AspectSet::TO,
            AspectSet::VALUE,
            AspectSet::DATA,
            AspectSet::OPERATION,
        ] {
            assert!(!AspectSet::empty().contains(aspect));
        }
    }

    #[test]
    fn aspect_set_all_contains_every_aspect() {
        for aspect in [
            AspectSet::TO,
            AspectSet::VALUE,
            AspectSet::DATA,
            AspectSet::OPERATION,
        ] {
            assert!(AspectSet::all().contains(aspect));
        }
    }

    #[test]
    fn aspect_set_labels_are_the_lowercase_flag_names_in_declaration_order() {
        let aspects = AspectSet::OPERATION | AspectSet::TO;
        assert_eq!(
            aspects
                .labels()
                .map(CoverageLabel::as_str)
                .collect::<Vec<_>>(),
            vec!["to", "operation"]
        );
    }

    #[test]
    fn aspect_set_required_for_is_all_for_a_call_with_value_and_data() {
        let c = call(U256::from(1u64), vec![0xde, 0xad], Operation::Call);
        assert_eq!(AspectSet::required_for(&c), AspectSet::all());
    }

    #[test]
    fn aspect_set_required_for_drops_value_when_value_is_zero() {
        let c = call(U256::ZERO, vec![0xde, 0xad], Operation::Call);
        assert!(!AspectSet::required_for(&c).contains(AspectSet::VALUE));
    }

    #[test]
    fn aspect_set_required_for_drops_value_for_a_delegatecall_even_with_nonzero_value() {
        let c = call(U256::from(1u64), vec![0xde, 0xad], Operation::DelegateCall);
        assert!(!AspectSet::required_for(&c).contains(AspectSet::VALUE));
    }

    #[test]
    fn aspect_set_required_for_drops_data_when_data_is_empty() {
        let c = call(U256::from(1u64), Bytes::new(), Operation::Call);
        assert!(!AspectSet::required_for(&c).contains(AspectSet::DATA));
    }

    #[test]
    fn aspect_set_required_for_always_requires_to_and_operation() {
        let c = MetaTransaction {
            to: Address::ZERO,
            ..Default::default()
        };
        let required = AspectSet::required_for(&c);
        assert!(required.contains(AspectSet::TO));
        assert!(required.contains(AspectSet::OPERATION));
    }

    #[test]
    fn calls_claims_the_same_aspects_for_every_index() {
        let coverage = Coverage::calls(3, AspectSet::TO | AspectSet::DATA);
        let required = Coverage::calls(3, AspectSet::TO | AspectSet::DATA);
        assert!(coverage.contains(&required));
    }

    #[test]
    fn refund_claims_only_the_refund_leg() {
        let coverage = Coverage::refund(1);
        assert!(coverage.contains(&Coverage::refund(1)));
        assert!(!coverage.contains(&Coverage::calls(1, AspectSet::TO)));
    }

    #[test]
    fn union_combines_partial_per_call_claims() {
        let first = Coverage::calls(2, AspectSet::TO | AspectSet::OPERATION);
        let second = Coverage::calls(2, AspectSet::VALUE | AspectSet::DATA);
        let combined = first.union(second);
        assert!(combined.contains(&Coverage::calls(2, AspectSet::all())));
    }

    #[test]
    fn union_of_a_refund_claim_and_a_calls_claim_keeps_both() {
        let calls_claim = Coverage::calls(1, AspectSet::all());
        let refund_claim = Coverage::refund(1);
        let combined = calls_claim.union(refund_claim);

        let required = Coverage::calls(1, AspectSet::all()).union(Coverage::refund(1));
        assert!(combined.contains(&required));
    }

    #[test]
    #[should_panic(expected = "Coverage::union")]
    fn union_panics_on_claims_about_different_numbers_of_calls() {
        // Two claims widened to different call counts can't be about the
        // same proposal — combining them would either drop one side's
        // claim or fabricate coverage for calls it never examined.
        let _ = Coverage::calls(1, AspectSet::TO).union(Coverage::calls(3, AspectSet::TO));
    }

    #[test]
    fn contains_requires_every_required_call_index_and_the_refund_leg() {
        let covered = Coverage::calls(2, AspectSet::TO).union(Coverage::refund(2));
        assert!(covered.contains(&Coverage::calls(2, AspectSet::TO)));
        assert!(covered.contains(&Coverage::refund(2)));
        assert!(!covered.contains(&Coverage::calls(2, AspectSet::DATA)));
    }

    #[test]
    fn missing_is_per_call_and_includes_the_refund_leg() {
        let covered = Coverage::calls(2, AspectSet::TO);
        let required = Coverage::calls(2, AspectSet::all()).union(Coverage::refund(2));

        let missing = covered.missing(&required);
        assert!(missing.contains(&Coverage::calls(
            2,
            AspectSet::VALUE | AspectSet::DATA | AspectSet::OPERATION
        )));
        assert!(missing.contains(&Coverage::refund(2)));
    }

    #[test]
    fn missing_is_empty_when_fully_covered() {
        let required = Coverage::calls(2, AspectSet::all()).union(Coverage::refund(2));
        let missing = required.missing(&required);
        assert!(!missing.contains(&Coverage::calls(2, AspectSet::TO)));
        assert!(!missing.contains(&Coverage::refund(2)));
    }

    #[test]
    fn required_for_requires_every_call_s_own_aspects() {
        let batch = proposal(
            vec![
                call(U256::from(1u64), vec![0xde, 0xad], Operation::Call),
                call(U256::ZERO, Bytes::new(), Operation::Call),
            ],
            U256::ZERO,
        );

        let required = Coverage::required_for(&batch);
        let expected = Coverage {
            calls: vec![AspectSet::all(), AspectSet::TO | AspectSet::OPERATION],
            refund: false,
        };
        assert_eq!(required, expected);
    }

    #[test]
    fn required_for_requires_the_refund_leg_only_when_gas_price_is_nonzero() {
        let unrelayed = proposal(vec![MetaTransaction::default()], U256::ZERO);
        assert!(!Coverage::required_for(&unrelayed).refund);

        let relayed = proposal(vec![MetaTransaction::default()], U256::from(1u64));
        assert!(Coverage::required_for(&relayed).refund);
    }

    #[test]
    fn labels_yields_each_aspect_kind_at_most_once_across_calls_plus_refund() {
        let coverage = Coverage::calls(2, AspectSet::TO).union(Coverage::refund(2));
        assert_eq!(
            coverage
                .labels()
                .map(CoverageLabel::as_str)
                .collect::<Vec<_>>(),
            vec!["to", "refund"]
        );
    }

    #[test]
    fn all_labels_covers_every_aspect_including_refund() {
        assert_eq!(
            Coverage::all_labels()
                .map(CoverageLabel::as_str)
                .collect::<Vec<_>>(),
            vec!["to", "value", "data", "operation", "refund"]
        );
    }

    #[test]
    fn display_names_only_non_empty_calls_and_the_refund_leg() {
        let coverage = Coverage {
            calls: vec![
                AspectSet::TO | AspectSet::OPERATION,
                AspectSet::empty(),
                AspectSet::DATA,
            ],
            refund: false,
        }
        .union(Coverage::refund(3));

        assert_eq!(coverage.to_string(), "call0:to|operation,call2:data,refund");
    }
}
