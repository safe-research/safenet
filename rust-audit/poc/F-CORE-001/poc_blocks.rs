// PoC for F-CORE-001 — persisted indexer state is bound to block NUMBERS only,
// so a reorg during downtime is invisible and silently defeats
// `max_reorg_depth`.
//
// NEVER COMPILED. No Rust toolchain on the audit host (baseline.md §1).
// Identifiers checked by hand against commit 2893917.
//
// WHERE THIS GOES
// ---------------
// Test 1 and 2 → the EXISTING `#[cfg(test)] mod tests` at the bottom of
//   `crates/core/src/index/blocks.rs`, before its final `}`. They reuse that
//   module's `config()`, `block()`, `block_with()`, `block_hash()`,
//   `new_block_update()` helpers.
// Test 3 → the EXISTING `mod tests` at the bottom of
//   `crates/core/src/state/storage.rs`.
//
// RUN
// ---
//   cargo test -p safenet-core --lib poc_f_core_001
//
// Revert with:
//   git checkout -- crates/core/src/index/blocks.rs crates/core/src/state/storage.rs

/// THE CORE DEMONSTRATION: the watcher's behaviour on resume is IDENTICAL
/// whether or not the persisted rollback anchor is still on the canonical
/// chain, because its identity is never consulted.
///
/// `BlockWatcher::initialize` receives only `BlockStatus { latest, safe }` —
/// two integers (`blocks.rs:244-289`). The `snapshots` table stores
/// `(block_number, state)` and no hash (`state/storage.rs:51-57`), and
/// `status()` returns `MIN`/`MAX` of the block numbers (`:86-101`). Nothing
/// anywhere compares a persisted identity against the chain.
///
/// This test runs the same resume twice against two DIFFERENT chains that share
/// block numbers but not block hashes at and around the anchor, and asserts the
/// two runs are distinguishable. On this checkout they are byte-identical.
///
/// FIXTURES — literal:
///   config: max_reorg_depth = 2, block_time = 2_000 ms,
///           block_propagation_delay = 500, start_block = None
///   persisted: BlockStatus { latest: 900, safe: 898 }
///           (i.e. snapshots for blocks 898..=900, `MIN` = 898)
///   chain A (the honest continuation): blocks 998, 999, 1000 with the module's
///           default `block_hash(n)` identities
///   chain B (the post-reorg chain): the SAME block numbers with DIFFERENT
///           hashes — every header's `hash` and `parent_hash` are re-derived
///           from `keccak256("reorged" || n)`. In particular block 898, the
///           anchor whose state is persisted, is orphaned on chain B.
///   RPC responses in both runs: latest (1000), then 998, then 999.
///
/// EXPECTED RESULT ON A HEALTHY SYSTEM: the run against chain B fails —
/// `BlockWatcher::new` returns an error (the finding's proposed
/// `Error::ForkedResumePoint`), or at minimum fetches block 898 to check it, so
/// the two runs differ in either their result or their RPC traffic.
///
/// EXPECTED RESULT ON THIS CHECKOUT: both runs succeed and produce the SAME
/// queue —
///   [Uncle { number: 899 }, Warp { from: 899, to: 998 },
///    New { 999 }, New { 1000 }]
/// — and neither issues any RPC for block 898 or 899 at all. The state derived
/// from the orphaned fork is accepted as the rollback anchor and canonical logs
/// are replayed on top of it, with no error, no warning and no metric.
#[tokio::test]
async fn poc_f_core_001_resume_ignores_whether_the_persisted_anchor_is_canonical() {
    use alloy::primitives::keccak256;

    // A block on the post-reorg chain: same number, different identity.
    fn reorged_block(number: u64) -> Block {
        block_with(number, |header| {
            header.hash = keccak256(format!("reorged{number}"));
            header.inner.parent_hash = keccak256(format!("reorged{}", number - 1));
        })
    }

    // The persisted resume point. Snapshots exist for 898..=900, so
    // `SnapshotStore::status()` yields `{ safe: 898, latest: 900 }`
    // (`state/storage.rs:86-101`) — the anchor is block 898.
    let indexed = BlockStatus {
        latest: 900,
        safe: 898,
    };

    // ---------- RUN A: block 898 is still canonical.
    let asserter_a = Asserter::new();
    asserter_a.push_success(&block(1000));
    asserter_a.push_success(&block(998));
    asserter_a.push_success(&block(999));
    let mut a = BlockWatcher::new(Provider::mocked(&asserter_a), config(), Some(indexed))
        .await
        .unwrap();
    let queue_a = a.ready().collect::<Vec<_>>();
    assert!(
        asserter_a.read_q().is_empty(),
        "run A drained its responses; the watcher issued exactly three RPCs \
         (latest, 998, 999) and NONE for the persisted anchor 898"
    );

    // ---------- RUN B: a reorg deeper than `max_reorg_depth` happened while the
    // process was down, so block 898 — the anchor whose state is persisted — is
    // orphaned. This is precisely the event class A5 declares must be fatal, and
    // it is what triggers `ExceededMaxReorgDepth` (`blocks.rs:435-439`) when it
    // happens WHILE RUNNING.
    let asserter_b = Asserter::new();
    asserter_b.push_success(&reorged_block(1000));
    asserter_b.push_success(&reorged_block(998));
    asserter_b.push_success(&reorged_block(999));
    let mut b = BlockWatcher::new(Provider::mocked(&asserter_b), config(), Some(indexed))
        .await
        .unwrap();
    let queue_b = b.ready().collect::<Vec<_>>();
    assert!(asserter_b.read_q().is_empty());

    // Sanity: the two chains really are different where it counts.
    assert_ne!(
        block(998).header.hash,
        reorged_block(998).header.hash,
        "sanity: the fixture chains must differ"
    );

    // THE ASSERTION THAT MATTERS.
    //
    // The block-number half of the queue is necessarily the same. What must NOT
    // be the same is the outcome: resuming onto a chain where the persisted
    // anchor is orphaned has to be detectable somehow. On this checkout the only
    // difference between the two queues is the `hash` field of the `New` updates
    // for 999 and 1000 — blocks the watcher fetched — while the rollback anchor
    // itself was never looked at.
    let numbers_a = queue_a
        .iter()
        .map(|update| match update {
            BlockUpdate::New { number, .. } => ("new", *number),
            BlockUpdate::Uncle { number } => ("uncle", *number),
            BlockUpdate::Warp { from, .. } => ("warp", *from),
        })
        .collect::<Vec<_>>();
    let numbers_b = queue_b
        .iter()
        .map(|update| match update {
            BlockUpdate::New { number, .. } => ("new", *number),
            BlockUpdate::Uncle { number } => ("uncle", *number),
            BlockUpdate::Warp { from, .. } => ("warp", *from),
        })
        .collect::<Vec<_>>();

    assert_ne!(
        numbers_a, numbers_b,
        "resuming against a chain on which the persisted rollback anchor (block \
         898) is ORPHANED produced exactly the same plan as resuming against the \
         chain the state was derived from: {numbers_a:?}. The anchor's identity \
         is never fetched or compared, so the state machine rolls back to state \
         derived from an orphaned fork and replays canonical logs on top of it, \
         with no error, no warning log and no metric. While RUNNING, the same \
         reorg is fatal (ExceededMaxReorgDepth, blocks.rs:435-439)."
    );
}

/// The self-reinforcing variant, stated as an assertion about the pruning
/// policy rather than about a chain: the retained window is exactly
/// `max_reorg_depth` deep, so the anchor sits at precisely the depth at which a
/// reorg is fatal while running.
///
/// `prune` retains everything from the watcher's `safe` block upward
/// (`state/storage.rs:145-161`) and `safe = latest - max_reorg_depth`
/// (`blocks.rs:246`). So `MIN(block_number) == latest - max_reorg_depth`, and
/// `Uncle { MIN + 1 }` is emitted on every restart (`blocks.rs:261-266`).
///
/// The consequence, which is what makes option 2 of the remediation ("walk back
/// through the retained snapshots until one matches") weak on its own: there is
/// nothing to walk back TO. The whole retained set is inside the window a
/// fatal-depth reorg replaces.
///
/// EXPECTED RESULT ON A HEALTHY SYSTEM: the retained snapshot window is deeper
/// than `max_reorg_depth`, leaving at least one anchor below the depth at which
/// a reorg is declared fatal.
///
/// EXPECTED RESULT ON THIS CHECKOUT: the window is exactly `max_reorg_depth + 1`
/// snapshots (`safe ..= latest`), so every retained anchor is within the fatal
/// depth.
#[tokio::test]
async fn poc_f_core_001_retained_window_is_exactly_the_fatal_depth() {
    let asserter = Asserter::new();
    asserter.push_success(&block(1000));
    asserter.push_success(&block(998));
    asserter.push_success(&block(999));
    let blocks = BlockWatcher::new(Provider::mocked(&asserter), config(), None)
        .await
        .unwrap();

    let status = blocks.status();
    let max_reorg_depth = config().max_reorg_depth;

    // This is the value `StateMachine::prune` is called with on every update
    // (`crates/core/src/driver.rs:257`), and therefore the oldest snapshot the
    // store retains.
    assert_eq!(status.latest - status.safe, max_reorg_depth);

    // THE ASSERTION THAT MATTERS.
    assert!(
        status.latest - status.safe > max_reorg_depth,
        "the oldest retained snapshot sits exactly max_reorg_depth ({max_reorg_depth}) \
         blocks below the head, i.e. at exactly the depth at which a reorg is \
         declared fatal while running (blocks.rs:435-439). After a restart that \
         same depth is accepted silently, and there is no deeper retained anchor \
         to fall back to — which is why remediation option 2 (walk back through \
         retained snapshots) cannot work without also retaining more of them."
    );
}

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
        "the snapshots table stores no block hash — columns are {columns:?}. \
         Nothing in the persisted state identifies the chain it was derived from, \
         so a resume cannot tell a canonical anchor from an orphaned one. The \
         same gap accepts a restored SQLite backup and a database pointed at a \
         different RPC endpoint or chain (no chain_id is persisted either; \
         provider/mod.rs:135 reads it once and never records it)."
    );
}
