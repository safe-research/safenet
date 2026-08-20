use crate::{bindings::consensus::SafeTransaction, checker::CheckOutcome};
use safe_tx::rule::RuleId;

/// A single policy check, evaluated against a proposed transaction.
/// `StaticChecker::check` runs its checks in a fixed order and stops at the
/// first denial.
trait Check {
    fn evaluate(&self, tx: &safe_tx::SafeTransaction) -> Result<(), RuleId>;
}

/// Article IV Part A base guarantees, shared with the validator's
/// FROST-signing path.
struct BaseGuarantees;

impl Check for BaseGuarantees {
    fn evaluate(&self, tx: &safe_tx::SafeTransaction) -> Result<(), RuleId> {
        safe_tx::checks::check_transaction(tx)
    }
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
        Self {
            checks: vec![Box::new(BaseGuarantees)],
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::Address;

    const A1: Address = Address::new([1u8; 20]);
    const A2: Address = Address::new([2u8; 20]);

    #[test]
    fn denied_self_call_not_on_settings_allow_list() {
        let checker = StaticChecker::new();
        let safe = A1;
        let decision = checker.check(&SafeTransaction {
            safe,
            to: safe,
            data: vec![0xde, 0xad, 0xbe, 0xef].into(),
            ..Default::default()
        });
        assert_eq!(decision, CheckOutcome::Denied(RuleId::R4_1SettingsChange));
    }

    #[test]
    fn denied_delegatecall_to_unknown_target() {
        let checker = StaticChecker::new();
        let decision = checker.check(&SafeTransaction {
            safe: A1,
            to: A2,
            operation: crate::bindings::consensus::Operation::DELEGATECALL,
            ..Default::default()
        });
        assert_eq!(
            decision,
            CheckOutcome::Denied(RuleId::R4_2DelegatecallIntegrity)
        );
    }
}
