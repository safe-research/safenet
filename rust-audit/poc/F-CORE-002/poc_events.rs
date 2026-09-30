// PoC for F-CORE-002 — `use_client_filtering`'s log-completeness check
// disables itself after `block_single_query_retry_count` failures, and the
// `IncompleteLogs` errors it raises are what exhaust that budget.
//
// NEVER COMPILED. No Rust toolchain on the audit host (baseline.md §1).
// Identifiers checked by hand against commit 2893917.
//
// WHERE THIS GOES
// ---------------
// `Fetch`, `Step` and `EventWatcher::block` are private to
// `crates/core/src/index/events.rs`, and `Provider::mocked` is gated behind
// `#[cfg(any(test, feature = "test-util"))]` which `cargo test -p safenet-core`
// does not enable for an integration test. Paste these tests into the EXISTING
// `#[cfg(test)] mod tests` block at the bottom of
// `crates/core/src/index/events.rs`, immediately before its final `}`. They
// reuse that module's `watcher()`, `log()`, `event_log()`, `Erc20` `sol!` block
// and `WATCHED` constant verbatim.
//
// RUN
// ---
//   cargo test -p safenet-core --lib index::events::tests::poc_f_core_002
//
// Revert with `git checkout -- crates/core/src/index/events.rs` afterwards.

/// The consensus-relevant claim: **logs are committed as complete when they are
/// not.**
///
/// `EventWatcher::block` selects `Fetch::ClientFiltered` — the only shape that
/// bloom-verifies the node's answer — only while
/// `retries < block_single_query_retry_count` (`events.rs:369-380`). Every
/// failure increments `retries`, including the `Error::IncompleteLogs` that the
/// bloom check itself raises (`events.rs:450-456`, `:383-390`). Once the budget
/// is spent the watcher falls back to `Fetch::MultipleQueries`, a node-filtered
/// query with **no completeness check of any kind**, and whatever comes back —
/// an empty vector included — is returned as the block's logs.
///
/// `StateMachine::handle_update` then applies zero events and commits a snapshot
/// at that block (`crates/core/src/state/mod.rs:200-239`), so the block is
/// permanently recorded as fully processed and is never re-fetched.
///
/// FIXTURES — literal:
///   config: use_client_filtering = true, block_single_query_retry_count = 1
///           (the shipped default is 3; 1 keeps the test short and the mechanism
///           is identical — see the `_default_budget` variant below)
///   block:  number 1337, hash 0x1313…13
///   logs_bloom: the bloom of ONE `Erc20::Transfer` log — i.e. the header
///           asserts that a watched log IS present in this block
///   node responses, in order:
///     1. `[]`  (attempt 1, client-filtered) — the documented Nethermind-below-1.36
///              behaviour: an empty result for a block queried too soon
///     2. `[]`  (attempt 2, per-topic query for Erc20::Transfer)
///     3. `[]`  (attempt 2, per-topic query for Erc20::Approval)
///
/// EXPECTED RESULT ON A HEALTHY SYSTEM: attempt 2 is still bloom-verified (or
/// the empty result is rejected against the header bloom), so `next()` returns
/// `Err(Error::IncompleteLogs { .. })` again rather than an empty update. The
/// indexer stalls loudly instead of losing the block.
///
/// EXPECTED RESULT ON THIS CHECKOUT: attempt 1 returns
/// `Err(Error::IncompleteLogs { block_hash: 0x1313…13 })` and attempt 2 returns
/// `Ok(Some(EventUpdate { blocks: 1337..=1337, logs: [] }))` — **an empty log
/// set accepted as authoritative for a block whose own header bloom says a
/// watched log is present.**
#[tokio::test]
async fn poc_f_core_002_incomplete_logs_exhaust_the_budget_that_detects_them() {
    let asserter = Asserter::new();
    let mut events = watcher(
        &asserter,
        Config {
            use_client_filtering: true,
            block_single_query_retry_count: NonZeroU64::new(1).unwrap(),
            ..Default::default()
        },
    );

    // The block genuinely contains one watched `Transfer`. The header bloom is
    // computed over it, so the bloom is the ground truth the check compares
    // against.
    let real_log = log(
        (1337, 0),
        Erc20::Transfer {
            amount: uint!(1_U256),
            ..Default::default()
        },
    );
    let header_bloom = bloom::compute_logs_bloom(std::slice::from_ref(&real_log));
    assert_ne!(
        header_bloom,
        Bloom::ZERO,
        "sanity: the header must assert that a watched log is present"
    );

    events
        .on_block_update(BlockUpdate::New {
            number: 1337,
            hash: B256::repeat_byte(0x13),
            logs_bloom: header_bloom,
        })
        .unwrap();

    // Attempt 1 — client-filtered. The node serves nothing (queried too soon).
    // `compute_logs_bloom(&[]) == Bloom::ZERO != header_bloom` → IncompleteLogs.
    asserter.push_success(&Vec::<Log>::new());
    assert_matches!(
        events.next().await,
        Err(Error::IncompleteLogs { block_hash }) if block_hash == B256::repeat_byte(0x13),
        "attempt 1 must detect the incomplete answer"
    );

    // Attempt 2 — the budget is spent, so `Fetch::MultipleQueries` is used: one
    // node-filtered query per watched topic (`Erc20` has two: Transfer and
    // Approval). The node is still lagging and serves nothing for either.
    asserter.push_success(&Vec::<Log>::new());
    asserter.push_success(&Vec::<Log>::new());

    // THE ASSERTION THAT MATTERS.
    //
    // The header bloom still says a watched log is in this block. Accepting an
    // empty set here is the defect: `StateMachine` will commit a snapshot at
    // 1337 with no events applied, and block 1337 is never re-fetched.
    let result = events.next().await;
    assert_matches!(
        result,
        Err(Error::IncompleteLogs { .. }),
        "the node's second, unverified answer was accepted as complete: {result:?} \
         — a block whose header bloom asserts a watched log was committed with \
         zero events"
    );
    assert!(asserter.read_q().is_empty());
}

/// The same defect reached without any incomplete response at all: three
/// UNRELATED transient failures (HTTP 429 from a rate-limited provider is the
/// likeliest, and is explicitly in scope under A4) exhaust the same budget, so
/// the very next attempt is unverified even against a node that is otherwise
/// healthy.
///
/// This variant matters because it removes the "the node is broken anyway"
/// objection: the integrity check is stripped off by rate limiting, and the
/// answer that is then accepted comes from a node that would have passed the
/// check.
///
/// FIXTURES: shipped default `block_single_query_retry_count = 3`;
/// `use_client_filtering = true`; block 1337 / hash 0x1313…13 / bloom of one
/// `Transfer`; node responses: three `push_failure_msg("429 Too Many Requests")`
/// then two empty per-topic results.
///
/// EXPECTED RESULT ON A HEALTHY SYSTEM: attempt 4 is still bloom-verified, so
/// the empty answer is rejected.
///
/// EXPECTED RESULT ON THIS CHECKOUT: attempt 4 returns
/// `Ok(Some(EventUpdate { blocks: 1337..=1337, logs: [] }))`.
#[tokio::test]
async fn poc_f_core_002_transient_failures_strip_the_integrity_check_default_budget() {
    let asserter = Asserter::new();
    let mut events = watcher(
        &asserter,
        Config {
            use_client_filtering: true,
            // shipped default (`events.rs:98`)
            block_single_query_retry_count: NonZeroU64::new(3).unwrap(),
            ..Default::default()
        },
    );

    let real_log = log(
        (1337, 0),
        Erc20::Transfer {
            amount: uint!(1_U256),
            ..Default::default()
        },
    );
    let header_bloom = bloom::compute_logs_bloom(std::slice::from_ref(&real_log));

    events
        .on_block_update(BlockUpdate::New {
            number: 1337,
            hash: B256::repeat_byte(0x13),
            logs_bloom: header_bloom,
        })
        .unwrap();

    // Attempts 1-3: rate limited. Nothing to do with log integrity.
    for _ in 0..3 {
        asserter.push_failure_msg("429 Too Many Requests");
        assert_matches!(events.next().await, Err(Error::Rpc(_)));
    }

    // Attempt 4: budget spent, so `Fetch::MultipleQueries` with no bloom check.
    // The node answers empty for both topics.
    asserter.push_success(&Vec::<Log>::new());
    asserter.push_success(&Vec::<Log>::new());

    // THE ASSERTION THAT MATTERS.
    let result = events.next().await;
    assert_matches!(
        result,
        Err(Error::IncompleteLogs { .. }),
        "three rate-limit responses stripped the integrity check off the next \
         attempt, and its empty answer was accepted: {result:?}"
    );
    assert!(asserter.read_q().is_empty());
}

/// The related exposure the finding records in the same place, made concrete:
/// bloom verification is per-block, so it NEVER applies to a warp range — the
/// path every restart takes (`events.rs:303-322`; `warp` picks
/// `Fetch::SingleQuery` or `Fetch::MultipleQueries`, never `Fetch::ClientFiltered`).
///
/// With `use_client_filtering = true` the operator has been told (validator
/// handbook, lines 31-40) that log integrity is checked; for the entire
/// catch-up range after downtime — potentially thousands of blocks — it is not.
///
/// EXPECTED RESULT ON A HEALTHY SYSTEM: unclear by design; there is no header
/// bloom for a range. The point of this test is documentary — it asserts the
/// CURRENT behaviour so that the report can state the limit precisely, and so
/// that any future change to warp verification breaks it visibly.
///
/// EXPECTED RESULT ON THIS CHECKOUT: it PASSES, i.e. the warp accepts an empty
/// result with no verification whatsoever, using exactly one RPC round trip.
#[tokio::test]
async fn poc_f_core_002_warp_range_is_never_bloom_verified() {
    let asserter = Asserter::new();
    let mut events = watcher(
        &asserter,
        Config {
            use_client_filtering: true,
            block_page_size: NonZeroU64::new(100).unwrap(),
            ..Default::default()
        },
    );

    events
        .on_block_update(BlockUpdate::Warp { from: 1, to: 50 })
        .unwrap();

    // ONE query for the whole 50-block range, node-filtered, unverified.
    asserter.push_success(&Vec::<Log>::new());

    assert_eq!(
        events.next().await.unwrap(),
        Some(EventUpdate {
            blocks: range(1..=50),
            logs: vec![],
        }),
        "documentary: with use_client_filtering ENABLED, a 50-block warp is \
         served by one unverified node-filtered query"
    );
    assert!(asserter.read_q().is_empty());
}
