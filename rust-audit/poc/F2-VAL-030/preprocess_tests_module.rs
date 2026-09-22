
#[cfg(test)]
mod qa2_val_b {
    //! QA2-VAL-B proof-of-concept for F2-VAL-030 / F2-VAL-062 (temporary;
    //! reverted after the run). A pure `NonceState` simulation of the phantom
    //! chunk reservation that a failed `NonceTree` effect strands, and the
    //! `expected_chunk`-vs-contract desync cascade that follows.
    use super::*;

    /// Mirrors `FROSTNonceCommitmentSet.commit`: given the contract's current
    /// `next` and a signing `sequence`, returns the chunk the contract assigns
    /// and its advanced `next`.
    fn contract_commit(next: u64, sequence: u64) -> (u64, u64) {
        let (mut chunk, _offset) = preprocess::decode_sequence(sequence);
        if next > chunk {
            chunk = next;
        }
        (chunk, chunk + 1)
    }

    #[test]
    fn f2_val_030_failed_nonce_tree_strands_a_phantom_reservation_and_cascades() {
        let r0 = B256::repeat_byte(0xa0);
        let r1 = B256::repeat_byte(0xa1);

        // Step 1: healthy state. Chunk 0 is linked; next_sequence is deep in it.
        // The contract has committed exactly chunk 0, so its `next` is 1.
        let mut n = NonceState {
            next_sequence: 925,
            chunks: BTreeMap::from([(0u64, Some(r0))]),
        };
        let mut contract_next = 1u64;
        assert_eq!(n.available(), 99, "1024 - 925 = 99 remaining in chunk 0");

        // Step 2: available() < 100 so a top-up reserves the next chunk.
        // expected_chunk = max(last_key + 1, seq_chunk) = max(1, 0) = 1.
        assert_eq!(n.reserve_chunk(), Some(1));
        assert_eq!(n.chunks.get(&1), Some(&None), "chunk 1 is a None reservation");

        // Step 3: the Effect::NonceTree FAILS (cold generator / DB error), so
        // nothing links or removes the reservation. `available()` now counts
        // the phantom None chunk as a full 1024, so NO further top-up fires.
        assert!(n.available() >= 100, "phantom reservation suppresses further top-ups");
        assert_eq!(n.available(), 99 + 1024);

        // Step 4: drain chunk 0, then every Sign whose sequence lands in the
        // phantom chunk 1 is dropped (observe -> None -> handle_sign drop arm).
        assert!(n.observe(1023).is_some(), "last offset of the linked chunk 0 still serves");
        assert!(
            n.observe(1024).is_none(),
            "PHANTOM DROP: first offset of the reserved-but-unfilled chunk 1 yields no nonce"
        );

        // Step 5: advance to offset 925 of chunk 1. available() re-crosses the
        // threshold and a top-up reserves chunk 2 -- but the contract, whose
        // `next` is still 1 (chunk 1 was never really committed), assigns
        // chunk 1. The local reservation is one chunk AHEAD of the contract.
        assert!(n.observe(1948).is_none(), "offsets 1..924 of chunk 1 are all dropped");
        assert_eq!(n.available(), 99, "1024 - 925 = 99 remaining in chunk 1");
        let local_reserved = n.reserve_chunk();
        let (contract_chunk, new_next) = contract_commit(contract_next, 1949);
        contract_next = new_next;
        assert_eq!(local_reserved, Some(2), "validator reserves chunk 2");
        assert_eq!(contract_chunk, 1, "the contract assigns chunk 1");
        assert_ne!(
            local_reserved,
            Some(contract_chunk),
            "DESYNC: the local reservation index does not match the contract's assignment"
        );

        // Step 6: the resulting Preprocess links the contract's chunk 1, so the
        // phantom moves to chunk 2 and the pattern repeats: offsets 925..1023
        // serve, offsets 0..924 of the next chunk are dropped, indefinitely.
        n.link(contract_chunk, r1);
        assert_eq!(n.chunks.get(&1), Some(&Some(r1)));
        assert_eq!(n.chunks.get(&2), Some(&None), "chunk 2 remains a phantom reservation");
        assert!(n.observe(1949).is_some(), "offset 925 of chunk 1 now serves");
        assert!(n.observe(2047).is_some(), "up through offset 1023 of chunk 1 serves");
        assert!(
            n.observe(2048).is_none(),
            "CASCADE: offset 0 of chunk 2 is dropped, exactly as chunk 1 was"
        );
        let _ = contract_next;
    }
}
