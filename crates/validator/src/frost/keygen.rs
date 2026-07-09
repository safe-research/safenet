//! Distributed key generation, driving the FROST `keys::dkg::part{1,2,3}`
//! rounds with address-derived identifiers and ECDH-encrypted secret shares.

use super::{
    ecdh::EncryptionKey,
    error::{Culprit as _, Error},
    marshal, participants,
};
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

/// A participant's generated secrets created at [`setup`] and required by
/// [`verify_commitment`] and [`generate_secret_shares`]. Persisted to the
/// secret store between rounds.
#[derive(Clone, Deserialize, Serialize)]
pub struct Secrets {
    encryption_key: EncryptionKey,
    secret_package: round1::SecretPackage,
}

/// The result of [`setup`]: the secrets to persist across rounds and the
/// onchain commitment to publish.
///
/// Note that the secrets are generated with randomness, meaning that they must
/// be persisted in a reorg-resistant way.
pub struct Setup {
    pub secrets: Secrets,
    pub commitment: bindings::KeyGenCommitment,
}

/// Sets up key generation for `me`: samples the ECDH encryption key and the
/// secret polynomial (to persist to the secret store) alongside the onchain
/// [`KeyGenCommitment`](bindings::KeyGenCommitment) to publish. The `rng` is
/// supplied by the caller.
pub fn setup<R>(rng: &mut R, me: Address, count: u16, threshold: u16) -> Result<Setup, Error>
where
    R: rand::RngCore + rand::CryptoRng,
{
    let identifier = participants::identifier(me);
    let encryption_key = EncryptionKey::generate(&mut *rng);
    let (secret_package, package) =
        dkg::part1(identifier, count, threshold, &mut *rng).err_unexpected()?;
    let commitment = marshal::solidity_commitment(&encryption_key.public_key(), &package);

    Ok(Setup {
        secrets: Secrets {
            encryption_key,
            secret_package,
        },
        commitment,
    })
}

/// A validated public commitment from a participant.
#[derive(Clone, Deserialize, Serialize)]
pub struct VerifiedCommitment {
    encryption_public_key: EncryptionPublicKey,
    package: round1::Package,
}

/// Verifies a participant's public commitment.
///
/// These are applied to _both_ `me`, the validator itself, and all peers that
/// publish key generation commitments onchain.
pub fn verify_commitment(
    secrets: &Secrets,
    participant: Address,
    commitment: &bindings::KeyGenCommitment,
) -> Result<VerifiedCommitment, Error> {
    let identifier = participants::identifier(participant);
    marshal::frost_commitment(commitment)
        .and_then(|(encryption_public_key, package)| {
            if package.commitment().coefficients().len() as u16
                != *secrets.secret_package.min_signers()
            {
                return Err(frost_secp256k1::Error::IncorrectNumberOfCommitments);
            }
            frost_core::keys::dkg::verify_proof_of_knowledge(
                identifier,
                package.commitment(),
                package.proof_of_knowledge(),
            )?;
            Ok(VerifiedCommitment {
                encryption_public_key,
                package,
            })
        })
        .err_with_culprit(participant)
}

/// A participant's own secret key-generation state after sharing. Persisted to
/// the secret store and consumed by [`finalize`].
///
/// Unlike [`Secrets`], this can be reconstructed using the generated secrets
/// from [`setup`] and onchain state, and therefore can be stored in the state
/// machine (and does not require a reorg-resistant storage).
#[derive(Clone, Deserialize, Serialize)]
pub struct SharingState {
    encryption_key: EncryptionKey,
    secret_package: round2::SecretPackage,
    group_commitment: keys::VerifiableSecretSharingCommitment,
    commitments: BTreeMap<Address, VerifiedCommitment>,
}

/// The result of [`generate_secret_shares`]: the sharing state and the onchain
/// secret share to publish.
pub struct SecretShares {
    pub sharing_state: SharingState,
    pub share: bindings::KeyGenSecretShare,
}

/// Given every participant's verified commitment, produces the sharing state
/// to persist and the ECDH-encrypted secret shares for the peers, ordered by
/// ascending peer address.
pub fn generate_secret_shares(
    secrets: Secrets,
    commitments: BTreeMap<Address, VerifiedCommitment>,
) -> Result<SecretShares, Error> {
    let (round1_peer_packages, round1_me_package) = peer_packages(
        secrets.secret_package.identifier(),
        &commitments,
        |commitment| commitment.package.clone(),
    )?;

    // By construction, the second round of DKG should have no participant
    // culprits (as the `commitments` have already verified the length of the
    // commitments and the proof of knowledge). The only way we encounter an
    // error here is if the caller were to mix secrets and commitments from
    // different DKG ceremonies.
    dkg::part2(secrets.secret_package.clone(), &round1_peer_packages)
        .and_then(|(secret_package, round2_packages)| {
            // Ensure that the `me` commitment is also valid, the FROST library
            // doesn't check this by default.
            if round1_me_package.commitment() != secret_package.commitment() {
                return Err(frost_secp256k1::Error::IncorrectCommitment);
            }

            // Build the verifying share (i.e. the participant's "public key")
            // from the commitments.
            let group_commitment = frost_core::keys::sum_commitments(
                &commitments
                    .values()
                    .map(|verified| verified.package.commitment())
                    .collect::<Vec<_>>(),
            )?;
            let verifying_share = keys::VerifyingShare::from_commitment(
                *secrets.secret_package.identifier(),
                &group_commitment,
            );

            // Encrypt each of the other shares for the other participants,
            // using their publicly broadcasted public encryption keys.
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
                    Ok(secrets
                        .encryption_key
                        .ecdh(encryption_public_key, signing_share))
                })
                .collect::<Result<Vec<_>, frost_secp256k1::Error>>()?;
            let share = marshal::solidity_secret_share(&verifying_share, &encrypted_shares);

            Ok(SecretShares {
                sharing_state: SharingState {
                    encryption_key: secrets.encryption_key,
                    secret_package,
                    group_commitment,
                    commitments,
                },
                share,
            })
        })
        .err_unexpected()
}

/// A validated signing share from a peer.
#[derive(Clone, Deserialize, Serialize)]
pub struct VerifiedShare {
    package: round2::Package,
}

/// Verifies a participant's secret signing share.
///
/// These are applied to _both_ `me`, the validator itself, and all peers that
/// publish key generation secret shares onchain.
pub fn verify_secret_share(
    sharing_state: &SharingState,
    participant: Address,
    share: &bindings::KeyGenSecretShare,
) -> Result<VerifiedShare, Error> {
    if share.f.len() as u16 != sharing_state.secret_package.max_signers() - 1 {
        return Err(frost_secp256k1::Error::IncorrectNumberOfShares).err_with_culprit(participant);
    }

    let identifier = participants::identifier(participant);
    let (commitment, secret_share) = if identifier == *sharing_state.secret_package.identifier() {
        (
            sharing_state.secret_package.commitment().clone(),
            sharing_state
                .secret_package
                .secret_share()
                .to_bytes()
                .into(),
        )
    } else {
        sharing_state
            .commitments
            .get(&participant)
            .ok_or(frost_secp256k1::Error::UnknownIdentifier)
            .and_then(|round1| {
                let encryption_public_key = &round1.encryption_public_key;
                let commitment = round1.package.commitment().clone();

                // Compute the index of encrypted secret share for _me_ and
                // decrypt it so that we can verify it against the peer's
                // commitments.
                let index = sharing_state
                    .commitments
                    .keys()
                    .filter(|address| **address != participant)
                    .position(|address| {
                        participants::identifier(*address)
                            == *sharing_state.secret_package.identifier()
                    })
                    .ok_or(frost_secp256k1::Error::UnknownIdentifier)?;
                let encrypted_share = share
                    .f
                    .get(index)
                    .ok_or(frost_core::Error::IncorrectNumberOfShares)?;
                let secret_share = sharing_state
                    .encryption_key
                    .ecdh(encryption_public_key, encrypted_share.to_be_bytes());

                Ok((commitment, secret_share))
            })
            .err_unexpected()?
    };

    let package = marshal::frost_point(&share.y)
        .and_then(|y| {
            let verifying_share =
                keys::VerifyingShare::from_commitment(identifier, &sharing_state.group_commitment);
            if y != verifying_share.to_element() {
                return Err(frost_secp256k1::Error::MalformedVerifyingKey);
            }

            // Pre-verify the share to make sure it matches the commitment from
            // the peer; this allows us to complain right away in case an
            // invalid share was provided.
            let signing_share =
                keys::SigningShare::new(marshal::frost_scalar(&U256::from_be_bytes(secret_share))?);
            let _ = keys::SecretShare::new(
                *sharing_state.secret_package.identifier(),
                signing_share,
                commitment,
            )
            .verify()?;

            Ok(round2::Package::new(signing_share))
        })
        .err_with_culprit(participant)?;

    Ok(VerifiedShare { package })
}

/// A participant's share of the group signing key: its secret signing share,
/// verifying share, and the group verifying key. The final result of DKG,
/// persisted to the secret store and used to sign.
#[derive(Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct KeyShare(keys::KeyPackage);

impl KeyShare {
    /// The underlying FROST key package, used to produce signature shares.
    pub(super) fn as_key_package(&self) -> &keys::KeyPackage {
        &self.0
    }

    /// Creates a dummy key share, used for testing.
    #[cfg(test)]
    pub(crate) fn dummy() -> Self {
        KeyShare(frost_secp256k1::keys::KeyPackage::new(
            frost_secp256k1::Identifier::try_from(1).unwrap(),
            frost_secp256k1::keys::SigningShare::new(k256::Scalar::ONE),
            frost_secp256k1::keys::VerifyingShare::new(k256::ProjectivePoint::GENERATOR),
            frost_secp256k1::VerifyingKey::new(k256::ProjectivePoint::GENERATOR),
            1,
        ))
    }
}

/// Finalizes key generation given all participant's verified secret shares
/// and derives the participant's [`KeyShare`].
pub fn finalize(
    sharing_state: SharingState,
    shares: BTreeMap<Address, VerifiedShare>,
) -> Result<KeyShare, Error> {
    let (round1_peer_packages, _) = peer_packages(
        sharing_state.secret_package.identifier(),
        &sharing_state.commitments,
        |commitment| commitment.package.clone(),
    )?;
    let (round2_peer_packages, round2_me_package) = peer_packages(
        sharing_state.secret_package.identifier(),
        &shares,
        |share| share.package.clone(),
    )?;

    // Verify that the me package matches the secret state. This is otherwise
    // not checked by the FROST library.
    if round2_me_package.signing_share().to_scalar() != sharing_state.secret_package.secret_share()
    {
        return Err(frost_secp256k1::Error::IncorrectPackage).err_unexpected();
    }

    // By construction, the third round of DKG should have no participant
    // culprits (as the `commitments` and `shares` have already been verified).
    // The only way we encounter an error here is if the caller were to mix
    // secrets and commitments and shares from different DKG ceremonies.
    let (key_package, _) = dkg::part3(
        &sharing_state.secret_package,
        &round1_peer_packages,
        &round2_peer_packages,
    )
    .err_unexpected()?;

    Ok(KeyShare(key_package))
}

fn peer_packages<T, P, F>(
    me: &Identifier,
    items: &BTreeMap<Address, T>,
    select: F,
) -> Result<(BTreeMap<Identifier, P>, P), Error>
where
    F: Fn(&T) -> P,
{
    let mut peers = items
        .iter()
        .map(|(participant, item)| (participants::identifier(*participant), select(item)))
        .collect::<BTreeMap<_, _>>();
    let me = peers
        .remove(me)
        .ok_or(frost_secp256k1::Error::UnknownIdentifier)
        .err_unexpected()?;
    Ok((peers, me))
}
