# Safenet Rust Codebase Map

> **Scope note (run 2):** `crates/sentinel-engine` is out of scope (PROMPT.md Section 11). Section 6.4, the engine inventory rows and reviewers R8/R9 were removed; remaining mentions of `ENG-H*` leads or the engine are run-1 history.

| Field | Value |
| --- | --- |
| Produced | against commit `82b3e0d` on `main`, by Claude Fable 5.1 acting as Manager plus four read-only per-crate analysis agents |
| Method | Every `.rs` file in the four crates was read in full by an agent; the Manager read the driver, state machine, effect runner, and the validator's crypto and secret-store modules directly and spot-checked the citations behind the top leads of every crate. No toolchain was available: nothing compiled, nothing executed. |
| Status | Reference material for the audit run described in [PROMPT.md](./PROMPT.md). Every "lead" is class `I` or `E2` by inspection and is not a finding until a reviewer, a Critic and, where possible, QA have processed it. |
| Full detail | The four agent reports under [analysis/](./analysis/) (about 260 KB) hold the module maps, interface inventories, invariant tables, panic censuses, per-hypothesis reasoning and rejected non-findings that this map condenses. |

How to use this map: a Reviewer reads Section 1, Section 5, its own subsection of Section 6, Section 8, and then the full analysis file for its crate. A Critic uses the lead tables to know what the reviewer started from, but sets certainty independently. The Coverage Critic uses Section 2 as the canonical file list. The Documentation agent uses Section 8 to group findings by root cause.

## 1. Architecture in one page

All three services are instances of one runtime defined in `safenet-core`:

```text
RPC provider (one chain)
   |  eth_getBlockByNumber / eth_getLogs / eth_call / sendRawTransaction / fee estimates
   v
core::index::Watcher  = BlockWatcher (head, reorg detection, max_reorg_depth) + EventWatcher (ordered decoded logs)
   |  Update::Block{New, Uncle, Warp} | Update::Logs{blocks, events}
   v
core::driver::Driver::run  (single loop; biased select on shutdown; an input, once selected, is processed to completion)
   |  1. TransactionQueue::update_block_status   (reconcile queue with the new chain view)
   |  2. StateMachine::handle_update / handle_resume  (pure, non-fallible transitions; snapshots persisted per block; rolled back on reorg)
   |  3. StateMachine::prune(safe block)
   v
Commands:  Action  -> Service::ActionEncoder -> TransactionQueue (durable SQLite queue, nonce and fee management, expiry) -> RPC
           Effect  -> EffectManager (tokio JoinSet, unordered) -> Resume -> back into the state machine
```

Contracts the runtime documents and every reviewer should hold the code to (`crates/core/src/state/mod.rs`, `effects.rs`, `driver.rs`):

- Transitions are pure and never fail; unexpected input must be handled gracefully inside the transition.
- Effects may run more than once for the same chain message (crash, reorg replay). Handlers encode consumptive outcomes such as "nonce already used" in the resume value.
- Resume ordering is undefined.
- Watcher errors are retried after a short delay, except `ExceededMaxReorgDepth`, which ends the process.
- The transaction queue is reconciled before the state machine advances, so freshly queued actions see the latest chain view.

Service-specific layers:

- Validator: watches `Consensus`, `FROSTCoordinator` (address read from `Consensus.getCoordinator()`), and configured oracles; state machine in `state/` (keygen, preprocess, sign, transactions, epoch); FROST logic in `frost/`; a second, deliberately non-rolled-back `SecretStore` (`secrets/store.rs`) for DKG secrets and signing nonces; background OS threads generating 1024-nonce chunks (`secrets/nonces.rs`).
- Sentinel: watches `SentinelOracle` and `Consensus`; one large state machine in `service.rs`; calls the engine over HTTP (`engine.rs`); commit/reveal votes with bonds; deterministic reveal salt derived from the signer key with HKDF (`hashing.rs`, `core/tx/signer.rs`).
- Sentinel engine: axum server, no keys, no database; a checker chain where cheaper checkers run first and RPC-backed checkers (refund, address poisoning) last (`main.rs`); verdict semantics `secure` / `insecure` with a rule code / `abstain`.

Trust boundaries, by input:

| Input | Enters through | Trust level (assumption) |
| --- | --- | --- |
| Block headers, logs, receipts, fee estimates | `core/provider`, `core/index`, `core/tx` | Semi-trusted RPC (A4) |
| Other participants' onchain messages | `validator/bindings.rs`, `sentinel/bindings.rs` | Adversarial within the fault bound (A2) |
| Proposed Safe transactions | `sentinel/service.rs` -> `sentinel-engine/api` | Fully adversarial (A2) |
| Engine verdicts | `sentinel/engine.rs` | Trusted co-deployed component (A3) |
| Config files, signer keys, SQLite files | `*/config.rs`, `core/utils.rs`, `core/tx/signer` | Trusted operator (A1) |
| HTTP headers `x-request-id`, `x-request-timeout` | `sentinel-engine/api/extractors.rs` | Trusted sentinel, but parse defensively (A3) |

## 2. Inventory (canonical file list for coverage)

Line counts are `wc -l` at the audited commit; tests are `#[test]` and `#[tokio::test]` occurrences in that file. Line coverage is from the CI coverage report posted on PR #896 (`cargo llvm-cov`, unit tests only); `n/a` means the file had no instrumented lines. Zero-coverage files are candidates for QA proof-of-concept tests.

**`core`**

| File                       | LOC  | Tests | Line cov |
| -------------------------- | ---- | ----- | -------- |
| `driver.rs`                | 318  | 0     | 0.0%     |
| `effects.rs`               | 220  | 6     | 99.0%    |
| `index/blocks.rs`          | 1330 | 23    | 99.8%    |
| `index/bloom.rs`           | 523  | 2     | 100%     |
| `index/clock.rs`           | 103  | 2     | 100%     |
| `index/events.rs`          | 1516 | 19    | 98.1%    |
| `index/mod.rs`             | 468  | 5     | 99.2%    |
| `kdf.rs`                   | 81   | 4     | 100%     |
| `lib.rs`                   | 25   | 0     | n/a      |
| `metrics.rs`               | 90   | 0     | 8.8%     |
| `observability/logging.rs` | 21   | 0     | 0.0%     |
| `observability/metrics.rs` | 80   | 1     | 83.8%    |
| `observability/mod.rs`     | 94   | 2     | 77.4%    |
| `provider/mod.rs`          | 166  | 0     | 18.1%    |
| `serialization.rs`         | 34   | 0     | 57.1%    |
| `state/mod.rs`             | 644  | 6     | 99.4%    |
| `state/storage.rs`         | 294  | 7     | 98.0%    |
| `tx/fees.rs`               | 109  | 3     | 100%     |
| `tx/mod.rs`                | 719  | 9     | 95.2%    |
| `tx/signer.rs`             | 118  | 1     | 82.0%    |
| `tx/storage.rs`            | 507  | 7     | 98.9%    |
| `tx/types.rs`              | 87   | 0     | 100%     |
| `utils.rs`                 | 97   | 0     | 0.0%     |

**`validator`**

| File                    | LOC  | Tests | Line cov |
| ----------------------- | ---- | ----- | -------- |
| `bindings.rs`           | 247  | 0     | n/a      |
| `config.rs`             | 290  | 4     | 96.2%    |
| `consensus/epoch.rs`    | 95   | 1     | 42.2%    |
| `consensus/group.rs`    | 459  | 5     | 87.2%    |
| `consensus/hashing.rs`  | 249  | 4     | 90.5%    |
| `consensus/mod.rs`      | 5    | 0     | n/a      |
| `frost/ecdh.rs`         | 181  | 4     | 81.2%    |
| `frost/error.rs`        | 46   | 0     | 50.0%    |
| `frost/keygen.rs`       | 516  | 0     | 86.5%    |
| `frost/marshal.rs`      | 176  | 0     | 94.8%    |
| `frost/mod.rs`          | 258  | 1     | 100%     |
| `frost/participants.rs` | 33   | 1     | 100%     |
| `frost/preprocess.rs`   | 189  | 1     | 77.2%    |
| `frost/sign.rs`         | 204  | 1     | 100%     |
| `main.rs`               | 99   | 0     | 0.0%     |
| `merkle.rs`             | 142  | 4     | 100%     |
| `metrics.rs`            | 132  | 0     | 0.0%     |
| `secrets/mod.rs`        | 6    | 0     | n/a      |
| `secrets/nonces.rs`     | 348  | 3     | 92.6%    |
| `secrets/store.rs`      | 447  | 6     | 97.8%    |
| `service/action.rs`     | 381  | 0     | 0.0%     |
| `service/effect.rs`     | 275  | 0     | 0.0%     |
| `service/mod.rs`        | 129  | 0     | 0.0%     |
| `state/keygen.rs`       | 1459 | 0     | 0.0%     |
| `state/mod.rs`          | 515  | 0     | 0.0%     |
| `state/preprocess.rs`   | 248  | 0     | 0.0%     |
| `state/sign.rs`         | 868  | 0     | 0.0%     |
| `state/transactions.rs` | 101  | 0     | 0.0%     |

**`sentinel`**

| File          | LOC  | Tests | Line cov |
| ------------- | ---- | ----- | -------- |
| `action.rs`   | 43   | 0     | 100%     |
| `bindings.rs` | 170  | 0     | 77.8%    |
| `config.rs`   | 144  | 4     | 86.1%    |
| `effect.rs`   | 134  | 1     | 100%     |
| `engine.rs`   | 392  | 10    | 100%     |
| `hashing.rs`  | 224  | 5     | 100%     |
| `main.rs`     | 89   | 0     | 0.0%     |
| `metrics.rs`  | 134  | 0     | 100%     |
| `service.rs`  | 1851 | 15    | 91.9%    |
| `state.rs`    | 167  | 2     | 87.5%    |

**`sentinel-engine`**

| File                             | LOC  | Tests | Line cov |
| -------------------------------- | ---- | ----- | -------- |
| `api/extractors.rs`              | 69   | 0     | 0.0%     |
| `api/mod.rs`                     | 60   | 0     | 0.0%     |
| `checkers/address_poisoning.rs`  | 457  | 4     | 50.6%    |
| `checkers/base.rs`               | 765  | 20    | 96.2%    |
| `checkers/blocklist.rs`          | 89   | 3     | 93.3%    |
| `checkers/cancellation.rs`       | 72   | 2     | 91.7%    |
| `checkers/cow.rs`                | 1407 | 31    | 96.6%    |
| `checkers/escape_hatch.rs`       | 61   | 0     | 0.0%     |
| `checkers/excessive_approval.rs` | 136  | 4     | 96.3%    |
| `checkers/mod.rs`                | 48   | 0     | 0.0%     |
| `checkers/nested.rs`             | 47   | 0     | 0.0%     |
| `checkers/refund.rs`             | 206  | 7     | 90.9%    |
| `checkers/staking.rs`            | 183  | 0     | 0.0%     |
| `config.rs`                      | 154  | 4     | 91.5%    |
| `contracts/bindings.rs`          | 172  | 0     | n/a      |
| `contracts/mod.rs`               | 5    | 0     | n/a      |
| `contracts/multi_send.rs`        | 186  | 0     | 92.9%    |
| `contracts/target_effects.rs`    | 454  | 14    | 100%     |
| `engine/mod.rs`                  | 121  | 2     | 93.5%    |
| `engine/rule.rs`                 | 164  | 2     | 100%     |
| `engine/transaction.rs`          | 170  | 4     | 100%     |
| `main.rs`                        | 87   | 0     | 0.0%     |

**Non-Rust files in scope**

| File                                     | Lines |
| ---------------------------------------- | ----- |
| `Cargo.toml`                             | 26    |
| `Cargo.lock`                             | 6,169 |
| `crates/core/Cargo.toml`                 | 31    |
| `crates/validator/Cargo.toml`            | 28    |
| `crates/sentinel/Cargo.toml`             | 24    |
| `crates/validator/Dockerfile`            | 37    |
| `crates/sentinel/Dockerfile`             | 38    |
| `crates/validator/validator.sample.toml` | 77    |
| `crates/sentinel/sentinel.sample.toml`   | 61    |

## 3. Toolchain, build and CI facts

- Workspace: Rust edition 2024, resolver 3, four members, no `unsafe` blocks anywhere, `publish = false`.
- Pinned families (`Cargo.toml`): `alloy` 2 (`full`, `json-rpc`), `sqlx` 0.9 (`sqlite`, `runtime-tokio`, system SQLite, not `bundled`), `tokio` 1 (`full`), `axum` 0.8, `reqwest` 0.13 (`rustls`), `k256` 0.13 (`hash2curve`, `serde`), `frost-core` 3 (`internals`), `frost-secp256k1` 3, `hkdf` 0.13, `sha2` 0.11, `rand` 0.8, `rand_chacha` 0.3, `rayon` 1, `metrics` 0.24, `tokio-metrics` 0.5, `tracing-subscriber` 0.3 (`json`), `toml` 1, `argh` 0.1.
- CI (`.github/workflows/ci.yml`, `Justfile`): `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace`, OpenAPI lint. Integration (`integration.yml`): sentinel, sentinel-engine with the external test-vector corpus, validator happy path, validator reorg-nonce regression, validator deep-reorg regression. Coverage via `cargo llvm-cov`.
- Not in CI: `cargo audit` or `cargo deny` (no advisory or licence gate), Miri, fuzzing, sanitizers. `Cargo.lock` has 6,169 lines.
- Images: `rust:1-slim` builder, `debian:trixie-slim` runtime, `ENTRYPOINT` with `--config-file`, CA certificates plus `libsqlite3-0` for validator and sentinel.
- Convention (`AGENTS.md`): sentinel-engine checkers get no unit tests; the external corpus is the oracle (assumption A8). RPC-backed checkers must verify `chain_id` themselves; nothing enforces it centrally.

## 4. Known items (assumption A12)

| Location | Note |
| --- | --- |
| `crates/sentinel/src/main.rs:45` | Startup timing metric should be derived from effect lifecycle data. |
| `crates/sentinel/src/config.rs:44` and `:121` | Default for a zero-address configuration value still to be chosen (epic E2). |
| `epics/2026_07_14_validator_state_machine_flow_test_harness.md` | Validator state machine has little end-to-end behavioural coverage. |

## 5. Cross-cutting checklist (every reviewer)

1. Panics reachable from untrusted input: `unwrap`, `expect`, indexing, slicing, `as` casts, unchecked arithmetic in non-test code. In the engine a panic is an HTTP 500 and a missing vote; in the validator or sentinel it can be a crash loop.
2. Reorg and crash consistency: is every durable write ordered so a crash between "side effect" and "record" is safe? Is every effect idempotent under replay, as `core::state` requires?
3. Trust boundary parsing: chain logs, RPC responses, HTTP bodies, config, DB rows. What is validated, what is assumed.
4. Consensus-critical determinism: hashing, encoding, ordering, and Merkle logic must match the Solidity reference exactly.
5. Secrets: generation entropy, storage, zeroisation, `Debug` and `Display` impls, `tracing` fields, error messages, metrics labels.
6. Concurrency: `select!` cancel-safety, spawned task failure propagation, locks held across awaits, unbounded queues or maps, blocking work (rayon, SQLite) on the async runtime.
7. Resource bounds: per-request and per-block work, RPC call fan-out, retry storms, memory growth over long runs.
8. Configuration: defaults, validation, `deny_unknown_fields`, dangerous combinations, sample files matching the schema.
9. Dependencies: advisories (`cargo audit`), duplicate major versions (`cargo tree -d`), features that widen attack surface.
10. Tests: what the existing tests prove, what they mock away, and whether a proposed fix has a test hook.

## 6. Crate by crate

Each subsection ends with a table of seeded leads. The confidence column is the analysis agent's own estimate and is not the certainty the audit will report; the Critic sets that.

### 6.1 `safenet-core` (7,644 lines, 97 tests)

Purpose: the shared runtime (Section 1). Full analysis: [analysis/analysis-core.md](./analysis/analysis-core.md), including a complete census of every `unwrap`, `expect`, cast and unchecked arithmetic in non-test code (section 8) and the SQLite schema and RPC method inventory (section 3).

Runtime facts a reviewer must hold in mind: one non-spawning driver loop with a biased shutdown select, where an input once selected is processed without cancellation (`driver.rs:170-198`); the indexer keeps a `safe` anchor plus `max_reorg_depth` (default 5) recent headers and unwinds one uncle per call; on restart it re-anchors on the node by block number with no hash check (`index/blocks.rs:244-290`, acknowledged in PR #834); every restart emits a synthetic uncle and replays at least the retained window, re-emitting actions and effects; snapshots are one JSON row per block (`state/storage.rs:50-57`), resumes are not committed until the next log batch; the transaction queue infers execution solely from `eth_getTransactionCount`, allocates `MAX(chain_nonce, MAX(nonce)+1)`, never drops an allocated nonce, and bumps stale in-flight fees by at least 10% with no ceiling; the provider has no retry layer and no request timeout; log completeness is verified only with `use_client_filtering`, and then only for the first `block_single_query_retry_count` attempts (`index/events.rs:369-380`); event decoding is address-agnostic (`index/events.rs:405-409`); fatal exits return code 0 in both binaries.

Hotspots: `index/blocks.rs:244-368` and `421-535` (anchoring, reorg unwinding, revalidation), `index/events.rs:303-469` (fetch strategies, fallback), `state/mod.rs:166-258` (update validation, rollback, resume), `tx/mod.rs:145-296` (per-block reconciliation, submission, replacement), `tx/storage.rs:131-235` (nonce allocation, execution marking), `tx/fees.rs`, `driver.rs:206-287`.

Invariants to verify: persisted state is bound to the canonical chain across restarts; a block's events are never committed as complete when they are not; every fatal path yields a non-zero exit and an unhealthy `/health`; replayed actions cannot cause harmful duplicate submissions; fee replacement is bounded; nonce reconciliation survives inconsistent RPC views; deterministic errors do not become infinite retry loops.

Seeded hypotheses (from the analysis agent; the Manager re-read the citations for CORE-H1, H2, H3 and H4 and they match the code):

| Lead | Location | One line | Agent confidence | Severity guess |
| --- | --- | --- | --- | --- |
| CORE-H1 | `index/blocks.rs:255-266`, `state/storage.rs:50-57` | Restart resumes from an unverified, possibly orphaned snapshot; reorg-depth protection is not persisted (no block hash stored with snapshots). | 85% | High |
| CORE-H2 | `index/events.rs:369-380` | With `use_client_filtering`, the bloom check silently degrades to unverified per-topic queries after three failures, so incomplete logs can be committed. | 80% | High to Medium |
| CORE-H3 | `driver.rs:186-196`, `crates/*/src/main.rs` | Fatal driver errors exit with code 0, so orchestrators and alerts see success. | 95% | Medium |
| CORE-H4 | `index/events.rs:405-409`, `491-519`; `validator/state/mod.rs:415-460` | Address-agnostic decoding lets any watched address, including operator-configured oracle contracts, inject coordinator- or consensus-shaped events into the validator. | 70% | Medium to High |
| CORE-H5 | `index/blocks.rs:261-266`, `tx/storage.rs:96-100` | Restart and rollback replay re-queue actions with fresh nonces; no deduplication, so duplicate onchain transactions are routine. | 75% | Medium |
| CORE-H6 | `tx/fees.rs:53-56`, `tx/mod.rs:224-237` | Replacement bumps compound without a ceiling and the priority-fee cap does not apply to bumps. | 55% | Medium |
| CORE-H7 | `tx/mod.rs:362-368`, `tx/storage.rs:293-305` | First-submission "transaction underpriced" and non-geth messages are not recognised; the row retries forever without a bump and blocks later nonces. | 55% | Medium |
| CORE-H8 | `index/events.rs:491-519`, `driver.rs:206-225` | Deterministic decode or too-many-logs errors are retried forever; indexing stalls while `/health` stays OK. | 70% | Medium |
| CORE-H9 | `index/blocks.rs:504-507` | A lagging backend answering `null` on revalidation produces a spurious uncle and a replay. | 45% | Low to Medium |
| CORE-H10 to H17 | see analysis | Stale resumes after rollback; nonce reconciliation under forked RPC views; no RPC timeouts; wall-clock polling; depth-0 loop; `fallible_events`; HKDF info concatenation; no chain-id binding of the database. | 35% to 60% | Low to Medium |

Considered and rejected by the agent: concurrent nonce allocation races; double execution after a crash between broadcast and recording; `bump` producing an invalid fee pair; the three `expect`s (provably unreachable); rollback to a missing snapshot corrupting the store; prune deleting a needed anchor; out-of-order logs corrupting state (fail-stop instead); effect replay causing FROST nonce reuse (burn-before-use in the validator); key leakage via `Debug` or logs.

Reviewer checklist: items 1 to 16 in the analysis file, section 12. Items 1, 3 and 4 are the ones with consensus impact.

### 6.2 `validator` (8,098 lines, 35 tests)

Purpose: FROST distributed key generation and signing coordinated onchain; epoch rollovers and attestations. Full analysis: [analysis/analysis-validator.md](./analysis/analysis-validator.md), including the enforced-versus-assumed invariant table (section 5), the reorg and restart walkthrough per effect (section 6), and the crypto inventory (section 9).

Runtime facts a reviewer must hold in mind (all cited in the analysis): the state machine is pure and effects are asynchronous, concurrent and not durable, so a restart within about one block loses in-flight `KeyGenSetup` and `NonceTree` resumes; all Rust hashing and encoding (identifiers, leaves, group id, EIP-712, point and scalar marshalling) was checked against the Solidity libraries by reading and no mismatch was found; every secret is sampled from the OS-seeded RNG and persisted in the reorg-immune `SecretStore` with delete-on-use nonces; the ECDH encryption key `q` is separate from `C[0]` (the overview is out of date), has no proof of possession, and the pad `x(q_peer * sk_me)` is symmetric, so the same pad encrypts the share in both directions of every pair (`frost/ecdh.rs:117-129`, `frost/keygen.rs:196-214`); a complaint against this validator triggers an unconditional plaintext share reveal (`state/keygen.rs:734-745`); coordinator and consensus events are dispatched without checking the emitting address (`state/mod.rs:415-460`); `Coordinator.sign` is permissionless and every `Sign` event advances the local nonce sequence before matching.

Hotspots: `frost/ecdh.rs`, `frost/keygen.rs:79-100`, `196-214`, `353-428`, `state/keygen.rs:41-54`, `692-745`, `1003-1024`, `1115-1180`, `state/preprocess.rs:96-102`, `180-247`, `state/sign.rs:30-34`, `106-114`, `282-404`, `522-616`, `service/effect.rs:130-256`, `secrets/store.rs`, `secrets/nonces.rs`, `service/action.rs` (fixed gas limits).

Invariants to verify: a nonce pair is used for at most one signing package under every resume ordering, restart and backup-restore scenario; no encrypted share can be recovered by a participant who does not own the corresponding encryption key; no secret appears in `Debug` output, logs, or metrics (this depends on `frost-core` redaction, which no agent could read offline); a lost effect never leaves the validator permanently unable to participate; rollover never enters a set below the two-thirds bound.

Seeded hypotheses (from the analysis agent; the Manager re-read the citations for VAL-H1, H2 and H5 and they match the code):

| Lead | Location | One line | Agent confidence | Severity guess |
| --- | --- | --- | --- | --- |
| VAL-H1 | `frost/ecdh.rs:110-121`, `frost/keygen.rs:196-214`, `420-428`, `state/keygen.rs:734-745` | No proof of possession on `q` plus a symmetric unhashed pad plus unconditional plaintext complaint responses: a member who republishes a peer's `q` learns that peer's share while staying in the group. | 85% | Critical |
| VAL-H2 | `state/mod.rs:415-463`, `core/index/events.rs:405-408` | Protocol events are not bound to the coordinator or consensus address; an allow-listed oracle contract can inject `KeyGenComplained`, `Sign`, or `SignRevealedNonces` events. Same root as CORE-H4. | 85% given the precondition, 30% that it is realistic | High |
| VAL-H3 | `state/preprocess.rs:96-102`, `199-247`, `service/effect.rs:154-172`, `state/mod.rs:470-479` | A lost or `Unavailable` `NonceTree` effect leaves a phantom chunk reservation that counts as capacity and is never retried; the restart ordering makes this deterministic. | 65% | High |
| VAL-H4 | `state/keygen.rs:41-54`, `1003-1024` | The stuck-setup recovery from PR #851 needs a deadline, but genesis has none; a restart within one block of the genesis `KeyGen` event stalls the network until manual repair. | 55% | High |
| VAL-H5 | `frost/ecdh.rs:110-121`, `frost/keygen.rs:210-213`, `379-381`, `docs/overview.md` | Pairwise pads are two-time pads (both directions) of raw x-coordinates, contradicting the documented one-time-pad argument; the XOR of two shares per pair is public. | 90% violated, 15% exploitable alone | Medium, Critical with H1 |
| VAL-H6 | `state/sign.rs:30-34`, `106-114`, `contracts/src/FROSTCoordinator.sol:530-542` | Anyone can burn every validator's committed nonce sequence by calling `sign` with junk; sessions that land in unlinked chunks are dropped and never rejoined. | 70% | Medium |
| VAL-H7 | `service/effect.rs:202-238`, `state/mod.rs:464-481` | `ReconcileGroupSecrets` computes its retention set from pre-log state and runs concurrently with the same block's store writes. | 30% | Medium |
| VAL-H8, H9 | `state/sign.rs:282-284`, `522-616`; `state/keygen.rs:692-695`, `1083-1090` | Re-revealing nonces lets a signer appoint itself responsible and stall a ceremony; a late complaint excludes the plaintiff and wastes a DKG round. | 50% to 60% | Low |
| VAL-H10 | `service/effect.rs:24-62`, `249`, `frost/keygen.rs:27`, `144`, `302`, `433` | Secret-bearing structs derive `Debug` and are printed at `warn` on effect failure; safe only if `frost-core` redacts. | 25% that secrets print | Critical if they do, else Info |
| VAL-H11 | `secrets/store.rs:198-218`, `docs/validator-handbook.md:75` | Restoring a database backup after a reorg or a ceremony restart can resurrect consumed nonces. | 40% | Medium |

Manager's note on VAL-H1 and VAL-H5: the symmetric pad is a fact of `ecdh.rs` (`receiver_pubkey * sender_privkey` is commutative, and the crate's own `ecdh_is_commutative` test says so). Whether the complaint flow turns it into share recovery, and whether the contract or the Rust side rejects a duplicated `q`, is what the reviewer must establish; grep found no duplicate-`q` check in the Rust keygen code.

Considered and rejected by the agent: marshal canonicality; identifier, leaf, group-id and EIP-712 mismatches; nonce reuse across reorgs through the state machine; stale `Resume::Nonce` on a restarted session (relies on upstream `frost-core`, flagged as a check instead); selection-root ambiguity on timeout; Merkle second-preimage; `as u16` truncation; Lagrange coefficients; rayon blocking the runtime; complaint threshold off-by-one.

Reviewer checklist: items 1 to 14 in the analysis file, section 12. Items 1 to 4 decide the crate's security posture.

### 6.3 `sentinel` (3,348 lines, 37 tests)

Purpose: watches `SentinelOracle` and `Consensus`, asks the engine, commits a bonded vote, reveals, finalises, claims. Full analysis: [analysis/analysis-sentinel.md](./analysis/analysis-sentinel.md).

Runtime facts a reviewer must hold in mind (all cited in the analysis): the request state machine lives in `service.rs` with states `WaitingForEngineCheck`, `WaitingForRequest`, `CollectingCommitments`, `CollectingVotes`, `WaitingForDisputeResolution` (`state.rs:25-77`); the engine is called once per proposal with no retry (`engine.rs:164-194`); the reveal salt is HKDF over the signer key and is never persisted (`hashing.rs:49-53`); approvals are exactly `bondTarget` to the oracle (`service.rs:228-231`, `678-688`); the sentinel bonds on every request unconditionally; on restart the core watcher rolls back to the reorg anchor and warps forward delivering events but no `NewBlock` messages (`core/index/blocks.rs:255-278`, `core/state/mod.rs:173-181`).

Hotspots: `service.rs:295-385` (commit and reveal event handling), `390-470` (block-driven deadlines), `607-672` (`finalize`), `226-242` (bond and commit actions), `engine.rs` (client), core replay semantics.

Invariants to verify: every path where the sentinel has committed also emits a `Reveal`; every terminal path with a non-zero bond emits `Claim`; `Committed`/`Revealed`/`DisputeResolved` are never discarded for a request the sentinel bonded on; actions are safe to replay after reorg or restart; the JSON body matches `openapi.yaml`.

Seeded hypotheses (from the analysis agent; unverified; the numbers are the agent's own confidence):

| Lead | Location | One line | Agent confidence | Severity guess |
| --- | --- | --- | --- | --- |
| SEN-H2 | `service.rs:307-319`, `415-417`; core `state/mod.rs:246-258` | A restart shortly after the engine answers replays our own `Committed` while the state is still `WaitingForEngineCheck`; it is discarded, no reveal follows, the bond is slashed for non-reveal. | 75% | Critical |
| SEN-H1 | `service.rs:307-319`, `372-375`, `631-633` | Commits from other sentinels that land before our engine resumes are not counted, so early finalise fires with `self_revealed == false` and the entry is dropped without a `Claim`. | 85% | High |
| SEN-H3 | `service.rs:347-362`, `626-633`; core `state/mod.rs:173-181` | Warps deliver no `NewBlock`, so reveals and dispute resolutions in the replayed range are ignored and `finalize` takes the wrong branch, claiming on a frozen request and dropping the entry. | 70% | High |
| SEN-H4 | `service.rs:139-143`, `226-242`; core `effects.rs:54-62`, `tx/mod.rs:88` | No bound on concurrent engine checks or outstanding bonds; a proposal flood forces abstention and can push reveals past the deadline. | 60% | High |
| SEN-H5 | `service.rs:226-242`, `676-704` | No balance, allowance, or registration pre-check; a doomed commit still costs an `approve` plus a reverting `commit` per request. | 75% | Medium |
| SEN-H6 | `service.rs:466`, `646-654` | `WaitingForDisputeResolution` has no deadline and the sentinel never calls `timeoutArbitration` (deferred in PR #883). | 90% | Medium |
| SEN-H7 | `service.rs:226-242`, `426-437`, `664-671`; core `tx/storage.rs:89-104` | Actions are not idempotent under replay; reorgs and restarts enqueue duplicate transactions that revert and burn gas. | 90% | Low to Medium |
| SEN-H8 | `service.rs:678-688` | Hard-coded 55,000 gas for `approve` may not fit the deployed fee token. | 55% | Medium if hit |
| SEN-H9 | `service.rs:173-180` | The engine has unbounded authority over bond exposure; no local loss budget or kill switch. | 60% | Medium |
| SEN-H10 to H15 | see analysis | Orphaned engine check after restart; single-attempt client; config not cross-validated; every sentinel submits `Finalize`; `reqwest` defaults; key text not zeroised. | 40% to 95% | Low to Info |

Considered and rejected by the agent (do not re-derive unless you disagree): commit-hash and request-id parity with Solidity (parity vectors on both sides), unlimited approvals, salt predictability, engine-inflated `reason` length, vote alteration after commit, key in logs, panics in non-test code. One open check: whether the pinned `alloy` decodes invalid UTF-8 `string` event fields lossily; if not, a malicious `reason` string could stall every sentinel's indexer (`core/index/events.rs:491-519`).

Reviewer checklist: items 1 to 13 in the analysis file, section 13, in that order.

## 7. Leads from the Manager's own reading

These come from the Manager reading `ecdh.rs`, `nonces.rs`, `merkle.rs`, `preprocess.rs`, `store.rs`, `kdf.rs`, `signer.rs`, `hashing.rs`, `driver.rs` and parts of `keygen.rs` and `utils.rs` directly. Class I or E2 by inspection; nothing executed.

| Lead | Location | Observation | Confidence | What confirms or refutes it |
| --- | --- | --- | --- | --- |
| M1 | `crates/validator/src/frost/keygen.rs:54`, `secrets/store.rs:105-124` | `EncryptionKey::generate` runs once per `keygen::setup`, and the secrets row is keyed by `(group_id, me)` and never overwritten, so a replayed keygen reuses the same key and produces the same ciphertexts. Reuse across two distinct keygens for the same group id (the PR #851 recovery path) is the case to trace. | 35% | Trace every caller of `store_keygen_secrets`; show no two different share values are ever padded with the same `(sk_me, q_peer)` pad. Superseded in importance by VAL-H1 and VAL-H5. |
| M2 | `crates/validator/src/frost/ecdh.rs:117-129` | The pad is a raw x-coordinate, so it is never uniform over 256 bits (about one bit of bias). Negligible alone; part of the VAL-H5 fix (hash the shared secret). | 70% | Informational. |
| M3 | `docs/overview.md` KeyGen section vs `frost/keygen.rs:42`, `bindings.rs:60` | Confirmed documentation drift: the code publishes a dedicated encryption public key `q` in the commitment; the overview still says `C[0]` is reused for ECDH, and its one-time-pad argument no longer matches the code. | 90% | Informational; fix the docs together with VAL-H5. |
| M4 | `crates/validator/src/merkle.rs:87-93`, `frost/preprocess.rs:161-169` | Sorted-pair hashing has no leaf/internal domain tag, but leaves hash 160 bytes while internal nodes hash 64 bytes, so leaf/internal confusion needs a length collision that does not exist. Odd levels pad with `B256::ZERO`; `proof(index)` has no bounds check and silently returns a wrong proof for `index >= leaves.len()`. | 75% | Compare with the Solidity Merkle verifier (same pad value, same ordering); check every `proof()` caller passes a valid index. |
| M5 | `secrets/store.rs:198-218`, `service/effect.rs:190-200`, `state/sign.rs:339-404` | The nonce is deleted from the store before the share is computed and the deleted nonce then lives only in snapshot state, which reorgs roll back. Safety depends on the snapshot never holding an unspent secret nonce that a rollback could revive for a different message. The validator analysis (section 6) argues it holds; a reviewer should re-derive it. | 45% | Trace `Resume::Nonce` into the snapshot: when is the snapshot taken relative to `handle_nonces`; can a rollback restore a state where the nonce is present but the share not yet produced. Related: VAL-H11 (backup restore). |
| M6 | `secrets/store.rs:72-92`, `core/utils.rs:56-62` | `ON DELETE CASCADE` needs `PRAGMA foreign_keys = ON`; `sqlx` enables it by default and `connect_sqlite` does not override it, so this is most likely fine. If wrong, retired groups leave orphaned secret nonces on disk. | 20% | A two-line test deleting a chunk row and counting `nonces`. |
| M7 | `frost/preprocess.rs:105-125` | One `ChaCha12Rng` seed per chunk, per-nonce `set_stream(offset + 1)`: distinct keystreams from one 256-bit seed drawn from the OS-seeded thread RNG. Sound if the seed has full entropy; a seed leak is a chunk leak, and `frost-core` additionally mixes the signing share into each nonce. | 70% | Informational; confirm nothing logs or persists the seed. |
| M8 | `crates/sentinel/src/hashing.rs:16-40`, `core/tx/signer.rs:58-68` | Commit hash binds `reason`; the reveal must present the identical string. The sentinel analysis confirms `approve` and `reason` are carried in state from the moment of the verdict (`state.rs:38-55`) and never re-derived, so this is resolved unless a reviewer finds a path that re-queries the engine. | 15% | Closed by inspection unless contradicted. |
| M9 | `crates/core/src/driver.rs:206-225` | Every watcher error except the deep-reorg one is retried every 100 ms forever with no backoff; combined with CORE-H8 this is how deterministic errors become silent stalls. | 65% | Low; confirm metrics expose the stall (`safenet_core_block_number{status="processed"}` freezing). |
| M10 | `crates/core/src/driver.rs:240-253` | If `TransactionQueue::update_block_status` fails intermittently, the state machine still advances; check that an action encoded against a stale chain view cannot be dropped or double-submitted. | 40% | Read `tx/mod.rs:145-200` reconciliation and expiry logic. |

## 8. Cross-crate themes (root causes that appear in more than one crate)

| Theme | Mechanism owner | Leads | Reviewers |
| --- | --- | --- | --- |
| Restart replay: every restart emits a synthetic uncle and warps forward delivering events but no `NewBlock`; effects are not durable and resumes are not committed until the next log batch. | `core/index/blocks.rs`, `core/state/mod.rs`, `core/effects.rs` | CORE-H5, SEN-H2, SEN-H3, SEN-H7, SEN-H10, VAL-H3, VAL-H4 | R1, R2, R6, R7 |
| Persisted state is bound to block numbers, not hashes, so a reorg that coincides with downtime is invisible. | `core/index/blocks.rs`, `core/state/storage.rs` | CORE-H1, VAL checklist item 13 | R1, R2 |
| Event decoding and dispatch ignore the emitting address; the validator also watches operator-configured oracle contracts. | `core/index/events.rs` | CORE-H4, VAL-H2 | R1, R6 |
| Verdict chain semantics: the first affirming checker suppresses every later denial, and several affirmers ignore `value` and the refund leg. | `sentinel-engine/main.rs`, `engine/mod.rs` | ENG-H1 to ENG-H7 | R8, R9 |
| No timeouts: RPC calls, the engine's HTTP server, the CoW client and effect futures are unbounded. | `core/provider`, `sentinel-engine/api`, `checkers/cow.rs` | CORE-H12, ENG-H10, SEN-H4 | R2, R7, R8 |
| Fatal errors exit with code 0 and `/health` keeps answering OK; deterministic errors retry forever. | `core/driver.rs`, `core/observability` | CORE-H3, CORE-H8, M9 | R2 |
| Secret hygiene depends on upstream `Debug` redaction (`frost-core`) that no agent could read offline. | `validator/service/effect.rs`, `frost/keygen.rs` | VAL-H10 | R4, R10, QA |
| Documentation drift: the overview's ECDH description, the handbook's backoff claim, the engine guide's external-dependency list. | `docs/` | M3, ENG-H14, core analysis section 3.2 | Documentation |

## 9. Reviewer assignments

Ten reviewers, split by risk domain rather than by crate size. A reviewer reads every listed file completely, including tests, plus the referenced sections of the Solidity sources when a lead needs them.

| Reviewer | Scope | Files | Lines | Start from |
| --- | --- | --- | --- | --- |
| R1 | core indexing and reorgs | `core/src/index/{mod,blocks,events,bloom,clock}.rs`, `core/src/provider/mod.rs` | 4,106 | CORE-H1, H2, H4, H8, H9, H14; core checklist 1, 3, 4, 8, 9 |
| R2 | core runtime, state, effects, observability | `core/src/{driver,effects,kdf,utils,serialization,metrics,lib}.rs`, `core/src/state/*`, `core/src/observability/*` | 1,993 | CORE-H3, H5, H10, H12, H13, M9, M10; core checklist 2, 10, 11, 12, 14 |
| R3 | core transaction queue | `core/src/tx/{mod,storage,fees,signer,types}.rs` | 1,540 | CORE-H6, H7, H11; core checklist 5, 6, 7 |
| R4 | validator DKG path | `validator/src/frost/{mod,keygen,ecdh,participants,marshal,error}.rs`, `validator/src/state/keygen.rs`, `validator/src/consensus/{mod,group,epoch}.rs` | 3,228 | VAL-H1, H4, H5, H7, H9, M1, M3; validator checklist 1, 3, 7, 8, 12 |
| R5 | validator signing path and secrets | `validator/src/frost/{preprocess,sign}.rs`, `validator/src/merkle.rs`, `validator/src/secrets/*`, `validator/src/state/{preprocess,sign,transactions}.rs`, `validator/src/consensus/hashing.rs` | 2,802 | VAL-H3, H6, H8, H11, M4, M5, M6, M7; validator checklist 3, 4, 5, 10 |
| R6 | validator service, wiring, config | `validator/src/state/mod.rs`, `validator/src/service/*`, `validator/src/{bindings,config,main,metrics}.rs`, `validator.sample.toml`, `validator/Dockerfile` | 2,068 | VAL-H2, H7, H10, CORE-H4; validator checklist 2, 6, 9, 11, 13 |
| R7 | sentinel | all of `sentinel/src/*`, `sentinel.sample.toml`, `sentinel/Dockerfile` | 3,348 | SEN-H1 to H15, M8; sentinel checklist 1 to 13 |
| R10 | cross-cutting | `Cargo.toml`, `Cargo.lock`, `crates/*/Cargo.toml`, all Dockerfiles and sample configs; plus a repo-wide sweep for secrets in `Debug`, logs and metrics, the panic and cast censuses in each analysis (section 8), `cargo audit` and `cargo tree -d` output from Phase 0, and CI gaps (Section 3) | n/a | VAL-H10, SEN-H14, SEN-H15, CORE-H17, ENG-H14 |

Critics: one per crate (`C-CORE` covers R1 to R3, `C-VAL` covers R4 to R6, `C-SEN` covers R7, `C-ENG` covers R8 and R9), one for R10, and the Coverage Critic. QA: one per crate that has at least one Confirmed or Plausible finding after Phase 2. The Manager may split any reviewer with more than about forty findings or observations into two agents at the same scope boundary.

## 10. Method and limitations

- Read: all 81 Rust files, the Cargo manifests, Dockerfiles, sample configs, the OpenAPI spec, the docs folder, the epic, the CI workflows, the integration scripts, and the Solidity libraries needed to check hashing and encoding parity.
- Not read: the source of `frost-core`, `alloy`, `sqlx`, `reqwest` or any other dependency (no registry on disk), the `sentinel-test-vectors` corpus, and the Safenet Charter text that the engine's rule codes cite. Claims about upstream behaviour are marked as such in the analyses and must be verified once the toolchain is present (`cargo doc --open` or the vendored registry source).
- Not executed: anything. Every line number is from the audited commit and will drift after it; Phase 0 re-checks the inventory.
- Agent output quality: the Manager verified the cited lines of the top three to five leads per crate against the source and found them accurate; the remaining citations are unverified. Hallucinated identifiers or lines are possible and are exactly what the Critic role exists to catch.
