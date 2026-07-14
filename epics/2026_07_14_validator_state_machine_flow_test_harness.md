# Plan: Validator state-machine flow-test harness

Component: the existing `crates/validator` crate (Cargo package `validator`), with test-only use of
the `safenet-core` state machine and the real `contracts` Foundry artifacts on a local Anvil chain.
No `safenet-core` API or production validator runtime changes are expected, apart from a narrowly
scoped test seam for generating a reduced usable nonce prefix if the first benchmark confirms it is
needed.

---

## Overview

The validator state machine in `crates/validator/src/state/` has very little end-to-end behavioral
coverage. Testing transitions by constructing the private `State`, calling a specific transition,
and asserting the complete `(State, Commands)` result would couple tests to the state machine's
internal representation. Routine refactors would then require large, low-value test rewrites.

This epic adds declarative **flow tests** around the highest practical test boundary. Each validator
node runs the real [`ValidatorService`](../crates/validator/src/service/mod.rs),
[`StateMachine`](../crates/core/src/state/mod.rs), effect handler, secret store, and action encoder.
The encoded transactions execute against real `FROSTCoordinator`, `Consensus`, and oracle contracts
on Anvil. Tests assert only observable protocol behavior: mined typed calls, receipts and events,
contract views, responsible actors, retries, exclusions, and attestations.

The harness deliberately replaces only the production orchestration that would make these tests
slow or nondeterministic: watcher polling, the durable transaction queue, fee replacement, and the
open-ended [`Driver::run`](../crates/core/src/driver.rs) loop. A synchronous block pump broadcasts all
currently queued actions, mines exactly one block, decodes its real logs, and feeds the corresponding
updates to every online validator. Protocol waits are expressed in bounded block counts; there are no
wall-clock sleeps or interval mining in a flow.

The work is split into a foundation vertical slice followed by independently reviewable groups of
flows:

- Anvil lifecycle, contract deployment, node wiring, block pumping, tracing, and a happy genesis DKG.
- Typed fault injection plus transaction and signing recovery flows.
- Key generation complaints/timeouts, epoch rollover, membership, and oracle flows.
- Restart and reorg security flows, followed by a coverage-guided gap audit.

This is not a replacement for the existing process-level integration test. That test remains a small
smoke test for the real watcher, transaction queue, configuration, startup reconciliation, and runtime
loop. The new suite is where state-machine behavior receives broad and diagnostic coverage.

---

## Architecture Decision

The harness will be a crate-internal `#[cfg(test)]` module in the validator binary. Keeping it inside
the crate gives one small adapter access to private validator types without making state-machine
internals public or first restructuring the binary as a library.

```text
declarative flow test
        |
        v
 Scenario / TestDriver -----------------------> typed external calls and faults
        |                                                 |
        |                                                 v
        +--> TestNode[]                             TestChain (Anvil)
        |      |                                          |
        |      +-- ValidatorService::components()         +-- real contracts
        |      +-- safenet_core::StateMachine             +-- real receipts/logs/views
        |      +-- real effects + per-node SQLite          |
        |      +-- real ActionEncoder <--------------------+
        |
        +--> semantic trace + observable assertions
```

### Test boundary

Every protocol action must cross the same boundaries it crosses in production:

1. A real state transition emits an action.
2. The production `ActionEncoder` converts it to destination, calldata, gas, and expiry.
3. The harness decodes the transaction into a semantic call for tracing/fault matching, without
   changing the bytes by default.
4. The validator's signer submits it to Anvil.
5. The real contract accepts or rejects it and emits logs.
6. The existing validator `Event` decoder turns those raw logs back into state-machine events.

The tests must not synthesize successful `KeyGen`, `Sign`, `SignCompleted`, attestation, or rollover
events. Those endogenous events have to arise from mined contract calls. Scenarios may introduce only
exogenous inputs—such as starting genesis, proposing a transaction, resolving a manual oracle, mining
blocks, stopping a validator, or creating a fork—and explicit faults.

Individual flow tests import a small harness prelude. Only the node/call adapters may import private
`State`, `Action`, `Effect`, or transition types. A refactor of an internal enum should therefore
affect at most the adapter, not every test case.

### Synchronous block pump

Anvil runs with automining and interval mining disabled and FIFO mempool ordering. Validator actions
created while processing block `N` are queued for the next explicit mine. A block step:

1. Takes the encoded validator calls queued by previous updates and any explicitly queued external
   calls, preserving actor and action order.
2. Applies semantic fault rules and drops calls whose production expiry is at or before the current
   head.
3. Allocates signer nonces only to calls that will actually be broadcast, then submits them without
   awaiting receipts.
4. Mines exactly one block, including an empty block when a timeout must advance.
5. Collects every receipt and fails immediately on an unexpected revert.
6. Fetches the actual block header and logs, retaining `(block, log index, address)` ordering.
7. Feeds each online node `Update::Block(BlockUpdate::New { ... })` followed by the matching
   `Update::Logs`; the core state machine settles effects and resume messages internally.
8. Encodes newly emitted actions immediately and queues them for the following block.

External transactions default to a documented stable position relative to already queued validator
calls. Tests that intentionally exercise same-block ordering must construct an explicit ordered block
plan rather than depend on incidental async scheduling.

### Observable assertions

Assertions are built around three public observation sources:

- Contract state and view calls, for example active/staged epoch, group key, participant key,
  signature value/verification, and transaction attestation.
- Decoded receipts and events, including the actor and transaction that caused each event.
- Decoded outbound calls, including attempt count, selected sender, callback presence, expiry, and
  whether a call was mined, delayed, dropped, reverted, or expired.

Tests do not compare complete state snapshots, command vectors, exact random group keys/nonces, or
incidental block numbers. Exact public calldata may be compared only where it is itself the security
property—for example proving that a DKG commitment is replayed unchanged after a reorg. Assertions
such as `attested`, `not_attested`, `submitted_by`, `sign_attempts`, and `group_excludes` hide ABI
details and include the semantic trace in their failure output.

### Typed fault injection

Faults are applied after real action encoding but before nonce allocation and submission. Calls are
classified by destination and decoded with the generated `Consensus`/`Coordinator` call interfaces,
yielding a stable `ProtocolCall` enum. Matchers select an actor, call kind, protocol identifier, and/or
occurrence rather than matching raw bytes.

The initial fault vocabulary is:

- Drop once/always, delay by a bounded number of blocks, or reorder within an explicit block.
- Mute a node's outbound calls, pause inbound block delivery, resume, and catch up from recorded
  canonical blocks.
- Corrupt a typed DKG encrypted share, complaint response, nonce reveal, or signature share before it
  is re-encoded and signed.
- Suppress a completion callback by converting a valid `*WithCallback` call to its non-callback
  equivalent while preserving the valid share/confirmation.
- Mark a deliberately malformed call as an expected revert; all other reverts fail the test.

Faults are semantic and narrowly scoped. The harness will not provide a general raw-log injection API
for flow tests, because that would make impossible contract histories easy to create.

### Persistence and reorg model

Each validator has its own temporary SQLite database, matching production isolation. Restart tests
drop and reconstruct the service components against the same database and replay any canonical blocks
missed while the node was offline. Restarts occur at fully processed block boundaries; persistence of
the production transaction queue remains outside this harness.

Reorg tests pair Anvil's `evm_snapshot`/`evm_revert` with the real core
`BlockUpdate::Uncle { number }`. The state machine rolls its snapshot tables back while the secret
store tables deliberately remain untouched. The alternate branch is then mined and delivered through
the normal block pump. This directly exercises the two critical security properties:

- A replayed DKG setup reuses the retained coefficients and publishes the same commitment.
- A signing nonce burned on the removed branch is not reused for a different message on the alternate
  branch.

### Alternatives Considered

- **Direct `(State, Message) -> (State, Commands)` tests for every branch.** Rejected as the primary
  strategy because they encode private state layout and command sequencing. Small direct tests remain
  appropriate for pure helpers and defensive branches that real contracts cannot produce.
- **Run the full production `Driver` in every flow.** Rejected for this suite because watcher polling,
  fee estimation/replacement, durable queue timing, and shutdown make state behavior slower and less
  diagnostic. Those components already have focused core tests and retain one process-level
  integration smoke test.
- **Use a mock RPC chain and script protocol events, matching
  [`test-exemplar`](../test-exemplar/) literally.** Rejected
  for endogenous validator events. It would test that the state machine accepts the history written by
  the test, while missing invalid calldata, callback behavior, contract ordering, proof verification,
  and reverts. The exemplar's typed scenarios, named actors, presets, and public assertions are kept;
  its mocked event source is not.
- **Implement a Rust model of the Coordinator and Consensus contracts.** Rejected because it creates
  a second protocol implementation that can drift from Solidity. Local Anvil execution is fast enough
  once mining is explicit.
- **Use fixed block times and sleep until a condition becomes true.** Rejected because it is slow and
  flaky. Every eventual assertion has a maximum number of explicitly mined blocks.
- **Assert seeded cryptographic outputs.** Rejected as the default because commitments and nonces are
  not the behavior under test, and exact values make harmless cryptographic refactors noisy. Actor
  keys, scenario inputs, ordering, and group/signature identifiers are already deterministic. A seeded
  cryptographic RNG can be added later only if it materially improves reproduction of observed
  failures.
- **Share one initialized Anvil/database snapshot across all tests.** Deferred. Per-scenario processes
  provide clean isolation and parallel execution. Shared snapshots will be considered only if measured
  startup/deployment cost dominates the suite.

---

## Tech Specs

### Proposed source layout

```text
crates/validator/src/
  main.rs                         # #[cfg(test)] mod flow_tests;
  flow_tests/
    mod.rs                        # test modules and harness prelude
    harness/
      mod.rs
      chain.rs                    # Anvil lifecycle, RPC controls, block/fork history
      contracts.rs                # artifact loading, deployment, test-only calls/views
      node.rs                     # ValidatorService components + StateMachine adapter
      driver.rs                   # block pump, action collection, catch-up orchestration
      scenario.rs                 # declarative builder, named actors, presets
      calls.rs                    # ProtocolCall decoding/encoding and matchers
      faults.rs                   # fault plan and node controls
      trace.rs                    # semantic calls, receipts, events, branch/block trace
      fixtures.rs                 # participants, transactions, timeouts, ready-state helpers
      assertions.rs               # observable protocol assertions
    genesis.rs
    transactions.rs
    signing.rs
    keygen.rs
    rollover.rs
    oracle.rs
    recovery.rs
```

The split is intentionally modular, but no implementation PR should add this tree in one change.
Each phase below introduces fewer than ten files and aims to stay below 300 lines of production/test
code. Shared harness APIs are stabilized before the independent flow modules are written.

### Core harness types

The exact fields may evolve during implementation, but responsibilities should remain separated:

```rust
struct TestDriver {
    chain: TestChain,
    contracts: Contracts,
    nodes: Vec<TestNode>,
    pending: Vec<PendingCall>,
    faults: FaultPlan,
    trace: Trace,
}

struct TestNode {
    actor: Actor,
    machine: StateMachine<State, Transition, Handler>,
    encoder: Encoder,
    watched_addresses: Vec<Address>,
    database: TestDatabase,
    mode: NodeMode,
}

struct PendingCall {
    actor: Actor,
    transaction: safenet_core::tx::Transaction,
    expires_at: Option<u64>,
    decoded: ProtocolCall,
}
```

`TestNode::new` constructs a real `ValidatorService`, consumes `Service::components()`, builds the
real `StateMachine` over the node's pool, and retains the production encoder. It does not expose the
machine's state to a test.

`Scenario` supplies named deterministic actors (for example Alice, Bob, Carol, Dave, proposer, and
oracle approver), participant epoch windows, short block-based timeouts, allowed oracles, and faults.
Presets such as `four_validators`, `active_genesis`, and `allowed_transaction` perform real chain
work; they never inject the successful events that they claim to establish.

A representative test should read at roughly this level:

```rust
#[tokio::test]
async fn retries_without_a_validator_that_withholds_nonces() -> Result<()> {
    let mut flow = Scenario::four_validators()
        .drop_once(carol(), CallKind::RevealNonceCommitments)
        .start()
        .await?;
    flow.complete_genesis().await?;

    let proposal = flow.propose(allowed_transaction("payment")).await?;
    flow.run_until(Observed::transaction_attested(proposal), 24)
        .await?;

    flow.assert().sign_attempts(proposal, 2);
    flow.assert().attempt_excludes(proposal, 2, carol());
    Ok(())
}
```

### Anvil and contracts

- Spawn one Anvil instance per scenario on an OS-selected random loopback port, with a fixed mnemonic,
  `--no-mining`, and `--order fifo`. Use an RAII process wrapper so panics and early returns kill it.
- Use Alloy's Anvil node binding as a validator dev dependency if it provides the required lifecycle
  and endpoint handling cleanly; otherwise keep a small `std::process::Command` wrapper local to
  `chain.rs`. Do not add a repository-wide process abstraction for this test-only need.
- Run `forge build --root contracts --quiet` once per Rust test binary through a shared one-time cell.
  Foundry artifacts are ignored by Git, so tests must work on a clean checkout and must not rely on a
  developer's existing `contracts/build/out` directory.
- Load creation bytecode from the generated JSON and deploy `FROSTCoordinator`, `Consensus`, and
  `AlwaysApproveOracle` directly through Alloy. Deploy `SimpleOracle` only for scenarios that need a
  delayed/rejected result. Direct deployment avoids concurrent Forge broadcast files and a process
  spawn per scenario.
- Define test-only ABI bindings for external triggers and views not needed by the validator's
  production bindings: genesis start, transaction/oracle proposal, oracle resolution, group and
  signature queries, epoch state, and attestation queries.
- Submit the exact gas limit from the production encoded `Transaction`. An out-of-date gas estimate is
  therefore observable as a failed flow instead of being hidden by RPC gas estimation.

The normal CI job already installs Foundry 1.5.1 before `cargo test --workspace`. Flow tests are part
of ordinary `cargo test --package validator`, are not `#[ignore]`d, and fail with an actionable
message if `forge` or `anvil` is unavailable.

### Log delivery and chain history

The chain wrapper records every canonical block header, receipt, and raw log. It uses the existing
`safenet_core::index::events::Events` implementation on the validator `Event` type for decoding, and
constructs real `EventLog`/`EventUpdate` values with the mined address and log index. Per-node watched
address filtering mirrors production, especially for allowed oracle addresses.

The test block update keeps the safe boundary behind every block needed by the scenario so snapshots
are not prematurely pruned. Reorg tests explicitly record the fork point and first uncled block;
ordinary tests never emit synthetic uncle updates.

An offline node receives no updates. On resume, the driver reconstructs it if requested and replays
the recorded canonical `(New block, Logs)` pairs in order before it may submit new actions. The trace
marks actions produced during catch-up separately, making accidental stale submissions diagnosable.

### Performance and determinism

- Contract compilation is cached once per test binary; deployment and databases remain per scenario.
- A global test semaphore caps concurrent Anvil scenarios to avoid oversubscribing CI. The cap is
  configurable through a test-only environment variable for local profiling.
- The first vertical slice records Anvil startup, deployment, genesis DKG, nonce preprocessing, and
  one signing round separately. The target is a low-single-digit-second happy flow and a complete flow
  suite that remains practical in the normal workspace test and coverage jobs; timing is reported, not
  asserted against a flaky wall-clock threshold.
- Nonce generation is expected to dominate: production creates 1024 unique nonce pairs per chunk.
  The contract requires every inclusion proof to have the full 10-element depth, so the existing
  smaller-tree `NonceChunk::with_size` helper cannot be used for contract-backed flows. Add a
  compile-time test-only constructor that generates a small usable prefix (initially 16 unique nonce
  pairs), pads the leaves to the protocol's full 1024-leaf tree, and stores proofs only for the usable
  prefix. These remain real, unique FROST nonces with valid offset-dependent 10-element proofs; an
  offset outside the prefix has no stored secret and safely produces no action. Production remains
  fixed at 1024 usable nonces. Reduced-prefix scenarios fail fast if they advance beyond the prefix,
  while preprocessing/top-up and nonce-reorg security tests opt into the production path where
  relevant.
- Do not seed the production cryptographic path merely to make snapshots stable. Fixed actors,
  ordered blocks, semantic matchers, and public observations provide deterministic tests without
  coupling to random bytes.

### Diagnostics

Each block trace contains:

- Block/fork number and ordered external/protocol calls.
- Actor, decoded call kind and relevant public protocol IDs.
- Applied fault, expiry, broadcast hash, receipt status, and decoded revert when available.
- Ordered decoded events and the actions queued by each node for the following block.
- Node stop/restart/catch-up boundaries.

Harness errors and assertion failures render the compact trace automatically. Verbose mode may render
full public calldata and Anvil logs, but must never dump DKG coefficients, signing nonces, signing
shares from SQLite, private keys, or database contents. Values already published onchain may be shown.

Every eventual helper takes a maximum block count and reports the last observation plus trace when it
does not converge. Process readiness may use a short bounded RPC probe; protocol progress may not use
sleep-based polling.

### Flow coverage matrix

The matrix is behavior-driven rather than a promise of 100% line coverage. `P0` flows are required for
the epic; `P1` flows are added where the coverage audit shows meaningful reachable gaps.

| Area          | Priority | Flow                                                                                      | Observable result                                                                                        |
| ------------- | -------- | ----------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| Genesis       | P0       | Four validators complete commitments, encrypted shares, confirmations, and preprocessing  | Non-zero group/participant keys; every required confirmation and nonce root landed; no unexpected revert |
| Transaction   | P0       | Allowed external call is proposed and signed                                              | One valid coordinator signature and `TransactionAttested`; Consensus view returns it                     |
| Transaction   | P0       | Forbidden Safe self-call is proposed                                                      | Validators decline; no nonce reveal, signature share, or attestation for the packet                      |
| Transaction   | P0       | Duplicate proposal during an active ceremony                                              | One signing ceremony/attestation; duplicate does not reset progress                                      |
| Transaction   | P1       | Two proposals in the same block                                                           | Independent signing IDs and attestations; no state collision                                             |
| Signing       | P0       | One validator withholds its nonce reveal                                                  | Timeout opens a later attempt without it; threshold signs and attests                                    |
| Signing       | P0       | Enough reveals/shares are missing to lose threshold                                       | Bounded retries stop; no invalid attestation is produced                                                 |
| Signing       | P0       | Valid signature completes but callback is suppressed                                      | `SignCompleted` lands, then direct timeout fallback attests/stages exactly once                          |
| Signing       | P1       | A corrupted outbound nonce/share reverts or is rejected                                   | Expected failure is traced; honest threshold either recovers or safely stops                             |
| Keygen        | P0       | One validator withholds commitment, share, or confirmation during non-genesis DKG         | DKG restarts with a new group excluding it and can still stage the epoch                                 |
| Keygen        | P0       | Too many validators stall                                                                 | No under-threshold group is confirmed or staged; epoch is safely skipped/abandoned                       |
| Keygen        | P0       | Recipient detects a corrupted encrypted share and receives a valid complaint response     | Complaint and response land; DKG completes with the verified revealed share                              |
| Keygen        | P0       | Complaint receives a wrong or no response                                                 | Accused validator is excluded on restart; the abandoned group never confirms or stages                   |
| Rollover      | P0       | Happy epoch DKG, rollover signature, staging, and activation                              | Proposed group is staged with a valid signature and becomes active at the rollover block                 |
| Rollover      | P1       | Participant joins, leaves, or observes outside its epoch window                           | Onchain group membership and submitted actors match configured windows                                   |
| Oracle        | P0       | Allowed synchronous and delayed-approved oracle proposals                                 | Signing begins only after the correct result and produces `OracleTransactionAttested`                    |
| Oracle        | P0       | Allowed oracle rejects or never responds                                                  | No shares/attestation after rejection or timeout; session is eventually dropped                          |
| Oracle        | P0       | Proposal names a disallowed oracle                                                        | Validators ignore it and emit no signing participation                                                   |
| Restart       | P0       | Node restarts after a committed block during DKG and during signing                       | It restores/catches up and completes without duplicate or stale protocol calls                           |
| Reorg         | P0       | Branch containing a DKG setup/commitment is removed and replayed                          | Replayed public commitment is byte-identical because retained secrets are reused                         |
| Reorg         | P0       | Branch burns a nonce for message A; alternate branch uses the same sequence for message B | The node never emits a second signature share using the burned nonce                                     |
| Preprocessing | P1       | Remaining nonces cross the top-up threshold                                               | Exactly one pending chunk is registered/linked and later becomes usable                                  |

Contract-impossible or deliberately malformed histories—wrong event fields that Solidity cannot emit,
events from unwatched addresses, badly sorted updates, or a resume with no matching effect—should not
be forced through Anvil. After the flow suite is in place, retain or add small direct tests for those
defensive invariants and for pure transaction-policy/merkle helpers.

### Definition of done

- Flow modules use only the harness prelude and do not construct or inspect validator `State`.
- Every validator-originated transaction passes through the production `ActionEncoder` and executes
  against the real contracts.
- Successful protocol events are mined from real calls; normal flows contain no scripted logs.
- All eventual behavior is block-bounded, with no protocol sleeps or interval mining.
- Unexpected reverts and timeouts produce a semantic actor/call/event trace.
- The P0 matrix passes under normal `cargo test --package validator` and the Rust coverage job.
- A state enum/layout refactor changes only harness adapters unless observable behavior changes.
- The existing process integration remains as runtime-infrastructure smoke coverage.
- Each implementation PR runs `cargo fmt --all`, `cargo clippy --package validator`, and
  `cargo test --package validator`; the final phase also runs the workspace checks and coverage report.
- This epic specification is removed once every required phase has merged and the definition of done
  is satisfied.

---

## Implementation Phases

Each numbered item is a separate PR unless it explicitly lists sub-PRs. PRs have one primary purpose,
target fewer than 300 changed lines of code and fewer than ten files, and avoid mixing production
refactors with new flow cases. The current epic specification is its own plan-only PR.

### Phase 1 — Contract-backed chain foundation

Add the test module entry point, Anvil RAII lifecycle, one-time Foundry build, artifact loader, direct
contract deployment, deterministic actors, and typed external/view bindings. Prove the layer with a
small deployment/view smoke test; do not add state-machine flows yet.

Expected files: `crates/validator/Cargo.toml`, `Cargo.lock`, `crates/validator/src/main.rs`,
`flow_tests/mod.rs`, and `flow_tests/harness/{mod,chain,contracts}.rs`.

### Phase 2 — Real node adapter and block pump

Build `TestNode` from `ValidatorService::components()`, a per-node SQLite database, the real
`StateMachine`, and the real encoder. Add pending-call collection, deterministic submission/mining,
receipt checking, existing-event decoding, per-node log filtering, and a minimal semantic trace. A
vertical internal test starts genesis and observes that encoded keygen calls mine and feed back into
the nodes; it need not complete DKG yet.

Expected files: `flow_tests/harness/{node,driver,calls,trace}.rs` and small updates to
`flow_tests/harness/mod.rs` and `flow_tests/mod.rs`.

### Phase 3 — Declarative scenario API and happy vertical slice

Add named actors, participant/timeouts builders, Safe transaction fixtures, bounded `run_until`,
public assertions, and `complete_genesis`. Land the first complete P0 flows: happy genesis DKG plus one
allowed transaction attestation. This validates the architecture before fault machinery expands it.

Expected files: `flow_tests/harness/{scenario,fixtures,assertions}.rs`, `flow_tests/genesis.rs`,
`flow_tests/transactions.rs`, and module declarations.

### Phase 4 — Nonce-generation performance seam

Benchmark the Phase 3 slice under normal tests and `cargo llvm-cov`. In a dedicated refactor PR, add a
test-only nonce constructor that generates 16 unique usable nonce pairs but pads the commitment to the
full 1024-leaf protocol tree, plus an effect-handler construction path selecting it. Unit tests must
prove that usable proofs have the contract-required depth, unavailable offsets contain no secret, and
production construction remains at `SEQUENCE_CHUNK_SIZE`. Wire the flow fixture to this reduced-prefix
mode, document its scenario limit, and re-run the real contract proof path. Do not introduce repeated
nonces or a production configuration knob.

Expected files: `crates/validator/src/service/effect.rs`, its focused tests, and
`flow_tests/harness/node.rs` or `fixtures.rs`. If the benchmark shows the full-size path already meets
the agreed suite budget, omit this PR and record that decision in the epic PR thread.

### Phase 5 — Typed faults and diagnostics

Add `ProtocolCall`, call matchers, drop/delay/mute/corrupt/callback-suppression rules, explicit expected
reverts, node controls, and compact trace rendering. Prove each fault primitive with one narrow harness
test; avoid adding the broad behavior matrix in the same PR.

Expected files: `flow_tests/harness/{calls,faults,driver,trace,assertions}.rs` and a small dedicated
harness test module.

### Phase 6 — Transaction and signing flows

Split into two sequential test-focused PRs:

- **6A — transaction intake:** forbidden self-call decline, duplicate proposal, and simultaneous
  proposal flows in `flow_tests/transactions.rs`, with only fixture/assertion extensions needed by
  those behaviors.
- **6B — signing recovery:** missing reveal/share, threshold retained/lost, corrupted submission, and
  callback fallback flows in `flow_tests/signing.rs`, with only fault/assertion extensions needed by
  those behaviors.

Both depend on Phase 5. The implementation should prefer multiple short tests sharing real setup
presets over one long test with many unrelated assertions.

### Phase 7 — Key generation and rollover flows

Split along protocol concerns so complaint cryptography is not mixed with timeout policy:

- **7A — DKG timeout policy:** missing commitment/share/confirmation, one-member exclusion, and
  too-many-members failure in `flow_tests/keygen.rs`.
- **7B — DKG complaints:** corrupted encrypted share, valid response, invalid response, and missing
  response in `flow_tests/keygen.rs`. Extend typed corruption only as required.
- **7C — rollover and membership:** happy stage/activation, callback fallback for staging, join/leave
  windows, observer behavior, and stale attempt abandonment in `flow_tests/rollover.rs`.

These PRs are sequential where they edit the same test module, but 7C can be developed in parallel
with 7B once shared assertion APIs are frozen.

### Phase 8 — Oracle flows

Add on-demand `SimpleOracle` deployment and typed approve/reject external calls. Cover allowed
synchronous approval, delayed approval, rejection, timeout, and disallowed oracle behavior in
`flow_tests/oracle.rs`. Keep oracle contract helpers test-only; no production allow-list semantics
should be duplicated in the harness.

Expected files: `flow_tests/harness/{contracts,fixtures,assertions}.rs`, `flow_tests/oracle.rs`, and
module declarations. This phase can run in parallel with Phase 7 after Phase 5.

### Phase 9 — Restart and reorg recovery

Split persistence and fork mechanics into separate review units:

- **9A — restart/catch-up:** reconstruct nodes over the same database, replay recorded canonical
  blocks, and cover restarts during DKG and signing.
- **9B — reorgs:** add snapshot/revert and uncle delivery, then cover DKG commitment reuse and burned
  nonce non-reuse on an alternate message. These security flows use real/full nonce generation where
  the reduced fixture could hide the property.

Expected files: `flow_tests/harness/{node,driver,chain,trace}.rs` and `flow_tests/recovery.rs`, divided
between the two PRs. Do not add a test-only state-inspection method to prove recovery; use calls,
events, and contract state.

### Phase 10 — Coverage-guided hardening

Run validator line/branch coverage and map uncovered lines back to the behavior matrix. Add P1 flows
where a reachable protocol behavior is missing. Add narrowly scoped direct tests only for impossible
defensive inputs or pure helpers, and document why each cannot be reached through real contracts.
Remove redundant internal-shape tests rather than maintaining two assertions for the same behavior.

Finally, measure the complete validator suite under regular and coverage instrumentation, tune the
scenario semaphore if necessary, run all required format/lint/test commands, and keep a short module
doc describing how future contributors add a scenario without reaching into `State`.

After Phases 1–5 stabilize the harness API, Phases 6A, 7A/7C, and 8 can be assigned in parallel. Reorg
work remains after the block-history abstraction is stable. Each parallel branch should add its own
fixtures locally first and only promote a helper to the shared harness when at least two flow modules
need it.

---

## Open Questions and Assumptions

- **Assumption: four validators are the default scenario.** Their threshold permits tests that lose
  one participant and still complete. Smaller/larger groups are created only when the threshold edge
  itself is under test.
- **Assumption: the core `StateMachine` public update API is sufficient.** The harness does not need
  `Driver::step` to become public and should not add core test hooks unless the Phase 2 vertical slice
  demonstrates a concrete missing capability.
- **Assumption: flow tests are ordinary Rust tests.** CI already installs Foundry 1.5.1, so they should
  not be feature-gated, silently skipped, or moved exclusively to the slower integration workflow.
- **Assumption: one Anvil per scenario is affordable.** Start with a conservative global concurrency
  cap. Reconsider shared snapshots only from measurements, not pre-emptively.
- **Decision gate: reduced usable nonce prefix.** The recommended test prefix is 16 unique nonce
  pairs in a full 1024-leaf tree, provided every reduced-prefix scenario is bounded below its usable
  offsets. Phase 3 must validate a 10-element proof against the real contract and benchmark both test
  and coverage builds before the Phase 4 seam lands.
- **Assumption: cryptographic RNG seeding is unnecessary.** If a nondeterministic crypto failure is
  actually observed, add seed injection as its own reviewable change, keep production entropy as the
  default, print only the seed/public artifacts, and continue asserting semantics rather than exact
  random values.
- **Assumption: transaction-queue persistence is out of scope.** The harness applies action expiry and
  signs/mines encoded calls, but it does not model fee bumping, in-flight limits, durable nonces, or
  restart recovery of queued transactions. Those remain core queue tests plus the process integration
  smoke test.
- **Question for the first coverage audit:** which defensive state branches are impossible to reach
  through the deployed contracts? The answer should be recorded next to any retained direct test so a
  future contract change can promote it to a flow.
- **Question for suite budgeting:** what runtime ceiling should CI enforce socially? Start by
  recording per-phase timings rather than adding wall-clock assertions; use the data to agree on a
  budget before considering shared fixtures or more invasive optimizations.
