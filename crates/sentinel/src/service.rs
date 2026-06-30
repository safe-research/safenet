use crate::{
    action::{SentinelAction, SentinelActionKind},
    bindings::oracle::{ERC20, SentinelOracle},
};
use alloy::{
    primitives::{Address, U256},
    sol_types::SolCall,
};
use safenet_core::tx::Transaction;

/// The sentinel service: maps actions to encoded transactions.
///
/// Holds the oracle and fee-token addresses needed to route each action to
/// the correct contract. Plugs into the `Driver` once it lands (PR #486) by
/// also implementing `StateTransition` (B2) and `Service` (D1, here).
pub struct SentinelService {
    oracle: Address,
    fee_token: Address,
}

impl SentinelService {
    #[cfg_attr(not(test), expect(dead_code))]
    pub fn new(oracle: Address, fee_token: Address) -> Self {
        Self { oracle, fee_token }
    }

    /// Maps each action to a `(Transaction, expires_at)` pair for the queue.
    ///
    /// The `expires_at` is the request's voting deadline, forwarded from the
    /// action so the `TransactionQueue` can drop it if it goes unsubmitted
    /// past that block.
    #[cfg_attr(not(test), expect(dead_code))]
    pub fn encode_actions(&self, actions: Vec<SentinelAction>) -> Vec<(Transaction, u64)> {
        actions
            .into_iter()
            .map(|SentinelAction { kind, expires_at }| (self.encode_action(kind), expires_at))
            .collect()
    }

    fn encode_action(&self, kind: SentinelActionKind) -> Transaction {
        match kind {
            SentinelActionKind::ApproveToken { bond } => Transaction {
                to: self.fee_token,
                value: U256::ZERO,
                data: ERC20::approveCall {
                    spender: self.oracle,
                    amount: bond,
                }
                .abi_encode()
                .into(),
                gas: 55_000,
            },
            SentinelActionKind::CommitApprove { id } => Transaction {
                to: self.oracle,
                value: U256::ZERO,
                data: SentinelOracle::commitApproveCall { requestId: id }
                    .abi_encode()
                    .into(),
                gas: 100_000,
            },
            SentinelActionKind::CommitDeny { id } => Transaction {
                to: self.oracle,
                value: U256::ZERO,
                data: SentinelOracle::commitDenyCall { requestId: id }
                    .abi_encode()
                    .into(),
                gas: 100_000,
            },
            SentinelActionKind::Finalize { id } => Transaction {
                to: self.oracle,
                value: U256::ZERO,
                data: SentinelOracle::finalizeCall { requestId: id }
                    .abi_encode()
                    .into(),
                gas: 100_000,
            },
            SentinelActionKind::Claim { id } => Transaction {
                to: self.oracle,
                value: U256::ZERO,
                data: SentinelOracle::claimCall { requestId: id }
                    .abi_encode()
                    .into(),
                gas: 100_000,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{B256, address, uint};

    const ORACLE: Address = address!("1111111111111111111111111111111111111111");
    const FEE_TOKEN: Address = address!("2222222222222222222222222222222222222222");

    fn service() -> SentinelService {
        SentinelService::new(ORACLE, FEE_TOKEN)
    }

    #[test]
    fn encodes_approve_token() {
        let bond = uint!(1_000_U256);
        let tx = service().encode_action(SentinelActionKind::ApproveToken { bond });

        assert_eq!(tx.to, FEE_TOKEN);
        assert_eq!(tx.value, U256::ZERO);
        assert_eq!(tx.gas, 55_000);
        assert_eq!(
            tx.data.as_ref(),
            ERC20::approveCall {
                spender: ORACLE,
                amount: bond
            }
            .abi_encode(),
        );
    }

    #[test]
    fn encodes_commit_approve() {
        let id = B256::repeat_byte(0x01);
        let tx = service().encode_action(SentinelActionKind::CommitApprove { id });

        assert_eq!(tx.to, ORACLE);
        assert_eq!(tx.value, U256::ZERO);
        assert_eq!(tx.gas, 100_000);
        assert_eq!(
            tx.data.as_ref(),
            SentinelOracle::commitApproveCall { requestId: id }.abi_encode(),
        );
    }

    #[test]
    fn encodes_commit_deny() {
        let id = B256::repeat_byte(0x02);
        let tx = service().encode_action(SentinelActionKind::CommitDeny { id });

        assert_eq!(tx.to, ORACLE);
        assert_eq!(tx.value, U256::ZERO);
        assert_eq!(tx.gas, 100_000);
        assert_eq!(
            tx.data.as_ref(),
            SentinelOracle::commitDenyCall { requestId: id }.abi_encode(),
        );
    }

    #[test]
    fn encodes_finalize() {
        let id = B256::repeat_byte(0x03);
        let tx = service().encode_action(SentinelActionKind::Finalize { id });

        assert_eq!(tx.to, ORACLE);
        assert_eq!(tx.value, U256::ZERO);
        assert_eq!(tx.gas, 100_000);
        assert_eq!(
            tx.data.as_ref(),
            SentinelOracle::finalizeCall { requestId: id }.abi_encode(),
        );
    }

    #[test]
    fn encodes_claim() {
        let id = B256::repeat_byte(0x04);
        let tx = service().encode_action(SentinelActionKind::Claim { id });

        assert_eq!(tx.to, ORACLE);
        assert_eq!(tx.value, U256::ZERO);
        assert_eq!(tx.gas, 100_000);
        assert_eq!(
            tx.data.as_ref(),
            SentinelOracle::claimCall { requestId: id }.abi_encode(),
        );
    }

    #[test]
    fn encode_actions_forwards_expiry() {
        let bond = uint!(500_U256);
        let id = B256::repeat_byte(0xab);
        let deadline = 999u64;
        let encoded = service().encode_actions(vec![
            SentinelAction {
                kind: SentinelActionKind::ApproveToken { bond },
                expires_at: deadline,
            },
            SentinelAction {
                kind: SentinelActionKind::CommitApprove { id },
                expires_at: deadline,
            },
        ]);

        assert_eq!(encoded.len(), 2);
        assert_eq!(encoded[0].0.to, FEE_TOKEN);
        assert_eq!(encoded[1].0.to, ORACLE);
        assert_eq!(encoded[0].1, deadline);
        assert_eq!(encoded[1].1, deadline);
    }
}
