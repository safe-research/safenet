//! ECDH-XOR encryption of FROST secret shares for the onchain publishing
//! channel.

use frost_secp256k1::{Identifier, Signature, keys};
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
/// effect handler and never leaves the secret store; only its [`package`] is
/// published onchain.
///
/// [`package`]: EncryptionKey::package
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

    /// Builds the package published onchain for peers to encrypt shares to,
    /// with a proof of possession of the encryption key for `identifier`.
    pub(super) fn package<R>(
        &self,
        identifier: Identifier,
        rng: R,
    ) -> Result<Package, frost_secp256k1::Error>
    where
        R: CryptoRng + RngCore,
    {
        let public_key = self.public_key();
        let proof_of_possession = frost_core::keys::dkg::compute_proof_of_knowledge(
            identifier,
            &[*self.0],
            &public_key.as_commitment(),
            rng,
        )?;
        Ok(Package::new(public_key.0, proof_of_possession))
    }

    /// The public key `q` for peers to encrypt shares to.
    fn public_key(&self) -> EncryptionPublicKey {
        EncryptionPublicKey(ProjectivePoint::GENERATOR * *self.0)
    }

    /// Encrypts or decrypts a 32-byte secret-share `msg` against a peer's
    /// encryption public key. See [`ecdh`].
    pub(super) fn ecdh(&self, public_key: &EncryptionPublicKey, msg: [u8; 32]) -> [u8; 32] {
        ecdh(&self.0, public_key, msg)
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

/// An encryption public key along with its proof of possession, as published
/// onchain by a participant.
#[derive(Clone, Debug)]
pub(super) struct Package {
    public_key_point: ProjectivePoint,
    proof_of_possession: Signature,
}

impl Package {
    /// Creates a new package from an unverified public key and its proof of
    /// possession.
    pub(super) fn new(public_key_point: ProjectivePoint, proof_of_possession: Signature) -> Self {
        Self {
            public_key_point,
            proof_of_possession,
        }
    }

    /// Returns the unverified public key point.
    pub(super) fn public_key_point(&self) -> &ProjectivePoint {
        &self.public_key_point
    }

    /// Returns the proof of possession of the public key.
    pub(super) fn proof_of_possession(&self) -> &Signature {
        &self.proof_of_possession
    }

    /// Verifies the proof of possession of the public key for the participant
    /// with `identifier`, returning the verified encryption public key.
    pub(super) fn verified_public_key(
        self,
        identifier: Identifier,
    ) -> Result<EncryptionPublicKey, frost_secp256k1::Error> {
        let public_key = EncryptionPublicKey::from_point(self.public_key_point)?;
        frost_core::keys::dkg::verify_proof_of_knowledge(
            identifier,
            &public_key.as_commitment(),
            &self.proof_of_possession,
        )?;
        Ok(public_key)
    }
}

impl<'de> Deserialize<'de> for Package {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Package {
            public_key_point: EncodedPoint,
            proof_of_possession: Signature,
        }

        let package = Package::deserialize(deserializer)?;
        let public_key_point = ProjectivePoint::from_encoded_point(&package.public_key_point)
            .into_option()
            .ok_or_else(|| de::Error::custom("invalid encryption public key encoding"))?;
        Ok(Self::new(public_key_point, package.proof_of_possession))
    }
}

impl Serialize for Package {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct Package<'a> {
            public_key_point: EncodedPoint,
            proof_of_possession: &'a Signature,
        }

        Package {
            public_key_point: self.public_key_point.to_encoded_point(true),
            proof_of_possession: &self.proof_of_possession,
        }
        .serialize(serializer)
    }
}

/// An encryption public key that can be serialized.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncryptionPublicKey(ProjectivePoint);

impl EncryptionPublicKey {
    /// Tries to construct an encryption public key from a projective point.
    fn from_point(point: ProjectivePoint) -> Result<Self, frost_secp256k1::Error> {
        if point.is_identity().into() {
            return Err(frost_secp256k1::GroupError::InvalidIdentityElement.into());
        }
        Ok(Self(point))
    }

    /// Returns the public key as a secret sharing commitment with the public
    /// key as its only coefficient. The proof of possession uses the same
    /// proof of knowledge scheme as the constant term of the DKG polynomial,
    /// so it is computed and verified over this commitment.
    fn as_commitment(&self) -> keys::VerifiableSecretSharingCommitment {
        keys::VerifiableSecretSharingCommitment::new(vec![
            frost_core::keys::CoefficientCommitment::new(self.0),
        ])
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

/// Encrypts or decrypts `msg` via ECDH: `msg XOR (receiver_pubkey * sender_privkey).x`.
///
/// XOR is its own inverse, so this serves both directions. `receiver_pubkey`
/// must be a valid non-identity point and `sender_privkey` must be non-zero.
fn ecdh(
    sender_privkey: &NonZeroScalar,
    receiver_pubkey: &EncryptionPublicKey,
    msg: [u8; 32],
) -> [u8; 32] {
    let shared_secret = (receiver_pubkey.0 * **sender_privkey).to_affine().x();
    let mut result = msg;
    for (byte, secret) in result.iter_mut().zip(shared_secret) {
        *byte ^= secret;
    }
    result
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

    fn id(id: u16) -> Identifier {
        Identifier::try_from(id).unwrap()
    }

    fn key(pk: u64) -> EncryptionKey {
        EncryptionKey(NonZeroScalar::new(Scalar::from(pk)).unwrap())
    }

    #[test]
    fn roundtrip_encrypt_decrypt() {
        let mut rng = rand::thread_rng();
        let key = EncryptionKey::generate(&mut rng);
        let peer = EncryptionKey::generate(&mut rng);
        let msg = [0x5a; 32];

        let enc = key.ecdh(&peer.public_key(), msg);
        let dec = peer.ecdh(&key.public_key(), enc);
        assert_eq!(dec, msg);
    }

    #[test]
    fn ecdh_is_commutative() {
        let alice = key(2);
        let bob = key(3);
        let msg = [0x00; 32];
        assert_eq!(
            alice.ecdh(&bob.public_key(), msg),
            bob.ecdh(&alice.public_key(), msg),
        );
    }

    #[test]
    fn different_recipient_different_ciphertext() {
        let alice = key(2);
        let bob = key(3);
        let charlie = key(4);
        let msg = [0xff; 32];
        assert_ne!(
            alice.ecdh(&bob.public_key(), msg),
            alice.ecdh(&charlie.public_key(), msg),
        );
    }

    #[test]
    fn proof_of_possession_is_bound_to_identifier() {
        let mut rng = rand::thread_rng();
        let key = EncryptionKey::generate(&mut rng);
        let package = key.package(id(1), &mut rng).unwrap();

        let public_key = package.clone().verified_public_key(id(1)).unwrap();
        assert_eq!(public_key, key.public_key());
        package.verified_public_key(id(2)).unwrap_err();
    }

    #[test]
    fn package_roundtrips_serialization() {
        let mut rng = rand::thread_rng();
        let key = EncryptionKey::generate(&mut rng);
        let package = key.package(id(1), &mut rng).unwrap();

        let json = serde_json::to_string(&package).unwrap();
        let package = serde_json::from_str::<Package>(&json).unwrap();
        let public_key = package.verified_public_key(id(1)).unwrap();
        assert_eq!(public_key, key.public_key());
    }

    #[test]
    fn ecdh_rejects_degenerate_public_keys_at_infinity() {
        assert!(EncryptionPublicKey::from_point(ProjectivePoint::IDENTITY).is_err());
    }
}
