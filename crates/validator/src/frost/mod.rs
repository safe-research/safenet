//! A thin Safenet-specific layer over the ZCash Foundation FROST crates.
//!
//! Safenet uses the standard RFC 9591 FROST(secp256k1, SHA-256) ciphersuite.
//!
//! This module provides an interface that is compatible with the onchain
//! FROST coordinator contract, internally managing the marshalling between
//! [`frost-secp256k1`] values and their Solidity ABI representations.

#![expect(dead_code)]

pub mod ecdh;
pub mod error;
pub mod keygen;
mod marshal;
mod participants;

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;
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
    }
}
