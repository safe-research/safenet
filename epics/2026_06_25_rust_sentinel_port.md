# Plan: Port the TypeScript sentinel to Rust

Component: new crate `crates/sentinel` (Cargo crate `sentinel`), porting
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
spot (see Open Question 1 for which computation we benchmark).

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
config format rather than mirroring `better-sqlite3` tables or the exact `zod`/env layout. The crate
follows the conventions just established by the sibling `validator` crate (`crates/validator`,
PR #489): a bare package name, workspace-inherited dependencies, an `argh` CLI, and a TOML
configuration file.

This port **builds on the `safenet-core` crate** (`crates/core`) rather than re-implementing shared
infrastructure, per the `AGENTS.md` "reuse, don't reinvent" guideline. As of this writing the core
crate is complete and provides everything the sentinel needs: the typed event **indexer**
(`index`), **observability** (`observability`), a **reorg-aware state machine + snapshot storage**
(`state`), and **reliable transaction submission** (`tx`: a `TransactionQueue` with a local
`Signer`). There is no longer any pending core dependency.

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
| `sentinel/protocol.ts` | `SentinelActionQueue` (`SqliteQueue`) + `SentinelProtocol` (`BaseActionQueue`): action → ABI-encoded tx via `TransactionManager`. | `protocol.rs` (action → `tx::Transaction`, enqueued on `safenet-core::tx::TransactionQueue`) |
| `sentinel/storage.ts` | `SentinelStateStorage`: plain SQLite key/value table of request states (**not** reorg-rolled-back). | superseded by `safenet-core::state::storage::SnapshotStore` |
| `sentinel/watcher.ts` | `SentinelTransitionWatcher` over the shared `BlockchainWatcher`, address-filtered to `[oracle, consensus]`. | `watcher.rs` |
| `sentinel/detector.ts` | Blocklist detector: approve unless `payload.to` is blocklisted. | `detector.rs` |
| `sentinel/abis.ts` | Event + function ABIs (SentinelOracle, ERC-20). | `bindings.rs` (`sol!`) |

Shared dependencies the sentinel pulls in (and their Rust replacements):

| TS dependency | Used for | Rust replacement |
| --- | --- | --- |
| `shared/watcher.ts` `BlockchainWatcher` | Block + event indexing, reorg detection | `safenet-core::index::Watcher` (**done**) |
| (new) reorg-aware state | persist + roll back service state on reorg | `safenet-core::state::{StateMachine, storage::SnapshotStore}` (**done**) |
| `consensus/protocol/transaction.ts` `TransactionManager`, `GasFeeEstimator` | Nonce mgmt, fee bump, resubmission, pending-check loop | `safenet-core::tx::TransactionQueue` (**done**) |
| `consensus/protocol/sqlite.ts` `SqliteTxStorage` | Nonce/status persistence | internal to `safenet-core::tx::TransactionQueue` (**done**) |
| `consensus/protocol/base.ts` `BaseActionQueue` + `utils/queue.ts` `SqliteQueue` | Persistent FIFO action queue + serialized retry/timeout loop | subsumed by `safenet-core::tx::TransactionQueue` (durable queue with per-block resubmission and block-number expiry); no separate action queue needed |
| account / signing | `viem` account | `safenet-core::tx::Signer` (**done**) |
| `utils/logging.ts`, `utils/metrics.ts` | Logging + Prometheus | `safenet-core::observability` (**done**) |
| `consensus/verify/oracleTx/hashing.ts`, `.../safeTx/hashing.ts` | `requestId` (EIP-712) | `hashing.rs` via `alloy` `sol!` + `SolStruct::eip712_signing_hash` |
| `machine/transitions/types.ts` `OracleTransactionProposedEvent` | Consensus event shape | `bindings.rs` + `transitions.rs` |
| `types/schemas.ts` `sentinelConfigSchema` (`zod`) | Env validation | `config.rs` (`serde` + TOML via `argh`, like the `validator` crate) |

---

## Architecture Decision

The crate is an `async` (`tokio`) **binary** that consumes `safenet-core`, mirroring the language
choices already made for the core crate so the two compose cleanly.

| Concern | TypeScript today | Rust choice | Notes |
| --- | --- | --- | --- |
| RPC / primitives / signing / EIP-712 | `viem` | **`alloy`** | `Provider`, `Address`/`B256`/`U256`/`Bytes`, `PrivateKeySigner`, typed events & EIP-712 via `sol!`. |
| Indexing | `shared/watcher.ts` | **`safenet-core::index`** | `Watcher<P, E>` over a typed event set; reuse, don't re-port. |
| Service state + reorg rollback | `sentinel/storage.ts` (no rollback) | **`safenet-core::state`** | `StateMachine` drives pure `StateTransition`s and `SnapshotStore` rolls back on reorg. |
| Transaction submission + queueing | `transaction.ts` + `SqliteQueue`/`BaseActionQueue` | **`safenet-core::tx::TransactionQueue`** | Durable queue with nonce mgmt, fee bump/resubmit, in-flight cap and block-number expiry; subsumes both the TS tx manager and the action queue. |
| Signing | `viem` account | **`safenet-core::tx::Signer`** | Local `Signer` (k256) owned by the `TransactionQueue`. |
| SQLite | `better-sqlite3` (sync) | **`sqlx`** (async sqlite) | One shared `SqlitePool` across the snapshot store and the tx queue's storage — matching the single-`Database` TS pattern. |
| Logging / metrics | `winston` / `prom-client` | **`safenet-core::observability`** | `tracing` + Prometheus. |
| `requestId` hashing | `viem hashTypedData` | **`alloy` `SolStruct::eip712_signing_hash`** | Onchain-identical *and* native-speed; the perf-sensitive hot path. |
| Config validation | `zod` | **`serde` + `toml` + `argh`** | TOML config file loaded via an `argh` CLI, mirroring the `validator` crate. |
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

- **Reuse `safenet-core::tx::TransactionQueue` for submission, queueing and nonce management.** The
  TS sentinel layered a `SqliteQueue`/`BaseActionQueue` (durable action queue + retry/timeout) on top
  of a `TransactionManager` + `SqliteTxStorage` + `GasFeeEstimator`. The Rust `TransactionQueue`
  collapses all of that into one component: it persists each queued transaction, assigns nonces,
  submits, and on every block update marks executed/pruned/resubmits stale transactions with bumped
  fees. So the sentinel needs **no separate action queue** — it maps each `SentinelAction` to a
  `tx::Transaction { to, value, data, gas }` and calls `queue.queue(tx, expires_at)`. Fees come from
  `alloy`'s `estimate_eip1559_fees` (with the queue's `priority_fee_cap_percentage`), so the TS
  `gasFeeEstimator.invalidate()` maps to nothing. Two consequences to handle in `protocol.rs`:
  (a) `tx::Transaction.gas` is **mandatory** — the queue does not estimate gas — so the protocol must
  set a gas limit per action (estimate via `provider.estimate_gas`, or use per-action constants);
  (b) `expires_at` is a **block number** (not the TS 10-minute wall-clock TTL), so it is derived from
  the relevant request deadline / voting window.

- **Per-block updates feed both the state machine and the tx queue.** The service hands each
  `Update::Block(BlockUpdate)` to both `StateMachine::handle_update` (driving the request FSM, which
  on a new block runs `handleBlockAdvance`) and `TransactionQueue::handle_block_update` (the per-block
  housekeeping that replaces the TS `triggerPendingCheck`, and which also processes `Uncle`/`Warp`).
  The latter must see every block update, not just `New`.

- **Reorg behavior is now first-class but submission stays best-effort.** The `StateMachine` rolls
  request state back to the reorg's common ancestor and re-applies forward; the `TransactionQueue` is
  itself reorg-aware (it un-marks executed transactions on `Uncle`) but never "un-submits" a
  broadcast transaction. An action emitted from a transition that is later reorged out cannot be
  un-sent — the same best-effort property the TS sentinel has — but on re-application the transition
  re-emits it, and the queue's nonce reuse means the resubmission replaces rather than duplicates.
  This is an improvement over the TS sentinel, which detects reorgs but does not roll state back at
  all.

- **Pure transition, separated from I/O.** Keeping the FSM in the `StateTransition` impl (no async
  I/O inside `new_block`/`event`) keeps the onchain-semantics-critical logic trivially
  unit-testable for parity and isolated from the async plumbing.

- **TOML config + `argh` CLI, mirroring the `validator` crate.** The binary exposes an `argh`
  `Options` (`--config-file`, default `sentinel.toml`; `--version`) and a `Config` struct that
  deserializes from TOML with `#[serde(default)]` and an async `Config::load(&Path)`, exactly like
  `crates/validator/src/{main,config}.rs`. The `Config` composes the core module configs it needs
  (`observability::Config`, `index::Config`, `tx::Config`) plus the sentinel-specific fields (RPC URL,
  signer key, chain id, oracle / consensus / fee-token addresses, voting window, blocklist). This
  replaces the TS `zod`/env-var layout; config compatibility is not required, so the env-var scheme is
  dropped in favour of the TOML file the rest of the Rust workspace uses.

- **Workspace-inherited dependencies and a bare crate name.** Like `validator`, the crate is named
  plainly (`sentinel`, `version = "0.2.0"`, `edition = "2024"`, `publish = false`) and pulls every
  shared dependency from `[workspace.dependencies]` via `dep.workspace = true`
  (`alloy`, `argh`, `serde`, `serde_json`, `sqlx`, `thiserror`, `tokio`, `toml`, `tracing`, and
  `url`/`futures` as needed), with `safenet-core.path = "../core"`. No crate-local version pins, so
  versions stay aligned across the workspace.

### Alternatives Considered

- **Re-implement indexing / state / tx submission inside the sentinel crate.** Rejected — duplicates
  `safenet-core` and violates the reuse guideline. Every module the sentinel needs (`index`,
  `observability`, `state`, `tx`) has already landed in the core crate, so there is no build-ordering
  cost to reusing them.
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
  Cargo.toml                 # name = "sentinel"; workspace deps (dep.workspace = true) + safenet-core.path = "../core"
  # no migrations/ — the snapshot store and tx queue create/own their own tables in safenet-core
  src/
    main.rs                  # argh Options (--config-file/--version), Config::load, observability::init, build service, run
    config.rs                # TOML Config (observability + index + tx + sentinel fields), #[serde(default)], async load()
    error.rs                 # crate error type (thiserror)
    bindings.rs              # sol!: SentinelOracle + Consensus events/calls, ERC20; SafeTx / *Proposal EIP-712 structs
    hashing.rs               # request_id(domain, proposal) + safe_tx_hash(tx) via eip712_signing_hash (+ parity vectors)
    detector.rs              # blocklist Detector
    state.rs                 # SentinelRequestState FSM, SentinelAction, SentinelConfig, the S state type
    transition.rs            # StateTransition impl (new_block = handleBlockAdvance; event = per-log handlers)
    transitions.rs           # watcher_events! set + log -> SentinelOracleTransition decoding
    watcher.rs               # SentinelTransitionWatcher over safenet-core::index::Watcher
    protocol.rs              # action -> tx::Transaction (calldata + gas) -> safenet-core::tx::TransactionQueue::queue
    service.rs               # SentinelService: Watcher -> StateMachine + TransactionQueue
```

Modules are introduced only when first used (no empty stubs), matching the core crate's convention.

### Cargo manifest

Mirrors `crates/validator/Cargo.toml`: a bare package name and workspace-inherited dependencies, so
no versions are pinned in the crate itself. Any new shared dependency is added to the root
`[workspace.dependencies]` first, then referenced with `.workspace = true`.

```toml
[package]
name = "sentinel"
version = "0.2.0"
edition = "2024"
publish = false

[dependencies]
safenet-core.path = "../core"
alloy.workspace = true        # primitives, sol!, EIP-712, provider, signer
argh.workspace = true         # CLI
serde.workspace = true
serde_json.workspace = true   # snapshot/transition (de)serialization
sqlx.workspace = true         # shared SqlitePool
thiserror.workspace = true
tokio.workspace = true
toml.workspace = true         # config file
tracing.workspace = true
```

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

### Protocol — action submission (`protocol.rs`)

- Maps each `SentinelAction` to a `tx::Transaction { to, value, data, gas }`: the calldata is the
  `sol!`-generated call (`ERC20::approveCall`, `SentinelOracle::{commitApprove,commitDeny,finalize,
  claim}Call`), `value` is zero, and `gas` is set from a `provider.estimate_gas` (or a per-action
  constant). It then calls `TransactionQueue::queue(transaction, expires_at)`, where `expires_at` is
  the block by which the action is no longer useful (derived from the request deadline / voting
  window). No dedicated action queue is needed — `TransactionQueue` is the durable queue and owns
  retry, fee-bump resubmission, the in-flight cap and expiry.
- Ordering: actions are queued in the order the state machine returns them (e.g. `ApproveToken`
  before a commit), and the queue assigns nonces FIFO, preserving that order onchain.

### Service & binary (`service.rs`, `main.rs`, `config.rs`)

- `SentinelService` owns the `StateMachine<S, SentinelTransition>` (built from a `SnapshotStore` on
  the shared pool), the watcher, and the `TransactionQueue`. Its loop: pull the next `Update` from
  the watcher; when it is `Update::Block(b)`, hand `b` to `TransactionQueue::handle_block_update`
  (the per-block housekeeping that replaces `triggerPendingCheck`); pass the `Update` to
  `state_machine.handle_update`; and for each returned `SentinelAction`, map it via `protocol.rs` and
  `TransactionQueue::queue` it. Exposes `run`/`shutdown`.
- `main.rs`: load `Config`, `safenet_core::observability::init`, build the `alloy` provider and a
  `tx::Signer` from the configured key, open the shared `SqlitePool`, construct the `Watcher`,
  `StateMachine` and `TransactionQueue`, run the service, and handle SIGINT/SIGTERM.

### Testing

- Unit tests mirror the TS test intent (behavior, not implementation): the `StateTransition`
  (FSM transitions, vote-won/timeout, deadline cleanup), `request_id` parity vectors, transition
  decoding, and the action → `tx::Transaction` mapping (calldata/gas/expiry). Reorg rollback of
  request state and tx resubmission are already covered by core's `StateMachine`/`TransactionQueue`
  tests, but a sentinel-level test should assert a reorged `NewRequest` is rolled back.
- `sqlx` tests run against `sqlite::memory:` (matching the TS `:memory:` default).
- An interop/integration test (Phase F) runs the Rust sentinel against a `SentinelOracle` on Anvil.

### Tooling

- Per `AGENTS.md`: `cargo fmt --all`, `cargo clippy --package sentinel`,
  `cargo test --package sentinel`. `Cargo.lock` committed.
- Dependency features are inherited from `[workspace.dependencies]` (e.g. `tokio`/`alloy` are already
  `full` there), so the crate pins no versions or features of its own; new shared deps go to the
  workspace root.

---

## Implementation Phases

Each PR has a single purpose, targets < 300 changed LOC and < 10 files, and is independently
reviewable. "Depends on" lists hard ordering; everything else may proceed in parallel.

### Phase A — Foundation, bindings & EIP-712 (blocks all other phases)

- **A1 — Crate scaffolding & dependencies.** New `crates/sentinel` binary crate (package `sentinel`),
  picked up by the existing `members = ["crates/*"]` glob; dependencies inherited from
  `[workspace.dependencies]` via `dep.workspace = true` plus `safenet-core.path = "../core"` (see the
  Cargo manifest above), adding any missing shared dep to the workspace root first; commit
  `Cargo.lock`. A minimal `argh` + `Config::load` + `observability::init` `main.rs` (like the
  `validator` scaffold) is enough to get a runnable binary. No empty module stubs.
  _Single purpose: scaffold the crate and its dependencies._
- **A2 — Onchain bindings.** `bindings.rs`: `sol!` for SentinelOracle + Consensus events/calls and
  ERC-20, plus the `SafeTx`/`TransactionProposal`/`OracleTransactionProposal` EIP-712 structs.
  Depends on A1.
- **A3 — `requestId` hashing + parity vectors.** `hashing.rs`: `request_id` and `safe_tx_hash` via
  `eip712_signing_hash`, with tests asserting byte-identical output vs TS/contract vectors. Depends
  on A2. _The onchain-compatibility linchpin and the perf-sensitive hot path._

### Phase B — Domain logic (depends on A; parallel with C & D2)

- **B1 — State types & detector.** `state.rs` (`SentinelRequestState`, `SentinelAction`, the
  snapshot state `S`, `SentinelConfig`) and `detector.rs` (blocklist) + tests. Depends on A1.
- **B2 — State transition.** `transition.rs`: the `StateTransition` impl (`new_block` +
  `event`), porting all five handlers, with unit tests for the FSM, vote-won/timeout, and deadline
  cleanup. Depends on A3 (`request_id`), B1.

### Phase C — Indexing integration (depends on A; parallel with B & D2)

- **C1 — Event set & transition decoding.** `transitions.rs`: `watcher_events!` over the two event
  enums and `log → SentinelOracleTransition`, with decoding tests. Depends on A2.
- **C2 — Transition watcher.** `watcher.rs`: wrap `safenet-core::index::Watcher` for
  `[oracle, consensus]`, resuming from the snapshot store. Depends on C1 and `safenet-core::index`
  (done).

### Phase D — Action submission

- **D2 — Sentinel protocol.** `protocol.rs`: map each `SentinelAction` to a `tx::Transaction`
  (`sol!` calldata + gas) and `TransactionQueue::queue(tx, expires_at)` it; tests for the mapping.
  Depends on A2 and `safenet-core::tx` (done). _(There is no longer a D1 — the TS action queue is
  subsumed by `TransactionQueue`.)_

### Phase E — Orchestration & binary

- **E1 — SentinelService.** `service.rs`: build the `StateMachine` over a `SnapshotStore` and the
  `TransactionQueue` over the shared pool; run the watcher loop, feeding each `Update::Block` to both
  the state machine and `TransactionQueue::handle_block_update`, and queueing the actions the state
  machine returns. Depends on B2, C2, D2.
- **E2 — Config & binary.** Flesh out `config.rs` (the composed TOML `Config`) and `main.rs` (the
  `argh` `Options`, `Config::load`, `observability::init`, building the provider + `tx::Signer` +
  shared pool, then running the service) — extending the A1 scaffold to the full wiring. Depends on E1.

### Phase F — Validation & wrap-up

- **F1 — Interop/integration test.** Run the Rust sentinel against a `SentinelOracle` on Anvil
  (extend `scripts/`/integration tests), asserting it commits/finalizes/claims correctly and
  interoperates with the TS sentinel in a shared dispute — the deliverable's "work together onchain"
  acceptance check. Depends on E2.
- **F2 — Docs & cleanup.** Update `README.md`/`AGENTS.md` to list the new `sentinel` crate. No
  per-crate feature narrowing is needed since dependency features are inherited from
  `[workspace.dependencies]`; if any shared dependency's feature set was widened for the sentinel,
  reconcile it at the workspace level here. Depends on all implementation phases.
- **Cleanup.** Per the planning convention, **remove this epic file**
  (`epics/2026_06_25_rust_sentinel_port.md`) once the epic is complete.

### Critical path

`A1 → A2 → A3 → B2 → E1 → E2 → F1`. After A, phases B, C and D2 proceed concurrently; every
`safenet-core` module the sentinel consumes (`index`, `observability`, `state`, `tx`) is already
available, so there is no external gating dependency.

---

## Open Questions and Assumptions

**Open questions**

1. **Which "computation" is the performance problem?** The task cites "nonce computation." The
   sentinel does no FROST nonce work; the plausible hot paths are (a) the per-`OracleTransactionProposed`
   EIP-712 `requestId` hashing (addressed natively by `alloy` `sol!` keccak) and (b) transaction
   nonce management (now handled by `safenet-core::tx::TransactionQueue`). **Recommended:** confirm the
   target and add a throughput benchmark for it as an acceptance criterion (likely the `requestId`
   hashing in A3).
2. **`expires_at` and gas for queued actions.** `TransactionQueue::queue` takes a block-number
   expiry and `tx::Transaction.gas` is mandatory (the queue does not estimate gas). **Recommended:**
   derive `expires_at` from the request deadline / voting window, and set `gas` from a
   `provider.estimate_gas` per action (falling back to per-action constants if estimation is
   undesirable). Confirm the exact expiry policy per action type.
3. **Fee configuration mapping.** The TS entrypoint attaches `ChainFees`
   (`baseFeeMultiplier`, `maxPriorityFeePerGas`) via `viem extractChain`. The Rust
   `tx::Config` exposes `priority_fee_cap_percentage`, `max_in_flight_transactions` and
   `blocks_before_resubmit`, and `alloy`'s `estimate_eip1559_fees` handles base-fee headroom
   internally. **Recommended:** map `PRIORITY_FEE_CAP_PERCENTAGE` → `priority_fee_cap_percentage` and
   drop the bespoke base-fee multiplier / explicit priority fee unless a concrete need surfaces;
   confirm.

**Assumptions**

- **Onchain compatibility** means identical `requestId` derivation, event decoding, ABI calldata,
  `ResolveReason` decoding, and FSM-driven action timing — **not** DB or config compatibility (per
  the deliverable).
- The port **reuses `safenet-core`** for indexing, observability, reorg-aware state, transaction
  submission, and the SQLite foundation rather than re-implementing them.
- The crate **follows the sibling `validator` crate's conventions**: package name `sentinel`,
  workspace-inherited dependencies (`dep.workspace = true`), an `argh` CLI, and a TOML config file
  (`sentinel.toml`). This settles the earlier crate-name and config-format questions.
- It is a greenfield Rust **binary**; the TS sentinel remains the reference implementation during the
  port, with no FFI, and both can run against the same `SentinelOracle`.
- `alloy` is the EVM library; EIP-712 hashing is done via `sol!` structs and
  `SolStruct::eip712_signing_hash`.
- The sentinel FSM is implemented as a pure `safenet-core::state::StateTransition` and tested for
  behavior (not implementation), mirroring the existing `*.test.ts` intent.
- Following the planning convention, this plan is proposed as a **docs-only PR** containing no epic
  implementation code.
