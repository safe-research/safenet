# Plan: Port the TypeScript validator to Rust

Component: the existing `crates/validator` crate (Cargo package `validator`) plus one enabling
change to `crates/core` (`safenet-core`). Ports `validator/src/` — the state machine (`service/`,
`machine/`), the consensus protocol (`consensus/`), and the FROST cryptography (`frost/`).

---

## Overview

The Safenet validator is currently a TypeScript service (`validator/src/`, the largest client in the
repo). It follows the `Consensus` and `Coordinator` contracts, runs an epoch-rollover / distributed
key generation (DKG) state machine and a threshold-signing state machine over a custom **FROST**
(secp256k1) scheme, and submits the resulting commitments, shares, nonces, signature shares and
attestations onchain. This epic ports it to Rust on top of `safenet-core`.

The `crates/validator` crate already exists as a **scaffold**: `main.rs` wires a `DummyService`
through `safenet_core::Driver::new(..).run()`, and `config.rs` loads a TOML config with the
driver/observability/tx sub-configs flattened in (PRs #489, #497). This epic replaces the dummy with
the real validator: a `safenet_core::driver::Service` whose `State` is the DKG + signing + rollover
state, whose `StateTransition` ports the TS handlers, and whose `encode_actions` ports the onchain
protocol encoding — plus the FROST library the transitions depend on.

Motivation matches the sibling sentinel port (`epics/2026_06_25_rust_sentinel_port.md`): **a single
shared Rust codebase built on `safenet-core`**, and — for the validator specifically — access to the
p2p stack that lives on the Rust side. As with the sentinel, the **hard requirement is onchain and
peer compatibility**: a Rust validator must produce byte-identical FROST commitments, proofs,
nonces, signature shares and attestations, so it can participate in the _same_ group and the _same_
signing session as a TypeScript validator and have the `Consensus`/`Coordinator` contracts accept its
transactions. Database and configuration backwards-compatibility are **not** required (the port picks
its own SQLite schema and TOML config).

The work divides into seven phases, most of which parallelize after the foundation:

- **A** — one enabling change in `safenet-core`: a `Service` **initialization hook** so a service can
  read onchain state at startup and emit actions (needed for the validator's staker-address
  reconciliation; the driver has no such hook today).
- **B** — validator **bindings** (`sol!` for `Consensus`/`Coordinator`) and **config**.
- **C** — the **FROST layer**: a thin Safenet-specific wrapper over the ZCash Foundation
  `frost-secp256k1` / `frost-core` crates (the standard RFC 9591 ciphersuite the scheme already uses) —
  DKG, signing, ECDH share encryption, nonce-tree preprocessing, merkle trees and solidity marshalling,
  with **parity vectors on the wrapping layers**.
- **D** — **state**: the snapshotted `State` (rollover + signing FSMs + consensus + the deterministic
  DKG material) and the **separate, reorg-immune secret store** for locally-generated random secrets
  (DKG coefficients + encryption key, and signing nonces), with pruning.
- **E** — the **state transitions**: rollover/keygen/signing/attestation handlers.
- **F** — **service assembly**: `encode_actions`, the staker-address `initialize` hook, and wiring the
  real service into `main.rs`.
- **G** — **validation & cleanup**: an Anvil integration/interop test, docs, and removing this plan.

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
| `service/service.ts`                                                     | `ValidatorService`: wires storage + protocol + state machine + watcher; **`start()` reconciles the staker address**, then starts the watcher. | the loop is `safenet-core::driver::Driver`; the validator supplies a `Service`; staker reconciliation becomes `Service::initialize` (Phase A + F2) |
| `service/machine.ts`                                                     | `SafenetStateMachine`: transition **queue**, block/event dispatch, applies `StateDiff`s, yields `ProtocolAction`s.                            | subsumed by `safenet-core::state::StateMachine`; the validator only implements the `StateTransition`                                               |
| `machine/types.ts`, `machine/transitions/types.ts`                       | `RolloverState` / `SigningState` FSMs, `ConsensusState`, `StateDiff`, and the full typed event set.                                           | `state.rs` (the snapshot `State`) + the `sol!` event set in `bindings.rs`                                                                          |
| `consensus/rollover.ts`, `keygen/*`                                      | Epoch rollover, DKG rounds (genesis, commitments, shares, confirmations, complaints), timeouts.                                               | `state.rs` transition handlers (Phase E1/E2)                                                                                                       |
| `consensus/signing/*`                                                    | Nonce-tree preprocessing, nonce-commitment reveal, signature-share creation (+ **nonce burn**), completion.                                   | transition handlers (Phase E3) + the nonce store (Phase D1)                                                                                        |
| `consensus/keyGen/client.ts`, `signing/client.ts`                        | Thin wrappers over `SqliteClientStorage` holding **all** crypto state.                                                                        | **removed**: crypto state moves into the snapshot `State`; nonces move into the separate nonce store (see Architecture Decision)                   |
| `consensus/storage/sqlite.ts`                                            | 7-table SQLite store: groups, participants, secret shares, nonce links, nonces, signatures, commitments.                                      | **split**: non-nonce crypto state → snapshot `State`; nonces → `nonces.rs` store (Phase D)                                                         |
| `consensus/verify/engine.ts`, `service/checks.ts`, `verify/*/hashing.ts` | Packet verification, Safe-transaction policy checks, EIP-712 message hashing.                                                                 | `checks.rs` + transition handlers (Phase E4); hashing via `alloy` `SolStruct::eip712_signing_hash`                                                 |
| `consensus/protocol/{base,onchain,transaction}.ts`                       | Action queue + action→calldata encoding + tx submission/nonce/fee management.                                                                 | `encode_actions` (Phase F1); the queue/fees/resubmit are `safenet-core::tx::TransactionQueue`                                                      |
| `consensus/protocol/sqlite.ts`                                           | Persistent action queue + tx nonce/fee store.                                                                                                 | subsumed by the `Driver` + `TransactionQueue` (no separate action queue)                                                                           |
| `frost/*`, `consensus/merkle.ts`, `utils/participants.ts`                | FROST math, hashing, VSS, ECDH, nonces, signing, merkle trees.                                                                                | `frost/*` + `merkle.rs` (Phase C)                                                                                                                  |
| `shared/watcher.ts`, `watcher/*`                                         | Block + event indexing, reorg detection.                                                                                                      | `safenet-core::index::Watcher` (**done**)                                                                                                          |
| `utils/logging.ts`, `utils/metrics.ts`                                   | Logging + Prometheus.                                                                                                                         | `safenet-core::observability` (**done**)                                                                                                           |
| `types/schemas.ts` (`zod`)                                               | Env validation.                                                                                                                               | `config.rs` (`serde` + TOML), already scaffolded                                                                                                   |

Shared infrastructure the validator consumes from `safenet-core` (all merged): `index` (watcher),
`state` (`StateMachine` + `SnapshotStore`), `tx` (`TransactionQueue` + `Signer`), `observability`,
`driver` (`Driver` + `Service`), and the shared `SqlitePool`.

---

## Architecture Decision

The validator is the `async` (`tokio`) **binary** it already is; the port fills in the real
`Service` and the FROST library it needs, and adds one hook to `safenet-core`.

| Concern                              | TypeScript today                                                      | Rust choice                                                 | Notes                                                                                                                                    |
| ------------------------------------ | --------------------------------------------------------------------- | ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| RPC / primitives / signing / EIP-712 | `viem`                                                                | **`alloy`**                                                 | `Provider`, `Address`/`B256`/`U256`, typed events & EIP-712 via `sol!`; already used by the crate.                                       |
| Service orchestration                | `SafenetStateMachine` + `ValidatorService`                            | **`safenet-core::driver::{Driver, Service}`**               | The validator implements `Service`; the `Driver` runs indexing + state + submission. The TS transition queue is the core `StateMachine`. |
| Indexing                             | `shared/watcher.ts`                                                   | **`safenet-core::index`**                                   | `Watcher<P, E>` over the `sol!` event set; reuse, don't re-port.                                                                         |
| Service state + reorg rollback       | `SqliteClientStorage` + `SqliteStateStorage` (no rollback)            | **`safenet-core::state`**                                   | `StateMachine` drives a pure `StateTransition`; `SnapshotStore` rolls back per-block on reorg (an improvement over the TS store).        |
| Crypto state (groups, shares, keys)  | separate `KeyGenClient`/`SigningClient` over SQLite                   | **embedded in the snapshot `State`**                        | **Deviation (user-requested):** DKG material lives in the state machine, not in separate clients. See below.                             |
| FROST **nonces**                     | rows in the shared SQLite store                                       | **separate SQLite table, burned on use, never rolled back** | **Deviation (user-requested):** makes the transition impure; see below.                                                                  |
| Transaction submission + queueing    | `TransactionManager` + `SqliteQueue`                                  | **`safenet-core::tx::TransactionQueue`**                    | Durable queue: nonce mgmt, fee bump/resubmit, in-flight cap, per-tx block expiry. Subsumes the TS tx manager and action queue.           |
| Startup onchain reconciliation       | `service.start()` → `getValidatorStaker` → maybe `setValidatorStaker` | **new `Service::initialize` hook on the `Driver`**          | The driver has no bootstrap hook today (Phase A).                                                                                        |
| FROST library                        | hand-rolled on `@noble/curves`                                        | **ZCash `frost-secp256k1` / `frost-core`** (see decision below) | The scheme is standard RFC 9591 FROST(secp256k1, SHA-256); the crate is byte-compatible by construction. Only Safenet wrappers are ours. |
| SQLite                               | `better-sqlite3` (sync)                                               | **`sqlx`** (async), one shared `SqlitePool`                 | Snapshot store, tx queue and nonce store share one pool, matching the single-`Database` TS pattern.                                      |
| Config validation                    | `zod`                                                                 | **`serde` + `toml` + `argh`**                               | Already scaffolded in `config.rs`/`main.rs`.                                                                                             |
| Errors                               | `viem` `BaseError`                                                    | **`thiserror`**                                             | Per-module error enums; `main` returns `Box<dyn Error>`, as today.                                                                       |

Key decisions:

- **Implement `Service`; the `Driver` runs everything.** The validator supplies one type implementing
  `StateTransition` (`new_block` + `event`) plus `encode_actions`; `main.rs` already builds the
  `Watcher`/`StateMachine`/`TransactionQueue` and hands them to `Driver::new(..).run()`. The TS
  `SafenetStateMachine`'s transition queue, block/event ordering and reorg handling are all the core
  `StateMachine`'s job. There is **no** validator-side service loop or action queue.

- **The `Service::Event` is the `sol!`-generated typed event set.** As in the sentinel, the state
  machine's `Event` _is_ the watcher's event type. `bindings.rs` declares it with `watcher_events!`
  over the generated `Consensus::ConsensusEvents` and `Coordinator::CoordinatorEvents` enums (plus the
  oracle result event), and `StateTransition::event` matches on it directly. This replaces
  `machine/transitions/types.ts` and the `zod` decoding in the TS watcher.

- **Deterministic crypto state is embedded in the snapshot `State`, not in separate clients (user
  deviation #3).** The TS `KeyGenClient` and `SigningClient` are thin wrappers over a plain SQLite
  store holding group info, commitments, secret shares, verification/signing shares, and the group
  public key. In Rust the **deterministic** subset — everything derivable by replaying onchain events
  (group participants/threshold, peers' published commitments and encrypted shares, and the
  verification share, signing share and group public key derived from them) — becomes fields of the
  `Serialize + Deserialize` snapshot `State`. This is correct under reorgs: a reorg rolls it back and
  the replayed events recompute it identically. The FROST math functions (Phase C) become **pure,
  stateless** functions the transitions call with data drawn from `State` and the secret store; the
  client classes disappear. The **locally-generated random secrets are the exception** — they must
  **not** live in the snapshot (see the next decision).

- **Locally-generated random secrets live in a separate, reorg-immune SQLite store (user deviations #3
  and #4).** Two kinds of secret are sampled locally and then committed to onchain, and **neither can
  be in the reorg-aware snapshot**:

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
  does **not** roll back. The store is keyed so a transition can look up an existing secret before
  generating a new one. The consequence, called out explicitly by the user, is that **the state
  transition is not pure**: the same transition on the same starting `State` can behave differently
  depending on the store. Because `safenet-core`'s `StateTransition` is **infallible** (`&mut self`,
  `async`, "must gracefully recover"), the transitions handle this explicitly:

  - **keygen setup** looks up the group's coefficients / encryption key in the store; if present (a
    pre-reorg generation whose commitment may have landed), it **reuses** them rather than sampling
    fresh — so a reorged-and-re-included commitment stays consistent with the retained secret. Only
    when absent does it sample and persist.
  - **signature-share creation** burns the nonce; a replay that finds its nonce already burned emits
    **no** signature-share action (graceful no-op) rather than erroring (the TS code throws here and
    the caller swallows it; the Rust port makes the recovery explicit).

  **Pruning is required so the store does not grow unbounded.** DKG secrets are pruned when the group's
  keygen resolves — successfully (`KeyGenConfirmed` / signing share finalized) **or** unsuccessfully
  (keygen timeout / complaint abort); at that point the coefficients and encryption key are no longer
  needed. Nonces are burned on use and their tree pruned when the owning group is retired (its epoch
  rolls out of the reorg window). The transition reaches the store through a handle held on the
  `Service` (`&mut self` + the shared pool).

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
  end-to-end DKG-and-sign round asserting the crate interoperates with a TS-produced group.

- **EIP-712 message hashing via `sol!` + `SolStruct::eip712_signing_hash`.** The messages the group
  signs (Safe-tx hash, epoch-rollover packet, oracle-tx proposal) are hashed the same onchain-identical
  way the sentinel port established. Reuse the sentinel crate's `SafeTx`/proposal `sol!` structs and
  hashing where they overlap (candidate for a small shared module); the source of truth for typehashes
  is `contracts/src/libraries/ConsensusMessages.sol`.

- **Actions encode to transactions; the queue owns fees.** `encode_actions` maps each validator action
  to `(tx::Transaction, expires_at)` — calldata from the `sol!`-generated `Coordinator`/`Consensus`
  call, per-action gas (ported from `consensus/protocol/onchain.ts`), and `expires_at` derived from the
  relevant FSM deadline. Fees (estimate, cap, replacement bump) are entirely the `TransactionQueue`'s
  job; the validator does not compute fees.

- **The initialization hook is a `safenet-core` change (Phase A).** `Service` gains
  `initialize<P: Provider>(&mut self, provider: &P) -> impl Future<Output = Vec<Self::Action>>` with a
  default no-op body, and `Driver::run` calls it before the indexer loop, encoding the returned actions
  and queuing them. This is backward-compatible (the sentinel/dummy inherit the no-op) and general (any
  service can reconcile onchain state at startup). The validator implements it to read
  `getValidatorStaker(account)` and emit a `SetValidatorStaker` action when it differs from the
  configured staker — a faithful port of `service.ts`'s `#setStakerAddress`.

### Alternatives Considered

- **Keep the `KeyGenClient`/`SigningClient` split with a plain (non-reorg) crypto store.** Rejected per
  the user's deviation #3: embedding the deterministic crypto state in the reorg-aware snapshot is
  strictly more correct and removes a parallel storage layer. (Random secrets are the deliberate
  exception — see below.)
- **Put the random secrets (DKG coefficients / encryption key / nonces) in the snapshot too (fully
  pure transition).** Rejected per deviations #3 and #4: a reorg could roll back a locally-generated
  secret **while the transaction that committed to it is re-included on the reorged chain** — for DKG
  coefficients / the encryption key this strands the validator mid-keygen (it can no longer produce the
  matching shares or decrypt peers' shares); for nonces it would permit reuse and leak the signing
  share. These secrets must be reorg-immune even at the cost of transition purity.
- **Hand-roll the FROST primitives on `k256` (the TS `@noble/curves` approach).** Rejected: Safenet's
  scheme is standard RFC 9591 FROST(secp256k1, SHA-256), which the ZCash Foundation
  `frost-secp256k1`/`frost-core` crates implement exactly and byte-compatibly; re-implementing the
  field/curve/hash math would only add a large surface to keep in lock-step with the contracts for no
  benefit. Only the Safenet-specific wrappers (identifiers, ECDH, merkle trees, nonce-tree
  preprocessing, marshalling) are ours.
- **Do staker reconciliation in `main.rs` before `Driver::run`.** Rejected: it belongs _on the
  service_ (the user's framing — "initialization should be defined on the service"), needs the same
  action encoding as everything else, and generalizes to other services; hence the `Driver` hook.
- **A separate transition/event type decoded from raw logs (mirroring `machine/transitions`).**
  Rejected — the `Driver` ties the state machine's `Event` to the watcher's typed event set, so the
  `sol!` enum is used directly.
- **Hand-roll a transition queue / action queue.** Rejected — the core `StateMachine` and
  `TransactionQueue` already provide ordering, persistence, retry and expiry.

---

## Tech Specs

### Crate layout

```
crates/core/
  src/driver.rs              # Phase A: add Service::initialize hook + Driver call site (only core change)

crates/validator/
  Cargo.toml                 # add crate-local frost-secp256k1/frost-core/k256/rand (NOT workspace deps)
  src/
    main.rs                  # exists — Phase F3 swaps DummyService for the real Service
    config.rs                # exists — Phase B2 adds validator fields
    bindings.rs              # Phase B1: sol! Consensus + Coordinator; watcher_events! event set
    frost/                   # thin Safenet-specific layer over frost-secp256k1 / frost-core
      mod.rs                 # re-exports
      keygen.rs              # DKG rounds via frost keys::dkg::part{1,2,3} (port of the spike)
      signing.rs             # signing via frost round1::commit / round2::sign / aggregate
      secret.rs              # ECDH-XOR share encryption/decryption (port of the spike)
      participants.rs        # address-derived Identifier + sorting (port of the spike)
      nonces.rs              # 1024-nonce preprocessing: batch frost SigningNonces, merkle, sequence
      marshal.rs             # FROST <-> solidity Point{x,y}/U256/signature/commitment (port of the spike)
    merkle.rs                # keccak256 merkle: participants root/proof, signer set, nonce-tree proof
    secrets.rs               # Phase D1: separate reorg-immune SQLite store for local random secrets
                             #   (DKG coefficients + encryption key; nonce trees + burn) with pruning
    state.rs                 # Phase D2: snapshot State (rollover + signing FSM + consensus + derived DKG)
    checks.rs                # Phase E4a: Safe-transaction policy checks (delegatecall/multisend/config)
    service.rs               # exists (Dummy) — Phase E/F: real StateTransition + encode_actions + initialize
    hashing.rs               # EIP-712 message hashing (SafeTx / rollover / oracle proposal); reuse sentinel's where possible
```

Modules are introduced only when first used (no empty stubs), matching the crate convention. If
`service.rs` grows past the size budget, the transition handlers split into `keygen.rs` / `signing.rs`
/ `rollover.rs` submodules (the phase breakdown already anticipates this).

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
- **`rand = "0.8"`** — RNG for DKG / nonce / encryption-key sampling (matching FROST v3's `rand_core`).

These four are pinned in the crate rather than inherited; the general "new shared crates go to
`[workspace.dependencies]` first" rule still applies to any dependency that is genuinely shared. No
hand-rolled hash/curve crate (`sha2` etc.) is needed — the ciphersuite is the `frost-*` crates.

### Onchain bindings (`bindings.rs`)

- `sol!` blocks for the two contracts, transcribed from `validator/src/types/abis.ts` (events +
  functions) and `contracts/src/`:
  - **`Consensus`** — events `EpochStaged`, `EpochProposed`, `TransactionProposed`,
    `TransactionAttested`, `OracleTransactionProposed`, `OracleTransactionAttested`; calls
    `proposeEpoch`, `stageEpoch`, `attestTransaction`, `attestOracleTransaction`, `setValidatorStaker`,
    `getValidatorStaker`.
  - **`Coordinator`** — events `KeyGen`, `KeyGenCommitted`, `KeyGenSecretShared`, `KeyGenConfirmed`,
    `KeyGenComplained`, `KeyGenComplaintResponded`, `Preprocess`, `Sign`, `SignRevealedNonces`,
    `SignShared`, `SignCompleted`; calls `keyGenAndCommit`, `keyGenCommit`, `keyGenSecretShare`,
    `keyGenComplain`, `keyGenComplaintResponse`, `keyGenConfirm[WithCallback]`, `preprocess`, `sign`,
    `signDecline`, `signRevealNonces`, `signShare[WithCallback]`.
  - The oracle `OracleResult` event (for `handleOracleResult`).
  - Shared `Point{uint256 x; uint256 y}`, `Attestation{Point r; uint256 z}` and the `SafeTransaction`
    tuple, modelled on `project/ports:validator-rust/src/bindings.rs`.
- The watcher event set is declared with `watcher_events!` over the generated `*Events` enums (plus a
  variant for the oracle event). **`alloy` types are an area LLMs get wrong** — expect to hand-hold
  this PR against the compiler; the spike's `bindings.rs` is the working reference to copy the `sol!`
  layout and the `.into_inner()` decode bridge from.

### FROST cryptography (`frost/*`, `merkle.rs`) — a thin layer over the ZCash crates

The FROST ciphersuite is **`frost-secp256k1` (RFC 9591 FROST(secp256k1, SHA-256))** — the DKG rounds,
nonce/commitment generation, signature shares and aggregation come from the crate and need no porting.
Our code is only the Safenet-specific wrapping, modelled on the spike
(`project/ports:validator-rust/src/frost/*`):

- **DKG** (`keygen.rs`): drive `keys::dkg::part1/2/3`, keyed by address-derived identifiers, with
  ECDH-encrypted secret shares — a direct port of the spike's `generate_round{1,2,3}`
  (`round1_packages` / `round2_encrypted_shares` helpers). The **random secrets** these produce
  (`round1/round2::SecretPackage`, encryption key) go to the reorg-immune secret store (Phase D1), not
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
  in the secret store (Phase D1).
- **Marshalling** (`marshal.rs`): FROST point/scalar ↔ solidity `Point{x,y}`/`U256` (uncompressed),
  signature ↔ `(Point r, uint256 z)`, commitment tuple `(q, c[], r, mu)` — port of the spike's
  `marshal.rs`.

**Parity vectors target the wrapping layers, not the ciphersuite** (which is byte-compatible by
construction). Vectors captured from the TS / a contract `eth_call` cover identifier derivation, ECDH,
the merkle roots/proofs, the nonce-tree commitment and the marshalled ABI encodings; one end-to-end
test asserts a Rust participant interoperates with a TS-produced group through a full DKG-and-sign
round. A tiny TS dump script (alongside `cmd/derive-genesis.ts`) or committed JSON fixtures produce the
vectors.

### State & secret store (`state.rs`, `secrets.rs`)

- `state.rs` — the snapshot `State` (`Serialize + Deserialize + Default`), composing:
  - **consensus**: `active_epoch`, `genesis_group_id?`, `epoch_groups` (epoch→group id),
    `signature_id_to_message`, `group_pending_nonces` (port of `ConsensusState`).
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
    all recomputable from replayed events, so it is reorg-safe in the snapshot. The
    **locally-generated random secrets are excluded** — the group's `coefficients` and
    `encryption secret key`, and all nonces, live in the reorg-immune secret store below.
- `secrets.rs` — a `sqlx` store over the shared pool holding the locally-generated random secrets,
  **not** part of the snapshot and **not** rolled back on reorg:
  - **DKG secrets** table: per `(group_id, address)`, the `coefficients` and `encryption_secret_key`.
    Written by keygen setup (before the commitment action is emitted); read on setup replay so a
    reorged-and-re-included commitment reuses the same secrets. Pruned when the group's keygen resolves
    (`prune_dkg_secrets(group_id)`).
  - **Nonces** tables (ported from `consensus/storage/sqlite.ts`): `nonce_links(root PK, group_id,
    address, chunk)` and `nonces(leaf PK, root FK, offset, hiding, hiding_commitment, binding,
    binding_commitment)`.
  - methods: `dkg_secrets(group, me)` / `store_dkg_secrets(group, me, coefficients, encryption_key)` /
    `prune_dkg_secrets(group)`; `register_nonce_tree`, `link_nonce_tree(chunk→root)`,
    `nonce_tree(group, me, chunk)`, `burn_nonce(group, me, chunk, offset)` (NULLs the secret scalars),
    `available_nonce_count`, `prune_group_nonces(group)`.
  - `store_dkg_secrets` is a no-op when secrets for the key already exist (reuse-not-overwrite);
    `burn_nonce` is idempotent and permanent. Both reads survive a snapshot rollback. Tests cover:
    DKG-secret reuse across a simulated reorg, that pruning removes resolved-group secrets, burn
    idempotency, and that a re-requested burned nonce yields "already burned" (→ the transition's
    graceful no-op).

### State transitions (`service.rs` + submodules)

The `Service` struct holds the machine config (account, participants + info, `genesis_salt`,
`blocks_per_epoch`, `key_gen_timeout`, `signing_timeout`, `allowed_oracles`, `oracle_timeout`) and the
secret-store handle, and implements `StateTransition<State>`:

- `new_block(state, block)` ports `progressToBlock`: epoch-rollover check (→ trigger next keygen /
  `StageEpoch`), keygen-timeout check, signing-timeout check. Timeout/abort transitions that retire a
  keygen also **prune its DKG secrets** from the secret store.
- `event(state, event)` matches the typed event set and ports the per-event handlers (genesis + keygen
  committed/shared/confirmed/complaints; preprocess/sign/reveal/share/completed/decline;
  epoch proposed/staged; transaction & oracle proposed/attested; oracle result). Transaction handlers
  run the Safe-tx `checks.rs` and EIP-712 hashing to decide attest-vs-decline and produce the message
  to sign. The impure, secret-store-touching handlers are:
  - **keygen setup** samples the group's `coefficients` + `encryption secret key` — but first looks
    them up in the secret store and **reuses** any existing entry, so a reorged-and-re-included
    commitment stays consistent (the reorg-safety fix). `KeyGenConfirmed` / signing-share finalization
    **prunes** the group's DKG secrets.
  - **signing** generates nonce trees into the store, and on signature-share creation **burns** the
    nonce; a replay onto an already-burned nonce emits no action (graceful no-op).

The **`Action`** enum ports `consensus/protocol/types.ts` `ProtocolAction` (13 variants across keygen /
signing / consensus, including `SetValidatorStaker`).

### Action encoding (`encode_actions`)

Maps each `Action` to `(tx::Transaction, expires_at)`: calldata from the `sol!` call (e.g.
`Coordinator::keyGenAndCommitCall{..}.abi_encode()`), `value = 0`, `gas` per the
`consensus/protocol/onchain.ts` estimates (e.g. keyGenAndCommit ≈ 250k, keyGenSecretShare ≈
250k + 25k·shares, sign/attest/stage ≈ 400k, decline ≈ 80k), and `expires_at` from the driving FSM
deadline. Fees are the `TransactionQueue`'s responsibility.

### Service initialization (`initialize`)

`initialize(provider)` reads `Consensus::getValidatorStaker(account)`; if it differs from the
configured staker it returns a single `SetValidatorStaker` action (encoded + queued by the driver
before the loop). Idempotent — safe to run on every startup, matching the TS `#setStakerAddress`.

### Config (`config.rs`)

Extend the existing `Config` with the validator fields (consensus + coordinator addresses, staker,
participants + participant info, `genesis_salt`, `blocks_per_epoch`, `key_gen_timeout`,
`signing_timeout`, `allowed_oracles`, `oracle_timeout`, `skip_genesis`), keeping `#[serde(default)]`
and the flattened `driver`/`observability` sub-configs. `main.rs` follows the events emitted by
`[consensus, coordinator, ...allowed_oracles]`.

### Testing

- Unit tests mirror the TS `*.test.ts` intent (behavior, not implementation): FROST parity vectors
  (per Phase C PR), the secret store (DKG-secret reuse-across-reorg, pruning, nonce burn/idempotency),
  the `StateTransition` (rollover/keygen/signing FSMs, timeouts, the **burned-nonce graceful no-op**),
  `checks.rs`, and `encode_actions` (calldata + gas + expiry). Reorg rollback and tx resubmission are
  covered by core; a validator-level test asserts a reorged keygen event rolls back the derived DKG
  material in the snapshot **while the DKG secrets and burned nonces in the secret store survive**.
- `sqlx` tests run against `sqlite::memory:`.
- The `consensus/integration.test.ts` end-to-end flow (genesis DKG → signing) is the model for the
  Phase G interop test.

### Tooling

Per `AGENTS.md`: `cargo fmt --all`, `cargo clippy -w validator` / `-w safenet-core`,
`cargo test -w validator` / `-w safenet-core`; `Cargo.lock` committed. Features are workspace-inherited.

---

## Implementation Phases

Each PR has a single purpose, targets < 300 changed LOC and < 10 files, and is independently
reviewable. "Depends on" lists hard ordering; everything else may proceed in parallel. FROST (Phase C)
is the long pole and is intentionally sliced small.

### Phase A — Core `Driver` initialization hook (`safenet-core`; unblocks F2)

- **A1 — `Service::initialize` hook.** In `crates/core/src/driver.rs`, add
  `initialize<P: Provider>(&mut self, provider: &P) -> impl Future<Output = Vec<Self::Action>>` to the
  `Service` trait with a default no-op body, and call it from `Driver::run` before the loop, encoding
  the returned actions via `encode_actions` and queuing them. Update the `DummyService` (inherits the
  no-op) and add a test that a returned action is queued at startup. _Single purpose: the bootstrap
  hook._ Independent of all validator phases except F2.

### Phase B — Bindings & config (validator; unblocks C2/E/F)

- **B1 — Onchain bindings.** `bindings.rs`: `sol!` for `Consensus` + `Coordinator` (events + calls) and
  the oracle result event, the shared `Point`/`Attestation`/`SafeTransaction` structs, and the
  `watcher_events!` event set. Modeled on the spike's `bindings.rs`. _No behavior; the typed surface._
- **B2 — Config fields.** Extend `config.rs` with the validator fields + tests (parses required fields;
  flattened driver/observability still default). Depends on nothing else; parallel with B1.

### Phase C — FROST layer over `frost-secp256k1` (depends on B1 for the marshalling/DKG ABI types)

A thin wrapper over the ZCash crates; each PR ships parity vectors for its Safenet-specific layer.
C1/C2 are the foundation; the rest fan out.

- **C1 — Identifiers, ECDH & merkle.** `frost/{participants,secret}.rs` + `merkle.rs`: address-derived
  `Identifier`, canonical sorting, the ECDH-XOR `EncryptionKey`, and the keccak merkle utilities
  (participants root/POAP, signer-set proof, nonce-tree leaf hashing/proof). Parity vectors vs the TS
  `identifier.ts`/`secret.ts`/`merkle.ts`. Depends only on the `frost-core` types + keccak.
- **C2 — Solidity marshalling.** `frost/marshal.rs`: FROST point/scalar/signature/commitment ↔ the ABI
  `Point`/`U256`/`(Point,U256)`/`(q,c[],r,mu)` tuples — port of the spike's `marshal.rs`. Depends on B1
  (the `sol!` `Point`/commitment structs) and C1. Parity vectors against contract-shaped encodings.
- **C3 — DKG.** `frost/keygen.rs`: `keys::dkg::part{1,2,3}` driven with address identifiers and
  ECDH-encrypted shares — port of the spike's `generate_round{1,2,3}`. Depends on C1, C2. Vectors: a
  full small-group DKG round, and interop with a TS-produced commitment/share set.
- **C4 — Nonce preprocessing.** `frost/nonces.rs`: batch 1024 crate `SigningNonces` into a chunk under
  a keccak merkle root, `sequence`→`(chunk, offset)`. Depends on C1. Parity vectors vs `signing/nonces.ts`.
- **C5 — Signing & aggregation.** `frost/signing.rs`: `round1::commit` / `round2::sign` / `aggregate`
  keyed by the Safenet identifiers + signer-set merkle. Depends on C1, C2, C4. Vectors + a full
  sign→aggregate→verify round interoperating with TS-produced shares.

### Phase D — State & secret store (depends on C; parallel with late C)

- **D1 — Secret store.** `secrets.rs`: the separate reorg-immune `sqlx` store for the
  locally-generated random secrets — the DKG-secrets table (`dkg_secrets` / `store_dkg_secrets` /
  `prune_dkg_secrets`) and the nonce tables (`register` / `link` / `nonce_tree` / `burn_nonce` /
  `available_count` / `prune_group_nonces`) — with tests for DKG-secret reuse (no overwrite), pruning,
  and burn idempotency. Depends on C1 (`EncryptionKey`), C3 (DKG `SecretPackage` types) and C4 (nonce
  types). _This is the deliberately-impure store that makes DKG coefficients and nonces reorg-immune._
- **D2 — Snapshot `State`.** `state.rs`: the `State` type (consensus + rollover + signing FSMs + the
  **deterministic** per-group DKG material; random secrets excluded), `Serialize + Deserialize +
  Default`, plus the `Action` enum. Depends on C1/C3 (identifier + DKG types) and B1 (event/id types).
  _Types only; transitions are Phase E._

### Phase E — State transitions (depends on B1, C, D)

- **E1 — Rollover, epoch & genesis + timeouts.** `new_block` epoch-rollover / keygen-trigger /
  timeout checks; genesis + epoch-staged handlers. Ports `consensus/rollover.ts`, `keygen/genesis.ts`.
  Depends on B1, C3, D2.
- **E2 — KeyGen event handlers.** `KeyGen`/`KeyGenCommitted`/`KeyGenSecretShared`/`KeyGenConfirmed` +
  complaint submitted/responded; DKG rounds over `State`'s deterministic group material. Keygen setup
  samples coefficients + encryption key into the secret store **reusing any existing entry** (the
  reorg-safety fix), and confirmation/abort **prunes** the group's DKG secrets. Ports `keygen/*`.
  Depends on C3, D1, D2.
- **E3 — Signing event handlers.** `Preprocess`/`Sign`/`SignRevealedNonces`/`SignShared`/
  `SignCompleted`/decline; nonce-tree generation, commitment reveal, signature-share creation **+ nonce
  burn**, incl. the **graceful no-op on an already-burned nonce**. Reads/writes the secret store. Ports
  `signing/*`. Depends on C4/C5, D1, D2. _The impure heart of the machine._
- **E4a — Safe-transaction checks.** `checks.rs`: delegatecall/self/selector/multisend/config-call
  policy checks. Ports `service/checks.ts`. Depends on B1.
- **E4b — Transaction & oracle handlers.** `TransactionProposed`/`Attested`,
  `OracleTransactionProposed`/`Attested`, `OracleResult`; EIP-712 message hashing (`hashing.rs`, reuse
  sentinel structs where possible), verify→attest-or-decline→`sign_request`. Depends on E4a, C, D2.

### Phase F — Service assembly (depends on E; F2 also on A1)

- **F1 — `encode_actions`.** Map every `Action` → `(Transaction, expires_at)` (calldata + gas +
  deadline), with mapping tests. Depends on B1, D2 (`Action`).
- **F2 — `Service::initialize` (staker bootstrap).** Implement `initialize` reading
  `getValidatorStaker` and emitting `SetValidatorStaker` on mismatch. Depends on A1, B1, F1.
- **F3 — Wire the real service into `main.rs`.** Replace `DummyService` with the validator `Service`;
  follow `[consensus, coordinator, ...allowed_oracles]`; remove the dummy. Depends on all of E, F1, F2.

### Phase G — Validation & wrap-up

- **G1 — Integration/interop test.** Run the Rust validator against `Consensus`/`Coordinator` on Anvil:
  genesis DKG happy path + one signing round, ideally interoperating with a TS validator in the same
  group (the deliverable's "work together onchain" check). Depends on F3.
- **G2 — Docs & cleanup.** Update `README.md`/`AGENTS.md` for the completed validator crate; reconcile
  any widened workspace dependency. Depends on all implementation phases.
- **Cleanup.** Per the planning convention, **remove this epic file**
  (`epics/2026_07_01_rust_validator_port.md`) and the companion index
  (`epics/2026_06_09_rust_validator_port_index.md`) once the epic is complete.

### Critical path

`B1 → C1 → C4 → C5 → E3 → F1 → F3 → G1`. Phase C fans out from C1 (C2 also needs B1; C3 needs C1+C2;
C4 needs C1; C5 needs C1+C2+C4). D follows C; E follows B/C/D; F follows E. **Phase A is fully
independent** and only F2 depends on it, so it can land at any time. FROST (C) is a thin wrapper now,
so the state machine (E) shares the critical path with it.

---

## Open Questions and Assumptions

**Open questions**

1. **`frost-secp256k1` / `frost-core` version alignment.** The spike pinned `frost-secp256k1 = "3"`,
   `frost-core = { version = "3", features = ["internals"] }`, `k256 = "0.13"`, `rand = "0.8"`.
   **Recommended:** pin these **directly in `crates/validator/Cargo.toml`** (validator-only, not a
   workspace dependency) at those versions, confirm the `internals`-feature surface the wrappers touch
   is still exposed, and verify they interoperate with the deployed contracts via the Phase C interop
   vectors.
2. **`expires_at` for encoded transactions.** The core queue needs a per-tx expiry block; the TS action
   queue used a wall-clock timeout. **Recommended:** derive `expires_at` from the driving FSM deadline
   (keygen/signing/rollover), falling back to a config-driven block horizon for deadline-less actions
   (e.g. `SetValidatorStaker`). Confirm the mapping.
3. **Per-action gas limits.** `tx::Transaction.gas` is mandatory and the queue does not estimate it.
   **Recommended:** port the constants from `consensus/protocol/onchain.ts`; revisit with
   `provider.estimate_gas` if they prove brittle.
4. **Shared EIP-712 hashing with the sentinel.** The `SafeTx` / proposal `sol!` structs and hashing
   overlap the sentinel crate. **Recommended:** factor the shared structs into a small shared module
   (in `safenet-core` or a shared crate) rather than duplicating; decide when E4b lands.
5. **`callback` variants (`keyGenConfirmWithCallback` / `signShareWithCallback`).** The TS actions carry
   an optional callback context. Confirm whether the Rust port must support callbacks in the initial
   port or can defer them.

**Assumptions**

- The motivation is **a single shared Rust codebase** on `safenet-core` (and p2p access for the
  validator), not performance. The **hard requirement is onchain + peer compatibility**: byte-identical
  FROST commitments/proofs/nonces/signature-shares/attestations and event/calldata encoding, so a Rust
  and a TS validator interoperate in the same group and signing session. DB and config compatibility
  are **not** required.
- The **deterministic** DKG state (recomputable from replayed events) is **embedded in the reorg-aware
  snapshot `State`** (deviation #3), while the **locally-generated random secrets** — DKG coefficients
  + encryption key, and signing nonces — live in a **separate, reorg-immune SQLite store** (deviations
  #3 and #4), because a reorg could otherwise roll them back while the transaction that committed to
  them is re-included (stranding a keygen) or un-burn a nonce (leaking the signing share). This makes
  the `StateTransition` **impure**: keygen setup reuses an existing secret rather than resampling, a
  replay onto an already-burned nonce gracefully emits no action, and the store is pruned when a keygen
  resolves or a group retires.
- Service **initialization is a `Service` hook on the `Driver`** (Phase A): the validator reads
  `getValidatorStaker` at startup and may emit a `SetValidatorStaker` action.
- The port **reuses `safenet-core`** for indexing, state/reorg, tx submission, observability, the
  SQLite pool and the `Driver`; the validator supplies a `Service` (a `StateTransition` +
  `encode_actions` + `initialize`).
- FROST uses the **ZCash Foundation `frost-secp256k1` / `frost-core` crates** (standard RFC 9591
  FROST(secp256k1, SHA-256), the scheme Safenet already implements) — **not** a hand-rolled
  implementation. Only the Safenet-specific wrappers (address identifiers, ECDH share encryption,
  keccak merkle trees, nonce-tree preprocessing, solidity marshalling) are ported, modelled on the
  spike's `frost/*`; **parity vectors target those wrapping layers** plus one TS-interop DKG-and-sign
  round.
- Following the planning convention, this plan is proposed as a **docs-only PR** with no epic
  implementation code, and is removed on completion.
