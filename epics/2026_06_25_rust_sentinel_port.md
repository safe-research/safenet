# Plan: Port the TypeScript sentinel to Rust

Component: new crate `crates/sentinel` (Cargo crate `safenet-sentinel`), porting
`validator/src/sentinel/` + `validator/src/sentinel.ts`.

---

## Overview

The Safenet `sentinel` client is currently a TypeScript service (`validator/src/sentinel/`,
~780 LOC, plus the `sentinel.ts` entrypoint). It watches the `SentinelOracle` and `Consensus`
contracts, decides whether to approve or deny each proposed oracle transaction (via a blocklist
detector), and puts up bonds by committing a vote, finalizing, and claiming onchain. This epic
ports it to Rust.

Motivation (from the task): TypeScript has caused paper cuts and the **nonce/hash computation
performance is a problem**. Porting to Rust is expected to increase velocity and remove that hot
spot (see Open Question 2 for which computation we benchmark).

**The single hard requirement is onchain compatibility with the TypeScript sentinel:** a Rust
sentinel and a TS sentinel must be able to participate in the *same* `SentinelOracle` dispute and
behave correctly together. Concretely, onchain compatibility means byte-identical:

1. **`requestId` derivation** — the EIP-712 typed-data hash `oracleTxProposalHash` over
   `{epoch, oracle, safeTxHash}` with domain `{chainId, verifyingContract: consensus}`, where
   `safeTxHash` is itself the EIP-712 `SafeTx` hash (`validator/src/consensus/verify/oracleTx/hashing.ts`,
   `.../safeTx/hashing.ts`). The Rust port must produce the same 32-byte id the contract and the TS
   sentinel use to key a request.
2. **Event decoding** — `NewRequest`, `Committed`, `OracleResult`, `Claimed` (SentinelOracle) and
   `OracleTransactionProposed` (Consensus).
3. **Transaction calldata** — `commitApprove`/`commitDeny`/`finalize`/`claim` on the oracle and
   ERC-20 `approve` for the bond token.
4. **Result decoding** — `OracleResult.result` is `abi.encode(ResolveReason)`; `TIMEOUT = 2`.
5. **FSM-driven action *timing*** — which onchain transactions get sent and when (deadlines,
   finalize eligibility), so the two implementations make the same moves in a shared dispute.

Explicitly **not** required (per the deliverable): database backwards compatibility and
configuration backwards compatibility. This frees the port to choose its own SQLite schema and
config format rather than mirroring `better-sqlite3` tables or the exact `zod`/env layout.

This port **builds on the in-progress `safenet-core` crate** (`crates/core`,
[epic](./2026_06_09_safenet_core_crate.md)) rather than re-implementing shared infrastructure, per
the `AGENTS.md` "reuse, don't reinvent" guideline. `safenet-core` already provides the typed
event **indexer** (`index`) and **observability** (`observability`); its **transaction submission**
(`tx`) and **SQLite state foundation** (`state`/`db`) modules are still being implemented (core
epic Phases D & E). The sentinel consumes all four. See Open Question 1 for the sequencing
dependency.

The TS sentinel remains the reference implementation throughout the port; there is no FFI and the
two are independent processes.

---

## How the TypeScript sentinel works (port surface)

A faithful port needs the whole data flow, so it is catalogued here. File references are under
`validator/src/`.

| TS file | Responsibility | Rust home |
| --- | --- | --- |
| `sentinel.ts` | Entrypoint: parse env config, build account/metrics/transport/client, open SQLite, start `SentinelService`, signal handling. | `main.rs` + `config.rs` |
| `sentinel/service.ts` | `SentinelService`: wires storage + action queue + tx storage + tx manager + protocol + watcher; routes each transition (block → `handleBlockAdvance` + `triggerPendingCheck`; log → handlers), applies diffs, enqueues actions. | `service.rs` |
| `sentinel/types.ts` | `SentinelAction` (5 variants), `SentinelRequestState` FSM (`preparing`→`pending`→`committed`→`finalized`), `SentinelConfig`, `SentinelStateDiff`. | `state.rs` |
| `sentinel/handlers.ts` | **Pure** functions: `handleOracleTransactionProposed` (compute `requestId`, run detector), `handleNewRequest`, `handleCommitted`, `handleResolved` (decode `ResolveReason`, vote-won logic), `handleBlockAdvance` (deadline FSM + finalize/cleanup). | `handlers.rs` |
| `sentinel/transitions.ts` | Decode raw logs → `SentinelOracleTransition` (`zod`-validated args). | `transitions.rs` |
| `sentinel/protocol.ts` | `SentinelActionQueue` (`SqliteQueue`) + `SentinelProtocol` (`BaseActionQueue`): action → ABI-encoded tx via `TransactionManager`. | `protocol.rs` (+ `queue.rs`) |
| `sentinel/storage.ts` | `SentinelStateStorage`: plain SQLite key/value table of request states (**not** reorg-rolled-back). | `storage.rs` |
| `sentinel/watcher.ts` | `SentinelTransitionWatcher` over the shared `BlockchainWatcher`, address-filtered to `[oracle, consensus]`. | `watcher.rs` |
| `sentinel/detector.ts` | Blocklist detector: approve unless `payload.to` is blocklisted. | `detector.rs` |
| `sentinel/abis.ts` | Event + function ABIs (SentinelOracle, ERC-20). | `bindings.rs` (`sol!`) |

Shared dependencies the sentinel pulls in (and their Rust replacements):

| TS dependency | Used for | Rust replacement |
| --- | --- | --- |
| `shared/watcher.ts` `BlockchainWatcher` | Block + event indexing, reorg detection | `safenet-core::index::Watcher` (**done**) |
| `consensus/protocol/transaction.ts` `TransactionManager`, `GasFeeEstimator` | Nonce mgmt, fee bump, resubmission, pending-check loop | `safenet-core::tx::manager` (**core epic Phase E**) |
| `consensus/protocol/sqlite.ts` `SqliteTxStorage` | Nonce/status persistence | `safenet-core::tx::storage` (**core epic Phase E**) |
| `consensus/protocol/base.ts` `BaseActionQueue` | Serialized action retry/timeout loop | ported into `queue.rs` (Open Question 3) |
| `utils/queue.ts` `SqliteQueue` | Persistent FIFO action queue | ported into `queue.rs` |
| `utils/logging.ts`, `utils/metrics.ts` | Logging + Prometheus | `safenet-core::observability` (**done**) |
| `consensus/verify/oracleTx/hashing.ts`, `.../safeTx/hashing.ts` | `requestId` (EIP-712) | `hashing.rs` via `alloy` `sol!` + `SolStruct::eip712_signing_hash` |
| `machine/transitions/types.ts` `OracleTransactionProposedEvent` | Consensus event shape | `bindings.rs` + `transitions.rs` |
| `types/schemas.ts` `sentinelConfigSchema` (`zod`) | Env validation | `config.rs` (`serde`); sourcing is consumer-owned |

---

## Architecture Decision

The crate is an `async` (`tokio`) **binary** that consumes `safenet-core`, mirroring the language
choices already made for the core crate so the two compose cleanly.

| Concern | TypeScript today | Rust choice | Notes |
| --- | --- | --- | --- |
| RPC / primitives / signing / EIP-712 | `viem` | **`alloy`** | `Provider`, `Address`/`B256`/`U256`/`Bytes`, `PrivateKeySigner`, typed events & EIP-712 via `sol!`. |
| Indexing | `shared/watcher.ts` | **`safenet-core::index`** | `Watcher<P, E>` over a typed event set; reuse, don't re-port. |
| Transaction submission | `transaction.ts` | **`safenet-core::tx`** | `TransactionManager` (nonce store, resubmit, fee bump, pending-check). Core epic Phase E. |
| Tx + state persistence | `better-sqlite3` (sync) | **`sqlx`** (async sqlite) | One shared `SqlitePool` across request store, action queue, and core's tx storage — matching the single-`Database` TS pattern. |
| Logging / metrics | `winston` / `prom-client` | **`safenet-core::observability`** | `tracing` + Prometheus. |
| `requestId` hashing | `viem hashTypedData` | **`alloy` `SolStruct::eip712_signing_hash`** | Onchain-identical *and* native-speed; the perf-sensitive hot path. |
| Action queue + retry | `SqliteQueue` + `BaseActionQueue` | **ported** (`sqlx` FIFO + `tokio` retry loop) | Open Question 3 (sentinel-local vs promote to core). |
| Config validation | `zod` | **`serde`** | Deserializable structs; file/env/CLI sourcing is owned by the binary (consumer concern, per core epic). |
| Errors | `viem` `BaseError` | **`thiserror`** | Typed error enums per module. |

Key decisions:

- **Build on `safenet-core`; do not re-implement shared infra.** The sentinel adds only the
  sentinel-specific pieces (detector, request FSM, handlers, transition decoding, action→calldata
  mapping, service wiring). Indexing, observability, transaction submission, and the SQLite
  foundation come from the core crate.

- **`requestId` derivation via `alloy` `sol!` EIP-712 is the linchpin of onchain compatibility.**
  Declare `SafeTx`, `TransactionProposal`, and `OracleTransactionProposal` as `sol!` structs and
  compute their hashes with `SolStruct::eip712_signing_hash(domain)`. This produces byte-identical
  output to the contract and the TS `viem hashTypedData`, and — being native `keccak` over a packed
  encoding — directly addresses the computation-performance motivation. Parity is locked down with
  test vectors captured from the TS implementation / contract (PR A3).

- **Typed event indexer over both contracts.** Use the core `watcher_events!` macro over the
  `sol!`-generated `SentinelOracle::*Events` and `Consensus::*Events` enums and let
  `safenet-core::index::Watcher` fetch/decode/order logs for addresses `[oracle, consensus]`,
  dispatching by `log.address()`. This is the typed analog of the TS `SENTINEL_ALL_EVENTS` list.

- **Reuse the core transaction manager for submission and nonce management.** The TS sentinel uses
  the same `TransactionManager` + `SqliteTxStorage` + `GasFeeEstimator` as the validator. The Rust
  sentinel consumes `safenet-core::tx` identically; fresh fees come from `alloy`'s
  `estimate_eip1559_fees` (the core epic drops the bespoke `GasFeeEstimator`, keeping only the
  replacement-fee bump), so the TS `gasFeeEstimator.invalidate()` per-block call maps to nothing.
  The sentinel only maps each `SentinelAction` to ABI-encoded calldata and calls
  `submit_action` / `trigger_pending_check`.

- **Pure handlers, separated from I/O.** Port `handlers.ts` as pure functions
  `(state, transition, config) -> Vec<SentinelStateDiff>`, exactly as in TS. This keeps the FSM
  trivially unit-testable for behavioral parity and isolates the onchain-semantics-critical logic
  from async plumbing.

- **Request-state storage mirrors the TS plain table (not reorg-rolled-back).** The TS
  `SentinelStateStorage` is a flat `id → stateJson` table; the shared watcher detects reorgs but
  does not roll state back. The Rust port keeps this behavior for parity (a `sqlx` request table on
  the shared pool). It deliberately does **not** use core's reorg-aware `SnapshotStore`; see
  Alternatives and Open Question 6. Because DB compatibility is not required, the schema is chosen
  freely (e.g. `BLOB`/JSON as convenient).

- **Config is `serde`-deserializable; sourcing is the binary's job.** Per the core epic's
  `zod`-out-of-scope decision, each module exposes a plain `Config` struct; `main.rs` assembles them
  from env/file. Since config compatibility is not required, env-var names can stay (`SENTINEL_*`,
  for ops continuity) or move to a config file — Open Question 4.

### Alternatives Considered

- **Re-implement indexing / tx submission inside the sentinel crate.** Rejected — duplicates
  `safenet-core` and violates the reuse guideline. The cost is a build-ordering dependency on core
  Phases D/E (Open Question 1), which is acceptable.
- **Hand-roll EIP-712 / `keccak` for `requestId`.** Rejected — `alloy`'s `sol!` + `SolStruct` is the
  canonical, contract-matching implementation and removes a whole class of compatibility bugs.
- **Reorg-aware `SnapshotStore` for request state.** Deferred — the bounded, event-driven request
  FSM matches the TS no-rollback behavior today, and adopting snapshots would diverge from the
  reference for marginal benefit. The store API is kept narrow so it could be swapped later.
- **`rusqlite` (sync) instead of `sqlx`.** Rejected — core's tx storage is `sqlx`-backed and the
  sentinel shares one connection/pool with it, as the TS code shares one `Database`.
- **Keep TS as a subprocess / add FFI.** Rejected — a full port is the stated goal; FFI would keep
  the TS paper cuts and the hot path in JS.
- **Untyped log-filter indexer.** Rejected — the core indexer is typed; we keep compile-time-checked
  event handling.

---

## Tech Specs

### Crate layout

```
crates/sentinel/
  Cargo.toml                 # deps: safenet-core, alloy, sqlx, tokio, serde, serde_json, thiserror, tracing
  migrations/                # sentinel request table + action queue table (sqlx)
  src/
    main.rs                  # config load, observability init, provider/signer, build service, signals, run
    config.rs                # serde Config (observability + index + tx + sentinel fields) + env/file sourcing
    error.rs                 # crate error type (thiserror)
    bindings.rs              # sol!: SentinelOracle + Consensus events/calls, ERC20; SafeTx / *Proposal EIP-712 structs
    hashing.rs               # request_id(domain, proposal) + safe_tx_hash(tx) via eip712_signing_hash (+ parity vectors)
    detector.rs              # blocklist Detector
    state.rs                 # SentinelRequestState FSM, SentinelAction, SentinelStateDiff, SentinelConfig
    handlers.rs              # pure transition handlers -> diffs
    transitions.rs           # watcher_events! set + log -> SentinelOracleTransition decoding
    watcher.rs               # SentinelTransitionWatcher over safenet-core::index::Watcher
    queue.rs                 # sqlx FIFO queue + action retry/timeout loop (SqliteQueue + BaseActionQueue analog)
    protocol.rs              # SentinelProtocol: action -> calldata -> safenet-core::tx::TransactionManager
    service.rs               # SentinelService orchestration
```

Modules are introduced only when first used (no empty stubs), per the core epic's convention.

### Onchain bindings & hashing (`bindings.rs`, `hashing.rs`)

- `sol!` blocks for `SentinelOracle` (events `NewRequest`, `Committed`, `OracleResult`, `Claimed`,
  `DisputeResolved`; calls `commitApprove`, `commitDeny`, `finalize`, `claim`), `Consensus`
  (event `OracleTransactionProposed`), and an `ERC20` (`approve`, `allowance`).
- `SafeTx`, `TransactionProposal`, `OracleTransactionProposal` declared as `sol!` EIP-712 structs
  matching `safeTx/hashing.ts` and `oracleTx/hashing.ts` field-for-field
  (`SafeTx`: `to,value,data,operation,safeTxGas,baseGas,gasPrice,gasToken,refundReceiver,nonce`;
  `OracleTransactionProposal`: `epoch (uint64), oracle (address), safeTxHash (bytes32)`).
- `safe_tx_hash(tx)` uses domain `{chainId: tx.chainId, verifyingContract: tx.safe}`;
  `request_id(domain, proposal)` uses domain `{chainId, verifyingContract: consensus}`. Both via
  `SolStruct::eip712_signing_hash`.
- **Parity tests are mandatory** (PR A3): assert byte-identical hashes against vectors captured from
  the TS functions and/or a live contract `eth_call`, covering the exact values exercised by
  `handleOracleTransactionProposed`.

### Request FSM, handlers & detector (`state.rs`, `handlers.rs`, `detector.rs`)

- `SentinelRequestState` = `{ deadline: U256/u64, approve: bool, status }` with
  `status ∈ {Preparing, Pending, Committed, Finalized}`; `SentinelAction` =
  `{ApproveToken{bond}, CommitApprove{id}, CommitDeny{id}, Finalize{id}, Claim{id}}`;
  `SentinelStateDiff { request: Option<(B256, Option<State>)>, actions: Vec<SentinelAction> }`.
- Handlers ported 1:1 from `handlers.ts` as pure functions, including:
  `handleOracleTransactionProposed` (oracle-address gate, `request_id`, detector → `approve`,
  `deadline = block + votingWindow`); `handleNewRequest` (only acts on `preparing`; emits
  `approve_token` + commit); `handleCommitted` (own-address gate, `pending → committed`);
  `handleResolved` (decode `ResolveReason`; `voteWon = reason == TIMEOUT(2) || approved == ours`;
  claim if won); `handleBlockAdvance` (past-deadline: `committed → finalized` + `finalize`, else drop).
- `detector.rs`: `approve` unless `payload.to` ∈ blocklist (address-equality).

### Transition decoding & watcher (`transitions.rs`, `watcher.rs`)

- `watcher_events!` over `SentinelOracle::SentinelOracleEvents` and `Consensus::ConsensusEvents`;
  `log → SentinelOracleTransition` decoding replaces the `zod` arg schemas with `sol!`-typed
  decoding (the indexer already hands back typed events).
- `SentinelTransitionWatcher` constructs `safenet-core::index::Watcher` with
  `addresses = [oracle, consensus]` and adapts `Update::Block` → block transition (drives
  `handleBlockAdvance` + `trigger_pending_check`) and `Update::Logs` → per-log transitions.

### Storage, queue & protocol (`storage.rs`, `queue.rs`, `protocol.rs`)

- `storage.rs`: `SentinelStateStorage` over the shared `SqlitePool`; in-memory request map loaded
  on start, `apply_diff` upserts/deletes a row and returns the diff's actions. Mirrors `storage.ts`.
- `queue.rs`: a `sqlx` FIFO queue (`SqliteQueue` analog) plus the serialized retry/timeout loop from
  `BaseActionQueue` (10-min action TTL, 1s→5s backoff on failure), reimplemented with `tokio` timers.
- `protocol.rs`: `SentinelProtocol` maps each action to ABI-encoded calldata
  (`ERC20::approveCall`, `SentinelOracle::{commitApprove,commitDeny,finalize,claim}Call`) and submits
  via `safenet-core::tx::TransactionManager::submit_action`.

### Service & binary (`service.rs`, `main.rs`, `config.rs`)

- `SentinelService` mirrors `service.ts`: on a block, run `handleBlockAdvance` over current requests
  and `trigger_pending_check`; on a log, route to the matching handler; apply diffs to storage and
  enqueue resulting actions into the protocol. Exposes `run`/`shutdown`.
- `main.rs`: load `Config`, `safenet_core::observability::init`, build the `alloy` provider/signer,
  open the `SqlitePool` + run migrations, construct and run the service, handle SIGINT/SIGTERM.

### Testing

- Unit tests mirror the TS test intent (behavior, not implementation): the pure handlers
  (FSM transitions, vote-won/timeout, deadline cleanup), `request_id` parity vectors, transition
  decoding, the request store round-trip, and the action queue's retry/timeout behavior.
- Indexer-facing tests reuse `safenet-core`'s mock-provider approach; `sqlx` tests run against
  `sqlite::memory:` (matching the TS `:memory:` default).
- An interop/integration test (Phase F) runs the Rust sentinel against a `SentinelOracle` on Anvil.

### Tooling

- Per `AGENTS.md`: `cargo fmt --all`, `cargo clippy --package safenet-sentinel`,
  `cargo test --package safenet-sentinel`. `Cargo.lock` committed.
- Dependency features stay permissive during implementation (e.g. `full` on `tokio`/`alloy`) and are
  narrowed in a wrap-up PR (F2), matching the core epic's approach.

---

## Implementation Phases

Each PR has a single purpose, targets < 300 changed LOC and < 10 files, and is independently
reviewable. "Depends on" lists hard ordering; everything else may proceed in parallel.

### Phase A — Foundation, bindings & EIP-712 (blocks all other phases)

- **A1 — Crate scaffolding & dependencies.** New `crates/sentinel` binary crate, added to the
  workspace; dependency set (`safenet-core`, `alloy`, `sqlx`, `tokio`, `serde`, `serde_json`,
  `thiserror`, `tracing`); commit `Cargo.lock`. No empty module stubs. _Single purpose: dependencies._
- **A2 — Onchain bindings.** `bindings.rs`: `sol!` for SentinelOracle + Consensus events/calls and
  ERC-20, plus the `SafeTx`/`TransactionProposal`/`OracleTransactionProposal` EIP-712 structs.
  Depends on A1.
- **A3 — `requestId` hashing + parity vectors.** `hashing.rs`: `request_id` and `safe_tx_hash` via
  `eip712_signing_hash`, with tests asserting byte-identical output vs TS/contract vectors. Depends
  on A2. _The onchain-compatibility linchpin and the perf-sensitive hot path._

### Phase B — Pure domain logic (depends on A; parallel with C & D)

- **B1 — Request FSM, actions & detector.** `state.rs` (`SentinelRequestState`, `SentinelAction`,
  `SentinelStateDiff`, `SentinelConfig`) and `detector.rs` (blocklist) + tests. Depends on A1.
- **B2 — Transition handlers (pure).** `handlers.rs`: the five handlers → diffs, with unit tests for
  the FSM, vote-won/timeout, and deadline cleanup. Depends on A3 (`request_id`) and B1.

### Phase C — Indexing integration (depends on A; parallel with B & D)

- **C1 — Event set & transition decoding.** `transitions.rs`: `watcher_events!` over the two event
  enums and `log → SentinelOracleTransition`, with decoding tests. Depends on A2.
- **C2 — Transition watcher.** `watcher.rs`: wrap `safenet-core::index::Watcher` for
  `[oracle, consensus]` and adapt `Update` → transitions. Depends on C1 and `safenet-core::index`
  (done).

### Phase D — Storage, queue & submission

- **D1 — Request-state storage.** `storage.rs` + migration: `sqlx` request table on the shared pool
  (load + `apply_diff`) + tests. Depends on B1 and `safenet-core::state::db` (**core epic Phase D1**).
- **D2 — Action queue + retry loop.** `queue.rs`: `sqlx` FIFO queue and the `tokio` retry/timeout
  loop + tests. Depends on D1's DB foundation.
- **D3 — Sentinel protocol.** `protocol.rs`: action → calldata → `TransactionManager::submit_action`.
  Depends on A2, D2, and `safenet-core::tx` (**core epic Phase E**).

### Phase E — Orchestration & binary

- **E1 — SentinelService.** `service.rs`: wire storage/watcher/protocol/tx-manager; route
  block/log transitions; apply diffs and enqueue actions. Depends on B2, C2, D1, D3.
- **E2 — Config & binary.** `config.rs` (`serde` config) + `main.rs` (env/file load, observability
  init, provider/signer, migrations, signals, run). Depends on E1.

### Phase F — Validation & wrap-up

- **F1 — Interop/integration test.** Run the Rust sentinel against a `SentinelOracle` on Anvil
  (extend `scripts/`/integration tests), asserting it commits/finalizes/claims correctly and
  interoperates with the TS sentinel in a shared dispute — the deliverable's "work together onchain"
  acceptance check. Depends on E2.
- **F2 — Docs, feature narrowing & cleanup.** Update `README.md`/`AGENTS.md` to list the new crate;
  narrow `alloy`/`tokio` features to what is used; refresh `Cargo.lock`. Depends on all
  implementation phases.
- **Cleanup.** Per the planning convention, **remove this epic file**
  (`epics/2026_06_25_rust_sentinel_port.md`) once the epic is complete.

### Critical path

`A1 → A2 → A3 → B2 → E1 → E2 → F1`, additionally gated by the `safenet-core` `tx` module
(core epic Phase E) and `state::db` (core epic Phase D1) for `D1`/`D3`/`E1`. After A, phases B, C
and D proceed concurrently.

---

## Open Questions and Assumptions

**Open questions**

1. **Dependency on `safenet-core` Phases D & E.** The sentinel consumes `safenet-core::tx`
   (`TransactionManager`, `TransactionStorage`, `Account`) and the `state::db` `sqlx` foundation,
   neither of which is implemented yet (core epic Phases D/E; the index and observability modules are
   done). **Recommended:** sequence this epic after — or co-develop alongside — those phases and
   consume the modules directly. If they slip, the fallback is to port the minimal tx/queue subset
   locally and migrate to core later (extra churn). Phases A, B and C have no such dependency and can
   start immediately.
2. **Which "computation" is the performance problem?** The task cites "nonce computation." The
   sentinel does no FROST nonce work; the plausible hot paths are (a) the per-`OracleTransactionProposed`
   EIP-712 `requestId` hashing (addressed natively by `alloy` `sol!` keccak) and (b) transaction
   nonce management (addressed by core's `TransactionManager` + `alloy`). **Recommended:** confirm the
   target and add a throughput benchmark for it as an acceptance criterion (likely the `requestId`
   hashing in A3).
3. **Where do the action queue + retry loop live?** `SqliteQueue`/`BaseActionQueue` are shared
   between the validator and the sentinel in TS. **Recommended:** implement sentinel-local (`queue.rs`)
   now, designed for later promotion to `safenet-core` when the validator is ported.
4. **Config format & sourcing.** Keep the TS env-var names (`SENTINEL_ORACLE_ADDRESS`,
   `SENTINEL_VOTING_WINDOW`, …) for operational continuity, or move to a TOML file like the
   `validator-rust` spike? Config compatibility is not required, so either is acceptable.
5. **Crate name/location.** Proposed `crates/sentinel` / `safenet-sentinel`. Confirm.
6. **Reorg handling for request state.** Mirror the TS no-rollback plain table (recommended, for
   parity), or adopt core's reorg-aware `SnapshotStore` once available (more correct under reorgs,
   diverges from reference)?
7. **Chain/fee configuration.** The TS entrypoint uses `viem extractChain` (gnosis/sepolia/anvil) to
   attach `ChainFees` (`baseFeeMultiplier`, `maxPriorityFeePerGas`). Confirm the equivalent fee
   configuration to pass through to `alloy`/core's tx manager, since alloy handles chain fees
   differently.

**Assumptions**

- **Onchain compatibility** means identical `requestId` derivation, event decoding, ABI calldata,
  `ResolveReason` decoding, and FSM-driven action timing — **not** DB or config compatibility (per
  the deliverable).
- The port **reuses `safenet-core`** for indexing, observability, transaction submission, and the
  SQLite foundation rather than re-implementing them.
- It is a greenfield Rust **binary**; the TS sentinel remains the reference implementation during the
  port, with no FFI, and both can run against the same `SentinelOracle`.
- `alloy` is the EVM library; EIP-712 hashing is done via `sol!` structs and
  `SolStruct::eip712_signing_hash`.
- The pure handlers are ported faithfully and tested for behavior (not implementation), mirroring the
  existing `*.test.ts` intent.
- Following the planning convention, this plan is proposed as a **docs-only PR** containing no epic
  implementation code.
