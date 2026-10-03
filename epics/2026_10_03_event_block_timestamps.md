# Plan: Event block timestamps

Component: `crates/core` (Cargo package `safenet-core`) — the `index::events` and `index::blocks` modules. Downstream consumers of `EventLog` in `crates/sentinel` and `crates/validator` are updated mechanically.

---

## Overview

The event watcher takes each log's block timestamp from the `blockTimestamp` field of the node's `eth_getLogs` response. That field is non-standard: widely supported, but not by every RPC provider. When it is missing, `EventLog::block_timestamp` is `None`, and the sentinel silently drops the `x-proposal-timestamp` header it sends to its sentinel engine (see [`docs/sentinel-engine.md`](../docs/sentinel-engine.md)). So the same sentinel, watching the same chain, hands its engine different information depending on which RPC provider it is configured with, and any check relying on the proposal timestamp abstains on some providers but not on others.

This epic makes the event watcher always produce a block timestamp, falling back to block headers when the node does not include one with its logs:

1. **Refactor** — group a log's block position into an `EventBlock { number, timestamp }` type.
2. **Timestamp fallback** — `decode_and_sort` takes a map of known block timestamps to fill in what the logs leave out. New blocks pass the timestamp of the header the block watcher already fetched.
3. **Warp support** — warped pages fetch the headers of the blocks whose logs are missing a timestamp.
4. **Resumable warp pages** — the warp state keeps the logs and timestamps fetched so far, so an intermittent RPC failure only retries what is missing instead of the whole page.
5. **Degraded-performance warning** — a one-time per-indexer warning when the node omits log timestamps.
6. **Required timestamps** — with every path guaranteed to produce a timestamp, `EventBlock::timestamp` becomes a `u64`, and the sentinel always sends `x-proposal-timestamp`.

Phases are sequential up to 3. Phases 4, 5 and 6 each depend only on 3 and can be done in parallel (they touch different parts of the code, though 4 and 5 both touch the warp path in `events.rs`, so whichever lands second rebases on the first).

---

## Architecture Decision

### Fill in missing timestamps from block headers

Every `eth_getLogs` response identifies a log's block by number and hash, so a missing timestamp can always be recovered with `eth_getBlockByNumber`. The watcher does this inside `EventWatcher`, so every consumer of `Update::Logs` gets a timestamp regardless of the node, and no consumer has to know about the fallback.

The timestamps are passed to `decode_and_sort` as a `block_timestamps: BTreeMap<u64, u64>` keyed by block number. Keying by number (rather than hash) is safe in both places the map is built:

- **New blocks** are fetched by hash, and the block watcher already holds that block's header, so the map is a single entry built from it — no extra RPC request.
- **Warps** only cover the reorg-safe range, so a block number identifies a single canonical block.

A log that carries its own `blockTimestamp` always uses it; the map is only consulted for logs that do not. Headers are only requested for blocks that actually contain a watched log missing its timestamp, so nodes that include `blockTimestamp` pay nothing, and nodes that omit it pay at most one header request per block with watched logs.

### New blocks carry their timestamp

`BlockUpdate::New` gains a `timestamp` field, populated from the header the block watcher fetched to produce the update. `Step::Block` keeps it alongside the hash and bloom so retries of the same block reuse it.

### Warp pages become resumable

Today a warp page is a single `eth_getLogs` call: on failure the page is halved and retried. Adding header requests to a page (phase 3) makes a page several requests, and failing one of the header requests would throw away the logs and every header already fetched — and halve the page size, even though the logs query succeeded. Phase 4 adds a `fetched: Option<FetchedPage>` field to `Step::Warping` holding the current page's logs and the timestamps resolved so far, so a page is fetched in two steps within the same state:

1. While `fetched` is `None`, fetch the page's logs, with the existing halve-on-failure behaviour.
2. Once `fetched` is set, fetch only the missing block timestamps for those logs. A failure keeps the logs and resolved timestamps and retries only the missing headers, without touching the page size.

Holding a `Vec<Log>` means `Step` is no longer `Copy`; `next` takes the step out of `self` (`std::mem::replace`) instead of copying it.

### Required timestamps

Once phases 2 and 3 land, every log produced by the watcher has a timestamp, and a log for which neither the node nor the map provides one is an error (`Error::MissingBlockTimestamp`, which can only happen through a bug in the watcher or an inconsistent node). Phase 6 encodes that in the type: `EventBlock::timestamp: u64`, and the sentinel's `Effect::EngineCheck::proposal_timestamp` becomes a `u64` so it always sends `x-proposal-timestamp`. The header stays optional in the sentinel engine's HTTP contract, since third-party callers may still omit it.

### Alternatives Considered

- **Always fetch headers for every block with logs.** Simpler, but it penalizes the common case of nodes that do return `blockTimestamp`, and `eth_getLogs` responses already carry what we need for them.
- **Interpolate timestamps from block numbers and block time.** No extra requests, but wrong on chains with variable block times or missed slots, and a wrong timestamp is worse than a missing one for a security check.
- **Leave the timestamp optional and let consumers fetch it.** Pushes the same RPC logic into every consumer and keeps the provider-dependent behaviour this epic is meant to remove.
- **Fetch headers with a JSON-RPC batch request.** Fewer round trips, but not every provider supports batches (or batches of arbitrary size), and it would be a new code path in `Provider`. Concurrent `eth_getBlockByNumber` requests reuse the existing provider, and phase 4 makes partial failures cheap.
- **Key timestamps by block hash.** Warps query by range, so we would have to fetch headers by hash taken from each log — equivalent in practice since the range is reorg-safe, but keying by number keeps the map independent of the shape of individual logs.

---

## Tech Specs

### Types (`crates/core/src/index/events.rs`)

```rust
/// The block an event log was emitted in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventBlock {
    /// The block number.
    pub number: u64,
    /// The block timestamp, in seconds since the Unix epoch.
    pub timestamp: Option<u64>, // `u64` from phase 6
}

pub struct EventLog<E> {
    pub block: EventBlock,
    pub index: u64,
    pub address: Address,
    pub data: E,
}
```

`EventUpdate`, `Update` and the `(block, index)` ordering are unchanged; ordering uses `block.number`.

### `decode_and_sort`

```rust
fn decode_and_sort<E>(
    logs: &[Log],
    block_timestamps: &BTreeMap<u64, u64>,
) -> Result<Vec<EventLog<E>>, Error>
```

The timestamp of a log is `log.block_timestamp.or_else(|| block_timestamps.get(&number).copied())`. Until phase 6 an unresolved timestamp stays `None`; from phase 6 it is `Error::MissingBlockTimestamp { block_number }`.

### Block updates (`crates/core/src/index/blocks.rs`)

`BlockUpdate::New { number, hash, timestamp, logs_bloom }`, with `timestamp` taken from the fetched `BlockHeader`.

### Warp state

After phase 4, `Step::Warping` holds the current page's progress in a `fetched` field rather than adding another state:

```rust
enum Step {
    Idle,
    /// Warping over the reorg-safe range `from_block..=to_block` in pages of
    /// `page_size` blocks.
    Warping {
        from_block: u64,
        to_block: u64,
        page_size: NonZeroU64,
        /// The current page, once its logs have been fetched.
        fetched: Option<FetchedPage>,
    },
    Block { .. },
}

/// A warp page whose logs have been fetched, but some of whose block
/// timestamps are still missing.
struct FetchedPage {
    /// The last block of the page, `from_block..=to_block`.
    to_block: u64,
    logs: Vec<Log>,
    block_timestamps: BTreeMap<u64, u64>,
}
```

With `fetched: None`, `warp` queries the page's logs as today (halving `page_size` on failure). With `fetched: Some(..)`, it skips the logs query and only requests the headers missing from `block_timestamps`. A page whose logs all carry `blockTimestamp` (or that has no logs) never sets `fetched`, and `fetched` is reset to `None` when advancing to the next page. Header requests for a page are issued concurrently; each one that succeeds is recorded in `block_timestamps` before the failure (if any) is returned, so the retry only requests what is still missing. A header that the node reports as missing for a reorg-safe block is an RPC error and is retried like any other failure.

### Warning

`EventWatcher` gets a `warned_missing_timestamps: bool`. The first time a fetch produces a watched log without `blockTimestamp`, it logs once at `warn` level, e.g.:

> node does not include `blockTimestamp` in logs; falling back to fetching block headers, which requires additional RPC requests and degrades indexing performance

The flag is per `EventWatcher`, so each indexer (the validator's and the sentinel's) warns at most once per process.

### Test cases

Extend the existing `events.rs` test module (and `blocks.rs` for the new `timestamp` field):

- `decode_and_sort` prefers the log's timestamp, falls back to the map, and (phase 6) errors when neither has one.
- A new block's events use the header timestamp when the node omits `blockTimestamp`, without any extra request.
- A warped page whose logs omit `blockTimestamp` fetches one header per distinct block with logs, and none when the logs carry timestamps.
- (Phase 4) A header failure keeps the fetched logs and resolved timestamps, does not change the page size, and the retry only requests the missing headers.
- (Phase 5) The warning is logged once across multiple pages/blocks.

---

## Implementation Phases

### Phase 1 — `EventBlock` refactor

Pure refactor, no behaviour change.

- Add `EventBlock { number, timestamp: Option<u64> }` to `crates/core/src/index/events.rs` and replace `EventLog::{block, block_timestamp}` with `EventLog::block: EventBlock`; export it from `index/mod.rs`.
- Update `decode_and_sort`, the ordering checks in `crates/core/src/state/mod.rs`, and the tests in `core`.
- Update consumers: `crates/validator/src/state/mod.rs` (`log.block` → `log.block.number`) and `crates/sentinel/src/service.rs`.

### Phase 2 — Timestamp fallback for new blocks

Depends on phase 1.

- Add `timestamp` to `BlockUpdate::New` (`crates/core/src/index/blocks.rs`) and to `Step::Block` in `events.rs`; update the places that construct `BlockUpdate::New` (`blocks.rs`, `driver.rs`, `index/mod.rs`, `state/mod.rs` and tests).
- Add the `block_timestamps: &BTreeMap<u64, u64>` parameter to `decode_and_sort`. `fetch_logs` passes it through; new blocks pass a one-element map from the block update, warps pass an empty map for now.
- Tests: `decode_and_sort` fallback, and new-block events getting the header timestamp when the node omits it.

### Phase 3 — Timestamps for warped events

Depends on phase 2.

- In `EventWatcher::warp`, after fetching a page, collect the distinct block numbers of logs without `blockTimestamp`, fetch their headers concurrently via the provider, and pass the resulting map to `decode_and_sort`.
- A failed header request fails the page like a failed logs query does today (phase 4 refines this).
- Tests: a warped page with and without node-provided timestamps, asserting the number of header requests.

### Phase 4 — Resumable warp pages

Depends on phase 3; parallel with phases 5 and 6.

- Add `fetched: Option<FetchedPage>` to `Step::Warping` as described in [Warp state](#warp-state); drop `Copy` from `Step` and take it out of `self` in `next`.
- Header failures keep the fetched logs and partial timestamps and do not halve the page size.
- Tests: a header failure followed by a successful retry that only requests the missing headers, with the page size unchanged.

### Phase 5 — One-time warning

Depends on phase 3; parallel with phases 4 and 6.

- Add `warned_missing_timestamps` to `EventWatcher` and log the warning described in [Warning](#warning) the first time a watched log without `blockTimestamp` is seen.
- Tests: only if the existing test setup can capture logs without new test infrastructure; otherwise this is verified manually against a node that omits `blockTimestamp`.

### Phase 6 — Required timestamps

Depends on phase 3; parallel with phases 4 and 5.

- `EventBlock::timestamp` becomes `u64`; `decode_and_sort` returns `Error::MissingBlockTimestamp` when neither the log nor the map provides one. Update `core` tests and the `EventLog` constructions in `state/mod.rs` and `index/mod.rs` tests.
- `crates/sentinel`: `handle_oracle_transaction_proposed` and `Effect::EngineCheck::proposal_timestamp` take a `u64`, and `effect.rs` always sets the proposal timestamp on the engine check.
- Docs: update the `x-proposal-timestamp` paragraph in [`docs/sentinel-engine.md`](../docs/sentinel-engine.md) — the Safenet sentinel always sends it, but the header stays optional in the engine contract for other callers. `openapi.yaml` is unchanged.

### End — Remove this spec

A separate cleanup PR deleting this file once all phases have landed.

---

## Open Questions and Assumptions

- **Assumption:** every node that omits `blockTimestamp` from logs still serves `eth_getBlockByNumber` for blocks in the reorg-safe range. A node that cannot is treated as failing, and the warp retries indefinitely as it does today for failed log queries.
- **Assumption:** the number of distinct blocks with watched logs in a warp page is small enough to request their headers concurrently without a separate concurrency limit. Page size (`block_page_size`, default 100) bounds it.
- **Open question:** should a log whose `blockTimestamp` disagrees with the fetched header be treated as an error? Since headers are only fetched for logs that lack a timestamp, this case does not arise, and this plan does not cross-check.
