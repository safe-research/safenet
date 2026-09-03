//! Transaction checks run by the sentinel engine.

mod address_poisoning;
mod base;
mod blocklist;
mod cancellation;
mod cow;
mod escape_hatch;
mod excessive_approval;
mod refund;
mod staking;

pub use self::{
    address_poisoning::AddressPoisoningChecker, base::BaseChecker, blocklist::BlocklistChecker,
    cancellation::CancellationChecker, cow::CowChecker, escape_hatch::EscapeHatchChecker,
    excessive_approval::ExcessiveApprovalChecker, refund::RefundChecker, staking::StakingChecker,
};

use crate::engine::{CheckContext, SafeTransaction, Verdict};
use std::sync::Arc;

/// A transaction check in the sentinel engine's checker chain.
#[async_trait::async_trait]
pub trait Checker: Send + Sync {
    /// A short, log-friendly identifier for this checker.
    fn name(&self) -> &'static str;

    /// Assesses `transaction` or abstains so the next checker can run.
    /// `context` carries caller-supplied hints outside the transaction
    /// itself (see [`CheckContext`]); most checks ignore it.
    async fn check(&self, transaction: &SafeTransaction, context: &CheckContext) -> Verdict;
}

/// Lets an [`Arc`]-shared checker (e.g. one both run directly and wrapped by
/// another checker, like [`AddressPoisoningChecker`] and [`RefundChecker`])
/// be boxed into the engine's checker chain like any other.
#[async_trait::async_trait]
impl<T: Checker> Checker for Arc<T> {
    fn name(&self) -> &'static str {
        (**self).name()
    }

    async fn check(&self, transaction: &SafeTransaction, context: &CheckContext) -> Verdict {
        (**self).check(transaction, context).await
    }
}
