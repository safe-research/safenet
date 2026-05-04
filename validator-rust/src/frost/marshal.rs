//! Utilities for marshalling between Solidity and FROST types.

use crate::bindings;
use alloy::primitives::U256;
use anyhow::{Context as _, Result};
use frost_core::keys;
use frost_secp256k1::keys::dkg;
use k256::{
    Scalar,
    elliptic_curve::{
        PrimeField as _,
        sec1::{FromEncodedPoint as _, ToEncodedPoint as _},
    },
};
use std::collections::BTreeMap;

/// Converts a FROST [`dkg::round1::Package`] and an encryption public key to the ABI
/// [`KeyGenCommitment`] expected by the FROSTCoordinator contract.
///
/// The mapping is:
///   - `q`  = encryption public key for ECDH-encrypted secret shares
///   - `c`  = all coefficient commitments [g^a₀, …, g^a_{t-1}]
///   - `r`  = proof-of-knowledge nonce commitment R
///   - `mu` = proof-of-knowledge scalar μ
pub fn solidity_commitment(
    encryption_public_key: &k256::ProjectivePoint,
    package: &dkg::round1::Package,
) -> bindings::KeyGenCommitment {
    let q = solidity_point(encryption_public_key);
    let c = package
        .commitment()
        .coefficients()
        .iter()
        .map(|c| solidity_point(&c.value()))
        .collect::<Vec<_>>();
    let (r, mu) = solidity_signature(package.proof_of_knowledge());
    bindings::KeyGenCommitment { q, c, r, mu }
}

/// Converts FROST [`dkg::round2::Package`]s and an encryption public key to the ABI
/// [`KeyGenSecretShare`] expected by the FROSTCoordinator contract.
pub fn solidity_secret_share(
    verifying_share: &frost_secp256k1::keys::VerifyingShare,
    encrypted_secret_shares: &BTreeMap<frost_secp256k1::Identifier, [u8; 32]>,
) -> bindings::KeyGenSecretShare {
    let y = solidity_point(&verifying_share.to_element());
    let f = encrypted_secret_shares
        .values()
        .copied()
        .map(U256::from_be_bytes)
        .collect();
    bindings::KeyGenSecretShare { y, f }
}

/// Converts a secp256k1 projective point to the ABI `Point { uint256 x; uint256 y }` format.
///
/// Encodes uncompressed (`04 || x || y`), discards the `04` prefix byte, then reads the two
/// 32-byte halves as big-endian `U256` values.
pub fn solidity_point(point: &k256::ProjectivePoint) -> bindings::Point {
    let encoded = point.to_encoded_point(false);
    debug_assert!(encoded.as_bytes().len() == 65, "unexpected point encoding");
    let (chunks, _) = encoded.as_bytes()[1..].as_chunks::<32>();
    let x = U256::from_be_bytes(chunks[0]);
    let y = U256::from_be_bytes(chunks[1]);
    bindings::Point { x, y }
}

/// Converts a secp256k1 scalar to a `U256`.
pub fn solidity_scalar(scalar: &k256::Scalar) -> U256 {
    U256::from_be_bytes(scalar.to_bytes().into())
}

/// Converts a FROST signature to the ABI `(Point r, uint256 z)` pair.
pub fn solidity_signature(signature: &frost_secp256k1::Signature) -> (bindings::Point, U256) {
    let r = solidity_point(signature.R());
    let z = solidity_scalar(signature.z());
    (r, z)
}

// TODO: Claude
pub fn frost_commitment(
    commitment: &bindings::KeyGenCommitment,
) -> Result<(k256::ProjectivePoint, dkg::round1::Package)> {
    let encryption_public_key = frost_point(&commitment.q)?;
    let coefficients = keys::VerifiableSecretSharingCommitment::new(
        commitment
            .c
            .iter()
            .map(|c| Ok(keys::CoefficientCommitment::new(frost_point(c)?)))
            .collect::<Result<_>>()?,
    );
    let proof_of_knowledge = frost_signature(&commitment.r, &commitment.mu)?;
    let package = dkg::round1::Package::new(coefficients, proof_of_knowledge);
    Ok((encryption_public_key, package))
}

/// Converts a `U256` to a secp256k1 scalar.
pub fn frost_scalar(scalar: &U256) -> Result<k256::Scalar> {
    Scalar::from_repr(scalar.to_be_bytes().into())
        .into_option()
        .context("invalid scalar value")
}

/// Converts an ABI `Point { uint256 x; uint256 y }` to a FROST projecive point.
pub fn frost_point(point: &bindings::Point) -> Result<k256::ProjectivePoint> {
    if point.x.is_zero() && point.y.is_zero() {
        return Ok(k256::ProjectivePoint::IDENTITY);
    }
    let mut buffer = [0; 65];
    buffer[0] = 0x04;
    buffer[1..33].copy_from_slice(&point.x.to_be_bytes::<32>());
    buffer[33..65].copy_from_slice(&point.y.to_be_bytes::<32>());
    let encoded = k256::EncodedPoint::from_bytes(buffer)?;
    let point = k256::ProjectivePoint::from_encoded_point(&encoded)
        .into_option()
        .context("invalid secp256k1 point encoding")?;
    Ok(point)
}

/// Converts an ABI Schnorr signature into a FROST signature.
pub fn frost_signature(r: &bindings::Point, z: &U256) -> Result<frost_secp256k1::Signature> {
    Ok(frost_secp256k1::Signature::new(
        frost_point(r)?,
        frost_scalar(z)?,
    ))
}
