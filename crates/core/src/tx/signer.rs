//! The local account used to sign transactions for submitting onchain.

use crate::{kdf, tx::types::UnsignedTransaction};
use alloy::{
    consensus::{SignableTransaction, TxEip1559, TxEip7702},
    eips::{Encodable2718 as _, eip7702},
    network::TxSignerSync as _,
    primitives::{Address, B256, TxHash, TxKind, U256, keccak256},
    rpc::types::AccessList,
    signers::{Signature, SignerSync as _, local::PrivateKeySigner},
};
use k256::{ecdsa::SigningKey, elliptic_curve::zeroize::Zeroize};
use serde::{Deserialize, Deserializer, de};
use std::fmt::{self, Debug, Formatter};

/// An error ECDSA signing a transaction.
#[derive(Debug, thiserror::Error)]
#[error("an error occurred signing an Ethereum transaction")]
pub struct SigningError;

/// A local account that signs and submits transactions onchain on behalf of a
/// service.
#[derive(Clone)]
pub struct Signer(PrivateKeySigner);

/// A raw signed transaction.
pub struct SignedTransaction(Vec<u8>);

impl Signer {
    /// Creates an account for the given `private_key`.
    pub fn new(private_key: SigningKey) -> Self {
        let signer = PrivateKeySigner::from_signing_key(private_key);
        Self(signer)
    }

    /// The address of the local account.
    pub fn address(&self) -> Address {
        self.0.address()
    }

    /// Signs a transaction, as an EIP-7702 transaction if it carries an
    /// authorization and an EIP-1559 transaction otherwise.
    ///
    /// The account signs the authorization itself, for the nonce after the
    /// transaction's, since the transaction increments the nonce before the
    /// authorization is applied.
    pub fn sign_transaction(
        &self,
        tx: UnsignedTransaction,
    ) -> Result<SignedTransaction, SigningError> {
        let raw_tx = match tx.authorization {
            None => {
                let mut tx = TxEip1559 {
                    chain_id: tx.chain_id,
                    nonce: tx.nonce,
                    gas_limit: tx.gas_limit,
                    max_fee_per_gas: tx.max_fee_per_gas,
                    max_priority_fee_per_gas: tx.max_priority_fee_per_gas,
                    to: TxKind::Call(tx.to),
                    value: tx.value,
                    access_list: AccessList::default(),
                    input: tx.input,
                };
                let signature = self.signature(&mut tx)?;
                tx.into_signed(signature).encoded_2718()
            }
            Some(authorization) => {
                let authorization = eip7702::Authorization {
                    chain_id: U256::from(tx.chain_id),
                    address: authorization.address,
                    nonce: tx.nonce.checked_add(1).ok_or(SigningError)?,
                };
                let signature = self
                    .0
                    .sign_hash_sync(&authorization.signature_hash())
                    .map_err(|_| SigningError)?;
                let mut tx = TxEip7702 {
                    chain_id: tx.chain_id,
                    nonce: tx.nonce,
                    gas_limit: tx.gas_limit,
                    max_fee_per_gas: tx.max_fee_per_gas,
                    max_priority_fee_per_gas: tx.max_priority_fee_per_gas,
                    to: tx.to,
                    value: tx.value,
                    access_list: AccessList::default(),
                    authorization_list: vec![authorization.into_signed(signature)],
                    input: tx.input,
                };
                let signature = self.signature(&mut tx)?;
                tx.into_signed(signature).encoded_2718()
            }
        };
        Ok(SignedTransaction(raw_tx))
    }

    /// Signs `tx`, returning its signature.
    fn signature(
        &self,
        tx: &mut dyn SignableTransaction<Signature>,
    ) -> Result<Signature, SigningError> {
        self.0.sign_transaction_sync(tx).map_err(|_| SigningError)
    }

    /// Deterministically derives a 32-byte value from this account's private key using
    /// HKDF-SHA256, bound to the caller-supplied `domain` and `message`.
    ///
    /// `domain` must be a non-empty, caller-chosen constant identifying the specific use case
    /// (e.g. `"safenet-sentinel-reveal-salt"`), so that derivations for unrelated purposes over
    /// the same private key can never collide. See [`kdf::derive_key`] for details.
    ///
    /// Since the output is keyed by the account's own private key, it is
    /// reproducible without persisting anything beyond `domain` and `message`.
    pub fn derive_key(&self, domain: &[u8], message: &[u8]) -> B256 {
        let mut key = self.0.to_bytes();
        let derived = kdf::derive_key(key.as_slice(), domain, &[message]);
        key.0.zeroize();
        derived
    }
}

impl SignedTransaction {
    /// Compute the hash of the signed transaction.
    pub fn hash(&self) -> TxHash {
        keccak256(self.0.as_slice())
    }

    /// Turn a signed transaction into its raw underlying bytes.
    pub fn into_raw(self) -> Vec<u8> {
        self.0
    }

    /// Views the signed transaction as its raw underlying bytes.
    pub fn as_raw(&self) -> &[u8] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for Signer {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut raw = B256::deserialize(deserializer)?;
        let result = SigningKey::from_slice(raw.as_slice());
        raw.0.zeroize();
        result.map(Signer::new).map_err(de::Error::custom)
    }
}

impl Debug for Signer {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_tuple("Signer").field(&self.address()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tx::types::Authorization;
    use alloy::{consensus::Signed, eips::Decodable2718, primitives::address};

    fn signer() -> Signer {
        Signer::new(SigningKey::from_bytes(&keccak256("top secret key").0.into()).unwrap())
    }

    fn unsigned(authorization: Option<Authorization>) -> UnsignedTransaction {
        UnsignedTransaction {
            chain_id: 100,
            nonce: 5,
            gas_limit: 100_000,
            max_fee_per_gas: 2_000,
            max_priority_fee_per_gas: 1_000,
            to: address!("0x5FF137D4b0FDCD49DcA30c7cF57E578a026d2789"),
            value: U256::from(42),
            input: "0x5afe".parse().unwrap(),
            authorization,
        }
    }

    #[test]
    fn signs_eip1559_transactions() {
        let account = signer();
        let tx = unsigned(None);
        let signed = account.sign_transaction(tx.clone()).unwrap();

        let decoded = Signed::<TxEip1559, Signature>::decode_2718_exact(signed.as_raw()).unwrap();
        assert_eq!(decoded.recover_signer().unwrap(), account.address());
        assert_eq!(decoded.tx().chain_id, tx.chain_id);
        assert_eq!(decoded.tx().nonce, tx.nonce);
        assert_eq!(decoded.tx().gas_limit, tx.gas_limit);
        assert_eq!(decoded.tx().max_fee_per_gas, tx.max_fee_per_gas);
        assert_eq!(
            decoded.tx().max_priority_fee_per_gas,
            tx.max_priority_fee_per_gas
        );
        assert_eq!(decoded.tx().to, TxKind::Call(tx.to));
        assert_eq!(decoded.tx().value, tx.value);
        assert_eq!(decoded.tx().input, tx.input);
    }

    #[test]
    fn signs_eip7702_transactions_with_a_self_signed_authorization() {
        let account = signer();
        let delegate = address!("0x7702770277027702770277027702770277027702");
        let tx = unsigned(Some(Authorization { address: delegate }));
        let signed = account.sign_transaction(tx.clone()).unwrap();

        let decoded = Signed::<TxEip7702, Signature>::decode_2718_exact(signed.as_raw()).unwrap();
        assert_eq!(decoded.recover_signer().unwrap(), account.address());
        assert_eq!(decoded.tx().chain_id, tx.chain_id);
        assert_eq!(decoded.tx().nonce, tx.nonce);
        assert_eq!(decoded.tx().gas_limit, tx.gas_limit);
        assert_eq!(decoded.tx().max_fee_per_gas, tx.max_fee_per_gas);
        assert_eq!(
            decoded.tx().max_priority_fee_per_gas,
            tx.max_priority_fee_per_gas
        );
        assert_eq!(decoded.tx().to, tx.to);
        assert_eq!(decoded.tx().value, tx.value);
        assert_eq!(decoded.tx().input, tx.input);

        let [authorization] = decoded.tx().authorization_list.as_slice() else {
            panic!("expected a single authorization");
        };
        assert_eq!(
            authorization.recover_authority().unwrap(),
            account.address()
        );
        assert_eq!(authorization.chain_id, U256::from(tx.chain_id));
        assert_eq!(authorization.address, delegate);
        assert_eq!(authorization.nonce, tx.nonce + 1);
    }

    #[test]
    fn cannot_authorize_past_the_last_nonce() {
        let tx = UnsignedTransaction {
            nonce: u64::MAX,
            ..unsigned(Some(Authorization {
                address: Address::ZERO,
            }))
        };
        assert!(signer().sign_transaction(tx).is_err());
    }
}
