//! Utilities for marshalling between Solidity and FROST types.

use crate::bindings;
use alloy::primitives::U256;
use frost_secp256k1::keys::dkg::round1;
use k256::elliptic_curve::sec1::ToEncodedPoint as _;

/// Converts a FROST [`round1::Package`] to the ABI [`KeyGenCommitment`] expected by the
/// FROSTCoordinator contract.
///
/// The mapping is:
///   - `q`  = first coefficient commitment (g^a₀, the public key contribution)
///   - `c`  = all coefficient commitments [g^a₀, …, g^a_{t-1}]
///   - `r`  = proof-of-knowledge nonce commitment R
///   - `mu` = proof-of-knowledge scalar μ
pub fn solidity_commitment(package: round1::Package) -> bindings::KeyGenCommitment {
    let c = package
        .commitment()
        .coefficients()
        .iter()
        .map(|c| solidity_point(&c.value()))
        .collect::<Vec<_>>();
    let q = c.first().cloned().unwrap_or_default();
    let (r, mu) = solidity_signature(package.proof_of_knowledge());
    bindings::KeyGenCommitment { q, c, r, mu }
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
