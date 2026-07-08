//! Marshalling between secp256k1 primitives and their Solidity types.

use super::error;
use crate::{bindings, frost::ecdh::EncryptionPublicKey};
use alloy::primitives::U256;
use frost_secp256k1::keys::{self, dkg};
use k256::{
    Scalar,
    elliptic_curve::{
        Group, PrimeField as _,
        sec1::{FromEncodedPoint as _, ToEncodedPoint as _},
    },
};

/// Converts a DKG round 1 [`Package`](dkg::round1::Package) and an encryption
/// public key to the ABI [`KeyGenCommitment`](bindings::KeyGenCommitment):
///
/// - `q`  the encryption public key for ECDH-encrypted secret shares,
/// - `c`  all coefficient commitments `[g^a₀, …, g^a_{t-1}]`,
/// - `r`  the proof-of-knowledge nonce commitment `R`,
/// - `mu` the proof-of-knowledge scalar `μ`.
pub fn solidity_commitment(
    encryption_public_key: &EncryptionPublicKey,
    package: &dkg::round1::Package,
) -> bindings::KeyGenCommitment {
    let q = solidity_point(encryption_public_key.as_point());
    let c = package
        .commitment()
        .coefficients()
        .iter()
        .map(|coefficient| solidity_point(&coefficient.value()))
        .collect();
    let bindings::Signature { r, z } = solidity_signature(package.proof_of_knowledge());
    bindings::KeyGenCommitment { q, c, r, mu: z }
}

/// Converts a verifying share and the peer-encrypted secret shares to the ABI
/// [`KeyGenSecretShare`](bindings::KeyGenSecretShare).
///
/// `encrypted_secret_shares` must already be in the canonical publishing order
/// (ascending participant address, excluding self), matching the TypeScript
/// `KeyGenClient.createSecretShares`; the contract and peers index `f` by this
/// order.
pub fn solidity_secret_share(
    verifying_share: &keys::VerifyingShare,
    encrypted_secret_shares: &[[u8; 32]],
) -> bindings::KeyGenSecretShare {
    let y = solidity_point(&verifying_share.to_element());
    let f = encrypted_secret_shares
        .iter()
        .copied()
        .map(U256::from_be_bytes)
        .collect();
    bindings::KeyGenSecretShare { y, f }
}

/// Converts a secp256k1 point to the ABI `Point { uint256 x; uint256 y }`.
pub fn solidity_point(point: &k256::ProjectivePoint) -> bindings::Point {
    if point.is_identity().into() {
        return bindings::Point {
            x: U256::ZERO,
            y: U256::ZERO,
        };
    }

    let encoded = point.to_encoded_point(false);
    let (chunks, _) = encoded.as_bytes()[1..].as_chunks::<32>();
    bindings::Point {
        x: U256::from_be_bytes(chunks[0]),
        y: U256::from_be_bytes(chunks[1]),
    }
}

/// Converts a secp256k1 scalar to a `U256`.
pub fn solidity_scalar(scalar: &k256::Scalar) -> U256 {
    U256::from_be_bytes(scalar.to_bytes().into())
}

/// Converts a FROST signature to the ABI `(Point r, uint256 z)` pair.
pub fn solidity_signature(signature: &frost_secp256k1::Signature) -> bindings::Signature {
    bindings::Signature {
        r: solidity_point(signature.R()),
        z: solidity_scalar(signature.z()),
    }
}

/// Converts an ABI [`KeyGenCommitment`](bindings::KeyGenCommitment) back to the
/// peer's encryption public key and DKG round 1 package.
pub fn frost_commitment(
    commitment: &bindings::KeyGenCommitment,
) -> Result<(EncryptionPublicKey, dkg::round1::Package), frost_secp256k1::Error> {
    let encryption_public_key = frost_point(&commitment.q)?.try_into()?;
    let coefficients = keys::VerifiableSecretSharingCommitment::new(
        commitment
            .c
            .iter()
            .map(|coefficient| {
                let coefficient = frost_point(coefficient)?;
                Ok(frost_core::keys::CoefficientCommitment::new(coefficient))
            })
            .collect::<Result<_, frost_secp256k1::Error>>()?,
    );
    let proof_of_knowledge = frost_signature(&commitment.r, &commitment.mu)?;
    let package = dkg::round1::Package::new(coefficients, proof_of_knowledge);
    Ok((encryption_public_key, package))
}

/// Converts a `U256` to a canonical secp256k1 scalar.
pub fn frost_scalar(scalar: &U256) -> Result<k256::Scalar, frost_secp256k1::Error> {
    Scalar::from_repr(scalar.to_be_bytes().into())
        .into_option()
        .ok_or_else(error::malformed_scalar)
}

/// Converts an ABI `Point { uint256 x; uint256 y }` to a secp256k1 point. The
/// zero point decodes to the identity.
pub fn frost_point(
    point: &bindings::Point,
) -> Result<k256::ProjectivePoint, frost_secp256k1::Error> {
    if point.x.is_zero() && point.y.is_zero() {
        return Ok(k256::ProjectivePoint::IDENTITY);
    }

    let mut buffer = [0; 65];
    buffer[0] = 0x04;
    buffer[1..33].copy_from_slice(&point.x.to_be_bytes::<32>());
    buffer[33..65].copy_from_slice(&point.y.to_be_bytes::<32>());
    let encoded = k256::EncodedPoint::from_bytes(buffer).map_err(|_| error::malformed_element())?;
    k256::ProjectivePoint::from_encoded_point(&encoded)
        .into_option()
        .ok_or_else(error::malformed_element)
}

/// Converts an ABI Schnorr signature `(Point r, uint256 z)` to a FROST signature.
pub fn frost_signature(
    r: &bindings::Point,
    z: &U256,
) -> Result<frost_secp256k1::Signature, frost_secp256k1::Error> {
    Ok(frost_secp256k1::Signature::new(
        frost_point(r)?,
        frost_scalar(z)?,
    ))
}
