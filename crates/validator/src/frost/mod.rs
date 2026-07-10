//! A thin Safenet-specific layer over the ZCash Foundation FROST crates.
//!
//! Safenet uses the standard RFC 9591 FROST(secp256k1, SHA-256) ciphersuite.
//!
//! This module provides an interface that is compatible with the onchain
//! FROST coordinator contract, internally managing the marshalling between
//! [`frost-secp256k1`] values and their Solidity ABI representations.

#![cfg_attr(not(test), expect(dead_code))]

pub mod ecdh;
pub mod error;
pub mod keygen;
mod marshal;
pub mod nonces;
mod participants;
pub mod signing;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merkle::MerkleRoot;
    use alloy::primitives::{address, keccak256};
    use std::collections::BTreeMap;

    #[test]
    fn ceremony() {
        let mut rng = rand::thread_rng();
        let participants = [
            address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
            address!("70997970C51812dc3A010C7d01b50e0d17dc79C8"),
            address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"),
        ];
        let count = participants.len() as u16;
        let threshold = 2;

        let mut round1 = BTreeMap::new();
        for participant in participants {
            let r1 = keygen::generate_round1(&mut rng, participant, count, threshold).unwrap();
            round1.insert(participant, r1);
        }

        let commitments = participants
            .into_iter()
            .map(|participant| {
                let commitment = keygen::verify_round1_commitment(
                    threshold,
                    participant,
                    &round1[&participant].commitment,
                )
                .unwrap();
                (participant, commitment)
            })
            .collect();

        let mut round2 = BTreeMap::new();
        for participant in participants {
            let r1 = &round1[&participant];
            let r2 = keygen::generate_round2(&r1.encryption_key, &r1.secret_package, &commitments)
                .unwrap();
            round2.insert(participant, r2);
        }

        let shares = participants
            .into_iter()
            .map(|me| {
                let r1 = &round1[&me];
                let r2 = &round2[&me];
                let shares = participants
                    .into_iter()
                    .filter(|peer| *peer != me)
                    .map(|peer| {
                        let share = &round2[&peer].share;
                        let share = keygen::verify_round2_share(
                            &r1.encryption_key,
                            &r2.secret_package,
                            &r2.public_key_package,
                            &commitments,
                            peer,
                            share,
                        )
                        .unwrap();
                        (peer, share)
                    })
                    .collect();
                (me, shares)
            })
            .collect::<BTreeMap<_, _>>();

        let mut round3 = BTreeMap::new();
        for participant in participants {
            let r2 = &round2[&participant];
            let r3 =
                keygen::generate_round3(&r2.secret_package, &commitments, &shares[&participant])
                    .unwrap();
            round3.insert(participant, r3);
        }

        let group_key = round3
            .values()
            .next()
            .unwrap()
            .public_key_package
            .verifying_key();
        for r3 in round3.values() {
            assert_eq!(group_key, r3.public_key_package.verifying_key());
        }

        // Signing ceremony: a threshold set of signers jointly signs a message,
        // each contributing a signature share and the signer-set selection.
        let message = keccak256("Hello, Safenet!");
        let signers = [participants[0], participants[2]];

        // Every signer preprocesses a nonce chunk, reveals its first nonce and
        // verifies each signer's revealed commitment.
        let mut secret_nonces = BTreeMap::new();
        let mut revealed_nonces = BTreeMap::new();
        for signer in signers {
            let key_package = &round3[&signer].key_package;
            let chunk =
                nonces::NonceChunk::with_size(1, key_package.signing_share(), &mut rng).unwrap();
            let nonces = chunk.nonces.into_iter().next().unwrap();
            let (sign_nonces, proof) = nonces.reveal();

            // Verify the Merkle proof as is done on the smart contract. This
            // is not expected to be enfoced by the clients, but added here for
            // testing.
            assert!(
                chunk
                    .commitment
                    .verify(nonces::nonces_leaf(0, &sign_nonces), proof)
            );

            secret_nonces.insert(signer, nonces);
            revealed_nonces.insert(signer, sign_nonces);
        }

        // Each signer independently produces its signature share.
        let revealed = revealed_nonces
            .iter()
            .map(|(participant, nonces)| {
                let commitment = signing::verify_revealed_nonces(*participant, nonces).unwrap();
                (*participant, commitment)
            })
            .collect();
        let signatures = signers
            .into_iter()
            .map(|signer| {
                let signature = signing::signature_share(
                    &round3[&signer].key_package,
                    &secret_nonces[&signer],
                    &revealed,
                    &message,
                )
                .unwrap();
                (signer, signature)
            })
            .collect::<BTreeMap<_, _>>();

        // Every signer reconstructs the identical selection: the same group
        // commitment `r` and signer-set root.
        let selection = &signatures[&signers[0]].selection;
        for (address, signature) in &signatures {
            assert_eq!(signature.selection.root, selection.root);
            assert_eq!(signature.selection.r.x, selection.r.x);
            assert_eq!(signature.selection.r.y, selection.r.y);

            // Verify the Merkle proof as is done on the smart contract. This
            // is not expected to be enforced by the clients, but added here for
            // testing.
            assert!(MerkleRoot::from(selection.root).verify(
                signing::signer_leaf(
                    address,
                    &signature.share.r,
                    &signature.share.l,
                    &signature.selection.r
                ),
                &signature.proof,
            ));
        }

        // Aggregate the shares to verify the constructed signature using only
        // data that is available onchain.
        {
            let signing_commitments = revealed_nonces
                .iter()
                .map(|(address, nonces)| {
                    (
                        participants::identifier(*address),
                        marshal::frost_signing_commitments(nonces).unwrap(),
                    )
                })
                .collect();
            let signing_package =
                frost_secp256k1::SigningPackage::new(signing_commitments, message.as_slice());
            let signature_shares = signatures
                .iter()
                .map(|(address, signature)| {
                    (
                        participants::identifier(*address),
                        frost_secp256k1::round2::SignatureShare::deserialize(
                            &signature.share.z.to_be_bytes::<32>(),
                        )
                        .unwrap(),
                    )
                })
                .collect();
            let verifying_shares = round3
                .iter()
                .map(|(address, r3)| {
                    (
                        participants::identifier(*address),
                        *r3.key_package.verifying_share(),
                    )
                })
                .collect();
            let public_key_package = frost_secp256k1::keys::PublicKeyPackage::new(
                verifying_shares,
                *group_key,
                Some(threshold),
            );

            let signature = frost_secp256k1::aggregate(
                &signing_package,
                &signature_shares,
                &public_key_package,
            )
            .unwrap();

            // Check that the FROST signature is valid for the group.
            public_key_package
                .verifying_key()
                .verify(message.as_slice(), &signature)
                .unwrap();

            // Check that the selection's group commitment is the sum of the
            // commitment shares, and that it matches the aggregate signature's
            // calculated group commitment; this is specific to Safenet's FROST
            // setup and is not checked as part of regular FROST operations.
            for group_commitment in [
                // group commitment from the sum of commitment shares.
                signatures
                    .values()
                    .map(|signature| marshal::frost_point(&signature.share.r).unwrap())
                    .sum::<k256::ProjectivePoint>(),
                // group commitment from the signer selection.
                marshal::frost_point(&selection.r).unwrap(),
            ] {
                assert_eq!(group_commitment, *signature.R());
            }
        }
    }
}
