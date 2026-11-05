// run 3 re-run (rv-core-a, 2026-10-08, audit commit 361aaf3): test 3 of ../poc_blocks.rs, unchanged
// apart from em dashes in comments and messages. Paste into the mod tests of crates/core/src/state/storage.rs.

/// The persistence side, isolated: `SnapshotStore` has nowhere to PUT a chain
/// identity even if the watcher wanted to check one.
///
/// PASTE THIS ONE INTO `crates/core/src/state/storage.rs`'s `mod tests`.
///
/// EXPECTED RESULT ON A HEALTHY SYSTEM: the schema carries a block hash (and,
/// per remediation option 4, a chain id and a watched-address digest), so this
/// query finds a column and the test passes.
///
/// EXPECTED RESULT ON THIS CHECKOUT: `snapshots` has exactly two columns,
/// `block_number` and `state` (`state/storage.rs:51-57`), so the assertion
/// fails. This is the root of the defect: even if `initialize` wanted to verify
/// its anchor, there is no stored identity to verify it against.
#[tokio::test]
async fn poc_f_core_001_snapshots_persist_no_chain_identity() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let store = SnapshotStore::<u64>::new(pool.clone()).await.unwrap();
    store.commit(900, &42).await.unwrap();

    let columns = sqlx::query_scalar::<_, String>("SELECT name FROM pragma_table_info('snapshots')")
        .fetch_all(&pool)
        .await
        .unwrap();

    // THE ASSERTION THAT MATTERS.
    assert!(
        columns.iter().any(|name| name.contains("hash")),
        "the snapshots table stores no block hash: columns are {columns:?}. \
         Nothing in the persisted state identifies the chain it was derived from, \
         so a resume cannot tell a canonical anchor from an orphaned one. The \
         same gap accepts a restored SQLite backup and a database pointed at a \
         different RPC endpoint or chain (no chain_id is persisted either; \
         provider/mod.rs:135 reads it once and never records it)."
    );
}
