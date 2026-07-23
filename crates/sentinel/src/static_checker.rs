use crate::bindings::consensus::SafeTransaction;
use alloy::primitives::{Address, U256};
use safe_tx::rule::RuleId;
use safe_tx::target_effects::{EffectKind, decode_target_effects};
use std::{borrow::Cow, collections::HashSet};

/// A [`StaticChecker`]'s verdict on a proposed oracle transaction: whether to
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
/// type. `StaticChecker::check` runs its checks in a fixed order and stops at
/// the first denial. New checks (R-4.5, R-4.3/R-4.4, ...) plug in here as
/// later phases of the epic ship them.
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

/// R-4.5: denies functionally unlimited approvals (§2.5's deterministic
/// sub-case only — max `uint256` for ERC-20, "approval for all tokens" for
/// ERC-721/1155 operator approvals). Recurses through MultiSend via
/// `decode_target_effects`, so a batched unlimited approval is caught the
/// same as a standalone one.
struct ExcessiveApproval;

impl Check for ExcessiveApproval {
    fn evaluate(&self, tx: &safe_tx::types::SafeTransaction) -> Result<(), RuleId> {
        for effect in decode_target_effects(tx) {
            let unlimited = match effect.kind {
                EffectKind::Erc20Approval { amount } => amount == U256::MAX,
                EffectKind::OperatorApproval { approved } => approved,
                _ => false,
            };
            if unlimited {
                return Err(RuleId::R4_5ExcessiveApproval);
            }
        }
        Ok(())
    }
}

/// Decides whether a proposed oracle transaction should be approved by
/// running deterministic, local, synchronous checks against its calldata —
/// as opposed to [`crate::dynamic_checker`]'s externally-pluggable,
/// potentially statistical/time-varying checks. Built once at construction,
/// in a fixed evaluation order.
pub struct StaticChecker {
    checks: Vec<Box<dyn Check>>,
}

impl StaticChecker {
    pub fn new(blocklist: impl IntoIterator<Item = Address>) -> Self {
        Self {
            checks: vec![
                Box::new(BaseGuarantees),
                Box::new(Blocklist(blocklist.into_iter().collect())),
                Box::new(ExcessiveApproval),
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
    use alloy::sol_types::SolCall as _;

    alloy::sol! {
        function approve(address spender, uint256 amount) external;
        function setApprovalForAll(address operator, bool approved) external;
    }

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
        let checker = StaticChecker::new(vec![A1, A2]);
        let decision = checker.check(&tx(A1));
        assert!(!decision.approve);
        assert_eq!(decision.reason, RuleId::R4_6KnownMaliciousTarget.code());
        assert!(!checker.check(&tx(A2)).approve);
    }

    #[test]
    fn approved_with_empty_blocklist() {
        let checker = StaticChecker::new(vec![]);
        assert!(checker.check(&tx(A1)).approve);
    }

    #[test]
    fn approved_when_not_blocklisted() {
        let checker = StaticChecker::new(vec![A1, A2]);
        let decision = checker.check(&tx(A3));
        assert!(decision.approve);
        assert_eq!(decision.reason, "");
    }

    #[test]
    fn denied_self_call_not_on_settings_allow_list() {
        let checker = StaticChecker::new(vec![]);
        let safe = A1;
        let decision = checker.check(&SafeTransaction {
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
        let checker = StaticChecker::new(vec![]);
        let decision = checker.check(&SafeTransaction {
            safe: A1,
            to: A2,
            operation: crate::bindings::consensus::Operation::DELEGATECALL,
            ..Default::default()
        });
        assert!(!decision.approve);
        assert_eq!(decision.reason, RuleId::R4_2DelegatecallIntegrity.code());
    }

    #[test]
    fn denied_unlimited_erc20_approval() {
        let checker = StaticChecker::new(vec![]);
        let data = approveCall {
            spender: A2,
            amount: U256::MAX,
        }
        .abi_encode();
        let decision = checker.check(&SafeTransaction {
            to: A1,
            data: data.into(),
            ..Default::default()
        });
        assert!(!decision.approve);
        assert_eq!(decision.reason, RuleId::R4_5ExcessiveApproval.code());
    }

    #[test]
    fn approved_bounded_erc20_approval() {
        let checker = StaticChecker::new(vec![]);
        let data = approveCall {
            spender: A2,
            amount: U256::from(1_000u64),
        }
        .abi_encode();
        let decision = checker.check(&SafeTransaction {
            to: A1,
            data: data.into(),
            ..Default::default()
        });
        assert!(decision.approve);
    }

    #[test]
    fn denied_operator_approval_for_all() {
        let checker = StaticChecker::new(vec![]);
        let data = setApprovalForAllCall {
            operator: A2,
            approved: true,
        }
        .abi_encode();
        let decision = checker.check(&SafeTransaction {
            to: A1,
            data: data.into(),
            ..Default::default()
        });
        assert!(!decision.approve);
        assert_eq!(decision.reason, RuleId::R4_5ExcessiveApproval.code());
    }

    #[test]
    fn approved_operator_approval_revocation() {
        let checker = StaticChecker::new(vec![]);
        let data = setApprovalForAllCall {
            operator: A2,
            approved: false,
        }
        .abi_encode();
        let decision = checker.check(&SafeTransaction {
            to: A1,
            data: data.into(),
            ..Default::default()
        });
        assert!(decision.approve);
    }
}
