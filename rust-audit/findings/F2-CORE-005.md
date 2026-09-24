# F2-CORE-005 Event-fetch failures retry forever without bound, backoff or escalation, and they starve the block watcher so head-following and reorg detection stop on any persistent error other than `-32001`

| Field | Value |
| --- | --- |
| Status | QA'd |
| Crate and module | safenet-core, index/events.rs, index/mod.rs |
| Location | crates/core/src/index/mod.rs:85-130; crates/core/src/index/events.rs:337-348, 382-392, 474-486, 491-516 (related: crates/core/src/driver.rs:208-227) |
| Severity | Medium / Medium |
| Certainty | 90% (QA2-CORE; Critic C2-CORE-A set 70%) |
| Assumptions involved | A2, A4 |
| Tags | dos, input-validation |

Audited commit: `3ec8bc5`.

## Claim

`Watcher::next` advances the block watcher only when `EventWatcher::next` returns `Ok(None)`. While the event watcher is in `Step::Block` or `Step::Warping` and every attempt fails, the driver retries the identical fetch every 100 ms forever (`next_input` retries every error except `ExceededMaxReorgDepth`), the head is never polled, no reorg can be detected (the only recovery hook is `is_resource_not_found`, i.e. exactly JSON-RPC code `-32001`), and the only signal is one `warn` line per attempt. The retry counter in `Step::Block` changes the strategy once and is never used to give up; the warp page size halves down to one block and, as the code comments, "single-block pages are retried indefinitely".

Deterministic failures that produce this stall:

- (a) `DecodeLog`: a log from a watched address whose `topic0` is watched but whose layout does not decode (the crate's own test shows ERC-20 vs ERC-721 `Transfer`), or a log without `blockNumber`/`logIndex`. Decoding happens after every strategy, so the per-topic fallback does not help. Reachable by a chain participant through a watched contract that emits such a log — for the validator that includes operator-configured oracle contracts (F2-CORE-004).
- (b) `TooManyLogs` when `max_logs_per_query` is configured and one block contains at least that many logs for one topic: `block()` falls back to per-topic queries (same count for that topic), `warp()` halves to one block and then per-topic. `Consensus.proposeTransaction` is `public`, so the number of `TransactionProposed` (and coordinator `Sign`) logs per block is participant-controlled, bounded by gas.
- (c) A provider that rejects `blockHash` filters, or that answers a request for an uncled block's logs with an error code other than `-32001`: the block is never revalidated, the head never advances (node behaviour is provider-specific and was not verified offline).

Impact: silent liveness loss of the affected service (validator: every later ceremony missed; sentinel: every later vote missed) while the process stays up. `/health` semantics and the frozen `processed` block gauge are R2's.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The block watcher is polled only when the event watcher is drained | E2 | crates/core/src/index/mod.rs:85-96 | `if let Some(events) = self.next_logs().await? {` ... `} else {` / `// The event watcher is drained, so advance the chain head and hand` / `// the update to the event watcher to fetch its logs from.` / `let update = self.blocks.next().await?;` |
| 2 | Recovery exists only for `-32001` | E2 | crates/core/src/index/mod.rs:109, 135-142 | `Err(err) if is_resource_not_found(&err) => {` ... `events::Error::Rpc(rpc)` / `if rpc.as_error_resp().is_some_and(\|payload\| payload.code` / `== EthRpcErrorCode::ResourceNotFound.code() as i64)` |
| 3 | `ResourceNotFound` is `-32001` in the pinned alloy | I | registry: alloy-rpc-types-eth-2.0.5/src/error.rs:34 (dependency source, read this session) | `Self::ResourceNotFound => -32001,` |
| 4 | Block fetch failures only bump a counter; no upper bound | E2 | crates/core/src/index/events.rs:382-392 | `self.step = if result.is_ok() {` / `Step::Idle` / `} else {` / `Step::Block {` / `block_number,` / `block_hash,` / `logs_bloom,` / `retries: retries + 1,` / `}` / `};` |
| 5 | Warp pages halve to one block and then retry indefinitely (by design comment) | E2 | crates/core/src/index/events.rs:337-342 | `// Narrow the page size to query fewer logs on the next attempt.` / `// Rounding up keeps it from going below one, so single-block` / `// pages are retried indefinitely.` / `let page_size = NonZeroU64::new(page_size.get().div_ceil(2))` |
| 6 | `TooManyLogs` is raised deterministically at the cap | E2 | crates/core/src/index/events.rs:474-484 | `if let Some(max) = self.config.max_logs_per_query` / `&& logs.len() >= max.get()` / `{` ... `return Err(Error::TooManyLogs);` |
| 7 | `DecodeLog` on any non-decodable or field-less log; decoding after every strategy | E2 | crates/core/src/index/events.rs:468, 498-514 | `decode_and_sort(&logs)` ... `E::decode_log(log.topics(), &log.data().data)` ... `.ok_or_else(\|\| Error::DecodeLog {` |
| 8 | Same-topic, different-layout logs fail decoding (colliding `Transfer`) | E1 | crates/core/src/index/events.rs:750-753 (run this session, passes) | `assert_matches!(` / `decode_and_sort::<Erc20::Erc20Events>(&logs),` / `Err(Error::DecodeLog { log_index, .. }) if log_index == 1` / `);` |
| 9 | The driver retries every other error after 100 ms, forever | E2 | crates/core/src/driver.rs:210-226 | `loop {` / `match self.watcher.next().await {` / `Ok(update) => return Ok(update),` / `Err(` / `err @ index::Error::Blocks(index::blocks::Error::ExceededMaxReorgDepth(_)),` / `) => {` / `return Err(err);` / `}` / `Err(err) => {` ... `tokio::time::sleep(STEP_RETRY_DELAY).await;` |
| 10 | `proposeTransaction` is permissionless (reference contract) | E2 | contracts/src/Consensus.sol:253-264 | `function proposeTransaction(address oracle, bytes calldata oracleData, SafeTransaction.T memory transaction)` / `public` ... `emit TransactionProposed(safeTxHash, safeId, oracle, epochs.active, oracleData, transaction);` |

## Trigger

- (a) Mock: `on_block_update(New { n })`, then answer every logs query with one log from the watched address whose `topic0` is a watched selector but whose topics/data do not decode (e.g. the ERC-721-shaped `Transfer` from the crate's test). Every `Watcher::next` returns `Err(Events(DecodeLog))`; `blocks.next()` is never invoked; the mock chain can advance arbitrarily without being observed.
- (b) Config `max_logs_per_query = Some(N)`; a block with N `TransactionProposed` logs (validator watches `Consensus`). Every strategy returns N for that topic; the loop never ends. Requires a non-default cap low enough to be reached within one block's gas.
- (c) Provider-specific; not reproduced.

## Considered and rejected

- _Retry-forever is right for transient errors._ Yes; the defect is the absence of any classification (deterministic vs transient), any bound, and any reorg check while stuck. The code already classifies one error (`ExceededMaxReorgDepth`) as terminal and one (`-32001`) as "check for an uncle", so the machinery exists.
- _The `-32001` path handles uncled blocks._ Only for that code and only for nodes that use it.
- _Protocol contracts never emit undecodable logs._ True for (a) via `Consensus`/`FROSTCoordinator`/`SentinelOracle`; the validator's oracle addresses are the reachable route, and (b)/(c) do not need a malformed log.

## Remediation options

1. After K consecutive failures on a `New` block, call `revalidate_last_block` regardless of the error code; if the block is still canonical, escalate after M further failures (error log, metric, and either skip-with-alert or exit).
2. Treat `DecodeLog` as a per-log skip with a metric (or surface an `Undecodable { address, topic0 }` event to the service) instead of failing the whole block.
3. For `TooManyLogs` at page size one, fall back to `ClientFiltered` (full block + bloom equality) as a last resort, or fail loudly.
4. Exponential backoff with a ceiling in the driver (R2).

Tests to add: mocks for (a) and (b) asserting that the watcher gives up or escalates within a bounded number of attempts and that a reorg during the stall is detected.

## Trail

- Reviewer R1: drafted, self-estimate 70%. Confirms lead CORE-H8 and extends it with the block-watcher starvation mechanism. Seams: retry cadence and exit/health semantics (R2), validator oracle route (R6).
- QA2-CORE: Reproduced (routes (a) and (b) at `Watcher` level; block watcher never polled again; (c) not attempted); certainty 70 → 90; PoC `poc/F2-CORE-005/`.

## Critic (C2-CORE-A)

Independent read of `crates/core/src/index/mod.rs:85-130` and `crates/core/src/index/events.rs:281-398, 402-486`: `Watcher::next` calls `blocks.next()` only after `next_logs()` returns `Ok(None)`; every `Err` from the event watcher propagates; and the driver's `next_input` loop is the only retry:

```rust
            loop {
                match self.watcher.next().await {
                    Ok(update) => return Ok(update),
                    Err(
                        err @ index::Error::Blocks(index::blocks::Error::ExceededMaxReorgDepth(_)),
                    ) => {
                        return Err(err);
                    }
                    Err(err) => {
                        tracing::warn!(
                            ?err,
                            "failed to get next blockchain update; retrying after delay"
                        );
                        tokio::time::sleep(STEP_RETRY_DELAY).await;
                    }
```

(`crates/core/src/driver.rs:211-224`, `STEP_RETRY_DELAY` = 100 ms at line 28.) There is no counter, no growth and no ceiling: "no backoff" is accurate in the sense of no escalation - a constant delay only. `revalidate_last_block` is reachable solely through `is_resource_not_found` (`mod.rs:109, 135-142`); `Step::Block.retries` changes the strategy once and never terminates; a warp page at size one is retried indefinitely by the code's own comment (`events.rs:338-340`). Matches the reviewer.

Per-claim verdicts: 1, 2, 4-10 **Supported**. Claim 3 upgraded from `I` to E2: `~/.cargo/registry/src/index.crates.io-*/alloy-rpc-types-eth-2.0.5/src/error.rs:34` reads `Self::ResourceNotFound => -32001,` and `Cargo.lock:528-529` pins 2.0.5. Claim 8's test re-run, passes. `decode_raw_log` in the pinned `alloy-sol-types` 1.6.0 is the two-argument form (`src/types/interface/event.rs:21`), so a topic0 match with a mismatched layout is `Err` -> `None` -> `DecodeLog`, as claimed.

Route-by-route: (a) needs a watched address that emits a malformed watched-topic log; the protocol contracts do not, and the validator's oracle list is operator-trusted (see F2-CORE-004), so it is not attacker-reachable within A1/A7. (b) needs `max_logs_per_query` set below what one block can carry; neither sample config sets it (grep of `crates/*/*.sample.toml`), and R1's own O7 concedes realistic caps are unreachable within one Gnosis block's gas. (c) is unverified node behaviour (`I`): if a node in use answers an uncled block's by-hash `eth_getLogs` with any code other than -32001, a reorg _within_ `max_reorg_depth` produces a permanent stall until restart - that would meet §8's High wording, but it is not shown, and a restart heals it (the uncommitted block is re-derived from the retained anchor).

Verdict: **Confirmed** (mechanism verified; (a)/(b) trigger sequences concrete; (c) stays `I`). Certainty **70**. Severity **Medium / Medium**: a persistent per-block error of any kind other than -32001 stalls head-following and reorg detection silently, with no bound, no escalation and no signal beyond a warn line - a robustness defect under A4's degraded-RPC assumption; not High because no attacker-reachable route survives A1/A7 and the shipped defaults. F2-CORE-008 is the depth-0 instance of this starvation; both are kept, this finding is canonical for the mechanism and 008 for the broken depth-0 promise. F2-CORE-010 covers the request-volume side of the same fallback. Remediation 1 is sound; remediation 2 must keep a metric or the loss becomes invisible. A16/A17 not applicable.

## QA (QA2-CORE)

**Outcome: Reproduced** — routes (a) and (b) at `Watcher` level including the block-watcher starvation; (c) not attempted (provider-specific).

Command: paste `poc/F2-CORE-005/index_mod_tests.rs` into the `mod tests` of `crates/core/src/index/mod.rs`; `cargo test -p safenet-core --lib qa_f2_core_005 -- --nocapture --test-threads=1`; file reverted.

Decisive output (`poc/F2-CORE-005/output.txt`): (a) a watched-address log with the watched `topic0` but no indexed topic and no data: `attempt  1 (SingleQuery): Err(Events(DecodeLog { .. }))` through `attempt 12 (MultipleQueries): Err(Events(DecodeLog { .. }))`; `after 12 attempts: block watcher status BlockStatus { latest: 1000, safe: 999 }; unconsumed mock responses: 1` — the canonical block 1001 queued at the mock was never requested. (b) `max_logs_per_query = 1` and one watched log in the block: `attempt 1..8: Err(Events(TooManyLogs))` under both strategies; block 1001 never requested. The driver's constant 100 ms retry (`driver.rs:28, 218-224`) is not exercised and stays `E2`.

Certainty: 70 → **90**. Confirmed plus `E1` for the mechanism and the starvation. The Critic's reachability caveats (operator-trusted oracle for (a), non-default cap for (b)) stand; they are reflected in the Medium severity, not in the certainty.

Remediation check: option 1 sound (bounded attempts, revalidate regardless of the error code, then escalate). Option 2 is unsound as written for protocol contracts: an undecodable `Consensus`/`FROSTCoordinator`/`SentinelOracle` log means an ABI mismatch, and skipping it silently derives a wrong state — restrict per-log skipping to non-protocol addresses or fail loudly, always with a metric. Options 3 and 4 fine; 4 alone does not fix the starvation.

## Reconciliation (run 2)

**Final: EXTENDS `F-CORE-004` (canonical) — combined Medium, 90 (E1).** Run 1 filed the no-terminal-error-state defect and the `DecodeLog`-via-oracle route (Medium 75, E2); this file adds and executes the block-watcher starvation (reorg detection stops while the event watcher errs) and the `TooManyLogs` route, lifting certainty to 90. The escalation remedy is shared with `F-CORE-034` option 2; `F2-CORE-008` = `F-CORE-005` is the depth-0 instance (`state/run2/reconciliation/core.md` §1).
