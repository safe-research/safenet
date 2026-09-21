//! Recognition of SafenetGuard escape-hatch calls.

use super::{Assessment, Checker};
use crate::{
    contracts::bindings::safenet_guard,
    engine::{CheckContext, Coverage, Operation, Proposal, SafeTransaction},
};
use alloy::sol_types::SolCall as _;

/// Considers a Safe's own call into a SafenetGuard escape-hatch function
/// secure, independent of the transaction it announces or cancels.
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
/// Claims `Coverage::action()`, not `Refund`: a relayed call (nonzero
/// `gasPrice`) is just as structurally safe, but this checker has no way to
/// tell a trusted relayer from an untrusted one, so the refund leg is left
/// to `RefundChecker` — or, absent a voucher for it, to the engine's
/// abstention.
pub struct EscapeHatchChecker;

#[async_trait::async_trait]
impl Checker for EscapeHatchChecker {
    fn name(&self) -> &'static str {
        "escape_hatch"
    }

    async fn check(&self, proposal: &Proposal, _context: &CheckContext) -> Assessment {
        if is_escape_hatch_call(&proposal.transaction) {
            Assessment::Secure {
                coverage: Coverage::action(),
            }
        } else {
            Assessment::Abstain
        }
    }
}

/// True if `tx` is a zero-value `CALL` whose calldata invokes one of the two
/// SafenetGuard escape-hatch functions.
fn is_escape_hatch_call(tx: &SafeTransaction) -> bool {
    if tx.operation != Operation::Call || !tx.value.is_zero() {
        return false;
    }
    tx.data
        .starts_with(&safenet_guard::announceTransactionCall::SELECTOR)
        || tx
            .data
            .starts_with(&safenet_guard::cancelAnnouncementCall::SELECTOR)
}
