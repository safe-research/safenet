//! Transaction checks run by the sentinel engine.

mod address_poisoning;
mod base;
mod blocklist;
mod cancellation;
mod cow;
mod escape_hatch;
mod excessive_approval;
mod nested;
mod refund;
mod staking;

pub use self::{
    address_poisoning::AddressPoisoningChecker, base::BaseChecker, blocklist::BlocklistChecker,
    cancellation::CancellationChecker, cow::CowChecker, escape_hatch::EscapeHatchChecker,
    excessive_approval::ExcessiveApprovalChecker, nested::NestedSafeChecker, refund::RefundChecker,
    staking::StakingChecker,
};

use crate::engine::{CallCoverage, CheckContext, Proposal, RuleId};
use std::sync::Arc;

/// What a single check concluded. Distinct from [`crate::engine::Verdict`],
/// which is the engine's own answer and the wire type — a check contributes
/// evidence, the engine reaches the verdict.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Assessment {
    /// The check found a violation. Denials are final.
    Insecure {
        /// The rule violated by the transaction.
        rule: RuleId,
    },
    /// The check found nothing wrong in `coverage`, and vouches for exactly
    /// those aspects — no more.
    Secure {
        /// The calls (and/or refund leg) this check vouches for.
        coverage: CallCoverage,
    },
    /// No opinion.
    Abstain,
}

/// A transaction check in the sentinel engine's checker chain.
#[async_trait::async_trait]
pub trait Checker: Send + Sync {
    /// A short, log-friendly identifier for this checker.
    fn name(&self) -> &'static str;

    /// Assesses `proposal` or abstains so the next checker can run.
    /// `context` carries caller-supplied hints outside the transaction
    /// itself (see [`CheckContext`]); most checks ignore it.
    async fn check(&self, proposal: &Proposal, context: &CheckContext) -> Assessment;
}

/// Lets an [`Arc`]-shared checker (e.g. one both run directly and wrapped by
/// another checker, like [`AddressPoisoningChecker`] and [`RefundChecker`])
/// be boxed into the engine's checker chain like any other.
#[async_trait::async_trait]
impl<T: Checker> Checker for Arc<T> {
    fn name(&self) -> &'static str {
        (**self).name()
    }

    async fn check(&self, proposal: &Proposal, context: &CheckContext) -> Assessment {
        (**self).check(proposal, context).await
    }
}
