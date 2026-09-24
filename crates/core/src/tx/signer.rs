//! The local account used to sign transactions for submitting onchain.

use crate::{kdf, tx::types::UnsignedTransaction};
use alloy::{
    consensus::SignableTransaction as _,
    eips::{Encodable2718 as _, eip7702::Authorization},
    network::TxSignerSync as _,
    primitives::{Address, B256, TxHash, U256, keccak256},
    signers::{SignerSync as _, local::PrivateKeySigner},
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

    /// Signs a transaction. For an [`Eip7702`](UnsignedTransaction::Eip7702)
    /// transaction, this also builds and signs its sole authorization list
    /// entry, delegating to `delegate` at the transaction's own `nonce + 1`,
    /// since a self-signed authorization must apply after the sender's nonce
    /// is incremented by the transaction itself.
    pub fn sign_transaction(
        &self,
        tx: UnsignedTransaction,
    ) -> Result<SignedTransaction, SigningError> {
        match tx {
            UnsignedTransaction::Eip1559(mut tx) => {
                let signature = self
                    .0
                    .sign_transaction_sync(&mut tx)
                    .map_err(|_| SigningError)?;
                Ok(SignedTransaction(tx.into_signed(signature).encoded_2718()))
            }
            UnsignedTransaction::Eip7702 { mut tx, delegate } => {
                let authorization = Authorization {
                    chain_id: U256::from(tx.chain_id),
                    address: delegate,
                    nonce: tx.nonce + 1,
                };
                let authorization_signature = self
                    .0
                    .sign_hash_sync(&authorization.signature_hash())
                    .map_err(|_| SigningError)?;
                tx.authorization_list = vec![authorization.into_signed(authorization_signature)];

                let signature = self
                    .0
                    .sign_transaction_sync(&mut tx)
                    .map_err(|_| SigningError)?;
                Ok(SignedTransaction(tx.into_signed(signature).encoded_2718()))
            }
        }
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
    use alloy::{
        consensus::{Signed, TxEip1559, TxEip7702},
        eips::Decodable2718,
        signers::Signature,
    };

    #[test]
    fn can_sign_transactions() {
        let private_key = SigningKey::from_bytes(&keccak256("top secret key").0.into()).unwrap();
        let account = Signer::new(private_key);
        let tx = TxEip1559::default();
        let signed = account
            .sign_transaction(UnsignedTransaction::Eip1559(tx.clone()))
            .unwrap();

        let decoded = Signed::<TxEip1559, Signature>::decode_2718_exact(signed.as_raw()).unwrap();
        assert_eq!(decoded.tx(), &tx);
        assert_eq!(decoded.recover_signer().unwrap(), account.address());
    }

    #[test]
    fn signs_delegation_transactions_with_a_matching_authorization() {
        let private_key = SigningKey::from_bytes(&keccak256("top secret key").0.into()).unwrap();
        let account = Signer::new(private_key);
        let delegate = Address::repeat_byte(0x42);
        let tx = TxEip7702 {
            chain_id: 1,
            nonce: 7,
            to: account.address(),
            ..Default::default()
        };
        let signed = account
            .sign_transaction(UnsignedTransaction::Eip7702 {
                tx: tx.clone(),
                delegate,
            })
            .unwrap();

        let decoded = Signed::<TxEip7702, Signature>::decode_2718_exact(signed.as_raw()).unwrap();
        assert_eq!(decoded.recover_signer().unwrap(), account.address());

        let authorization_list = &decoded.tx().authorization_list;
        assert_eq!(authorization_list.len(), 1);
        let authorization = &authorization_list[0];
        assert_eq!(authorization.inner().address, delegate);
        assert_eq!(authorization.inner().nonce, tx.nonce + 1);
        assert_eq!(authorization.inner().chain_id, U256::from(tx.chain_id));
        assert_eq!(
            authorization.recover_authority().unwrap(),
            account.address()
        );
    }
}
