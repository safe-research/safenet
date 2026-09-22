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
//!
//! Evaluates every call of `proposal.calls`, not just a top-level one:
//! affirming requires *every* call in a batch to itself be a nested
//! `execTransaction` call — a batch mixing one in with an unrelated call
//! still abstains, since this checker cannot vouch for the unrelated call's
//! own action. When it does affirm, `To | Data | Operation` is claimed for
//! every call index, not a proper subset.

use super::{Assessment, Checker};
use crate::{
    contracts::bindings::safe,
    engine::{AspectSet, CheckContext, MetaTransaction, Operation, Proposal},
};
use alloy::{primitives::Address, sol_types::SolCall as _};

/// Considers the `to`, `data` and `operation` of a call to another
/// contract's `execTransaction` secure, regardless of the nested transaction
/// it carries.
pub struct NestedSafeChecker;

#[async_trait::async_trait]
impl Checker for NestedSafeChecker {
    fn name(&self) -> &'static str {
        "nested_safe"
    }

    async fn check(&self, proposal: &Proposal, _context: &CheckContext) -> Assessment {
        let safe_address = proposal.transaction.safe;
        if proposal
            .calls
            .iter()
            .all(|call| is_nested_exec_transaction(safe_address, call))
        {
            Assessment::Secure {
                coverage: proposal.checked(AspectSet::TO | AspectSet::DATA | AspectSet::OPERATION),
            }
        } else {
            Assessment::Abstain
        }
    }
}

/// A `Call` (never a delegatecall) to a different address than
/// `safe_address`, carrying `execTransaction` calldata for that address to
/// decode and enforce on its own terms.
fn is_nested_exec_transaction(safe_address: Address, call: &MetaTransaction) -> bool {
    call.operation == Operation::Call
        && call.to != safe_address
        && call.data.starts_with(&safe::execTransactionCall::SELECTOR)
        && safe::execTransactionCall::abi_decode(&call.data).is_ok()
}
