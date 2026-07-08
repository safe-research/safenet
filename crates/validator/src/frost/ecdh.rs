//! ECDH-XOR encryption of FROST secret shares for the onchain publishing
//! channel.

use crate::bindings;

use super::{error::Error, marshal};
use alloy::primitives::B256;
use k256::{
    NonZeroScalar, ProjectivePoint, Scalar,
    elliptic_curve::{
        hash2curve::{self, ExpandMsgXmd},
        point::AffineCoordinates as _,
        zeroize::{Zeroize, ZeroizeOnDrop},
    },
    sha2::Sha256,
};
use rand::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};

/// A locally-generated ECDH encryption key. The secret scalar is sampled by the
/// effect handler and never leaves the secret store; only [`public_key`] is
/// published onchain.
///
/// [`public_key`]: EncryptionKey::public_key
#[derive(Clone, Deserialize, Serialize)]
pub struct EncryptionKey(NonZeroScalar);

impl EncryptionKey {
    /// Samples a fresh encryption key from `rng`.
    pub fn generate<R>(mut rng: R) -> Self
    where
        R: CryptoRng + RngCore,
    {
        let mut entropy = [0; 32];
        rng.fill_bytes(&mut entropy);
        Self(hash_to_scalar(b"enc", &entropy))
    }

    /// The public key `q` published onchain for peers to encrypt shares to.
    pub fn public_key(&self) -> bindings::Point {
        let public_key = ProjectivePoint::GENERATOR * *self.0;
        marshal::solidity_point(&public_key)
    }

    /// Encrypts or decrypts a 32-byte secret-share `msg` against a peer's
    /// encryption public key. See [`ecdh`].
    pub fn ecdh(&self, public_key: &bindings::Point, msg: B256) -> Result<B256, Error> {
        let public_key = marshal::frost_point(public_key)?;
        let encrypted = ecdh(&self.0, &public_key, msg.0)?;
        Ok(encrypted.into())
    }
}

impl Drop for EncryptionKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl ZeroizeOnDrop for EncryptionKey {}

/// Encrypts or decrypts `msg` via ECDH: `msg XOR (receiver_pubkey * sender_privkey).x`.
///
/// XOR is its own inverse, so this serves both directions. `receiver_pubkey`
/// must be a valid non-identity point and `sender_privkey` must be non-zero.
fn ecdh(
    sender_privkey: &NonZeroScalar,
    receiver_pubkey: &ProjectivePoint,
    msg: [u8; 32],
) -> Result<[u8; 32], Error> {
    if *receiver_pubkey == ProjectivePoint::IDENTITY {
        return Err(Error::malformed_element());
    }

    let shared_secret = (*receiver_pubkey * **sender_privkey).to_affine().x();
    let mut result = msg;
    for (byte, secret) in result.iter_mut().zip(shared_secret) {
        *byte ^= secret;
    }
    Ok(result)
}

fn hash_to_scalar(discriminant: &[u8], msg: &[u8]) -> NonZeroScalar {
    let mut u = [Scalar::ZERO];
    hash2curve::hash_to_field::<ExpandMsgXmd<Sha256>, Scalar>(
        &[msg],
        &[b"FROST-secp256k1-SHA256-v1", discriminant],
        &mut u,
    )
    .expect("hash to secp256k1 scalar never fails for a single output");
    NonZeroScalar::new(u[0]).expect("hashing to zero is cryptographically impossible")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(pk: u64) -> EncryptionKey {
        EncryptionKey(NonZeroScalar::new(Scalar::from(pk)).unwrap())
    }

    fn m(msg: [u8; 32]) -> B256 {
        msg.into()
    }

    #[test]
    fn roundtrip_encrypt_decrypt() {
        let mut rng = rand::thread_rng();
        let key = EncryptionKey::generate(&mut rng);
        let peer = EncryptionKey::generate(&mut rng);
        let msg = m([0x5a; 32]);

        let enc = key.ecdh(&peer.public_key(), msg).unwrap();
        let dec = peer.ecdh(&key.public_key(), enc).unwrap();
        assert_eq!(dec, msg);
    }

    #[test]
    fn ecdh_is_commutative() {
        let alice = key(2);
        let bob = key(3);
        let msg = m([0x42; 32]);
        assert_eq!(
            alice.ecdh(&bob.public_key(), msg).unwrap(),
            bob.ecdh(&alice.public_key(), msg).unwrap(),
        );
    }

    #[test]
    fn different_recipient_different_ciphertext() {
        let alice = key(2);
        let bob = key(3);
        let charlie = key(4);
        let msg = m([0x42; 32]);
        assert_ne!(
            alice.ecdh(&bob.public_key(), msg).unwrap(),
            alice.ecdh(&charlie.public_key(), msg).unwrap(),
        );
    }

    #[test]
    fn ecdh_rejects_degenerate_inputs() {
        let alice = key(2);
        assert!(alice.ecdh(&bindings::Point::default(), B256::ZERO).is_err());
    }
}
