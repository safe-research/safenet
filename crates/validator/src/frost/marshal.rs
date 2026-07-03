//! Marshalling between secp256k1 primitives and their Solidity types.

use super::error::Error;
use crate::bindings;
use alloy::primitives::U256;
use k256::{
    Scalar,
    elliptic_curve::{
        Group, PrimeField as _,
        sec1::{FromEncodedPoint as _, ToEncodedPoint as _},
    },
};

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

/// Converts a `U256` to a canonical secp256k1 scalar.
pub fn frost_scalar(scalar: &U256) -> Result<k256::Scalar, Error> {
    Scalar::from_repr(scalar.to_be_bytes().into())
        .into_option()
        .ok_or_else(Error::malformed_element)
}

/// Converts an ABI `Point { uint256 x; uint256 y }` to a secp256k1 point. The
/// zero point decodes to the identity.
pub fn frost_point(point: &bindings::Point) -> Result<k256::ProjectivePoint, Error> {
    if point.x.is_zero() && point.y.is_zero() {
        return Ok(k256::ProjectivePoint::IDENTITY);
    }

    let mut buffer = [0; 65];
    buffer[0] = 0x04;
    buffer[1..33].copy_from_slice(&point.x.to_be_bytes::<32>());
    buffer[33..65].copy_from_slice(&point.y.to_be_bytes::<32>());
    let encoded = k256::EncodedPoint::from_bytes(buffer).map_err(|_| Error::malformed_element())?;
    k256::ProjectivePoint::from_encoded_point(&encoded)
        .into_option()
        .ok_or_else(Error::malformed_element)
}

/// Converts an ABI Schnorr signature `(Point r, uint256 z)` to a FROST signature.
pub fn frost_signature(r: &bindings::Point, z: &U256) -> Result<frost_secp256k1::Signature, Error> {
    Ok(frost_secp256k1::Signature::new(
        frost_point(r)?,
        frost_scalar(z)?,
    ))
}
