# F2-CORE-002 `use_client_filtering` integrity protection is silently abandoned after `block_single_query_retry_count` attempts

| Field | Value |
| --- | --- |
| Status | QA'd |
| Crate and module | safenet-core, index/events.rs |
| Location | crates/core/src/index/events.rs:362-398 (related: crates/core/src/index/events.rs:412-466, crates/core/src/driver.rs:28 and 208-227) |
| Severity | Medium (High on handbook providers per R1) / Medium |
| Certainty | 90% (QA2-CORE; Critic C2-CORE-A set 80%) |
| Assumptions involved | A4 |
| Tags | input-validation, config |

Audited commit: `3ec8bc5`.

## Claim

With `use_client_filtering = true`, the watcher fetches _all_ logs of a new block by hash and verifies them by recomputing the bloom filter and comparing it with the header's `logs_bloom`; a mismatch is `Error::IncompleteLogs`, which the driver retries after 100 ms. However `EventWatcher::block` chooses the strategy by `retries < block_single_query_retry_count` (default 3). From the fourth attempt on it uses `Fetch::MultipleQueries` — node-filtered per-topic `eth_getLogs` with no bloom check and no completeness check (unless the unrelated, default-off `max_logs_per_query` is set) — and accepts whatever the node returns, including an empty set. The empty `EventUpdate` is committed as the block's snapshot and the block is never fetched again.

Every failure counts toward the budget: bloom mismatches, HTTP 429, timeouts, transport errors. So the protection the handbooks offer for providers that "return an empty array if the logs are queried too soon after a block is observed" lasts about 3 x (100 ms + round trip) after the header is first seen, and a burst of transient errors spends the budget before the first verified attempt. The handbooks state that "the integrity of logs are critical for proper validator operation". The fallback was designed for a different problem (nodes that cap response sizes) and cannot be turned off independently of that use; its coupling to the integrity check is undocumented.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Strategy selection abandons client filtering after the retry budget | E2 | crates/core/src/index/events.rs:369-380 | see quote A below |
| 2 | Only `ClientFiltered` verifies completeness | E2 | crates/core/src/index/events.rs:448-456 | `// Verify the node served a complete set of logs for the block by` / `// recomputing the bloom filter over every returned log.` / `if bloom::compute_logs_bloom(&logs) != logs_bloom {` ... `return Err(Error::IncompleteLogs { block_hash });` |
| 3 | `MultipleQueries` has no check beyond the optional log cap | E2 | crates/core/src/index/events.rs:415-420, 474-477 | `.event_signature(*topic);` / `let logs = self.provider.get_logs(&filter).await?;` / `self.check_logs_limit(logs)` ... `if let Some(max) = self.config.max_logs_per_query` / `&& logs.len() >= max.get()` |
| 4 | Any error increments `retries` | E2 | crates/core/src/index/events.rs:382-392 | `let result = self.fetch_logs(fetch).await;` / `self.step = if result.is_ok() {` / `Step::Idle` / `} else {` / `Step::Block {` ... `retries: retries + 1,` |
| 5 | The driver retries every error after a fixed 100 ms | E2 | crates/core/src/driver.rs:28, 218-224 | `const STEP_RETRY_DELAY: Duration = Duration::from_millis(100);` ... `Err(err) => {` / `tracing::warn!(` / `?err,` / `"failed to get next blockchain update; retrying after delay"` / `);` / `tokio::time::sleep(STEP_RETRY_DELAY).await;` |
| 6 | Default budget is 3; `max_logs_per_query` defaults to off | E2 | crates/core/src/index/events.rs:97-100 | `block_page_size: NonZeroU64::new(100).expect("100 is nonzero"),` / `block_single_query_retry_count: NonZeroU64::new(3).expect("3 is nonzero"),` / `use_client_filtering: false,` / `max_logs_per_query: None,` |
| 7 | A successful (possibly empty) update is committed at that block, after which the head moves on | E2 | crates/core/src/state/mod.rs:225-236 (R2 file, mechanism only) | `_ => {` / `let pending = next_block(blocks.last)?;` / `Status::BlockPending { pending }` / `}` / `};` / `self.snapshots.commit(blocks.last, &state).await?;` |
| 8 | The existing fallback test exercises the path only without client filtering and shows the fourth attempt is trusted | E1 | crates/core/src/index/events.rs:1385-1448 (run this session, passes) | `block_single_query_retry_count: NonZeroU64::new(2).unwrap(),` ... `// The retries are exhausted, so the watcher falls back to one query per` / `// event (two for \`Erc20\`).` |
| 9 | Handbook guidance and problem statement | E2 | docs/validator-handbook.md:33-41; docs/sentinel-handbook.md:23-31 | `if the logs are queried too soon after a block is observed then an empty array will be returned even if there are logs in that block.` ... `The integrity of logs are critical for proper validator operation.` |

Quote A (`crates/core/src/index/events.rs:369-380`):

```rust
        let fetch = if retries < self.config.block_single_query_retry_count.get() {
            if self.config.use_client_filtering {
                Fetch::ClientFiltered {
                    block_hash,
                    logs_bloom,
                }
            } else {
                Fetch::SingleQuery(BlockFilter::Hash(block_hash))
            }
        } else {
            Fetch::MultipleQueries(BlockFilter::Hash(block_hash))
        };
```

## Trigger

A provider with the documented behaviour that still serves `[]` (or a partial set) for a fresh block about 0.4-1 s after its header became visible; or three transient failures (429s, a timeout) on a new block followed by an empty node-filtered answer. Attempt 4 is accepted without verification.

Mock (mirrors the existing fallback test): `Config { use_client_filtering: true, ..Default::default() }`; `on_block_update(New { number: n, hash: h, logs_bloom: B })` with `B` the bloom of a non-empty log set; push three `[]` responses (each fails with `IncompleteLogs`); then push one `[]` per watched topic. `next()` returns `Some(EventUpdate { blocks: n..=n, logs: [] })` and the watcher is `Idle`.

## Considered and rejected

- _Three attempts are enough for the Nethermind lag._ The lag length is not specified anywhere; the budget is fixed and small, and transient errors consume it. The design choice that the operator made ("verify every block") is overridden by a constant they were not told about.
- _The operator can raise `block_single_query_retry_count`._ It is a `NonZeroU64`, so a very large value effectively disables the fallback; the coupling is undocumented, the sample configs (crates/validator/validator.sample.toml:70, crates/sentinel/sentinel.sample.toml:54) only mention `use_client_filtering`, and doing so also disables the size-cap fallback.
- _`MultipleQueries` returning `[]` means there are no logs._ That is exactly the node-filtered answer the flag was introduced to distrust.

## Remediation options

1. Never leave `ClientFiltered` once it is enabled: fall back to `MultipleQueries` only when `use_client_filtering` is false. Tradeoff: a size-capped node with client filtering enabled would retry the full-block fetch forever; that combination is already unsupported in spirit (the full-block fetch is what client filtering means).
2. Keep a fallback but gate it: when the fallback returns no logs for a topic, reject the result if `bloom::may_contain_log(&logs_bloom, &self.addresses, &[topic])` is true (bounded false-positive cost, no false negatives).
3. Separate the two budgets (`client_filtering_retry_count` unbounded by default) and document the interaction in both handbooks.

Tests to add: the mock above, asserting that with client filtering enabled no `EventUpdate` is ever produced by an unverified fetch.

## Trail

- Reviewer R1: drafted, self-estimate 85%. Confirms lead CORE-H2. Retry cadence cited from driver.rs (R2's file) for the timing claim only.
- QA2-CORE: Reproduced (bloom-rejected `[]` x3, then unverified empty update accepted; transient errors spend the same budget); certainty 80 → 90; PoC `poc/F2-CORE-002/`.

## Critic (C2-CORE-A)

Independent read of `crates/core/src/index/events.rs:362-398` before the Claim: the strategy is selected from `retries` alone; `use_client_filtering` is consulted only inside the `retries < block_single_query_retry_count` branch, so from the fourth attempt (default budget 3) the fetch is `MultipleQueries`, whose only check is the optional `max_logs_per_query` cap (`events.rs:412-440, 474-486`). An all-empty per-topic answer concatenates to `[]`, decodes to `Ok(vec![])`, is returned as a complete `EventUpdate` and committed by the state machine (`state/mod.rs:230-236`). The retry counter is per block and is never reset by the kind of error, so 429s and timeouts spend the same budget. Matches the reviewer.

Per-claim verdicts: 1-8 **Supported** (quotes verified at the cited lines; the test in claim 8 re-run this session, 51/51). Claim 9 **Supported**: `docs/validator-handbook.md:33-35` and `docs/sentinel-handbook.md:23-25` carry both quoted sentences verbatim. The mock in the Trigger mirrors `new_block_falls_back_to_multiple_queries_after_retries` (`events.rs:1385-1448`); a QA test needs only `use_client_filtering: true` and a non-zero bloom.

Timing check: the driver retries after a constant `STEP_RETRY_DELAY` of 100 ms (`driver.rs:28, 223`), so the verified protection window is three attempts within about 300 ms plus three round trips after the header is observed (itself `block_propagation_delay` = 500 ms after the expected slot time). The handbook does not bound the provider lag, so no argument that three attempts suffice can be made from the checkout.

Verdict: **Confirmed**. Certainty **80** (E2). Severity: reviewer Medium (High on the handbook's providers) -> **Medium**. The impact is silent event loss for one service (for a validator: missed sessions and a diverged state), but the input is a degraded RPC - A4 explicitly expects incomplete `eth_getLogs`, and the code's opt-in defence is what fails - not attacker-controlled input or a reorg, which is what PROMPT §8 reserves High for. The undocumented knob (`block_single_query_retry_count` very large) also argues against High. A16/A17 not applicable.

Remediation: option 1 is sound and minimal. Option 2 is weaker than it reads: `may_contain_log` answers "maybe" for nearly every busy Gnosis block (2048-bit bloom, dozens of logs), so it would rarely reject an empty answer - see F2-CORE-003. Distinct from F2-CORE-003 (default path) and F2-CORE-010 (request volume of the same fallback).

## QA (QA2-CORE)

**Outcome: Reproduced.**

Command: paste `poc/F2-CORE-002/events_tests.rs` into the `mod tests` of `crates/core/src/index/events.rs`; `cargo test -p safenet-core --lib qa_f2_core_002 -- --nocapture --test-threads=1`; file reverted.

Decisive output (`poc/F2-CORE-002/output.txt`), `Config { use_client_filtering: true, ..Default::default() }` and a header bloom computed from a real watched `Transfer` log: `attempt 1 (ClientFiltered, node serves []): Err(IncompleteLogs { .. })` (same for attempts 2 and 3), then `attempt 4 (MultipleQueries, node serves [] per topic): Ok(Some(EventUpdate { blocks: 1337..=1337, logs: [] }))` and the watcher is idle: the block is finished without its log. Second test: three `429` transport errors spend the same budget with the identical outcome.

Certainty: 80 → **90**. Confirmed plus `E1`; the provider lag itself remains an A4 assumption.

Remediation check: option 1 is sound and minimal; note that on a size-capped node it turns the silent loss into an indefinite retry, i.e. the F2-CORE-005 stall pattern — acceptable (loud beats silent) but 005's escalation should ship with it. Option 2 is weak (Critic). Option 3 sound.

## Reconciliation (run 2)

**Final: CONFIRMS `F-CORE-002` (canonical) — combined High, 99 (E1).** Same defect; this file's `EventWatcher`-level PoC matches run 1's Phase 8 A/B, which drove the loss to an on-chain balance change (−4,500) after three 429s. High is carried; this file's Medium 90 is recorded as the dissent (`state/run2/reconciliation/core.md` §1.1). Run 1's sibling `F-CORE-012` (bloom equality blind to repeated `(address, topics)` shapes; `check_logs_limit` absent on the `ClientFiltered` path) was **not** rediscovered by run 2 and remains a Medium 70 finding beside this one.
