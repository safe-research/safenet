use crate::bindings::{
    consensus::{OracleTransactionProposal, SafeTransaction},
    safe::{Operation as SafeOperation, SafeTx},
};
use alloy::{
    primitives::{Address, B256, Keccak256, U256},
    sol_types::{Eip712Domain, SolStruct},
};
use safenet_core::tx::Signer;

/// Domain separator for deriving a sentinel's reveal salt from its account key. Passed as the
/// HKDF salt in [`Signer::derive_key`]; see `Signer::derive_key`'s own docs for why this alone
/// (not a hard security requirement) is enough separation from other uses of the same key.
const REVEAL_SALT_DOMAIN: &[u8] = b"safenet-sentinel-reveal-salt";

/// Computes the blind commit-hash preimage for the sentinel game's commit-reveal vote, mirroring
/// `SentinelOracleCommitment.computeHash` (`contracts/src/libraries/SentinelOracleCommitmentsV2.sol`):
/// `keccak256(abi.encodePacked(approve, salt, sentinel, requestId, reason))`. Binding `sentinel`
/// and `requestId` into the preimage (not just `approve`/`salt`) is load-bearing, not
/// defense-in-depth — without it, a commitment computed for one sentinel/request could be
/// replayed to satisfy another. `reason` must be the exact same value later supplied to
/// `reveal`, or the onchain hash check fails with `InvalidReveal()`.
#[must_use]
pub fn commit_hash(
    sentinel: Address,
    request_id: B256,
    approve: bool,
    salt: B256,
    reason: &str,
) -> B256 {
    let mut hasher = Keccak256::new();
    hasher.update([u8::from(approve)]);
    hasher.update(salt);
    hasher.update(sentinel);
    hasher.update(request_id);
    hasher.update(reason);
    hasher.finalize()
}

/// Extends [`Signer`] with the sentinel game's reveal-salt derivation, so callers can write
/// `signer.reveal_salt(request_id)` alongside its other account operations.
pub trait RevealSalt {
    /// Deterministically derives this account's reveal salt for `request_id`, so nothing needs
    /// to be persisted between `commit` and `reveal` (see the epic's Architecture Decision:
    /// "Deterministic salt derivation via HMAC-SHA256").
    fn reveal_salt(&self, request_id: B256) -> B256;
}

impl RevealSalt for Signer {
    fn reveal_salt(&self, request_id: B256) -> B256 {
        self.derive_key(REVEAL_SALT_DOMAIN, request_id.as_slice())
    }
}

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
    use alloy::{
        primitives::{Bytes, address, b256, keccak256},
        signers::k256::ecdsa::SigningKey,
    };

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

    /// Parity vector: `SentinelOracleCommitment.computeHash` from
    /// `contracts/src/libraries/SentinelOracleCommitmentsV2.sol`, obtained by exercising that
    /// library directly with `forge test`. Mirrored on the Solidity side by
    /// `test_HashCommitment_ParityWithRustImplementation` in
    /// `contracts/test/SentinelOracleV2.t.sol` — keep both in sync if either implementation or
    /// expected hash changes.
    /// Inputs: sentinel=0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045, requestId=1, approve=true,
    /// salt=keccak256("test-salt"), reason="destination is not blocklisted".
    #[test]
    fn commit_hash_parity() {
        let sentinel = address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
        let request_id = b256!("0000000000000000000000000000000000000000000000000000000000000001");
        let salt = b256!("8bcfa1e0aed22543ed44d41a95e315383294a18f9fb6e67ee082afcd585a6ff1");

        assert_eq!(
            commit_hash(
                sentinel,
                request_id,
                true,
                salt,
                "destination is not blocklisted"
            ),
            b256!("109cc7dede05c71271a7347e049111921bc1f9f5b8f43d724c24ffbf4b1bdb6c"),
        );
    }

    /// Independently computed (Python `hmac`/`hashlib` HKDF-SHA256 per RFC 5869 §2.2/§2.3,
    /// mirroring `kdf::derive_key`'s own reference-vector test), since `reveal_salt` has no
    /// onchain/TS counterpart to cross-check against.
    /// Inputs: private key = keccak256("top secret key"), requestId=1.
    #[test]
    fn reveal_salt_parity() {
        let key = SigningKey::from_bytes(&keccak256("top secret key").0.into()).unwrap();
        let signer = Signer::new(key);
        let request_id = b256!("0000000000000000000000000000000000000000000000000000000000000001");

        assert_eq!(
            signer.reveal_salt(request_id),
            b256!("40b40e386211784e67ff361d1c7be5ffaf17b1b59eb91d176cfe0f02f28d7461"),
        );
    }

    #[test]
    fn reveal_salt_is_bound_to_request_id() {
        let key = SigningKey::from_bytes(&keccak256("top secret key").0.into()).unwrap();
        let signer = Signer::new(key);

        assert_ne!(
            signer.reveal_salt(b256!(
                "0000000000000000000000000000000000000000000000000000000000000001"
            )),
            signer.reveal_salt(b256!(
                "0000000000000000000000000000000000000000000000000000000000000002"
            )),
        );
    }
}
