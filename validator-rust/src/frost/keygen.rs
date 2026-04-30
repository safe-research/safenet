use anyhow::Result;
use frost_secp256k1::{
    Identifier,
    keys::dkg::{self, round1},
    rand_core::OsRng,
};

use crate::{bindings::KeyGenCommitment, frost::marshal};

/// Runs DKG round 1 for the given participant, returning the secret package (for persistence)
/// and the on-chain commitment data.
pub fn generate_round1(
    identifier: Identifier,
    max_signers: u16,
    min_signers: u16,
) -> Result<(round1::SecretPackage, KeyGenCommitment)> {
    let (secret_package, package) = dkg::part1(identifier, max_signers, min_signers, OsRng)?;
    Ok((secret_package, marshal::solidity_commitment(package)))
}
