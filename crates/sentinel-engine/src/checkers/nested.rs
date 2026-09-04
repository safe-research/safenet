//! Recognition of nested Safe transactions: a Safe calling another
//! contract's `execTransaction`.
//!
//! Article IV Part A already lets a Safe call any other contract freely —
//! only self-calls and delegatecalls are restricted (see
//! [`crate::checkers::BaseChecker`]). Calling another Safe's
//! `execTransaction` is just such a call: whatever the nested transaction
//! does is that Safe's own guard's concern (if it has one), not this
//! transaction's, so it's secure independent of the nested transaction's own
//! content. Runs after [`crate::checkers::BlocklistChecker`] so a nested call
//! to a known malicious `to` is still denied rather than short-circuited.

use super::Checker;
use crate::{
    contracts::bindings::safe,
    engine::{CheckContext, Operation, SafeTransaction, Verdict},
};
use alloy::sol_types::SolCall as _;

/// Considers a call to another contract's `execTransaction` secure,
/// regardless of the nested transaction it carries.
pub struct NestedSafeChecker;

#[async_trait::async_trait]
impl Checker for NestedSafeChecker {
    fn name(&self) -> &'static str {
        "nested_safe"
    }

    async fn check(&self, transaction: &SafeTransaction, _context: &CheckContext) -> Verdict {
        if is_nested_exec_transaction(transaction) {
            Verdict::Secure
        } else {
            Verdict::Abstain
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
