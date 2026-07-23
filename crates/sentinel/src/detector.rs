use crate::bindings::consensus::SafeTransaction;
use alloy::primitives::Address;
use safe_tx::rule::RuleId;
use std::{borrow::Cow, collections::HashSet};

/// The detector's verdict on a proposed oracle transaction: whether to
/// approve it, and the justification to carry, verbatim, into the blind
/// commit-reveal vote. `reason` is always a static string literal today, so
/// `Cow` avoids allocating one on every `check` call.
///
/// Deliberately does not carry a `RuleId` — that's an evaluation-time
/// concept, known only while a check runs, not part of the onchain-facing
/// protocol `Decision` feeds into. A denial's `reason` is rendered from
/// `RuleId::code()` (see `safe_tx::rule`), so it's still structurally tied
/// to the cited rule, but `RuleId` itself never leaves the check layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub approve: bool,
    pub reason: Cow<'static, str>,
}

impl Decision {
    fn approved() -> Self {
        Self {
            approve: true,
            reason: Cow::Borrowed(""),
        }
    }

    fn denied(rule: RuleId) -> Self {
        Self {
            approve: false,
            reason: Cow::Borrowed(rule.code()),
        }
    }
}

/// A single policy check, evaluated against the shared `safe-tx` transaction
/// type. `Detector::check` runs its checks in a fixed order and stops at the
/// first denial. New checks (R-4.5, R-4.3/R-4.4, ...) plug in here as later
/// phases of the epic ship them.
trait Check {
    fn evaluate(&self, tx: &safe_tx::types::SafeTransaction) -> Result<(), RuleId>;
}

/// Article IV Part A base guarantees, shared with the validator's
/// FROST-signing path.
struct BaseGuarantees;

impl Check for BaseGuarantees {
    fn evaluate(&self, tx: &safe_tx::types::SafeTransaction) -> Result<(), RuleId> {
        safe_tx::checks::check_transaction(tx)
    }
}

/// The static destination blocklist, reclassified as R-4.6 (see
/// [`RuleId::R4_6KnownMaliciousTarget`] for the MVP caveat). Never changes
/// once the check is created.
struct Blocklist(HashSet<Address>);

impl Check for Blocklist {
    fn evaluate(&self, tx: &safe_tx::types::SafeTransaction) -> Result<(), RuleId> {
        if self.0.contains(&tx.to) {
            Err(RuleId::R4_6KnownMaliciousTarget)
        } else {
            Ok(())
        }
    }
}

/// Decides whether a proposed oracle transaction should be approved by
/// running its checks, in a fixed order, built once at construction.
pub struct Detector {
    checks: Vec<Box<dyn Check>>,
}

impl Detector {
    pub fn new(blocklist: impl IntoIterator<Item = Address>) -> Self {
        Self {
            checks: vec![
                Box::new(BaseGuarantees),
                Box::new(Blocklist(blocklist.into_iter().collect())),
            ],
        }
    }

    pub fn check(&self, tx: &SafeTransaction) -> Decision {
        let shared_tx: safe_tx::types::SafeTransaction = tx.into();

        for check in &self.checks {
            if let Err(rule) = check.evaluate(&shared_tx) {
                return Decision::denied(rule);
            }
        }
        Decision::approved()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A1: Address = Address::new([1u8; 20]);
    const A2: Address = Address::new([2u8; 20]);
    const A3: Address = Address::new([3u8; 20]);

    fn tx(to: Address) -> SafeTransaction {
        SafeTransaction {
            to,
            ..Default::default()
        }
    }

    #[test]
    fn denied_when_blocklisted() {
        let detector = Detector::new(vec![A1, A2]);
        let decision = detector.check(&tx(A1));
        assert!(!decision.approve);
        assert_eq!(decision.reason, RuleId::R4_6KnownMaliciousTarget.code());
        assert!(!detector.check(&tx(A2)).approve);
    }

    #[test]
    fn approved_with_empty_blocklist() {
        let detector = Detector::new(vec![]);
        assert!(detector.check(&tx(A1)).approve);
    }

    #[test]
    fn approved_when_not_blocklisted() {
        let detector = Detector::new(vec![A1, A2]);
        let decision = detector.check(&tx(A3));
        assert!(decision.approve);
        assert_eq!(decision.reason, "");
    }

    #[test]
    fn denied_self_call_not_on_settings_allow_list() {
        let detector = Detector::new(vec![]);
        let safe = A1;
        let decision = detector.check(&SafeTransaction {
            safe,
            to: safe,
            data: vec![0xde, 0xad, 0xbe, 0xef].into(),
            ..Default::default()
        });
        assert!(!decision.approve);
        assert_eq!(decision.reason, RuleId::R4_1SettingsChange.code());
    }

    #[test]
    fn denied_delegatecall_to_unknown_target() {
        let detector = Detector::new(vec![]);
        let decision = detector.check(&SafeTransaction {
            safe: A1,
            to: A2,
            operation: crate::bindings::consensus::Operation::DELEGATECALL,
            ..Default::default()
        });
        assert!(!decision.approve);
        assert_eq!(decision.reason, RuleId::R4_2DelegatecallIntegrity.code());
    }
}
