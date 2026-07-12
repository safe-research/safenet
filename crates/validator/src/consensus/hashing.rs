//! EIP-712 hashing for the packets validators attest and sign over.
//!
//! Every packet is hashed in two domains: the classic Safe transaction hash
//! is computed in the *target Safe's* domain (`chainId`/`safe` from the
//! transaction itself), then embedded as a field of a small packet struct
//! hashed in the *validator network's* domain (the chain the `Consensus`
//! contract lives on, and the contract's own address). The latter hash is
//! the message a validator group's FROST signature actually attests to.

use crate::{
    bindings::{Point, SafeTransaction},
    consensus::epoch::EpochId,
};
use alloy::{
    primitives::{Address, B256, U256},
    sol,
    sol_types::{Eip712Domain, SolStruct},
};
use std::num::NonZeroU64;

sol! {
    /// The classic Safe `SafeTx` EIP-712 struct.
    struct SafeTx {
        address to;
        uint256 value;
        bytes data;
        uint8 operation;
        uint256 safeTxGas;
        uint256 baseGas;
        uint256 gasPrice;
        address gasToken;
        address refundReceiver;
        uint256 nonce;
    }

    /// The consensus-domain packet proposing a Safe transaction for
    /// attestation.
    struct TransactionProposal {
        uint64 epoch;
        bytes32 safeTxHash;
    }

    /// The consensus-domain packet proposing an oracle-backed Safe
    /// transaction for attestation.
    struct OracleTransactionProposal {
        uint64 epoch;
        address oracle;
        bytes32 safeTxHash;
    }

    /// The consensus-domain packet proposing an epoch rollover for
    /// attestation.
    struct EpochRollover {
        uint64 activeEpoch;
        uint64 proposedEpoch;
        uint64 rolloverBlock;
        uint256 groupKeyX;
        uint256 groupKeyY;
    }
}

impl From<&SafeTransaction> for SafeTx {
    fn from(tx: &SafeTransaction) -> Self {
        SafeTx {
            to: tx.to,
            value: tx.value,
            data: tx.data.clone(),
            operation: tx.operation as u8,
            safeTxGas: tx.safeTxGas,
            baseGas: tx.baseGas,
            gasPrice: tx.gasPrice,
            gasToken: tx.gasToken,
            refundReceiver: tx.refundReceiver,
            nonce: tx.nonce,
        }
    }
}

/// The classic Safe transaction hash: `tx`'s `SafeTx` struct hashed in
/// the target Safe's own domain (`chainId`/`safe`).
pub fn safe_tx_hash(tx: &SafeTransaction) -> B256 {
    let domain = Eip712Domain::new(None, None, Some(tx.chainId), Some(tx.safe), None);
    SafeTx::from(tx).eip712_signing_hash(&domain)
}

/// The EIP-712 domain for the Safenet consensus.
pub struct ConsensusDomain(Eip712Domain);

impl ConsensusDomain {
    /// Builds the domain a validator network's own consensus group attests
    /// over: the chain the `Consensus` contract lives on and its address.
    pub const fn new(chain: u64, consensus: Address) -> Self {
        Self(Eip712Domain::new(
            None,
            None,
            Some(U256::from_limbs([chain, 0, 0, 0])),
            Some(consensus),
            None,
        ))
    }

    /// The consensus-domain hash of a Safe transaction proposal, embedding the
    /// already-computed [`safe_tx_hash`] as its `safeTxHash` field.
    pub fn transaction_proposal_hash(&self, epoch: EpochId, safe_tx_hash: B256) -> B256 {
        TransactionProposal {
            epoch: epoch.raw_value(),
            safeTxHash: safe_tx_hash,
        }
        .eip712_signing_hash(&self.0)
    }

    /// The consensus-domain hash of a Safe transaction packet: shorthand for
    /// [`transaction_proposal_hash`] with [`safe_tx_hash`] computed from `tx`.
    pub fn transaction_packet_hash(&self, epoch: EpochId, tx: &SafeTransaction) -> B256 {
        self.transaction_proposal_hash(epoch, safe_tx_hash(tx))
    }

    /// The consensus-domain hash of an oracle-backed Safe transaction proposal,
    /// embedding the already-computed [`safe_tx_hash`] as its `safeTxHash` field.
    pub fn oracle_transaction_proposal_hash(
        &self,
        epoch: EpochId,
        oracle: Address,
        safe_tx_hash: B256,
    ) -> B256 {
        OracleTransactionProposal {
            epoch: epoch.raw_value(),
            oracle,
            safeTxHash: safe_tx_hash,
        }
        .eip712_signing_hash(&self.0)
    }

    /// The consensus-domain hash of an oracle-backed Safe transaction packet:
    /// shorthand for [`oracle_transaction_proposal_hash`] with [`safe_tx_hash`]
    /// computed from `tx`.
    pub fn oracle_transaction_packet_hash(
        &self,
        epoch: EpochId,
        oracle: Address,
        tx: &SafeTransaction,
    ) -> B256 {
        self.oracle_transaction_proposal_hash(epoch, oracle, safe_tx_hash(tx))
    }

    /// The consensus-domain hash of an epoch rollover proposal.
    pub fn epoch_rollover_hash(
        &self,
        active_epoch: EpochId,
        proposed_epoch: NonZeroU64,
        rollover_block: u64,
        group_key: &Point,
    ) -> B256 {
        EpochRollover {
            activeEpoch: active_epoch.raw_value(),
            proposedEpoch: proposed_epoch.get(),
            rolloverBlock: rollover_block,
            groupKeyX: group_key.x,
            groupKeyY: group_key.y,
        }
        .eip712_signing_hash(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::Operation;
    use alloy::primitives::{Bytes, address, b256};

    const TEST_ADDRESS: Address = address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
    const TEST_DOMAIN: ConsensusDomain = ConsensusDomain::new(1, TEST_ADDRESS);
    const EPOCH_ONE: EpochId = EpochId::from_raw(1);

    fn safe_tx() -> SafeTransaction {
        SafeTransaction {
            chainId: U256::from(1u64),
            safe: TEST_ADDRESS,
            to: TEST_ADDRESS,
            value: U256::ZERO,
            data: Bytes::new(),
            operation: Operation::CALL,
            safeTxGas: U256::ZERO,
            baseGas: U256::ZERO,
            gasPrice: U256::ZERO,
            gasToken: Address::ZERO,
            refundReceiver: Address::ZERO,
            nonce: U256::ZERO,
        }
    }

    #[test]
    fn reference_safe_tx_hash() {
        assert_eq!(
            safe_tx_hash(&safe_tx()),
            b256!("fe8b85e8d090b16fe8f142d3c9292dc1fc77daf9eb4af8f7cf4a7707d95f4028")
        );
    }

    #[test]
    fn reference_transaction_packet_hash() {
        assert_eq!(
            TEST_DOMAIN.transaction_packet_hash(EPOCH_ONE, &safe_tx()),
            b256!("3ff98ecae85843603560e9509346df2f35c0ad1dd1ceda5dcbb145745dfc4e00")
        );
    }

    #[test]
    fn sample_oracle_transaction_packet_hash() {
        assert_eq!(
            TEST_DOMAIN.oracle_transaction_packet_hash(EPOCH_ONE, TEST_ADDRESS, &safe_tx()),
            b256!("b89cd5ddc8b9a71c6469b79711f8ce0000edd6fc3f47ad057a772302fcfa82af")
        );
    }

    #[test]
    fn sample_epoch_rollover_hash() {
        let group_key = Point {
            x: U256::from(1u64),
            y: U256::from(2u64),
        };
        assert_eq!(
            TEST_DOMAIN.epoch_rollover_hash(EpochId::Genesis, NonZeroU64::MIN, 1000, &group_key),
            b256!("75b33b36b42d249c4cccf1c86bce59897c0ebbaa829ab5d8926e1bff1cee4355")
        );
    }
}
