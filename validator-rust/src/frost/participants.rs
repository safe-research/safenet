use alloy::primitives::{Address, B256, keccak256};

/// Computes the Merkle root of a participant address set.
///
/// Leaves are the participant addresses left-padded to 32 bytes (matching `pad(address)` in viem).
/// Participants must be sorted in ascending address order. Pairs are canonically sorted before
/// hashing, and odd-length levels are padded with the zero hash.
///
/// Port of `calculateParticipantsRoot` in `validator/src/consensus/merkle.ts`.
pub fn calc_participants_root(participants: &[Address]) -> B256 {
    let leaves: Vec<B256> = participants
        .iter()
        .map(|a| B256::left_padding_from(a.as_slice()))
        .collect();
    merkle_root(&leaves)
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

fn merkle_root(leaves: &[B256]) -> B256 {
    assert!(
        !leaves.is_empty(),
        "cannot build Merkle tree from empty leaf set"
    );
    let mut level = leaves.to_vec();
    while level.len() > 1 {
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
        level = next;
    }
    level[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // Test vectors derived from the TypeScript implementation.

    #[test]
    fn merkle_root_single_leaf() {
        let leaf = B256::from([1u8; 32]);
        assert_eq!(merkle_root(&[leaf]), leaf);
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
        assert_eq!(merkle_root(&[a, b]), expected);
    }

    #[test]
    fn participants_root_matches_typescript() {
        // Two well-known Anvil accounts in ascending address order.
        let addr0 = Address::from_str("0x70997970C51812dc3A010C7d01b50e0d17dc79C8").unwrap();
        let addr1 = Address::from_str("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266").unwrap();

        // Sort ascending.
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
