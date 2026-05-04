//! ECDH share encryption and decryption for the public key channel used in Safenet.
//!
//! Port of `validator/src/frost/secret.ts`.

use anyhow::{Result, ensure};
use k256::{AffinePoint, ProjectivePoint, Scalar, elliptic_curve::point::AffineCoordinates as _};

/// Encrypts or decrypts `msg` using ECDH between `sender_privkey` and `receiver_pubkey`.
///
/// The operation is `msg XOR (receiver_pubkey * sender_privkey).x`. Because XOR is its own
/// inverse this single function serves both encryption and decryption.
///
/// The `receiver_pubkey` must be a valid, non-identity point on secp256k1.
/// The `sender_privkey` must be non-zero.
///
/// `msg` is interpreted as a 256-bit scalar (the domain of secret shares).
pub fn ecdh(
    msg: [u8; 32],
    sender_privkey: &Scalar,
    receiver_pubkey: &AffinePoint,
) -> Result<[u8; 32]> {
    ensure!(
        *sender_privkey != Scalar::ZERO,
        "private key must not be zero"
    );
    ensure!(
        *receiver_pubkey != AffinePoint::IDENTITY,
        "public key must not be the identity"
    );

    let shared_secret = ProjectivePoint::from(*receiver_pubkey) * sender_privkey;
    let shared_secret = shared_secret.to_affine().x();

    let mut result = msg;
    for (byte, secret) in result.iter_mut().zip(shared_secret) {
        *byte ^= secret;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(scalar: u64) -> AffinePoint {
        (ProjectivePoint::GENERATOR * Scalar::from(scalar)).to_affine()
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

        let encrypted = ecdh(msg, &alice_priv, &bob_pub).unwrap();
        let decrypted = ecdh(encrypted, &bob_priv, &alice_pub).unwrap();
        assert_eq!(decrypted, msg);
    }

    #[test]
    fn commutativity() {
        let alice_priv = Scalar::from(2u64);
        let bob_priv = Scalar::from(3u64);
        let alice_pub = g(2);
        let bob_pub = g(3);
        let msg = [0x42; 32];

        let encrypted_a = ecdh(msg, &alice_priv, &bob_pub).unwrap();
        let encrypted_b = ecdh(msg, &bob_priv, &alice_pub).unwrap();
        assert_eq!(encrypted_a, encrypted_b);
    }

    #[test]
    fn different_recipient_different_ciphertext() {
        let alice_priv = Scalar::from(2u64);
        let bob_pub = g(3);
        let carol_pub = g(5);
        let msg = [0x42; 32];

        assert_ne!(
            ecdh(msg, &alice_priv, &bob_pub).unwrap(),
            ecdh(msg, &alice_priv, &carol_pub).unwrap(),
        );
    }
}
