//! Recognition of nested Safe transactions: a Safe calling another
//! contract's `execTransaction`.
//!
//! Article IV Part A already lets a Safe call any other contract freely —
//! only self-calls and delegatecalls are restricted (see
//! [`crate::checkers::BaseChecker`]). Calling another Safe's
//! `execTransaction` is just such a call: whatever the nested transaction
//! does is that Safe's own guard's concern (if it has one), not this
//! transaction's, so the outer call's `to`, `data` and `operation` are
//! secure independent of the nested transaction's own content. It does not
//! inspect `value`, so it claims nothing about a nested call that also moves
//! native currency.

use super::{Assessment, Checker};
use crate::{
    contracts::bindings::safe,
    engine::{Aspect, CheckContext, Coverage, Operation, SafeTransaction},
};
use alloy::sol_types::SolCall as _;

/// Considers the `to`, `data` and `operation` of a call to another
/// contract's `execTransaction` secure, regardless of the nested transaction
/// it carries.
pub struct NestedSafeChecker;

#[async_trait::async_trait]
impl Checker for NestedSafeChecker {
    fn name(&self) -> &'static str {
        "nested_safe"
    }

    async fn check(&self, transaction: &SafeTransaction, _context: &CheckContext) -> Assessment {
        if is_nested_exec_transaction(transaction) {
            Assessment::Secure {
                coverage: Coverage::of(&[Aspect::To, Aspect::Data, Aspect::Operation]),
            }
        } else {
            Assessment::Abstain
        }
    }
}

/// A `Call` (never a delegatecall) to a different address, carrying
/// `execTransaction` calldata for that address to decode and enforce on its
/// own terms.
fn is_nested_exec_transaction(tx: &SafeTransaction) -> bool {
    tx.operation == Operation::Call
        && tx.to != tx.safe
        && tx.data.starts_with(&safe::execTransactionCall::SELECTOR)
        && safe::execTransactionCall::abi_decode(&tx.data).is_ok()
}
