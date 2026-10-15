// F3-VAL-032 PoC (temporary): inserted into the existing `#[cfg(test)] mod tests`
// in crates/validator/src/secrets/store.rs (reuses its `store()`, `signing_nonces()`,
// `GROUP`, `ME`, `signature_id()`, `has_signing_nonces_row()` helpers).
//
// Command (from the rv-val worktree):
//   cargo test -p validator --bins collected_signature_id_is_given_a_fresh_pair -- --nocapture
// Result: test ... ok (1 passed).
//
// Reproduces finding claim 1 (the carrying claim): the module header promises
// "the first pair stored for a ceremony is the only one it ever gets" and that
// a used ceremony "is never given a fresh pair" (store.rs:18-22). That holds
// only while the row exists. Once the group's signing-nonce rows are COLLECTED
// (scheduled for deletion, then pruned), a replay that re-runs GenerateNonces
// for the SAME signature id inserts a FRESH, usable pair.

#[tokio::test]
async fn collected_signature_id_is_given_a_fresh_pair() {
    let store = store().await;
    let first = signing_nonces();

    // First pair stored and used.
    let first_commitments = store
        .store_signing_nonces(GROUP, signature_id(1), ME, first.clone())
        .await
        .unwrap()
        .unwrap();
    store.take_signing_nonces(signature_id(1)).await.unwrap().unwrap();

    // While the row exists, the documented guarantee holds: no fresh pair.
    assert!(
        store
            .store_signing_nonces(GROUP, signature_id(1), ME, signing_nonces())
            .await
            .unwrap()
            .is_none()
    );

    // Collect the group's rows: schedule (nothing retained) then prune.
    assert!(
        store
            .schedule_group_secrets_deletion(10, &BTreeMap::new())
            .await
            .unwrap()
    );
    let pruned = store.prune_scheduled_secrets(10).await.unwrap();
    assert_eq!(pruned.signing_nonces, 1, "the used row is collected");
    assert!(!has_signing_nonces_row(&store, signature_id(1)).await);

    // Replay GenerateNonces for the SAME used signature id: a fresh pair is
    // inserted and returned, against the module header's promise.
    let second = signing_nonces();
    let second_commitments = store
        .store_signing_nonces(GROUP, signature_id(1), ME, second.clone())
        .await
        .unwrap()
        .expect("a fresh pair is stored for the collected signature id");
    assert_ne!(
        first_commitments, second_commitments,
        "the collected ceremony got a different (fresh) pair"
    );
    // And it is usable: it can be taken.
    let taken = store.take_signing_nonces(signature_id(1)).await.unwrap().unwrap();
    assert_eq!(
        serde_json::to_string(&taken).unwrap(),
        serde_json::to_string(&second).unwrap(),
    );
}
