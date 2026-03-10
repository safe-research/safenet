# Safenet Validator: Complete Learning Guide

This guide is designed to help you navigate, understand, and reason about every
part of the validator codebase. It covers the full development history, the
architecture, the runtime lifecycle of every subsystem, and the prerequisite
knowledge you need to read the code confidently.

---

## Table of Contents

1. [Prerequisite Knowledge](#1-prerequisite-knowledge)
2. [What the Validator Does in One Paragraph](#2-what-the-validator-does)
3. [Development History: Phase by Phase](#3-development-history)
4. [Repository and Folder Map](#4-repository-and-folder-map)
5. [Validator Architecture Deep-Dive](#5-validator-architecture)
6. [FROST Cryptography Primer](#6-frost-cryptography-primer)
7. [The State Machine: States and Transitions](#7-the-state-machine)
8. [The Watcher Layer: Blocks and Events](#8-the-watcher-layer)
9. [The Protocol Layer: Onchain Actions](#9-the-protocol-layer)
10. [Storage Layer: In-Memory vs SQLite](#10-the-storage-layer)
11. [Verification Engine: Packet Checking](#11-the-verification-engine)
12. [The Entry Point: validator.ts](#12-the-entry-point)
13. [Code Quality Standards Comparison](#13-code-quality-standards)
14. [Key Design Decisions and Why They Were Made](#14-key-design-decisions)
15. [Lifecycle: A Transaction from Proposal to Attestation](#15-end-to-end-lifecycle)
16. [Lifecycle: Epoch Rollover End-to-End](#16-epoch-rollover-end-to-end-lifecycle)
17. [Lifecycle: Nonce Chunk Pre-commitment](#17-nonce-chunk-pre-commitment-lifecycle)
18. [Lifecycle: Complaint Flow](#18-complaint-flow-lifecycle)
19. [Complete Environment Variable Reference](#19-complete-environment-variable-reference)
20. [Complete ProtocolAction Reference](#20-complete-protocolaction-reference)
21. [Watcher Inner Loop and Backoff](#21-watcher-inner-loop-and-backoff)
22. [Testing Guide](#22-testing-guide)

---

## 1. Prerequisite Knowledge

Before reading any validator code, you should be comfortable with the following.
Each section below tells you *why* the knowledge matters and what specifically to
learn.

### TypeScript (Strict Mode)

The validator is TypeScript 5.8 with `strict: true`. You need:

- **Discriminated unions**: The entire state machine is modelled as a union of
  object types where an `id` literal field acts as the tag. Every `RolloverState`
  or `SigningState` is one of many variants identified by `id`. TypeScript narrows
  the type for you inside a `switch (state.id)` block.
- **Generics and `Prettify`**: Several types use helper generics like `Prettify<T>`
  (from viem) to flatten intersections for better IDE hover output.
- **Private class fields (`#field`)**: The codebase uses the native JS private
  field syntax. These fields are truly private (not just TypeScript-private) and
  cannot be accessed from subclasses or outside the class.
- **`bigint`**: All block numbers, epoch numbers, scalars, and cryptographic
  values use native `bigint`. You will see `0n`, `1n`, arithmetic operators, and
  `BigInt(...)` casts everywhere. This is because the numbers involved exceed
  `Number.MAX_SAFE_INTEGER`.
- **`async`/`await` and event loops**: The watcher is an infinite async loop.
  Understanding how the Node.js event loop processes promises and I/O is critical
  to understanding why the watcher is structured as it is.

### Zod (Runtime Schema Validation)

Zod is used in two major places:

1. **Config parsing** (`validator/src/types/schemas.ts`): All environment
   variables are parsed and validated with Zod schemas at startup. If any are
   missing or malformed, the process exits with a descriptive error.
2. **Event parsing** (`validator/src/machine/transitions/schemas.ts`): Every
   EVM log received from the blockchain is parsed through a Zod schema before
   entering the state machine. This is the type-safety boundary between the
   "untyped blockchain world" and the "fully typed validator world."

Key Zod concepts used: `.refine()`, `.transform()`, `.extend()`,
`z.discriminatedUnion()`, `z.preprocess()`, `z.coerce.bigint()`.

### viem (Ethereum Client Library)

viem replaces ethers.js in this project. You need to understand:

- **`PublicClient`**: Read-only Ethereum client. Used for `getBlock`,
  `getLogs`, `readContract`.
- **`WalletClient`**: Write client. Used to sign and send transactions.
- **`AbiEvent`**: The ABI type for a single event. The event watcher takes
  `readonly AbiEvent[]`.
- **`Log`**: The raw log type returned by `getLogs`. The event watcher wraps this
  in its own `Log<E>` generic that ties logs to their parsed ABI.
- **Bloom filters**: EVM block headers contain a Bloom filter (`logsBloom`) that
  lets you skip `getLogs` calls for blocks that definitely do not contain logs
  for a given address or topic. The validator uses this aggressively.
- **`createPublicClient` / `createWalletClient`**: Factory functions for building
  Ethereum clients. The transport is either `http(url)` or `webSocket(url)`.

### better-sqlite3 (SQLite Bindings)

SQLite is used as persistent storage for validator state, action queues, and
chain-indexing progress. Key things to know:

- It is **synchronous** (no promises). All database calls block the thread.
  This is intentional: it makes transaction guarantees easier to reason about.
- **Prepared statements**: `db.prepare(sql)` returns a `Statement` you can call
  `.run()`, `.get()`, or `.all()` on.
- **`ON CONFLICT` upsert**: Used extensively for idempotent writes (e.g., updating
  `lastIndexedBlock` only if the new value is greater than the stored one).
- **WAL mode**: Not explicitly set in the code seen, but recommended for
  concurrent read performance.

### secp256k1 Elliptic Curve Cryptography

The FROST algorithm works on the `secp256k1` curve (the same curve used by
Ethereum and Bitcoin). You need a mental model of:

- **Point**: A coordinate `(x, y)` on the curve. Adding two points produces
  another point. Multiplying a point by a scalar is like repeated addition.
- **Scalar**: An integer mod the curve order `N`. This represents private keys
  and secret shares.
- **Generator `G`**: The base point. A private key `k` maps to public key `k*G`.
- **ECDH**: If Alice has private key `a` and Bob has public key `B = b*G`, then
  `a*B = a*b*G = b*A`. Both compute the same shared secret. Used in FROST
  DKG to encrypt secret shares between participants.

The validator uses the `@noble/curves` library for all EC math
(`validator/src/frost/math.ts`). Noble curves is audited, pure-JS, and
constant-time where it matters.

### Merkle Trees

Used in two places:

1. **Nonce commitments**: Instead of posting each nonce pair on-chain individually
   (expensive), validators post a Merkle root for a *chunk* of 1024 nonce pairs.
   During signing, the specific nonce pair for the ceremony is revealed with a
   Merkle proof.
2. **Signature share selection root**: Defines which participants are included in
   a signing ceremony and what their nonce commitments are.

You need to understand: leaves, parent hashing, root, and inclusion proofs.

### Schnorr Signatures

FROST produces Schnorr signatures, not ECDSA. A Schnorr signature `(R, z)` is
verified by checking `z*G == R + c*Y` where `c` is the hash of `(R, Y, msg)`.
Schnorr is linear: partial signatures from different participants can be summed
into a group signature. This linearity is the foundation of FROST.

---

## 2. What the Validator Does

The validator is a long-running Node.js process that:

1. **Watches the blockchain** for events emitted by the `FROSTCoordinator` and
   `Consensus` smart contracts.
2. **Feeds those events** into a state machine that tracks the current phase of
   two concurrent protocols: *epoch rollover (KeyGen)* and *transaction signing*.
3. **Reacts** to state changes by queuing and submitting Ethereum transactions
   (called "actions") to participate in the FROST cryptographic protocol.
4. **Verifies** every signing request before participating, to ensure it attests
   only to valid Safe transactions or valid epoch rollovers.

---

## 3. Development History

The commit history shows six clear development phases.

### Phase 0 – Bootstrapping (commits 1–3, ~Aug 2024)

**Commits**: `Bootstrap Contracts`, `Started Bootstrapping some Contracts`,
`rename to ShiedNet`

Started as "ShieldNet" (later renamed to Safenet). The first three commits
establish Foundry for Solidity, a `Counter.sol` placeholder, and the
`secp256k1` math library. There is no validator yet.

**Why it matters**: Shows the project started from the contracts side, not the
validator side. The cryptographic primitives were established before the
off-chain coordinator.

---

### Phase 1 – Validator Scaffolding (PRs #1–6, ~Sept 2024)

**Commits**: `Validator setup`, `Simple event listener`, `Use zod for config`,
`Validator CI setup`, `Add docker setup`, `Add simple test for schema`

**PR #1 – Validator setup**: Creates the npm monorepo workspace structure,
adds `biome.json` for linting, a dev container, and a minimal
`validator/src/validator.ts` entry point (just a skeleton at this point).

**PR #2 – Simple event listener**: Adds the first real logic — a `service.ts`
that connects to viem and listens for contract events. This is the ancestor of
the entire watcher layer.

**PR #3 – Zod for config validation**: Introduces `schemas.ts` with the first
Zod schema for parsing environment variables. The pattern of "validate all
external input at the boundary" is established here and never abandoned.

**PR #4 – Validator CI**: Adds a GitHub Actions workflow for the validator.
Biome linting is enforced in CI from this commit onwards.

**PR #5 – Docker setup**: Adds the `Dockerfile` using a multi-stage build.
Establishes that the validator is distributed as a container image.

**PR #6 – Schema tests**: Adds Vitest and the first unit tests for the Zod
schemas. Establishes the testing pattern early.

**Why these matter**: The scaffolding phase locked in three non-negotiable
architectural principles that pervade every later commit:
1. All external input (env vars, blockchain events) passes through Zod.
2. The validator runs in a container.
3. Linting and tests run in CI.

---

### Phase 2 – FROST Contracts + Validator Core (PRs #7–26, ~Oct–Nov 2024)

This is the longest and most complex phase. It builds the full FROST protocol
both onchain (Solidity) and off-chain (TypeScript).

**PR #7 – Staking v0.1**: The `Staking.sol` contract appears. Not directly
relevant to the validator internals but establishes the economic layer.

**PR #8 – Onchain FROST KeyGen**: The `FROSTCoordinator.sol` contract gets its
first real implementation. Onchain storage for FROST groups, participant maps,
and Merkle maps for nonce commitments. The `Secp256k1.sol` library handles
on-chain EC math.

**PR #13 – Implement KeyGen flow in validator**: The first major validator logic.
Adds:
- `validator/src/frost/math.ts`: The off-chain secp256k1 math wrapper (using
  `@noble/curves`).
- `validator/src/consensus/client.ts`: The first "consensus client" that
  orchestrates the KeyGen ceremony by calling the coordinator contract.
- `validator/src/consensus/merkle.ts`: Merkle tree implementation used for
  nonce commitment chunks.

**PR #16 – FROST Signing Ceremony**: Onchain signing support is added to the
coordinator contract. A `FROST.sol` library computes challenges and verifies
signature shares on-chain.

**PR #17 – FROST Algorithm Tests**: Test vectors from the reference FROST Rust
crate are added. This PR *found real bugs*: wrong nonce generation (using public
key instead of secret scalar), a DST offset bug in hashing. The lesson: always
validate cryptographic implementations against reference vectors.

**PR #24 – Validator Signing Flow**: The storage abstraction is introduced as a
separate concern. The validator is refactored to separate: client (ceremony
logic) → storage (state) → coordinator (contract calls). The architecture
diagram in the PR description shows three clear layers.

**PR #26 – Trustless KeyGen**: A major design change. The FROST group ID is now
deterministically computed from the group parameters (participants + context),
not assigned by the coordinator. This means:
- Any validator can start a KeyGen ceremony without waiting for coordination.
- If a validator misbehaves, others can restart KeyGen with the offender excluded
  and compute the same deterministic group ID for the new group.
- The epoch rotation ceremony is now permissionless and DoS-resistant.

**PR #30 – Verification Engine scaffolding**: The `VerificationEngine` appears.
This is the pluggable checker system that validates what the validator is being
asked to sign before it participates.

---

### Phase 3 – State Machine (PRs #39–56, ~Nov–Dec 2024)

**PR #39 – Implement State Machine**: The most architecturally significant PR.
Before this, the validator was a loosely connected set of event handlers. After
this, there is a single `SafenetStateMachine` class that owns all validator
state and reacts to typed `StateTransition` events. The key insight: a
blockchain event is just a *trigger*; what the validator actually does depends
entirely on what *state it is currently in*.

**PR #40 – Actions queue**: Instead of calling protocol functions directly
inside state transitions (which could fail, causing state inconsistency), state
transitions now return a list of `ProtocolAction` objects. These go into a queue
and are executed asynchronously, with retry logic. This is the "actions pattern":
separate the decision of what to do from the execution of doing it.

**PR #41 – Initial service implementation**: Wires the state machine into the
`ValidatorService` class. Environment variable config is extended to include all
required parameters.

**PR #44 – Handle error states**: Error and timeout handling is added. A signing
ceremony that receives no nonce commitments by the deadline will transition to a
failure state and restart. This makes the validator resilient to intermittent
failures.

**PR #45–48 – SQLite storage**: The in-memory storage is kept for testing, but a
persistent SQLite storage adapter is built. State is preserved across restarts.
Without this, a validator restart during an epoch would leave it unable to
produce signatures.

**PR #49 – State machine refactoring**: Each state transition is extracted into
its own file under `machine/keygen/`, `machine/signing/`, and
`machine/consensus/`. The `TransitionState` local-state class tracks pending
changes before they are committed. This is the final structure of the state
machine.

**PR #56 – Persist state machine**: The `ConsensusState`, `RolloverState`, and
all `SigningState` records are persisted in separate SQLite tables using JSON
columns. SQLite's native JSON support allows the state to be stored without
designing a complex normalized schema for every possible state shape.

---

### Phase 4 – Reliability and Production Hardening (PRs #62–103, ~Jan–Feb 2025)

**PR #62 – Transition watcher with persisted state**: The block indexing
progress (`lastIndexedBlock`) is persisted in SQLite. On restart, the watcher
replays events from the last safe block instead of from the chain tip, preventing
missed events.

**PR #63 – Wait for manual KeyGen for genesis**: Adds the `SKIP_GENESIS` flag.
Operators can skip the genesis KeyGen (useful when joining a running network).

**PR #74 – Logging framework**: Replaces `console.log` with a structured logger
(`pino`-style) that supports log levels and outputs JSON for log aggregators.

**PR #83 – Simplify last participant handling**: The "responsible participant"
concept (the last participant to act must drive the ceremony forward in case of
failure) is simplified and correctly implemented.

**PR #84 – Prometheus metrics**: A metrics endpoint on `:3555` exposes
operational data (RPC call counts, action queue depth, signing ceremony latency)
for monitoring.

**PR #92 – Handle SIGTERM**: Adds graceful shutdown for SIGINT and SIGTERM,
allowing the container to stop cleanly.

**PR #97–99 – Complaint flow**: The FROST DKG complaint protocol is implemented.
If a validator receives an invalid secret share, it files a complaint on-chain
and the accused validator must respond by revealing the secret. If they don't,
or if too many complaints are filed, the group is marked compromised and KeyGen
restarts without the offender.

**PR #121 – Block Indexer**: The `BlockWatcher` class is separated from the
event watcher. The block watcher is responsible for producing a reliable stream
of canonical blocks (including reorg detection). The event watcher consumes the
block stream.

**PR #137 – Event Indexer**: The `EventWatcher` class is similarly separated and
made robust: it checks the Bloom filter before making `getLogs` calls, handles
fallible events, and uses a paginated "warp" strategy for catching up on many
blocks at once.

**PR #140 – Exponential backoff**: A backoff decorator is wrapped around the
watcher loop to prevent hammering the RPC node on failures.

---

### Phase 5 – Polish and Audit Prep (PRs #104–241, ~Mar–Aug 2025)

- **PR #107 – Shieldnet Explorer**: A separate React frontend workspace is added.
- **PR #111–112 – Certora formal verification**: Setup for the Certora prover
  to formally verify Solidity contract properties.
- **PR #166 – Use Safe Transaction Hash**: The message being signed changes to
  use the actual Safe transaction hash, making the attestation directly verifiable
  by a Safe guard.
- **PR #175 – Shieldnet → Safenet rename**: The project is officially renamed.
- **PR #199 – Dedicated Encryption Key**: DKG secret share encryption is improved
  to use a dedicated key.
- **PR #200 – Validator Staker functionality**: The validator can now set its own
  staker address via `consensus_set_validator_staker`.
- **PR #234 – Configurable block range for consensus queries**: `BLOCK_PAGE_SIZE`
  and related config added to handle RPC nodes with log query limits.
- **PR #237 – Dynamic coordinator address resolution**: The coordinator address
  can be resolved dynamically from the consensus contract instead of being
  hardcoded in config.
- **PR #241 – Remove multicall**: Multicall usage removed as it wasn't configured;
  validator labels improved.

---

## 4. Repository and Folder Map

```
shield-net/
├── contracts/          Foundry Solidity project
│   └── src/
│       ├── FROSTCoordinator.sol   Manages FROST KeyGen and Signing ceremonies
│       ├── Consensus.sol          Epoch management, transaction attestation requests
│       └── Staking.sol            Validator registration and staking
├── validator/          Node.js TypeScript validator service
│   └── src/
│       ├── validator.ts           Entry point: parse config, create service, start
│       ├── service/
│       │   ├── service.ts         ValidatorService: wires everything together
│       │   ├── machine.ts         SafenetStateMachine: the state machine
│       │   └── checks.ts          buildSafeTransactionCheck(): chain of security checks
│       ├── watcher/
│       │   ├── blocks.ts          BlockWatcher: produces canonical block stream
│       │   ├── events.ts          EventWatcher: filters and paginates getLogs
│       │   ├── backoff.ts         Exponential backoff decorator
│       │   └── index.ts           watchBlocksAndEvents(): combines both watchers
│       ├── machine/
│       │   ├── types.ts           All state types (RolloverState, SigningState, etc.)
│       │   ├── transitions/
│       │   │   ├── schemas.ts     Zod schemas for all StateTransition variants
│       │   │   ├── onchain.ts     logToTransition(): converts viem Logs to transitions
│       │   │   ├── watcher.ts     OnchainTransitionWatcher: drives the watcher loop
│       │   │   └── queue.ts       Transition queue management
│       │   ├── state/
│       │   │   ├── local.ts       TransitionState: accumulates diffs during transition
│       │   │   └── diff.ts        applyDiff(): applies StateDiff to storage
│       │   ├── storage/
│       │   │   ├── types.ts       StateStorage interface
│       │   │   ├── inmemory.ts    InMemoryStateStorage
│       │   │   └── sqlite.ts      SqliteStateStorage
│       │   ├── keygen/            One file per KeyGen state transition handler
│       │   ├── signing/           One file per Signing state transition handler
│       │   └── consensus/         One file per Consensus event handler
│       ├── consensus/
│       │   ├── keyGen/
│       │   │   └── client.ts      KeyGenClient: FROST DKG ceremony logic
│       │   ├── signing/
│       │   │   └── client.ts      SigningClient: FROST signing ceremony logic
│       │   ├── protocol/
│       │   │   ├── types.ts       SafenetProtocol interface, ProtocolAction types
│       │   │   ├── onchain.ts     OnchainProtocol: submits actions as EVM transactions
│       │   │   └── sqlite.ts      SqliteActionQueue, SqliteTxStorage
│       │   ├── storage/
│       │   │   ├── types.ts       ClientStorage interface (nonces, shares, groups)
│       │   │   ├── inmemory.ts    InMemoryClientStorage
│       │   │   └── sqlite.ts      SqliteClientStorage
│       │   ├── verify/
│       │   │   ├── engine.ts      VerificationEngine: routes packets to handlers
│       │   │   ├── safeTx/        SafeTransactionHandler and checks
│       │   │   └── rollover/      EpochRolloverHandler
│       │   └── merkle.ts          Merkle tree for nonce chunk commitments
│       ├── frost/
│       │   ├── math.ts            secp256k1 scalar/point operations (wraps @noble/curves)
│       │   ├── secret.ts          Secret coefficient generation
│       │   ├── hashes.ts          FROST-specific hash functions (H1..H5)
│       │   └── types.ts           FrostPoint, GroupId, ParticipantId, SignatureId
│       ├── types/
│       │   ├── abis.ts            ABI definitions for all contract events and functions
│       │   ├── chains.ts          Supported chain definitions (Gnosis, etc.)
│       │   ├── interfaces.ts      ProtocolConfig interface
│       │   └── schemas.ts         Zod schemas for env vars and common types
│       └── utils/
│           ├── config.ts          withDefaults() helper
│           ├── logging.ts         createLogger() (structured JSON logger)
│           ├── metrics/           Prometheus metrics setup
│           ├── bloom.ts           Bloom filter membership check
│           ├── errors.ts          formatError() helper
│           ├── json.ts            BigInt-aware JSON replacer/reviver
│           ├── math.ts            maxBigInt() and similar bigint helpers
│           └── queue.ts           InMemoryQueue<T>
└── explorer/           React 19 + Vite frontend (separate concern)
```

---

## 5. Validator Architecture

The validator has a clean layered architecture where data flows in one direction:

```
Blockchain (RPC Node)
        |
        v
  [BlockWatcher]          -- produces: canonical block stream
        |
        v
  [EventWatcher]          -- produces: filtered, sorted EVM logs
        |
        v
  [OnchainTransitionWatcher]  -- converts logs to StateTransition events
        |                      -- persists lastIndexedBlock in SQLite
        v
  [SafenetStateMachine]   -- dispatches transitions to handlers
        |                  -- accumulates StateDiff via TransitionState
        |                  -- applies diffs to StateStorage
        v
  [StateStorage]          -- persists RolloverState, SigningState, ConsensusState
        |
        v
  [ProtocolAction queue]  -- actions scheduled by state machine
        |
        v
  [OnchainProtocol]       -- submits actions as EVM transactions
        |
        v
Blockchain (via WalletClient)
```

### Key Architectural Principles

**1. Separation of concerns at every layer**

The watcher knows nothing about FROST. The state machine knows nothing about
SQLite. The storage layer knows nothing about the blockchain. Each layer has a
well-defined interface.

**2. All external input passes through Zod**

Every event that enters the state machine is parsed with a Zod
`discriminatedUnion` schema. If an event doesn't match the schema (e.g., a
contract upgrade changes an event signature), the validator logs an error and
continues rather than crashing or entering a corrupt state.

**3. The state machine is pure and synchronous**

The `SafenetStateMachine.transition()` method is synchronous. It applies one
transition at a time. If a second transition arrives while one is being processed,
it is queued (via `#transitionQueue`). This means you never have to reason about
concurrent mutations of the state.

**4. Actions are separate from decisions**

State transitions never *directly* submit transactions. They return
`ProtocolAction[]` objects which are queued and executed by `OnchainProtocol`.
If a transaction submission fails, the action is retried without re-running the
state transition logic.

**5. Dual storage backends**

Every storage concern has two implementations: `InMemory*` for tests and
`Sqlite*` for production. The interfaces are identical, so integration tests run
against the real SQLite implementation by setting `SAFENET_TEST_STORAGE=sqlite`.

---

## 6. FROST Cryptography Primer

Understanding FROST requires three sub-topics: Distributed Key Generation
(DKG), Signing, and how Safenet adapts both for onchain communication.

### Distributed Key Generation (DKG)

Goal: N participants collectively generate a shared group key `Y` and individual
signing shares `s_i`, such that no individual knows the full private key.

**Round 1 – Commit**: Each participant `i` generates a random secret polynomial
`f_i(x)` of degree `t-1`. They commit to it by publishing `C_i = [f_i(0)*G,
f_i(1)*G, ...]`. The sum of all `C_j[0]` values gives the group public key `Y`.

**Round 2 – Secret Share**: Each participant sends participant `j` the value
`f_i(j)` (evaluated at `j`'s identifier). This is encrypted with ECDH using
`C_i[0]` as the encryption key. The signing share for `j` is `s_j = sum of all
f_i(j) over all i`.

**Round 3 – Confirm**: Each participant verifies their share using the public
commitments and then publishes a confirmation. Only after all confirmations is
the group key considered finalized.

**In the validator code**: `KeyGenClient` in `consensus/keyGen/client.ts`
orchestrates all three rounds. `frost/math.ts` provides `evalPoly()` (evaluates
`f_i(j)`), `evalCommitment()` (verifies using public commitments), and
`createSigningShare()` (sums all received shares).

### Signing

Goal: A threshold `t` of participants produce a Schnorr signature `(R, z)` for
group key `Y` without any one participant knowing the full private key.

**Pre-processing**: Each participant pre-generates nonce pairs `(d, e)` and
commits to them in a Merkle tree. These chunks of 1024 nonces are posted
on-chain as a Merkle root.

**Round 1 – Commit nonces**: For a specific signing ceremony, each participant
reveals the specific `(D, E) = (d*G, e*G)` nonce pair for this ceremony along
with a Merkle proof.

**Round 2 – Share**: Each participant computes and publishes their signature
share `z_i = d_i + rho_i * e_i + lambda_i * s_i * c` where `rho_i` is a
binding factor and `lambda_i` is a Lagrange coefficient.

**Aggregation**: The coordinator contract sums all `z_i` to get `z`, and
computes `R = sum of (D_i + rho_i * E_i)`. The final signature is `(R, z)`.

**In the validator code**: `SigningClient` in `consensus/signing/client.ts`
handles both rounds. `consensus/signing/nonces.ts` manages nonce storage.
`consensus/signing/shares.ts` computes signature shares.

### Safenet-Specific Adaptations

| Standard FROST | Safenet FROST |
|---|---|
| Private communication channels | All communication is on-chain (public) |
| Nonces chosen per ceremony | Nonces pre-committed in chunks of 1024 |
| Signing starts when message is known | Nonces committed before message known |
| No specific ordering | Blockchain provides absolute message ordering |
| N/A | Complaint flow for invalid DKG shares |
| N/A | Third confirmation round for on-chain readiness |

---

## 7. The State Machine

The state machine (`service/machine.ts`, `SafenetStateMachine`) is the brain of
the validator. It maintains two concurrent state machines:

### Rollover State Machine

Tracks the KeyGen ceremony for the next epoch. The state is a `RolloverState`
discriminated union (defined in `machine/types.ts`):

```
waiting_for_genesis
    |
    | [first KeyGen event seen]
    v
collecting_commitments   <--- timeout --> epoch_skipped
    |
    | [all commitments received]
    v
collecting_shares        <--- compromised --> collecting_commitments (restart)
    |                                         (with fewer participants)
    | [all shares received]
    v
collecting_confirmations <--- timeout/compromised --> collecting_commitments
    |
    | [all confirmations received]
    v
sign_rollover            <--- EpochProposed event triggers signing
    |
    | [EpochStaged event received]
    v
epoch_staged
```

### Signing State Machine

One instance exists per pending transaction. The state is a `SigningState`
discriminated union:

```
waiting_for_request      (TransactionProposed event received)
    |
    | [Sign event from coordinator]
    v
collect_nonce_commitments  <--- timeout --> failure --> restart or skip
    |
    | [all nonce commitments received]
    v
collect_signing_shares     <--- timeout --> failure --> restart or skip
    |
    | [SignCompleted event from coordinator]
    v
waiting_for_attestation    <--- timeout --> failure
    |
    | [TransactionAttested event]
    v
[done / cleaned up]
```

### How Transitions Work

1. `OnchainTransitionWatcher` calls `stateMachine.transition(t)` for each
   `StateTransition`.
2. The `transition()` method enqueues the transition and processes the queue
   synchronously (one at a time).
3. For each transition, a `TransitionState` accumulates a `StateDiff` (what
   changed).
4. The relevant handler function (e.g., `handleKeyGenCommitted`) is called with
   the current state and the transition. It returns a `StateDiff`.
5. The diff is applied to `StateStorage` (SQLite or in-memory).
6. Any `ProtocolAction[]` in the diff are added to the action queue.
7. `OnchainProtocol` picks up actions from the queue and submits them on-chain.

### Handler File Map

| Event | Handler file |
|---|---|
| `event_key_gen` | `machine/keygen/genesis.ts` |
| `event_key_gen_committed` | `machine/keygen/committed.ts` |
| `event_key_gen_secret_shared` | `machine/keygen/secretShares.ts` |
| `event_key_gen_confirmed` | `machine/keygen/confirmed.ts` |
| `event_key_gen_complaint_submitted` | `machine/keygen/complaintSubmitted.ts` |
| `event_key_gen_complaint_responded` | `machine/keygen/complaintResponse.ts` |
| `event_sign_request` | `machine/signing/sign.ts` |
| `event_nonce_commitments_hash` | `machine/signing/preprocess.ts` |
| `event_nonce_commitments` | `machine/signing/nonces.ts` |
| `event_signature_share` | `machine/signing/shares.ts` |
| `event_signed` | `machine/signing/completed.ts` |
| `event_epoch_proposed` | `machine/consensus/rollover.ts` |
| `event_epoch_staged` | `machine/consensus/epochStaged.ts` |
| `event_transaction_proposed` | `machine/consensus/transactionProposed.ts` |
| `event_transaction_attested` | `machine/consensus/transactionAttested.ts` |
| `block_new` | Timeout checks in `machine/keygen/timeouts.ts` and `machine/signing/timeouts.ts` |

### `TransitionState` and the Proxy Pattern

This is the most architecturally subtle part of the state machine and is worth
understanding in full detail.

**The problem**: A single incoming transition (e.g., `event_key_gen_committed`)
may call multiple handler functions in sequence, each of which needs to *read*
the state as it would look *after the previous handler already modified it*.
But we cannot commit the changes to `StateStorage` mid-transition — if any
later handler fails, we'd have a partially-applied state in persistent storage.

**The solution**: a `TransitionState` instance acts as a copy-on-write view of
the committed state. It is created fresh for each incoming transition and thrown
away once the transition completes (successfully or not).

`TransitionState` in [machine/state/local.ts](validator/src/machine/state/local.ts)
holds two inner objects:

- `machines: LocalMachineStates` — a copy-on-write view of the rollover and
  signing states.
- `consensus: LocalConsensusStates` — a copy-on-write view of the consensus
  state.

Each of these uses a JavaScript `Proxy` for the record-shaped fields (like
`signing` and `epochGroups`). The proxy has a private `temp` buffer alongside
the original `source`:

```
get(key)  → if key in temp  → return temp[key]   (local override)
           → else           → return source[key]  (fall through to committed state)

set(key)  → temp[key] = value  (write to buffer only, never touch source)

delete(key) → temp[key] = undefined  (mark as deleted in buffer)
```

This means:
1. **Reads see the in-progress changes.** If handler A sets `signing["0xabc"]` to
   a new state, handler B running next will read that new state, even though
   nothing has been written to SQLite yet.
2. **The committed state is never mutated during a transition.** `source` is
   the `Readonly<>` snapshot from `StateStorage`.
3. **`apply(diff)`** calls `applyMachines` / `applyConsensus` from
   [machine/state/diff.ts](validator/src/machine/state/diff.ts) against the
   local objects, updating the temp buffer.
4. **`diffs: StateDiff[]`** accumulates every diff applied during the transition.
   At the end, all diffs in this array are flushed to `StateStorage` in one
   atomic SQLite transaction.

**What this means for a developer reading a handler**: every handler function
receives `machineStates` and `consensusState` arguments that are actually
`TransitionState.machines` and `TransitionState.consensus` — the live
copy-on-write views, not the raw committed state. This is why handlers can chain
effects: each one calls `transitionState.apply(diff)` and subsequent handlers
immediately see the result.

```
event arrives
  → TransitionState created (wraps committed state via Proxy)
  → handler A called (reads/writes via TransitionState)
      → transitionState.apply(diffA)       // temp buffer updated
  → handler B called (reads updated state)
      → transitionState.apply(diffB)
  → all diffs flushed atomically to StateStorage (SQLite transaction)
  → TransitionState discarded
```

If any handler throws, no diffs reach `StateStorage` — the committed state is
unchanged.

---

## 8. The Watcher Layer

### BlockWatcher (`watcher/blocks.ts`)

Responsibility: produce a reliable, in-order stream of `BlockUpdate` events.

**The three update types**:
- `watcher_update_new_block`: A new canonical block was produced. Contains the
  block number, hash, and `logsBloom`.
- `watcher_update_uncle_block`: A previously seen block was reorged. The watcher
  detected that the parent hash of a new block doesn't match the previous block's
  hash, so it "uncles" the previous block and re-queues it.
- `watcher_update_warp_to_block`: On startup (when catching up), instead of
  emitting each block individually, the watcher emits a single "warp" event
  covering the block range. The event watcher uses this to make paginated
  `getLogs` queries across the range.

**Reorg detection**: The watcher keeps the last `maxReorgDepth` (default 5)
blocks in memory. When a new block arrives, it checks that `newBlock.parentHash
=== lastBlock.hash`. If not, a reorg has occurred.

**Startup behaviour**: On startup with a `lastIndexedBlock`, the watcher:
1. Emits an `uncle_block` for `lastIndexedBlock - (maxReorgDepth - 1)` to
   re-index recent blocks (safe against shallow reorgs).
2. Emits a `warp_to_block` from the uncle point to the safe point (`latestBlock
   - maxReorgDepth`).
3. Emits individual `new_block` updates for the recent blocks within reorg depth.

This three-step startup ensures the validator never misses events across restarts.

**Block timing**: The watcher knows the expected block time (e.g., 5 seconds on
Gnosis Chain). It waits until `lastBlockTimestamp + blockTime +
blockPropagationDelay` before querying for the next block. This avoids hammering
the RPC with premature requests.

### EventWatcher (`watcher/events.ts`)

Responsibility: given a block stream, produce filtered and sorted EVM logs.

**Bloom filter optimization**: Before calling `getLogs` for a block, the watcher
checks the `logsBloom` header field. If neither the contract addresses nor the
event topic signatures appear in the bloom filter, the block definitely contains
no relevant logs and the RPC call is skipped. This drastically reduces RPC usage
on quiet chains.

**Warp mode (catch-up)**: When warping, the watcher queries logs across a page
of blocks (default 100 blocks at a time) with a single `getLogs` call. If the
RPC returns an error (often because the range is too large), the page size is
halved and retried. If it reaches page size 1, it falls back to querying
event-by-event.

**Fallible events**: The `TransactionProposed` event is marked fallible. This
means if a `getLogs` call for that event fails (e.g., RPC content-length limit),
the validator logs a warning instead of crashing. Other events are critical and
their failure is propagated as an error.

### OnchainTransitionWatcher (`machine/transitions/watcher.ts`)

Responsibility: bridge the watcher layer and the state machine.

It calls `watchBlocksAndEvents()` and converts updates into `StateTransition`
objects via `logToTransition()` (`machine/transitions/onchain.ts`). Each
transition is stamped with the block number. The `lastIndexedBlock` is persisted
in SQLite so that on restart the watcher starts from where it left off.

---

## 9. The Protocol Layer

### SafenetProtocol Interface (`consensus/protocol/types.ts`)

Defines all the onchain operations the state machine can request:
- KeyGen: `frost_key_gen_and_commit`, `frost_key_gen_share`, `frost_key_gen_confirm`
- Complaint: `frost_key_gen_complaint`, `frost_key_gen_complaint_response`
- Nonces: `frost_nonce_commitments`
- Signing: `frost_sign`, `frost_signature_share`
- Consensus: `consensus_propose_epoch`, `consensus_attest_transaction`,
  `consensus_request_sign`, `consensus_set_validator_staker`

### OnchainProtocol (`consensus/protocol/onchain.ts`)

The production implementation of `SafenetProtocol`. Each action is:
1. Added to the `SqliteActionQueue` with a timeout.
2. When `checkPendingActions()` is called (on each new block), overdue and
   pending actions are picked up and submitted via `walletClient.writeContract`.
3. If the transaction fails with a "nonce too low" error, the nonce is
   incremented and retried.
4. If the transaction is successfully mined, the action is removed from the
   queue.
5. A global exponential backoff is applied to action submissions to handle
   RPC rate limits gracefully.

### GasFeeEstimator

Estimates gas fees for transactions using EIP-1559. On each new block,
`gasFeeEstimator.invalidate()` is called to flush the cached fee estimate so
the next action submission fetches fresh fees.

---

## 10. The Storage Layer

### ClientStorage (`consensus/storage/types.ts`)

Stores FROST ceremony state:
- **Groups**: FROST group metadata (participants, threshold, keys)
- **KeyGen**: Secret coefficients, received shares, confirmations
- **Nonces**: Pre-computed nonce pairs and the Merkle tree for their chunks
- **Signatures**: Signature requests, received nonce commitments, shares

### StateStorage (`machine/storage/types.ts`)

A minimal interface with just two methods:
- `applyDiff(diff: StateDiff): ProtocolAction[]`: Applies a state diff
  atomically. Returns any protocol actions that were in the diff.
- `consensusState(): ConsensusState`: Returns the current consensus state.
- `machineStates(): MachineStates`: Returns the current rollover and signing
  states.

### SQLite Schema

Key tables:
- `transition_watcher`: Stores `(chainId, lastIndexedBlock)` for watcher restart.
- `consensus_state`: JSON blob of the consensus state.
- `rollover_state`: JSON blob of the current rollover state.
- `signing_states`: One row per pending transaction hash, JSON blob of signing state.
- `action_queue`: Pending protocol actions with timeout and retry metadata.
- `tx_storage`: Pending Ethereum transaction submissions.
- Groups, KeyGen, nonces, signatures tables in `SqliteClientStorage`.

---

## 11. The Verification Engine

Before participating in a signing ceremony, the validator must verify that it
agrees with what it is being asked to sign.

### VerificationEngine (`consensus/verify/engine.ts`)

Routes incoming packets to the correct handler based on packet type string:
- `"safe_transaction_packet"` → `SafeTransactionHandler`
- `"epoch_rollover_packet"` → `EpochRolloverHandler`

### SafeTransactionHandler (`consensus/verify/safeTx/handler.ts`)

Runs a chain of deterministic checks on the Safe transaction:
1. **Basic checks**: Is the chain ID valid? Is the epoch correct?
2. **Singleton check**: Is the Safe using an approved singleton (implementation)?
3. **Guard check**: Is the Safe using an approved guard?
4. **Fallback handler check**: Is the Safe using an approved fallback handler?
5. **Module check**: Are the Safe's modules all approved?
6. **MultiSend check**: If the transaction is a MultiSend, are all batched calls
   valid?

All checks are deterministic: every honest validator, given the same transaction,
will reach the same pass/fail decision. This is the fundamental requirement for
a BFT network to agree.

### EpochRolloverHandler (`consensus/verify/rollover/handler.ts`)

Verifies that the epoch rollover being proposed is consistent with the validator's
own view of the new epoch (same group key, same participants, same epoch number).

### `buildSafeTransactionCheck()` — The Security Policy in Detail

[service/checks.ts](validator/src/service/checks.ts) constructs a
chain-of-responsibility tree using combinator functions. The tree is built
bottom-up and evaluated top-down on every incoming transaction.

The top-level check evaluates which `to` address the transaction targets, then
routes it through the appropriate sub-check:

```
incoming transaction
  ↓
buildAddressSplitCheck({
  "0xA83c..."  → buildMultiSendCallOnlyCheck(baseChecks)    (MultiSend 1.3.0)
  "0x9641..."  → buildMultiSendCallOnlyCheck(baseChecks)    (MultiSend 1.4.1)
  "0x40A2..."  → buildMultiSendCallOnlyCheck(baseChecks)    (MultiSend 1.5.0)
  fallback     → allowedDelegateCalls
})
  ↓ (if not MultiSend)
buildAddressSplitCheck({
  "0x6439..."  → buildSingletonUpgradeChecks()  (Safe Migration 1.x)
  "0x5266..."  → buildSingletonUpgradeChecks()  (Safe Migration 2.x)
  fallback     → baseChecks
})
  ↓ (if not MultiSend or singleton migration)
baseChecks = buildCombinedChecks([
  selfChecks,               (if to == Safe itself)
  buildNoDelegateCallCheck()
])
```

#### Self-call checks (`to == Safe address`)

When the Safe calls itself (`operation = 0, to = safe`), only these function
selectors are permitted:

| Function | Purpose |
|---|---|
| `setFallbackHandler(address)` | Only to an approved handler address |
| `setGuard(address)` | Only to `address(0)` (currently no guards allowed) |
| `setModuleGuard(address)` | Only to `address(0)` |
| `enableModule(address)` | Only approved modules (currently none) |
| `disableModule(address, address)` | Always allowed |
| `addOwnerWithThreshold(address, uint256)` | Always allowed |
| `removeOwner(address, address, uint256)` | Always allowed |
| `swapOwner(address, address, address)` | Always allowed |
| `changeThreshold(uint256)` | Always allowed |
| empty calldata | Always allowed (pure ETH transfer to self) |

Any other function selector on the Safe itself is rejected with
`"invalid_self_call"`.

#### Approved fallback handlers (as of current codebase)

| Address | Contract |
|---|---|
| `0x85a8...` | ExtensibleFallbackHandler 1.5.0 |
| `0x2f55...` | ExtensibleFallbackHandler (CoW) |
| `0x3EfC...` | CompatibilityFallbackHandler 1.5.0 |
| `0xfd07...` | CompatibilityFallbackHandler 1.4.1 |
| `0xf48f...` | CompatibilityFallbackHandler 1.3.0 canonical |
| `0x017062...` | CompatibilityFallbackHandler 1.3.0 EIP-155 |
| `address(0)` | Removing the fallback handler |

#### Currently empty allowlists

- **Guards** (`setGuard`): no guards are currently allowed. A transaction
  attempting `setGuard(nonZeroAddress)` is rejected.
- **Modules** (`enableModule`): no modules are currently allowed.
- **Module guards** (`setModuleGuard`): no module guards are currently allowed.

#### MultiSend checks

The three hardcoded MultiSend addresses (1.3.0, 1.4.1, 1.5.0) are allowed only
with `operation = 1` (delegatecall). Each inner call is decoded and recursively
checked against `baseChecks`. A MultiSend that includes any disallowed inner
call causes the whole transaction to be rejected.

#### Singleton migration (delegatecall)

Delegatecall to the two Safe migration contracts is allowed only for:
- `migrateSingleton()`
- `migrateWithFallbackHandler()`
- `migrateL2Singleton()`
- `migrateL2WithFallbackHandler()`

Any other selector on those addresses is rejected.

#### Why this design matters for BFT

Every check is 100% deterministic given only the transaction data. There are no
RPC calls, no on-chain reads, no reliance on external state. This is essential:
all honest validators must agree on whether a transaction is valid. If any check
depended on external state (e.g., current token price), validators could
disagree and the network would never reach consensus.

---

## 12. The Entry Point

`validator/src/validator.ts` is the process entry point. It:

1. Loads `.env` with `dotenv`.
2. Parses `process.env` through `validatorConfigSchema` (Zod). Exits on failure.
3. Creates the structured logger.
4. Creates the viem `Account` from the private key.
5. Builds the `ProtocolConfig` and `WatcherConfig` structs from parsed env.
6. Creates the Prometheus metrics service.
7. Calls `createValidatorService()` which:
   a. Picks HTTP or WebSocket transport based on the RPC URL.
   b. Opens the SQLite database file (if `STORAGE_FILE` is set).
   c. Constructs all the subsystems and wires them together.
   d. Returns a `ValidatorService` instance.
8. Registers `SIGINT` and `SIGTERM` handlers for graceful shutdown.
9. Calls `service.start()` which starts the watcher loop.

---

## 13. Code Quality Standards

### What the codebase does well

| Standard | Practice in the Codebase |
|---|---|
| Runtime validation | All external input (env, blockchain events) goes through Zod |
| Interface segregation | Storage and protocol have thin interfaces, multiple implementations |
| Immutability | State types use `Readonly<>` wrappers; state machine never mutates directly |
| Error containment | `formatError()` used everywhere; state machine never crashes on transition error |
| Observability | Structured JSON logs, Prometheus metrics, log levels |
| Testing | Unit tests for each state transition, integration tests with full chain |
| Bloom filter optimization | `getLogs` calls skipped using `logsBloom` header |
| Graceful shutdown | SIGTERM handler cleans up correctly |
| Deterministic group IDs | Prevents KeyGen DoS attacks |

### Areas that could be improved (vs. ideal standards)

| Concern | Current | Ideal |
|---|---|---|
| Reorg handling | Logs a warning, but no state rollback logic | Full state rollback on deep reorg |
| Key management | Private key loaded from env var in plaintext | KMS / HSM integration |
| SQLite WAL mode | Not explicitly configured | `PRAGMA journal_mode=WAL` at startup |
| SQLite plaintext secrets | Secret shares stored unencrypted | Encrypt-at-rest with a KMS-backed key |
| Action retry strategy | Fixed retry with global backoff | Per-action exponential backoff with jitter |
| Formal spec | Exists in PR #42 as prose | Formal model in TLA+ or a similar spec language |

---

## 14. Key Design Decisions

### Why onchain communication (not P2P)?

Onchain communication gives the protocol:
- **Absolute ordering**: The blockchain provides a canonical order for all messages.
- **Global clock**: Block numbers serve as a deterministic timeout mechanism.
- **No exposure to the internet**: Validators only need an RPC connection, not
  a publicly routable IP.
- **Permissionless interaction**: Users can trigger attestation requests directly.

The trade-off is cost and scalability: it scales quadratically with validator
count because each validator must process each other validator's transactions.

### Why FROST and not ECDSA multisig?

Standard `m-of-n` ECDSA multisig requires all `m` signatures to be submitted and
verified on-chain separately, making the cost grow linearly with `m`. A FROST
Schnorr threshold signature is a single `(R, z)` pair regardless of how many
participants signed. This is both gas-efficient and verifiable with standard
`ecrecover` (Ethereum precompile `0x1`).

### Why deterministic group IDs?

Early versions of the coordinator assigned group IDs. This created a
coordination problem: *who* triggers the new KeyGen? If that participant is
offline, the whole ceremony stalls. By making group IDs deterministic (a hash of
the participants + context), every validator can independently start the ceremony
and will arrive at the same group ID. No designated leader needed.

### Why nonce chunks of 1024?

Nonces must be pre-committed to prevent Wagner's generalized birthday attack
(which can forge signatures if the attacker can choose the message *after* seeing
all nonce commitments). But committing one nonce at a time is too frequent and
expensive. 1024 was chosen as a pragmatic value: large enough that a chunk lasts
for roughly 1024 signatures before the validator needs to commit a new chunk, but
small enough that the Merkle tree for a chunk can be computed between two blocks.

### Why the action queue pattern?

The state machine must be deterministic and not block on I/O. If state
transitions directly awaited RPC calls, a slow or failing RPC would stall the
entire state machine. The action queue decouples the "decision" (synchronous,
in-memory) from the "execution" (async, fallible). Failed actions can be retried
without corrupting the state machine.

### Why both in-memory and SQLite storage?

In-memory storage makes unit tests fast and deterministic (no file system, no
teardown). SQLite makes production safe across restarts. Having identical
interfaces forces the abstractions to be clean. Running integration tests with
`SAFENET_TEST_STORAGE=sqlite` validates that the SQLite implementation is
correct, not just the in-memory one.

---

## 15. End-to-End Lifecycle

Here is a complete trace of what happens from the moment a Safe transaction is
proposed to when it is attested.

### Step 1: Transaction Proposed

A Safe wallet owner calls `consensus.proposeTransaction(tx)` on-chain.
The consensus contract:
1. Validates the transaction format.
2. Emits `TransactionProposed(transactionHash, chainId, safe, epoch, transaction)`.

### Step 2: Validator Sees the Event

`BlockWatcher` detects the new block. `EventWatcher` checks the bloom filter —
the consensus contract address and the `TransactionProposed` topic are both in
the filter, so `getLogs` is called. The log is returned.

`OnchainTransitionWatcher` calls `logToTransition(log)` which parses the log
through `transactionProposedEventTransitionSchema` (Zod). The result is a typed
`StateTransition` with `id: "event_transaction_proposed"`.

### Step 3: State Machine Reacts

`SafenetStateMachine.transition()` is called. The transition is dispatched to
`handleTransactionProposed()`. This function:
1. Calls `verificationEngine.verify(packet)` → `SafeTransactionHandler` runs
   all the deterministic checks.
2. If valid: creates a new `SigningState` with `id: "waiting_for_request"`.
3. Adds action `consensus_request_sign` to the diff (queues the on-chain call to
   start a signing ceremony).

### Step 4: Request Sign Action Submitted

`OnchainProtocol` picks up the `consensus_request_sign` action. It calls
`consensus.requestSign(transactionHash)`. The consensus contract calls
`coordinator.sign(groupKey, message, callback)`.

### Step 5: Sign Event

The coordinator emits `Sign(initiator, gid, message, sid, sequence)`.

The validator's `EventWatcher` picks it up. The state machine transitions
`SigningState` from `waiting_for_request` to `collect_nonce_commitments`.

The state machine also checks: does the validator have a pre-committed nonce
chunk for this group? If yes, add action `frost_sign` (reveal the nonce pair for
this ceremony with a Merkle proof). If no nonces are committed, add action
`frost_nonce_commitments` first.

### Step 6: Nonce Commitments

Each validator calls `coordinator.revealNonces(sid, identifier, nonces, proof)`.
The coordinator emits `NonceCommitmentsRevealed(sid, identifier, nonces)`.

Once all participants have revealed nonces, `SignRevealedNonces(completed=true)`
is emitted.

### Step 7: Signature Shares

The state transitions to `collect_signing_shares`. Each validator computes their
signature share `z_i` and calls `coordinator.submitShare(sid, selectionRoot,
identifier, z_i)`. The coordinator verifies each share on-chain.

Once threshold shares are collected, the coordinator aggregates them into `z`,
computes `R`, and emits `SignCompleted(sid, selectionRoot, (R, z))`.

### Step 8: Attestation

The state transitions to `waiting_for_attestation`. The consensus contract
(via the callback registered in step 4) receives the completed signature and
calls `attestTransaction(transactionHash, (R, z))`. It emits
`TransactionAttested(transactionHash, epoch, (R, z))`.

The validator's event watcher picks up `TransactionAttested`. The state machine
transitions the signing state to "done" and removes it from the signing states
map.

The final Schnorr signature `(R, z)` is now on-chain and can be verified by any
Safe guard on any EVM-compatible chain using `ecrecover` and `sha256`.

---

## 16. Epoch Rollover End-to-End Lifecycle

The epoch rollover is the most complex protocol path. It involves a full
distributed key generation ceremony (three rounds), followed by a signing
ceremony where the *current* epoch's group attests to the *new* epoch's group
key. Two parallel state machines are active simultaneously during a rollover: the
`RolloverState` machine (tracking KeyGen for the new epoch) and, toward the end,
a `SigningState` machine (signing the rollover message with the old epoch's key).

The entry point for the whole flow is `checkEpochRollover()` in
[machine/consensus/rollover.ts](validator/src/machine/consensus/rollover.ts),
which is called on every `block_new` transition.

---

### Step 1: Epoch boundary detected

On every new block, `SafenetStateMachine` calls `checkEpochRollover()`. It
computes `currentEpoch = block / blocksPerEpoch`. If `currentState.nextEpoch <=
currentEpoch`, the rollover deadline has passed and it is time to start KeyGen
for the next epoch.

This check fires whether the previous epoch ended cleanly (`epoch_staged`) or
was skipped. In the `epoch_staged` case, the function also advances `activeEpoch`
in the consensus state as part of the same diff.

---

### Step 2: `triggerKeyGen()` computes the group and queues `key_gen_start`

[machine/keygen/trigger.ts](validator/src/machine/keygen/trigger.ts) is called
with `nextEpoch = currentEpoch + 1`. It:

1. Calls `calcMinimumParticipants()` — if the remaining participant list is too
   short (below 2/3 of the full validator set), the epoch is immediately skipped
   and rollover state becomes `epoch_skipped`. No KeyGen happens.
2. Calls `calcThreshold(count)` — `floor(count / 2) + 1`.
3. Calls `keyGenClient.setupGroup(participants, threshold, context)`, which:
   - Generates the validator's random secret polynomial (secret coefficients).
   - Computes the public commitment to each coefficient: `C[j] = coeff[j] * G`.
   - Computes a proof of knowledge (Schnorr proof over `C[0]`).
   - Computes a proof of attestation participation (POAP).
   - Derives the deterministic group ID from a hash of
     `(participantsRoot, count, threshold, context)`.
   - The `context` for non-genesis epochs is
     `encodePacked(["uint32","address","uint64"], [0, consensusAddress, nextEpoch])`.
4. Returns a `StateDiff` with:
   - `rollover: { id: "collecting_commitments", groupId, nextEpoch, deadline }`
   - `consensus.epochGroup: [nextEpoch, { groupId, participantId }]`
   - `actions: [{ id: "key_gen_start", ... }]`

The `key_gen_start` action is picked up by `OnchainProtocol` and submitted as a
call to `coordinator.keyGenAndCommit(...)`. Every validator does this
independently because the group ID is deterministic — they will all compute the
same `gid` for the same parameters, so there is no coordination required to
"start" the ceremony.

---

### Step 3: Coordinator emits `KeyGen` + `KeyGenCommitted`

The coordinator contract:
1. Records the group if it doesn't exist.
2. Records the participant's commitment.
3. Emits `KeyGenCommitted(gid, identifier, participant, commitment, committed)`
   where `committed = true` once *all* participants have posted their commitments.

Each validator's `EventWatcher` picks this up, `logToTransition()` parses it
into `event_key_gen_committed`, and the state machine calls
`handleKeyGenCommitted()` in
[machine/keygen/committed.ts](validator/src/machine/keygen/committed.ts).

For each commitment event:
- The validator checks it is in `collecting_commitments` and the `gid` matches.
- It calls `keyGenClient.handleKeygenCommitment()` to store the public commitment
  from the sender and verify the proof of knowledge.
- Invalid commitments are logged; the participant will be excluded on timeout.
- When `event.committed === true` (all commitments in):
  - Calls `keyGenClient.createSecretShares(gid)` to compute the encrypted secret
    polynomial evaluation `f_i(j)` for every other participant `j`.
  - Transitions to `collecting_shares`.
  - Queues `key_gen_publish_secret_shares`.

---

### Step 4: Secret shares exchanged

The `key_gen_publish_secret_shares` action submits the validator's encrypted
shares and verification share to `coordinator.secretShare(gid, verificationShare,
shares)`. The coordinator emits `KeyGenSecretShared(gid, identifier, share,
shared)` where `shared = true` once all participants have submitted.

`handleKeyGenSecretShared()` in
[machine/keygen/secretShares.ts](validator/src/machine/keygen/secretShares.ts)
processes each event:

- Calls `keyGenClient.handleKeygenSecrets(gid, senderId, encryptedShares)`.
  - Decrypts the share sent to this validator using ECDH (`a * C_sender[0]`).
  - Verifies the share against the sender's public commitment:
    `f_sender(myId) * G == evalCommitment(sender.commitments, myId)`.
  - If **invalid**: adds the sender to `missingSharesFrom`, queues
    `key_gen_complain`.
  - If **valid**: stores the share.
- Tracks `lastParticipant` — the last participant to submit a share becomes the
  *responsible participant* who must drive the protocol forward if something goes
  wrong at this stage.
- When `event.shared === true` (all participants submitted, valid or not):
  - Transitions to `collecting_confirmations` with three deadlines:
    - `complaintDeadline`: deadline for filing complaints.
    - `responseDeadline`: deadline for responding to complaints.
    - `deadline`: overall confirmation deadline.
  - If all received shares were valid: queues `key_gen_confirm` with a
    `callbackContext` encoding the epoch rollover proposal.

---

### Step 5: Confirmations (and the callback mechanism)

The `key_gen_confirm` action calls `coordinator.confirmWithCallback(gid,
callbackContext)`. The `callbackContext` encodes what the coordinator should do
when the last confirmation arrives — specifically, propose the epoch rollover to
the consensus contract automatically.

The coordinator emits `KeyGenConfirmed(gid, identifier, confirmed)` where
`confirmed = true` once all participants have confirmed.

`handleKeyGenConfirmed()` in
[machine/keygen/confirmed.ts](validator/src/machine/keygen/confirmed.ts)
processes each confirmation:

- Tracks `confirmationsFrom`. Updates `lastParticipant`.
- When all participants have confirmed, two paths diverge:

**Genesis group path**: If `consensusState.genesisGroupId === groupId`:
- The genesis KeyGen is done. No signing ceremony needed.
- Transitions rollover to `epoch_staged` with `nextEpoch: 0n`.
- Generates a nonce tree for the genesis group and queues
  `sign_register_nonce_commitments`.
- The next `block_new` will trigger KeyGen for epoch 1 (the first real epoch).

**Non-genesis path**:
- Computes the epoch rollover message by building an `EpochRolloverPacket`:
  `{ activeEpoch, proposedEpoch, rolloverBlock, groupKeyX, groupKeyY }`.
- Passes it through `verificationEngine.verify()` → `EpochRolloverHandler` to
  get the deterministic message hash.
- Transitions rollover to `sign_rollover` with the message hash.
- If this validator is in the *current* epoch's group: starts a `SigningState`
  with `id: "waiting_for_request"` for the rollover message.

---

### Step 6: Signing the rollover (with the old epoch's key)

From this point the flow is identical to the transaction attestation lifecycle
in Section 15, but the message being signed is the epoch rollover packet hash,
and the signing group is the **current epoch's** FROST group (not the new one).

The `responsible` participant from `handleKeyGenConfirmed` (the last to confirm)
is the one expected to submit the `sign_request` on-chain to start the signing
ceremony.

---

### Step 7: `EpochProposed` event

When the signing ceremony succeeds, the coordinator fires its callback, which
calls `consensus.proposeEpoch(...)`. The consensus contract emits `EpochProposed(
activeEpoch, proposedEpoch, rolloverBlock, groupKey)`.

`machine/signing/sign.ts` picks this up via `event_epoch_proposed` and starts
the signing process for the epoch rollover message by queuing
`sign_reveal_nonce_commitments`.

---

### Step 8: `EpochStaged` event

Once the signing ceremony completes and the rollover attestation is produced,
the consensus contract emits `EpochStaged(activeEpoch, proposedEpoch,
rolloverBlock, groupKey, attestation)`.

`handleEpochStaged()` in
[machine/consensus/epochStaged.ts](validator/src/machine/consensus/epochStaged.ts)
processes this:

1. Transitions rollover state from `sign_rollover` to
   `epoch_staged` with `nextEpoch = event.proposedEpoch`.
2. Cleans up the rollover signing state.
3. Generates a fresh nonce tree for the **new** group (now that KeyGen is
   confirmed and the group is active).
4. Queues `sign_register_nonce_commitments` for the new group.
5. Marks `groupPendingNonces[groupId] = true` (nonce chunk posted).

---

### Step 9: Epoch advances on the next block boundary

The next time `block_new` fires and `checkEpochRollover()` runs:
- `currentState.id === "epoch_staged"` and `currentState.nextEpoch <=
  currentEpoch`: the function promotes `nextEpoch` to `activeEpoch` in the
  consensus state and starts KeyGen for `currentEpoch + 1` immediately.
- Old epoch groups below the cleanup threshold are garbage-collected via
  `computeCleanupThreshold()`: any epoch group whose epoch number is strictly
  less than the minimum epoch referenced by any active signing session is removed
  from memory and the FROST crypto storage is unregistered.

---

### Epoch Rollover: Full Event Sequence

```
block_new (epoch boundary detected)
    → checkEpochRollover() → triggerKeyGen()
    → action: key_gen_start
        → coordinator.keyGenAndCommit()
            → event: KeyGenCommitted (per participant)
                → handleKeyGenCommitted() [n times]
                    → last: action key_gen_publish_secret_shares
                        → coordinator.secretShare()
                            → event: KeyGenSecretShared (per participant)
                                → handleKeyGenSecretShared() [n times]
                                    → last: action key_gen_confirm (with callback)
                                        → coordinator.confirmWithCallback()
                                            → event: KeyGenConfirmed (per participant)
                                                → handleKeyGenConfirmed() [n times]
                                                    → last: rollover → sign_rollover
                                                    → signing → waiting_for_request
                                                    → [signing ceremony as per Section 15]
                                                        → EpochProposed
                                                        → EpochStaged
                                                            → handleEpochStaged()
                                                            → action: sign_register_nonce_commitments
block_new (next epoch boundary)
    → checkEpochRollover() → advance activeEpoch
    → triggerKeyGen() for epoch N+2
```

---

### Genesis vs. Regular Epoch KeyGen: Key Differences

| Aspect | Genesis (epoch 0) | Regular epoch |
|---|---|---|
| Trigger | `event_key_gen` seen + `waiting_for_genesis` state | `block_new` at epoch boundary |
| Group context | `keccak256("genesis" \|\| genesisSalt)` or `zeroHash` | `encodePacked(0, consensusAddress, nextEpoch)` |
| Timeout | `maxUint64` (no timeout) | `block + keyGenTimeout` |
| After confirmation | Transition to `epoch_staged`, start preprocessing | Compute rollover message, start signing ceremony |
| Signs anything? | No — genesis group only needs to exist | Yes — old group signs the new epoch rollover |

**The chicken-and-egg problem**: the genesis group ID is computed from the group
parameters including a `context`. For regular epochs, the context includes the
consensus contract address. But the consensus contract address is determined
during deployment, and the deployment often uses the genesis group ID as a
constructor argument — making the genesis group ID a dependency of the contract
address which is a dependency of the genesis group ID.

The solution in [machine/keygen/group.ts:62-84](validator/src/machine/keygen/group.ts#L62-L84):
for genesis, the context does not use the consensus contract address. Instead it
uses `GENESIS_SALT` (a user-configured `bytes32` value):

```ts
const context = genesisSalt === zeroHash
  ? zeroHash
  : keccak256(encodePacked(["string", "bytes32"], ["genesis", genesisSalt]))
```

**Operational implication**: if you want to run the same validator set for two
different deployments of the consensus contract (e.g., mainnet and testnet), you
must set a different `GENESIS_SALT` for each. Otherwise both deployments would
derive the same genesis group ID and validators would confuse events from one
deployment with the other.

**`SKIP_GENESIS`**: if a validator joins a network that already completed genesis,
it sets `SKIP_GENESIS=true`. The rollover state starts as `{ id: "skip_genesis" }`
rather than `{ id: "waiting_for_genesis" }`. On the next epoch boundary,
`checkEpochRollover()` sees `skip_genesis`, transitions to `epoch_skipped` for
the current in-progress epoch, and from the next epoch boundary onward
participates in KeyGen normally.

---

### What happens when KeyGen fails

If a timeout fires while in `collecting_commitments`, `collecting_shares`, or
`collecting_confirmations`, `checkKeyGenTimeouts()` in
[machine/keygen/timeouts.ts](validator/src/machine/keygen/timeouts.ts) runs:

- Identifies which participants did not participate in time.
- Removes them from the participant list.
- If the remaining count ≥ `calcMinimumParticipants()`: calls `triggerKeyGen()`
  again with the reduced participant set. The new group ID is different because
  the `participantsRoot` changed.
- If the remaining count < minimum: transitions to `epoch_skipped`. The current
  epoch continues for another period, and KeyGen will be reattempted at the next
  epoch boundary.

---

## 17. Nonce Chunk Pre-commitment Lifecycle

Nonces are one of the most security-critical parts of FROST. Reusing a nonce for
two different messages leaks the signing key share. The chunk pre-commitment
system ensures nonces are safely bound to specific ceremonies before any message
is known.

### Why chunks?

Committing one nonce at a time on-chain would require an on-chain transaction
before every single signing ceremony — expensive and slow. Instead, the validator
pre-commits a Merkle root covering a *chunk* of 1024 nonces at once. During each
signing ceremony, the specific nonce pair for that ceremony is revealed with a
Merkle proof, which the coordinator contract verifies against the stored root.
This is defined in
[consensus/signing/nonces.ts](validator/src/consensus/signing/nonces.ts) as
`SEQUENCE_CHUNK_SIZE = 1024n`.

### Phase 1: Generating the nonce tree

Triggered at two points:
1. After genesis KeyGen completes (`handleKeyGenConfirmed` for the genesis group).
2. After every epoch is staged (`handleEpochStaged`).

`signingClient.generateNonceTree(groupId)` calls `createNonceTree(signingShare)`:

```
for i in 0..1023:
    hidingNonce[i]  = H3(randomBytes(32) || signingShare_bytes)
    bindingNonce[i] = H3(randomBytes(32) || signingShare_bytes)
    D[i] = hidingNonce[i] * G
    E[i] = bindingNonce[i] * G
    leaf[i] = keccak256(abi.encode(i, D[i].x, D[i].y, E[i].x, E[i].y))

root = calculateMerkleRoot(leaves)
```

The `H3` hash function (FROST spec) mixes randomness with the signing share,
preventing a compromised RNG from leaking the nonce. The `i` offset is included
in each leaf hash so a nonce pair is cryptographically bound to exactly one
sequence slot, preventing swapping attacks.

Both the nonces and the full Merkle tree are stored in `ClientStorage` (SQLite
in production). Only the `root` is returned to the state machine; the action
queued is:

```
{ id: "sign_register_nonce_commitments", groupId, nonceCommitmentsHash: root }
```

`groupPendingNonces[groupId]` is set to `true` in the consensus state, meaning:
"a nonce tree has been generated and posted but the on-chain chunk number has not
yet been confirmed."

### Phase 2: Posting the root on-chain

`OnchainProtocol` submits `sign_register_nonce_commitments` by calling
`coordinator.commitNonces(groupId, root)`. The coordinator:
- Assigns this root a monotonically increasing `chunk` number for the group.
- Emits `NonceCommitmentsHash(gid, identifier, chunk, commitment)`.

### Phase 3: Linking the chunk number (`handlePreprocess`)

[machine/signing/preprocess.ts](validator/src/machine/signing/preprocess.ts)
handles `event_nonce_commitments_hash`:

```ts
signingClient.handleNonceCommitmentsHash(gid, identifier, commitment, chunk)
```

This links the on-chain `chunk` number to the locally stored nonce tree so the
validator knows which nonces map to which on-chain sequence numbers.
`groupPendingNonces[gid]` is cleared (`false`) — the chunk is now active.

### Phase 4: Using a nonce in a signing ceremony

When a signing ceremony starts with sequence number `s`, the validator computes:

```
chunk  = s / 1024     (which chunk this sequence belongs to)
offset = s % 1024     (position within that chunk)
```

via `decodeSequence(sequence)`. It then calls
`nonceCommitmentsWithProof(nonceTree, offset)` which:
1. Returns the `(D, E)` commitment pair at position `offset` in the stored tree.
2. Generates a Merkle proof: the sibling hashes needed for the coordinator to
   verify `leaf[offset]` against the stored root.

Both are included in the `sign_reveal_nonce_commitments` action. The coordinator
verifies the proof on-chain against the stored root before accepting the nonce.

**The nonce is deleted from storage immediately after use.** This is the primary
defence against nonce reuse in a reorg scenario: even if the signing ceremony is
replayed on a different message, the validator no longer has the nonce and cannot
produce a second signature share for the same `(D, E)` pair.

### Phase 5: Chunk exhaustion → next tree

When all 1024 nonces in a chunk have been used, the validator generates a new
nonce tree and queues another `sign_register_nonce_commitments`. The
`groupPendingNonces` flag gates participation: a validator will not reveal nonces
for a sequence number in a chunk it has not yet committed, preventing reuse of a
nonce from a future (uncommitted) chunk.

### Nonce security properties

| Property | How it is enforced |
|---|---|
| No nonce reuse | Nonce deleted from storage immediately after use |
| Bound to group | Tree generated from that group's signing share as entropy |
| Bound to ceremony | Merkle proof ties each `(D, E)` to exactly one sequence slot |
| Wagner's attack prevented | Nonces committed before the message is known; sequence number uniquely allocates a nonce per message |
| Biased nonce prevented | `H3(random \|\| signingShare)` mixes randomness with a secret; a broken RNG alone cannot leak the key |

---

## 18. Complaint Flow Lifecycle

The complaint flow is the DKG fault-tolerance mechanism. If validator A sends
validator B an invalid secret share, B cannot silently ignore it — that would
leave B with a corrupted signing share. Instead B files a public complaint
on-chain, forcing A to reveal the secret so all validators can verify it.

### When is a complaint triggered?

Inside `handleKeyGenSecretShared()`, after decrypting and verifying the received
share from `senderId`:

```ts
const response = await keyGenClient.handleKeygenSecrets(groupId, senderId, shares)
// response === "invalid_share" when:
//   f_sender(myId) * G  !=  evalCommitment(sender.commitments, myId)
if (response === "invalid_share") {
    actions.push({ id: "key_gen_complain", groupId, accused: senderId })
}
```

The verification check is: the public commitment to the share
(`evalCommitment(sender.commitments, myId)`) must equal the share point
(`share * G`). If they differ, the share is wrong.

### Step 1: Filing the complaint on-chain

`key_gen_complain` calls `coordinator.complain(gid, accused)`. The coordinator
increments an internal complaint counter against the accused and emits:

```
KeyGenComplaintSubmitted(gid, plaintiff, accused, compromised)
```

`compromised = true` when the count against `accused` exceeds the signing
threshold — enough validators have complained that the group can no longer
produce a valid signing share even after removing the accused.

`handleComplaintSubmitted()` in
[machine/keygen/complaintSubmitted.ts](validator/src/machine/keygen/complaintSubmitted.ts)
updates `machineStates.rollover.complaints[accused]`. If `compromised = true`,
it immediately calls `triggerKeyGen()` with `accused` removed from the participant
set.

### Step 2: The accused must respond

If `compromised = false`, the accused has until `responseDeadline` (2x
`keyGenTimeout` after the share phase ended) to respond by revealing the
plaintext secret share. This allows all validators to independently verify
whether the share was genuinely invalid or whether the plaintiff filed a false
complaint.

The accused calls `coordinator.complaintResponse(gid, plaintiff, secretShare)`.
The coordinator verifies the revealed `secretShare` against the commitment
on-chain and emits:

```
KeyGenComplaintResponded(gid, plaintiff, accused, secretShare)
```

`handleComplaintResponded()` in
[machine/keygen/complaintResponse.ts](validator/src/machine/keygen/complaintResponse.ts)
decrements `complaints[accused].unresponded`. If the share is verified valid
on-chain, the complaint was unfounded (the plaintiff lied or had a bug). If the
share is verified invalid on-chain, the coordinator sets a `compromised` flag and
the next complaint event will trigger a group restart.

### Step 3: Response timeout

If the accused does not respond by `responseDeadline`, `checkKeyGenTimeouts()`
fires. The non-response is treated as an admission of guilt: the accused is
removed from the participant set and KeyGen is restarted. This is enforced purely
by the validator's state machine — the absence of a `KeyGenComplaintResponded`
event by the deadline is sufficient.

### Step 4: Complaint flow summary

```
handleKeygenSecrets() → invalid_share
  → action: key_gen_complain
      → coordinator.complain()
          → KeyGenComplaintSubmitted(compromised=false)
              → accused responds within responseDeadline
                  → coordinator.complaintResponse()
                      → KeyGenComplaintResponded()
                          → share was valid  → plaintiff wrong, continue
                          → share was invalid → compromised on next complaint
              → accused does NOT respond by responseDeadline
                  → checkKeyGenTimeouts() → accused removed → triggerKeyGen()
          → KeyGenComplaintSubmitted(compromised=true)
              → handleComplaintSubmitted() → accused removed → triggerKeyGen()
```

---

## 19. Complete Environment Variable Reference

All variables are defined in `validatorConfigSchema` in
[types/schemas.ts](validator/src/types/schemas.ts) and documented in detail in
[validator/.env.sample](validator/.env.sample). The table below maps each
variable to the code path it feeds and the consequence of a wrong value.

### Required variables

| Variable | Type | Code path | What breaks if wrong |
|---|---|---|---|
| `RPC_URL` | URL string | viem `http()` or `webSocket()` transport | No chain connection; validator cannot start |
| `PRIVATE_KEY` | `0x` hex 32 bytes | `privateKeyToAccount()` → validator's on-chain identity | Transactions sent from wrong address; wrong participant recognised on-chain |
| `CONSENSUS_ADDRESS` | checksummed address | `OnchainTransitionWatcher` watch list + `OnchainProtocol` call targets | No consensus events received; actions target wrong contract |
| `COORDINATOR_ADDRESS` | checksummed address | `OnchainTransitionWatcher` watch list + `OnchainProtocol` call targets | No coordinator events received; KeyGen/signing calls to wrong contract |
| `CHAIN_ID` | `100` \| `11155111` \| `31337` | `extractChain()` → viem chain config | Wrong block time; wrong fee model; chain mismatch errors |
| `PARTICIPANTS` | comma-separated addresses | `participantsSchema` → `[{address, id: BigInt(i+1)}]`; feeds all KeyGen group computations | Wrong list/order → wrong group ID → genesis KeyGen never matches peers |

### Optional variables — logging & monitoring

| Variable | Default | Effect |
|---|---|---|
| `LOG_LEVEL` | `"notice"` | Verbosity. `"debug"` shows per-transition details; `"silly"` shows crypto internals |
| `METRICS_PORT` | none | Starts Prometheus HTTP server on this port, exposing `/metrics` |

### Optional variables — storage

| Variable | Default | Effect |
|---|---|---|
| `STORAGE_FILE` | none | Path to SQLite DB. **Required for production.** Without it all FROST key material is lost on restart |
| `STORAGE_BACKUP` | none | `printf` format string for Docker entrypoint backup (`backup-%s.db`). Only used by `bin/entrypoint.sh`, not the Node process itself |

### Optional variables — protocol

| Variable | Default | Effect |
|---|---|---|
| `GENESIS_SALT` | `0x000...0` | Bytes32 salt for genesis group context. Must differ per independent consensus deployment. See Section 16 |
| `SKIP_GENESIS` | `false` | Set `true` when joining a network after genesis completed. Skips `waiting_for_genesis` |
| `BLOCKS_PER_EPOCH` | `17280` (~1 day at 5s/block) | `currentEpoch = block / blocksPerEpoch`. Must match all peers |
| `STAKER_ADDRESS` | validator's own address | Updated on startup via `consensus_set_validator_staker`. Use when a delegator stakes on your behalf |

### Optional variables — transaction submission

| Variable | Default | Effect |
|---|---|---|
| `BLOCKS_BEFORE_RESUBMIT` | `1` | Blocks before bumping fees on a stuck tx. `1` = re-submit every block |
| `BASE_FEE_MULTIPLIER` | `2` | Multiplier on base fee for `maxFeePerGas`. Higher = more fee buffer during spikes |
| `PRIORITY_FEE_PER_GAS` | RPC estimate | Fixed miner tip in wei. Overrides `eth_maxPriorityFeePerGas` |

### Optional variables — block watching

| Variable | Default | Effect |
|---|---|---|
| `BLOCK_TIME_OVERRIDE` | chain built-in | Block interval in ms. **Required for Sepolia and Anvil** (no built-in viem value) |
| `MAX_REORG_DEPTH` | `5` | Rolling window of block hashes for reorg detection |
| `BLOCK_PAGE_SIZE` | `100` | Blocks per `eth_getLogs` call during catch-up. Auto-halves on RPC errors |
| `MAX_LOGS_PER_QUERY` | none | If a `getLogs` response has ≥ this many logs, assume silent truncation and retry with smaller range |

### Critical ordering note for `PARTICIPANTS`

The schema assigns IDs by index:

```ts
participants.map((address, i) => ({ address, id: BigInt(i + 1) }))
```

`PARTICIPANTS=0xAlice,0xBob,0xCarol` → Alice=1, Bob=2, Carol=3.

All validators must use **the same list in the same order**. A single
transposition (`0xBob,0xAlice,...`) produces a different `participantsRoot`,
a different group ID, and the validator's `key_gen_start` transactions will
be silently ignored by all peers.

---

## 20. Complete `ProtocolAction` Reference

All 12 action types are defined in
[consensus/protocol/types.ts](validator/src/consensus/protocol/types.ts).
Every action is serialised to the `SqliteActionQueue` with a `validUntil`
timestamp and executed by `OnchainProtocol` on each `block_new`.

### KeyGen actions

| `id` | Contract call | When queued |
|---|---|---|
| `key_gen_start` | `coordinator.keyGenAndCommit(...)` | `triggerKeyGen()` — every epoch boundary or genesis |
| `key_gen_publish_secret_shares` | `coordinator.secretShare(gid, verificationShare, shares)` | After `event.committed === true` (all commitments in) |
| `key_gen_confirm` | `coordinator.confirmWithCallback(gid, callbackContext)` | After all own received shares are valid |
| `key_gen_complain` | `coordinator.complain(gid, accused)` | When an invalid secret share is detected |
| `key_gen_complaint_response` | `coordinator.complaintResponse(gid, plaintiff, secretShare)` | When this validator is the accused and must respond |

### Signing actions

| `id` | Contract call | When queued |
|---|---|---|
| `sign_register_nonce_commitments` | `coordinator.commitNonces(groupId, root)` | After genesis/epoch confirmed; when nonce chunk exhausted |
| `sign_reveal_nonce_commitments` | `coordinator.revealNonces(sid, nonces, proof)` | When a `Sign` event is received and nonces are available |
| `sign_request` | `consensus.requestSign(gid, message)` | When `waiting_for_request` and this validator is the responsible participant |
| `sign_publish_signature_share` | `coordinator.shareWithCallback(sid, signersRoot, signersProof, R, R_i, z_i, lambda_i, callbackContext)` | After all nonce commitments are revealed |

### Consensus actions

| `id` | Contract call | When queued |
|---|---|---|
| `consensus_attest_transaction` | `consensus.attestTransaction(epoch, txHash, signatureId)` | After `SignCompleted` event when this validator is responsible |
| `consensus_stage_epoch` | `consensus.stageEpoch(proposedEpoch, rolloverBlock, groupId, signatureId)` | Via callback from the last KeyGen confirmation |
| `consensus_set_validator_staker` | `consensus.setValidatorStaker(staker)` | On startup if on-chain staker doesn't match config |

### Action lifecycle in code

```
StateDiff { actions: [...] }
  → flushed to SqliteActionQueue (validUntil = now + timeout)

block_new fires
  → OnchainProtocol.checkPendingActions(block)
      → load overdue actions from queue
      → for each action:
          → GasFeeEstimator.estimate() (cached per block, invalidated on block_new)
          → walletClient.writeContract(...)
          → success          → remove from queue
          → nonce too low    → increment nonce, retry immediately
          → rate limit       → global exponential backoff (1→64 seconds)
          → validUntil past  → drop action, log warning
```

---

## 21. Watcher Inner Loop and Backoff

### The `Watcher` class loop (`watcher/index.ts`)

The `Watcher` in [watcher/index.ts](validator/src/watcher/index.ts) runs a
**drain-then-advance** loop in `#next()`:

```
Phase 1 — drain event pages:
  while true:
    logs = events.next()
    if logs === null → break (EventWatcher is idle)
    if logs.length > 0 → onUpdate({ type: "watcher_update_new_logs", logs })
    if !running → return (clean shutdown mid-warp)

Phase 2 — advance block:
  update = blocks.next()           (sleeps until block expected time)
  events.onBlockUpdate(update)     (tell EventWatcher about new/uncle block)
  onUpdate(update)                 (notify state machine: block_new or uncle)
```

**Why drain first**: the `EventWatcher` may be mid-warp over hundreds of
historical blocks. Calling `events.next()` repeatedly drains one page per call
until `null` indicates "caught up". Only then does the watcher poll for the next
live block. This guarantees historical events are never skipped and are always
processed before live ones.

**Error isolation**: the outer `while` loop in `#run()` wraps `#next()` in a
`try/catch`. A transient RPC error throws, is caught, logged as a warning,
and the loop restarts. The state machine is never corrupted because
`#onUpdate` is only called on success.

### Backoff (`watcher/backoff.ts`)

The `Backoff` class uses exponential delays: `[1, 2, 4, 8, 16, 32, 64]` seconds.

**The key distinction**: only two error types *increase* the delay:
- HTTP 429 (`HttpRequestError` with `status === 429`)
- EIP-1474 rate limit (`LimitExceededRpcError`)

All other errors **reset** the delay to zero. This is intentional: non-rate-limit
errors (timeouts, 500s, block-not-found) are transient and should be retried
immediately. Only sustained 429s warrant backing off. If the validator hits a
rate limit and then gets a 500, the backoff resets and the next attempt is
immediate.

---

## 22. Testing Guide

### Unit tests

```sh
npm run test -w validator
npm run test -w validator -- --watch   # watch mode
```

Tests live alongside source files as `*.test.ts` and use **Vitest**. Every
state machine handler has a corresponding test file under `machine/keygen/`,
`machine/signing/`, and `machine/consensus/`.

**Pattern**: each test:
1. Builds a minimal `MachineStates` / `ConsensusState` for the state the handler
   cares about.
2. Builds a typed `StateTransition` event (usually via a helper in
   `src/__tests__/utils.ts`).
3. Calls the handler function directly.
4. Asserts on the returned `StateDiff` — specifically `diff.rollover?.id`,
   `diff.actions`, `diff.signing`, and `diff.consensus`.

No state machine, no SQLite, no blockchain needed for unit tests. Every handler
is a pure async function: `(config, clients, states, event) → StateDiff`.

### Integration tests

```sh
npm run test:integration
SAFENET_TEST_STORAGE=inmemory npm run test:integration
SAFENET_TEST_STORAGE=sqlite   npm run test:integration
```

The script (`scripts/run_integration_test.sh`):
1. Starts an Anvil local chain.
2. Deploys all contracts with `forge script`.
3. Runs multiple validator processes against Anvil.
4. Submits test transactions and asserts that attestations appear on-chain.

`SAFENET_TEST_STORAGE=sqlite` exercises the full SQLite persistence path,
validating that state survives across simulated restarts.

### What to test when adding a new state transition handler

1. **Happy path**: correct input → correct `StateDiff` (right state transition,
   right actions).
2. **Wrong state guard**: when in a state that should ignore this event, returns
   `{}`.
3. **Wrong group/signature ID**: event `gid`/`sid` doesn't match current state,
   returns `{}`.
4. **Timeout boundary**: calling the timeout checker at `deadline + 1n` produces
   the expected failure transition and correct participant exclusion.
5. **Partial progress**: intermediate events (e.g., not yet `event.committed ===
   true`) update tracking fields but do not advance the state.

---

*Last updated: 2026-03-07. Written against commit 9c7abb7 (main branch).*
