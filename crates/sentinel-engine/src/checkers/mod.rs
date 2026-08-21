//! Transaction checks run by the sentinel engine.

mod address_poisoning;
mod cancellation;

pub use self::{address_poisoning::AddressPoisoningChecker, cancellation::CancellationChecker};

use crate::engine::Verdict;
use safe_tx::SafeTransaction;

/// A transaction check in the sentinel engine's checker chain.
#[async_trait::async_trait]
pub trait Checker: Send + Sync {
    /// Assesses `transaction` or abstains so the next checker can run.
    async fn check(&self, transaction: &SafeTransaction) -> Verdict;
}
