use crate::{bindings::consensus::SafeTransaction, checker::CheckOutcome};
use alloy::primitives::U256;
use safe_tx::{
    rule::RuleId,
    target_effects::{EffectKind, decode_target_effects},
};

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

/// R-4.5: denies functionally unlimited approvals (§2.5's deterministic
/// sub-case only — max `uint256` for ERC-20, "approval for all tokens" for
/// ERC-721/1155 operator approvals). Recurses through MultiSend via
/// `decode_target_effects`, so a batched unlimited approval is caught the
/// same as a standalone one.
struct ExcessiveApproval;

impl Check for ExcessiveApproval {
    fn evaluate(&self, tx: &safe_tx::SafeTransaction) -> Result<(), RuleId> {
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
/// as opposed to the sentinel engine's externally-pluggable, potentially
/// statistical/time-varying checks. Built once at construction, in a fixed
/// evaluation order.
pub struct StaticChecker {
    checks: Vec<Box<dyn Check>>,
}

impl StaticChecker {
    pub fn new() -> Self {
        Self {
            checks: vec![Box::new(BaseGuarantees), Box::new(ExcessiveApproval)],
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
    use alloy::sol_types::SolCall as _;

    alloy::sol! {
        function approve(address spender, uint256 amount) external;
        function setApprovalForAll(address operator, bool approved) external;
    }

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

    #[test]
    fn denied_unlimited_erc20_approval() {
        let checker = StaticChecker::new();
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
        assert_eq!(
            decision,
            CheckOutcome::Denied(RuleId::R4_5ExcessiveApproval)
        );
    }

    #[test]
    fn approved_bounded_erc20_approval() {
        let checker = StaticChecker::new();
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
        assert_eq!(decision, CheckOutcome::Approved);
    }

    #[test]
    fn denied_operator_approval_for_all() {
        let checker = StaticChecker::new();
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
        assert_eq!(
            decision,
            CheckOutcome::Denied(RuleId::R4_5ExcessiveApproval)
        );
    }

    #[test]
    fn approved_operator_approval_revocation() {
        let checker = StaticChecker::new();
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
        assert_eq!(decision, CheckOutcome::Approved);
    }
}
