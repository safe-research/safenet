//! Distributed key generation, driving the FROST `keys::dkg::part{1,2,3}`
//! rounds with address-derived identifiers and ECDH-encrypted secret shares.

use super::{ecdh::EncryptionKey, error::Error, marshal, participants};
use crate::{bindings, frost::ecdh::EncryptionPublicKey};
use alloy::primitives::Address;
use frost_secp256k1::{
    Identifier,
    keys::{
        self, PublicKeyPackage,
        dkg::{self, round1, round2},
    },
};
use k256::elliptic_curve::PrimeField as _;
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

/// The output of DKG round 2: the secret package (persisted) and the onchain
/// secret share to publish.
pub struct Round2 {
    pub secret_package: round2::SecretPackage,
    pub share: bindings::KeyGenSecretShare,
}

/// Runs DKG round 2 for `me` given every participant's round 1 commitment
/// (including `me`'s own). Produces the round 2 secret package and the ECDH
/// encrypted secret shares for the peers, ordered by ascending peer address.
pub fn generate_round2(
    me: Address,
    encryption_key: &EncryptionKey,
    secret_package: &round1::SecretPackage,
    commitments: &BTreeMap<Address, bindings::KeyGenCommitment>,
) -> Result<Round2, Error> {
    let (encryption_keys, round1_packages) = decode_peer_commitments(me, commitments)?;
    let (secret_package, round2_packages) = dkg::part2(secret_package.clone(), &round1_packages)?;

    let public_key_package = PublicKeyPackage::from_dkg_commitments(
        &round1_packages
            .iter()
            .map(|(identifier, package)| (*identifier, package.commitment()))
            .chain(std::iter::once((
                *secret_package.identifier(),
                secret_package.commitment(),
            )))
            .collect(),
    )?;
    let verifying_share = public_key_package
        .verifying_shares()
        .get(secret_package.identifier())
        .ok_or(frost_secp256k1::Error::IncorrectNumberOfPackages)?;

    let encrypted_shares = encryption_keys
        .iter()
        .map(|(peer, encryption_key_peer)| {
            let package = round2_packages
                .get(&participants::identifier(*peer))
                .ok_or(frost_secp256k1::Error::UnknownIdentifier)?;
            let signing_share = package.signing_share().to_scalar().to_bytes().into();
            Ok(encryption_key.ecdh(encryption_key_peer, signing_share))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let share = marshal::solidity_secret_share(verifying_share, &encrypted_shares);

    Ok(Round2 {
        secret_package,
        share,
    })
}

/// The output of DKG round 3: the finalized key material.
pub struct Round3 {
    pub key_package: keys::KeyPackage,
    pub public_key_package: PublicKeyPackage,
}

/// Runs DKG round 3 for `me`, decrypting the peers' secret shares (indexed by
/// `me`'s position in each publisher's address-ordered participant set) and
/// finalizing the group key material.
pub fn generate_round3(
    me: Address,
    encryption_key: &EncryptionKey,
    secret_package: &round2::SecretPackage,
    commitments: &BTreeMap<Address, bindings::KeyGenCommitment>,
    shares: &BTreeMap<Address, bindings::KeyGenSecretShare>,
) -> Result<Round3, Error> {
    let (encryption_keys, round1_packages) = decode_peer_commitments(me, commitments)?;
    let round2_packages = shares
        .iter()
        .filter(|(publisher, _)| **publisher != me)
        .map(|(publisher, share)| {
            let index = shares
                .keys()
                .filter(|address| *address != publisher)
                .position(|address| *address == me)
                .ok_or(frost_secp256k1::Error::UnknownIdentifier)?;
            let encrypted = share
                .f
                .get(index)
                .ok_or(frost_core::Error::IncorrectNumberOfShares)?;
            let encryption_key_peer = encryption_keys
                .get(publisher)
                .ok_or(frost_secp256k1::Error::UnknownIdentifier)?;
            let secret_share = encryption_key.ecdh(encryption_key_peer, encrypted.to_be_bytes());
            let signing_share = keys::SigningShare::new(
                k256::Scalar::from_repr(secret_share.into())
                    .into_option()
                    .ok_or_else(Error::malformed_scalar)?,
            );
            Ok((
                participants::identifier(*publisher),
                round2::Package::new(signing_share),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, Error>>()?;

    let (key_package, public_key_package) =
        dkg::part3(secret_package, &round1_packages, &round2_packages)?;
    Ok(Round3 {
        key_package,
        public_key_package,
    })
}

type PeerCommitments = (
    BTreeMap<Address, EncryptionPublicKey>,
    BTreeMap<Identifier, round1::Package>,
);

fn decode_peer_commitments(
    me: Address,
    commitments: &BTreeMap<Address, bindings::KeyGenCommitment>,
) -> Result<PeerCommitments, Error> {
    let mut encryption_keys = BTreeMap::new();
    let mut round1_packages = BTreeMap::new();
    for (address, commitment) in commitments {
        if *address == me {
            continue;
        }

        let (encryption_key, package) = marshal::frost_commitment(commitment)?;
        encryption_keys.insert(*address, encryption_key);
        round1_packages.insert(participants::identifier(*address), package);
    }
    Ok((encryption_keys, round1_packages))
}
