//! A thin Safenet-specific layer over the ZCash Foundation FROST crates.
//!
//! Safenet uses the standard RFC 9591 FROST(secp256k1, SHA-256) ciphersuite.
//!
//! This module provides an interface that is compatible with the onchain
//! FROST coordinator contract, internally managing the marshalling between
//! [`frost_secp256k1`] values and their Solidity ABI representations.

pub mod ecdh;
pub mod error;
pub mod keygen;
mod marshal;
mod participants;
pub mod preprocess;
mod serialization;
pub mod sign;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merkle::MerkleRoot;
    use alloy::primitives::{Address, address, keccak256};
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

        // --- KEY GENERATION ---

        // First do the key generation setup, which generates some random
        // secrets that need to be persisted, as well as some public commitments
        // that are submitted onchain.
        let mut secrets = BTreeMap::new();
        let mut commitments = BTreeMap::new();
        for participant in participants {
            let participant_secrets =
                keygen::setup(&mut rng, participant, count, threshold).unwrap();
            commitments.insert(participant, participant_secrets.commitment());
            secrets.insert(participant, participant_secrets);
        }

        // Each participant verifies the commitments they observe onchain.
        let verified_commitments = participants
            .into_iter()
            .map(|participant| {
                let commitment =
                    keygen::verify_commitment(participant, &commitments[&participant]).unwrap();
                (participant, commitment)
            })
            .collect::<BTreeMap<_, _>>();

        // Each participant uses their random secrets to proceed to the next
        // stage and generate encrypted secret shares that published onchain.
        let mut sharing_states = BTreeMap::new();
        let mut shares = BTreeMap::new();
        for (participant, secrets) in secrets {
            let (sharing_state, share) =
                keygen::generate_secret_shares(secrets, verified_commitments.clone()).unwrap();
            sharing_states.insert(participant, sharing_state);
            shares.insert(participant, share);
        }

        // Once the secret sharing is complete, the key generation process can
        // finalize, producing a key share for the group.
        let mut key_shares = BTreeMap::new();
        for (participant, sharing_state) in sharing_states {
            let verified_shares = shares
                .iter()
                .map(|(peer, share)| {
                    let encrypted_shares = keygen::verify_secret_share(
                        sharing_state.group_commitments(),
                        *peer,
                        share,
                    )
                    .unwrap();
                    let verified_share = keygen::verify_encrypted_secret_share(
                        &sharing_state,
                        *peer,
                        encrypted_shares,
                    )
                    .unwrap();

                    (*peer, verified_share)
                })
                .collect();
            let key_share = keygen::finalize(sharing_state, verified_shares).unwrap();
            key_shares.insert(participant, key_share);
        }

        // Here, all participants should share the same group verifying key.
        // Additionally, the group key should equal to the sum of all the
        // publicly posted commitments C_0.
        let group_commitments = keygen::group_commitments(verified_commitments).unwrap();
        for key_share in key_shares.values() {
            assert_eq!(
                group_commitments.verifying_key(),
                key_share.as_key_package().verifying_key()
            );
        }
        assert_eq!(
            group_commitments.verifying_key().to_element(),
            commitments
                .values()
                .map(|commitments| marshal::frost_point(&commitments.c[0]).unwrap())
                .sum::<k256::ProjectivePoint>()
        );

        // --- SIGNING CEREMONY ---

        // A threshold set of signers jointly signs a message, each contributing
        // a signature share and the signer-set selection.
        let message = keccak256("Hello, Safenet!");
        let signers = [participants[0], participants[2]];

        // Every signer preprocesses a nonce chunk, reveals its first nonce and
        // verifies each signer's revealed commitment.
        let mut secret_nonces = BTreeMap::new();
        let mut revealed_nonces = BTreeMap::new();
        for signer in signers {
            let key_share = &key_shares[&signer];
            let chunk = preprocess::NonceChunk::with_size(1, key_share, &mut rng).unwrap();
            let nonces = chunk.nonces.into_iter().next().unwrap();
            let (sign_nonces, proof) = nonces.reveal();

            // Verify the Merkle proof as is done on the smart contract. This
            // is not expected to be enforced by the clients, but added here for
            // testing.
            assert!(
                chunk
                    .commitment
                    .verify(preprocess::nonces_leaf(0, &sign_nonces), proof)
            );

            secret_nonces.insert(signer, nonces);
            revealed_nonces.insert(signer, sign_nonces);
        }

        // Each signer independently produces its signature share.
        let mut signatures = BTreeMap::new();
        for signer in signers {
            let revealed = revealed_nonces
                .iter()
                .map(|(participant, nonces)| {
                    let commitment = sign::verify_revealed_nonces(*participant, nonces).unwrap();
                    (*participant, commitment)
                })
                .collect();
            let signature = sign::signature_share(
                &key_shares[&signer],
                secret_nonces.remove(&signer).unwrap(),
                &revealed,
                &message,
            )
            .unwrap();
            signatures.insert(signer, signature);
        }

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
            assert!(MerkleRoot(selection.root).verify(
                sign::signer_leaf(
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
            let verifying_shares = key_shares
                .iter()
                .map(|(address, key_share)| {
                    (
                        participants::identifier(*address),
                        *key_share.as_key_package().verifying_share(),
                    )
                })
                .collect();
            let public_key_package = frost_secp256k1::keys::PublicKeyPackage::new(
                verifying_shares,
                *group_commitments.verifying_key(),
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

    fn commitments() -> [(Address, crate::bindings::KeyGenCommitment); 3] {
        let mut rng = rand::thread_rng();
        [
            address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
            address!("70997970C51812dc3A010C7d01b50e0d17dc79C8"),
            address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"),
        ]
        .map(|participant| {
            let secrets = keygen::setup(&mut rng, participant, 3, 2).unwrap();
            (participant, secrets.commitment())
        })
    }

    fn group_commitments(
        commitments: &[(Address, crate::bindings::KeyGenCommitment)],
    ) -> keygen::GroupCommitments {
        let verified = commitments
            .iter()
            .map(|(participant, commitment)| {
                let verified = keygen::verify_commitment(*participant, commitment).unwrap();
                (*participant, verified)
            })
            .collect();
        keygen::group_commitments(verified).unwrap()
    }

    fn assert_roundtrip<T>(value: &T)
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        let json = serde_json::to_string(value).unwrap();
        let decoded = serde_json::from_str::<T>(&json).unwrap();
        assert_eq!(serde_json::to_string(&decoded).unwrap(), json);
    }

    #[test]
    fn persists_cancelled_group_commitment_coefficients() {
        let mut commitments = commitments();

        // Only the constant term is bound by the proof of knowledge, so the
        // last participant to commit can cancel out any other coefficient of
        // the group commitment.
        let (honest, attacker) = commitments.split_at_mut(2);
        let sum = honest
            .iter()
            .map(|(_, commitment)| marshal::frost_point(&commitment.c[1]).unwrap())
            .sum::<k256::ProjectivePoint>();
        attacker[0].1.c[1] = marshal::solidity_point(&-sum);

        assert_roundtrip(&group_commitments(&commitments));
    }

    #[test]
    fn accepts_cancelled_public_key_shares() {
        let mut commitments = commitments();
        let victim = commitments[0].0;
        let x = participants::identifier(victim).to_scalar();

        // Similarly, the last participant to commit can cancel out the
        // victim's verifying share, which the victim then publishes. The victim
        // is honest, so its share must not be rejected.
        let (honest, attacker) = commitments.split_at_mut(2);
        let evaluation = honest
            .iter()
            .chain([&attacker[0]])
            .map(|(_, commitment)| marshal::frost_point(&commitment.c[0]).unwrap())
            .sum::<k256::ProjectivePoint>()
            + honest
                .iter()
                .map(|(_, commitment)| marshal::frost_point(&commitment.c[1]).unwrap() * x)
                .sum::<k256::ProjectivePoint>();
        attacker[0].1.c[1] = marshal::solidity_point(&(-evaluation * x.invert().unwrap()));

        let share = crate::bindings::KeyGenSecretShare {
            y: crate::bindings::Point::default(),
            f: vec![],
        };
        keygen::verify_secret_share(&group_commitments(&commitments), victim, &share).unwrap();
    }
}
