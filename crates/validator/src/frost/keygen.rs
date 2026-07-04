//! Distributed key generation, driving the FROST `keys::dkg::part{1,2,3}`
//! rounds with address-derived identifiers and ECDH-encrypted secret shares.

use super::{ecdh::EncryptionKey, error::Error, marshal, participants};
use crate::{bindings, frost::ecdh::EncryptionPublicKey};
use alloy::primitives::{Address, U256};
use frost_secp256k1::{
    Identifier,
    keys::{
        self,
        dkg::{self, round1, round2},
    },
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The output of DKG round 1: the secret polynomial (persisted to the secret
/// store) and the onchain commitment to publish.
pub struct Round1 {
    pub encryption_key: EncryptionKey,
    pub secret_package: round1::SecretPackage,
    pub commitment: bindings::KeyGenCommitment,
}

/// Runs DKG round 1 for `me`, producing the round 1 secret package and the
/// onchain [`KeyGenCommitment`](bindings::KeyGenCommitment). The `encryption_key`
/// and `rng` are supplied (and the secrets persisted) by the caller.
pub fn generate_round1<R>(
    rng: &mut R,
    me: Address,
    count: u16,
    threshold: u16,
) -> Result<Round1, Error>
where
    R: rand::RngCore + rand::CryptoRng,
{
    let identifier = participants::identifier(me);
    let encryption_key = EncryptionKey::generate(&mut *rng);
    let (secret_package, package) = dkg::part1(identifier, count, threshold, &mut *rng)?;
    let commitment = marshal::solidity_commitment(&encryption_key.public_key(), &package);
    Ok(Round1 {
        encryption_key,
        secret_package,
        commitment,
    })
}

/// A validated round 1 public commitment from a participant.
#[derive(Clone, Deserialize, Serialize)]
pub struct Round1Commitment {
    encryption_public_key: EncryptionPublicKey,
    package: round1::Package,
}

/// Verifies a round 1 public commitment.
///
/// These are applied to _both_ `me`, the validator itself, and all peers that
/// publish key generation commitments onchain.
pub fn verify_round1_commitment(
    min_signers: u16,
    participant: Address,
    commitment: &bindings::KeyGenCommitment,
) -> Result<Round1Commitment, Error> {
    let identifier = participants::identifier(participant);
    let (encryption_public_key, package) = marshal::frost_commitment(commitment)?;
    if package.commitment().coefficients().len() as u16 != min_signers {
        return Err(frost_secp256k1::Error::IncorrectNumberOfCommitments.into());
    }
    frost_core::keys::dkg::verify_proof_of_knowledge(
        identifier,
        package.commitment(),
        package.proof_of_knowledge(),
    )?;
    Ok(Round1Commitment {
        encryption_public_key,
        package,
    })
}

/// The output of DKG round 2: the secret package (persisted) and the onchain
/// secret share to publish.
pub struct Round2 {
    pub secret_package: round2::SecretPackage,
    pub public_key_package: keys::PublicKeyPackage,
    pub share: bindings::KeyGenSecretShare,
}

/// Runs DKG round 2 given every participant's round 1 commitment. Produces the
/// round 2 secret package and the ECDH encrypted secret shares for the peers,
/// ordered by ascending peer address.
pub fn generate_round2(
    encryption_key: &EncryptionKey,
    secret_package: &round1::SecretPackage,
    commitments: &BTreeMap<Address, Round1Commitment>,
) -> Result<Round2, Error> {
    let round1_peer_packages =
        peer_packages(secret_package.identifier(), commitments, |commitment| {
            commitment.package.clone()
        });

    let (secret_package, round2_packages) =
        dkg::part2(secret_package.clone(), &round1_peer_packages)?;

    let round1_commitments = round1_peer_packages
        .iter()
        .map(|(identifier, package)| (*identifier, package.commitment()))
        .chain(std::iter::once((
            *secret_package.identifier(),
            secret_package.commitment(),
        )))
        .collect();
    let public_key_package = keys::PublicKeyPackage::from_dkg_commitments(&round1_commitments)?;
    let verifying_share = public_key_package
        .verifying_shares()
        .get(secret_package.identifier())
        .ok_or(frost_secp256k1::Error::UnknownIdentifier)?;

    let encrypted_shares = commitments
        .iter()
        .map(|(participant, commitment)| {
            (
                participants::identifier(*participant),
                &commitment.encryption_public_key,
            )
        })
        .filter(|(participant, _)| participant != secret_package.identifier())
        .map(|(peer, encryption_public_key)| {
            let package = round2_packages
                .get(&peer)
                .ok_or(frost_secp256k1::Error::UnknownIdentifier)?;
            let signing_share = package.signing_share().to_scalar().to_bytes().into();
            Ok(encryption_key.ecdh(encryption_public_key, signing_share))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let share = marshal::solidity_secret_share(verifying_share, &encrypted_shares);

    Ok(Round2 {
        secret_package,
        public_key_package,
        share,
    })
}

/// A validated round 2 signing share from a peer.
#[derive(Clone, Deserialize, Serialize)]
pub struct Round2Share {
    package: round2::Package,
}

/// Verifies a round 2 secret signing share received from a peer.
pub fn verify_round2_share(
    encryption_key: &EncryptionKey,
    secret_package: &round2::SecretPackage,
    public_key_package: &keys::PublicKeyPackage,
    commitments: &BTreeMap<Address, Round1Commitment>,
    peer: Address,
    share: &bindings::KeyGenSecretShare,
) -> Result<Round2Share, Error> {
    if share.f.len() as u16 != secret_package.max_signers() - 1 {
        return Err(frost_secp256k1::Error::IncorrectNumberOfShares.into());
    }

    let identifier = participants::identifier(peer);
    if identifier == *secret_package.identifier() {
        return Err(frost_secp256k1::Error::IncorrectPackage.into());
    }

    let round1 = &commitments
        .get(&peer)
        .ok_or(frost_secp256k1::Error::UnknownIdentifier)?;
    let encryption_public_key = &round1.encryption_public_key;
    let commitment = round1.package.commitment().clone();

    let verifying_share = public_key_package
        .verifying_shares()
        .get(&identifier)
        .ok_or(frost_secp256k1::Error::UnknownIdentifier)?;

    let index = commitments
        .keys()
        .filter(|address| **address != peer)
        .position(|address| participants::identifier(*address) == *secret_package.identifier())
        .ok_or(frost_secp256k1::Error::UnknownIdentifier)?;
    let encrypted_share = share
        .f
        .get(index)
        .ok_or(frost_core::Error::IncorrectNumberOfShares)?;
    let secret_share = encryption_key.ecdh(encryption_public_key, encrypted_share.to_be_bytes());

    let y = marshal::frost_point(&share.y)?;
    if y != verifying_share.to_element() {
        return Err(frost_secp256k1::Error::MalformedVerifyingKey.into());
    }

    let signing_share =
        keys::SigningShare::new(marshal::frost_scalar(&U256::from_be_bytes(secret_share))?);

    // Pre-verify the share to make sure it matches the commitment from
    // the peer; this allows us to complain right away in case an
    // invalid share was provided.
    let _ =
        keys::SecretShare::new(*secret_package.identifier(), signing_share, commitment).verify()?;

    let package = round2::Package::new(signing_share);
    Ok(Round2Share { package })
}

/// The output of DKG round 3: the finalized key material.
pub struct Round3 {
    pub key_package: keys::KeyPackage,
    pub public_key_package: keys::PublicKeyPackage,
}

/// Runs DKG round 3 for `me`, decrypting the peers' secret shares (indexed by
/// `me`'s position in each publisher's address-ordered participant set) and
/// finalizing the group key material.
pub fn generate_round3(
    secret_package: &round2::SecretPackage,
    commitments: &BTreeMap<Address, Round1Commitment>,
    shares: &BTreeMap<Address, Round2Share>,
) -> Result<Round3, Error> {
    let round1_peer_packages =
        peer_packages(secret_package.identifier(), commitments, |commitment| {
            commitment.package.clone()
        });
    let round2_peer_packages = peer_packages(secret_package.identifier(), shares, |share| {
        share.package.clone()
    });

    let (key_package, public_key_package) =
        dkg::part3(secret_package, &round1_peer_packages, &round2_peer_packages)?;
    Ok(Round3 {
        key_package,
        public_key_package,
    })
}

fn peer_packages<T, P, F>(
    me: &Identifier,
    items: &BTreeMap<Address, T>,
    select: F,
) -> BTreeMap<Identifier, P>
where
    F: Fn(&T) -> P,
{
    items
        .iter()
        .map(|(participant, item)| (participants::identifier(*participant), select(item)))
        .filter(|(participant, _)| participant != me)
        .collect()
}
