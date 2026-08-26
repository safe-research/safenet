//! Recognition of SafenetGuard escape-hatch calls.

use super::Checker;
use crate::{
    contracts::bindings::safenet_guard,
    engine::{Operation, SafeTransaction, Verdict},
};
use alloy::sol_types::SolCall as _;

/// Considers a Safe's own call into a SafenetGuard escape-hatch function
/// secure.
///
/// `announceTransaction`/`cancelAnnouncement` only register or clear a
/// nonce-free announcement keyed by the calling Safe (`msg.sender`); per
/// §2.18 of the Safenet Charter this is always allowed, regardless of what
/// transaction is being announced — that transaction is checked separately,
/// on its own merits, when it is itself proposed for execution. This holds
/// structurally for any `to`, not just a canonical, registered SafenetGuard
/// deployment (none is tracked here): a zero-value plain `CALL` cannot move
/// the Safe's funds or touch its storage. Both guards matter — a
/// `DELEGATECALL` would run arbitrary code from `to` inside the Safe's own
/// storage context (which is exactly why `SafenetGuard` itself refuses to
/// auto-allow anything but a `CALL`), and nonzero value handed to an
/// unrelated `to` would simply be spent.
///
/// Relayed calls (nonzero `gasPrice`) are excluded and abstained on rather
/// than affirmed: Safe pays a refund out of its own funds to
/// `refundReceiver` (or `tx.origin` when unset), and this checker doesn't
/// yet have a way to tell a trusted relayer from an untrusted one. Deciding
/// that is a follow-up. `baseGas` alone doesn't gate this — Safe.sol only
/// calls `handlePayment` `if (gasPrice > 0)`, so a nonzero `baseGas` next to
/// a zero `gasPrice` pays nothing.
pub struct EscapeHatchChecker;

#[async_trait::async_trait]
impl Checker for EscapeHatchChecker {
    fn name(&self) -> &'static str {
        "escape_hatch"
    }

    async fn check(&self, transaction: &SafeTransaction) -> Verdict {
        if is_escape_hatch_call(transaction) {
            Verdict::Secure
        } else {
            Verdict::Abstain
        }
    }
}

/// True if `tx` is an unrelayed, zero-value `CALL` whose calldata invokes
/// one of the two SafenetGuard escape-hatch functions.
fn is_escape_hatch_call(tx: &SafeTransaction) -> bool {
    if tx.operation != Operation::Call || !tx.value.is_zero() || !tx.gas_price.is_zero() {
        return false;
    }
    tx.data
        .starts_with(&safenet_guard::announceTransactionCall::SELECTOR)
        || tx
            .data
            .starts_with(&safenet_guard::cancelAnnouncementCall::SELECTOR)
}
