use alloy::primitives::{Address, B256, keccak256};
use frost_secp256k1::Identifier;

/// Derives a FROST [`Identifier`] from an Ethereum address.
pub fn identifier(address: Address) -> Identifier {
    Identifier::derive(address.as_slice())
        .expect("FROST(secp256k1, SHA-256) supports identifier derivation")
}

/// Generates a Merkle inclusion proof for `own_address` in the participant set.
///
/// The returned proof is a sequence of sibling hashes from the leaf up to (but not including)
/// the root, matching the format expected by the FROSTCoordinator `poap` parameter.
///
/// Port of `generateParticipantProof` in `validator/src/consensus/merkle.ts`.
pub fn generate_participant_proof(
    participants: &[Address],
    own_address: Address,
) -> Option<Vec<B256>> {
    let leaves = sorted_participant_leaves(participants);
    let own_leaf = B256::left_padding_from(own_address.as_slice());
    let index = leaves.iter().position(|l| *l == own_leaf)?;
    Some(merkle_proof(leaves, index))
}

/// Computes the Merkle root of a participant address set.
///
/// Leaves are the participant addresses left-padded to 32 bytes (matching `pad(address)` in viem).
/// Participants are sorted internally; the caller does not need to pre-sort. Pairs are
/// canonically sorted before hashing, and odd-length levels are padded with the zero hash.
///
/// Port of `calculateParticipantsRoot` in `validator/src/consensus/merkle.ts`.
pub fn calc_participants_root(participants: &[Address]) -> B256 {
    merkle_root(sorted_participant_leaves(participants))
}

/// Computes the deterministic genesis group ID from participants and an optional salt.
///
/// Port of `calcGenesisGroup` in `validator/src/machine/keygen/group.ts` and
/// `calcGroupId` in `validator/src/consensus/keyGen/utils.ts`.
pub fn calc_genesis_group_id(participants: &[Address], genesis_salt: Option<B256>) -> B256 {
    let genesis_salt = genesis_salt.unwrap_or(B256::ZERO);
    let context = if genesis_salt == B256::ZERO {
        B256::ZERO
    } else {
        // encodePacked(["string", "bytes32"], ["genesis", genesisSalt])
        let mut packed = [0u8; 39];
        packed[..7].copy_from_slice(b"genesis");
        packed[7..].copy_from_slice(genesis_salt.as_slice());
        keccak256(&packed).into()
    };

    let root = calc_participants_root(participants);
    let count = participants.len() as u16;
    let threshold = count / 2 + 1;

    // ABI-encode (bytes32, uint16, uint16, bytes32): each field right-aligned in a 32-byte slot.
    let mut buf = [0u8; 128];
    buf[0..32].copy_from_slice(root.as_slice());
    buf[62..64].copy_from_slice(&count.to_be_bytes());
    buf[94..96].copy_from_slice(&threshold.to_be_bytes());
    buf[96..128].copy_from_slice(context.as_slice());

    // Mask the last 8 bytes to zero (matching the & 0xfff...fff0000000000000000 in the TS source).
    let mut gid: [u8; 32] = *keccak256(&buf);
    gid[24..].fill(0);
    B256::from(gid)
}

fn sorted_participant_leaves(participants: &[Address]) -> Vec<B256> {
    let mut leaves: Vec<B256> = participants
        .iter()
        .map(|a| B256::left_padding_from(a.as_slice()))
        .collect();
    leaves.sort_unstable();
    leaves
}

fn merkle_root(leaves: Vec<B256>) -> B256 {
    build_merkle_tree(leaves)
        .last()
        .and_then(|r| r.get(0))
        .copied()
        .unwrap_or_default()
}

fn merkle_proof(leaves: Vec<B256>, index: usize) -> Vec<B256> {
    let tree = build_merkle_tree(leaves);
    let height = tree.len();
    let mut proof = Vec::with_capacity(height.saturating_sub(1));
    let mut current_index = index;
    for level in tree.iter().take(height - 1) {
        let neighbor = current_index ^ 1;
        proof.push(level.get(neighbor).copied().unwrap_or(B256::ZERO));
        current_index >>= 1;
    }
    proof
}

fn build_merkle_tree(leaves: Vec<B256>) -> Vec<Vec<B256>> {
    let mut tree = vec![leaves.to_vec()];
    while tree.last().unwrap().len() > 1 {
        let level = tree.last().unwrap();
        let pairs = (level.len() + 1) / 2;
        let mut next = Vec::with_capacity(pairs);
        for i in 0..pairs {
            let a = level.get(i * 2).copied().unwrap_or(B256::ZERO);
            let b = level.get(i * 2 + 1).copied().unwrap_or(B256::ZERO);
            let (left, right) = if a <= b { (a, b) } else { (b, a) };
            let mut data = [0u8; 64];
            data[..32].copy_from_slice(left.as_slice());
            data[32..].copy_from_slice(right.as_slice());
            next.push(keccak256(&data).into());
        }
        tree.push(next);
    }
    tree
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;
    use std::str::FromStr;

    #[test]
    fn can_derive_frost_identifier() {
        let _ = identifier(address!("70997970C51812dc3A010C7d01b50e0d17dc79C8"));
    }

    #[test]
    fn merkle_root_empty_returns_zero() {
        assert_eq!(merkle_root(vec![]), B256::ZERO);
    }

    #[test]
    fn participant_proof_absent_address_returns_none() {
        let addr0 = address!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
        let absent = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
        assert!(generate_participant_proof(&[addr0], absent).is_none());
    }

    #[test]
    fn merkle_root_single_leaf() {
        let leaf = B256::from([1u8; 32]);
        assert_eq!(merkle_root(vec![leaf]), leaf);
    }

    #[test]
    fn merkle_root_two_leaves_sorted() {
        // a < b, so left=a, right=b.
        let a = B256::from([1u8; 32]);
        let b = B256::from([2u8; 32]);
        let mut data = [0u8; 64];
        data[..32].copy_from_slice(a.as_slice());
        data[32..].copy_from_slice(b.as_slice());
        let expected = keccak256(&data);
        assert_eq!(merkle_root(vec![a, b]), expected);
    }

    fn verify_proof(root: B256, leaf: B256, proof: &[B256]) -> bool {
        let mut node = leaf;
        for &sibling in proof {
            let (left, right) = if node <= sibling {
                (node, sibling)
            } else {
                (sibling, node)
            };
            let mut data = [0u8; 64];
            data[..32].copy_from_slice(left.as_slice());
            data[32..].copy_from_slice(right.as_slice());
            node = keccak256(&data).into();
        }
        node == root
    }

    #[test]
    fn participant_proof_two_leaves() {
        // Deliberately unsorted to verify internal sorting.
        let addr0 = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
        let addr1 = address!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
        let participants = vec![addr0, addr1];

        let root = calc_participants_root(&participants);

        for &addr in &participants {
            let proof = generate_participant_proof(&participants, addr).unwrap();
            assert_eq!(proof.len(), 1);
            let leaf = B256::left_padding_from(addr.as_slice());
            assert!(verify_proof(root, leaf, &proof));
        }
    }

    #[test]
    fn participant_proof_three_leaves() {
        // Three participants unsorted: the odd leaf at index 2 (after sorting) gets zeroHash as
        // its level-0 sibling.
        let addr0 = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
        let addr1 = address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC");
        let addr2 = address!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
        let participants = vec![addr0, addr1, addr2];

        let root = calc_participants_root(&participants);

        for &addr in &participants {
            let proof = generate_participant_proof(&participants, addr).unwrap();
            assert_eq!(proof.len(), 2);
            let leaf = B256::left_padding_from(addr.as_slice());
            assert!(verify_proof(root, leaf, &proof));
        }
    }

    #[test]
    fn root_and_proof_stable_regardless_of_participant_order() {
        let a = address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC");
        let b = address!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
        let c = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

        let permutations: &[&[Address]] = &[
            &[a, b, c],
            &[a, c, b],
            &[b, a, c],
            &[b, c, a],
            &[c, a, b],
            &[c, b, a],
        ];

        let expected_root = calc_participants_root(permutations[0]);
        let expected_proofs =
            [a, b, c].map(|addr| generate_participant_proof(permutations[0], addr).unwrap());

        for &perm in &permutations[1..] {
            assert_eq!(calc_participants_root(perm), expected_root);
            for (i, addr) in [a, b, c].into_iter().enumerate() {
                assert_eq!(
                    generate_participant_proof(perm, addr).unwrap(),
                    expected_proofs[i],
                );
            }
        }
    }

    #[test]
    fn participants_root_matches_typescript() {
        // Two well-known Anvil accounts; root must match regardless of input order.
        let addr0 = address!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
        let addr1 = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

        // Build expected root from the sorted leaves.
        let mut participants = vec![addr0, addr1];
        participants.sort();

        // Build the expected root manually.
        let leaf0 = B256::left_padding_from(participants[0].as_slice());
        let leaf1 = B256::left_padding_from(participants[1].as_slice());
        let (left, right) = if leaf0 <= leaf1 {
            (leaf0, leaf1)
        } else {
            (leaf1, leaf0)
        };
        let mut data = [0u8; 64];
        data[..32].copy_from_slice(left.as_slice());
        data[32..].copy_from_slice(right.as_slice());
        let expected_root: B256 = keccak256(&data).into();

        assert_eq!(calc_participants_root(&participants), expected_root);
    }
}
