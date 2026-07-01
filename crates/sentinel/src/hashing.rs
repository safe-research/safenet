use crate::bindings::{
    consensus::{OracleTransactionProposal, SafeTransaction},
    safe::{Operation as SafeOperation, SafeTx},
};
use alloy::{
    primitives::{Address, B256, U256},
    sol_types::{Eip712Domain, SolStruct},
};

/// Computes the EIP-712 signing hash for a Safe transaction.
#[must_use]
#[cfg_attr(not(test), expect(dead_code))]
pub fn safe_tx_hash(tx: &SafeTransaction) -> B256 {
    let domain = Eip712Domain {
        chain_id: Some(tx.chainId),
        verifying_contract: Some(tx.safe),
        ..Default::default()
    };
    SafeTx {
        to: tx.to,
        value: tx.value,
        data: tx.data.clone(),
        operation: SafeOperation::try_from(Into::<u8>::into(tx.operation))
            .unwrap_or(SafeOperation::CALL),
        safeTxGas: tx.safeTxGas,
        baseGas: tx.baseGas,
        gasPrice: tx.gasPrice,
        gasToken: tx.gasToken,
        refundReceiver: tx.refundReceiver,
        nonce: tx.nonce,
    }
    .eip712_signing_hash(&domain)
}

/// Computes the EIP-712 requestId (the proposal hash) for an oracle transaction proposal.
#[must_use]
pub fn oracle_tx_proposal_hash(
    chain_id: U256,
    consensus: Address,
    epoch: u64,
    oracle: Address,
    stx_hash: B256,
) -> B256 {
    let domain = Eip712Domain {
        chain_id: Some(chain_id),
        verifying_contract: Some(consensus),
        ..Default::default()
    };
    OracleTransactionProposal {
        epoch,
        oracle,
        safeTxHash: stx_hash,
    }
    .eip712_signing_hash(&domain)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::consensus::Operation;
    use alloy::primitives::{Bytes, address, b256};

    fn zero_tx(chain_id: u64, safe: Address, to: Address) -> SafeTransaction {
        SafeTransaction {
            chainId: U256::from(chain_id),
            safe,
            to,
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

    /// Parity vector: TX_HASH from
    /// `validator/src/consensus/verify/safeTx/hashing.test.ts`.
    /// Inputs: chainId=1, safe=to=0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045, all other
    /// fields zero.
    #[test]
    fn safe_tx_hash_parity() {
        let addr = address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
        assert_eq!(
            safe_tx_hash(&zero_tx(1, addr, addr)),
            b256!("fe8b85e8d090b16fe8f142d3c9292dc1fc77daf9eb4af8f7cf4a7707d95f4028"),
        );
    }

    /// Parity vector: `oracleTxPacketHash(VALID_PACKET)` from
    /// `validator/src/consensus/verify/oracleTx/handler.test.ts`.
    /// VALID_PACKET: domain.chain=23, consensus=0x22Cb221c..., epoch=11,
    /// oracle=safe=0x4838B106..., to=0x22Cb221c..., chainId=1, all other fields zero.
    #[test]
    fn oracle_tx_proposal_hash_parity() {
        let consensus = address!("22Cb221caE98D6097082C80158B1472C45FEd729");
        let oracle = address!("4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97");

        let stx_hash = safe_tx_hash(&zero_tx(1, oracle, consensus));
        let id = oracle_tx_proposal_hash(U256::from(23u64), consensus, 11, oracle, stx_hash);
        assert_eq!(
            id,
            b256!("2a29f463bfd18f87230f795b09f22d8d126f0fcedab6149ce430777c827115b0"),
        );
    }
}
