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
the `AGENTS.md` "reuse, don't reinvent" guideline. As of this writing `safenet-core` already
provides the typed event **indexer** (`index`), **observability** (`observability`), and a
**reorg-aware state machine + snapshot storage** (`state`). Its **transaction submission** (`tx`)
module is the only remaining dependency still being implemented (core epic Phase E). See Open
Question 1.

The TS sentinel remains the reference implementation throughout the port; there is no FFI and the
two are independent processes.

---

## How the TypeScript sentinel works (port surface)

A faithful port needs the whole data flow, so it is catalogued here. File references are under
`validator/src/`.

| TS file | Responsibility | Rust home |
| --- | --- | --- |
| `sentinel.ts` | Entrypoint: parse env config, build account/metrics/transport/client, open SQLite, start `SentinelService`, signal handling. | `main.rs` + `config.rs` |
| `sentinel/service.ts` | `SentinelService`: wires storage + action queue + tx storage + tx manager + protocol + watcher; routes each transition; applies diffs; enqueues actions. | `service.rs` (now drives `safenet-core::state::StateMachine`) |
| `sentinel/types.ts` | `SentinelAction` (5 variants), `SentinelRequestState` FSM (`preparing`→`pending`→`committed`→`finalized`), `SentinelConfig`. | `state.rs` |
| `sentinel/handlers.ts` | **Pure** functions: `handleOracleTransactionProposed` (compute `requestId`, run detector), `handleNewRequest`, `handleCommitted`, `handleResolved` (decode `ResolveReason`, vote-won logic), `handleBlockAdvance` (deadline FSM + finalize/cleanup). | `transition.rs` (a `StateTransition` impl) |
| `sentinel/transitions.ts` | Decode raw logs → `SentinelOracleTransition` (`zod`-validated args). | `transitions.rs` |
| `sentinel/protocol.ts` | `SentinelActionQueue` (`SqliteQueue`) + `SentinelProtocol` (`BaseActionQueue`): action → ABI-encoded tx via `TransactionManager`. | `protocol.rs` (+ `queue.rs`) |
| `sentinel/storage.ts` | `SentinelStateStorage`: plain SQLite key/value table of request states (**not** reorg-rolled-back). | superseded by `safenet-core::state::storage::SnapshotStore` |
| `sentinel/watcher.ts` | `SentinelTransitionWatcher` over the shared `BlockchainWatcher`, address-filtered to `[oracle, consensus]`. | `watcher.rs` |
| `sentinel/detector.ts` | Blocklist detector: approve unless `payload.to` is blocklisted. | `detector.rs` |
| `sentinel/abis.ts` | Event + function ABIs (SentinelOracle, ERC-20). | `bindings.rs` (`sol!`) |

Shared dependencies the sentinel pulls in (and their Rust replacements):

| TS dependency | Used for | Rust replacement |
| --- | --- | --- |
| `shared/watcher.ts` `BlockchainWatcher` | Block + event indexing, reorg detection | `safenet-core::index::Watcher` (**done**) |
| (new) reorg-aware state | persist + roll back service state on reorg | `safenet-core::state::{StateMachine, storage::SnapshotStore}` (**done**) |
| `consensus/protocol/transaction.ts` `TransactionManager`, `GasFeeEstimator` | Nonce mgmt, fee bump, resubmission, pending-check loop | `safenet-core::tx::manager` (**core epic Phase E — pending**) |
| `consensus/protocol/sqlite.ts` `SqliteTxStorage` | Nonce/status persistence | `safenet-core::tx::storage` (**core epic Phase E — pending**) |
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
| Service state + reorg rollback | `sentinel/storage.ts` (no rollback) | **`safenet-core::state`** | `StateMachine` drives pure `StateTransition`s and `SnapshotStore` rolls back on reorg. |
| Transaction submission | `transaction.ts` | **`safenet-core::tx`** | `TransactionManager` (nonce store, resubmit, fee bump, pending-check). Core epic Phase E. |
| SQLite | `better-sqlite3` (sync) | **`sqlx`** (async sqlite) | One shared `SqlitePool` across the snapshot store, action queue, and core's tx storage — matching the single-`Database` TS pattern. |
| Logging / metrics | `winston` / `prom-client` | **`safenet-core::observability`** | `tracing` + Prometheus. |
| `requestId` hashing | `viem hashTypedData` | **`alloy` `SolStruct::eip712_signing_hash`** | Onchain-identical *and* native-speed; the perf-sensitive hot path. |
| Action queue + retry | `SqliteQueue` + `BaseActionQueue` | **ported** (`sqlx` FIFO + `tokio` retry loop) | Open Question 3 (sentinel-local vs promote to core). |
| Config validation | `zod` | **`serde`** | Deserializable structs; file/env/CLI sourcing is owned by the binary (consumer concern, per core epic). |
| Errors | `viem` `BaseError` | **`thiserror`** | Typed error enums per module. |

Key decisions:

- **Build on `safenet-core`; do not re-implement shared infra.** The sentinel adds only the
  sentinel-specific pieces (detector, request FSM, the state transition, transition decoding,
  action→calldata mapping, service wiring). Indexing, observability, reorg-aware state, transaction
  submission, and the SQLite foundation come from the core crate.

- **The sentinel's logic is a `safenet-core::state::StateTransition`.** Core's `StateMachine<S, T>`
  consumes the indexer's `Update<E>` stream and drives a pure, non-fallible transition `T`,
  persisting each block's state to a `SnapshotStore` and rolling it back on reorg. The sentinel
  implements `StateTransition<S>` where:
  - `S` = the request map (`SentinelRequestState` keyed by `requestId`), `Serialize + Default`.
  - `Event` = `SentinelOracleTransition`; `Action` = `SentinelAction`.
  - `new_block(state, block)` ports `handleBlockAdvance` (deadline FSM, finalize/cleanup).
  - `event(state, transition)` ports the per-log handlers (`handleOracleTransactionProposed`,
    `handleNewRequest`, `handleCommitted`, `handleResolved`).

  This replaces the TS `SentinelStateDiff` / `applyDiff` machinery and the manual block/log routing
  in `service.ts`: the service simply feeds `Update`s to `handle_update` and submits the returned
  actions. Transitions must be infallible ("gracefully recover"), which fits the TS handlers — they
  already return "no change" on unexpected input — once log decoding/validation has happened upstream
  in `transitions.rs`.

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
  The watcher emits `Update::Block(BlockUpdate)` / `Update::Logs(EventUpdate { blocks, logs })`,
  consumed directly by the `StateMachine`.

- **Reuse the core transaction manager for submission and nonce management.** The TS sentinel uses
  the same `TransactionManager` + `SqliteTxStorage` + `GasFeeEstimator` as the validator. The Rust
  sentinel consumes `safenet-core::tx` identically; fresh fees come from `alloy`'s
  `estimate_eip1559_fees` (the core epic drops the bespoke `GasFeeEstimator`, keeping only the
  replacement-fee bump), so the TS `gasFeeEstimator.invalidate()` per-block call maps to nothing.
  The service still peeks at each `BlockUpdate::New` to call `tx_manager.trigger_pending_check`
  before handing the update to the state machine; the sentinel otherwise only maps each
  `SentinelAction` to ABI-encoded calldata and calls `submit_action`.

- **Reorg behavior is now first-class but submission stays best-effort.** The `StateMachine` rolls
  request state back to the reorg's common ancestor and re-applies forward; the nonce-keyed tx
  storage is intentionally *not* rolled back (per the core epic). An action emitted from a
  transition that is later reorged out cannot be un-sent — the same best-effort property the TS
  sentinel has — but on re-application the transition can re-emit it, so the action mapping must be
  idempotent against the tx manager's nonce store. This is an improvement over the TS sentinel,
  which detects reorgs but does not roll back at all.

- **Pure transition, separated from I/O.** Keeping the FSM in the `StateTransition` impl (no async
  I/O inside `new_block`/`event`) keeps the onchain-semantics-critical logic trivially
  unit-testable for parity and isolated from the async plumbing.

- **Config is `serde`-deserializable; sourcing is the binary's job.** Per the core epic's
  `zod`-out-of-scope decision, each module exposes a plain `Config` struct; `main.rs` assembles them
  from env/file. Since config compatibility is not required, env-var names can stay (`SENTINEL_*`,
  for ops continuity) or move to a config file — Open Question 4.

### Alternatives Considered

- **Re-implement indexing / state / tx submission inside the sentinel crate.** Rejected — duplicates
  `safenet-core` and violates the reuse guideline. The cost is a build-ordering dependency on core's
  `tx` module (Open Question 1), which is acceptable now that `index`, `observability`, and `state`
  have landed.
- **A plain, non-reorg request table mirroring the TS `SentinelStateStorage`.** Rejected (this was
  the original draft's choice). Core's reorg-aware `StateMachine`/`SnapshotStore` is now available,
  is the blessed pattern, and is strictly more correct under reorgs. DB compatibility is not
  required, so adopting it costs nothing in scope.
- **Hand-roll EIP-712 / `keccak` for `requestId`.** Rejected — `alloy`'s `sol!` + `SolStruct` is the
  canonical, contract-matching implementation and removes a whole class of compatibility bugs.
- **`rusqlite` (sync) instead of `sqlx`.** Rejected — core's state and tx storage are `sqlx`-backed
  and the sentinel shares one connection/pool with them, as the TS code shares one `Database`.
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
  migrations/                # action queue table (sqlx); snapshot + tx tables come from safenet-core
  src/
    main.rs                  # config load, observability init, provider/signer, build service, signals, run
    config.rs                # serde Config (observability + index + tx + sentinel fields) + env/file sourcing
    error.rs                 # crate error type (thiserror)
    bindings.rs              # sol!: SentinelOracle + Consensus events/calls, ERC20; SafeTx / *Proposal EIP-712 structs
    hashing.rs               # request_id(domain, proposal) + safe_tx_hash(tx) via eip712_signing_hash (+ parity vectors)
    detector.rs              # blocklist Detector
    state.rs                 # SentinelRequestState FSM, SentinelAction, SentinelConfig, the S state type
    transition.rs            # StateTransition impl (new_block = handleBlockAdvance; event = per-log handlers)
    transitions.rs           # watcher_events! set + log -> SentinelOracleTransition decoding
    watcher.rs               # SentinelTransitionWatcher over safenet-core::index::Watcher
    queue.rs                 # sqlx FIFO queue + action retry/timeout loop (SqliteQueue + BaseActionQueue analog)
    protocol.rs              # SentinelProtocol: action -> calldata -> safenet-core::tx::TransactionManager
    service.rs               # SentinelService: Watcher -> StateMachine::handle_update -> protocol
```

Modules are introduced only when first used (no empty stubs), per the core epic's convention.

### Onchain bindings & hashing (`bindings.rs`, `hashing.rs`)

- `sol!` blocks for `SentinelOracle` (events `NewRequest`, `Committed`, `OracleResult`, `Claimed`,
  `DisputeResolved`; calls `commitApprove`, `commitDeny`, `finalize`, `claim`), `Consensus`
  (event `OracleTransactionProposed`), and an `ERC20` (`approve`, `allowance`).
- `SafeTx`, `TransactionProposal`, `OracleTransactionProposal` declared as `sol!` EIP-712 structs.
  **The field types must match the canonical onchain Solidity typehashes exactly.** The EIP-712 type
  hash is sensitive to the precise type of every field, so any mismatch (e.g. declaring `epoch` as
  `uint256` when it is `uint64`) yields a different `requestId` and silently breaks onchain
  compatibility. The source of truth is the contracts' precomputed typehashes in
  `contracts/src/libraries/ConsensusMessages.sol` —
  `OracleTransactionProposal(uint64 epoch,address oracle,bytes32 safeTxHash)` and
  `TransactionProposal(uint64 epoch,bytes32 safeTxHash)` (note `epoch` is **`uint64`**) — together
  with the canonical Safe `SafeTx` type
  (`to address, value uint256, data bytes, operation uint8, safeTxGas uint256, baseGas uint256,
  gasPrice uint256, gasToken address, refundReceiver address, nonce uint256`). These line up with
  the TS `oracleTx/hashing.ts` and `safeTx/hashing.ts` declarations; the parity tests below are the
  guard against any drift.
- `safe_tx_hash(tx)` uses domain `{chainId: tx.chainId, verifyingContract: tx.safe}`;
  `request_id(domain, proposal)` uses domain `{chainId, verifyingContract: consensus}`. Both via
  `SolStruct::eip712_signing_hash`.
- **Parity tests are mandatory** (PR A3): assert byte-identical hashes against vectors captured from
  the TS functions and/or a live contract `eth_call`, covering the exact values exercised by
  `handleOracleTransactionProposed`.

### State, transition & detector (`state.rs`, `transition.rs`, `detector.rs`)

- `state.rs`: `SentinelRequestState` = `{ deadline, approve: bool, status }` with
  `status ∈ {Preparing, Pending, Committed, Finalized}`; `SentinelAction` =
  `{ApproveToken{bond}, CommitApprove{id}, CommitDeny{id}, Finalize{id}, Claim{id}}`; and the
  snapshot state `S` (the request map). `S` derives `Serialize + Deserialize + Default`; the map is
  keyed by `requestId` (`B256` serializes as a hex string, usable as a JSON map key, or store a
  `Vec<(B256, State)>` if preferred).
- `transition.rs`: the `StateTransition<S>` impl. `new_block` ports `handleBlockAdvance`
  (past-deadline: `committed → finalized` + `Finalize` action; otherwise drop). `event` matches on
  `SentinelOracleTransition` and ports `handleOracleTransactionProposed` (oracle-address gate,
  `request_id`, detector → `approve`, `deadline = block + votingWindow`), `handleNewRequest` (acts
  only on `preparing`; emits `ApproveToken` + commit), `handleCommitted` (own-address gate,
  `pending → committed`), and `handleResolved` (drop the request unconditionally, but **only emit a
  `Claim` when we actually committed onchain** — i.e. the existing status is `committed` or
  `finalized`; for any other status drop silently, since our commit tx may never have confirmed.
  When committed, decode `ResolveReason` and claim iff
  `voteWon = reason == TIMEOUT(2) || approved == ours`). All branches return
  `(S, Vec<SentinelAction>)` and never error.
- `detector.rs`: `approve` unless `payload.to` ∈ blocklist (address-equality).

### Transition decoding & watcher (`transitions.rs`, `watcher.rs`)

- `watcher_events!` over `SentinelOracle::SentinelOracleEvents` and `Consensus::ConsensusEvents`;
  `log → SentinelOracleTransition` decoding replaces the `zod` arg schemas with `sol!`-typed
  decoding (the indexer already hands back typed events).
- `SentinelTransitionWatcher` constructs `safenet-core::index::Watcher` with
  `addresses = [oracle, consensus]` and resumes from the snapshot store's `current()` block.

### Action queue & protocol (`queue.rs`, `protocol.rs`)

- `queue.rs`: a `sqlx` FIFO queue (`SqliteQueue` analog) plus the serialized retry/timeout loop from
  `BaseActionQueue` (10-min action TTL, 1s→5s backoff on failure), reimplemented with `tokio` timers.
- `protocol.rs`: `SentinelProtocol` maps each action to ABI-encoded calldata
  (`ERC20::approveCall`, `SentinelOracle::{commitApprove,commitDeny,finalize,claim}Call`) and submits
  via `safenet-core::tx::TransactionManager::submit_action`.

### Service & binary (`service.rs`, `main.rs`, `config.rs`)

- `SentinelService` owns the `StateMachine<S, SentinelTransition>` (built from a `SnapshotStore` on
  the shared pool), the watcher, and the protocol queue. Its loop: pull the next `Update` from the
  watcher; if it is `BlockUpdate::New`, call `tx_manager.trigger_pending_check(number)`; pass the
  update to `state_machine.handle_update` and enqueue every returned `SentinelAction` into the
  protocol. Exposes `run`/`shutdown`.
- `main.rs`: load `Config`, `safenet_core::observability::init`, build the `alloy` provider/signer,
  open the `SqlitePool` (snapshot + tx + queue tables), construct and run the service, handle
  SIGINT/SIGTERM.

### Testing

- Unit tests mirror the TS test intent (behavior, not implementation): the `StateTransition`
  (FSM transitions, vote-won/timeout, deadline cleanup), `request_id` parity vectors, transition
  decoding, and the action queue's retry/timeout behavior. Reorg rollback of request state is
  already covered by core's `StateMachine` tests, but a sentinel-level test should assert a reorged
  `NewRequest` is rolled back.
- `sqlx` tests run against `sqlite::memory:` (matching the TS `:memory:` default).
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

### Phase B — Domain logic (depends on A; parallel with C & D)

- **B1 — State types & detector.** `state.rs` (`SentinelRequestState`, `SentinelAction`, the
  snapshot state `S`, `SentinelConfig`) and `detector.rs` (blocklist) + tests. Depends on A1.
- **B2 — State transition.** `transition.rs`: the `StateTransition` impl (`new_block` +
  `event`), porting all five handlers, with unit tests for the FSM, vote-won/timeout, and deadline
  cleanup. Depends on A3 (`request_id`), B1.

### Phase C — Indexing integration (depends on A; parallel with B & D)

- **C1 — Event set & transition decoding.** `transitions.rs`: `watcher_events!` over the two event
  enums and `log → SentinelOracleTransition`, with decoding tests. Depends on A2.
- **C2 — Transition watcher.** `watcher.rs`: wrap `safenet-core::index::Watcher` for
  `[oracle, consensus]`, resuming from the snapshot store. Depends on C1 and `safenet-core::index`
  (done).

### Phase D — Action queue & submission

- **D1 — Action queue + retry loop.** `queue.rs`: `sqlx` FIFO queue and the `tokio` retry/timeout
  loop + tests. Depends on A1 and the shared `SqlitePool` (no new core dependency).
- **D2 — Sentinel protocol.** `protocol.rs`: action → calldata → `TransactionManager::submit_action`.
  Depends on A2, D1, and `safenet-core::tx` (**core epic Phase E — pending**).

### Phase E — Orchestration & binary

- **E1 — SentinelService.** `service.rs`: build the `StateMachine` over a `SnapshotStore`, run the
  watcher → `handle_update` → protocol loop, with the per-`BlockUpdate::New`
  `trigger_pending_check`. Depends on B2, C2, D2.
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
(core epic Phase E) for `D2`/`E1`. After A, phases B, C and D proceed concurrently;
`index`, `observability` and `state` are already available.

---

## Open Questions and Assumptions

**Open questions**

1. **Dependency on `safenet-core` `tx` (core epic Phase E).** With `index`, `observability` and
   `state` now landed, the only remaining core dependency is the `tx` module
   (`TransactionManager`, `TransactionStorage`, `Account`). **Recommended:** sequence `D2`/`E1`
   after — or co-develop alongside — Phase E and consume the module directly. If it slips, the
   fallback is to port the minimal tx subset locally and migrate later (extra churn). Phases A, B, C
   and `D1` have no such dependency and can start immediately.
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
6. **Chain/fee configuration.** The TS entrypoint uses `viem extractChain` (gnosis/sepolia/anvil) to
   attach `ChainFees` (`baseFeeMultiplier`, `maxPriorityFeePerGas`). Confirm the equivalent fee
   configuration to pass through to `alloy`/core's tx manager, since alloy handles chain fees
   differently.

**Assumptions**

- **Onchain compatibility** means identical `requestId` derivation, event decoding, ABI calldata,
  `ResolveReason` decoding, and FSM-driven action timing — **not** DB or config compatibility (per
  the deliverable).
- The port **reuses `safenet-core`** for indexing, observability, reorg-aware state, transaction
  submission, and the SQLite foundation rather than re-implementing them.
- It is a greenfield Rust **binary**; the TS sentinel remains the reference implementation during the
  port, with no FFI, and both can run against the same `SentinelOracle`.
- `alloy` is the EVM library; EIP-712 hashing is done via `sol!` structs and
  `SolStruct::eip712_signing_hash`.
- The sentinel FSM is implemented as a pure `safenet-core::state::StateTransition` and tested for
  behavior (not implementation), mirroring the existing `*.test.ts` intent.
- Following the planning convention, this plan is proposed as a **docs-only PR** containing no epic
  implementation code.
