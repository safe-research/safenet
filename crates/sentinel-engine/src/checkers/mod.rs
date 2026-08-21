//! Transaction checks run by the sentinel engine.

mod address_poisoning;
mod base;
mod blocklist;
mod cancellation;
mod cow;
mod excessive_approval;

pub use self::{
    address_poisoning::AddressPoisoningChecker, base::BaseChecker, blocklist::BlocklistChecker,
    cancellation::CancellationChecker, cow::CowChecker,
    excessive_approval::ExcessiveApprovalChecker,
};

use crate::engine::{SafeTransaction, Verdict};

/// A transaction check in the sentinel engine's checker chain.
#[async_trait::async_trait]
pub trait Checker: Send + Sync {
    /// Assesses `transaction` or abstains so the next checker can run.
    async fn check(&self, transaction: &SafeTransaction) -> Verdict;
}
