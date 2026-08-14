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
    primitives::{Address, B256, Bytes, U256, b256, keccak256},
    sol,
    sol_types::{Eip712Domain, SolStruct},
};
use std::num::NonZeroU64;

/// `keccak256("TransactionProposal(uint64 epoch,address oracle,bytes oracleData,bytes32 safeTxHash)")`.
///
/// Kept in sync with `ConsensusMessages.TRANSACTION_PROPOSAL_TYPEHASH` onchain; used only by
/// [`ConsensusDomain::transaction_proposal_hash_from_data_hash`], which rebuilds the digest from a
/// pre-hashed `oracleData`.
const TRANSACTION_PROPOSAL_TYPE_HASH: B256 =
    b256!("9c6706f5afdb1de99f5ad39011e7770ce471f51d78380634f6cedb21a648b8d0");

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

    /// The consensus-domain packet proposing an oracle-backed Safe
    /// transaction for attestation.
    struct TransactionProposal {
        uint64 epoch;
        address oracle;
        bytes oracleData;
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

/// The EIP-712 `SafeTx` struct hash, without the target Safe's domain.
///
/// This is the partial hash accepted by the consensus attestation callbacks,
/// which reconstruct the complete Safe transaction hash from `chainId`,
/// `safe`, and this value.
pub fn safe_tx_struct_hash(tx: &SafeTransaction) -> B256 {
    SafeTx::from(tx).eip712_hash_struct()
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

    /// The consensus-domain hash of an oracle-backed Safe transaction proposal,
    /// embedding the already-computed [`safe_tx_hash`] as its `safeTxHash` field.
    ///
    /// This is the proposal/signing path, which always holds the full `oracleData`, so it uses the
    /// canonical `SolStruct` EIP-712 hashing (alloy hashes the `bytes` member itself).
    pub fn transaction_proposal_hash(
        &self,
        epoch: EpochId,
        oracle: Address,
        oracle_data: Bytes,
        safe_tx_hash: B256,
    ) -> B256 {
        TransactionProposal {
            epoch: epoch.raw_value(),
            oracle,
            oracleData: oracle_data,
            safeTxHash: safe_tx_hash,
        }
        .eip712_signing_hash(&self.0)
    }

    /// The same proposal digest, rebuilt from a pre-hashed `oracleData`.
    ///
    /// Used only on the attest path: the onchain `TransactionAttested` event carries just
    /// `keccak256(oracleData)` (so the validator callback stays a constant size) and the full bytes
    /// are not available there. EIP-712 encodes the `bytes oracleData` member as its `keccak256`, so
    /// this produces a digest identical to [`transaction_proposal_hash`] for the same data (asserted
    /// in the tests). It is spelled out by hand because alloy's `SolStruct` signing hash requires a
    /// value holding the full bytes, which this path does not have.
    pub fn transaction_proposal_hash_from_data_hash(
        &self,
        epoch: EpochId,
        oracle: Address,
        oracle_data_hash: B256,
        safe_tx_hash: B256,
    ) -> B256 {
        let mut data = [0u8; 160];
        data[0..32].copy_from_slice(TRANSACTION_PROPOSAL_TYPE_HASH.as_slice());
        data[32..64].copy_from_slice(&U256::from(epoch.raw_value()).to_be_bytes::<32>());
        data[64..96].copy_from_slice(oracle.into_word().as_slice());
        data[96..128].copy_from_slice(oracle_data_hash.as_slice());
        data[128..160].copy_from_slice(safe_tx_hash.as_slice());
        let struct_hash = keccak256(data);

        let mut message = [0u8; 66];
        message[0..2].copy_from_slice(&[0x19, 0x01]);
        message[2..34].copy_from_slice(self.0.separator().as_slice());
        message[34..66].copy_from_slice(struct_hash.as_slice());
        keccak256(message)
    }

    /// The consensus-domain hash of an oracle-backed Safe transaction packet:
    /// shorthand for [`transaction_proposal_hash`] with [`safe_tx_hash`]
    /// computed from `tx`.
    pub fn transaction_packet_hash(
        &self,
        epoch: EpochId,
        oracle: Address,
        oracle_data: Bytes,
        tx: &SafeTransaction,
    ) -> B256 {
        self.transaction_proposal_hash(epoch, oracle, oracle_data, safe_tx_hash(tx))
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
    fn sample_transaction_packet_hash() {
        assert_eq!(
            TEST_DOMAIN.transaction_packet_hash(EPOCH_ONE, TEST_ADDRESS, Bytes::new(), &safe_tx()),
            b256!("5ac4a916cb21b51c5b61c6d4479f7c7477856abecb96e439fb53d5462fb5b3ed")
        );
    }

    /// The hand-rolled attest-path reconstruction must match the canonical `SolStruct`-based proposal
    /// hash for the same data (EIP-712 encodes the `bytes oracleData` member as its keccak256).
    #[test]
    fn proposal_hash_from_data_hash_matches_full_data() {
        for oracle_data in [Bytes::new(), Bytes::from_static(&[0xca, 0xfe])] {
            let safe_tx = safe_tx();
            let full = TEST_DOMAIN.transaction_packet_hash(
                EPOCH_ONE,
                TEST_ADDRESS,
                oracle_data.clone(),
                &safe_tx,
            );
            let from_hash = TEST_DOMAIN.transaction_proposal_hash_from_data_hash(
                EPOCH_ONE,
                TEST_ADDRESS,
                keccak256(&oracle_data),
                safe_tx_hash(&safe_tx),
            );
            assert_eq!(full, from_hash);
        }
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
