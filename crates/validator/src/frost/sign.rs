//! Signature-share creation for a signing ceremony.
//!
//! This module wraps the [`frost_secp256k1`] crate with the Safenet-specific
//! signer-set selection.

use super::{
    error::{Culprit as _, Error},
    keygen::KeyShare,
    marshal, participants,
    preprocess::Nonces,
};
use crate::{bindings, merkle::MerkleTree};
use alloy::primitives::{Address, B256, U256, keccak256};
use frost_secp256k1::{Secp256K1Sha256, SigningPackage, round1, round2};
use k256::elliptic_curve::PrimeField as _;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A validated revealed nonce commitment from a signer.
///
/// Revealing happens onchain (`signRevealNonces`), so a peer's `d`/`e` points
/// have already been merkle-checked against their preprocessing commitment;
/// this additionally attributes malformed or identity point to a misbehaving
/// participant.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RevealedNonces {
    commitments: round1::SigningCommitments,
}

/// Verifies a signer's revealed nonce commitments.
///
/// This just verifies that the nonce commitments are valid values (i.e. they
/// are non-identity points on the secp256k1 curve). We do not verify the
/// inclusion proof that the nonces were part of a nonces chunk commitment
/// submitted during the preprocessing phase (this is verified by the FROST
/// coordinator contract onchain).
pub fn verify_revealed_nonces(
    participant: Address,
    nonces: &bindings::SignNonces,
) -> Result<RevealedNonces, Error> {
    let commitments = marshal::frost_signing_commitments(nonces).err_with_culprit(participant)?;
    Ok(RevealedNonces { commitments })
}

/// The onchain artifacts of a single participant's `signShare` submission.
pub struct SignatureShare {
    /// The signing selection: the group commitment `r` and the signer-set root.
    pub selection: bindings::SignSelection,
    /// This participant's signature share `(r, z, l)`.
    pub share: bindings::SignatureShare,
    /// The merkle proof of this participant's membership in the selection.
    pub proof: Vec<B256>,
}

/// Produces a signature share to be submitted onchain.
///
/// Builds the signer-set selection from every signer's verified revealed nonce
/// commitments then consumes the secret signing nonce to produce a signature
/// share for the specified key package.
pub fn signature_share(
    key_share: &KeyShare,
    nonces: Nonces,
    revealed: &BTreeMap<Address, RevealedNonces>,
    message: &B256,
) -> Result<SignatureShare, Error> {
    let key_package = key_share.as_key_package();
    let group_public_key = key_package.verifying_key();

    let commitments = revealed
        .iter()
        .map(|(address, revealed)| {
            let identifier = participants::identifier(*address);
            (identifier, revealed.commitments)
        })
        .collect();
    let signing_package = SigningPackage::new(commitments, message.as_slice());

    // The signing package is built from pre-verified revealed nonces. This
    // means that any error here is the fault of the caller (for example, by
    // mixing revealed nonces, signing nonces or key packages from different
    // signing ceremonies). There is, therefore, no culprit.
    round2::sign(&signing_package, nonces.signing_nonces(), key_package)
        .and_then(|signature_share| {
            // Each signer's commitment share and Lagrange coefficient are
            // public, so every participant reconstructs the identical signer
            // selection, ordered by FROST identifier. Ordering is guaranteed
            // as the `signers` are stored in a `BTreeMap` which iterates in
            // ascending order on its keys.
            let binding_factors =
                frost_core::compute_binding_factor_list(&signing_package, group_public_key, &[])?;
            let signers = revealed
                .iter()
                .map(|(address, revealed)| {
                    let identifier = participants::identifier(*address);
                    let binding_factor = binding_factors
                        .get(&identifier)
                        .ok_or(frost_secp256k1::Error::UnknownIdentifier)?;
                    let interpolating_value =
                        frost_core::derive_interpolating_value(&identifier, &signing_package)?;
                    let commitment_share = revealed.commitments.hiding().value()
                        + revealed.commitments.binding().value()
                            * binding_factor_to_scalar(binding_factor);

                    let r = marshal::solidity_point(&commitment_share);
                    let l = marshal::solidity_scalar(&interpolating_value);

                    Ok((identifier, (*address, r, l)))
                })
                .collect::<Result<BTreeMap<_, _>, frost_secp256k1::Error>>()?;

            // Now that we have computed the commitment shares and the lagrange
            // coefficients for each of the signers in the selection, we can
            // derive the group commitment and the selection root.
            let group_commitment =
                frost_core::compute_group_commitment(&signing_package, &binding_factors)?;
            let group_r = marshal::solidity_point(&group_commitment.to_element());
            let leaves = signers
                .iter()
                .map(|(_, (address, r, l))| signer_leaf(address, r, l, &group_r))
                .collect();
            let tree = MerkleTree::build(leaves);
            let selection = bindings::SignSelection {
                r: group_r,
                root: tree.root().0,
            };

            // Extract the signature share values that are needed onchain along
            // with the selection inclusion Merkle proof.
            let (_, r, l) = signers
                .get(key_package.identifier())
                .ok_or(frost_secp256k1::Error::UnknownIdentifier)?
                .clone();
            let z = marshal::solidity_signing_share(&signature_share);
            let share = bindings::SignatureShare { r, z, l };
            let index = signers.range(..key_package.identifier()).count();
            let proof = tree.proof(index);

            Ok(SignatureShare {
                selection,
                share,
                proof,
            })
        })
        .err_unexpected()
}

/// Extract the binding factor as a scalar.
fn binding_factor_to_scalar(
    binding_factor: &frost_core::BindingFactor<Secp256K1Sha256>,
) -> k256::Scalar {
    // Unfortunately, the `frost_core` crate (even with the internals feature)
    // does not provide a mechanism for extracting the raw binding factor scalar
    // value that we need. Roundtrip it through the serialization mechanism.
    k256::Scalar::from_repr(
        <[u8; 32]>::try_from(binding_factor.serialize())
            .expect("binding factor always serializes the correct number of bytes")
            .into(),
    )
    .expect("binding factor is always a valid scalar")
}

/// The leaf hash for a participant in the signer-set selection tree. Defined as
/// `keccak256(participant, r.x, r.y, l, group_r.x, group_r.y)`, matching
/// `FROSTSignatureShares._hash` (the participant occupies a full 32-byte slot).
pub(super) fn signer_leaf(
    participant: &Address,
    r: &bindings::Point,
    l: &U256,
    group_r: &bindings::Point,
) -> B256 {
    let mut buf = [0u8; 192];
    buf[12..32].copy_from_slice(participant.as_slice());
    buf[32..64].copy_from_slice(&r.x.to_be_bytes::<32>());
    buf[64..96].copy_from_slice(&r.y.to_be_bytes::<32>());
    buf[96..128].copy_from_slice(&l.to_be_bytes::<32>());
    buf[128..160].copy_from_slice(&group_r.x.to_be_bytes::<32>());
    buf[160..192].copy_from_slice(&group_r.y.to_be_bytes::<32>());
    keccak256(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{address, b256};

    #[test]
    fn computes_the_canonical_signer_leaf_value() {
        let participant = address!("1111111111111111111111111111111111111111");
        let r = bindings::Point {
            x: U256::from(2),
            y: U256::from(3),
        };
        let l = U256::from(4);
        let group_r = bindings::Point {
            x: U256::from(5),
            y: U256::from(6),
        };

        assert_eq!(
            signer_leaf(&participant, &r, &l, &group_r),
            b256!("da01939303f39ff14b730b8023a7902cfe7dc335052430e8b1efb957a909bf34")
        );
    }
}
