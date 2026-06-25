//! The local account used to sign transactions for submitting onchain.

use alloy::{
    consensus::{SignableTransaction as _, TxEip1559},
    eips::Encodable2718 as _,
    network::TxSignerSync as _,
    primitives::{Address, B256, TxHash, keccak256},
    signers::{
        k256::{ecdsa::SigningKey, elliptic_curve::zeroize::Zeroize},
        local::PrivateKeySigner,
    },
};
use serde::{Deserialize, Deserializer, de};

/// An error ECDSA signing a transaction.
#[derive(Debug, thiserror::Error)]
#[error("an error occurred signing an Ethereum transaction")]
pub struct SigningError;

/// A local account that signs and submits transactions onchain on behalf of a
/// service.
pub struct Signer {
    signer: PrivateKeySigner,
}

/// A raw signed transaction.
pub struct SignedTransaction(Vec<u8>);

impl Signer {
    /// Creates an account for the given `private_key`.
    pub fn new(private_key: SigningKey) -> Self {
        let signer = PrivateKeySigner::from_signing_key(private_key);
        Self { signer }
    }

    /// The address of the local account.
    pub fn address(&self) -> Address {
        self.signer.address()
    }

    /// Signs a transaction.
    pub fn sign_transaction(&self, mut tx: TxEip1559) -> Result<SignedTransaction, SigningError> {
        let signature = self
            .signer
            .sign_transaction_sync(&mut tx)
            .map_err(|_| SigningError)?;
        let raw_tx = tx.into_signed(signature).encoded_2718();
        Ok(SignedTransaction(raw_tx))
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::{consensus::Signed, eips::Decodable2718, signers::Signature};

    #[test]
    fn can_sign_transactions() {
        let private_key = SigningKey::from_bytes(&keccak256("top secret key").0.into()).unwrap();
        let account = Signer::new(private_key);
        let tx = TxEip1559::default();
        let signed = account.sign_transaction(tx.clone()).unwrap();

        let decoded = Signed::<TxEip1559, Signature>::decode_2718_exact(signed.as_raw()).unwrap();
        assert_eq!(decoded.tx(), &tx);
        assert_eq!(decoded.recover_signer().unwrap(), account.address());
    }
}
