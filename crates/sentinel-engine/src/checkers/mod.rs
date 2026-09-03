//! Transaction checks run by the sentinel engine.

mod address_poisoning;
mod base;
mod blocklist;
mod cancellation;
mod cow;
mod escape_hatch;
mod excessive_approval;
mod staking;

pub use self::{
    address_poisoning::AddressPoisoningChecker, base::BaseChecker, blocklist::BlocklistChecker,
    cancellation::CancellationChecker, cow::CowChecker, escape_hatch::EscapeHatchChecker,
    excessive_approval::ExcessiveApprovalChecker, staking::StakingChecker,
};

use crate::engine::{CheckContext, SafeTransaction, Verdict};

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
