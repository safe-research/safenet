use anyhow::Result;
use frost_secp256k1::{
    Identifier,
    keys::dkg::{self, round1},
    rand_core::OsRng,
};

use crate::bindings::KeyGenCommitment;

/// Derives a FROST [`Identifier`] from an Ethereum address.
pub fn identifier_from_address(address: alloy::primitives::Address) -> Identifier {
    todo!("derive FROST Identifier from Ethereum address using hid hash-to-scalar: {address}")
}

/// Runs DKG round 1 for the given participant, returning the secret package (serialized
/// for persistence) and the on-chain commitment data.
pub fn generate_round1(
    identifier: Identifier,
    max_signers: u16,
    min_signers: u16,
) -> Result<(round1::SecretPackage, KeyGenCommitment)> {
    let (secret_package, package) = dkg::part1(identifier, max_signers, min_signers, OsRng)?;
    Ok((secret_package, package_to_commitment(package)?))
}

/// Converts a FROST [`round1::Package`] to the ABI [`KeyGenCommitment`] expected by the
/// FROSTCoordinator contract.
///
/// The mapping is:
///   - `q`  = first coefficient commitment (g^a₀, the public key contribution)
///   - `c`  = all coefficient commitments [g^a₀, …, g^a_{t-1}]
///   - `r`  = proof-of-knowledge nonce commitment R
///   - `mu` = proof-of-knowledge scalar μ
fn package_to_commitment(_package: round1::Package) -> Result<KeyGenCommitment> {
    todo!(
        "extract commitment vector and proof_of_knowledge from round1::Package and marshal each point to ABI Point {{ uint256 x; uint256 y }}"
    )
}
