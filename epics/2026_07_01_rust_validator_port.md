# Plan: Port the TypeScript validator to Rust

Component: the existing `crates/validator` crate (Cargo package `validator`). Ports `validator/src/`
— the state machine (`service/`, `machine/`), the consensus protocol (`consensus/`), and the FROST
cryptography (`frost/`). No `safenet-core` changes are required.

---

## Overview

The Safenet validator is currently a TypeScript service (`validator/src/`, the largest client in the
repo). It follows the `Consensus` and `Coordinator` contracts, runs an epoch-rollover / distributed
key generation (DKG) state machine and a threshold-signing state machine over a custom **FROST**
(secp256k1) scheme, and submits the resulting commitments, shares, nonces, signature shares and
attestations onchain. This epic ports it to Rust on top of `safenet-core`.

The `crates/validator` crate already exists as a **scaffold**: `main.rs` wires a `DummyService`
through `safenet_core::Driver::new(..).run()`, and `config.rs` loads a TOML config with the driver
sub-config flattened in and observability nested (PRs #489, #497). Since this plan was first
drafted, core PR #512 (“Pure State Transition Functions”) and the sentinel component split reshaped
the service surface the port targets: `StateTransition` is now a **pure, synchronous** function
(`fn(&self, State, Message) -> (State, Commands)`), impure operations go through a new
**effect system** (`Command::Effect` → `EffectHandler::perform_effect` → `Message::Resume`), and a
`Service` is a **component bundle** (`components()` yielding a `Transition`, an `Effects` handler
and an `Actions` encoder). This plan is written against that surface — notably, the effect system
was introduced precisely for the validator’s impure needs (nonce burning, DKG coefficient reuse),
so the port uses it rather than the impure-transition design of the original draft.

This epic replaces the dummy with the real validator: a `Service` bundle whose pure `Transition`
ports the TS handlers over a snapshotted `State`, whose `Effects` handler owns the
locally-generated secrets (and the one startup RPC read), and whose `Actions` encoder ports the
onchain protocol encoding — plus the FROST library the transitions depend on.

Motivation matches the sibling sentinel port (`epics/2026_06_25_rust_sentinel_port.md`): **a single
shared Rust codebase built on `safenet-core`**, and — for the validator specifically — access to the
p2p stack that lives on the Rust side. As with the sentinel, the **hard requirement is onchain and
peer compatibility**: a Rust validator must produce byte-identical FROST commitments, proofs,
nonces, signature shares and attestations, so it can participate in the _same_ group and the _same_
signing session as a TypeScript validator and have the `Consensus`/`Coordinator` contracts accept its
transactions. Database and configuration backwards-compatibility are **not** required (the port picks
its own SQLite schema and TOML config).

The work divides into six phases, most of which parallelize after the foundation:

- **A** — validator **bindings** (`sol!` for `Consensus`/`Coordinator`) and **config**.
- **B** — the **FROST layer**: a thin Safenet-specific wrapper over the ZCash Foundation
  `frost-secp256k1` / `frost-core` crates (the standard RFC 9591 ciphersuite the scheme already uses) —
  DKG, signing, ECDH share encryption, nonce-tree preprocessing, merkle trees and solidity marshalling,
  with **parity vectors on the wrapping layers**.
- **C** — **state**: the snapshotted `State` (rollover + signing FSMs + consensus + the deterministic
  DKG material) plus the `Action` enum, and the **separate, reorg-immune secret store** for
  locally-generated random secrets (DKG coefficients + encryption key, and signing nonces), with
  pruning.
- **D** — the **state transitions and effects**: rollover/keygen/signing/attestation handlers as pure
  transitions, the `Effect`/`Resume` enums and the `EffectHandler` that fronts the secret store, and
  the effect-based staker-address reconciliation.
- **E** — **service assembly**: the `ActionEncoder` and wiring the real service into `main.rs`.
- **F** — **validation & cleanup**: an Anvil integration/interop test, docs, and removing this plan.

The TS validator remains the reference implementation throughout; there is no FFI and the two are
independent processes. The experimental spike on `project/ports` (`validator-rust/`, catalogued in
`epics/2026_06_09_rust_validator_port_index.md`) is a **snippet source only** (FROST DKG, marshalling,
`sol!` layout) — **the plan is the source of truth**, not the spike's architecture.

---

## How the TypeScript validator works (port surface)

A faithful port needs the whole data flow, so it is catalogued here. References are under
`validator/src/`.

| TS area                                                                  | Responsibility                                                                                                                                | Rust home                                                                                                                                          |
| ------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `validator.ts`                                                           | Entrypoint: parse env config, build account/metrics, construct + `start()` the service.                                                       | `main.rs` + `config.rs` (already scaffolded)                                                                                                       |
| `service/service.ts`                                                     | `ValidatorService`: wires storage + protocol + state machine + watcher; **`start()` reconciles the staker address**, then starts the watcher. | the loop is `safenet-core::driver::Driver`; the validator supplies the `Service` components; staker reconciliation becomes a `GetValidatorStaker` effect + `ValidatorStakerSet` handling (Phase D5) |
| `service/machine.ts`                                                     | `SafenetStateMachine`: transition **queue**, block/event dispatch, applies `StateDiff`s, yields `ProtocolAction`s.                            | subsumed by `safenet-core::state::StateMachine`; the validator implements the pure `StateTransition` + an `EffectHandler`                          |
| `machine/types.ts`, `machine/transitions/types.ts`                       | `RolloverState` / `SigningState` FSMs, `ConsensusState`, `StateDiff`, and the full typed event set.                                           | `state/` (the snapshot `State` + `Transition`) + the `sol!` event set in `service/mod.rs`                                                          |
| `consensus/rollover.ts`, `keygen/*`                                      | Epoch rollover, DKG rounds (genesis, commitments, shares, confirmations, complaints), timeouts.                                               | transition handlers (genesis DKG Phase D1; rollover + timeouts Phase D4)                                                                            |
| `consensus/signing/*`                                                    | Nonce-tree preprocessing, nonce-commitment reveal, signature-share creation (+ **nonce burn**), completion.                                   | transition handlers + nonce effects (Phase D3) + the secret store (Phase C1)                                                                       |
| `consensus/keyGen/client.ts`, `signing/client.ts`                        | Thin wrappers over `SqliteClientStorage` holding **all** crypto state.                                                                        | **removed**: deterministic crypto state moves into the snapshot `State`; random secrets move into the secret store behind the `EffectHandler` (see Architecture Decision) |
| `consensus/storage/sqlite.ts`                                            | 7-table SQLite store: groups, participants, secret shares, nonce links, nonces, signatures, commitments.                                      | **split**: non-secret crypto state → snapshot `State`; local random secrets → `secrets.rs` store (Phase C)                                         |
| `consensus/verify/engine.ts`, `service/checks.ts`, `verify/*/hashing.ts` | Packet verification, Safe-transaction policy checks, EIP-712 message hashing.                                                                 | `checks.rs` + transition handlers (Phase D2); hashing via `alloy` `SolStruct::eip712_signing_hash`                                                 |
| `consensus/protocol/{base,onchain,transaction}.ts`                       | Action queue + action→calldata encoding + tx submission/nonce/fee management.                                                                 | `ActionEncoder::encode_action` (Phase E1); the queue/fees/resubmit are `safenet-core::tx::TransactionQueue`                                        |
| `consensus/protocol/sqlite.ts`                                           | Persistent action queue + tx nonce/fee store.                                                                                                 | subsumed by the `Driver` + `TransactionQueue` (no separate action queue)                                                                           |
| `frost/*`, `consensus/merkle.ts`, `utils/participants.ts`                | FROST math, hashing, VSS, ECDH, nonces, signing, merkle trees.                                                                                | `frost/*` + `merkle.rs` (Phase B)                                                                                                                  |
| `shared/watcher.ts`, `watcher/*`                                         | Block + event indexing, reorg detection.                                                                                                      | `safenet-core::index::Watcher` (**done**)                                                                                                          |
| `utils/logging.ts`, `utils/metrics.ts`                                   | Logging + Prometheus.                                                                                                                         | `safenet-core::observability` (**done**)                                                                                                           |
| `types/schemas.ts` (`zod`)                                               | Env validation.                                                                                                                               | `config.rs` (`serde` + TOML), already scaffolded                                                                                                   |

Shared infrastructure the validator consumes from `safenet-core` (all merged): `index` (watcher),
`state` (`StateMachine` + pure `StateTransition` + the effect system, #512), `tx` (`TransactionQueue`
+ `Signer`), `observability`, `driver` (`Driver` + the component-bundle `Service` + `ActionEncoder`),
and the shared `SqlitePool`.

---

## Architecture Decision

The validator is the `async` (`tokio`) **binary** it already is; the port fills in the real
`Service` components and the FROST library they need. No `safenet-core` changes are required.

| Concern                              | TypeScript today                                                      | Rust choice                                                 | Notes                                                                                                                                    |
| ------------------------------------ | --------------------------------------------------------------------- | ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| RPC / primitives / signing / EIP-712 | `viem`                                                                | **`alloy`**                                                 | `Provider`, `Address`/`B256`/`U256`, typed events & EIP-712 via `sol!`; already used by the crate.                                       |
| Service orchestration                | `SafenetStateMachine` + `ValidatorService`                            | **`safenet-core::driver::{Driver, Service}` components**    | The validator implements `Service::components()` — a pure `Transition`, an `Effects` handler and an `Actions` encoder; the `Driver` runs indexing + state + submission. The TS transition queue is the core `StateMachine`. |
| Indexing                             | `shared/watcher.ts`                                                   | **`safenet-core::index`**                                   | `Watcher<P, E>` over the `sol!` event set; reuse, don't re-port.                                                                         |
| Service state + reorg rollback       | `SqliteClientStorage` + `SqliteStateStorage` (no rollback)            | **`safenet-core::state`**                                   | `StateMachine` drives a pure `StateTransition`; `SnapshotStore` rolls back per-block on reorg (an improvement over the TS store).        |
| Crypto state (groups, shares, keys)  | separate `KeyGenClient`/`SigningClient` over SQLite                   | **embedded in the snapshot `State`**                        | **Deviation (user-requested):** deterministic DKG material lives in the state machine, not in separate clients. See below.              |
| Local random secrets (DKG, nonces)   | rows in the shared SQLite store                                       | **separate reorg-immune SQLite store behind the `EffectHandler`; nonces burned on use, never rolled back** | **Deviation (user-requested):** transitions stay pure; all secret-store access goes through effects. See below.                         |
| Transaction submission + queueing    | `TransactionManager` + `SqliteQueue`                                  | **`safenet-core::tx::TransactionQueue`**                    | Durable queue: nonce mgmt, fee bump/resubmit, in-flight cap, per-tx block expiry. Subsumes the TS tx manager and action queue.           |
| Startup onchain reconciliation       | `service.start()` → `getValidatorStaker` → maybe `setValidatorStaker` | **a `GetValidatorStaker` effect + the `ValidatorStakerSet` event** | The effect system (#512) covers startup reads; the original plan's core `Driver` hook is superseded (see Alternatives).                  |
| FROST library                        | hand-rolled on `@noble/curves`                                        | **ZCash `frost-secp256k1` / `frost-core`** (see decision below) | The scheme is standard RFC 9591 FROST(secp256k1, SHA-256); the crate is byte-compatible by construction. Only Safenet wrappers are ours. |
| SQLite                               | `better-sqlite3` (sync)                                               | **`sqlx`** (async), one shared `SqlitePool`                 | Snapshot store, tx queue and secret store share one pool, matching the single-`Database` TS pattern.                                     |
| Config validation                    | `zod`                                                                 | **`serde` + `toml` + `argh`**                               | Already scaffolded in `config.rs`/`main.rs`.                                                                                             |
| Errors                               | `viem` `BaseError`                                                    | **`thiserror`**                                             | Per-module error enums; `main` returns `Box<dyn Error>`. Transitions and effect handlers are **infallible** — effect errors are encoded in the `Resume` value. |

Key decisions:

- **Implement the `Service` component bundle; the `Driver` runs everything.** Following the sentinel
  precedent (`SentinelService`/`SentinelTransition`/`SentinelEncoder`), the validator supplies a
  `ValidatorService` implementing `Service::components()`, yielding three parts:

  - `ValidatorTransition` — the **pure** `StateTransition<State>`: `apply_transition(&self, state,
    message)` over `Message::{NewBlock, Event, Resume}`, returning the new state plus
    `Command::{Action, Effect}`s. It holds only the machine config (account, staker, participants +
    info, `genesis_salt`, `blocks_per_epoch`, timeouts, `allowed_oracles`).
  - `ValidatorEffects` — the `EffectHandler`: owns the **secret store** handle, the **RNG**, and a
    **provider** clone (for the staker read). All impurity lives here.
  - `ValidatorEncoder` — the `ActionEncoder`: holds the contract addresses and gas constants, and
    maps each `Action` to a `(Transaction, expires_at)`.

  The TS `SafenetStateMachine`'s transition queue, block/event ordering, reorg handling and effect
  resumption are all the core `StateMachine`'s job. There is **no** validator-side service loop or
  action queue.

- **The `Service::Event` is the `sol!`-generated typed event set.** As in the sentinel, the state
  machine's `Event` _is_ the watcher's event type. `service/mod.rs` declares it with `watcher_events!`
  over the `bindings` `Consensus::ConsensusEvents` and `Coordinator::CoordinatorEvents` enums (plus the
  oracle result event), and the transition matches on it directly. This replaces
  `machine/transitions/types.ts` and the `zod` decoding in the TS watcher. The Rust event set
  additionally includes `Consensus::ValidatorStakerSet` (the TS validator does not watch it) to keep
  the staker in `State` — allowed, since only onchain/peer compatibility is required.

- **Deterministic crypto state is embedded in the snapshot `State`, not in separate clients (user
  deviation #3).** The TS `KeyGenClient` and `SigningClient` are thin wrappers over a plain SQLite
  store holding group info, commitments, secret shares, verification/signing shares, and the group
  public key. In Rust the **deterministic** subset — everything derivable by replaying onchain events
  (group participants/threshold, peers' published commitments and encrypted shares, and the
  verification share, signing share and group public key derived from them) — becomes fields of the
  `Serialize + Deserialize` snapshot `State`. This is correct under reorgs: a reorg rolls it back and
  the replayed events (with their replayed effects — see the next decision) recompute it identically.
  The FROST math functions (Phase B) are **pure, stateless** functions; the client classes disappear.
  The **locally-generated random secrets are the exception** — they must **not** live in the snapshot.

- **Locally-generated random secrets live in a separate, reorg-immune SQLite store, reached only
  through the effect system (user deviations #3 and #4; core #512).** Two kinds of secret are sampled
  locally and then committed to onchain, and **neither can be in the reorg-aware snapshot**:

  - **DKG polynomial secrets** — the random `coefficients` and the ECDH `encryption secret key` a
    participant generates in keygen setup. Their commitments / encryption public key are published in
    the `keyGenAndCommit` / `keyGenCommit` transaction. The failure the user identified: if these were
    in the snapshot, a reorg could roll them back **while the commitment transaction is re-included on
    the reorged chain** — leaving the validator unable to produce the matching secret shares or decrypt
    peers' shares, permanently stuck in that DKG. So the secrets must survive the reorg.
  - **FROST signing nonces** — the random hiding/binding scalars, their public commitments and the
    merkle tree. A burned nonce must **never** be un-burned: reusing a nonce across two signature
    shares leaks the signing share. Same requirement — survive the reorg (and stay burned).

  Both live in a dedicated **secret store**: tables in the _same_ `SqlitePool` that the snapshot store
  does **not** roll back. The transitions themselves stay **pure** — every secret-store interaction is
  a `Command::Effect` performed by `ValidatorEffects`, whose result re-enters the machine as a
  `Message::Resume`. Core #512 documents exactly this contract: effects **may be performed more than
  once for the same chain message** (crash or reorg replay), handlers must encode outcomes like
  "already used" in the `Resume` value, and **resume ordering is unspecified**, so every `Resume`
  variant carries its correlating ids (group id, signature id). Concretely:

  - **keygen setup** emits a `DkgCommit` effect; the handler looks up the group's coefficients /
    encryption key in the store and **reuses** any existing entry (a pre-reorg generation whose
    commitment may have landed) rather than sampling fresh — so a reorged-and-re-included commitment
    stays consistent with the retained secret. Only when absent does it sample and persist.
  - **signature-share creation** emits a `UseNonce` effect; the handler atomically fetches and burns
    the nonce. A replay that finds the nonce already burned resumes with `AlreadyBurned`, and the
    transition emits **no** signature-share action (graceful no-op — the TS code throws here and the
    caller swallows it; the Rust port makes the recovery explicit and pure).

  **Pruning is required so the store does not grow unbounded**, and is likewise effect-driven: DKG
  secrets are pruned when the group's keygen resolves — successfully (`KeyGenConfirmed` / signing
  share finalized) **or** unsuccessfully (keygen timeout / complaint abort); nonces are burned on use
  and their tree pruned when the owning group is retired (its epoch rolls out of the reorg window).

- **FROST uses the ZCash Foundation `frost-secp256k1` / `frost-core` crates — not a hand-rolled
  implementation.** Safenet's scheme is **standard RFC 9591 FROST(secp256k1, SHA-256)**: the TS
  domain tags (`"rho"`, `"chal"`, `"nonce"`, `"msg"`, `"com"`, `"dkg"`) and context string
  `"FROST-secp256k1-SHA256-v1"` are exactly that ciphersuite, so the ZCash crate is **byte-compatible
  with the contracts and the TS validator by construction** — hand-rolling the field/curve/hash
  primitives would only risk drifting from it. The port therefore builds on `frost-secp256k1 = "3"`
  and `frost-core = { version = "3", features = ["internals"] }`, using their `keys::dkg::part{1,2,3}`
  for DKG and `round1::commit` / `round2::sign` / `aggregate` for signing. The spike
  (`project/ports:validator-rust/src/frost/{keygen,marshal,secret,participants}.rs`) is the **working
  sample** of exactly how to drive this interface the way Safenet needs — copy its structure. Only the
  Safenet-specific layers *around* the ciphersuite are our code:
  - **address-derived participant identifiers** (`Identifier` from `hid(address)`, not FROST's default
    sequential ids),
  - **ECDH-XOR encryption** of the secret shares for onchain publishing (`secret.rs`),
  - the **keccak256 merkle trees** — participants root / POAP, signer set, and nonce tree (`merkle.rs`),
  - the **1024-nonce preprocessing scheme** that batches standard FROST `SigningNonces` under an
    onchain merkle commitment, plus burn (`nonces.rs` + the secret store),
  - **solidity marshalling** between FROST types and the ABI `Point`/`U256`/signature/commitment
    tuples (`marshal.rs`).

  Because the ciphersuite itself is the standard crate, parity effort concentrates on **these wrapping
  layers**: parity vectors (captured from the TS / a contract `eth_call`) cover identifier derivation,
  ECDH, the merkle roots/proofs, the nonce-tree commitment, and the marshalled ABI encodings, plus one
  end-to-end DKG-and-sign round asserting the crate interoperates with a TS-produced group. All FROST
  wrappers are pure functions; **sampling happens only in `ValidatorEffects`** (which owns the RNG).

- **EIP-712 message hashing via `sol!` + `SolStruct::eip712_signing_hash`.** The messages the group
  signs (Safe-tx hash, epoch-rollover packet, oracle-tx proposal) are hashed the same onchain-identical
  way the sentinel port established. Reuse the sentinel crate's `SafeTx`/proposal `sol!` structs and
  hashing where they overlap (candidate for a small shared module); the source of truth for typehashes
  is `contracts/src/libraries/ConsensusMessages.sol`.

- **Actions encode to transactions; the queue owns fees.** `ActionEncoder::encode_action` maps each
  validator action to `(tx::Transaction, expires_at)` — calldata from the `sol!`-generated
  `Coordinator`/`Consensus` call, per-action gas (ported from `consensus/protocol/onchain.ts`), and
  `expires_at` derived from the driving FSM deadline. Because the encoder is stateless, **each
  `Action` variant carries its deadline** — the exact shape of the TS `ActionWithTimeout`. Fees
  (estimate, cap, replacement bump) are entirely the `TransactionQueue`'s job; the validator does not
  compute fees.

- **Startup staker reconciliation is an effect, not a core hook.** The TS `service.start()` reads
  `getValidatorStaker(account)` and submits `setValidatorStaker` on mismatch before starting the
  watcher. In Rust, `State` tracks the onchain staker: on a `NewBlock` while it is unknown, the
  transition emits a `GetValidatorStaker` effect (the handler holds a provider; an RPC failure
  resumes with `None` and is retried on the next block). If the resumed staker differs from the
  configured one, the transition emits a `SetValidatorStaker` action and marks the request pending in
  `State` so it is not re-emitted every block; the `ValidatorStakerSet` event confirms it (and keeps
  `State` correct thereafter). This is a faithful port of `#setStakerAddress` — idempotent, safe on
  every startup — with no `safenet-core` change.

### Alternatives Considered

- **A core `Driver`/`Service` initialization hook (the original Phase A of this plan).** The first
  draft added `Service::initialize(provider) -> Vec<Action>` to `safenet-core` for the staker
  bootstrap. Superseded by core #512: the reshaped `Service` is consumed by `components()` (there is
  no service object left to hook), and the effect system is now the sanctioned home for impure reads
  feeding the pure transition. A `GetValidatorStaker` effect plus the `ValidatorStakerSet` event does
  the job with no core change — and keeps the staker consistent in `State` afterwards, which the
  one-shot hook did not.
- **An impure `StateTransition` accessing the secret store directly (`&mut self`, `async`).** The
  first draft's design, matching core's then-current trait. Superseded by core #512, which made
  transitions pure and introduced the effect system **explicitly motivated by the validator's nonce
  and DKG-coefficient impurity**; the port follows it.
- **Keep the `KeyGenClient`/`SigningClient` split with a plain (non-reorg) crypto store.** Rejected per
  the user's deviation #3: embedding the deterministic crypto state in the reorg-aware snapshot is
  strictly more correct and removes a parallel storage layer. (Random secrets are the deliberate
  exception — see below.)
- **Put the random secrets (DKG coefficients / encryption key / nonces) in the snapshot too.**
  Rejected per deviations #3 and #4: a reorg could roll back a locally-generated secret **while the
  transaction that committed to it is re-included on the reorged chain** — for DKG coefficients / the
  encryption key this strands the validator mid-keygen (it can no longer produce the matching shares
  or decrypt peers' shares); for nonces it would permit reuse and leak the signing share. These
  secrets must be reorg-immune, which is exactly what the effect-fronted secret store provides.
- **Hand-roll the FROST primitives on `k256` (the TS `@noble/curves` approach).** Rejected: Safenet's
  scheme is standard RFC 9591 FROST(secp256k1, SHA-256), which the ZCash Foundation
  `frost-secp256k1`/`frost-core` crates implement exactly and byte-compatibly; re-implementing the
  field/curve/hash math would only add a large surface to keep in lock-step with the contracts for no
  benefit. Only the Safenet-specific wrappers (identifiers, ECDH, merkle trees, nonce-tree
  preprocessing, marshalling) are ours.
- **Do staker reconciliation in `main.rs` before `Driver::run`.** Rejected: it belongs _on the
  service_ (the user's framing — "initialization should be defined on the service"), needs the same
  action encoding as everything else, and would race the queue's block-driven submission.
- **A separate transition/event type decoded from raw logs (mirroring `machine/transitions`).**
  Rejected — the `Driver` ties the state machine's `Event` to the watcher's typed event set, so the
  `sol!` enum is used directly.
- **Hand-roll a transition queue / action queue.** Rejected — the core `StateMachine` and
  `TransactionQueue` already provide ordering, persistence, retry and expiry.

---

## Tech Specs

### Crate layout

```
crates/validator/
  Cargo.toml                 # add crate-local frost-secp256k1/frost-core/k256/rand (NOT workspace deps)
  src/
    main.rs                  # exists — Phase E2 swaps DummyService for the real service
    config.rs                # exists — Phase A2 adds validator fields
    bindings.rs              # Phase A1: sol! Consensus + Coordinator contracts (events + calls)
    frost/                   # thin Safenet-specific layer over frost-secp256k1 / frost-core
      mod.rs                 # re-exports
      keygen.rs              # DKG rounds via frost keys::dkg::part{1,2,3} (port of the spike)
      signing.rs             # signing via frost round1::commit / round2::sign / aggregate
      secret.rs              # ECDH-XOR share encryption/decryption (port of the spike)
      participants.rs        # address-derived Identifier + sorting (port of the spike)
      nonces.rs              # 1024-nonce preprocessing: batch frost SigningNonces, merkle, sequence
      marshal.rs             # FROST <-> solidity Point{x,y}/U256/signature/commitment (port of the spike)
    merkle.rs                # keccak256 merkle: participants root/proof, signer set, nonce-tree proof
    secrets.rs               # Phase C1: separate reorg-immune SQLite store for local random secrets
                             #   (DKG coefficients + encryption key; nonce trees + burn) with pruning
    state/                   # Phase C2: the snapshot State and its pure Transition, grown through Phase D
      mod.rs                 #   State (starts as just RolloverState::WaitingForGenesis) + the pure
                             #   Transition (no-op skeleton in C2; rollover/signing/consensus/DKG in D)
    checks.rs                # Phase D2i: Safe-transaction policy checks (delegatecall/multisend/config)
    service/                 # Phase C2: the ValidatorService bundle + the watched event set
      mod.rs                 #   ValidatorService + the watcher_events! Event set (Consensus/Coordinator/Oracle)
      action.rs              #   the (empty in C2) Action enum + its Encoder, one arm per Phase D action
      effect.rs              #   Effect + Resume enums and their Handler (empty in C2; grows the
                             #   secret store + provider + RNG through D1/D3/D5)
    hashing.rs               # EIP-712 message hashing (SafeTx / rollover / oracle proposal); reuse sentinel's where possible
```

Modules are introduced only when first used (no empty stubs), matching the crate convention. If the
`state` transition grows past the size budget, its handlers split into `keygen.rs` / `signing.rs` /
`rollover.rs` submodules (the phase breakdown already anticipates this).

### Cargo manifest

Mirrors the existing `crates/validator/Cargo.toml`: a bare package name and workspace-inherited
dependencies for the crates shared across the workspace. **The FROST crates are the exception — they
are validator-only, declared directly in `crates/validator/Cargo.toml`, not in the root
`[workspace.dependencies]`**, because no other crate uses them (the sentinel and core have no FROST
dependency). Beyond what the crate already pulls in via `.workspace = true` (`alloy`, `safenet-core`,
`serde`, `sqlx`, `thiserror`, `tokio`, `toml`, `tracing`, `url`), the FROST work adds these
**crate-local** dependencies with their versions pinned in the crate manifest:

- **`frost-secp256k1 = "3"`** — the RFC 9591 FROST(secp256k1, SHA-256) ciphersuite (DKG + signing +
  aggregation), from the ZCash Foundation.
- **`frost-core = { version = "3", features = ["internals"] }`** — the generic FROST types the
  wrappers touch (identifiers, packages, shares, signatures); the `internals` feature exposes the
  pieces the spike uses.
- **`k256 = { version = "0.13", features = ["serde"] }`** — secp256k1 point/scalar encoding for the
  ECDH layer and solidity marshalling (matching the spike).
- **`rand = "0.8"`** — RNG for DKG / nonce / encryption-key sampling (matching FROST v3's
  `rand_core`); used only by `ValidatorEffects`, since transitions are pure.

These four are pinned in the crate rather than inherited; the general "new shared crates go to
`[workspace.dependencies]` first" rule still applies to any dependency that is genuinely shared. No
hand-rolled hash/curve crate (`sha2` etc.) is needed — the ciphersuite is the `frost-*` crates.

### Onchain bindings (`bindings.rs`)

- `sol!` blocks for the two contracts, transcribed from `validator/src/types/abis.ts` (events +
  functions) and `contracts/src/`:
  - **`Consensus`** — events `EpochStaged`, `EpochProposed`, `TransactionProposed`,
    `TransactionAttested`, `OracleTransactionProposed`, `OracleTransactionAttested`,
    `ValidatorStakerSet`; calls `proposeEpoch`, `stageEpoch`, `attestTransaction`,
    `attestOracleTransaction`, `setValidatorStaker`, `getValidatorStaker`.
  - **`Coordinator`** — events `KeyGen`, `KeyGenCommitted`, `KeyGenSecretShared`, `KeyGenConfirmed`,
    `KeyGenComplained`, `KeyGenComplaintResponded`, `Preprocess`, `Sign`, `SignRevealedNonces`,
    `SignShared`, `SignCompleted`; calls `keyGenAndCommit`, `keyGenCommit`, `keyGenSecretShare`,
    `keyGenComplain`, `keyGenComplaintResponse`, `keyGenConfirm[WithCallback]`, `preprocess`, `sign`,
    `signDecline`, `signRevealNonces`, `signShare[WithCallback]`.
  - The oracle `OracleResult` event (for `handleOracleResult`).
  - Shared `Point{uint256 x; uint256 y}`, `Attestation{Point r; uint256 z}` and the `SafeTransaction`
    tuple, modelled on `project/ports:validator-rust/src/bindings.rs`.
- The watcher event set is declared (in `service/mod.rs`, alongside `ValidatorService`) with
  `watcher_events!` over these generated `*Events` enums (plus a variant for the oracle event).
  **`alloy` types are an area LLMs get wrong** — expect to hand-hold this PR against the compiler; the
  spike's `bindings.rs` is the working reference to copy the `sol!` layout and the `.into_inner()`
  decode bridge from.

### FROST cryptography (`frost/*`, `merkle.rs`) — a thin layer over the ZCash crates

The FROST ciphersuite is **`frost-secp256k1` (RFC 9591 FROST(secp256k1, SHA-256))** — the DKG rounds,
nonce/commitment generation, signature shares and aggregation come from the crate and need no porting.
Our code is only the Safenet-specific wrapping, modelled on the spike
(`project/ports:validator-rust/src/frost/*`), and is **pure** — callers (the effect handler) supply
the RNG and secrets:

- **DKG** (`keygen.rs`): drive `keys::dkg::part1/2/3`, keyed by address-derived identifiers, with
  ECDH-encrypted secret shares — a direct port of the spike's `generate_round{1,2,3}`
  (`round1_packages` / `round2_encrypted_shares` helpers). The **random secrets** these produce
  (`round1/round2::SecretPackage`, encryption key) go to the reorg-immune secret store (Phase C1), not
  the snapshot.
- **Signing** (`signing.rs`): the spike did not port this; use the crate's `round1::commit`
  (SigningNonces + commitments), `round2::sign` (signature share) and `aggregate`, keyed by the same
  identifiers and Lagrange logic the crate provides.
- **ECDH share encryption** (`secret.rs`): the `EncryptionKey` / `ecdh` XOR scheme that encrypts FROST
  signing shares for onchain publishing — port of the spike's `secret.rs`.
- **Identifiers** (`participants.rs`): `Identifier` derived from `hid(address)` and canonical sorting —
  port of the spike's `participants.rs`.
- **Merkle** (`merkle.rs`): `calculateParticipantsRoot`, `generateParticipantProof` (POAP),
  `generateMerkleProofWithRoot` (signer set), nonce-tree leaf hashing + proof — keccak256, canonical
  pairing, byte-identical to `consensus/merkle.ts`. (Not provided by FROST.)
- **Nonce preprocessing** (`nonces.rs`): batch 1024 crate `SigningNonces` into a chunk, commit them
  under a keccak merkle root (the onchain `preprocess` commitment), and map a `sequence` to
  `(chunk, offset)`. This is a Safenet extension around standard FROST nonces; the secrets + burn live
  in the secret store (Phase C1).
- **Marshalling** (`marshal.rs`): FROST point/scalar ↔ solidity `Point{x,y}`/`U256` (uncompressed),
  signature ↔ `(Point r, uint256 z)`, commitment tuple `(q, c[], r, mu)` — port of the spike's
  `marshal.rs`.

**Parity vectors target the wrapping layers, not the ciphersuite** (which is byte-compatible by
construction). Vectors captured from the TS / a contract `eth_call` cover identifier derivation, ECDH,
the merkle roots/proofs, the nonce-tree commitment and the marshalled ABI encodings, plus one
end-to-end test asserting a Rust participant interoperates with a TS-produced group through a full
DKG-and-sign round. A tiny TS dump script (alongside `cmd/derive-genesis.ts`) or committed JSON
fixtures produce the vectors.

### State & secret store (`state/`, `service/action.rs`, `secrets.rs`)

The `state/` and `service/action.rs` shapes below are the **eventual** structure the state machine
converges on, reached incrementally through Phase D (C2 only lands the `WaitingForGenesis`
skeleton). They are documented here as the target, not as an up-front C2 deliverable — the FSM is
grown against the transitions that use it.

- `state/` — the snapshot `State` (`Serialize + Deserialize + Default`) alongside its `Transition`,
  composing:
  - **consensus**: `active_epoch`, `genesis_group_id?`, `epoch_groups` (epoch→group id),
    `signature_id_to_message`, `group_pending_nonces` (port of `ConsensusState`), plus the known
    onchain `staker` and the pending-staker-request marker (Phase D5).
  - **rollover**: the `RolloverState` enum — `WaitingForGenesis`, `SkipGenesis`, `EpochSkipped`,
    `CollectingCommitments`, `CollectingShares`, `CollectingConfirmations`, `SignRollover`,
    `EpochStaged` (port of `machine/types.ts` `RolloverState`, carrying deadlines / complaints /
    per-participant progress).
  - **signing**: a map message→`SigningState` — `WaitingForRequest`, `CollectNonceCommitments`,
    `CollectSigningShares`, `WaitingForAttestation`, `WaitingForOracle`, `WaitingToDecline` (port of
    `SigningState`, carrying the packet, signers, deadlines).
  - **groups** (the deviation): the **deterministic** per-group DKG material formerly in
    `KeyGenClient`/`SigningClient` storage — participants, threshold, peers' received commitments,
    received (encrypted) secret shares, verification shares, signing share, group public key. This is
    all recomputable from replayed events (plus replayed effects resuming the retained secrets), so it
    is reorg-safe in the snapshot. The **locally-generated random secrets are excluded** — the group's
    `coefficients` and `encryption secret key`, and all nonces, live in the reorg-immune secret store
    below.
- `service/action.rs` — the **`Action`** enum, porting `consensus/protocol/types.ts` `ProtocolAction`
  (13 variants across keygen / signing / consensus, including `SetValidatorStaker`). Each variant
  carries the deadline the encoder derives `expires_at` from (the TS `ActionWithTimeout` shape).
- `secrets.rs` — a `sqlx` store over the shared pool holding the locally-generated random secrets,
  **not** part of the snapshot and **not** rolled back on reorg. It is reached **only** from
  `ValidatorEffects::perform_effect`:
  - **DKG secrets** table: per `(group_id, address)`, the `coefficients` and `encryption_secret_key`.
    Written by the keygen-setup effect (before the commitment action is emitted); read on replay so a
    reorged-and-re-included commitment reuses the same secrets. Pruned when the group's keygen resolves
    (`prune_dkg_secrets(group_id)`).
  - **Nonces** tables (ported from `consensus/storage/sqlite.ts`): `nonce_links(root PK, group_id,
    address, chunk)` and `nonces(leaf PK, root FK, offset, hiding, hiding_commitment, binding,
    binding_commitment)`.
  - methods: `dkg_secrets(group, me)` / `store_dkg_secrets(group, me, coefficients, encryption_key)` /
    `prune_dkg_secrets(group)`; `register_nonce_tree`, `link_nonce_tree(chunk→root)`,
    `nonce_tree(group, me, chunk)`, `use_nonce(group, me, chunk, offset)` (**atomically** fetches the
    secret scalars and NULLs them in one statement, returning `None` when already burned),
    `available_nonce_count`, `prune_group_nonces(group)`.
  - `store_dkg_secrets` is a no-op when secrets for the key already exist (reuse-not-overwrite);
    `use_nonce` is idempotent and permanent. Both reads survive a snapshot rollback. Tests cover:
    DKG-secret reuse across a simulated reorg, that pruning removes resolved-group secrets, and that a
    second `use_nonce` for the same leaf yields `None` (→ the transition's graceful no-op).

### Effects (`service/effect.rs`)

The `Effect` and `Resume` enums plus the `Handler` (the `EffectHandler`), which owns the secret
store, the RNG and a provider clone. Effect payloads carry the **public** inputs the handler needs
(drawn from `State` by the transition); the handler contributes the stored secrets and randomness;
derived results flow back into the snapshot `State` via the resume transition, so replays reproduce
identical state. Handlers never fail — errors are data in `Resume`. The catalogue:

| Effect                                          | Handler behavior                                                                    | Resume (carries its ids)                              |
| ----------------------------------------------- | ----------------------------------------------------------------------------------- | ----------------------------------------------------- |
| `GetValidatorStaker`                            | `eth_call` `getValidatorStaker(account)`                                             | `ValidatorStaker(Option<Address>)` (`None` = RPC error, retried next block) |
| `DkgCommit { group_id, .. }`                    | reuse-or-sample coefficients + encryption key; persist; run `dkg::part1`             | round1 package + encryption public key                |
| `DkgShares { group_id, round1_packages }`       | load secrets; run `dkg::part2`; ECDH-encrypt the shares                              | encrypted shares for publishing                       |
| `DkgFinalize { group_id, encrypted_shares, .. }`| load encryption secret; decrypt peers' shares; run `dkg::part3`                      | signing share + verification shares + group key       |
| `PruneDkgSecrets { group_id }`                  | delete the group's DKG secrets                                                       | acknowledgment (no commands)                          |
| `NonceTree { group_id, chunk }`                 | reuse-or-sample 1024 `SigningNonces`; persist                                        | nonce commitments + merkle root (for `preprocess`)    |
| `UseNonce { group_id, signature_id, sequence }` | atomic fetch-and-burn via `use_nonce`                                                | the `SigningNonces`, or `AlreadyBurned`               |
| `PruneGroupNonces { group_id }`                 | delete the retired group's nonce trees                                               | acknowledgment (no commands)                          |

The exact effect granularity (one per DKG round, as listed) is confirmed as the D1 DKG slices land
(D1iii–D1v) — see Open Questions.

### State transitions (`state/` + submodules)

The state `Transition` holds the machine config (account, staker, participants + info, `genesis_salt`,
`blocks_per_epoch`, `key_gen_timeout`, `signing_timeout`, `allowed_oracles`, `oracle_timeout`) and
implements the pure `StateTransition<State>`, matching on the `Message`:

- `Message::NewBlock(n)` ports `progressToBlock`: epoch-rollover check (→ trigger next keygen /
  `StageEpoch`), keygen-timeout check, signing-timeout check, and the staker check (emit
  `GetValidatorStaker` while the onchain staker is unknown). Timeout/abort transitions that retire a
  keygen also emit `PruneDkgSecrets`.
- `Message::Event(log)` matches the typed event set and ports the per-event handlers (genesis + keygen
  committed/shared/confirmed/complaints; preprocess/sign/reveal/share/completed/decline;
  epoch proposed/staged; transaction & oracle proposed/attested; oracle result; validator staker set).
  Transaction handlers run the Safe-tx `checks.rs` and EIP-712 hashing to decide attest-vs-decline and
  produce the message to sign. Handlers that need secrets emit the corresponding effect instead of
  computing inline: keygen setup emits `DkgCommit`, share distribution `DkgShares`, confirmation
  `DkgFinalize` (and `PruneDkgSecrets` once resolved); signing emits `NonceTree` for preprocessing and
  `UseNonce` for signature-share creation.
- `Message::Resume(result)` completes the effectful flows: writes the DKG round outputs into `State`
  and emits the matching onchain action (`keyGenAndCommit`, `keyGenSecretShare`, `keyGenConfirm`,
  `preprocess`, `signShare`, ...); handles `ValidatorStaker` (emit `SetValidatorStaker` on mismatch);
  and on `AlreadyBurned` emits **nothing** (the graceful no-op). Since resume ordering is unspecified,
  each resume is matched to its pending FSM entry by the ids it carries.

### Action encoding (`ValidatorEncoder`)

Implements `ActionEncoder<Action>`, holding the `consensus`/`coordinator` addresses and gas constants.
`encode_action` maps each `Action` to `(tx::Transaction, expires_at)`: calldata from the `sol!` call
(e.g. `Coordinator::keyGenAndCommitCall{..}.abi_encode()`), `value = 0`, `gas` per the
`consensus/protocol/onchain.ts` estimates (e.g. keyGenAndCommit ≈ 250k, keyGenSecretShare ≈
250k + 25k·shares, sign/attest/stage ≈ 400k, decline ≈ 80k), and `expires_at` from the deadline
carried on the action variant. Fees are the `TransactionQueue`'s responsibility.

### Config (`config.rs`)

Extend the existing `Config` with the validator fields (consensus + coordinator addresses, staker,
participants + participant info, `genesis_salt`, `blocks_per_epoch`, `key_gen_timeout`,
`signing_timeout`, `allowed_oracles`, `oracle_timeout`, `skip_genesis`), keeping `#[serde(default)]`,
the nested `observability` and the flattened `driver` sub-configs. `main.rs` follows the events
emitted by `[consensus, coordinator, ...allowed_oracles]`.

### Testing

- Unit tests mirror the TS `*.test.ts` intent (behavior, not implementation): FROST parity vectors
  (per Phase B PR), the secret store (DKG-secret reuse-across-reorg, pruning, `use_nonce`
  idempotency), the transitions, `checks.rs`, and `encode_action` (calldata + gas + expiry).
- **Transition tests are pure and synchronous** — no async, no database, no mocks: feed a `Message`
  sequence (including hand-crafted `Resume` values such as `AlreadyBurned`) and assert on the returned
  `(State, Commands)`. This covers the rollover/keygen/signing FSMs, timeouts, and the
  **burned-nonce graceful no-op**.
- `ValidatorEffects` + `sqlx` tests run against `sqlite::memory:`, covering the reuse-not-overwrite
  and burn semantics under effect replay (the same effect performed twice must resume consistently).
- Reorg rollback and tx resubmission are covered by core; a validator-level `StateMachine` test
  asserts a reorged keygen event rolls back the derived DKG material in the snapshot **while the DKG
  secrets and burned nonces in the secret store survive**, and that replaying the events (with their
  replayed effects) reconverges on identical state.
- The `consensus/integration.test.ts` end-to-end flow (genesis DKG → signing) is the model for the
  Phase F interop test.

### Tooling

Per `AGENTS.md`: `cargo fmt --all`, `cargo clippy -w validator`, `cargo test -w validator`;
`Cargo.lock` committed. Features are workspace-inherited.

---

## Implementation Phases

Each PR has a single purpose, targets < 300 changed LOC and < 10 files, and is independently
reviewable. "Depends on" lists hard ordering; everything else may proceed in parallel. FROST (Phase B)
is the long pole and is intentionally sliced small. There are no `safenet-core` changes, so no
cross-crate ordering.

### Phase A — Bindings & config (validator; unblocks B2/C/D/E)

- **A1 — Onchain bindings.** `bindings.rs`: `sol!` for `Consensus` + `Coordinator` (events + calls,
  including `ValidatorStakerSet` / `getValidatorStaker` / `getCoordinator`) and the oracle result
  event, and the shared `Point`/`Attestation`/`SafeTransaction` structs. Modeled on the spike's
  `bindings.rs`. _No behavior; the typed surface._ (The `watcher_events!` event set over these enums
  lives with the service in `service/mod.rs`, landed in C2.)
- **A2 — Config fields.** Extend `config.rs` with the validator fields + tests (parses required fields;
  nested observability / flattened driver still default). Depends on nothing else; parallel with A1.

### Phase B — FROST layer over `frost-secp256k1` (depends on A1 for the marshalling/DKG ABI types)

A thin wrapper over the ZCash crates; each PR ships parity vectors for its Safenet-specific layer.
B1/B2 are the foundation; the rest fan out. All wrappers are pure — RNG is passed in by callers.

- **B1 — Identifiers, ECDH & merkle.** `frost/{participants,secret}.rs` + `merkle.rs`: address-derived
  `Identifier`, canonical sorting, the ECDH-XOR `EncryptionKey`, and the keccak merkle utilities
  (participants root/POAP, signer-set proof, nonce-tree leaf hashing/proof). Parity vectors vs the TS
  `identifier.ts`/`secret.ts`/`merkle.ts`. Depends only on the `frost-core` types + keccak.
- **B2 — Solidity marshalling.** `frost/marshal.rs`: FROST point/scalar/signature/commitment ↔ the ABI
  `Point`/`U256`/`(Point,U256)`/`(q,c[],r,mu)` tuples — port of the spike's `marshal.rs`. Depends on A1
  (the `sol!` `Point`/commitment structs) and B1. Parity vectors against contract-shaped encodings.
- **B3 — DKG.** `frost/keygen.rs`: `keys::dkg::part{1,2,3}` driven with address identifiers and
  ECDH-encrypted shares — port of the spike's `generate_round{1,2,3}`. Depends on B1, B2. Vectors: a
  full small-group DKG round, and interop with a TS-produced commitment/share set.
- **B4 — Nonce preprocessing.** `frost/nonces.rs`: batch 1024 crate `SigningNonces` into a chunk under
  a keccak merkle root, `sequence`→`(chunk, offset)`. Depends on B1. Parity vectors vs `signing/nonces.ts`.
- **B5 — Signing & aggregation.** `frost/signing.rs`: `round1::commit` / `round2::sign` / `aggregate`
  keyed by the Safenet identifiers + signer-set merkle. Depends on B1, B2, B4. Vectors + a full
  sign→aggregate→verify round interoperating with TS-produced shares.

### Phase C — State & secret store (depends on B; parallel with late B)

- **C1 — Secret store.** `secrets.rs`: the separate reorg-immune `sqlx` store for the
  locally-generated random secrets — the DKG-secrets table (`dkg_secrets` / `store_dkg_secrets` /
  `prune_dkg_secrets`) and the nonce tables (`register` / `link` / `nonce_tree` / atomic `use_nonce` /
  `available_count` / `prune_group_nonces`) — with tests for DKG-secret reuse (no overwrite), pruning,
  and burn idempotency. Depends on B1 (`EncryptionKey`), B3 (DKG `SecretPackage` types) and B4 (nonce
  types). _The store the effect handler fronts; it is what makes DKG coefficients and nonces
  reorg-immune._
- **C2 — Minimal service skeleton.** `state/` + `service/` (`mod.rs` + empty `action.rs` /
  `effect.rs`): stand up a real `ValidatorService` — a pure `state::Transition`, an `effect::Handler`
  and an `action::Encoder` — that **replaces `DummyService`** but does nothing yet. The
  snapshot `State` is just the starting `RolloverState::WaitingForGenesis`; the transition ignores
  every block and event; the `Action`, `Effect` and `Resume` sets are empty (uninhabited) enums that
  Phase D grows. It watches the `Consensus`/`Coordinator` (and oracle) event set — decoding but
  ignoring the events — so the `Driver` runs end to end. The `Coordinator` address is read from the
  `Consensus` contract (`getCoordinator`), not configured. Depends on A1.

  **The `State`, `Action` and `Effect` types are deliberately _not_ fixed up front. They grow
  organically in Phase D**, one transition at a time, so the FSM shape is decided against the actual
  transition logic that uses it rather than guessed in advance. The "State & secret store" and
  "State transitions" specs below describe the _eventual_ shape this converges on, not a C2
  deliverable.

### Phase D — State transitions & effects (depends on A1, B, C)

Each D PR **grows the snapshot `State` and the `Action` / `Effect` sets** with exactly the fields and
variants **one event (or one `NewBlock` check) needs** — the skeleton from C2 is fleshed out one
handler at a time, so the FSM structure is chosen against real transition logic and every PR is an
independently reviewable chunk. The shapes catalogued in the "State & secret store", "Effects" and
"Action encoding" specs are the target these converge on. Roman-numeral slices land in order within a
sub-phase; "Depends on" gives cross-slice ordering. `EpochProposed` is a deliberate no-op (its
rollover message was already verified when `KeyGenConfirmed` was handled) and needs no slice.

The sub-phases are ordered by dependency rather than by contract: the genesis group must go live
before any signing can run, and signing must exist before a non-genesis epoch rollover — whose packet
is itself signed — can complete. That cycle is broken by **splitting `KeyGenConfirmed`**: its
self-contained genesis branch lands in D1, and its rollover-packet branch lands in D4 once signing
exists. All `NewBlock`-driven checks (epoch rollover, keygen timeouts, signing timeouts) likewise
depend on the full event-driven machine and so are grouped into D4.

**D1 — Genesis DKG lifecycle.** Drive the genesis group through the rollover FSM
(`CollectingCommitments → CollectingShares → CollectingConfirmations → EpochStaged`), growing
`RolloverState` and the DKG effects/actions one `KeyGen*` event at a time. Ports `keygen/*`.

- **D1i — Group & participant-set derivation.** ✅ Landed (#551). `consensus/{group,epoch}.rs`: the
  pure group/threshold/context/root derivation the handlers build on.
- **D1ii — Genesis `KeyGen`.** ✅ Landed (#553). `WaitingForGenesis → CollectingCommitments` + the
  `BuildKeyGenCommitment` (`DkgCommit`) effect and `KeyGenAndCommit` action.
- **D1iii — `KeyGenCommitted`.** Register peers' commitments in `State`; when all have committed,
  `→ CollectingShares` and emit `KeyGenSecretShare` action (ECDH-encrypted shares).
  Ports `keygen/committed.ts`. Depends on B3, D1ii.
- **D1iv — `KeyGenSecretShared`.** Collect/verify shares (invalid share → `KeyGenComplain` action);
  when all shared, `→ CollectingConfirmations` and — if my share set completed — emit the `KeyGenConfirm` action. Ports `keygen/secretShares.ts`. Depends on B3, D1iii.
- **D1v — `KeyGenConfirmed` (genesis branch).** Collect confirmations; when the genesis group is fully
  confirmed, `→ EpochStaged{epoch 0}`, record `epoch_groups[0]`, emit `NonceTree` →
  `RegisterNonceCommitments`. _(The non-genesis rollover-packet branch lands in
  D4iii, once signing exists.)_ Ports the genesis path of `keygen/confirmed.ts`. Depends on B3/B4, C1,
  D1iv.
- **D1vi — `KeyGenComplained`.** Complaint accounting; at threshold, restart the keygen; if accused, emit the
  `KeyGenComplaintResponse` action. Ports `keygen/complaintSubmitted.ts`. Depends on D1iv.
- **D1vii — `KeyGenComplaintResponded`.** Register/verify the revealed share; invalid → restart;
  share set completed → `KeyGenConfirm`. Ports `keygen/complaintResponse.ts`. Depends on D1vi.
- **D1viii** Secrets pruning, keep a `Vec` of `(u64 /* block */, Prune)` of things to prune. Later, in D4v the actual pruning will happen.
  

**D2 — Transaction & oracle intake.** Verify proposed transactions and open the signing sessions
(the `WaitingForRequest` / `WaitingToDecline` / `WaitingForOracle` FSM entries). Pure verification and
hashing — no secrets. Ports `service/checks.ts`, `verify/*` and the consensus proposed/attested
handlers. Exercised end to end against a D1-established group, but each handler unit-tests against a
hand-crafted `State`.

- **D2i — Safe-transaction checks.** `checks.rs`: delegatecall / self / selector / multisend /
  config-call policy. No event; pure helper. Ports `service/checks.ts`. Depends on A1.
- **D2ii — EIP-712 hashing & packet verification.** `hashing.rs` + the packet→message-hash helper
  (reuse the sentinel `SolStruct` structs where possible) the transaction/oracle/rollover handlers
  share. No event; helper. Depends on A1, D2i.
- **D2iii — `TransactionProposed`.** Verify → `WaitingForRequest` (valid) or `WaitingToDecline`
  (invalid). Ports `consensus/transactionProposed.ts`. Depends on D2ii.
- **D2iv — `TransactionAttested`.** Clear the `WaitingForAttestation` entry once the attestation
  lands. Ports `consensus/transactionAttested.ts`. Depends on D2ii.
- **D2v — `OracleTransactionProposed`.** Verify → `WaitingForRequest` (oracle packet). Ports
  `consensus/oracleTransactionProposed.ts`. Depends on D2ii.
- **D2vi — `OracleTransactionAttested`.** Clear the entry. Ports
  `consensus/oracleTransactionAttested.ts`. Depends on D2ii.

**D3 — Signing lifecycle.** The nonce effects and the signing FSM that consumes the sessions D2 opens
— the effectful heart of the machine. The last signer's `SignShare` action carries the attestation
`callbackContext` that submits the result on the happy path (Open Question #5); the standalone
attest/stage actions are the timeout fallbacks in D4. Ports `signing/*` + `consensus/oracleResult.ts`.

- **D3i — `Preprocess`.** Link committed nonces to their chunk (the nonce-store `link` effect) and
  clear the group's pending-nonces marker. Ports `signing/preprocess.ts`. Depends on B4, C1.
- **D3ii — `Sign`.** For a live request: top up nonces when low (`NonceTree`), then reveal
  commitments (`RevealNonceCommitments` action, `→ CollectNonceCommitments`); oracle packet →
  `WaitingForOracle`; declined packet → `SignDecline`. Ports `signing/sign.ts` + `commitments.ts`.
  Depends on B4/B5, C1, D2iii/D2v.
- **D3iii — `OracleResult`.** On approval, reveal nonce commitments (as D3ii); on rejection, drop the
  session. Ports `consensus/oracleResult.ts`. Depends on D3ii, D2v.
- **D3iv — `SignRevealedNonces`.** Create the signature share, **burning the nonce** via the atomic
  `UseNonce` effect, with the **graceful no-op on `AlreadyBurned`**; `→ CollectSigningShares` + the
  `SignShare` action (carrying the transaction/oracle attestation callback; the rollover `stageEpoch`
  callback branch lands with D4iii). Ports `signing/nonces.ts` (`handleRevealedNonces`). Depends on
  B5, C1, D3ii.
- **D3v — `SignShared`.** Track collected signature shares. Ports `signing/shares.ts`. Depends on
  D3iv.
- **D3vi — `SignCompleted`.** `→ WaitingForAttestation`. Ports `signing/completed.ts`. Depends on
  D3v. _(Completes the minimal genesis DKG + one-signing-round flow the F1 interop test drives.)_

**D4 — Epoch rollover & timeouts.** The block-driven epoch machine, the non-genesis DKG trigger, and
all `NewBlock` timeout checks — reusing the D3 signing FSM for the rollover packet. Ports
`consensus/rollover.ts`, `keygen/trigger.ts`, `keygen/timeouts.ts`, `signing/timeouts.ts`, the
rollover branch of `keygen/confirmed.ts`, and `consensus/epochStaged.ts`.

- **D4i — `trigger_keygen` helper + `NewBlock` epoch rollover.** Extract the shared `trigger_keygen`
  (participants/threshold/context → `CollectingCommitments` + `DkgCommit`, or `EpochSkipped`) that
  D1vi/D1vii already call, then port `checkEpochRollover`: roll `active_epoch`, trigger the next
  epoch's keygen, and clean up retired epoch groups (`PruneGroupNonces`). Ports `consensus/rollover.ts`
  + `keygen/trigger.ts`. Depends on D1, C1.
- **D4ii — Keygen timeouts (`NewBlock`).** Retire timed-out participants and restart via
  `trigger_keygen`; retiring a keygen emits `PruneDkgSecrets`. Ports `keygen/timeouts.ts`. Depends on
  D4i.
- **D4iii — `KeyGenConfirmed` (rollover branch) + `EpochStaged`.** Non-genesis confirmation computes
  the epoch-rollover packet, `→ SignRollover`, and opens its signing session; `EpochStaged` moves
  `SignRollover → EpochStaged`, records `epoch_groups`, and preprocesses the new group (`NonceTree` →
  `RegisterNonceCommitments`). Completes `keygen/confirmed.ts`; ports `consensus/epochStaged.ts`.
  Depends on D1v, D2ii, D3.
- **D4iv — Signing timeouts (`NewBlock`).** Per-session retry / decline / drop across the signing FSM,
  emitting `SignRequest` on retry and the standalone `AttestTransaction` / `StageEpoch` fallback
  actions. Ports `signing/timeouts.ts`. Depends on D3vi, D4iii.
- **D4v — Pruning maturity** When a safe block is reached, trigger pruning effects for the things that
  need to be pruned.

**D5 — Staker reconciliation.** The `GetValidatorStaker` effect (handler holds the provider), the
`ValidatorStakerSet` event handler, the staker fields on `State`, the `NewBlock` staker check, and the
`SetValidatorStaker` action emission — a faithful, effect-based port of `service.ts`'s
`#setStakerAddress` (replaces the original plan's core `Service::initialize` hook). Depends on A1, C2.

### Phase E — Service assembly (depends on D)

- **E1 — `ValidatorEncoder`.** Implement `ActionEncoder<Action>`: map every `Action` →
  `(Transaction, expires_at)` (calldata + gas + deadline), with mapping tests. Depends on A1, C2.
- **E2 — Finalize service assembly.** The `ValidatorService` bundle and its `main.rs` wiring already
  exist from C2 (the dummy was replaced there); E2 confirms the assembled `Service::components()`
  (transition + effects + encoder) is complete once all of D has landed, and that the watched event
  set follows `[consensus, coordinator, ...allowed_oracles]`. Depends on all of D, E1.

### Phase F — Validation & wrap-up

- **F1 — Integration/interop test.** Run the Rust validator against `Consensus`/`Coordinator` on Anvil:
  genesis DKG happy path + one signing round, ideally interoperating with a TS validator in the same
  group (the deliverable's "work together onchain" check). Depends on E2.
- **F2 — Docs & cleanup.** Update `README.md`/`AGENTS.md` for the completed validator crate; reconcile
  any widened workspace dependency. Depends on all implementation phases.
- **Cleanup.** Per the planning convention, **remove this epic file**
  (`epics/2026_07_01_rust_validator_port.md`) and the companion index
  (`epics/2026_06_09_rust_validator_port_index.md`) once the epic is complete.

### Critical path

`A1 → B1 → B4 → B5 → D3 → D4iii → E1 → E2 → F1`. Phase B fans out from B1 (B2 also needs A1; B3 needs
B1+B2; B4 needs B1; B5 needs B1+B2+B4). C1 (secret store) follows B; C2 (the service skeleton) needs
only A1 and can land early. Phase D grows the state machine C2 stood up, one event per slice, in
dependency order: D1 (genesis DKG) → D2 (intake) → D3 (signing) unblocks the F1 interop test's
"genesis DKG + one signing round"; D4 (rollover + timeouts) then reuses D3's signing FSM. D2i
(`checks.rs`) needs only A1, and D5 (staker) only A1+C2, so both sit off the critical path. FROST (B)
is a thin wrapper now, so the state machine (D) shares the critical path with it.

---

## Open Questions and Assumptions

**Open questions**

1. **`frost-secp256k1` / `frost-core` version alignment.** The spike pinned `frost-secp256k1 = "3"`,
   `frost-core = { version = "3", features = ["internals"] }`, `k256 = "0.13"`, `rand = "0.8"`.
   **Recommended:** pin these **directly in `crates/validator/Cargo.toml`** (validator-only, not a
   workspace dependency) at those versions, confirm the `internals`-feature surface the wrappers touch
   is still exposed, and verify they interoperate with the deployed contracts via the Phase B interop
   vectors.
2. **`expires_at` for encoded transactions.** The core queue needs a per-tx expiry block; the TS action
   queue used a wall-clock timeout. **Recommended:** carry the driving FSM deadline
   (keygen/signing/rollover) on each `Action` variant (the TS `ActionWithTimeout` shape), falling back
   to a config-driven block horizon for deadline-less actions (e.g. `SetValidatorStaker`). Confirm the
   mapping.
3. **Per-action gas limits.** `tx::Transaction.gas` is mandatory and the queue does not estimate it.
   **Recommended:** port the constants from `consensus/protocol/onchain.ts`; revisit with
   `provider.estimate_gas` if they prove brittle.
4. **Shared EIP-712 hashing with the sentinel.** The `SafeTx` / proposal `sol!` structs and hashing
   overlap the sentinel crate. **Recommended:** factor the shared structs into a small shared module
   (in `safenet-core` or a shared crate) rather than duplicating; decide when D2ii lands.
5. **`callback` variants (`keyGenConfirmWithCallback` / `signShareWithCallback`).** The TS actions carry
   an optional callback context. Confirm whether the Rust port must support callbacks in the initial
   port or can defer them.
6. **DKG effect granularity.** The plan models one effect per DKG round (`DkgCommit` / `DkgShares` /
   `DkgFinalize`), keeping round outputs in the snapshot `State` and only the secrets in the store.
   **Recommended:** keep per-round effects (smallest impure surface); confirm against the actual
   `frost` API shapes as the D1 DKG slices (D1iii–D1v) land.

**Assumptions**

- The motivation is **a single shared Rust codebase** on `safenet-core` (and p2p access for the
  validator), not performance. The **hard requirement is onchain + peer compatibility**: byte-identical
  FROST commitments/proofs/nonces/signature-shares/attestations and event/calldata encoding, so a Rust
  and a TS validator interoperate in the same group and signing session. DB and config compatibility
  are **not** required — which also permits the Rust validator to watch `ValidatorStakerSet` even
  though the TS one polls instead.
- The **deterministic** DKG state (recomputable from replayed events + replayed effects) is **embedded
  in the reorg-aware snapshot `State`** (deviation #3), while the **locally-generated random secrets**
  — DKG coefficients + encryption key, and signing nonces — live in a **separate, reorg-immune SQLite
  store** (deviations #3 and #4), because a reorg could otherwise roll them back while the transaction
  that committed to them is re-included (stranding a keygen) or un-burn a nonce (leaking the signing
  share). The `StateTransition` stays **pure** (core #512): all secret-store access, sampling and the
  startup RPC read go through the **effect system** — the DKG-commit effect reuses existing secrets
  rather than resampling, an `AlreadyBurned` resume gracefully emits no action, resume values carry
  correlating ids (ordering is unspecified), and the store is pruned via effects when a keygen
  resolves or a group retires.
- Startup **staker reconciliation is effect-based** (a `GetValidatorStaker` effect + the
  `ValidatorStakerSet` event, Phase D5); the original plan's core `Driver`/`Service::initialize` hook
  is superseded by #512 and **no `safenet-core` changes are needed**.
- The port **reuses `safenet-core`** for indexing, state/reorg, tx submission, observability, the
  SQLite pool and the `Driver`; the validator supplies the `Service` components (a pure `Transition`,
  an `EffectHandler` and an `ActionEncoder`).
- FROST uses the **ZCash Foundation `frost-secp256k1` / `frost-core` crates** (standard RFC 9591
  FROST(secp256k1, SHA-256), the scheme Safenet already implements) — **not** a hand-rolled
  implementation. Only the Safenet-specific wrappers (address identifiers, ECDH share encryption,
  keccak merkle trees, nonce-tree preprocessing, solidity marshalling) are ported, modelled on the
  spike's `frost/*`; **parity vectors target those wrapping layers** plus one TS-interop DKG-and-sign
  round.
- Following the planning convention, this plan is proposed as a **docs-only PR** with no epic
  implementation code, and is removed on completion.
