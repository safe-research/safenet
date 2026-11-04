# Plan: Indexer uncled-block recovery and retry backoff

Component: `crates/core` (Cargo package `safenet-core`): the `index` module (`index/mod.rs`, `index/events.rs`, `index/blocks.rs`), `driver.rs` and `metrics.rs`. Both the validator and the sentinel run on `safenet_core::Driver`, so both get the fix without changes of their own.

---

## Overview

After a reorg at the chain head, a validator was found stuck on a block hash that no longer exists, sending `eth_getLogs` requests with no backoff until it ran out of RPC quota. Two defects in `safenet-core` cause this together:

1. **Recovery depends on the error code.** [`Watcher::next_logs`](../crates/core/src/index/mod.rs) only asks the block watcher to revalidate its last block when `eth_getLogs` fails with exactly `-32001` (`ResourceNotFound`, as Reth reports it). Geth-based nodes report an unknown `blockHash` as `-32000 "unknown block"`. On any other code, the event watcher stays in `Step::Block` on the uncled hash forever. `blocks.next()` is never reached, so the parent-hash check that would also expose the reorg never runs.
2. **No backoff.** [`Driver::next_input`](../crates/core/src/driver.rs) retries every watcher error, except `ExceededMaxReorgDepth`, after a fixed 100 ms `STEP_RETRY_DELAY`. `EventWatcher::block` has no retry limit and no delay of its own, and after `block_single_query_retry_count` failures it sends one `eth_getLogs` per watched topic. A permanent failure becomes about `10 × topics` requests per second.

Both were reproduced on `31baeae8` with a mocked-provider test: ten `-32000 "unknown block"` failures in a row produce ten identical errors, and no `eth_getBlockByNumber` request is ever sent.

The work is guided by three constraints:

- **No extra RPC requests in the happy case.** Every new request or delay happens only on a path that has already failed.
- **Keep the extra requests on failure paths small and bounded.** Revalidation is time-gated, so a stuck block costs at most one `eth_getBlockByNumber` per block time, and in some cases one extra immediately on a recognized error. The backoff lowers the request rate during long failures.
- **Detect new events as soon as possible.** Nodes are expected to sometimes expose a block before they can serve its logs. That short lag must still be retried as quickly as today. Backoff only grows after the lag has clearly gone on longer than expected.

The phases:

| Phase | PR | Purpose | Depends on |
| --- | --- | --- | --- |
| 1 | A | Detect an uncled in-flight block regardless of the node's error code | — |
| 2 | B | Bounded exponential backoff for the driver's watcher retries, plus a failure gauge | — (parallel with 1) |
| 3 | C (optional) | Don't fall back to one query per topic when splitting the query can't help | 1 |
| 4 | D (optional) | Make the block watcher's "wait a whole block time" fallback actually wait after a rewind | — (parallel with all) |
| 5 | — | Remove this spec | all landed phases |

---

## Architecture Decision

### The block watcher, not the error code, decides whether a block was uncled

`BlockWatcher::revalidate_last_block` (one `eth_getBlockByNumber`) is already the source of truth for whether the last emitted block is still canonical. It already treats both "different hash at this height" and "no block at this height" as uncled. The only client-specific part is deciding _when_ to call it. This epic separates that decision from the error code:

- **Revalidation only runs on a failed by-hash log query for the in-flight new block** (`Step::Block`). `EventWatcher` gets a small read-only accessor that returns the hash and timestamp of the block it is fetching, or `None` while idle or warping. This check is required, not only an optimization: `on_block_invalidated` rejects any hash that is not the in-flight `Step::Block`, so revalidating during a warp failure could rewind the block watcher while the event watcher refuses to follow.
- **Revalidation is time-gated, not run on every failure.** For a given in-flight block, revalidation is allowed:
  - **right away** if the error is one we recognize as "this block is unknown". This is the existing `-32001` check, extended with geth's `-32000` + `"unknown block"` message. The check is only a hint that lets a known client recover with no added delay. Correctness does not depend on it.
  - **otherwise once the next block is due**: when wall-clock time reaches `block.timestamp + block_time + block_propagation_delay`. This is the same moment the block watcher would poll for the next block anyway. A reorg that uncles the in-flight block is visible on the node by then, and a short logs lag should be over.
  - **after that, at most once per `block_time`** for the same block hash, whatever the error. This covers load-balanced RPCs whose backends briefly disagree, where a revalidation reports the old hash as canonical even though it is gone on another backend.

  So a stuck block costs at most one extra `eth_getBlockByNumber` per block time. A recognized error adds at most one more immediately. The happy path sends no extra request, because revalidation only runs inside the failure branch.

- **A canonical block's logs are never skipped.** If revalidation says the block is still canonical, the error goes up and the block is retried. Skipping a canonical block's logs would silently drop events, which is a security problem for both services. Escalation for this case comes from the driver's failure gauge and error log (phase 2), not from giving up on the block.

Wall-clock time comes from the existing `index::clock::Clock`, which is already compatible with paused tokio time in tests. `Watcher` keeps the last revalidation time for the current block hash. A new block resets it.

### Backoff lives in the driver, starts flat, and is capped by the block time

The driver is the only place that retries watcher errors, so the backoff lives there, as a small reusable `Backoff` type with unit tests. The schedule follows the event-detection constraint:

- **Flat start.** The first few retries (default 4) keep today's 100 ms delay. That keeps today's detection latency for the expected "block known before its logs" lag, which usually clears within a few hundred milliseconds.
- **Then exponential.** The delay doubles on each consecutive failure after that.
- **Capped at `block_time`.** The watcher never falls more than about one block behind because of backoff. A permanent failure drops from about 10 requests per second to about one per block time (5 s on Gnosis, 12 s on Sepolia).
- **Reset on any successful `watcher.next()`.**

The flat-start count and the base delay are constants, not new config fields. The cap uses the resolved `block_time` that the block watcher already has (exposed through a `Watcher` accessor), so it follows chain-specific or configured timings.

Observability: each retry logs at `warn` with the attempt count and the next delay. When the delay reaches the cap, the driver logs once at `error`, so operators get alerted while there is still quota left. A new gauge, `safenet_core_watcher_consecutive_failures`, is set on each failure and reset to 0 on success.

### Alternatives Considered

- **Only extend `is_resource_not_found` to match `-32000 "unknown block"`.** This is the smallest diff, but it depends on the client and its message text. The next node implementation, or a hosted proxy that rewrites errors, would hit the same deadlock. It is kept, but only as a fast-path hint on top of the time-gated rule.
- **Revalidate on every `Step::Block` failure (the report's proposal A).** This doesn't depend on the client, but it adds one `eth_getBlockByNumber` to every retry, including during the expected logs lag and during rate limiting, when extra requests are most expensive. The time gate gives the same guarantee for at most one request per block time.
- **Detect the reorg by fetching the next block instead of revalidating.** If the pending block's `parent_hash` does not match the in-flight hash, the in-flight block was uncled. That costs the same one `eth_getBlockByNumber`, but it moves the block watcher's state forward from outside its normal `next()` flow and doesn't work while the next block doesn't exist yet. Revalidation already exists, is tested, and has no such gap.
- **Bound the per-block retry with a distinct `BlockLogsUnavailable` error (the report's proposal C as written).** Its job was to force revalidation as a safety net. The time gate already guarantees periodic revalidation, so that job is covered. Phase 3 does a different, request-saving change instead.
- **Jitter in the backoff.** Each service instance talks to its own RPC endpoint, so there is no thundering herd for jitter to break up. Adding it would mean a new dependency (`rand`) in `safenet-core` without a real benefit. This can be revisited if operators start sharing endpoints.
- **Add retry or rate-limit handling in the provider stack** (`provider/mod.rs`). This would affect every RPC call (transaction submission, effects), not just the indexer, and would stack retries on top of the driver's. It is out of scope. The driver backoff already covers indexer `429`s.

---

## Tech Specs

### Phase 1: uncled-block detection (`index/mod.rs`, `index/events.rs`, `index/blocks.rs`)

- `EventWatcher::in_flight_block(&self) -> Option<(B256, u64)>`: hash and timestamp of `Step::Block`, otherwise `None`.
- `BlockWatcher::timings(&self) -> &Timings` (or narrower `block_time()` and `block_propagation_delay()` getters), so the watcher can compute the gate.
- `Watcher` gets a `Clock` and `last_revalidation: Option<(B256, u64 /* ms */)>`.
- `Watcher::next_logs` failure branch:
  1. If `events.in_flight_block()` is `None`, return the error. This keeps today's warp behavior.
  2. If the gate is closed (not a recognized error, before the next block is due, or revalidated less than `block_time` ago), return the error.
  3. Otherwise record the revalidation time and call `revalidate_last_block()`. `Some(invalidated)` goes to `on_block_invalidated` + `Ok(None)` (unchanged). `None` returns the original error.
- `is_unknown_block(err)` replaces `is_resource_not_found`: `-32001`, or `-32000` whose message contains `"unknown block"` (case-insensitive). It is documented as a latency hint only.

Tests (`index/mod.rs`, paused tokio time, mocked `Asserter`; each asserts the mock queue is fully drained so any extra request fails the test):

- `-32000 "unknown block"` on an uncled block: immediate revalidation, then `BlockUpdate::Uncle`, then the replacement block's `New` and its logs. This is the report's repro, changed to assert recovery.
- Unrecognized error (for example `-32603`) on an uncled block: no revalidation before the next block is due. After paused time is advanced past it, exactly one revalidation, then `Uncle`.
- Logs lag on a canonical block (`-32000 "unknown block"` followed by success): one revalidation that reports the block canonical, the error is returned, the next attempt delivers the logs, and no further revalidation happens.
- Repeated failures on a canonical block: at most one revalidation per `block_time`.
- Failure while warping: no revalidation (the queue holds only the warp's log responses).
- The existing `emits_block_updates_then_their_logs` and `recovers_from_an_uncled_block_with_unavailable_logs` keep passing unchanged. The first is the "no extra requests on the happy path" check.

### Phase 2: driver backoff (`driver.rs`, `metrics.rs`, `index/mod.rs`)

- `Watcher::block_time(&self) -> Duration`, delegating to the block watcher's timings. If phase 1 has landed, reuse its accessor.
- A private `Backoff { base, flat_attempts, cap, attempt }` with `next_delay(&mut self) -> Duration`, `reset(&mut self)` and `attempts()`. It lives in `driver.rs`, or in `utils.rs` if a second user shows up.
- `STEP_RETRY_DELAY` is replaced by `RETRY_BASE_DELAY = 100 ms` and `RETRY_FLAT_ATTEMPTS = 4`. The cap is `watcher.block_time()`.
- `next_input` keeps the `Backoff` on the `Driver`, not inside the loop, so a pending effect winning the `select!` doesn't reset it. It resets on `Ok`, sleeps `next_delay()` on `Err`, logs `warn` with `attempt` and `delay_ms`, and logs once at `error` when the cap is first reached.
- `metrics::watcher_consecutive_failures() -> Gauge` (`safenet_core_watcher_consecutive_failures`).

Tests: `Backoff` unit tests for the schedule (flat part, doubling, cap, reset). There are no driver-loop tests today, and the schedule is the behavior worth pinning, so no driver harness is added.

### Phase 3 (optional): no topic fan-out on unknown-block errors (`index/events.rs`)

`EventWatcher::block` switches to `Fetch::MultipleQueries` after `block_single_query_retry_count` failures. Splitting the query helps with oversized responses and timeouts. It cannot help when the node doesn't know the block at all, and it multiplies requests by the number of topics. When the previous attempt failed with an error that `is_unknown_block` recognizes (moved to `events.rs` and shared with phase 1), keep using a single query. The retry counter still increases, so other errors still escalate as today.

Tests (`index/events.rs`, in the style of `new_block_falls_back_to_multiple_queries_after_retries`): repeated unknown-block failures send one query per attempt. Repeated other failures still fan out.

### Phase 4 (optional): a real wait after a rewind (`index/blocks.rs`)

After a rewind (`next()` reorg detection and `revalidate_last_block`), `pending.timestamp_ms` is the uncled block's timestamp, which is in the past. That's intentional: the replacement block is fetched right away, which serves the event-detection constraint, and that should stay as it is. But if the replacement isn't served and `block_retry_delays` runs out, the "wait a whole block time" branch only adds `block_time` to a timestamp that is still in the past, so `wait_for_pending_block` returns immediately until it catches up with the wall clock. With `block_retry_delays = []` there is no sleep at all.

Change: in the exhausted-retries branch, move `timestamp_ms` forward in `block_time` steps until `timestamp_ms + block_propagation_delay` is in the future. That keeps the poll aligned with the slot grid, so it lands when the next slot's block is expected and not at an arbitrary later time. When the watcher isn't behind, this is a single step, the same as today. The rewind itself is unchanged.

Tests (`index/blocks.rs`, paused time): after an uncle whose replacement isn't served, with empty retry delays, the next `eth_getBlockByNumber` happens only at the next slot time, not immediately. The existing skipped-slot test keeps its cadence.

### Not changed

- The `ExceededMaxReorgDepth` handling and the shutdown behavior.
- Config schema: no new fields in phases 1 and 2. All new tunables are constants, which keeps `deny_unknown_fields` configs and `scripts/` working unchanged.
- The `scripts/run_validator_integration_test.sh` integration test. Anvil 1.5.1 returns `{"result": []}` for `eth_getLogs` with an unknown `blockHash`, not an error. An `anvil_reorg` test would pass on `main` and wouldn't cover this bug, so the mocked-provider tests are the regression tests.

---

## Implementation Phases

### Phase 1: detect an uncled in-flight block regardless of error code (PR A)

- Files: `crates/core/src/index/mod.rs`, `crates/core/src/index/events.rs`, `crates/core/src/index/blocks.rs`.
- Flows: uncled block with recognized error, uncled block with unrecognized error, logs lag, repeated failure on a canonical block, warp failure.
- Size: about 80 lines of logic, about 150 lines of tests.
- Fixes the reported deadlock on its own. Can be merged and released without the other phases.

### Phase 2: bounded exponential backoff for watcher retries (PR B), in parallel with phase 1

- Files: `crates/core/src/driver.rs`, `crates/core/src/metrics.rs`, `crates/core/src/index/mod.rs` (`block_time()` accessor; if phase 1 lands first, this is a trivial rebase onto its timings accessor).
- Flows: every watcher failure (RPC down, rate limited, stuck block, warp failure).
- Size: about 60 lines of logic, about 60 lines of tests.

### Phase 3 (optional): no topic fan-out on unknown-block errors (PR C), after phase 1

- Files: `crates/core/src/index/events.rs` (plus moving `is_unknown_block` out of `index/mod.rs`).
- Only worth doing if a deployment watches enough topics for the fan-out to matter. Decide after phases 1 and 2 have run in production.

### Phase 4 (optional): real wait in the exhausted-retries branch after a rewind (PR D), in parallel with all

- Files: `crates/core/src/index/blocks.rs`.
- The window is bounded today. This only removes a short burst of unthrottled `eth_getBlockByNumber` polling after a rewind whose replacement isn't served.

### Phase 5: remove this spec

- Delete `epics/2026_11_03_indexer_uncled_block_recovery_and_backoff.md` once phases 1 and 2, and any optional phases that were done, have landed. Note in the PR any optional phases that were dropped.

---

## Open Questions and Assumptions

- **Assumption: logs lag is short.** "Block known before its logs" is assumed to clear within a few hundred milliseconds, which is about what the 4 × 100 ms flat start covers. If operators see longer lags, increase `RETRY_FLAT_ATTEMPTS` rather than adding a config field.
- **Assumption: a node that lags on logs returns an error, not `[]`.** A node that answers `[]` for a block whose logs aren't indexed yet would make the watcher silently miss events. That is a separate, existing risk that `use_client_filtering` (bloom check) already covers, and this epic doesn't change it. With client filtering on, an uncled block answered with `[]` fails the bloom check (`IncompleteLogs`). Phase 1's time gate also recovers that case, which today only recovers after the multiple-queries fallback.
- **Assumption: block timestamps are close to wall-clock time.** The "next block is due" gate compares `block.timestamp` with the local clock. Clock skew only moves the first revalidation earlier or later. After that, the once-per-`block_time` limit applies either way, so the cost stays bounded.
- **Open: geth message matching.** Should the fast path match only `"unknown block"`, or also `"block not found"` / `"not found"`, which some proxies use? A wider match costs at most one early `eth_getBlockByNumber` per failing block. A narrower match only delays recovery until the next block is due.
- **Open: where the backoff cap comes from.** This plan caps at `block_time`. On very fast chains (block time under 1 s), the cap may be lower than the doubling ever reaches, so the backoff stays flat. If that matters, use `max(block_time, 1 s)`.
