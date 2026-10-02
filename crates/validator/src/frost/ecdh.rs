//! ECDH-XOR encryption of FROST secret shares for the onchain publishing
//! channel.

use frost_secp256k1::Identifier;
use hkdf::Hkdf;
use k256::{
    EncodedPoint, NonZeroScalar, ProjectivePoint, Scalar,
    elliptic_curve::{
        Group,
        hash2curve::{self, ExpandMsgXmd},
        point::AffineCoordinates as _,
        sec1::{FromEncodedPoint, ToEncodedPoint},
        zeroize::{Zeroize, ZeroizeOnDrop},
    },
    sha2::Sha256,
};
use rand::{CryptoRng, RngCore};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::fmt::{self, Debug, Formatter};

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
    pub(super) fn public_key(&self) -> EncryptionPublicKey {
        EncryptionPublicKey(ProjectivePoint::GENERATOR * *self.0)
    }

    /// Encrypts or decrypts a 32-byte secret-share `msg` sent from `sender` to
    /// `recipient` against the peer's encryption public key. See [`ecdh`].
    pub(super) fn ecdh(
        &self,
        public_key: &EncryptionPublicKey,
        sender: &Identifier,
        recipient: &Identifier,
        msg: [u8; 32],
    ) -> [u8; 32] {
        ecdh(&self.0, public_key, sender, recipient, msg)
    }
}

impl Debug for EncryptionKey {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_tuple("EncryptionKey").field(&"redacted").finish()
    }
}

impl Drop for EncryptionKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl ZeroizeOnDrop for EncryptionKey {}

/// An encryption public key that can be serialized.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncryptionPublicKey(ProjectivePoint);

impl EncryptionPublicKey {
    /// Tries to construct an encryption public key from a projective point.
    pub(super) fn from_point(point: ProjectivePoint) -> Result<Self, frost_secp256k1::Error> {
        if point.is_identity().into() {
            return Err(frost_secp256k1::GroupError::InvalidIdentityElement.into());
        }
        Ok(Self(point))
    }

    /// Returns the public key as a point.
    pub(super) fn as_point(&self) -> &ProjectivePoint {
        &self.0
    }
}

impl<'de> Deserialize<'de> for EncryptionPublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = EncodedPoint::deserialize(deserializer)?;
        ProjectivePoint::from_encoded_point(&encoded)
            .into_option()
            .map(EncryptionPublicKey::from_point)
            .ok_or_else(|| de::Error::custom("invalid encryption public key encoding"))?
            .map_err(de::Error::custom)
    }
}

impl Serialize for EncryptionPublicKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.to_encoded_point(true).serialize(serializer)
    }
}

/// Encrypts or decrypts `msg` via ECDH:
/// `msg XOR HECDH((peer_pubkey * privkey).x, sender, recipient)`.
///
/// XOR is its own inverse, so this serves both directions. `peer_pubkey` must
/// be a valid non-identity point and `privkey` must be non-zero.
fn ecdh(
    privkey: &NonZeroScalar,
    peer_pubkey: &EncryptionPublicKey,
    sender: &Identifier,
    recipient: &Identifier,
    msg: [u8; 32],
) -> [u8; 32] {
    // The pad is bound to the sender and recipient, and not just the shared
    // secret. Encryption public keys are not proven to be owned by the
    // participant that published them, so a participant can copy another's
    // key (or publish a related key) to share an ECDH secret with them. With
    // a pad derived from the shared secret alone, the plaintext revealed by a
    // complaint response would then also decrypt the share for the honest
    // owner of the key.
    let shared_secret = (peer_pubkey.0 * **privkey).to_affine().x();
    let pad = kdf(b"ecdh", &shared_secret, sender, recipient);
    let mut result = msg;
    for (byte, secret) in result.iter_mut().zip(pad) {
        *byte ^= secret;
    }
    result
}

/// Key derivation function for a a 32-byte encryption pad from an ECDH shared
/// secret, bound to the `sender` and `recipient` of the encrypted message.
fn kdf(
    discriminant: &[u8],
    shared_secret: &[u8],
    sender: &Identifier,
    recipient: &Identifier,
) -> [u8; 32] {
    // The info is domain separated with the ciphersuite context string and an
    // `"ecdh"` tag, following the ciphersuite's hash functions. Identifiers are
    // fixed-length, so their concatenation is unambiguous.
    let mut pad = [0; 32];
    Hkdf::<Sha256>::new(None, shared_secret)
        .expand_multi_info(
            &[
                &b"FROST-secp256k1-SHA256-v1"[..],
                discriminant,
                &sender.serialize(),
                &recipient.serialize(),
            ],
            &mut pad,
        )
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    pad
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

    fn id(id: u16) -> Identifier {
        Identifier::try_from(id).unwrap()
    }

    #[test]
    fn roundtrip_encrypt_decrypt() {
        let mut rng = rand::thread_rng();
        let key = EncryptionKey::generate(&mut rng);
        let peer = EncryptionKey::generate(&mut rng);
        let msg = [0x5a; 32];

        let enc = key.ecdh(&peer.public_key(), &id(1), &id(2), msg);
        let dec = peer.ecdh(&key.public_key(), &id(1), &id(2), enc);
        assert_eq!(dec, msg);
    }

    #[test]
    fn ecdh_is_commutative() {
        let alice = key(2);
        let bob = key(3);
        let msg = [0x00; 32];
        assert_eq!(
            alice.ecdh(&bob.public_key(), &id(1), &id(2), msg),
            bob.ecdh(&alice.public_key(), &id(1), &id(2), msg),
        );
    }

    #[test]
    fn different_recipient_different_ciphertext() {
        let alice = key(2);
        let bob = key(3);
        let charlie = key(4);
        let msg = [0xff; 32];
        assert_ne!(
            alice.ecdh(&bob.public_key(), &id(1), &id(2), msg),
            alice.ecdh(&charlie.public_key(), &id(1), &id(3), msg),
        );
    }

    #[test]
    fn copied_public_key_different_ciphertext() {
        // A participant that copies another participant's encryption public
        // key must not share an encryption pad with them.
        let alice = key(2);
        let bob = key(3);
        let msg = [0xff; 32];
        assert_ne!(
            alice.ecdh(&bob.public_key(), &id(1), &id(2), msg),
            alice.ecdh(&bob.public_key(), &id(1), &id(3), msg),
        );
    }

    #[test]
    fn ecdh_rejects_degenerate_public_keys_at_infinity() {
        assert!(EncryptionPublicKey::from_point(ProjectivePoint::IDENTITY).is_err());
    }
}
