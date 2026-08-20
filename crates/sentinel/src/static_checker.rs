use crate::{bindings::consensus::SafeTransaction, checker::CheckOutcome, engine::RuleId};

/// A single policy check, evaluated against a proposed transaction.
/// `StaticChecker::check` runs its checks in a fixed order and stops at the
/// first denial.
trait Check {
    fn evaluate(&self, tx: &safe_tx::SafeTransaction) -> Result<(), RuleId>;
}

/// Decides whether a proposed oracle transaction should be approved by
/// running deterministic, local, synchronous checks against its calldata —
/// as opposed to the sentinel engine's externally-pluggable, potentially
/// statistical/time-varying checks. Built once at construction, in a fixed
/// evaluation order.
pub struct StaticChecker {
    checks: Vec<Box<dyn Check>>,
}

impl StaticChecker {
    pub fn new() -> Self {
        Self { checks: vec![] }
    }

    pub fn check(&self, tx: &SafeTransaction) -> CheckOutcome {
        let Ok(tx) = tx.clone().try_into() else {
            tracing::error!(transaction = ?tx, "invalid Safe transaction value");
            return CheckOutcome::Unknown;
        };

        for check in &self.checks {
            if let Err(rule) = check.evaluate(&tx) {
                return CheckOutcome::Denied(rule);
            }
        }
        CheckOutcome::Approved
    }
}
