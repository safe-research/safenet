use std::collections::BTreeMap;

use crate::{
    bindings,
    frost::{marshal, secret::EncryptionKey},
};
use anyhow::{Context as _, Result};
use frost_secp256k1::{
    Identifier,
    keys::dkg::{self, round1, round2},
};

/// Result of DKG round 1 including the encryption keypair.
pub struct Round1 {
    pub encryption_key: EncryptionKey,
    pub secret_package: round1::SecretPackage,
    pub commitment: bindings::KeyGenCommitment,
}

/// Runs DKG round 1 for the given participant, returning an ECDH encryption keypair,
/// the polynomial coefficients (for secret share creation), and the on-chain commitment data.
///
/// Port of TypeScript `setupKeyGen` + `createProofOfKnowledge` + `createCommitments`.
pub fn generate_round1(
    identifier: Identifier,
    max_signers: u16,
    min_signers: u16,
) -> Result<Round1> {
    let mut rng = rand::thread_rng();
    let encryption_key = EncryptionKey::generate(&mut rng);
    let (secret_package, package) = dkg::part1(identifier, max_signers, min_signers, &mut rng)?;
    let commitment = marshal::solidity_commitment(&encryption_key.public_key(), &package);

    Ok(Round1 {
        encryption_key,
        secret_package,
        commitment,
    })
}

/// Result of DKG round 2 including the encryption keypair.
pub struct Round2 {
    pub secret_package: round2::SecretPackage,
    pub share: bindings::KeyGenSecretShare,
}

/// Runs DKG round 2 for the given participant, returning an ECDH encryption keypair,
/// the polynomial coefficients (for secret share creation), and the on-chain commitment data.
///
/// Port of TypeScript `setupKeyGen` + `createProofOfKnowledge` + `createCommitments`.
pub fn generate_round2(
    encryption_key: &EncryptionKey,
    secret_package: &round1::SecretPackage,
    commitments: &BTreeMap<Identifier, bindings::KeyGenCommitment>,
) -> Result<Round2> {
    let (encryption_public_keys, packages) = {
        let mut encryption_public_keys = BTreeMap::new();
        let mut packages = BTreeMap::new();
        for (&identifier, commitment) in commitments {
            let (encryption_public_key, package) = marshal::frost_commitment(commitment)?;
            encryption_public_keys.insert(identifier, encryption_public_key);
            packages.insert(identifier, package);
        }
        (encryption_public_keys, packages)
    };
    let (secret_package, package) = dkg::part2(secret_package.clone(), &packages)?;
    let public_key_package = frost_secp256k1::keys::PublicKeyPackage::from_dkg_commitments(
        &packages
            .iter()
            .map(|(&identifier, package)| (identifier, package.commitment()))
            .collect(),
    )?;
    let verifying_share = public_key_package
        .verifying_shares()
        .get(secret_package.identifier())
        .context("missing verifying share for self")?;
    let encrypted_signing_shares = package
        .iter()
        .map(|(&identifier, package)| {
            let encryption_public_key = encryption_public_keys
                .get(&identifier)
                .context("missing encryption key for participant")?;
            let signing_share = package.signing_share().to_scalar().to_bytes().into();
            Ok((
                identifier,
                encryption_key.encrypt(encryption_public_key, signing_share)?,
            ))
        })
        .collect::<Result<_>>()?;
    let share = marshal::solidity_secret_share(verifying_share, &encrypted_signing_shares);

    Ok(Round2 {
        secret_package,
        share,
    })
}
