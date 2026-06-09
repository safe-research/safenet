# Plan: Implement the `safenet-core` shared crate

Component: `crates/core` (Cargo crate `safenet-core`)

---

## Overview

The Rust port of the Safenet offchain services needs a shared foundation crate. The
`safenet-core` crate already exists as a stub (`crates/core/src/lib.rs`) whose doc comment
names the four pillars this epic delivers:

1. **Observability** — a default logging setup built on `tracing` / `tracing-subscriber`,
   plus Prometheus metrics.
2. **Indexing** — following the chain head and fetching event logs in order, with chain-reorg
   detection. This is a port of the TypeScript validator's `validator/src/watcher/` subsystem
   (`index.ts`, `blocks.ts`, `events.ts`). Note that `backoff.ts` is **not** ported: RPC
   rate-limiting backoff is delegated to `alloy`'s transport layers (see below).
3. **Reorg-aware state storage** — a general SQLite state store that, _in addition_ to
   persisting service state across restarts, keeps a **per-block snapshot history** so state can
   be **rolled back on a reorg**. This is **net-new design**: the TS validator detects reorgs but
   explicitly does _not_ roll back (`validator/src/shared/watcher.ts` logs
   `"Reorg detected, but currently not supported."`).
4. **Transaction submission** — reliable onchain transaction submission with nonce management,
   fee bumping and resubmission. This is a port of
   `validator/src/consensus/protocol/transaction.ts` (`TransactionManager`) and its SQLite backing
   (`validator/src/consensus/protocol/sqlite.ts`). Fee estimation itself is delegated to `alloy`'s
   built-in mechanism rather than ported (see below).

The crate is a **library** consumed by future Rust offchain services. It is **not** wired into
the existing TypeScript validator (no FFI); the TS validator remains the reference
implementation during the port.

The work is delivered across six phases (A–F). Phase A is foundational scaffolding that
everything else depends on. Phases B (observability), C (indexing), D (state) and E (transaction
submission) are largely independent of one another and can proceed in parallel after A. Phase F
is documentation/wrap-up.

---

## Architecture Decision

The crate targets an `async` (tokio) runtime and is built on the following stack, chosen to be
the idiomatic Rust analogs of the TypeScript dependencies and to **reuse existing,
well-maintained code rather than re-implementing primitives** (per `AGENTS.md` coding guidelines):

| Concern | TypeScript today | Rust choice | Notes |
| --- | --- | --- | --- |
| RPC client, primitives, signing | `viem` | **`alloy`** | `Provider`, `Address`/`B256`/`U256`/`Bytes`, `Bloom`, `Log`, EIP-1559 tx types, `PrivateKeySigner`, typed events via `sol!`. De-facto standard. |
| RPC rate-limit backoff | custom `backoff.ts` | **`alloy` transport layers** | `RetryBackoffLayer` (+ `RateLimitRetryPolicy`), optionally `FallbackLayer` / `ThrottleLayer`. No hand-rolled backoff. |
| Fee estimation | custom `GasFeeEstimator` | **`alloy` built-in** | `Provider::estimate_eip1559_fees` / recommended fillers (`GasFiller`). |
| SQLite | `better-sqlite3` (sync) | **`sqlx`** (async, `sqlite`) | Connection pool, async queries, embedded migrations. |
| Logging | `winston` | **`tracing` + `tracing-subscriber`** | Explicit requirement. |
| Metrics | `prom-client` | **`metrics` + `metrics-exporter-prometheus`** | Prometheus exporter over HTTP. |
| Config validation | `zod` | (consumer concern) | Out of scope; each component takes a plain config struct. |
| Errors | viem `BaseError` chain | **`thiserror`** | Typed error enums per module. |

Key architectural decisions:

- **Async-first.** `alloy`'s `Provider` and `sqlx` are async, so the crate is async throughout
  and runs on `tokio`. As a library it does not start a runtime; consumers own that.

- **RPC backoff is delegated to `alloy` transport layers — not hand-rolled.** Rather than porting
  the TS `Backoff` (`backoff.ts`: exponential delays + RPC-error classification), the crate
  configures the `Provider` with `alloy`'s `RetryBackoffLayer` (driven by `RateLimitRetryPolicy`),
  which retries rate-limited / 429 / transient transport errors with backoff at the transport
  level. A thin provider-builder helper (`rpc.rs`) wires up these layers. Two retry concerns that
  are **not** rate-limiting remain inside the indexer and are intentionally simple: (a) the
  BlockWatcher's "wait for the next block to be mined/propagated" poll (`block_retry_delays`), and
  (b) the stale-logs / bloom-mismatch case (RPC returned a successful-but-incomplete response),
  which is handled by simply re-polling on the next loop iteration — no custom error classifier.

- **Fee estimation is delegated to `alloy` — not ported.** The TS `GasFeeEstimator` (caching +
  manual EIP-1559 math + priority-fee cap) exists to work around `viem` API limitations. `alloy`'s
  `Provider::estimate_eip1559_fees` (and the recommended `GasFiller`) cover this directly, so the
  estimator is dropped. The `TransactionManager` retains only the **replacement-fee bump**
  (`max(fresh_estimate, previous × 1.1)`) for resubmissions, since alloy does not bump replacement
  transactions automatically.

- **Typed indexer over `alloy` `sol!` events.** The indexer is generic over a typed event set
  generated by `alloy`'s `sol!` macro and decoded via `SolEventInterface` (an event enum such as
  `ContractEvents`). It derives the watched event `topic0` set from that type, fetches logs, and
  emits **decoded, typed** events in strict `(block_number, log_index)` order. This is the typed
  analog of the TS `Watcher<E>` (generic over an `AbiEvent[]` runtime list) and gives consumers
  compile-time-checked event handling.

- **Reorg-aware state store = full per-block snapshots.** The generic state store is parameterised
  over a serializable state value `S`. On each indexed block the store persists `S` keyed by block
  number (and block hash). On a reorg to a common-ancestor block `M`, the store restores the
  snapshot taken at `M` and drops snapshots `> M`. Snapshots older than `tip − maxReorgDepth` are
  pruned. This trades storage (`state_size × reorgDepth`) for a trivially-correct, easy-to-verify
  rollback — appropriate given the small bounded state of these services and the lean-validator
  ethos (the handbook targets < 500 MB RAM and `maxReorgDepth` defaults to 5).

- **Transaction-submission storage is NOT reorg-rolled-back.** Submitted-transaction state
  (nonces, pending/executed) is keyed by nonce, not by block, and must survive reorgs unchanged —
  you never want to "un-submit" a transaction. It lives in its own tables, separate from the
  snapshot store, exactly as in the TS implementation. The two storage concerns share only the
  SQLite connection/migration foundation.

- **Reuse `alloy_primitives::Bloom`.** The TS `utils/bloom.ts` hand-rolls a 2048-bit bloom filter.
  `alloy_primitives::Bloom` already provides membership operations, so the port reuses it and only
  adds thin helpers (block-bloom membership check for the watched address + typed-event-`topic0`
  set).

- **Runtime-checked sqlx queries.** Use `sqlx::query`/`query_as` (runtime-checked) rather than the
  compile-time `query!` macros, so the build stays hermetic (no live DB or `.sqlx` metadata
  required in CI). Schema lives in embedded `crates/core/migrations/` and is applied on startup by
  a migration runner — closing the gap that TS migrations are currently applied only in tests.

- **Configurable metric prefix.** TS metrics are hard-prefixed `validator_`. Because multiple Rust
  services share this crate, the metrics module takes the prefix/namespace as a parameter.

### Alternatives Considered

- **Hand-rolling RPC backoff (porting `backoff.ts`).** **Rejected** — `alloy`'s `RetryBackoffLayer`
  / `RateLimitRetryPolicy` already implement rate-limit retry with backoff at the transport layer.

- **Porting the custom `GasFeeEstimator`.** **Rejected** — `alloy`'s built-in fee estimation /
  fillers cover it; the TS estimator only existed to work around `viem` API limitations.

- **Untyped log-filter indexer.** Watch `(address, topic0)` filters and emit raw `alloy` `Log`s,
  leaving decoding to the consumer. Simpler generics, but loses compile-time type safety.
  **Rejected** in favour of the typed `sol!`/`SolEventInterface` indexer.

- **`rusqlite` (sync) instead of `sqlx`.** Closer to `better-sqlite3` and gives the tightest
  control over multi-statement atomic transactions, but forces `spawn_blocking`/dedicated-thread
  plumbing inside an otherwise-async (alloy) stack. **Rejected** in favour of async-native `sqlx`.

- **Undo-journal or MVCC validity-range reorg storage.** A block-scoped undo journal or MVCC rows
  (`[from_block, to_block)` validity ranges) are more storage-efficient than full snapshots.
  **Rejected** for now: full snapshots are the simplest to reason about and verify, and the bounded
  state size makes the storage cost acceptable. The store API is written so a more efficient
  backend could be swapped in later without changing callers.

- **`ethers-rs` instead of `alloy`.** Deprecated in favour of `alloy`. **Rejected.**

---

## Tech Specs

### Crate layout

```
crates/core/
  Cargo.toml                 # workspace deps; lockfile committed (Cargo.lock)
  migrations/                # sqlx embedded migrations (state foundation, tx storage)
  src/
    lib.rs                   # module declarations + crate docs
    error.rs                 # crate-level error type(s) (thiserror)
    types.rs                 # BlockRef { number, hash }, re-exports of alloy primitives
    rpc.rs                   # Provider builder wired with RetryBackoffLayer (+ optional layers)
    observability/
      mod.rs
      logging.rs             # tracing-subscriber default init
      metrics.rs             # prometheus exporter + metric handles + /metrics + /health
    index/
      mod.rs                 # Watcher orchestration + watch_blocks_and_events (typed over E)
      bloom.rs               # thin helpers over alloy_primitives::Bloom
      blocks.rs              # BlockWatcher (chain head + reorg detection)
      events.rs              # EventWatcher (typed log fetch/decoding + range-warp state machine)
    state/
      mod.rs
      db.rs                  # sqlx SqlitePool setup, pragmas, migration runner
      snapshot.rs            # reorg-aware per-block snapshot state store
    tx/
      mod.rs
      account.rs             # Account trait (address + sign_transaction)
      storage.rs             # TransactionStorage (sqlx-backed nonce/status store)
      manager.rs             # TransactionManager (submit / resubmit / pending-check loop)
```

### RPC provider (`rpc.rs`)

- Thin helper that builds an `alloy` `Provider` with `RetryBackoffLayer` (rate-limit retry with
  backoff via `RateLimitRetryPolicy`), and optionally `FallbackLayer` / `ThrottleLayer`. This is
  where the "delegate backoff to alloy" decision lives; both the indexer and the transaction
  manager consume a provider built this way. Consumers may still supply their own provider.

### Indexing (port of `validator/src/watcher/`, typed)

- **`blocks.rs`** ← `blocks.ts`. `BlockWatcher` following the head: `BlockUpdate` enum
  (`WarpToBlock { from, to }`, `UncleBlock { number }`, `NewBlock { number, hash, logs_bloom }`),
  settings (`block_time`, `max_reorg_depth`), options (`block_propagation_delay`,
  `block_retry_delays`, injectable timer), a recent-block ring buffer for parent-hash reorg
  detection, `next()`, and `revalidate_last_block()` (handles RPC nodes that report a block but
  not its logs). Includes the resume/fresh-start/warp init logic. The `block_retry_delays` poll
  ("the next block isn't available yet") stays here; it is distinct from RPC rate-limit backoff,
  which the provider's `RetryBackoffLayer` owns. Mirrors `blocks.test.ts`.
- **`bloom.rs`** ← `utils/bloom.ts`. Thin helpers on `alloy_primitives::Bloom`: "can this block
  bloom possibly contain a watched (address, topic0)?" (topic0s derived from the typed event set)
  and "compute bloom from a log set" (for the all-logs integrity check).
- **`events.rs`** ← `events.ts`, **typed**. `EventWatcher<E>` is generic over an `alloy` `sol!`
  event set (`E: SolEventInterface`). State machine (`Idle` / `Warping` / `Block`) with the
  progressive log-fetch fallback (all-logs+bloom-check → one-query-all-events →
  one-query-per-event), page-size halving on error, `fallible_events`, `max_logs_per_query`
  truncation guard, and `on_block_update` / `on_block_invalidated` / `next`. Decodes each `Log`
  into a typed `E` via `SolEventInterface::decode_log`, surfacing decode failures (except for
  `fallible_events`). Mirrors `events.test.ts`.
- **`index/mod.rs`** ← `index.ts`. `Watcher<E>` orchestration: drains logs before advancing blocks
  (guaranteeing in-order logs), `next_logs()` recovery via `revalidate_last_block` on
  resource-not-found, subscribe/start/stop, and a `watch_blocks_and_events` helper. Emits
  `Update::{ Block(BlockUpdate), NewLogs(Vec<E>) }` with typed events.

### Reorg-aware state storage (net-new)

- **`db.rs`** — `SqlitePool` creation, pragmas (`WAL`, `foreign_keys=ON`), and a migration runner
  that applies `crates/core/migrations/` on startup.
- **`snapshot.rs`** — generic `SnapshotStore<S: Serialize + DeserializeOwned>`:
  - schema: `snapshots(block_number INTEGER PRIMARY KEY, block_hash BLOB NOT NULL, state BLOB NOT NULL)`.
  - `current() -> Option<(BlockRef, S)>` (the tip snapshot — also the resume point / `lastIndexedBlock`).
  - `commit(block: BlockRef, state: &S)` — insert/replace the snapshot at `block`.
  - `rollback_to(block_number) -> S` — delete snapshots `> block_number`, return the restored tip.
  - `prune(finalized_below)` — delete snapshots at/below `tip − maxReorgDepth`.
  - all multi-statement operations wrapped in a single transaction for atomicity.

### Transaction submission (port of `transaction.ts` + `sqlite.ts`)

- **`account.rs`** — `Account` trait: `fn address(&self) -> Address` and async
  `sign_transaction(&self, tx) -> Result<Bytes>` (analog of `ValidatorAccount`). A default impl
  wraps an `alloy` `PrivateKeySigner`.
- **`storage.rs`** ← `SqliteTxStorage`. Table
  `transaction_storage(nonce INTEGER PRIMARY KEY, transaction_json TEXT NOT NULL,
  transaction_hash TEXT, created_at, submitted_at INTEGER, fees_json TEXT)` and the full method set:
  `register` (atomic `INSERT ... SELECT MAX(...) RETURNING nonce`), `count_pending`, `delete`,
  `set_pending`, `set_fees`, `set_hash`, `set_executed_up_to`, `set_submitted_for_pending`,
  `max_nonce`, `submitted_up_to`. The implicit QUEUED/PENDING/EXECUTED state machine is preserved.
  Mirrors `consensus/protocol/sqlite.test.ts`.
- **`manager.rs`** ← `TransactionManager`. `submit_action` (reserve nonce, persist intent, submit),
  `submit_transaction`, and the pending-check loop (`trigger_pending_check`, mark submitted/executed
  by on-chain nonce, resubmit stale txs, nonce-too-low handling). Fresh fees come from `alloy`'s
  `estimate_eip1559_fees` (or the provider's `GasFiller`); on resubmission the manager applies the
  replacement-fee bump `max(fresh_estimate, previous × 1.1)`.

### Testing

- Unit tests per module, mirroring the existing TS test files
  (`blocks.test.ts`, `events.test.ts`, `sqlite.test.ts`) — behaviour, not implementation details,
  per `AGENTS.md`.
- The indexer uses an injectable timer and a mock `Provider` so block/reorg/warp sequences are
  deterministic (as the TS tests already do).
- A dedicated state-store test simulates a reorg and asserts snapshot rollback + pruning.
- sqlx tests run against an in-memory SQLite (`sqlite::memory:`), matching the TS default of
  `:memory:`.

### Tooling

- Per `AGENTS.md`: `cargo fmt --all`, `cargo clippy --package safenet-core`,
  `cargo test --package safenet-core`.
- `Cargo.lock` is committed; dependency versions are pinned in PR A1.

---

## Implementation Phases

Each PR has a single purpose, targets < 300 changed LOC and < 10 files, and is independently
reviewable. Refactors, if any arise, are split into their own PRs. "Depends on" lists hard
ordering; everything else may proceed in parallel.

### Phase A — Foundation (blocks all other phases)

- **A1 — Crate scaffolding & dependencies.** Add the dependency set (alloy, sqlx, tokio, tracing,
  tracing-subscriber, metrics, metrics-exporter-prometheus, serde, serde_json, thiserror) to
  `crates/core/Cargo.toml`; commit `Cargo.lock`; declare the (empty) modules in `lib.rs`; add
  `error.rs` (crate error enum) and `types.rs` (`BlockRef` + alloy re-exports). _Single purpose:
  set up the crate._
- **A2 — RPC provider builder.** `rpc.rs`: helper that builds an `alloy` `Provider` wired with
  `RetryBackoffLayer` (+ optional `FallbackLayer`/`ThrottleLayer`). Depends on A1. Small; this is
  the embodiment of the "delegate backoff to alloy" decision and is shared by C and E.

### Phase B — Observability (depends on A1; B1 ∥ B2)

- **B1 — Logging.** `observability/logging.rs`: default `tracing-subscriber` init — `EnvFilter`
  (`RUST_LOG`/level), JSON vs pretty switched by TTY, structured fields.
- **B2 — Metrics.** `observability/metrics.rs`: Prometheus exporter, metric handles
  (block_number, event_index, reorgs, transitions, rpc_requests, …), configurable prefix, and the
  `/metrics` + `/health` HTTP server.

### Phase C — Indexing (depends on A; runs in parallel with D & E)

- **C1 — Bloom helpers.** `index/bloom.rs` over `alloy_primitives::Bloom` + tests. Depends on A1.
  _(∥ C2)_
- **C2 — BlockWatcher.** `index/blocks.rs` + tests. Depends on A1, A2. _(∥ C1)_ — if it exceeds the
  size budget, split into C2a (types + chain-follow + reorg detection) and C2b (init/resume/warp +
  `revalidate_last_block`).
- **C3 — EventWatcher (typed).** `index/events.rs` + tests; generic over a `sol!` event set via
  `SolEventInterface`. Depends on C1 — split into C3a (state machine + warp) and C3b (single-block
  fallback strategies + decoding) if needed.
- **C4 — Watcher orchestration.** `index/mod.rs` + `watch_blocks_and_events` (typed) + tests.
  Depends on C2, C3.

### Phase D — Reorg-aware state storage (depends on A1; parallel with C & E)

- **D1 — SQLite foundation.** `state/db.rs`: `SqlitePool`, pragmas, migration runner, initial
  `migrations/`. Depends on A1.
- **D2 — Snapshot store.** `state/snapshot.rs`: full per-block snapshot store
  (`current`/`commit`/`rollback_to`/`prune`) + reorg-rollback test. Depends on D1.

### Phase E — Transaction submission (depends on A; storage depends on D1)

- **E1 — Account trait + tx types.** `tx/account.rs` (+ shared tx data types). Depends on A1.
  _(∥ E2)_
- **E2 — TransactionStorage.** `tx/storage.rs` + migration + tests. Depends on D1. _(∥ E1)_
- **E3 — TransactionManager.** `tx/manager.rs` + tests; uses alloy built-in fee estimation, keeps
  the replacement-fee bump for resubmits. Depends on A2, E1, E2.

### Phase F — Wrap-up

- **F1 — Docs (separate, docs-only PR).** Reconcile the crate-name discrepancy (see Open
  Questions): update `README.md` and `AGENTS.md` so the "shared crate" references match the actual
  `safenet-core` / `crates/core` (or rename per the chosen resolution), and document the modules.
- **F2 — (Optional) end-to-end example/integration test.** A small example or integration test
  wiring provider → watcher → snapshot store → transaction manager against an Anvil devnet.
  Depends on C4, D2, E3.
- **Cleanup.** Per the planning convention, **remove this epic file**
  (`epics/2026_06_09_safenet_core_crate.md`) once the epic is complete.

### Critical path

`A1 → A2 → E3` (gated also by `D1 → E2`) and `A1 → {C2, C3} → C4` are the longest chains. After
A1 (and A2 for the provider consumers), phases B, C, D and E proceed concurrently.

---

## Open Questions and Assumptions

**Open questions**

1. **Crate naming.** `README.md` and `AGENTS.md` call this the "shared crate" and reference
   `crates/shared`, but the crate on disk is `crates/core` / `safenet-core` (and `lib.rs` already
   documents it as "Safenet Core"). Resolution options: (a) keep `safenet-core` and fix the docs
   (recommended — less churn, matches existing code), or (b) rename the crate/dir to `shared`.
   Handled in F1.
2. **`RetryBackoffLayer` parameters.** Defaults for max retries, initial backoff, and
   compute-units-per-second, and whether a custom `RetryPolicy` is needed beyond
   `RateLimitRetryPolicy`.
3. **sqlx query verification mode.** Plan assumes runtime-checked queries (no live DB at build
   time). If compile-time `query!` macros are preferred, we'd commit `.sqlx` offline metadata and
   add a CI step.
4. **Metric set & names.** Which of the validator's metrics belong in the shared crate vs. remain
   service-specific, and the default namespace/prefix.
5. **`maxReorgDepth` / snapshot retention default** for the snapshot store (TS default is 5).
6. **Provider ownership.** Whether the crate exposes the `rpc.rs` provider-builder helper as the
   blessed path, or leaves provider construction entirely to consumers.

**Assumptions**

- The crate is greenfield for **future Rust services** and is **not** integrated into the existing
  TypeScript validator (no FFI). The TS validator stays as the reference implementation.
- `alloy` is the RPC/primitives/signing library; RPC backoff and fee estimation are delegated to
  `alloy` rather than ported; the crate is async on `tokio`.
- The indexer is **typed** over `alloy` `sol!` event definitions.
- Transaction-submission storage is intentionally **not** subject to reorg rollback.
- Config parsing/validation (the TS `zod` schemas) is a **consumer** concern; each component takes
  a plain config struct, so it is out of scope for this crate.
- Following the planning skill, this plan is proposed as a **docs-only PR** containing no epic
  implementation code.
