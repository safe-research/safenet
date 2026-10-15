// PoC for F-CORE-060 — the underpriced-rejection fee ratchet is unbounded,
// runs every block, and bypasses `priority_fee_cap_percentage`.
//
// NEVER COMPILED. No Rust toolchain on the audit host (baseline.md §1).
// Identifiers checked by hand against commit 2893917.
//
// WHERE THIS GOES
// ---------------
// `TransactionQueue`'s internals (`storage`, `block_status`) are crate-private
// and `Provider::mocked_with_chain` is `#[cfg(any(test, feature = "test-util"))]`.
// Paste these tests into the EXISTING `#[cfg(test)] mod tests` block at the
// bottom of `crates/core/src/tx/mod.rs`, immediately before its final `}`. They
// reuse that module's `queue()`, `tx()`, `block_status()`, `fee_history()` and
// `in_flight()` helpers verbatim.
//
// RUN
// ---
//   cargo test -p safenet-core --lib tx::tests::poc_f_core_060
//
// Revert with `git checkout -- crates/core/src/tx/mod.rs` afterwards.

/// Part 1 — the ratchet is unbounded and runs once per BLOCK.
///
/// `submit_transaction`'s underpriced arm records the attempted fees as the new
/// floor with `block: None` (`tx/mod.rs:265-283`). `stale_submissions` treats a
/// `NULL` `submitted_at` as unconditionally stale (`tx/storage.rs:285-311`), so
/// the row is eligible again on the very next block regardless of
/// `blocks_before_resubmit`. `AllocatedTransaction::build` then calls
/// `fees::bump(estimate, self.fees())` (`tx/types.rs:61-77`), and `bump_fee`
/// is `previous + previous.div_ceil(10)` with no ceiling (`tx/fees.rs:52-56`).
///
/// FIXTURES — literal:
///   chain_id                   = 1
///   transaction                = to = 0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789,
///                                data = 0x01, gas = 21_000, expires_at = None
///   signer nonce (every block) = 0
///   fee estimate (every block) = base_fee 100 (doubled to 200) + reward 10
///                                → max_fee 210, max_priority 10
///   node response, block 10    = success (hash 0x00…00)  — establishes the floor
///   node response, blocks 11.. = ERROR "replacement transaction underpriced"
///                                (matches `is_transaction_underpriced`,
///                                `tx/mod.rs:359-365`)
///   blocks driven              = 10 .. 70  (60 blocks; on Gnosis' ~5 s blocks,
///                                A10, that is five minutes)
///
/// EXPECTED RESULT ON A HEALTHY SYSTEM: `max_priority_fee_per_gas` is bounded by
/// something — an absolute cap, a multiple of the fresh estimate, or a bounded
/// number of consecutive underpriced rejections. `100` is used as a deliberately
/// generous stand-in for "any bound at all": it is ten times the honest estimate
/// of 10, which never moved for the whole run.
///
/// EXPECTED RESULT ON THIS CHECKOUT: the fee compounds by
/// `previous + previous.div_ceil(10)` on every block. Hand-evaluating the
/// recurrence over the 59 bumps this test drives gives
/// `max_priority_fee_per_gas == 4_037` and `max_fee_per_gas == 59_550` — about
/// 404x and 284x their honest estimates, in 60 blocks (five minutes on Gnosis'
/// ~5 s blocks, A10). Nothing in the code stops it there; in production the only
/// brake is the signer's balance, at which point the recorded floor is pinned
/// just under `balance / gas_limit`, and if the transaction is then included the
/// tip alone can consume the whole balance.
#[tokio::test]
async fn poc_f_core_060_underpriced_ratchet_is_unbounded_and_per_block() {
    let asserter = Asserter::new();
    let mut queue = queue(&asserter).await;
    queue.queue([(tx("0x01"), None)]).await.unwrap();

    // Block 10: the first submission succeeds and establishes the floor at
    // (210, 10).
    asserter.push_success(&U64::from(0)); // eth_getTransactionCount
    asserter.push_success(&fee_history()); // eth_feeHistory
    asserter.push_success(&B256::ZERO); // eth_sendRawTransaction
    queue.update_block_status(block_status(10)).await.unwrap();

    // Blocks 11..=70: the node rejects every replacement as underpriced. The
    // reason is deliberately NOT the fee level — this models trigger instance 1
    // from the finding (a private/bundled mempool, a load-balanced backend that
    // does not hold the original, or a rate-limit path reusing the internal-error
    // code). Because the fee is never actually why, raising it never helps.
    //
    // Note the RPC shape: at block 11 the row is not yet stale
    // (`blocks_before_resubmit` = 2), so only the nonce is fetched. From the
    // first rejection onward `submitted_at` is NULL, which
    // `stale_submissions(Some(block - 2))` matches unconditionally, so every
    // subsequent block does nonce + fee-history + send.
    asserter.push_success(&U64::from(0));
    queue.update_block_status(block_status(11)).await.unwrap();

    for block in 12..=70u64 {
        asserter.push_success(&U64::from(0));
        asserter.push_success(&fee_history());
        asserter.push_failure_msg("replacement transaction underpriced");
        queue.update_block_status(block_status(block)).await.unwrap();
    }
    assert!(
        asserter.read_q().is_empty(),
        "response queue drained exactly — if not, the per-block resubmission \
         cadence differs from the one this PoC assumes and the count must be \
         re-derived before reading anything into the fee"
    );

    let transaction = in_flight(&queue).await;
    let max_priority = transaction.max_priority_fee_per_gas.unwrap();
    let max_fee = transaction.max_fee_per_gas.unwrap();

    // THE ASSERTION THAT MATTERS.
    //
    // The honest estimate never moved: every block offered (210, 10). Anything
    // above that is pure ratchet.
    assert!(
        max_priority <= 100,
        "after 60 blocks of underpriced rejections the priority fee ratcheted to \
         {max_priority} wei/gas (max_fee {max_fee}) from an unchanged honest \
         estimate of 10 — compounding once per BLOCK, with no ceiling of any kind. \
         Expected value on this checkout: 4_037 / 59_550."
    );
}

/// Part 2 — `priority_fee_cap_percentage` does not survive a bump, so the option
/// documented as bounding overpayment provides no protection once the ratchet
/// has started.
///
/// `TransactionQueue::fees` applies `cap_priority_fee` to the FRESH estimate
/// (`tx/mod.rs:322-345`). `AllocatedTransaction::build` then calls
/// `fees::bump(estimate, self.fees())`, which raises each component to at least
/// 110% of the PREVIOUS submission's (`tx/types.rs:61-77`, `tx/fees.rs:38-56`).
/// The cap is never re-applied. `fees.rs:37` already says so in a doc comment —
/// "Note that fee bumps can cause priority fee caps to not be observed" — this
/// test measures how far past the cap it goes.
///
/// FIXTURES: identical to part 1, plus
///   Config { priority_fee_cap_percentage: Some(1.0), ..Default::default() }
///   The fresh estimate is max_fee 210 / priority 10, so base_fee = 200 and
///   `cap_priority_fee` solves p/(200+p) <= 0.01 → p = 2. Every fresh estimate
///   is therefore (max_fee 202, priority 2) — the cap IS binding, which is the
///   only configuration in which it can be observed to be violated.
///   blocks driven: 10 .. 40 (30 blocks, i.e. 29 bumps).
///
/// A cap of 5% would NOT show this: the honest estimate's own ratio is
/// 10/210 = 4.76%, already under 5%, and a uniform 1.1x bump of both components
/// preserves the ratio. The violation comes from `bump_fee`'s
/// `previous.div_ceil(10)` raising a small priority fee by MORE than 10% while
/// the much larger max fee rises by exactly 10%, so the ratio climbs every
/// block. That is precisely the case `tx/fees.rs:37` warns about in its doc
/// comment — "Note that fee bumps can cause priority fee caps to not be
/// observed" — and it is never re-checked anywhere.
///
/// EXPECTED RESULT ON A HEALTHY SYSTEM: `max_priority_fee_per_gas` stays at or
/// below 1% of `max_fee_per_gas`, which is what the config option says.
///
/// EXPECTED RESULT ON THIS CHECKOUT: hand-evaluating the recurrence over 29
/// bumps gives `max_priority_fee_per_gas == 104` against
/// `max_fee_per_gas == 3_277` — a ratio of **3.17%**, more than triple the
/// configured 1% cap, and an absolute priority fee 52x the capped estimate.
#[tokio::test]
async fn poc_f_core_060_priority_fee_cap_does_not_survive_the_bump() {
    let asserter = Asserter::new();

    // Same as the module's `queue()` helper, but with the cap configured.
    let mut queue = {
        let provider = Provider::mocked_with_chain(&asserter, CHAIN_ID);
        let private_key = SigningKey::from_slice(keccak256("test signer").as_slice()).unwrap();
        let signer = Signer::new(private_key);
        let pool = SqlitePool::connect("sqlite://:memory:").await.unwrap();
        TransactionQueue::new(
            provider,
            signer,
            pool,
            Config {
                priority_fee_cap_percentage: Some(1.0),
                ..Default::default()
            },
        )
        .await
        .unwrap()
    };
    queue.queue([(tx("0x01"), None)]).await.unwrap();

    asserter.push_success(&U64::from(0));
    asserter.push_success(&fee_history());
    asserter.push_success(&B256::ZERO);
    queue.update_block_status(block_status(10)).await.unwrap();

    asserter.push_success(&U64::from(0));
    queue.update_block_status(block_status(11)).await.unwrap();

    for block in 12..=40u64 {
        asserter.push_success(&U64::from(0));
        asserter.push_success(&fee_history());
        asserter.push_failure_msg("replacement transaction underpriced");
        queue.update_block_status(block_status(block)).await.unwrap();
    }
    assert!(asserter.read_q().is_empty());

    let transaction = in_flight(&queue).await;
    let max_priority = transaction.max_priority_fee_per_gas.unwrap();
    let max_fee = transaction.max_fee_per_gas.unwrap();

    // THE ASSERTION THAT MATTERS: the configured cap must still hold.
    assert!(
        max_priority * 100 <= max_fee,
        "priority_fee_cap_percentage = 1.0 was configured, but after 29 bumped \
         resubmissions max_priority_fee_per_gas = {max_priority} against \
         max_fee_per_gas = {max_fee} — the cap is applied only to the fresh \
         estimate (tx/mod.rs:322-345) and is silently overridden by \
         AllocatedTransaction::build's bump (tx/types.rs:61-77). \
         Expected value on this checkout: 104 / 3_277, i.e. 3.17% against a 1% cap."
    );
}

/// Part 3 — the rate, isolated. A pure unit test with no RPC at all: does an
/// underpriced rejection make the row eligible for resubmission more often than
/// `blocks_before_resubmit`?
///
/// This is the assertion the finding says would have caught the conflation in
/// the first place: `submitted_at = NULL` is used both for "never submitted" and
/// for "rejected as underpriced", and `stale_submissions` cannot tell them apart.
///
/// EXPECTED RESULT ON A HEALTHY SYSTEM: after an underpriced rejection at block
/// 10, `stale_submissions(Some(9))` — the query the queue issues at block 11
/// with `blocks_before_resubmit = 2` — returns nothing, because only two blocks
/// have not yet passed.
///
/// EXPECTED RESULT ON THIS CHECKOUT: it returns the transaction, because
/// `submitted_at IS NULL` short-circuits the block comparison.
///
/// NOTE: `TransactionStorage` lives in the PRIVATE `tx::storage` module, so this
/// third test must be pasted into `crates/core/src/tx/storage.rs`'s `mod tests`
/// (which already has `storage()`, `tx()` and `fees()`), NOT into `tx/mod.rs`.
#[tokio::test]
async fn poc_f_core_060_underpriced_rejection_ignores_blocks_before_resubmit() {
    let storage = storage().await;
    storage.enqueue([(tx("0x01"), None)]).await.unwrap();
    let submitted = storage
        .next_transaction(Status { nonce: 0, block: 10 })
        .await
        .unwrap()
        .unwrap();

    // The underpriced arm records the fee floor with no block (`tx/mod.rs:274-282`).
    storage
        .record_submission(Submission {
            block: None,
            nonce: submitted.nonce,
            fees: fees(210, 10),
        })
        .await
        .unwrap();

    // At block 11 with `blocks_before_resubmit = 2` the queue asks for
    // submissions from block 9 or earlier (`tx/mod.rs:224-226`).
    let stale = storage.stale_submissions(Some(9)).await.unwrap();

    // THE ASSERTION THAT MATTERS.
    assert!(
        stale.is_empty(),
        "an underpriced rejection at block 10 made the row eligible again at \
         block 11, one block later, despite blocks_before_resubmit = 2 — the \
         NULL submitted_at sentinel conflates 'never submitted' with 'rejected \
         as underpriced' (tx/storage.rs:285-311), giving the ratchet a per-block \
         cadence: {stale:?}"
    );
}
