//! Transaction checks run by the sentinel engine.

mod address_poisoning;
mod blocklist;
mod cancellation;
mod cow;

pub use self::{
    address_poisoning::AddressPoisoningChecker, blocklist::BlocklistChecker,
    cancellation::CancellationChecker, cow::CowChecker,
};

use crate::engine::Verdict;
use safe_tx::SafeTransaction;

/// A transaction check in the sentinel engine's checker chain.
#[async_trait::async_trait]
pub trait Checker: Send + Sync {
    /// Assesses `transaction` or abstains so the next checker can run.
    async fn check(&self, transaction: &SafeTransaction) -> Verdict;
}
