//! Serialization for FROST key generation values that may contain the identity
//! point, which FROST itself refuses to (de)serialize.
//!
//! Commitments published onchain are not checked to be non-identity: beyond
//! the constant term (which is bound by a proof of knowledge), a participant
//! can choose its coefficient commitments to cancel out a coefficient of the
//! group commitment. These values must remain persistable so that the
//! ceremony can progress to secret sharing, where the misbehaving participant
//! cannot produce valid shares and is identified.
//!
//! Points are encoded as SEC1 compressed points, with the identity encoded as
//! a single `0x00` byte.

use frost_secp256k1::keys::{self, dkg::round1};
use k256::AffinePoint;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Identity-tolerant serialization for a [`VerifiableSecretSharingCommitment`](keys::VerifiableSecretSharingCommitment).
pub mod vss_commitment {
    use super::*;

    pub fn serialize<S>(
        commitment: &keys::VerifiableSecretSharingCommitment,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_seq(
            commitment
                .coefficients()
                .iter()
                .map(|coefficient| AffinePoint::from(coefficient.value())),
        )
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<keys::VerifiableSecretSharingCommitment, D::Error>
    where
        D: Deserializer<'de>,
    {
        let coefficients = Vec::<AffinePoint>::deserialize(deserializer)?;
        Ok(keys::VerifiableSecretSharingCommitment::new(
            coefficients
                .into_iter()
                .map(|coefficient| frost_core::keys::CoefficientCommitment::new(coefficient.into()))
                .collect(),
        ))
    }
}

/// Identity-tolerant serialization for a DKG round 1 [`Package`](round1::Package).
pub mod round1_package {
    use super::*;

    #[derive(Deserialize, Serialize)]
    struct Package {
        #[serde(with = "vss_commitment")]
        commitment: keys::VerifiableSecretSharingCommitment,
        proof_of_knowledge: frost_secp256k1::Signature,
    }

    pub fn serialize<S>(package: &round1::Package, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Package {
            commitment: package.commitment().clone(),
            proof_of_knowledge: *package.proof_of_knowledge(),
        }
        .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<round1::Package, D::Error>
    where
        D: Deserializer<'de>,
    {
        let package = Package::deserialize(deserializer)?;
        Ok(round1::Package::new(
            package.commitment,
            package.proof_of_knowledge,
        ))
    }
}
