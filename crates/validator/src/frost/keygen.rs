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
    Identifier, Signature, VerifyingKey,
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
///
/// Note that the secrets are generated with randomness, meaning that they must
/// be persisted in a reorg-resistant way.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Secrets {
    encryption_key: EncryptionKey,
    secret_package: round1::SecretPackage,
    proof_of_knowledge: Signature,
}

impl Secrets {
    /// Builds the onchain [`KeyGenCommitment`](bindings::KeyGenCommitment) to
    /// publish for these secrets.
    pub fn commitment(&self) -> bindings::KeyGenCommitment {
        let round1_package = round1::Package::new(
            self.secret_package.commitment().clone(),
            self.proof_of_knowledge,
        );
        marshal::solidity_commitment(&self.encryption_key.public_key(), &round1_package)
    }
}

/// Sets up key generation for `me`: samples the ECDH encryption key and the
/// secret polynomial to persist to the secret store. The `rng` is supplied by
/// the caller.
pub fn setup<R>(rng: &mut R, me: Address, count: u16, threshold: u16) -> Result<Secrets, Error>
where
    R: rand::RngCore + rand::CryptoRng,
{
    let identifier = participants::identifier(me);
    let encryption_key = EncryptionKey::generate(&mut *rng);
    let (secret_package, package) =
        dkg::part1(identifier, count, threshold, &mut *rng).err_unexpected()?;
    let proof_of_knowledge = *package.proof_of_knowledge();

    Ok(Secrets {
        encryption_key,
        secret_package,
        proof_of_knowledge,
    })
}

/// A verified public commitment from a participant.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VerifiedCommitment {
    encryption_public_key: EncryptionPublicKey,
    package: round1::Package,
}

/// Verifies a participant's public commitment by decoding its values and
/// checking its proof of knowledge.
///
/// These are applied to _both_ `me`, the validator itself, and all peers that
/// publish key generation commitments onchain; unlike [`generate_secret_shares`]
/// this does not require `me` to be a participant in the key generation.
pub fn verify_commitment(
    participant: Address,
    commitment: &bindings::KeyGenCommitment,
) -> Result<VerifiedCommitment, Error> {
    // Note that we do not check the length of the commitments, this is enforced
    // by the smart contract and any issues will be caught later and produce an
    // unexpected FROST error.
    let identifier = participants::identifier(participant);
    marshal::frost_commitment(commitment)
        .and_then(|(encryption_public_key, package)| {
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

/// FROST group commitments.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GroupCommitments {
    commitments: BTreeMap<Address, VerifiedCommitment>,
    group_commitment: keys::VerifiableSecretSharingCommitment,
    verifying_key: VerifyingKey,
}

impl GroupCommitments {
    /// Returns the underlying FROST group verifying key.
    pub(super) fn verifying_key(&self) -> &VerifyingKey {
        &self.verifying_key
    }
}

/// Derives the group's public key from every participant's verified
/// commitment.
pub fn group_commitments(
    commitments: BTreeMap<Address, VerifiedCommitment>,
) -> Result<GroupCommitments, Error> {
    let group_commitment = group_commitment(&commitments).err_unexpected()?;
    let verifying_key = VerifyingKey::from_commitment(&group_commitment).err_unexpected()?;

    Ok(GroupCommitments {
        commitments,
        group_commitment,
        verifying_key,
    })
}

/// A participant's own secret key-generation state after sharing. Persisted to
/// the secret store and consumed by [`finalize`].
///
/// Unlike [`Secrets`], this can be reconstructed using the generated secrets
/// from [`setup`] and onchain state, and therefore can be stored in the state
/// machine (and does not require a reorg-resistant storage).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SharingState {
    encryption_key: EncryptionKey,
    secret_package: round2::SecretPackage,
    peer_packages: BTreeMap<Identifier, round2::Package>,
    group_commitments: GroupCommitments,
}

impl SharingState {
    /// Returns the group key for the sharing state.
    pub fn group_commitments(&self) -> &GroupCommitments {
        &self.group_commitments
    }
}

/// Given every participant's verified commitment, produces the sharing state
/// to persist and the ECDH-encrypted secret shares for the peers, ordered by
/// ascending peer address.
pub fn generate_secret_shares(
    secrets: Secrets,
    commitments: BTreeMap<Address, VerifiedCommitment>,
) -> Result<(SharingState, bindings::KeyGenSecretShare), Error> {
    let (round1_peer_packages, round1_me_package) = peer_packages(
        secrets.secret_package.identifier(),
        &commitments,
        |commitment| commitment.package.clone(),
    )?;

    // By construction, the second round of DKG should have no participant
    // culprits (the contract has checked coefficient lengths, and the
    // commitments' proofs of knowledge have been verified). The only way we
    // encounter an error here is if the caller were to mix secrets and
    // commitments from different DKG ceremonies.
    dkg::part2(secrets.secret_package.clone(), &round1_peer_packages)
        .and_then(|(secret_package, round2_packages)| {
            // Ensure that the `me` commitment is also valid, the FROST library
            // doesn't check this by default.
            if round1_me_package.commitment() != secret_package.commitment() {
                return Err(frost_secp256k1::Error::IncorrectCommitment);
            }

            // Build the verifying share (i.e. the participant's "public key")
            // from the commitments.
            let group_commitment = group_commitment(&commitments)?;
            let verifying_share = keys::VerifyingShare::from_commitment(
                *secrets.secret_package.identifier(),
                &group_commitment,
            );
            let verifying_key = VerifyingKey::from_commitment(&group_commitment)?;

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

            // Build the sharing state that we will need to verify secret shares
            // and finalize our key share.
            let sharing_state = SharingState {
                encryption_key: secrets.encryption_key,
                secret_package,
                peer_packages: round2_packages,
                group_commitments: GroupCommitments {
                    commitments,
                    group_commitment,
                    verifying_key,
                },
            };

            Ok((sharing_state, share))
        })
        .err_unexpected()
}

fn group_commitment(
    commitments: &BTreeMap<Address, VerifiedCommitment>,
) -> Result<keys::VerifiableSecretSharingCommitment, frost_secp256k1::Error> {
    frost_core::keys::sum_commitments(
        &commitments
            .values()
            .map(|verified| verified.package.commitment())
            .collect::<Vec<_>>(),
    )
}

/// A verified public key share.
///
/// Public key shares are verified against the commitments made by the
/// participant at the start of the keygen ceremony.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PublicKeyShare {
    verifying_share: keys::VerifyingShare,
}

/// A participant's encrypted key shares, in canonical publishing order.
pub struct EncryptedSecretShares {
    shares: Vec<[u8; 32]>,
}

/// Verifies the public portion of a participant's secret-share broadcast by
/// checking that its advertised public key share matches the group
/// commitments, and decodes its encrypted shares for participant-specific
/// verification.
///
/// These are applied to _both_ `me`, the validator itself, and all peers that
/// publish key generation secret shares onchain.
pub fn verify_secret_share(
    group_commitments: &GroupCommitments,
    participant: Address,
    share: &bindings::KeyGenSecretShare,
) -> Result<(PublicKeyShare, EncryptedSecretShares), Error> {
    if !group_commitments.commitments.contains_key(&participant) {
        return Err(frost_secp256k1::Error::UnknownIdentifier).err_unexpected();
    }

    let identifier = participants::identifier(participant);
    let public_key = marshal::frost_point(&share.y)
        .and_then(|y| {
            let verifying_share = keys::VerifyingShare::from_commitment(
                identifier,
                &group_commitments.group_commitment,
            );
            if y != verifying_share.to_element() {
                return Err(frost_secp256k1::Error::MalformedVerifyingKey);
            }

            Ok(PublicKeyShare { verifying_share })
        })
        .err_with_culprit(participant)?;

    // Note that we explicitly do not check the length of the encrypted shares,
    // as this is done by the contract. Any error in the length will trigger an
    // unexpected FROST error at finalization.
    let encrypted_shares = EncryptedSecretShares {
        shares: share.f.iter().map(U256::to_be_bytes).collect(),
    };

    Ok((public_key, encrypted_shares))
}

/// A validated signing share from a participant.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VerifiedShare {
    package: round2::Package,
}

impl VerifiedShare {
    /// Creates a new verified share from a plaintext secret share `secret_share`
    /// from `peer` against `commitment`, registered to `me`.
    fn new(
        me: Identifier,
        peer: Address,
        commitment: keys::VerifiableSecretSharingCommitment,
        secret_share: [u8; 32],
    ) -> Result<VerifiedShare, Error> {
        let package = marshal::frost_scalar(&U256::from_be_bytes(secret_share))
            .and_then(|secret_share| {
                // Pre-verify the share to make sure it matches the commitment from
                // the peer; this allows us to complain right away in case an
                // invalid share was provided.
                let signing_share = keys::SigningShare::new(secret_share);
                let _ = keys::SecretShare::new(me, signing_share, commitment).verify()?;

                Ok(round2::Package::new(signing_share))
            })
            .err_with_culprit(peer)?;

        Ok(VerifiedShare { package })
    }
}

/// Decrypts this validator's encrypted share from a participant and verifies
/// that it matches that participant's commitment.
///
/// These are applied to _both_ `me`, the validator itself, and all peers that
/// publish key generation secret shares onchain.
pub fn verify_encrypted_secret_share(
    sharing_state: &SharingState,
    participant: Address,
    encrypted_shares: EncryptedSecretShares,
) -> Result<VerifiedShare, Error> {
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
            .group_commitments
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
                    .group_commitments
                    .commitments
                    .keys()
                    .filter(|address| **address != participant)
                    .position(|address| {
                        participants::identifier(*address)
                            == *sharing_state.secret_package.identifier()
                    })
                    .ok_or(frost_secp256k1::Error::UnknownIdentifier)?;
                let encrypted_share = encrypted_shares
                    .shares
                    .get(index)
                    .ok_or(frost_core::Error::IncorrectNumberOfShares)?;
                let secret_share = sharing_state
                    .encryption_key
                    .ecdh(encryption_public_key, *encrypted_share);

                Ok((commitment, secret_share))
            })
            .err_unexpected()?
    };

    let me = *sharing_state.secret_package.identifier();
    VerifiedShare::new(me, participant, commitment, secret_share)
}

/// Verifies a secret share publicly revealed by `accused`, in response to a
/// complaint raised by `plaintiff`, against `accused`'s commitment.
///
/// Unlike [`verify_encrypted_secret_share`], this takes the plaintext scalar
/// directly (as published onchain in response to a complaint) rather than
/// this validator's own ECDH-encrypted broadcast.
pub fn verify_revealed_secret_share(
    group_commitments: &GroupCommitments,
    plaintiff: Address,
    accused: Address,
    secret_share: U256,
) -> Result<VerifiedShare, Error> {
    let plaintiff = participants::identifier(plaintiff);
    let commitment = group_commitments
        .commitments
        .get(&accused)
        .ok_or(frost_secp256k1::Error::UnknownIdentifier)
        .err_unexpected()?
        .package
        .commitment()
        .clone();

    VerifiedShare::new(plaintiff, accused, commitment, secret_share.to_be_bytes())
}

/// Reveals this validator's own plaintext secret share computed for `peer`,
/// published onchain in response to a complaint so that anyone can verify it
/// against this validator's committed polynomial.
pub fn reveal_secret_share(sharing_state: &SharingState, peer: Address) -> Result<U256, Error> {
    let identifier = participants::identifier(peer);
    sharing_state
        .peer_packages
        .get(&identifier)
        .map(|package| marshal::solidity_scalar(&package.signing_share().to_scalar()))
        .ok_or(frost_secp256k1::Error::UnknownIdentifier)
        .err_unexpected()
}

/// A participant's share of the group signing key: its secret signing share,
/// verifying share, and the group verifying key. The final result of DKG,
/// persisted to the secret store and used to sign.
#[derive(Clone, Debug, Deserialize, Serialize)]
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
        &sharing_state.group_commitments.commitments,
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
