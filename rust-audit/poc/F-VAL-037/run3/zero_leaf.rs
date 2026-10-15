//! RUN 3 (rv-val, 2026-10-08): F-VAL-037 at `361aaf3`. Child module of
//! `crate::merkle`. A participants-style tree (unhashed left-padded address
//! leaves, as `consensus/group.rs:244-246` builds it) over three addresses has
//! an odd level, and `B256::ZERO`, the leaf of `address(0)`, has a valid proof.

use super::*;
use alloy::primitives::Address;

#[test]
fn zero_address_leaf_proves_against_an_odd_participants_tree() {
    let addresses = [Address::repeat_byte(0x11), Address::repeat_byte(0x22), Address::repeat_byte(0x33)];
    let tree = MerkleTree::build(addresses.iter().map(|a| a.into_word()).collect());
    let root = tree.root();
    for (i, a) in addresses.iter().enumerate() {
        assert!(root.verify(a.into_word(), &tree.proof(i)), "real member {i}");
    }
    let zero_proof = tree.proof(3);
    let zero_ok = root.verify(Address::ZERO.into_word(), &zero_proof);
    println!("3-leaf participants tree: proof for address(0) at padding index 3 verifies = {zero_ok}");
    assert!(zero_ok);

    let even = MerkleTree::build(addresses[..2].iter().map(|a| a.into_word()).collect());
    let even_ok = even.root().verify(Address::ZERO.into_word(), &even.proof(2));
    println!("2-leaf tree (no odd level): proof for address(0) verifies = {even_ok}");
}
