//! Sorted-pair Keccak-256 Merkle trees.

#![cfg_attr(not(test), allow(dead_code))]

use alloy::primitives::{B256, keccak256};
use serde::{Deserialize, Serialize};

/// A Merkle tree.
pub struct MerkleTree(Vec<Vec<B256>>);

impl MerkleTree {
    /// Builds a Merkle tree from a vector of leaves.
    ///
    /// It is the callers responsibility to define how the leaves are computed.
    pub fn build(leaves: Vec<B256>) -> Self {
        let mut tree = vec![leaves];
        while let Some(level) = tree.last()
            && level.len() > 1
        {
            let pairs = level.len().div_ceil(2);
            let next = (0..pairs)
                .map(|i| {
                    let a = level.get(i * 2).copied().unwrap_or(B256::ZERO);
                    let b = level.get(i * 2 + 1).copied().unwrap_or(B256::ZERO);
                    hash_pair(a, b)
                })
                .collect();
            tree.push(next);
        }
        Self(tree)
    }

    /// Returns the height of the Merkle tree.
    pub fn height(&self) -> usize {
        self.0.len()
    }

    /// Returns the root of the Merkle tree.
    pub fn root(&self) -> MerkleRoot {
        let digest = self
            .0
            .last()
            .and_then(|level| level.first())
            .copied()
            .unwrap_or_default();
        MerkleRoot(digest)
    }

    /// Generates a Merkle inclusion proof for the leaf at `index`: the sibling
    /// hashes from the leaf up to (but excluding) the root.
    pub fn proof(&self, index: usize) -> Vec<B256> {
        let len = self.height().saturating_sub(1);
        let mut current = index;
        let mut proof = Vec::with_capacity(len);
        for level in self.0.iter().take(len) {
            let sibling = current ^ 1;
            proof.push(level.get(sibling).copied().unwrap_or(B256::ZERO));
            current >>= 1;
        }
        proof
    }
}

/// A Merkle root that can be used to verify a proof.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(transparent)]
pub struct MerkleRoot(pub B256);

impl MerkleRoot {
    /// Verifies a Merkle proof for a leaf.
    pub fn verify(&self, leaf: B256, proof: &[B256]) -> bool {
        let mut node = leaf;
        for &sibling in proof {
            node = hash_pair(node, sibling);
        }
        self.0 == node
    }
}

impl PartialEq<B256> for MerkleRoot {
    fn eq(&self, other: &B256) -> bool {
        self.0 == *other
    }
}

/// Hashes a pair of nodes with canonical (ascending) ordering, matching
/// OpenZeppelin's commutative `MerkleProof` hashing.
fn hash_pair(a: B256, b: B256) -> B256 {
    let (left, right) = if a <= b { (a, b) } else { (b, a) };
    let mut data = [0u8; 64];
    data[..32].copy_from_slice(left.as_slice());
    data[32..].copy_from_slice(right.as_slice());
    keccak256(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::b256;

    #[test]
    fn merkle_root_empty_returns_zero() {
        assert_eq!(MerkleTree::build(vec![]).root(), B256::ZERO);
    }

    #[test]
    fn merkle_root_single_leaf() {
        let leaf = B256::from([1u8; 32]);
        assert_eq!(MerkleTree::build(vec![leaf]).root(), leaf);
    }

    #[test]
    fn verifies_a_proof() {
        let leaves = (1..=13).map(|i| B256::from([i; 32])).collect::<Vec<_>>();
        let tree = MerkleTree::build(leaves.clone());
        let root = tree.root();

        for (i, leaf) in leaves.into_iter().enumerate() {
            let proof = tree.proof(i);
            assert!(root.verify(leaf, &proof))
        }
    }

    #[test]
    fn generates_the_expected_merkle_root() {
        let root = MerkleTree::build(vec![
            b256!("0000000000000000000000000000000000000000000000000000000000000001"),
            b256!("0000000000000000000000000000000000000000000000000000000000000002"),
            b256!("0000000000000000000000000000000000000000000000000000000000000003"),
            b256!("0000000000000000000000000000000000000000000000000000000000000004"),
            b256!("0000000000000000000000000000000000000000000000000000000000000005"),
            B256::ZERO,
            B256::ZERO,
            B256::ZERO,
        ])
        .root();

        assert_eq!(
            root,
            b256!("37e58bc84afff4e1afade4140135583af3d6d3523a435e60cec5dc75ae3d7e8b")
        );
    }
}
