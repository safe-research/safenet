//! The engine's entry-point parser: recognizes and flattens a MultiSend
//! batch before any check runs, so every check works from the same
//! structured view instead of re-deriving a batch itself through its own
//! decoding path. See the batched-meta-transactions epic's "same-authority
//! expansion" for why only a `DELEGATECALL` to a known MultiSend deployment
//! carrying a decodable payload is ever flattened — a plain `CALL` to a
//! MultiSend contract, or a delegatecall to something else, stays one
//! opaque call.

use super::{AspectSet, CallCoverage, MetaTransaction, Operation, SafeTransaction};
use crate::contracts::multi_send::decode_multi_send_call;
use alloy::primitives::Address;

/// How many levels of nested MultiSend batches the parser flattens before
/// giving up. A MultiSend deployment with `allows_delegate_calls == true`
/// can carry a sub-call that is itself a delegatecall to a MultiSend
/// contract, so nesting is possible in principle; this bound exists only to
/// keep parsing bounded, not because deeper nesting is otherwise meaningful.
/// A guess — revisit if the corpus or production traffic argues otherwise.
const MAX_BATCH_DEPTH: u32 = 4;

/// A proposed Safe transaction, plus the calls it makes: the top-level call
/// itself, or, for a recognized MultiSend batch, its packed sub-calls in
/// execution order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    /// The transaction as proposed: identity and refund fields, plus (for an
    /// unbatched transaction) its own action fields.
    pub transaction: SafeTransaction,
    /// `transaction`'s calls: one entry for a plain transaction, one per
    /// packed sub-call for a recognized MultiSend batch. Every check
    /// evaluates these rather than re-deriving a batch itself.
    pub calls: Vec<MetaTransaction>,
}

// Temporary: not yet called by any checker, since `Assessment::Secure` still
// carries the old `Coverage` until phase 7b's checker migration lands.
// Goes away once it does.
#[allow(dead_code)]
impl Proposal {
    /// Claims `aspects` for every one of this proposal's calls, uniformly —
    /// shorthand for the common case of [`CallCoverage::calls`] sized to
    /// `self.calls.len()`. Only right for a check that examines every call
    /// the same way and only ever affirms once all of them pass; see
    /// [`CallCoverage::calls`]'s own docs for when it isn't.
    pub fn checked(&self, aspects: AspectSet) -> CallCoverage {
        CallCoverage::calls(self.calls.len(), aspects)
    }

    /// Claims only this proposal's own refund leg — shorthand for
    /// [`CallCoverage::refund`] sized to `self.calls.len()`.
    pub fn refund_checked(&self) -> CallCoverage {
        CallCoverage::refund(self.calls.len())
    }
}

impl From<SafeTransaction> for Proposal {
    /// Wraps `transaction` as a single, unbatched call, without attempting
    /// to recognize a MultiSend batch — use [`parse`] for that. Meant for a
    /// transaction already known not to be a batch container, such as a
    /// checker's own synthesized transaction (e.g. `RefundChecker`'s
    /// resynthesized gas-refund transfer).
    fn from(transaction: SafeTransaction) -> Self {
        let calls = vec![transaction.as_meta_transaction()];
        Self { transaction, calls }
    }
}

/// `transaction` couldn't be parsed into a usable [`Proposal`] — a MultiSend
/// batch nested deeper than [`MAX_BATCH_DEPTH`]. The caller should abstain
/// without running any check, logging the reason.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ParseError(pub &'static str);

/// Parses `transaction` into a [`Proposal`], recognizing and depth-bounded-
/// recursively flattening a `DELEGATECALL` to a known MultiSend deployment
/// (see the module docs).
pub fn parse(transaction: SafeTransaction) -> Result<Proposal, ParseError> {
    let safe = transaction.safe;
    let calls =
        flatten(safe, transaction.as_meta_transaction(), MAX_BATCH_DEPTH).map_err(ParseError)?;
    Ok(Proposal { transaction, calls })
}

/// Flattens `call`, recognizing and expanding a MultiSend batch up to
/// `depth_remaining` further levels. Depth-first, so the result stays in
/// execution order — `StakingChecker::check_pair` depends on this.
///
/// Checking `depth_remaining` only after decoding (rather than before) is
/// deliberate: we can't tell whether `call` exceeds the bound without first
/// finding out whether it's a further batch at all — a non-batch call
/// bottoms out the recursion regardless of `depth_remaining`. Bounding it
/// here, before ever recursing further, is what keeps this function's own
/// call-stack depth capped at `MAX_BATCH_DEPTH + 1` regardless of how deeply
/// a proposal's calldata *claims* to nest — an attacker cannot drive this
/// into unbounded (and therefore stack-overflowing) recursion merely by
/// crafting deeply nested calldata, because we simply refuse to recurse past
/// the bound.
fn flatten(
    safe: Address,
    call: MetaTransaction,
    depth_remaining: u32,
) -> Result<Vec<MetaTransaction>, &'static str> {
    let Some((sub_calls, allows_delegate_calls)) = decode_batch(safe, &call) else {
        return Ok(vec![call]);
    };
    if depth_remaining == 0 {
        return Err("multisend batch nested deeper than the maximum recursion depth");
    }

    let mut flattened = Vec::with_capacity(sub_calls.len());
    for sub_call in sub_calls {
        if sub_call.operation == Operation::DelegateCall && !allows_delegate_calls {
            // `MultiSendCallOnly` reverts onchain on a delegatecall
            // sub-entry, so this can never actually run — but the proposal
            // is malformed, not a legitimate nested batch, so it stays
            // opaque for `BaseChecker` to deny under R-4.2, exactly as an
            // unknown delegatecall would be.
            flattened.push(sub_call);
        } else {
            flattened.extend(flatten(safe, sub_call, depth_remaining - 1)?);
        }
    }
    Ok(flattened)
}

/// Recognizes `call` as a `DELEGATECALL` to a known MultiSend deployment
/// carrying a decodable `multiSend(bytes)` payload, bridging into
/// `decode_multi_send_call` (which still takes a whole `SafeTransaction`)
/// the same way `BaseChecker` and `decode_target_effects` do today.
fn decode_batch(safe: Address, call: &MetaTransaction) -> Option<(Vec<MetaTransaction>, bool)> {
    let tx = SafeTransaction {
        safe,
        to: call.to,
        value: call.value,
        data: call.data.clone(),
        operation: call.operation,
        ..Default::default()
    };
    decode_multi_send_call(&tx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::bindings::multi_send;
    use alloy::{
        primitives::{Bytes, U256, address},
        sol_types::SolCall as _,
    };

    fn pack(operation: Operation, to: Address, value: U256, data: &[u8]) -> Vec<u8> {
        let mut out = vec![operation as u8];
        out.extend_from_slice(to.as_slice());
        out.extend_from_slice(&value.to_be_bytes::<32>());
        out.extend_from_slice(&U256::from(data.len()).to_be_bytes::<32>());
        out.extend_from_slice(data);
        out
    }

    fn multisend(sub_txs: &[Vec<u8>]) -> Bytes {
        let transactions: Vec<u8> = sub_txs.iter().flatten().cloned().collect();
        Bytes::from(
            multi_send::multiSendCall {
                transactions: Bytes::from(transactions),
            }
            .abi_encode(),
        )
    }

    /// Builds the packed `multiSend(bytes)` calldata for a chain of
    /// `levels` nested batches: each level delegatecalls `multi_send_addr`
    /// again with the next level's batch, and the innermost level wraps a
    /// single ordinary `Call` leaf.
    fn nested_multi_send_data(multi_send_addr: Address, levels: u32) -> Bytes {
        if levels == 1 {
            return multisend(&[pack(
                Operation::Call,
                Address::new([9u8; 20]),
                U256::ZERO,
                &[],
            )]);
        }
        let inner = nested_multi_send_data(multi_send_addr, levels - 1);
        multisend(&[pack(
            Operation::DelegateCall,
            multi_send_addr,
            U256::ZERO,
            inner.as_ref(),
        )])
    }

    const SAFE: Address = address!("F01888f0677547Ec07cd16c8680e699c96588E6B");
    /// Legacy MultiSend, `allows_delegate_calls: true`.
    const MULTI_SEND: Address = address!("38869bf66a61cF6bDB996A6aE40D5853Fd43B526");
    /// Legacy MultiSendCallOnly, `allows_delegate_calls: false`.
    const MULTI_SEND_CALL_ONLY: Address = address!("9641d764fc13c8B624c04430C7356C1C7C8102e2");
    /// v1.5.0+ MultiSend, `allows_delegate_calls: true`.
    const MULTI_SEND_V150: Address = address!("218543288004CD07832472D464648173c77D7eB7");

    fn transaction(to: Address, data: Bytes, operation: Operation) -> SafeTransaction {
        SafeTransaction {
            safe: SAFE,
            to,
            data,
            operation,
            ..Default::default()
        }
    }

    fn proposal(transaction: SafeTransaction) -> Proposal {
        parse(transaction)
            .unwrap_or_else(|ParseError(why)| panic!("expected a proposal, got an error: {why}"))
    }

    #[test]
    fn flattens_a_recognized_batch_in_execution_order() {
        let first_to = Address::new([1u8; 20]);
        let second_to = Address::new([2u8; 20]);
        let data = multisend(&[
            pack(Operation::Call, first_to, U256::from(1u64), &[0xaa]),
            pack(Operation::Call, second_to, U256::from(2u64), &[0xbb]),
        ]);

        let proposal = proposal(transaction(MULTI_SEND, data, Operation::DelegateCall));

        assert_eq!(
            proposal.calls,
            vec![
                MetaTransaction {
                    to: first_to,
                    value: U256::from(1u64),
                    data: vec![0xaa].into(),
                    operation: Operation::Call,
                },
                MetaTransaction {
                    to: second_to,
                    value: U256::from(2u64),
                    data: vec![0xbb].into(),
                    operation: Operation::Call,
                },
            ]
        );
    }

    #[test]
    fn resolves_a_v150_plus_self_call_to_the_safe() {
        let data = multisend(&[pack(Operation::Call, Address::ZERO, U256::ZERO, &[])]);

        let proposal = proposal(transaction(MULTI_SEND_V150, data, Operation::DelegateCall));

        assert_eq!(
            proposal.calls,
            vec![MetaTransaction {
                to: SAFE,
                ..Default::default()
            }]
        );
    }

    #[test]
    fn a_malformed_payload_stays_the_container_itself() {
        let tx = transaction(
            MULTI_SEND,
            Bytes::from(vec![0xde, 0xad]),
            Operation::DelegateCall,
        );

        let proposal = proposal(tx.clone());

        assert_eq!(proposal.calls, vec![tx.as_meta_transaction()]);
    }

    #[test]
    fn a_plain_call_to_a_multi_send_contract_stays_opaque() {
        let data = multisend(&[pack(
            Operation::Call,
            Address::new([9u8; 20]),
            U256::ZERO,
            &[],
        )]);
        let tx = transaction(MULTI_SEND, data, Operation::Call);

        let proposal = proposal(tx.clone());

        assert_eq!(proposal.calls, vec![tx.as_meta_transaction()]);
    }

    #[test]
    fn a_nested_delegatecall_inside_a_call_only_deployment_stays_opaque() {
        let inner_data = multisend(&[pack(
            Operation::Call,
            Address::new([9u8; 20]),
            U256::ZERO,
            &[],
        )]);
        let outer_data = multisend(&[pack(
            Operation::DelegateCall,
            MULTI_SEND,
            U256::ZERO,
            inner_data.as_ref(),
        )]);
        let tx = transaction(MULTI_SEND_CALL_ONLY, outer_data, Operation::DelegateCall);

        let proposal = proposal(tx);

        assert_eq!(
            proposal.calls,
            vec![MetaTransaction {
                to: MULTI_SEND,
                value: U256::ZERO,
                data: inner_data,
                operation: Operation::DelegateCall,
            }]
        );
    }

    #[test]
    fn flattens_a_batch_nested_up_to_the_depth_bound() {
        let tx = transaction(
            MULTI_SEND,
            nested_multi_send_data(MULTI_SEND, MAX_BATCH_DEPTH),
            Operation::DelegateCall,
        );

        assert!(parse(tx).is_ok());
    }

    #[test]
    fn abstains_on_a_batch_nested_deeper_than_the_depth_bound() {
        let tx = transaction(
            MULTI_SEND,
            nested_multi_send_data(MULTI_SEND, MAX_BATCH_DEPTH + 1),
            Operation::DelegateCall,
        );

        assert!(parse(tx).is_err());
    }
}
