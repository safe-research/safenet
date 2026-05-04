//! ECDH share encryption and decryption for the public key channel used in Safenet.
//!
//! Port of `validator/src/frost/secret.ts`.

use anyhow::{Result, ensure};
use k256::{
    ProjectivePoint, Scalar,
    elliptic_curve::{
        hash2curve::{self, ExpandMsgXmd},
        point::AffineCoordinates as _,
    },
    sha2::Sha256,
};
use rand::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};

/// Encryption key used for ECDH-encrypted secret shares.
#[derive(Clone, Copy, Deserialize, Serialize)]
pub struct EncryptionKey(Scalar);

impl EncryptionKey {
    pub fn generate<R>(mut rng: R) -> Self
    where
        R: CryptoRng + RngCore,
    {
        let mut entropy = [0; 32];
        rng.fill_bytes(&mut entropy);
        let secret_key = hash_to_scalar(b"enc", &entropy);
        Self(secret_key)
    }

    pub fn public_key(&self) -> ProjectivePoint {
        ProjectivePoint::GENERATOR * self.0
    }

    pub fn encrypt(&self, public_key: &ProjectivePoint, msg: [u8; 32]) -> Result<[u8; 32]> {
        ecdh(&self.0, public_key, msg)
    }
}

/// Encrypts or decrypts `msg` using ECDH between `sender_privkey` and `receiver_pubkey`.
///
/// The operation is `msg XOR (receiver_pubkey * sender_privkey).x`. Because XOR is its own
/// inverse this single function serves both encryption and decryption.
///
/// The `receiver_pubkey` must be a valid, non-identity point on secp256k1.
/// The `sender_privkey` must be non-zero.
///
/// `msg` is interpreted as a 256-bit scalar (the domain of secret shares).
fn ecdh(
    sender_privkey: &Scalar,
    receiver_pubkey: &ProjectivePoint,
    msg: [u8; 32],
) -> Result<[u8; 32]> {
    ensure!(
        *sender_privkey != Scalar::ZERO,
        "private key must not be zero"
    );
    ensure!(
        *receiver_pubkey != ProjectivePoint::IDENTITY,
        "public key must not be the identity"
    );

    let shared_secret = receiver_pubkey * sender_privkey;
    let shared_secret = shared_secret.to_affine().x();

    let mut result = msg;
    for (byte, secret) in result.iter_mut().zip(shared_secret) {
        *byte ^= secret;
    }
    Ok(result)
}

fn hash_to_scalar(domain: &[u8], msg: &[u8]) -> Scalar {
    let mut u = [Scalar::ZERO];
    hash2curve::hash_to_field::<ExpandMsgXmd<Sha256>, Scalar>(
        &[msg],
        &[b"FROST-secp256k1-SHA256-v1", domain],
        &mut u,
    )
    .expect("unexpected hash to scalar failure");
    u[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(scalar: u64) -> ProjectivePoint {
        ProjectivePoint::GENERATOR * Scalar::from(scalar)
    }

    #[test]
    fn roundtrip_encrypt_decrypt() {
        let alice_priv = Scalar::from(2u64);
        let bob_priv = Scalar::from(3u64);
        let alice_pub = g(2);
        let bob_pub = g(3);
        let msg = [
            0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];

        let encrypted = ecdh(&alice_priv, &bob_pub, msg).unwrap();
        let decrypted = ecdh(&bob_priv, &alice_pub, encrypted).unwrap();
        assert_eq!(decrypted, msg);
    }

    #[test]
    fn commutativity() {
        let alice_priv = Scalar::from(2u64);
        let bob_priv = Scalar::from(3u64);
        let alice_pub = g(2);
        let bob_pub = g(3);
        let msg = [0x42; 32];

        let encrypted_a = ecdh(&alice_priv, &bob_pub, msg).unwrap();
        let encrypted_b = ecdh(&bob_priv, &alice_pub, msg).unwrap();
        assert_eq!(encrypted_a, encrypted_b);
    }

    #[test]
    fn different_recipient_different_ciphertext() {
        let alice_priv = Scalar::from(2u64);
        let bob_pub = g(3);
        let carol_pub = g(5);
        let msg = [0x42; 32];

        assert_ne!(
            ecdh(&alice_priv, &bob_pub, msg).unwrap(),
            ecdh(&alice_priv, &carol_pub, msg).unwrap(),
        );
    }
}
