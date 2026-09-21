//! Detection of functionally unlimited token allowances.

use super::{Assessment, Checker};
use crate::{
    contracts::target_effects::{EffectKind, decode_target_effects},
    engine::{CheckContext, Proposal, RuleId},
};
use alloy::primitives::U256;

/// Denies functionally unlimited token approvals.
pub struct ExcessiveApprovalChecker;

#[async_trait::async_trait]
impl Checker for ExcessiveApprovalChecker {
    fn name(&self) -> &'static str {
        "excessive_approval"
    }

    async fn check(&self, proposal: &Proposal, _context: &CheckContext) -> Assessment {
        for effect in decode_target_effects(&proposal.transaction) {
            let unlimited = match effect.kind {
                EffectKind::Erc20Approval { amount } => amount == U256::MAX,
                EffectKind::OperatorApproval { approved } => approved,
                _ => false,
            };
            if unlimited {
                return Assessment::Insecure {
                    rule: RuleId::R4_5ExcessiveApproval,
                };
            }
        }
        Assessment::Abstain
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::SafeTransaction;
    use alloy::{primitives::Address, sol_types::SolCall as _};

    alloy::sol! {
        function approve(address spender, uint256 amount) external;
        function setApprovalForAll(address operator, bool approved) external;
    }

    const TOKEN: Address = Address::new([1u8; 20]);
    const SPENDER: Address = Address::new([2u8; 20]);

    #[tokio::test]
    async fn denies_unlimited_erc20_approval() {
        let transaction = SafeTransaction {
            to: TOKEN,
            data: approveCall {
                spender: SPENDER,
                amount: U256::MAX,
            }
            .abi_encode()
            .into(),
            ..Default::default()
        };

        assert_eq!(
            ExcessiveApprovalChecker
                .check(&Proposal::from(transaction), &CheckContext::default())
                .await,
            Assessment::Insecure {
                rule: RuleId::R4_5ExcessiveApproval,
            }
        );
    }

    #[tokio::test]
    async fn abstains_on_bounded_erc20_approval() {
        let transaction = SafeTransaction {
            to: TOKEN,
            data: approveCall {
                spender: SPENDER,
                amount: U256::from(1_000u64),
            }
            .abi_encode()
            .into(),
            ..Default::default()
        };

        assert_eq!(
            ExcessiveApprovalChecker
                .check(&Proposal::from(transaction), &CheckContext::default())
                .await,
            Assessment::Abstain
        );
    }

    #[tokio::test]
    async fn denies_operator_approval_for_all() {
        let transaction = SafeTransaction {
            to: TOKEN,
            data: setApprovalForAllCall {
                operator: SPENDER,
                approved: true,
            }
            .abi_encode()
            .into(),
            ..Default::default()
        };

        assert_eq!(
            ExcessiveApprovalChecker
                .check(&Proposal::from(transaction), &CheckContext::default())
                .await,
            Assessment::Insecure {
                rule: RuleId::R4_5ExcessiveApproval,
            }
        );
    }

    #[tokio::test]
    async fn abstains_on_operator_approval_revocation() {
        let transaction = SafeTransaction {
            to: TOKEN,
            data: setApprovalForAllCall {
                operator: SPENDER,
                approved: false,
            }
            .abi_encode()
            .into(),
            ..Default::default()
        };

        assert_eq!(
            ExcessiveApprovalChecker
                .check(&Proposal::from(transaction), &CheckContext::default())
                .await,
            Assessment::Abstain
        );
    }
}
