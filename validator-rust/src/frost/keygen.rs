use std::{cmp::Ordering, collections::BTreeMap, iter};

use crate::{
    bindings,
    frost::{marshal, participants, secret::EncryptionKey},
};
use alloy::primitives::Address;
use anyhow::{Context as _, Result};
use frost_secp256k1::keys::{
    self,
    dkg::{self, round1, round2},
};
use k256::elliptic_curve::PrimeField;

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
pub fn generate_round1(participant: Address, max_signers: u16, min_signers: u16) -> Result<Round1> {
    let mut rng = rand::thread_rng();
    let encryption_key = EncryptionKey::generate(&mut rng);
    let identifier = participants::identifier(participant);
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
    commitments: &BTreeMap<Address, bindings::KeyGenCommitment>,
) -> Result<Round2> {
    let (encryption_public_keys, packages) =
        round1_packages(secret_package.identifier(), commitments)?;
    let (secret_package, package) = dkg::part2(secret_package.clone(), &packages)?;
    let public_key_package = frost_secp256k1::keys::PublicKeyPackage::from_dkg_commitments(
        &packages
            .iter()
            .map(|(&identifier, package)| (identifier, package.commitment()))
            .chain(iter::once((
                *secret_package.identifier(),
                secret_package.commitment(),
            )))
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
                encryption_key.ecdh(encryption_public_key, signing_share)?,
            ))
        })
        .collect::<Result<_>>()?;
    let share = marshal::solidity_secret_share(verifying_share, &encrypted_signing_shares);

    Ok(Round2 {
        secret_package,
        share,
    })
}

/// Result of DKG round 3 including the encryption keypair.
pub struct Round3 {
    pub key_package: keys::KeyPackage,
}

/// Runs DKG round 3 for the given participant, returning an ECDH encryption keypair,
/// the polynomial coefficients (for secret share creation), and the on-chain commitment data.
///
/// Port of TypeScript `setupKeyGen` + `createProofOfKnowledge` + `createCommitments`.
pub fn generate_round3(
    encryption_key: &EncryptionKey,
    secret_package: &round2::SecretPackage,
    commitments: &BTreeMap<Address, bindings::KeyGenCommitment>,
    shares: &BTreeMap<Address, bindings::KeyGenSecretShare>,
) -> Result<Round3> {
    let (encryption_public_keys, round1_packages) =
        round1_packages(secret_package.identifier(), commitments)?;
    let encrypted_secret_shares = round2_encrypted_shares(secret_package.identifier(), shares)?;
    let round2_packages = encrypted_secret_shares
        .into_iter()
        .map(|(identifier, encrypted_secret_share)| {
            let encryption_public_key = encryption_public_keys
                .get(&identifier)
                .context("missing encryption public key")?;
            let secret_share =
                encryption_key.ecdh(encryption_public_key, encrypted_secret_share)?;
            let signing_share = keys::SigningShare::new(
                k256::Scalar::from_repr(secret_share.into())
                    .into_option()
                    .context("invalid decrypted signing share")?,
            );
            let package = round2::Package::new(signing_share);
            Ok((identifier, package))
        })
        .collect::<Result<_>>()?;
    let (key_package, _) = dkg::part3(secret_package, &round1_packages, &round2_packages)?;

    Ok(Round3 { key_package })
}

fn round1_packages(
    own_identifier: &frost_secp256k1::Identifier,
    commitments: &BTreeMap<Address, bindings::KeyGenCommitment>,
) -> Result<(
    BTreeMap<frost_secp256k1::Identifier, k256::ProjectivePoint>,
    BTreeMap<frost_secp256k1::Identifier, round1::Package>,
)> {
    let mut encryption_public_keys = BTreeMap::new();
    let mut packages = BTreeMap::new();
    for (participant, commitment) in commitments {
        let identifier = participants::identifier(*participant);
        if identifier == *own_identifier {
            continue;
        }

        let (encryption_public_key, package) = marshal::frost_commitment(commitment)?;
        encryption_public_keys.insert(identifier, encryption_public_key);
        packages.insert(identifier, package);
    }
    Ok((encryption_public_keys, packages))
}

fn round2_encrypted_shares(
    own_identifier: &frost_secp256k1::Identifier,
    shares: &BTreeMap<Address, bindings::KeyGenSecretShare>,
) -> Result<BTreeMap<frost_secp256k1::Identifier, [u8; 32]>> {
    let mut encrypted_secret_shares = BTreeMap::new();
    let index = shares
        .keys()
        .enumerate()
        .find_map(|(index, participant)| {
            (participants::identifier(*participant) == *own_identifier).then_some(index)
        })
        .context("participant not found in secret shares")?;
    for (participant, share) in shares {
        let identifier = participants::identifier(*participant);
        let index = match identifier.cmp(own_identifier) {
            Ordering::Less => index - 1,
            Ordering::Equal => continue,
            Ordering::Greater => index,
        };
        let encrypted_secret_share = share.f.get(index).context("missing secret share")?;
        encrypted_secret_shares.insert(identifier, encrypted_secret_share.to_be_bytes());
    }
    Ok(encrypted_secret_shares)
}
