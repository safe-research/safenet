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

Motivation: the goal is **a single shared Rust codebase with as much shared code as possible**, not
a performance fix. The validator is being ported to Rust to address its own concerns (and to use the
p2p support available there); the sentinel follows so both services live in one codebase and reuse
the `safenet-core` building blocks. There is no concrete performance problem in the sentinel itself,
and it does not need p2p — code unification is the reason.

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
follows the conventions of the sibling `validator` crate (`crates/validator`, PR #489): a bare
package name, workspace-inherited dependencies, an `argh` CLI, and a TOML configuration file.

This port **builds on the `safenet-core` crate** (`crates/core`) rather than re-implementing shared
infrastructure, per the `AGENTS.md` "reuse, don't reinvent" guideline. The core crate provides the
typed event **indexer** (`index`), **observability** (`observability`), the **reorg-aware state
machine + snapshot storage** (`state`), and **reliable transaction submission** (`tx`: a
`TransactionQueue` with a local `Signer`). It is also gaining a **`Driver`** (PR #486, in flight)
that ties those together for a service — the sentinel is written against that abstraction (see the
Architecture Decision).

The TS sentinel remains the reference implementation throughout the port; there is no FFI and the
two are independent processes.

---

## How the TypeScript sentinel works (port surface)

A faithful port needs the whole data flow, so it is catalogued here. File references are under
`validator/src/`.

| TS file | Responsibility | Rust home |
| --- | --- | --- |
| `sentinel.ts` | Entrypoint: parse env config, build account/metrics/transport/client, open SQLite, start `SentinelService`, signal handling. | `main.rs` + `config.rs` |
| `sentinel/service.ts` | `SentinelService`: wires storage + action queue + tx storage + tx manager + protocol + watcher; routes transitions; applies diffs; enqueues actions. | the loop is `safenet-core::driver::Driver`; the sentinel just supplies a `Service` impl (`service.rs`) |
| `sentinel/types.ts` | `SentinelAction` (5 variants), `SentinelRequestState` FSM (`preparing`→`pending`→`committed`→`finalized`), `SentinelConfig`. | `state.rs` |
| `sentinel/handlers.ts` | **Pure** functions: `handleOracleTransactionProposed` (compute `requestId`, run detector), `handleNewRequest`, `handleCommitted`, `handleResolved` (decode `ResolveReason`, vote-won logic), `handleBlockAdvance` (deadline FSM + finalize/cleanup). | the `StateTransition` (`new_block` + `event`) inside the `Service` impl (`service.rs`) |
| `sentinel/transitions.ts` | Decode raw logs → `SentinelOracleTransition` (`zod`-validated args). | folded into the `sol!`-generated typed event set (`bindings.rs`); the `event` handler matches on it directly |
| `sentinel/protocol.ts` | `SentinelActionQueue` (`SqliteQueue`) + `SentinelProtocol` (`BaseActionQueue`): action → ABI-encoded tx via `TransactionManager`. | `Service::encode_actions` (action → `tx::Transaction`); the queue/retry is the `Driver` + `TransactionQueue` |
| `sentinel/storage.ts` | `SentinelStateStorage`: plain SQLite key/value table of request states (**not** reorg-rolled-back). | `safenet-core::state::storage::SnapshotStore` (reorg-aware) |
| `sentinel/watcher.ts` | `SentinelTransitionWatcher` over the shared `BlockchainWatcher`, address-filtered to `[oracle, consensus]`. | a `safenet-core::index::Watcher` over the typed event set, owned by the `Driver` |
| `sentinel/detector.ts` | Blocklist detector: approve unless `payload.to` is blocklisted. | `detector.rs` |
| `sentinel/abis.ts` | Event + function ABIs (SentinelOracle, ERC-20). | `bindings.rs` (`sol!`) |

Shared dependencies the sentinel pulls in (and their Rust replacements):

| TS dependency | Used for | Rust replacement |
| --- | --- | --- |
| `shared/watcher.ts` `BlockchainWatcher` | Block + event indexing, reorg detection | `safenet-core::index::Watcher` (**done**) |
| (new) reorg-aware state | persist + roll back service state on reorg | `safenet-core::state::{StateMachine, storage::SnapshotStore}` (**done**) |
| `consensus/protocol/transaction.ts` `TransactionManager`, `GasFeeEstimator` | Nonce mgmt, fee bump, resubmission, pending-check loop | `safenet-core::tx::TransactionQueue` (**done**) |
| `consensus/protocol/sqlite.ts` `SqliteTxStorage` | Nonce/status persistence | internal to `safenet-core::tx::TransactionQueue` (**done**) |
| `consensus/protocol/base.ts` `BaseActionQueue` + `utils/queue.ts` `SqliteQueue` | Persistent FIFO action queue + serialized retry/timeout loop | subsumed by the `Driver` + `TransactionQueue`; no separate action queue |
| service orchestration (the `SentinelService` wiring) | drive indexing → state → submission | `safenet-core::driver::{Driver, Service}` (**PR #486, in flight**) |
| account / signing | `viem` account | `safenet-core::tx::Signer` (**done**) |
| `utils/logging.ts`, `utils/metrics.ts` | Logging + Prometheus | `safenet-core::observability` (**done**) |
| `consensus/verify/oracleTx/hashing.ts`, `.../safeTx/hashing.ts` | `requestId` (EIP-712) | `hashing.rs` via `alloy` `sol!` + `SolStruct::eip712_signing_hash` |
| `machine/transitions/types.ts` `OracleTransactionProposedEvent` | Consensus event shape | `bindings.rs` (`sol!`) |
| `types/schemas.ts` `sentinelConfigSchema` (`zod`) | Env validation | `config.rs` (`serde` + TOML via `argh`, like the `validator` crate) |

---

## Architecture Decision

The crate is an `async` (`tokio`) **binary** that consumes `safenet-core`, mirroring the choices of
the `validator` crate so the two compose cleanly.

| Concern | TypeScript today | Rust choice | Notes |
| --- | --- | --- | --- |
| RPC / primitives / signing / EIP-712 | `viem` | **`alloy`** | `Provider`, `Address`/`B256`/`U256`/`Bytes`, typed events & EIP-712 via `sol!`. |
| Service orchestration | the `SentinelService` wiring | **`safenet-core::driver::{Driver, Service}`** | The sentinel implements `Service`; the `Driver` runs indexing + state + submission. PR #486. |
| Indexing | `shared/watcher.ts` | **`safenet-core::index`** | `Watcher<P, E>` over a typed event set; reuse, don't re-port. |
| Service state + reorg rollback | `sentinel/storage.ts` (no rollback) | **`safenet-core::state`** | `StateMachine` drives a pure `StateTransition` and `SnapshotStore` rolls back on reorg. |
| Transaction submission + queueing | `transaction.ts` + `SqliteQueue`/`BaseActionQueue` | **`safenet-core::tx::TransactionQueue`** | Durable queue with nonce mgmt, fee bump/resubmit, in-flight cap and per-tx block expiry; subsumes both the TS tx manager and the action queue. |
| Signing | `viem` account | **`safenet-core::tx::Signer`** | Local `Signer` (k256) owned by the `TransactionQueue`. |
| SQLite | `better-sqlite3` (sync) | **`sqlx`** (async sqlite) | One shared `SqlitePool` across the snapshot store and the tx queue's storage — matching the single-`Database` TS pattern. |
| Logging / metrics | `winston` / `prom-client` | **`safenet-core::observability`** | `tracing` + Prometheus. |
| `requestId` hashing | `viem hashTypedData` | **`alloy` `SolStruct::eip712_signing_hash`** | Onchain-identical; the linchpin of compatibility. |
| Config validation | `zod` | **`serde` + `toml` + `argh`** | TOML config file loaded via an `argh` CLI, mirroring the `validator` crate. |
| Errors | `viem` `BaseError` | **`thiserror`** | Per-module error enums where useful; no crate-level error module (this is a binary). |

Key decisions:

- **Implement `safenet-core::driver::Service` and let the `Driver` run everything.** The core
  `Driver` (PR #486) owns the `Watcher`, `StateMachine` and `TransactionQueue` and runs the loop:
  on each indexer `Update` it feeds block updates to the transaction queue's per-block housekeeping,
  advances the state machine, and queues the transactions the actions encode to. A `Service` is just:

  ```rust
  pub trait Service: StateTransition<Self::State> {
      type State: Serialize + DeserializeOwned;
      fn encode_actions(&self, actions: Vec<Self::Action>) -> Vec<(Transaction, u64)>;
  }
  ```

  So the sentinel supplies one type implementing `StateTransition` (`new_block` + `event`) **plus**
  `encode_actions`, and `main.rs` constructs the `Watcher`/`StateMachine`/`TransactionQueue`, hands
  them with the `Service` to `Driver::new`, and calls `Driver::run`. There is **no** sentinel-side
  service loop, action queue, or manual block routing — those are the `Driver`'s job. (The maintainer
  is driving PR #486 partly to validate it against the sentinel and the validator, so some
  initialization may move into the `Driver` as it lands; the plan tracks the `Driver` API.)

- **The `Service::Event` is the `sol!`-generated typed event set — no separate transition type.**
  Because the `Driver` ties `Watcher<P, S::Event>` to `StateMachine<S::State, S>`, the state
  machine's `Event` *is* the watcher's event type, which must implement
  `safenet-core::index::events::Events`. The sentinel defines that set with the `watcher_events!`
  macro over the `sol!`-generated `SentinelOracle::*Events` and `Consensus::*Events` enums, and
  `StateTransition::event` matches on it directly. The small amount of arg massaging the TS
  `transitions.ts` did with `zod` becomes reading typed fields off the decoded event. (This also
  removes the `Update<RawEvent>` vs `Update<SentinelOracleTransition>` mismatch a reviewer flagged on
  the earlier draft: there is only one event type.)

- **The sentinel FSM is a pure `StateTransition`.** `S` = the request map (`SentinelRequestState`
  keyed by `requestId`), `Serialize + DeserializeOwned + Default`. `new_block(state, block)` ports
  `handleBlockAdvance`; `event(state, event)` ports the per-event handlers. Transitions are
  infallible ("gracefully recover"), which fits the TS handlers — they already return "no change" on
  unexpected input. The `StateMachine` persists `S` to the `SnapshotStore` per block and rolls it
  back to the common ancestor on a reorg (an improvement over the TS sentinel, which detects reorgs
  but does not roll back). Keeping the FSM pure (no async I/O in `new_block`/`event`) keeps the
  onchain-semantics-critical logic trivially unit-testable for parity.

- **`requestId` derivation via `alloy` `sol!` EIP-712 is the linchpin of onchain compatibility.**
  Declare `SafeTx`, `TransactionProposal`, and `OracleTransactionProposal` as `sol!` structs and
  compute their hashes with `SolStruct::eip712_signing_hash(domain)`. This produces byte-identical
  output to the contract and the TS `viem hashTypedData`. Field types must match the onchain
  typehashes exactly (see Tech Specs). Parity is locked down with test vectors captured from the TS
  implementation / contract (PR A3).

- **Actions encode to transactions; the queue owns submission and fees.** `encode_actions` maps each
  `SentinelAction` to a `(tx::Transaction, expires_at)` pair — calldata from the `sol!`-generated
  call, and `expires_at` the block by which it is no longer useful. For the sentinel that expiry is
  the **`SentinelOracle` voting deadline** carried on the request (each action is stamped with it in
  the transition, as the TS `SentinelActionWithTimeout` did). **Fees are handled entirely by the
  `TransactionQueue`** (estimate + cap + replacement bump); the sentinel does not compute fees.

- **TOML config + `argh` CLI, mirroring the `validator` crate.** The binary exposes an `argh`
  `Options` (`--config-file`, default `sentinel.toml`; `--version`) and a `Config` struct that
  deserializes from TOML with `#[serde(default)]` and an async `Config::load(&Path)`, like
  `crates/validator/src/{main,config}.rs` (and the `validator-rust/src/config` spike). The `Config`
  composes the core module configs it needs plus the sentinel-specific fields (RPC URL, signer key,
  chain id, oracle / consensus / fee-token addresses, voting window, blocklist). Config parameters
  the TS port had that have no Rust analog (e.g. the base-fee multiplier) are simply dropped.

- **Workspace-inherited dependencies and a bare crate name.** Like `validator`, the crate is named
  plainly (`sentinel`, `version = "0.2.0"`, `edition = "2024"`, `publish = false`) and pulls every
  shared dependency from `[workspace.dependencies]` via `dep.workspace = true`, with
  `safenet-core.path = "../core"`. New shared deps go to the workspace root first.

### Alternatives Considered

- **Hand-wire the watcher, state machine and transaction queue in a bespoke service loop.** Rejected
  — that is exactly what the core `Driver` exists to do generically; the sentinel only supplies a
  `Service`. (An action queue is likewise unnecessary — it is baked into the `Driver` +
  `TransactionQueue`.)
- **A separate `SentinelOracleTransition` type decoded from raw logs.** Rejected — the `Driver` ties
  the state machine's `Event` to the watcher's typed event set, so the `sol!` event enum is used
  directly; a second type would just need a redundant conversion.
- **A plain, non-reorg request table mirroring the TS `SentinelStateStorage`.** Rejected — core's
  reorg-aware `SnapshotStore` is the blessed pattern and is strictly more correct under reorgs. DB
  compatibility is not required, so adopting it costs nothing in scope.
- **Hand-roll EIP-712 / `keccak` for `requestId`.** Rejected — `alloy`'s `sol!` + `SolStruct` is the
  canonical, contract-matching implementation and removes a whole class of compatibility bugs.
- **`rusqlite` (sync) instead of `sqlx`.** Rejected — core's state and tx storage are `sqlx`-backed
  and the sentinel shares one pool with them, as the TS code shares one `Database`.
- **Keep TS as a subprocess / add FFI.** Rejected — a single shared Rust codebase is the goal.
- **A crate-level `error.rs` module.** Rejected — crate-level error modules do not work well outside
  libraries; this is a binary, so errors are per-module (`thiserror`) and `main` returns
  `Box<dyn Error>` as the `validator` binary does.

---

## Tech Specs

### Crate layout

```
crates/sentinel/
  Cargo.toml                 # name = "sentinel"; workspace deps (dep.workspace = true) + safenet-core.path = "../core"
  # no migrations/ — the snapshot store and tx queue create/own their own tables in safenet-core
  src/
    main.rs                  # argh Options (--config-file/--version), Config::load, observability::init,
                             #   build provider + Signer + pool + Watcher + StateMachine + TransactionQueue,
                             #   then Driver::new(service, ...).run()
    config.rs                # TOML Config (observability + index + tx + sentinel fields), #[serde(default)], async load()
    bindings.rs              # sol!: SentinelOracle + Consensus events/calls, ERC20; SafeTx / *Proposal EIP-712 structs;
                             #   watcher_events! event set (the Service::Event)
    hashing.rs               # request_id(domain, proposal) + safe_tx_hash(tx) via eip712_signing_hash (+ parity vectors)
    detector.rs              # blocklist Detector
    state.rs                 # SentinelRequestState FSM, SentinelAction (carrying expiry), the S state type
    service.rs               # the Service impl: StateTransition (new_block/event) + encode_actions
```

Modules are introduced only when first used (no empty stubs), matching the core crate's convention.
The transition handlers may be split into a `handlers.rs` if `service.rs` grows past the size budget.

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
serde_json.workspace = true   # snapshot state (de)serialization
sqlx.workspace = true         # shared SqlitePool
thiserror.workspace = true
tokio.workspace = true
toml.workspace = true         # config file
tracing.workspace = true
```

### Onchain bindings & hashing (`bindings.rs`, `hashing.rs`)

- `sol!` blocks for `SentinelOracle` (events `NewRequest`, `Committed`, `OracleResult`, `Claimed`,
  `DisputeResolved`; calls `commitApprove`, `commitDeny`, `finalize`, `claim`), `Consensus`
  (event `OracleTransactionProposed`), and an `ERC20` (`approve`, `allowance`). The event set the
  watcher/state machine use is declared with `watcher_events!` over the generated `*Events` enums.
  Use the existing Safenet `sol!` bindings as the model:
  `validator-rust/src/bindings.rs` (and the `crates/validator` bindings as they land). Note: `alloy`
  types are an area LLMs tend to get wrong — expect to hand-hold this PR against a compiler.
- `SafeTx`, `TransactionProposal`, `OracleTransactionProposal` declared as `sol!` EIP-712 structs.
  **The field types must match the canonical onchain Solidity typehashes exactly.** The EIP-712 type
  hash is sensitive to the precise type of every field, so any mismatch (e.g. declaring `epoch` as
  `uint256` when it is `uint64`, or `operation` as the wrong width) yields a different `requestId` and
  silently breaks onchain compatibility. The source of truth is the contracts' precomputed typehashes
  in `contracts/src/libraries/ConsensusMessages.sol` —
  `OracleTransactionProposal(uint64 epoch,address oracle,bytes32 safeTxHash)` and
  `TransactionProposal(uint64 epoch,bytes32 safeTxHash)` — together with the canonical Safe `SafeTx`
  type (`to address, value uint256, data bytes, operation uint8, safeTxGas uint256, baseGas uint256,
  gasPrice uint256, gasToken address, refundReceiver address, nonce uint256`).
- `safe_tx_hash(tx)` uses domain `{chainId: tx.chainId, verifyingContract: tx.safe}`;
  `request_id(domain, proposal)` uses domain `{chainId, verifyingContract: consensus}`. Both via
  `SolStruct::eip712_signing_hash`.
- **Parity tests are mandatory** (PR A3): assert byte-identical hashes against vectors captured from
  the TS functions and/or a live contract `eth_call`, covering the exact values exercised by
  `handleOracleTransactionProposed`.

### State, transition, detector & action encoding (`state.rs`, `service.rs`, `detector.rs`)

- `state.rs`: `SentinelRequestState` = `{ deadline, approve: bool, status }` with
  `status ∈ {Preparing, Pending, Committed, Finalized}`; `SentinelAction` =
  `{ApproveToken{bond}, CommitApprove{id}, CommitDeny{id}, Finalize{id}, Claim{id}}`, each carrying
  the expiry block (the request's voting deadline) so `encode_actions` can forward it; and the
  snapshot state `S` (the request map). `S` derives `Serialize + Deserialize + Default`; keyed by
  `requestId` (`B256` serializes as a hex string usable as a JSON map key, or store `Vec<(B256, …)>`).
- `service.rs`: a struct (holding the config addresses, chain id, voting window and detector) that
  implements `StateTransition<S>` and `Service`:
  - `new_block` ports `handleBlockAdvance` (past-deadline: `committed → finalized` + a `Finalize`
    action; otherwise drop).
  - `event` matches the typed event set and ports `handleOracleTransactionProposed` (oracle-address
    gate, `request_id`, detector → `approve`, `deadline = block + votingWindow`), `handleNewRequest`
    (acts only on `preparing`; emits `ApproveToken` + commit), `handleCommitted` (own-address gate,
    `pending → committed`), and `handleResolved` — which **drops the request unconditionally but only
    emits a `Claim` when we actually committed onchain** (status `committed` or `finalized`; for any
    other status drop silently, since our commit tx may never have confirmed), and when committed
    decodes `ResolveReason` and claims iff `voteWon = reason == TIMEOUT(2) || approved == ours`.
  - `encode_actions` maps each `SentinelAction` to `(tx::Transaction, expires_at)`: calldata is the
    `sol!`-generated call (`ERC20::approveCall`, `SentinelOracle::{commitApprove,commitDeny,finalize,
    claim}Call`), `value` is zero, gas is set per action, and `expires_at` is the stamped voting
    deadline. Fees are the `TransactionQueue`'s responsibility.
- `detector.rs`: `approve` unless `payload.to` ∈ blocklist (address-equality).

### Binary (`main.rs`, `config.rs`)

- `config.rs`: a `Config` deserialized from TOML (`#[serde(default)]`, async `load(&Path)`),
  composing `observability::Config`, `index::Config`, `tx::Config` and the sentinel fields. Mirrors
  `crates/validator/src/config.rs`.
- `main.rs`: an `argh` `Options` (`--config-file` default `sentinel.toml`, `--version`); load the
  `Config`; `observability::init`; build the `alloy` provider, a `tx::Signer` from the configured
  key, and the shared `SqlitePool`; construct the `Watcher` (over the `watcher_events!` set for
  `[oracle, consensus]`, resuming from the snapshot store), the `StateMachine` (over a
  `SnapshotStore`), the `TransactionQueue`, and the sentinel `Service`; then `Driver::new(...).run()`.
  Provider/signer/pool initialization lives directly in `main.rs`.

### Testing

- Unit tests mirror the TS test intent (behavior, not implementation): the `StateTransition`
  (FSM transitions, vote-won/timeout, deadline cleanup, the `handleResolved` status guard),
  `request_id` parity vectors, and the `encode_actions` mapping (calldata + expiry). Reorg rollback
  and tx resubmission are already covered by core's `StateMachine`/`TransactionQueue`/`Driver` tests;
  a sentinel-level test should assert a reorged `NewRequest` is rolled back.
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
  `[workspace.dependencies]` via `dep.workspace = true` plus `safenet-core.path = "../core"`; commit
  `Cargo.lock`. A minimal `argh` + `Config::load` + `observability::init` `main.rs` (like the
  `validator` scaffold) gives a runnable binary. No empty module stubs. _Single purpose: scaffold._
- **A2 — Onchain bindings.** `bindings.rs`: `sol!` for SentinelOracle + Consensus events/calls and
  ERC-20, the `SafeTx`/`TransactionProposal`/`OracleTransactionProposal` EIP-712 structs, and the
  `watcher_events!` event set. Modeled on the existing Safenet `sol!` bindings. Depends on A1.
- **A3 — `requestId` hashing + parity vectors.** `hashing.rs`: `request_id` and `safe_tx_hash` via
  `eip712_signing_hash`, with tests asserting byte-identical output vs TS/contract vectors. Depends
  on A2. _The onchain-compatibility linchpin._

### Phase B — Domain logic (depends on A; parallel with D)

- **B1 — State types & detector.** `state.rs` (`SentinelRequestState`, `SentinelAction` with expiry,
  the snapshot state `S`) and `detector.rs` (blocklist) + tests. Depends on A1.
- **B2 — Service: state transition.** `service.rs`: the `StateTransition` half of the `Service`
  (`new_block` + `event`), porting all five handlers, with unit tests for the FSM, vote-won/timeout,
  deadline cleanup and the `handleResolved` guard. Depends on A2 (typed events), A3 (`request_id`), B1.

### Phase D — Action encoding (depends on A; parallel with B)

- **D1 — `encode_actions`.** Complete the `Service` impl: map each `SentinelAction` to a
  `(tx::Transaction, expires_at)` (calldata + gas + voting-deadline expiry), with mapping tests.
  Depends on A2 and `safenet-core::tx` (done). Folds into `service.rs` alongside B2.

### Phase E — Orchestration & binary

- **E1 — Wire up the `Driver`.** `main.rs`: build the provider, `tx::Signer`, shared pool, `Watcher`,
  `StateMachine` (over a `SnapshotStore`) and `TransactionQueue`, construct the sentinel `Service`,
  and run `Driver::new(...).run()`. Depends on B2, D1, and `safenet-core::driver` (**PR #486**).
- **E2 — Config.** Flesh out `config.rs` (the composed TOML `Config`) and the `main.rs` argument /
  config plumbing. Depends on E1.

### Phase F — Validation & wrap-up

- **F1 — Interop/integration test.** Run the Rust sentinel against a `SentinelOracle` on Anvil
  (extend `scripts/`/integration tests), asserting it commits/finalizes/claims correctly and
  interoperates with the TS sentinel in a shared dispute — the deliverable's "work together onchain"
  acceptance check. Depends on E2.
- **F2 — Docs & cleanup.** Update `README.md`/`AGENTS.md` to list the new `sentinel` crate. No
  per-crate feature narrowing is needed (features are workspace-inherited); reconcile any widened
  shared dependency at the workspace level. Depends on all implementation phases.
- **Cleanup.** Per the planning convention, **remove this epic file**
  (`epics/2026_06_25_rust_sentinel_port.md`) once the epic is complete.

### Critical path

`A1 → A2 → A3 → B2 → E1 → E2 → F1`. After A, phases B and D proceed concurrently. Every
`safenet-core` module the sentinel consumes (`index`, `observability`, `state`, `tx`) is merged; the
one in-flight piece is the **`Driver`** (PR #486), which only `E1` needs — so all earlier phases can
proceed before it lands.

---

## Open Questions and Assumptions

**Open questions**

1. **`Driver` API / sequencing (PR #486).** The sentinel is written against `safenet-core::driver`,
   which is still in review. Its initialization may be simplified as it is validated against the
   sentinel and validator. **Recommended:** track the `Driver` API and start `E1` once it lands; all
   earlier phases are independent of it.
2. **Gas limits for encoded transactions.** `tx::Transaction.gas` is mandatory and the queue does not
   estimate it. **Recommended:** set `gas` per action via `provider.estimate_gas` (or per-action
   constants if estimation is undesirable); confirm.

**Assumptions**

- The motivation is **a single shared Rust codebase** (maximizing code shared with the validator),
  not performance and not p2p — the sentinel has no concrete performance problem.
- **Onchain compatibility** means identical `requestId` derivation, event decoding, ABI calldata,
  `ResolveReason` decoding, and FSM-driven action timing — **not** DB or config compatibility (per
  the deliverable). Config parameters with no Rust analog are dropped.
- The port **reuses `safenet-core`** for indexing, observability, reorg-aware state, transaction
  submission, the SQLite foundation, and the `Driver` orchestration rather than re-implementing them.
- The crate **follows the sibling `validator` crate's conventions**: package name `sentinel`,
  workspace-inherited dependencies (`dep.workspace = true`), an `argh` CLI, and a TOML config file
  (`sentinel.toml`); no crate-level error module.
- The sentinel is a `safenet-core::driver::Service`: a pure `StateTransition` (tested for behavior,
  mirroring the existing `*.test.ts` intent) plus an `encode_actions` mapping. Action expiry is the
  `SentinelOracle` voting deadline; fees are handled by the `TransactionQueue`.
- `alloy` is the EVM library; EIP-712 hashing is done via `sol!` structs and
  `SolStruct::eip712_signing_hash`.
- It is a greenfield Rust **binary**; the TS sentinel remains the reference implementation during the
  port, with no FFI, and both can run against the same `SentinelOracle`.
- Following the planning convention, this plan is proposed as a **docs-only PR** containing no epic
  implementation code.
