# Run 3 codebase map

| Field | Value |
| --- | --- |
| Audit commit | `361aaf3` (merge of `origin/main` `c355af1` into `audit/rust-services`; outside `rust-audit/` byte-identical to `c355af1`, checked with `git diff --name-only c355af1 361aaf3 -- . ':!rust-audit'`, empty) |
| Written | 2026-10-08 by Map3 (researcher department) |
| Scope | `crates/core`, `crates/validator`, `crates/sentinel` (every `.rs` file), workspace `Cargo.toml` and `Cargo.lock`, the three crates' `Cargo.toml`, both `Dockerfile`s (with their `.dockerignore`), both `*.sample.toml` (PROMPT.md Section 12) |
| Reference only | `contracts/src`, `docs/`, `scripts/`, `AGENTS.md`, `epics/`. `crates/sentinel-engine` is an opaque HTTP service seen from the sentinel |
| Inventory | 75 files: 64 `.rs` files with 23,319 lines (core 10,723, validator 8,221, sentinel 4,375) and 11 other files with 6,526 lines (6,141 of them `Cargo.lock`) |
| Change since run 2 | 50 commits touch the scope in `fe9e84c..361aaf3`; `git diff --shortstat` gives 52 files, 5,323 insertions, 2,472 deletions |
| Sources | The code at `361aaf3`, `git log`, the contracts, `docs/`, `rust-audit/PROMPT.md`, `rust-audit/state/run3/settled.md`, GitHub issues and PRs fetched 2026-10-08 into the run cache (`gh/`) |

This map was written blind: it does not draw on earlier findings, reports, PoCs, the old codebase map or the run-1 and run-2 analyses. It describes code, not defects. Section 4.4 asks questions; none of them is a claim. Line numbers are at `361aaf3`. "Production" or "non-test" lines are the lines before a file's `#[cfg(test)] mod tests`.

## 1. Architecture

### 1.1 Crates and modules

| Crate | Module | Purpose |
| --- | --- | --- |
| `safenet-core` | `driver.rs` | `Driver`: wires watcher, state machine, effects and transaction queue; the run loop (`crates/core/src/driver.rs:172-200`) |
|  | `index/` | `BlockWatcher` (`index/blocks.rs`), `EventWatcher` (`index/events.rs`), the combined `Watcher` (`index/mod.rs`), bloom helper (`index/bloom.rs`), wall clock (`index/clock.rs`) |
|  | `state/` | `StateMachine` with pure transitions (`state/mod.rs`) and the SQLite `SnapshotStore` (`state/storage.rs`) |
|  | `effects.rs` | `EffectHandler` trait and `EffectManager` (a `JoinSet` of effect tasks) |
|  | `tx/` | `TransactionQueue` (`tx/mod.rs`), SQLite queue storage (`tx/storage.rs`), 7702 bundling (`tx/bundle.rs`), config (`tx/config.rs`), fees (`tx/fees.rs`), signer (`tx/signer.rs`), types (`tx/types.rs`) |
|  | `provider/` | `Provider`: alloy `RootProvider` with an observability layer that strips URLs from errors (`provider/mod.rs:120-134`) |
|  | `config.rs`, `serialization.rs` | TOML loading with errors that never echo the document (`crates/core/src/config.rs:21-58`); `${NAME}` substitution for opted-in fields (`crates/core/src/serialization.rs:49-143`) |
|  | `kdf.rs` | HKDF-SHA256 `derive_key` (`kdf.rs:19-27`) |
|  | `observability/`, `metrics.rs`, `utils.rs` | Logging, Prometheus exporter, core metrics, shutdown signal, SQLite pool constructor (`utils.rs:56-62`) |
| `validator` | `main.rs` | Startup: config, provider, pool, coordinator lookup, service, driver, staker reconciliation (`crates/validator/src/main.rs:33-99`) |
|  | `service/` | `ValidatorService` and its `Event` set (`service/mod.rs:26-80`), effect handler (`service/effect.rs`), action encoder (`service/action.rs`) |
|  | `state/` | Snapshotted `State` and dispatch (`state/mod.rs`), DKG and rollover (`state/keygen.rs`), signing (`state/sign.rs`), transaction proposals (`state/transactions.rs`), secret reconciliation (`state/secrets.rs`) |
|  | `frost/` | Safenet layer over frost-core 3.0.0: DKG (`frost/keygen.rs`), signing (`frost/sign.rs`), ECDH share encryption and PoP (`frost/ecdh.rs`), ABI marshalling (`frost/marshal.rs`), identity-tolerant serde (`frost/serialization.rs`), address identifiers (`frost/participants.rs`) |
|  | `consensus/` | Group derivation (`consensus/group.rs`), epoch ids (`consensus/epoch.rs`), EIP-712 hashing (`consensus/hashing.rs`) |
|  | `secrets/` | Reorg-resistant `SecretStore` for DKG secrets and signing nonces (`secrets/store.rs`) |
|  | `merkle.rs`, `bindings.rs`, `config.rs`, `metrics.rs` | Sorted-pair Merkle tree, `sol!` bindings, TOML schema, metrics |
| `sentinel` | `main.rs` | Startup and the engine timeout derivation (`crates/sentinel/src/main.rs:33-93`) |
|  | `service.rs` | Request state machine and action encoder (`SentinelTransition`, `SentinelEncoder`) |
|  | `state.rs` | Snapshotted request states |
|  | `effect.rs`, `engine.rs`, `verdicts.rs` | The one effect (engine check), the HTTP engine client, the reorg-resistant verdict store |
|  | `hashing.rs`, `bindings.rs`, `action.rs`, `config.rs`, `metrics.rs` | Request id and commit hash, `sol!` bindings, actions, TOML schema, metrics |

Pinned library versions (`Cargo.lock`): alloy 2.0.5, frost-core 3.0.0 with the `internals` feature (`crates/validator/Cargo.toml:10`), frost-secp256k1 3.0.0, k256 0.13.4, sqlx 0.9.0 and sqlx-sqlite 0.9.0, libsqlite3-sys 0.37.0, reqwest 0.13.4 (rustls, no default features, `Cargo.toml:15`).

### 1.2 Data path

Both services run the same core loop. The service supplies three components (`crates/core/src/driver.rs:74-95`): a pure `StateTransition`, an `EffectHandler`, an `ActionEncoder`.

1. Startup. `Driver::new` (`driver.rs:122-152`) opens the `StateMachine` from the latest snapshot (`crates/core/src/state/mod.rs:134-156`), reads the snapshot range (`state/storage.rs:86-101`) and passes it to `Watcher::new`, then creates the `TransactionQueue` (`tx/mod.rs:105-147`). The validator creates its `SecretStore` first (`crates/validator/src/service/mod.rs:43-68`); the sentinel creates its `VerdictStore` (`crates/sentinel/src/main.rs:76`).
2. Block watcher. `BlockWatcher::initialize` (`crates/core/src/index/blocks.rs:298-423`) fetches the head, sets `safe = latest.number.saturating_sub(max_reorg_depth)` (`:300`), and on resume queues a synthetic `Uncle { indexed.safe + 1 }` and a `Warp` up to the chain's safe block (`:309-332`); on a fresh start with `start_block` it queues only a `Warp` (`:333-343`). It then scans `safe..=latest` into `recent`, restarting on a parent mismatch (`:348-381`). `next` (`:440-527`) polls the pending block on a wall-clock schedule (`:448-471`), emits `Uncle` when the parent hash does not match the last block (`:476-489`) and fails with `ExceededMaxReorgDepth` when even the safe anchor does not match (`:490-494`).
3. Event watcher. `on_block_update` (`index/events.rs:264-292`) turns a block update into a step: `Warping` pages over a reorg-safe range (`:356-437`) or one `Block` fetched by hash (`:445-490`). `fetch_logs` (`:494-579`) supports one query, one query per topic (dropping `fallible_events` on failure, `:516-528`) and client filtering checked against the header bloom (`:533-558`). `decode_and_sort` (`:675-711`) decodes by topic0 and attaches a timestamp from the log or from a header (#1047 to #1052; header fetch for warp pages `:598-634`, header from the block watcher for new blocks `:466-472`).
4. Watcher. `Watcher::next` (`index/mod.rs:85-97`) drains the event watcher before asking the block watcher; `next_logs` (`:106-130`) handles nodes that expose a block and then answer `ResourceNotFound` for its logs by revalidating it (`blocks.rs:540-592`).
5. Driver input. `next_input` (`driver.rs:208-233`) races the watcher (retrying failures every 100 ms, except a too-deep reorg, `:213-224`) against completed effects (`effects.rs:86-101`).
6. Update. `Driver::update` (`driver.rs:237-297`) first hands the watcher's block status to the queue (`tx/mod.rs:166-228`), then applies the update to the state machine (`crates/core/src/state/mod.rs:171-276`), prunes snapshots below the watcher's safe block (`driver.rs:263`), encodes actions and queues them (`driver.rs:272-290`), spawns effects (`:278`), and after a new block runs effect housekeeping (`:292-294`).
7. State machine. Status values (`crates/core/src/state/mod.rs:111-117`): a new block applies `NewBlock(n)` unless it was already applied optimistically (`:196-211`); logs must be sorted and in range (`:219-223`); warped blocks get a `NewBlock` per block before their logs (`:228-244`, #1017); the snapshot for `blocks.last` is committed (`:257`) and then `NewBlock(pending)` is applied optimistically so its actions can land in the pending block (`:259-268`, #1015). `Uncle(n)` restores the snapshot at `n - 1` (`:187-195`, `state/storage.rs:124-143`). Resumes change live state only (`crates/core/src/state/mod.rs:278-290`).
8. Effects. Spawned into a `JoinSet` (`effects.rs:61-69`); results come back as `Resume` messages. Validator effects: `KeyGenSetup`, `GenerateNonces`, `UseNonces`, `ReconcileGroupSecrets` (`crates/validator/src/service/effect.rs:22-51`). Sentinel effect: `EngineCheck` (`crates/sentinel/src/effect.rs:17-36`). A failed validator effect resumes `Noop` (`service/effect.rs:171-183`).
9. Actions and the queue. `ActionEncoder::encode_action` gives a `Transaction` and an expiry block (validator `service/action.rs:113-352`, sentinel `service.rs:864-937, 1004-1008`). `TransactionQueue::queue` stores them (`tx/storage.rs:112-127`) and calls `submit_pending` (`tx/mod.rs:235-286`), which allocates the next nonce to a bundle of queued transactions (`tx/storage.rs:156-224`), builds fees (`tx/types.rs:116-142`, `tx/fees.rs`), signs (`tx/signer.rs:55-108`) and broadcasts (`tx/mod.rs:308-363`). On each new latest block the queue reads the signer account with `eth_getProof` (`tx/mod.rs:368-386`), marks executed nonces, recovers an unused authorization nonce, resubmits stale transactions with bumped fees and submits more (`tx/mod.rs:207-225`).
10. 7702 batching (#977, #991 to #1005, #1023). With `executor` set (`tx/config.rs:40-75, 101-135`) every transaction is a self-call to the signer's own address carrying `ISafenet7702Executor.execute(calls)` (`tx/bundle.rs:15-29, 94-192`), one transaction in flight (`tx/config.rs:72`), and the first transaction carries an EIP-7702 authorization for the next nonce (`tx/mod.rs:248-258`, `tx/signer.rs:81-105`).

### 1.3 Service state machines

Validator `State` (`crates/validator/src/state/mod.rs:36-53`): `rollover`, `active_epoch`, `epochs` (group and `KeyShare` per participating epoch), `signing` (by message hash), `signature_id_to_message`.

| Machine | States | Entry and exits |
| --- | --- | --- |
| Rollover and DKG (`crates/validator/src/state/mod.rs:67-156`, `state/keygen.rs`) | `WaitingForGenesis`, `CollectingCommitments`, `CollectingShares`, `CollectingConfirmations`, `SigningRollover`, `EpochStaged`, `EpochSkipped`, `Halted` | Genesis starts on the `KeyGen` event for the locally derived genesis group (`state/keygen.rs:41-54`); later epochs start on `NewBlock` (`state/keygen.rs:927-996`). Commitments (`:155-265`), shares and complaints (`:275-427`, `:651-771`), responses (`:779-918`), confirmations (`:430-566`), staging (`:578-642`), timeouts (`:1007-1124`), restarts excluding culprits (`:1203-1240`). Genesis cannot restart and halts (`:1219-1225`, `:1414-1429`) |
| Signing (`crates/validator/src/state/mod.rs:257-356`, `state/sign.rs`) | `WaitingForRequest`, `WaitingForOracle`, `CollectNonceCommitments`, `CollectSigningShares`, `WaitingForAttestation` | Packets enter from `TransactionProposed` (`state/transactions.rs:16-77`) or the last `KeyGenConfirmed` (`state/keygen.rs:509-547`). `Sign` (`state/sign.rs:21-112`), oracle result (`:152-223`), nonce commitments (`:233-332`), own share (`:339-384`), shares (`:390-415`), completion (`:422-455`), attestation (`:460-488`), timeouts and restarts (`:492-693`) |
| Secret reconciliation (`state/secrets.rs:11-85`) | runs on every `NewBlock` | Drops epochs older than any open signing session and the active epoch (`:19-24`), then asks the effect handler to schedule deletion for every group it no longer retains (`:26-83`) |

Sentinel `State` (`crates/sentinel/src/state.rs:178-180`) maps request id to `SentinelRequestState` (`state.rs:22-96`): `WaitingForEngineCheck`, `WaitingForRequest`, `CollectingCommitments`, `CollectingVotes`, `WaitingForOutcome`, `WaitingForDisputeResolution`. Handlers: proposal (`crates/sentinel/src/service.rs:106-154`), engine result (`:159-203`), commit (`:207-253`), `NewRequest` (`:257-300`), `Committed` (`:304-340`), `Revealed` with early finalize (`:344-394`), block deadlines (`:400-511`), dispute resolution (`:524-571`), arbitration timeout and out of scope (`:581-618`), `Claimed` (`:626-635`), finalize (`:655-695`), `RequestTimedOut` (`:710-747`), `DisputeTriggered` (`:766-796`), `OracleResult` (`:819-861`).

### 1.4 Persistence

All tables live in the one SQLite file named by `database` and share one pool (`crates/validator/src/main.rs:46`, `crates/sentinel/src/main.rs:45`, `crates/core/src/utils.rs:56-62`). sqlx-sqlite 0.9.0 defaults: `create_if_missing: false`, `busy_timeout` 5 s, `foreign_keys` on (registry source `sqlx-sqlite-0.9.0/src/options/mod.rs:186-203`); sqlx-core 0.9.0 pool default `max_connections` 10 (`sqlx-core-0.9.0/src/pool/options.rs:151`).

| Table | Schema | Written | Rolled back on reorg |
| --- | --- | --- | --- |
| `snapshots` | `crates/core/src/state/storage.rs:50-55` | After each processed log range (`crates/core/src/state/mod.rs:257`); not after resumes (`:278-290`); deleted above an uncle (`crates/core/src/state/storage.rs:124-143`); pruned below the watcher's safe block, keeping the latest (`crates/core/src/state/storage.rs:151-161`, `crates/core/src/driver.rs:263`) | Yes, by design |
| `allocated_nonces`, `transactions` | `crates/core/src/tx/storage.rs:85-100` | Enqueue (`:112-127`), allocation (`:156-224`), submission (`:230-258`), executed marks (`:281-292`), authorization gap (`:301-312`), prune at safe (`:322-360`), unmark on reorg or startup (`:364-370`, `tx/mod.rs:188-200`) | No. Only the executed markers are cleared |
| `keygen_secrets`, `signing_nonces`, `group_secret_reconciliation` | `crates/validator/src/secrets/store.rs:120-145` | DKG secrets stored once per group and address (`:172-208`); nonces stored once per signature id (`:220-256`) and cleared when taken (`:266-294`); deletion scheduled by block-ordered reconciliation (`:312-352`); collected at or below the safe block in housekeeping (`:361-383`, `service/effect.rs:185-215`) | No, by design (`store.rs:1-41`) |
| `engine_verdicts` | `crates/sentinel/src/verdicts.rs:62-67` | Recorded before the resume, first verdict wins (`:99-121`); `get` raises the retained block (`:78-90`); pruned below the safe block (`:126-132`, `crates/sentinel/src/effect.rs:139-163`) | No, by design (`verdicts.rs:1-24`) |

The snapshot JSON also holds secret material, not only the secret store: DKG `Secrets` while commitments are collected (`crates/validator/src/state/mod.rs:161-176`, type at `frost/keygen.rs:27-33`), `SharingState` with the encryption key and the own round-2 secret package (`crates/validator/src/state/mod.rs:180-188`, `frost/keygen.rs:152-158`), and `KeyShare` (`crates/validator/src/state/mod.rs:58-63, 152, 192-201, 262, 291, 309, 330`, `frost/keygen.rs:432-434`). Plaintext secrets at rest are assumption A1 (`docs/validator-handbook.md:89-91`).

### 1.5 Parameters at the audit commit

| Parameter | Value at `361aaf3` | Source |
| --- | --- | --- |
| `max_reorg_depth` | 5 | `crates/core/src/index/blocks.rs:129-138` |
| Gnosis block timings | 5,000 ms block time, 500 ms propagation delay, retries 500, 500, 500, 1,000, 250, 250 ms | `blocks.rs:56-61` (#1010) |
| Event pages | `block_page_size` 100, `block_single_query_retry_count` 3, `use_client_filtering` false, `max_logs_per_query` none, `fallible_events` empty | `index/events.rs:105-115` |
| Queue | Direct mode, `max_in_flight_transactions` 16, `blocks_before_resubmit` 2, no priority fee cap; Batched `max_batch_gas` 2,000,000, which must not exceed half the block gas limit | `tx/config.rs:7-10, 27-37`, `tx/mod.rs:111-136` |
| Validator timings (blocks) | `blocks_per_epoch` 1440, `key_gen_timeout` 120, `signing_timeout` 6, `oracle_timeout` 24 | `crates/validator/src/config.rs:76-92` |
| Sentinel | `voting_window` mandatory, no default; sample value 6; engine timeout `3/4 * (voting_window - 1) * block_time`, at least 1 s, so 18,750 ms with 6 and 5 s blocks | `crates/sentinel/src/config.rs:36-52`, `crates/sentinel/sentinel.sample.toml:41`, `crates/sentinel/src/main.rs:48-65` |
| Aegis SentinelOracle | commit window 6, reveal window 6, arbitration timeout 483,840 blocks, fee 0.40 USDC.e, bond multiplier 2000, slashing 1 | `contracts/.env.aegis.sample` (`SENTINEL_*`), `docs/configuration.md:44-67` |

The first version of `settled.md` section 3 listed `oracle_timeout` 12 and `voting_window` 100 as values to re-check. Both changed in #1062 (`fcc7028`, 2026-10-07), whose body gives the reasons: 24 blocks cover Aegis's 6 plus 6 windows and `finalize`; 100 would have allowed about 6 minutes per engine check. The values above agree with Recon3's `state/run3/baseline.md` section 6 and with the updated `settled.md` section 3; where they ever differ, `baseline.md` wins.

## 2. Trust boundaries

| Boundary | What crosses it | Where it is parsed or checked | Assumption |
| --- | --- | --- | --- |
| Chain data from other participants | Coordinator events from any account: `keyGen` and `sign` are permissionless (`contracts/src/FROSTCoordinator.sol:336-348, 509-521`); commitments, shares, complaints, responses and nonce commitments come from group members (`FROSTCoordinator.sol:361-380, 417-432, 473-497, 534-540`) and signature shares from any registered key holder (`:560-582`). Consensus `proposeTransaction` from anyone, naming any oracle and any Safe transaction (`contracts/src/Consensus.sol:253-267`). SentinelOracle events from sentinels, the arbitrator and anyone calling `finalize`, `claim`, `timeoutArbitration` (`contracts/src/SentinelOracle.sol:245-350`) | Validator: oracle allow-list and participating epoch (`crates/validator/src/state/transactions.rs:22-40`), group id match on every keygen event (`state/keygen.rs:47, 82, 168, 290, 445, 673, 699, 795, 811`), commitment verification (`frost/keygen.rs:85-107`), share verification (`frost/keygen.rs:266-389`), nonce commitment checks (`state/sign.rs:253-285`), oracle address check (`state/sign.rs:159-168`). Sentinel: configured oracle only (`crates/sentinel/src/service.rs:113-115`), duplicate guard (`:127-134`) | A2 |
| RPC node | Block headers (number, hash, parent, timestamp, bloom), logs and `blockTimestamp`, chain id at connect (`crates/core/src/provider/mod.rs:145-153`), `eth_getProof` nonce and code hash (`tx/mod.rs:368-386`), fee history estimate (`tx/mod.rs:391-414`), latest block gas limit and executor code at startup (`tx/mod.rs:111-136`), `getCoordinator` and `getValidatorStaker` calls at startup (`crates/validator/src/main.rs:48-52, 82-86`) | Parent-hash chaining and reorg depth (`index/blocks.rs:476-494`), sorted and in-range logs (`crates/core/src/state/mod.rs:219-223`), optional bloom check (`index/events.rs:533-558`), optional truncation guard (`:639-651`) | A4 (stale or incomplete, not malicious), A5 |
| Engine HTTP | `POST {engine}/v1/security-check` with the Safe transaction and `block: latest`; headers `x-request-id`, `x-proposal-timestamp`, `x-request-timeout`; response `{"verdict": "secure"}`, `{"verdict": "insecure", "rule": "R-x.y"}` or `{"verdict": "abstain"}` | `crates/sentinel/src/engine.rs:71-83, 101-205`; anything but `secure` or `insecure` is no vote (`engine.rs:185-197`, `service.rs:182-189`) | A3 (`docs/sentinel-engine.md:37`) |
| Config and environment | TOML file; `rpc`, `signer`, `database` may use `${NAME}` (`crates/validator/src/config.rs:11-19`, `crates/sentinel/src/config.rs:11-19`, `crates/core/src/serialization.rs:49-143`); `deny_unknown_fields` with `flatten`; the validator `participants` list must be identical on every validator (`consensus/group.rs:177-217`) | `crates/core/src/config.rs:62-76`, `tx/config.rs:101-135`, `tx/signer.rs:152-172` | A1 |
| SQLite files | Snapshots, queue, secrets, verdicts (section 1.4) | Each store's queries; JSON decode errors are storage errors | A1, A17 |
| Restarts and reorgs | Synthetic uncle at the oldest retained snapshot plus one (`index/blocks.rs:315-320`); live uncles (`:476-489`); deeper reorgs exit (`driver.rs:213-217`); stores outside the snapshot are never rolled back (section 1.4); effects and resumes in flight are lost (`crates/core/src/effects.rs:35-43`, `crates/core/src/state/mod.rs:278-290`) | `crates/core/src/state/mod.rs:187-195`, `crates/core/src/tx/mod.rs:188-200` | A5, A16, A17 |
| Metrics listener | Prometheus text and `/health`, no authentication, default `127.0.0.1:0` (`crates/core/src/observability/mod.rs:16-36`, `observability/metrics.rs:16-48`); the handbooks suggest `0.0.0.0:3555` (`docs/validator-handbook.md:48-53`) | none | Hardening |

### 2.1 Library behaviour reviewers can rely on

- frost-core 3.0.0 redacts secrets in `Debug`: `SigningShare` (`frost-core-3.0.0/src/keys.rs:126-132`), `SigningKey` (`signing_key.rs:100`), round-1 nonces (`round1.rs:277-278`), DKG round-1 coefficients (`keys/dkg.rs:198`) and round-2 secret share (`keys/dkg.rs:345`). Safenet's own secret types implement redacting `Debug` for `EncryptionKey` (`crates/validator/src/frost/ecdh.rs:71-75`), `SigningNonces` (`frost/sign.rs:49-53`) and `Signer` (`crates/core/src/tx/signer.rs:174-178`). Reviewers still need to check every other path (section 4.4, Q57).
- `Signer::from_str` drops the hex error so the key cannot appear in it (`tx/signer.rs:152-163`); TOML parse errors drop the document (`crates/core/src/config.rs:32-49`); transport errors drop the URL (`provider/mod.rs:120-134`).

## 3. Invariants to check

Each line is a property the code relies on, with where it is enforced. Reviewers confirm or break them.

### 3.1 Core

- INV-C1. A snapshot at block `N` exists only after every log of blocks up to `N` was applied, and the optimistic `NewBlock(N + 1)` runs after the commit so a restart replays it (`crates/core/src/state/mod.rs:212-268`).
- INV-C2. Updates arrive in order; anything else is `BadUpdate`, which poisons the machine and stops the driver (`crates/core/src/state/mod.rs:176, 219-223, 272`, `driver.rs:186-199`).
- INV-C3. `Uncle(n)` restores exactly the snapshot at `n - 1`, and pruning never removes a snapshot a reorg within `max_reorg_depth` could need (`state/storage.rs:124-161`, `driver.rs:263`, `index/blocks.rs:504-518`).
- INV-C4. Effects may run more than once per chain message; consumable resources are recorded as outcomes in the resume (`crates/core/src/effects.rs:22-25`, `crates/core/src/state/mod.rs:58-77`).
- INV-C5. A reorg deeper than `max_reorg_depth` stops the service rather than continuing on a wrong branch (`index/blocks.rs:490-494`, `driver.rs:213-217`); `max_reorg_depth = 0` treats any reorg as too deep (`blocks.rs:93-96`).
- INV-C6. Warps and header-by-number lookups only touch the reorg-safe range (`index/blocks.rs:322-343`, `index/events.rs:596-597`); every decoded log carries a timestamp from the log or its block header (`events.rs:687-690`).
- INV-C7. A nonce is allocated once: `next_transaction` picks `max(onchain nonce, max allocated + 1)` inside `BEGIN IMMEDIATE` and an authorization takes the next nonce (`tx/storage.rs:156-224`, `tx/signer.rs:81-86`).
- INV-C8. Executed markers follow the account nonce at the latest block and are cleared above a reorged block and on startup above the safe block (`tx/mod.rs:186-216`, `tx/storage.rs:281-292, 364-370`); queue rows are pruned only below the safe block (`tx/storage.rs:314-360`).
- INV-C9. A replacement raises both fee fields by at least 10 % over the last accepted or underpriced fees (`tx/fees.rs:33-56`, `tx/mod.rs:332-361`).
- INV-C10. Batched mode keeps one transaction in flight, sends self-calls to the signer's own address, and refuses an executor without code, a zero executor and a batch limit above half the block gas limit (`tx/config.rs:62-75, 101-135`, `tx/mod.rs:111-136, 235-286`); chain id 0 is never signed (`tx/signer.rs:59-63`).
- INV-C11. Secrets from config never reach logs or error text (`crates/core/src/config.rs:21-58`, `crates/core/src/serialization.rs:75-134`, `tx/signer.rs:152-178`, `provider/mod.rs:120-134`).

### 3.2 Validator

- INV-V1. Every validator derives the same genesis group from its own config: participants with `active_from == 0`, the genesis salt context, `threshold = count / 2 + 1`, more than two thirds of the set and at least 2 (`crates/validator/src/consensus/group.rs:177-297`, `service/mod.rs:51-57`), matching `FROSTGroupId.create` (`contracts/src/libraries/FROSTGroupId.sol:36-60`) and `docs/overview.md:87-88`.
- INV-V2. Only the `KeyGen` event for that group starts genesis; later groups start on the block clock (`state/keygen.rs:41-54, 927-996`).
- INV-V3. Each commitment carries a valid PoK of `c[0]` and PoP of `q` for the sender's identifier (`frost/keygen.rs:85-107`, `frost/ecdh.rs:113-126`), as in `FROSTCoordinator.keyGenCommit` (`contracts/src/FROSTCoordinator.sol:361-380`); identity coefficients are accepted and persisted (`frost/serialization.rs:1-12`, #1025).
- INV-V4. DKG secrets are sampled once per group and address and reused afterwards; a commitment is never published twice for one group (`secrets/store.rs:164-208`, `state/keygen.rs:91-107, 1012-1039`).
- INV-V5. Shares: `y` equals the verifying share from the group commitment, the encrypted share index follows the sorted participant order without the sender, and each decrypted share verifies against the sender's commitment (`frost/keygen.rs:204-222, 266-389`, `frost/marshal.rs:45-48`).
- INV-V6. Complaints: a participant is excluded once complaints against it reach the threshold, late complaints count without needing a response, and an invalid revealed share excludes the accused (`state/keygen.rs:651-918`), matching `keyGenComplain` and `FROSTParticipantMap.complain` (`FROSTCoordinator.sol:473-483`, `contracts/src/libraries/FROSTParticipantMap.sol:181-193`, #1019).
- INV-V7. Epoch timing agrees with the contract: `next_number(block) = 1 + block / blocks_per_epoch` (`consensus/epoch.rs:6-9`), `rollover_block = epoch * blocks_per_epoch` (`state/keygen.rs:516-518, 1359-1369`), the active epoch advances at the first `NewBlock` at or after the rollover block (`state/keygen.rs:927-961`), and the contract rolls over at `rolloverBlock <= block.number` and accepts proposals only while `rolloverBlock > block.number` (`contracts/src/Consensus.sol:378-397`).
- INV-V8. The validator signs only messages it computed: transaction packets need an allow-listed oracle and a participating epoch (`state/transactions.rs:16-77`); rollover packets come from its own DKG state (`state/keygen.rs:509-547`); `Sign` for an untracked message or another group is ignored (`state/sign.rs:29-109`).
- INV-V9. A transaction packet commits nonces only after an approving `OracleResult` from the packet's oracle within `oracle_timeout` of the `Sign` block, except for ceremonies restarted after the share round (`state/sign.rs:38-62, 152-223, 656-668`).
- INV-V10. A nonce pair is used for at most one signature share: generated per signature id with first-stored-wins, cleared when taken before the share is computed, and a used signature id never gets fresh nonces (`secrets/store.rs:210-294`, `service/effect.rs:123-153`, `state/sign.rs:339-384`, `docs/overview.md:63-83`).
- INV-V11. Only a signer's first valid nonce commitment counts; non-signers and identity points are ignored (`state/sign.rs:253-285`, `frost/marshal.rs:166-181`), consistent with `signCommitNonces` (`FROSTCoordinator.sol:534-540`).
- INV-V12. The selection root and each signer's `(r, l)` are computed identically by every honest signer from the same `committed` set, with leaves as `FROSTSignatureShares._hash` (`frost/sign.rs:92-211`, `crates/validator/src/merkle.rs:13-93`, `contracts/src/libraries/FROSTSignatureShares.sol:79-133`).
- INV-V13. Messages are EIP-712 in the consensus domain `(chainId, verifyingContract)` and equal `ConsensusMessages` (`consensus/hashing.rs:89-169`, `contracts/src/libraries/ConsensusMessages.sol:16-114`); the attest-path hash equals the propose-path hash (`consensus/hashing.rs:116-150`).
- INV-V14. Secrets are deleted only for groups the state machine no longer retains, and only once the deletion block is at or below the safe block (`state/secrets.rs:11-85`, `secrets/store.rs:296-419`, `service/effect.rs:185-215`, `docs/validator-handbook.md:93-108`).

### 3.3 Sentinel

- INV-S1. The sentinel tracks only proposals for its configured oracle, and its request id equals the oracle's `requestId`, the EIP-712 `TransactionProposal` hash in the consensus domain (`crates/sentinel/src/service.rs:113-123`, `crates/sentinel/src/hashing.rs:80-102`, `contracts/src/Consensus.sol:253-267`).
- INV-S2. The revealed `(approve, reason)` equals the committed one: the verdict is recorded before the resume and never replaced (`crates/sentinel/src/verdicts.rs:92-121`, `crates/sentinel/src/effect.rs:65-119`); the salt is a deterministic HKDF of the signer key and the request id (`crates/sentinel/src/hashing.rs:11-53`, `crates/core/src/tx/signer.rs:118-132`); the commit hash is `keccak256(abi.encodePacked(approve, salt, sentinel, requestId, reason))` (`crates/sentinel/src/hashing.rs:16-38`, `contracts/src/libraries/SentinelOracleCommitments.sol:47-56`).
- INV-S3. No verdict, no vote: abstain, transport errors, timeouts and storage errors resolve to `Unknown` and drop the request (`crates/sentinel/src/engine.rs:173-204`, `crates/sentinel/src/effect.rs:72-119`, `crates/sentinel/src/service.rs:182-189`).
- INV-S4. Each action fits its contract window: commit while `block <= commitDeadline`, reveal while `commitDeadline < block <= revealDeadline`, finalize after the reveal deadline or once everyone revealed, `timeoutArbitration` after the arbitration deadline (`service.rs:400-511, 344-394`, action expiries `:240, 248, 444`; `contracts/src/libraries/SentinelOracleRequests.sol:112-137, 163-214, 224-227`).
- INV-S5. Every path on which this sentinel has a bond onchain ends in its own `claim` (`service.rs:513-861`, `state.rs:111-151`, `contracts/src/SentinelOracle.sol:303-322`).

## 4. Hotspots and questions

### 4.1 Changes since `fe9e84c`, by stack

| Stack | PRs | Files |
| --- | --- | --- |
| 7702 batching | #977, #991 to #1003, #1005, #1023 (#1004 changed only `scripts/`) | `crates/core/src/tx/{mod,storage,bundle,config,types,signer}.rs`, `crates/core/src/metrics.rs`, both samples and configs |
| Direct Nonce (per-ceremony nonces, preprocessing removed) | #1032 to #1044 (#1037 changed only `scripts/`) | `crates/validator/src/{frost/sign.rs, frost/mod.rs, secrets/store.rs, service/effect.rs, service/action.rs, state/sign.rs, state/mod.rs, state/secrets.rs, metrics.rs, bindings.rs}`; deleted `frost/preprocess.rs`, `secrets/nonces.rs`, `state/preprocess.rs` |
| New Block | #1014, #1015, #1017 | `crates/core/src/state/mod.rs`, `crates/sentinel/src/service.rs` |
| Event Timestamps | #1047 to #1052 | `crates/core/src/index/{events,mod,blocks}.rs`, `crates/core/src/state/mod.rs`, `crates/sentinel/src/{effect,service}.rs`, `crates/validator/src/state/mod.rs` |
| Verdict store | #978, #979 | `crates/sentinel/src/{verdicts,effect,engine,main,service}.rs` |
| KeyGen PoK, PoP, complaints, identity | #1019, #1025, #1027 | `crates/validator/src/{frost/ecdh.rs, frost/keygen.rs, frost/marshal.rs, frost/serialization.rs, frost/mod.rs, state/keygen.rs, state/mod.rs, bindings.rs}` |
| Config secrets | #1007, #1046 | `crates/core/src/{serialization,config,provider/mod,tx/signer}.rs`, both `config.rs` and samples |
| Signing recovery | #1008, #1009 | `crates/validator/src/state/{sign,keygen,mod,transactions}.rs` |
| Other | #974, #975, #1010, #1062 | sentinel request handling and engine timestamps; per-network block timings (`index/blocks.rs`); Aegis defaults (`crates/validator/src/config.rs`, `crates/sentinel/src/config.rs`, samples) |

Code with no change since `fe9e84c` and therefore last reviewed in run 2's tree: `crates/core/src/{driver.rs, index/bloom.rs, index/clock.rs, kdf.rs, observability/*, state/storage.rs, tx/fees.rs, utils.rs}`, `crates/validator/src/{consensus/*, frost/error.rs, frost/participants.rs, main.rs, merkle.rs, service/mod.rs}`, `crates/sentinel/src/{hashing.rs, metrics.rs}`, `crates/sentinel/Cargo.toml`, both Dockerfiles and `.dockerignore` files. Unchanged code can still interact with changed code.

### 4.2 Churn per file since `fe9e84c`

`git diff --numstat fe9e84c 361aaf3` and `git log --format=%s fe9e84c..361aaf3 -- <file>`, in-scope files that changed:

| File | Added | Removed | PRs since `fe9e84c` |
| --- | --- | --- | --- |
| `crates/core/src/tx/mod.rs` | 654 | 87 | #991, #992, #994, #995, #996, #997, #998, #999, #1000, #1001, #1002, #1005, #1023 |
| `crates/core/src/index/events.rs` | 619 | 84 | #975, #1047, #1048, #1049, #1050, #1051, #1052 |
| `crates/core/src/tx/storage.rs` | 565 | 82 | #977, #993, #995, #996, #997 |
| `crates/validator/src/secrets/store.rs` | 330 | 243 | #1033, #1035, #1039, #1040, #1041, #1044 |
| `crates/core/src/tx/bundle.rs` (new file) | 407 | 0 | #994, #995, #997, #1001, #1003 |
| `crates/validator/src/state/sign.rs` | 154 | 186 | #1008, #1009, #1032, #1036, #1038, #1041, #1042 |
| `crates/sentinel/src/service.rs` | 91 | 224 | #974, #975, #979, #1014, #1047, #1052 |
| `crates/sentinel/src/verdicts.rs` (new file) | 268 | 0 | #978 |
| `crates/validator/src/state/keygen.rs` | 114 | 126 | #1009, #1019, #1025, #1038 |
| `crates/core/src/index/blocks.rs` | 166 | 67 | #1010, #1048 |
| `crates/core/src/tx/signer.rs` | 203 | 24 | #998, #1007, #1023 |
| `crates/sentinel/src/effect.rs` | 168 | 39 | #975, #979, #1052 |
| `crates/core/src/tx/config.rs` (new file) | 194 | 0 | #991, #992 |
| `crates/validator/src/service/effect.rs` | 50 | 143 | #1033, #1035, #1036, #1038, #1039, #1040, #1041, #1042, #1044 |
| `crates/core/src/serialization.rs` | 184 | 0 | #1007 |
| `crates/core/src/config.rs` (new file) | 157 | 0 | #1046 |
| `crates/validator/src/frost/ecdh.rs` | 143 | 8 | #1027 |
| `crates/core/src/state/mod.rs` | 118 | 31 | #975, #1015, #1017, #1047, #1048, #1052 |
| `crates/validator/src/frost/mod.rs` | 92 | 25 | #1025, #1032, #1040, #1042 |
| `crates/validator/src/state/mod.rs` | 39 | 68 | #1008, #1009, #1019, #1025, #1035, #1036, #1038, #1040, #1041, #1042, #1047 |
| `crates/core/src/tx/types.rs` | 89 | 10 | #995, #997, #999, #1000 |
| `crates/validator/src/frost/sign.rs` | 61 | 29 | #1032, #1035, #1040, #1042, #1044 |
| `crates/validator/src/state/secrets.rs` (new file) | 86 | 0 | #1041 |
| `crates/validator/src/frost/serialization.rs` (new file) | 85 | 0 | #1025 |
| `crates/sentinel/src/engine.rs` | 54 | 16 | #975, #978 |
| `crates/validator/src/service/action.rs` | 6 | 34 | #1034, #1036, #1038, #1040 |
| `crates/core/src/index/mod.rs` | 28 | 11 | #975, #1010, #1047, #1048, #1052 |
| `crates/validator/src/frost/marshal.rs` | 21 | 16 | #1027 |
| `crates/validator/src/frost/keygen.rs` | 17 | 18 | #1025, #1027 |
| `crates/sentinel/sentinel.sample.toml` | 31 | 3 | #992, #1007, #1062 |
| `Cargo.lock` | 2 | 32 | #1040, #1046 |
| `crates/validator/src/config.rs` | 11 | 20 | #992, #1007, #1046, #1062 |
| `crates/validator/validator.sample.toml` | 28 | 1 | #992, #1007, #1062 |
| `crates/validator/src/metrics.rs` | 9 | 17 | #1033, #1035, #1040, #1041, #1042 |
| `crates/sentinel/src/config.rs` | 8 | 17 | #1007, #1046, #1062 |
| `crates/core/src/provider/mod.rs` | 18 | 2 | #1046 |
| `crates/core/src/metrics.rs` | 18 | 1 | #1003 |
| `crates/validator/src/bindings.rs` | 8 | 10 | #1027, #1034, #1040, #1042 |
| `crates/sentinel/src/state.rs` | 12 | 1 | #974 |
| `crates/sentinel/src/main.rs` | 6 | 2 | #978, #979, #1010 |
| `crates/sentinel/src/action.rs` | 3 | 0 | #974 |
| `crates/validator/Cargo.toml` | 0 | 2 | #1040 |
| `crates/core/src/effects.rs` | 1 | 1 | #1043 |
| `crates/core/Cargo.toml` | 2 | 0 | #1046 |
| `crates/validator/src/state/transactions.rs` | 1 | 0 | #1009 |
| `crates/validator/src/secrets/mod.rs` | 0 | 1 | #1040 |
| `crates/sentinel/src/bindings.rs` | 1 | 0 | #974 |
| `crates/core/src/lib.rs` | 1 | 0 | #1046 |
| `Cargo.toml` | 0 | 1 | #1040 |

### 4.3 Longest functions (non-test code)

Brace-matched spans, from a script over the tree at `361aaf3`. Length is a proxy for review effort, not for risk.

| Lines | Function | Location |
| --- | --- | --- |
| 239 | `encode_action` | `crates/validator/src/service/action.rs:114-352` |
| 202 | `handle_signing_timeouts` | `crates/validator/src/state/sign.rs:492-693` |
| 153 | `handle_key_gen_secret_shared` | `crates/validator/src/state/keygen.rs:275-427` |
| 140 | `handle_key_gen_complaint_responded` | `crates/validator/src/state/keygen.rs:779-918` |
| 137 | `handle_key_gen_confirmed` | `crates/validator/src/state/keygen.rs:430-566` |
| 126 | `initialize` | `crates/core/src/index/blocks.rs:298-423` |
| 121 | `handle_key_gen_complained` | `crates/validator/src/state/keygen.rs:651-771` |
| 118 | `handle_key_gen_timeouts` | `crates/validator/src/state/keygen.rs:1007-1124` |
| 112 | `handle_block_advance` | `crates/sentinel/src/service.rs:400-511` |
| 111 | `handle_key_gen_committed` | `crates/validator/src/state/keygen.rs:155-265` |
| 106 | `handle_update` | `crates/core/src/state/mod.rs:171-276` |
| 100 | `handle_sign_committed_nonces` | `crates/validator/src/state/sign.rs:233-332` |
| 92 | `handle_sign` | `crates/validator/src/state/sign.rs:21-112` |
| 88 | `next` | `crates/core/src/index/blocks.rs:440-527` |
| 86 | `signature_share` | `crates/validator/src/frost/sign.rs:92-177` |
| 86 | `fetch_logs` | `crates/core/src/index/events.rs:494-579` |
| 85 | `handle_key_gen_setup` | `crates/validator/src/state/keygen.rs:65-149` |
| 83 | `apply_transition` | `crates/validator/src/state/mod.rs:390-472` |
| 82 | `warp` | `crates/core/src/index/events.rs:356-437` |
| 75 | `handle_group_reconciliation` | `crates/validator/src/state/secrets.rs:11-85` |
| 73 | `encode_action_kind` | `crates/sentinel/src/service.rs:865-937` |
| 72 | `handle_oracle_result` | `crates/validator/src/state/sign.rs:152-223` |
| 72 | `generate_secret_shares` | `crates/validator/src/frost/keygen.rs:170-241` |
| 70 | `handle_rollover_new_block` | `crates/validator/src/state/keygen.rs:927-996` |
| 69 | `next_transaction` | `crates/core/src/tx/storage.rs:156-224` |

Census of panic sites and casts in non-test code (input to Q58 and Q59): 13 `expect`, 7 `unwrap` (4 of them in `const` defaults, `crates/validator/src/config.rs:78-90`, and 1 in a test-only helper, `frost/keygen.rs:446`), 1 `assert!` (`crates/core/src/kdf.rs:20`), 0 `unsafe`, 34 `as` casts in `crates/core/src/{driver.rs, index/blocks.rs, index/clock.rs, index/mod.rs, tx/bundle.rs, tx/fees.rs}`, `crates/sentinel/src/service.rs`, `crates/validator/src/{consensus/group.rs, consensus/hashing.rs, secrets/store.rs, service/action.rs, state/keygen.rs, state/sign.rs}`.

### 4.4 Questions for reviewers

Questions only. Each has an owner (section 6). Check `settled.md` before filing.

#### Core indexing (R1)

- Q1. On restart the block watcher replays from the oldest retained snapshot with a synthetic `Uncle` (`crates/core/src/index/blocks.rs:309-332`), and snapshots store no block hashes (`crates/core/src/state/storage.rs:50-55`). Is a reorg that happened while the service was down, and that replaced blocks at or below the oldest retained snapshot, detected, or replayed as canonical?
- Q2. Warp pages have no completeness check: the bloom comparison only runs for client-filtered new blocks (`index/events.rs:533-558`) and `max_logs_per_query` defaults to none (`events.rs:111`). Under A4, what limits the effect of a warp page that silently misses logs?
- Q3. A log with a watched topic0 that does not decode fails the whole page (`events.rs:675-711`), and the warp retries with halved pages forever (`events.rs:416-431, 654-667`). Can a watched contract emit such a log, and does the driver then stall?
- Q4. `watcher_events!` decodes by topic0 and tries each contract's event enum in order (`events.rs:749-785`); the validator dispatches Coordinator and Consensus events without looking at `log.address` (`crates/validator/src/state/mod.rs:396-441`) and passes the address only for `OracleResult` (`:437-438`). Do any two watched contracts share an event signature (validator: Consensus, FROSTCoordinator, configured oracles, `crates/validator/src/main.rs:56-57`; sentinel: SentinelOracle and Consensus, `crates/sentinel/src/main.rs:84`)?
- Q5. Warp logs without `blockTimestamp` get a header fetched by number (`events.rs:598-634`); new-block logs get the block watcher's header timestamp (`events.rs:466-472`). Can a log ever carry the timestamp of a different block, and what consumes it (`crates/sentinel/src/service.rs:954-963`, engine header `x-proposal-timestamp`)?
- Q6. While a warp is queued, `revalidate_last_block` returns `Ok(None)` (`blocks.rs:541-547`) and `next_logs` returns the original error (`index/mod.rs:124-125`), which the driver retries every 100 ms (`crates/core/src/driver.rs:218-224`). Is there an RPC answer sequence within A4 that loops here forever, or that makes `on_block_invalidated` fail with `UnexpectedBlockInvalidation` (`events.rs:299-310`)?
- Q7. Does a warp now produce the same transitions at the same blocks as live processing, given the per-block `NewBlock` (`crates/core/src/state/mod.rs:228-244`, #1017) and the optimistic `NewBlock(pending)` after each page (`crates/core/src/state/mod.rs:259-268`)?
- Q8. When the block retries run out, the watcher moves `pending.timestamp_ms` forward by one block time (`blocks.rs:448-471`). After a long RPC outage or several skipped slots on Gnosis (`blocks.rs:56-61`), can block discovery fall behind or stop?

#### Core runtime (R2)

- Q9. Resumes change live state without a snapshot (`crates/core/src/state/mod.rs:278-290`), while the actions they produce go straight to the persistent queue (`crates/core/src/driver.rs:272-290`). After an `Uncle` restores a snapshot from before the resume, which validator or sentinel transitions can queue a second, different action for the same purpose? Open issue #973 describes the sentinel case.
- Q10. A panicked effect task is logged and skipped (`crates/core/src/effects.rs:86-101`), so its resume never arrives. Which states wait on a resume with nothing else to move them (for example validator `CollectingCommitments` with `secrets: None`, sentinel `WaitingForEngineCheck`), and what clears them?
- Q11. Which inputs within A2 or A4 can make `StateMachine::handle_update` fail (`crates/core/src/state/mod.rs:171-276`), given that any failure poisons the machine (`:176`) and the driver exits (`driver.rs:186-199`) with status 0 (known item)?
- Q12. The optimistic `NewBlock(pending)` (`crates/core/src/state/mod.rs:259-268`) runs before the pending block's events are known. Which irreversible actions does `NewBlock` emit (validator `crates/validator/src/state/sign.rs:492-693`, `state/keygen.rs:927-996, 1007-1124`; sentinel `crates/sentinel/src/service.rs:400-511`), and is each still right when that block carries an event that would have changed the decision?
- Q13. Both top-level configs combine `deny_unknown_fields` with `#[serde(flatten)]` (`crates/validator/src/config.rs:8-29`, `crates/sentinel/src/config.rs:8-32`), as does the flattened `[index]` table (`crates/core/src/index/mod.rs:19-29`). Does a misspelled key, for example `use_client_filter` or `max_reorg_dept`, fail startup or vanish silently?
- Q14. `from_str_with_env` maps the target type's parse error with `de::Error::custom` (`crates/core/src/serialization.rs:60-70`). For `Url` and `SqliteConnectOptions`, can that text contain the substituted secret, and does `ParseError` then print it (`crates/core/src/config.rs:26-58`)?
- Q15. On SIGTERM the driver finishes the current update and drops the `EffectManager`, which aborts running effects (`driver.rs:172-200`, `effects.rs:35-43`). Which effects can be aborted after their database write committed and before their resume was applied (for example `take_signing_nonces`, `crates/validator/src/secrets/store.rs:266-294`), and is the cost only liveness?
- Q16. Which actions are queued with no expiry (validator genesis actions through `deadline: None`, `state/keygen.rs:97-106`, and `SetValidatorStaker`, `crates/validator/src/service/action.rs:340-350`; sentinel `TimeoutArbitration`, `Claim` and `Finalize`, `service.rs:494-505, 561-569, 610-616, 680-686, 737-745, 851-859`), and how many duplicates can a restart replay (`crates/core/src/index/blocks.rs:315-320`) or a reorg add?

#### Core transaction queue (R3)

- Q17. Each resubmission raises fees by at least 10 % (`crates/core/src/tx/fees.rs:33-56`) every `blocks_before_resubmit` blocks (default 2, `tx/config.rs:27-37`), and the priority fee cap applies only to fresh estimates (`tx/mod.rs:391-414`). Is there a ceiling for a transaction the mempool accepts but never includes, for example one behind a nonce gap, and what does it cost at Gnosis prices before anything stops it?
- Q18. `priority_fee_cap_percentage` is an unchecked `f64` (`tx/config.rs:24, 85`). What does `cap_priority_fee` (`tx/fees.rs:12-31`) do with NaN, negative, infinite or above-100 values?
- Q19. Executed marks come from the account nonce returned by `eth_getProof` at `block_status.latest` (`tx/mod.rs:207-216, 368-386`). With a lagging or reorging node (open issue #961 shows executed nonces being resubmitted), can a transaction be marked executed when it is not, or stay unexecuted when it is, and what does each case do to allocation (`tx/storage.rs:190-195`) and pruning (`:322-360`)?
- Q20. Underpriced rejections are recognised by matching error text (`tx/mod.rs:417-437`). Which messages from the clients behind Gnosis RPC endpoints (Nethermind, Erigon, Reth) does this miss, and does a miss leave the fee floor stuck?
- Q21. Expiry only filters rows that have no nonce yet (`tx/storage.rs:172-178, 212-220`); an allocated transaction is resubmitted until executed, after its expiry too. Which actions are harmful or wasteful when they land late (validator `CommitNonces`, `SignShare`, `Sign`; sentinel `Reveal` after `revealDeadline`)?
- Q22. The authorization-gap recovery turns the delegate row into `Transaction::default()`, a zero-value transfer to the zero address with 21,000 gas (`tx/storage.rs:294-312`, `tx/types.rs:30-39`). In Batched mode, is that row sent as a plain transaction or bundled, and is it valid while the account is delegated (`tx/mod.rs:288-304`)?
- Q23. Does the batch gas formula (`tx/bundle.rs:49-75, 195-209`) leave enough gas for the executor's per-call check (`contracts/src/Safenet7702Executor.sol:96-111`) and cold access for the largest validator actions (`crates/validator/src/service/action.rs:138, 147, 268`)? The missing worst-case test is settled item L5; look for a concrete shortfall.
- Q24. `next_transaction` streams rows into the bundler and then sets `nonce` on `id <= last` with the expiry filter repeated (`tx/storage.rs:165-221`). Can the update select a different row set than the bundler took?

#### Validator DKG (R4)

- Q25. Commitments and confirmations are counted by address with no local membership check (`crates/validator/src/state/keygen.rs:161-213, 445-468`). Which contract checks make that safe (`contracts/src/libraries/FROSTParticipantMap.sol:146-153, 220-226`), and can the local share count (`state/keygen.rs:298-369`) diverge from the contract's `pending` counter (`contracts/src/FROSTCoordinator.sol:417-432`, `FROSTParticipantMap.sol:161-168`)?
- Q26. The PoP of `q` and the PoK of `c[0]` use the same proof and challenge (`crates/validator/src/frost/ecdh.rs:41-57, 113-126`, `frost/keygen.rs:95-100`, `FROSTCoordinator.sol:375-376`, `contracts/src/libraries/FROST.sol:213-246`). Can one stand in for the other, and does `q == c[0]` or a copied `q` weaken share encryption for anyone but the sender?
- Q27. The position of this validator's share in a sender's `f` vector (`frost/keygen.rs:361-377`) must match the publisher's order (`frost/keygen.rs:204-222`, `crates/validator/src/frost/marshal.rs:45-48`). Does it hold for every group size and exclusion set, including the lowest and highest addresses?
- Q28. Identity coefficients are now persisted (#1025, `crates/validator/src/frost/serialization.rs:1-12`). Can a participant make the group key, a verifying share or `y` the identity, and does frost-core 3.0.0 reject that in `VerifyingKey::from_commitment` and `VerifyingShare::from_commitment` (`frost/keygen.rs:133-144, 195-200, 276-288`)?
- Q29. If `keyGenConfirmWithCallback` reverts because `Consensus.proposeEpoch` rejects a late rollover (`contracts/src/Consensus.sol:217-224, 395-397`), the last confirmation never lands. Does the confirm timeout then exclude an honest validator (`state/keygen.rs:1085-1110`), and can that repeat each epoch?
- Q30. Late complaints count but need no response (`state/keygen.rs:704-729`) and only recorded unresponded complaints accept a response (`state/keygen.rs:835-842`). Does that match `FROSTParticipantMap.complain` and `respond` (`FROSTParticipantMap.sol:181-210`) in every phase, including a complaint raised while collecting shares and answered while collecting confirmations?
- Q31. A restart or reorg can resume `KeyGenSetup` after the state moved on (`state/keygen.rs:65-82`); stored secrets are reused per group and address (`crates/validator/src/secrets/store.rs:164-208`). Can a commitment be published twice for one group with different secrets? (A prune of key-gen secrets during a replay is a known, unverified item.)
- Q32. Validators with different `participants` lists or `active_from`/`active_before` windows (`crates/validator/src/config.rs:94-108`, `crates/validator/src/consensus/group.rs:177-217`): does the mismatch fail loudly, or does each derive a different group id (`group.rs:249-264`, `contracts/src/libraries/FROSTGroupId.sol:36-60`)?

#### Validator signing (R5)

- Q33. `Resume::Nonces` carries only `message` (`crates/validator/src/service/effect.rs:82-86`), and `handle_nonces` signs with the `signature_id` and `committed` set the state holds for that message when the resume arrives (`crates/validator/src/state/sign.rs:339-384`). Can nonces taken for one signature id meet the commitments of another (timeout, restart or reorg between spawn and resume), and is the result only a failed share?
- Q34. Single use rests on `take_signing_nonces` clearing the row before the share exists (`secrets/store.rs:258-294`) and on the row outliving every replay of its ceremony (`store.rs:24-41, 354-383`, `crates/validator/src/state/secrets.rs:11-85`). Is there any path that uses one nonce pair for two different messages (`docs/overview.md:63-83`)?
- Q35. When the commitment round times out, the signers that committed continue (`state/sign.rs:566-608`, #1008). Do all honest validators compute the same set, given that commitments in the deadline block are applied after `NewBlock(deadline)`?
- Q36. When the share round times out, the largest selection with a threshold of shares restarts (`state/sign.rs:609-669`). Is it unique when the signers are already a subset after a commitment timeout, and can a misbehaving signer split honest shares across roots (`contracts/src/FROSTCoordinator.sol:560-582`, `contracts/src/libraries/FROSTSignatureShares.sol:79-95`)?
- Q37. Restarted ceremonies skip the oracle wait (`state/sign.rs:656-668`, #1009). Can a transaction packet reach `CollectSigningShares`, or a restart, without an approving `OracleResult` on the current branch, for example after a reorg that removes the result?
- Q38. `sign` on the coordinator is permissionless (`FROSTCoordinator.sol:509-521`). What happens when an outsider calls it for a message a validator holds in `WaitingForRequest` (`state/sign.rs:21-112`); can it make validators commit nonces under a signature id the honest set does not finish? (A `Sign` event for another group dropping a pending session is accepted risk.)
- Q39. Do `transaction_packet_hash` and `transaction_proposal_hash` (`crates/validator/src/consensus/hashing.rs:116-150`) always agree, and does the callback context (`state/sign.rs:757-800`) decode to the same message in `Consensus.onSignCompleted` (`contracts/src/Consensus.sol:329-349`) and `ConsensusMessages.transactionProposal` (`contracts/src/libraries/ConsensusMessages.sol:95-114`)?
- Q40. Epoch reaping keeps the epochs that signing sessions reference plus the active epoch (`state/secrets.rs:19-24`). Can a `KeyShare` disappear while a ceremony that needs it can still restart or replay?

#### Validator wiring (R6)

- Q41. Are the fixed gas limits (`crates/validator/src/service/action.rs:138, 147, 177, 186-199, 226, 244, 268, 286, 314, 336, 347`) enough for Aegis groups of 5 with threshold 3, given that a failed action is not detected (no receipt check)?
- Q42. The samples use `database = "sqlite:/var/lib/.../storage.db"` (`crates/validator/validator.sample.toml:26`, `crates/sentinel/sentinel.sample.toml:26`) while the integration scripts append `?mode=rwc` (`scripts/lib/shared_test_scripts.sh:177`). With `create_if_missing: false` in sqlx-sqlite 0.9.0, does a fresh node start?
- Q43. The coordinator address is read once at startup and the watched set is fixed (`crates/validator/src/main.rs:48-57`). What happens when `oracles` is empty (`crates/validator/src/config.rs:54-57`) or lists an address that is not an oracle?
- Q44. Staker reconciliation queues `SetValidatorStaker` with no expiry before the driver runs (`crates/validator/src/main.rs:81-93`). Is a restart before it is included safe, and can a second call be queued?
- Q45. Does anything in `crates/validator/src/bindings.rs` drift from the contract ABIs (event field order, indexed flags, `KeyGenCommitment` with `q` and `pop`) at `361aaf3`?
- Q46. Do the validator metrics (`crates/validator/src/metrics.rs`) expose anything an outsider should not see when bound to `0.0.0.0:3555` as the handbook suggests (`docs/validator-handbook.md:48-53`)?

#### Sentinel (R7)

- Q47. `Committed` events that arrive while a request is in `WaitingForEngineCheck` or `WaitingForRequest` are logged and not counted (`crates/sentinel/src/service.rs:304-340`). What does an undercounted `committed_count` do to the early finalize (`service.rs:381-383`) and to the guard that drops a request this sentinel did not reveal (`service.rs:674-678`)? Compare open issue #961.
- Q48. `SentinelOracle.claim` pays only the caller's own commitment (`contracts/src/SentinelOracle.sol:303-322`). On every path where this sentinel has a bond onchain, does the state machine end in a `Claim` (`service.rs:513-618, 697-861`), and which paths drop a request with the bond still locked (`service.rs:423-427, 674-678`)?
- Q49. `ApproveToken` sets the allowance to one bond per request (`service.rs:235-242`). With several requests open, in Direct mode with up to 16 in flight or in one batch, can a `Commit` run against less allowance than its bond?
- Q50. The request id uses the event's `safeTxHash` while the engine checks `event.transaction` (`service.rs:116-152`). Does `Consensus.proposeTransaction` guarantee they match (`contracts/src/Consensus.sol:253-267`), and does the id match the oracle's `requestId` for the configured `consensus` and the provider's chain id (`crates/sentinel/src/hashing.rs:80-102`, `crates/sentinel/src/main.rs:46, 67-77`)?
- Q51. The engine timeout is three quarters of `voting_window - 1` blocks (`crates/sentinel/src/main.rs:48-65`), 18.75 s with the sample value. It is counted from when the check starts, not from the proposal block. Can a check that starts late (catch-up, restart, open issue #973) still lead to a commit after `commitDeadline`, and what happens to that commit?
- Q52. Verdicts are recorded before the resume and never replaced (`crates/sentinel/src/verdicts.rs:92-121`), and pruned by proposal block below the safe block (`verdicts.rs:123-132`, `crates/sentinel/src/effect.rs:139-163`). Is there a path where the revealed vote differs from the commitment, or where a verdict is pruned while its proposal can still replay?
- Q53. Engine responses (`crates/sentinel/src/engine.rs:71-83, 173-204`): what happens with an unknown verdict, extra fields, a very large body, or a rule code that parses but means nothing, given the rule becomes the onchain `reason` (`service.rs:182-185, 894-911`)?
- Q54. `DisputeResolved.outcome` decodes into a six-variant enum (`crates/sentinel/src/bindings.rs:14-22`). Would an out-of-range value fail the whole log page (`crates/core/src/index/events.rs:675-711`)?
- Q55. Reveal and finalize fire at the first `NewBlock` past each deadline (`service.rs:420-475`) with expiries at the deadlines (`service.rs:240, 248, 444`). Do the queue's expiry rule (`crates/core/src/tx/storage.rs:172-178`) and the optimistic `NewBlock` (`crates/core/src/state/mod.rs:259-268`) always leave a block in which the action can land? (A bond stranded at `revealDeadline + 1` is a known item.)

#### Cross-cutting (R8)

- Q56. Does `cargo audit` (Recon3 runs it) or `cargo tree -d` show advisories, yanked crates or duplicate versions that reach the services, and do the open requirements in `Cargo.toml:5-26` (`alloy = "2"`, `rand = "0.8"`, `reqwest = "0.13"`) matter given the Dockerfiles build with `--locked` (`crates/validator/Dockerfile:18`, `crates/sentinel/Dockerfile:18`)?
- Q57. Which types holding secrets derive `Debug` or `Serialize`, and which reach logs? Candidates: `Secrets` (`crates/validator/src/frost/keygen.rs:27-33`), `SharingState` (`:152-158`), `KeyShare` (`:432-434`), `Effect::GenerateNonces` and `Resume::Setup` (`crates/validator/src/service/effect.rs:22-87`), logged at `crates/core/src/effects.rs:62, 66, 90`, `crates/core/src/driver.rs:267` and `crates/validator/src/service/effect.rs:177`, and `SentinelActionKind::Reveal { salt }` (`crates/sentinel/src/action.rs:9-28`). Section 2.1 lists what frost-core already redacts.
- Q58. Can chain data, RPC answers, engine answers or config reach a panic? `expect` in non-test code: `crates/core/src/index/blocks.rs:390, 513`, `crates/core/src/index/events.rs:108, 109, 660`, `crates/core/src/kdf.rs:25`, `crates/core/src/tx/mod.rs:423`, `crates/validator/src/consensus/group.rs:146`, `crates/validator/src/frost/ecdh.rs:238, 239`, `crates/validator/src/frost/participants.rs:18`, `crates/validator/src/frost/sign.rs:188, 191`; `unwrap` at `crates/core/src/utils.rs:20, 26`; `assert!` at `crates/core/src/kdf.rs:20`.
- Q59. Which of the 34 `as` casts (section 4.3) can truncate a value that comes from chain data, for example `commitments.len() as u16` (`crates/validator/src/state/keygen.rs:109, 196`) or `bondAmount.to::<u128>() as f64` (`crates/sentinel/src/service.rs:337`)?
- Q60. CI runs clippy with `-D warnings` and the tests (`.github/workflows/ci.yml:37-39`, `Justfile:28-29, 44`) but no `cargo audit` or `cargo deny`, and the sentinel integration test uses an in-memory database (`scripts/run_sentinel_integration_test.sh:209`), so no sentinel restart path runs in CI. Which in-scope behaviours have no test at all?
- Q61. Both Dockerfiles build from floating tags and run as root (`crates/validator/Dockerfile:5, 24, 37`, `crates/sentinel/Dockerfile:5, 25, 38`). Does anything in the images or the sample configs go beyond hardening?

#### Cryptography lens (L-crypto)

- Q62. The reveal salt is HKDF-SHA256 over the signer's private key, with the domain as salt and the request id as info (`crates/core/src/tx/signer.rs:118-132`, `crates/core/src/kdf.rs:19-27`, `crates/sentinel/src/hashing.rs:11-14, 49-53`), and it becomes public at reveal. Is that a sound PRF use, and is the same key used for any other derivation?
- Q63. The ECDH pad is the x-coordinate of the shared point (known item, `crates/validator/src/frost/ecdh.rs:214-229`). Beyond the known symmetry, is any pad applied to two different plaintexts, for example when a DKG restart reuses an encryption key?
- Q64. Is `hash_to_scalar(b"enc", entropy)` (`ecdh.rs:29-37, 231-240`) separated from frost-core's own `hash_to_field` uses, and is the PoP bound to the participant identifier as `FROST.verifyProofOfKnowledge` expects (`contracts/src/libraries/FROST.sol:213-246`)?
- Q65. Does each signer's selection leaf (`crates/validator/src/frost/sign.rs:121-168, 194-211`) use exactly the commitment share and Lagrange coefficient that `FROST.verifyShare` checks (`contracts/src/libraries/FROST.sol:192-203`) and the layout of `FROSTSignatureShares._hash` (`contracts/src/libraries/FROSTSignatureShares.sol:118-133`)? Is the binding factor round trip (`frost/sign.rs:179-192`) exact?
- Q66. Merkle trees pad odd levels with a zero hash and sort pairs (`crates/validator/src/merkle.rs:13-59, 85-93`), and participant leaves are raw address words (`crates/validator/src/consensus/group.rs:242-247`). Any second preimage or leaf and node confusion against OpenZeppelin `MerkleProof` in `FROSTParticipantMap.register` (`contracts/src/libraries/FROSTParticipantMap.sol:146-153`) and `FROSTSignatureShares.register` (`FROSTSignatureShares.sol:79-95`)?
- Q67. Is the domain without name and version (`crates/validator/src/consensus/hashing.rs:89-103`, `crates/sentinel/src/hashing.rs:80-102`) identical to `ConsensusMessages.domain` (`contracts/src/libraries/ConsensusMessages.sol:16-50`), and are the `SafeTx` hashes (`consensus/hashing.rs:21-87`, `crates/sentinel/src/hashing.rs:55-78`) identical to `SafeTransaction.hash` and `partialHash` (`contracts/src/libraries/SafeTransaction.sol:78-138`)?
- Q68. Nonces come from `round1::SigningNonces::new(signing_share, rng)` with `rand::thread_rng()` (`frost/sign.rs:33-41`, `crates/validator/src/service/effect.rs:127-131`). Is that adequate, and can a serde round trip of `SigningNonces` (`frost/sign.rs:26-31`) produce a copy that gets used?
- Q69. The EIP-7702 authorization is signed for `nonce + 1` and the provider's chain id, never chain 0 (`crates/core/src/tx/signer.rs:55-108`). Is it bound tightly enough to the intended executor and chain?

#### Rust lens (L-rust)

- Q70. Cancel safety of `Driver::next_input` and of the warp header fetch is a known item (`crates/core/src/driver.rs:208-233`, #1050). Beyond it, are `EventWatcher::next` (`crates/core/src/index/events.rs:318-347`), `BlockWatcher::next` (`crates/core/src/index/blocks.rs:440-527`) and `revalidate_last_block` (`:540-592`) safe to drop at each `.await` when an effect completes first?
- Q71. Panics: effect tasks (`crates/core/src/effects.rs:86-101`), the runtime metrics task (`crates/core/src/metrics.rs:98-107`), the signal handlers (`crates/core/src/utils.rs:17-36`). Can a panic end the process, or a task, without a non-zero exit or a log line?
- Q72. Effects share one pool (10 connections, 5 s busy timeout). Are the deferred transactions in `store_keygen_secrets` and `store_signing_nonces` (`crates/validator/src/secrets/store.rs:172-256`) and the `BEGIN IMMEDIATE` in `next_transaction` (`crates/core/src/tx/storage.rs:165`) correct under concurrent calls, and can `SQLITE_BUSY` become a fatal storage error (`crates/core/src/tx/mod.rs:62-84`)?
- Q73. Secret copies: the config file is read into a plain `String` (`crates/core/src/config.rs:62-69`); substitutions are zeroized (`crates/core/src/serialization.rs:75-134`); the signer key is zeroized in `derive_key` and `from_str` (`crates/core/src/tx/signer.rs:118-163`); `EncryptionKey` zeroizes on drop (`crates/validator/src/frost/ecdh.rs:77-83`). Which copies stay in memory, and does any error type carry a secret?
- Q74. Can chain input grow the effect `JoinSet` (`effects.rs:39-69`), the sentinel request map (`crates/sentinel/src/state.rs:178-180`), or the validator `signing` and `signature_id_to_message` maps (`crates/validator/src/state/mod.rs:47-52`) without bound, and with them the snapshot written per log range (`crates/core/src/state/storage.rs:103-116`)?
- Q75. `StateMachine` holds a `tokio::sync::Mutex` and moves the state out with `mem::take` across awaits (`crates/core/src/state/mod.rs:171-290`). Is there an await after the take whose failure poisons the machine when it need not?

## 5. Known items (A12)

### 5.1 Markers in scope

`git grep -n -E 'TODO|FIXME|XXX|HACK|unimplemented!|todo!|unreachable!' 361aaf3 -- crates/core crates/validator crates/sentinel Cargo.toml` gives three lines, two of them markers:

| Location | Text |
| --- | --- |
| `crates/sentinel/src/config.rs:37-41` | `TODO(epic Phase E2, follow-up)`: pick and document a default for `voting_window`; mandatory for now so a missing value fails loudly |
| `crates/sentinel/src/config.rs:112` | Test comment pointing back at that TODO (inside `mod tests`) |
| `crates/sentinel/src/main.rs:48-52` | `TODO`: derive the engine timeout from effect lifecycle data so time spent before the effect starts is deducted from the reveal deadline |

No `FIXME`, `XXX`, `HACK`, `unimplemented!`, `todo!` or `unreachable!` in scope.

### 5.2 Open GitHub issues touching the services

Fetched once on 2026-10-08 (`gh/issues-open.json`, 21 items, 1 of them PR #905; no pagination link). Engine-only issues (#651, #825, #965 to #972) are out of scope and omitted; explorer and contract issues are omitted.

| Issue | Opened | Title | Relevance |
| --- | --- | --- | --- |
| #20 | 2025-11-13, nlordell | KeyGen Scaling on Secret Share Reveals | Protocol scaling of commitments and shares, `O(n²)` event data; affects DKG gas (Q41) |
| #69 | 2025-12-10, nlordell | Optimize Key Generation facing Multiple Dishonest Participants | Only one dishonest participant is removed per failed keygen; liveness, not safety. Known for R4 |
| #113 | 2026-01-05, rmeissner | Prevent sensitive information from being logged | Body: "RPC Keys". Partly addressed by #1007 and #1046; R8 and L-rust check what is left (Q14, Q57, Q73) |
| #465 | 2026-06-20, nlordell | Extend Alloy Asserter to Check Inputs | Test gap: mocked RPC tests do not assert request contents. Known for R1, R3 |
| #549 | 2026-07-10, rmeissner | Reevaluate sentinel bond slashing | Timeouts return all bonds and fees; rmeissner (2026-08-03): "does not seem to be exploitable and therefore can be kept as is for an initial version" (`gh/issue-549-comments.json`). Known for R7 |
| #961 | 2026-09-22, yuryoparin | Sentinel logged "ignoring unexpected commitment" and missed the reveal | Testnet incident: a commit landed while the request was still `waiting_for_engine_check`, no reveal followed, the bond stayed locked until a manual `finalize` and `claim`; the queue resubmitted executed nonces. Known for R7, R3 (Q19, Q47) |
| #973 | 2026-09-23, rmeissner | Sentinel: Retrigger in-flight engine check effects on restart | In-flight engine checks are lost on restart and on live reorgs that drop a resume; proposes storing what `EngineCheck` needs and re-emitting it. Known for R7, R2 (Q9, Q51) |

Reviewers may file on these only if they add something, tagged `known` (A12).

### 5.3 Open pull requests

One open PR (`gh/pulls-open.json`): #905 "AI Rust Audit Results [ONLY for Review]", draft, remedcu, head `3a0b6df`. Its file list has 6 pages of 100 (`gh/pull-905-files.json` holds page 1, all under `rust-audit/`); `git diff --name-only 8b6a75d 3a0b6df` lists 512 files, all under `rust-audit/`. No open PR touches the three crates.

### 5.4 Other known items

- The run-3 settled and known list: `rust-audit/state/run3/settled.md`.
- #1062 (`fcc7028`, 2026-10-07) records a known issue it does not fix: oracle fees are paid for results the validators never act on. A result that lands after `oracle_timeout` is ignored (`crates/validator/src/state/sign.rs:553-565`), yet the sponsor pays and sentinels collect; a re-proposal in the same epoch reverts unless the oracle data changes. Treat as known.
- The validator flow-test epic (`epics/2026_07_14_validator_state_machine_flow_test_harness.md`) is not implemented at `361aaf3` (no `flow_tests` module in `crates/validator`); its text still describes 1024-nonce chunks that Direct Nonce removed. Test-gap findings that the epic already plans are `known`.

## 6. Reviewer split

Line counts are at `361aaf3`, tests included. "Second reader" marks a high-risk file that another reviewer owns; "context" marks a shared file read for its types and dispatch only. Findings use the ID ranges below (PROMPT.md Section 12 naming). Every reviewer reads `settled.md` first.

### R1: core indexing, reorgs and event timestamps

5 files, 4,591 lines, 1,644 of them outside test modules.

| File                              | Lines | Non-test lines | Role    |
| --------------------------------- | ----- | -------------- | ------- |
| `crates/core/src/index/blocks.rs` | 1,429 | 610            | primary |
| `crates/core/src/index/events.rs` | 2,051 | 787            | primary |
| `crates/core/src/index/mod.rs`    | 485   | 143            | primary |
| `crates/core/src/index/bloom.rs`  | 523   | 41             | primary |
| `crates/core/src/index/clock.rs`  | 103   | 63             | primary |

Checklist: Q1 to Q8. Also confirm INV-C3, INV-C5, INV-C6.

Contract and doc references: none onchain beyond event shapes (`crates/validator/src/bindings.rs`, `crates/sentinel/src/bindings.rs`); `docs/validator-handbook.md:31-44` (`eth_getProof`, `eth_getLogs` reliability). Read `crates/core/src/state/mod.rs:171-276` and `crates/core/src/driver.rs:208-297` as context.

Finding IDs: `F3-CORE-001` to `F3-CORE-029`.

### R2: core runtime

15 files, 3,969 lines, 2,168 of them outside test modules.

| File | Lines | Non-test lines | Role |
| --- | --- | --- | --- |
| `crates/core/src/driver.rs` | 328 | 328 | primary |
| `crates/core/src/state/mod.rs` | 731 | 326 | primary |
| `crates/core/src/state/storage.rs` | 294 | 169 | primary |
| `crates/core/src/effects.rs` | 267 | 112 | primary |
| `crates/core/src/config.rs` | 157 | 77 | primary |
| `crates/core/src/serialization.rs` | 218 | 144 | primary |
| `crates/core/src/observability/mod.rs` | 94 | 64 | primary |
| `crates/core/src/observability/logging.rs` | 21 | 21 | primary |
| `crates/core/src/observability/metrics.rs` | 80 | 49 | primary |
| `crates/core/src/metrics.rs` | 107 | 107 | primary |
| `crates/core/src/provider/mod.rs` | 182 | 182 | primary |
| `crates/core/src/utils.rs` | 97 | 97 | primary |
| `crates/core/src/kdf.rs` | 81 | 28 | primary |
| `crates/core/src/lib.rs` | 26 | 26 | primary |
| `crates/core/src/tx/mod.rs` | 1,286 | 438 | second reader |

Checklist: Q9 to Q16. Also confirm INV-C1, INV-C2, INV-C4, INV-C11.

References: `docs/overview.md:63-83`; open issue #973 (section 5.2).

Finding IDs: `F3-CORE-030` to `F3-CORE-059`.

### R3: core transaction queue

7 files, 3,449 lines, 1,609 of them outside test modules.

| File                            | Lines | Non-test lines | Role    |
| ------------------------------- | ----- | -------------- | ------- |
| `crates/core/src/tx/mod.rs`     | 1,286 | 438            | primary |
| `crates/core/src/tx/storage.rs` | 990   | 423            | primary |
| `crates/core/src/tx/bundle.rs`  | 407   | 210            | primary |
| `crates/core/src/tx/config.rs`  | 194   | 136            | primary |
| `crates/core/src/tx/fees.rs`    | 109   | 57             | primary |
| `crates/core/src/tx/signer.rs`  | 297   | 179            | primary |
| `crates/core/src/tx/types.rs`   | 166   | 166            | primary |

Checklist: Q17 to Q24. Also confirm INV-C7 to INV-C10.

Contract references: `contracts/src/Safenet7702Executor.sol:96-111`, `contracts/src/interfaces/ISafenet7702Executor.sol`. Settled: 7702 lows L1 to L5 (`settled.md` section 1). Docs: `docs/validator-handbook.md:65-77`.

Finding IDs: `F3-CORE-060` to `F3-CORE-089`.

### R4: validator DKG

11 files, 3,669 lines, 3,402 of them outside test modules.

| File | Lines | Non-test lines | Role |
| --- | --- | --- | --- |
| `crates/validator/src/state/keygen.rs` | 1,447 | 1,447 | primary |
| `crates/validator/src/frost/keygen.rs` | 515 | 515 | primary |
| `crates/validator/src/frost/ecdh.rs` | 316 | 241 | primary |
| `crates/validator/src/frost/marshal.rs` | 181 | 181 | primary |
| `crates/validator/src/frost/serialization.rs` | 85 | 85 | primary |
| `crates/validator/src/frost/participants.rs` | 33 | 20 | primary |
| `crates/validator/src/frost/error.rs` | 46 | 46 | primary |
| `crates/validator/src/consensus/group.rs` | 459 | 298 | primary |
| `crates/validator/src/consensus/epoch.rs` | 95 | 77 | primary |
| `crates/validator/src/consensus/mod.rs` | 5 | 5 | primary |
| `crates/validator/src/state/mod.rs` | 487 | 487 | context |

Checklist: Q25 to Q32. Also confirm INV-V1 to INV-V7.

Contract references: `contracts/src/FROSTCoordinator.sol:336-497`, `contracts/src/libraries/FROSTParticipantMap.sol:125-256`, `contracts/src/libraries/FROST.sol:78-83, 213-246`, `contracts/src/libraries/FROSTGroupId.sol:36-60`, `contracts/src/Consensus.sol:217-248, 321-324, 378-397`. Docs: `docs/overview.md:46-52, 85-143`.

Finding IDs: `F3-VAL-001` to `F3-VAL-029`.

### R5: validator signing, nonces and secret store

11 files, 3,759 lines, 2,669 of them outside test modules.

| File | Lines | Non-test lines | Role |
| --- | --- | --- | --- |
| `crates/validator/src/state/sign.rs` | 836 | 836 | primary |
| `crates/validator/src/state/transactions.rs` | 102 | 102 | primary |
| `crates/validator/src/state/secrets.rs` | 86 | 86 | primary |
| `crates/validator/src/frost/sign.rs` | 236 | 212 | primary |
| `crates/validator/src/frost/mod.rs` | 325 | 16 | primary |
| `crates/validator/src/merkle.rs` | 142 | 94 | primary |
| `crates/validator/src/secrets/store.rs` | 1,056 | 426 | primary |
| `crates/validator/src/secrets/mod.rs` | 5 | 5 | primary |
| `crates/validator/src/consensus/hashing.rs` | 249 | 170 | primary |
| `crates/validator/src/service/effect.rs` | 235 | 235 | primary |
| `crates/validator/src/state/mod.rs` | 487 | 487 | context |

Checklist: Q33 to Q40. Also confirm INV-V8 to INV-V14.

Contract references: `contracts/src/FROSTCoordinator.sol:509-605`, `contracts/src/libraries/FROSTSignatureShares.sol:79-133`, `contracts/src/libraries/FROST.sol:178-203`, `contracts/src/Consensus.sol:253-349`, `contracts/src/libraries/ConsensusMessages.sol`, `contracts/src/libraries/SafeTransaction.sol:78-138`. Docs: `docs/overview.md:54-83, 145-206`, `docs/validator-handbook.md:79-108`.

Finding IDs: `F3-VAL-030` to `F3-VAL-059`.

### R6: validator wiring

14 files, 3,722 lines, 3,550 of them outside test modules.

| File | Lines | Non-test lines | Role |
| --- | --- | --- | --- |
| `crates/validator/src/main.rs` | 99 | 99 | primary |
| `crates/validator/src/config.rs` | 281 | 109 | primary |
| `crates/validator/src/service/mod.rs` | 129 | 129 | primary |
| `crates/validator/src/service/action.rs` | 353 | 353 | primary |
| `crates/validator/src/service/effect.rs` | 235 | 235 | second reader |
| `crates/validator/src/bindings.rs` | 245 | 245 | primary |
| `crates/validator/src/metrics.rs` | 173 | 173 | primary |
| `crates/validator/src/state/mod.rs` | 487 | 487 | primary |
| `crates/validator/src/state/transactions.rs` | 102 | 102 | context |
| `crates/validator/validator.sample.toml` | 104 | n/a | primary |
| `crates/validator/Dockerfile` | 37 | n/a | primary |
| `crates/validator/Dockerfile.dockerignore` | 4 | n/a | primary |
| `crates/validator/Cargo.toml` | 26 | n/a | primary |
| `crates/validator/src/state/keygen.rs` | 1,447 | 1,447 | second reader |

Checklist: Q41 to Q46. As second reader of `state/keygen.rs`, also answer Q29 and Q32 independently of R4.

Contract references: ABIs of `contracts/src/Consensus.sol`, `contracts/src/FROSTCoordinator.sol`, `contracts/src/interfaces/IConsensus.sol`, `contracts/src/interfaces/IOracle.sol`. Docs: `docs/validator-handbook.md`, `docs/configuration.md:75-145`.

Finding IDs: `F3-VAL-060` to `F3-VAL-089`.

### R7: sentinel

15 files, 4,530 lines, 2,526 of them outside test modules.

| File                                      | Lines | Non-test lines | Role    |
| ----------------------------------------- | ----- | -------------- | ------- |
| `crates/sentinel/src/service.rs`          | 2,214 | 1,051          | primary |
| `crates/sentinel/src/state.rs`            | 383   | 181            | primary |
| `crates/sentinel/src/verdicts.rs`         | 268   | 148            | primary |
| `crates/sentinel/src/effect.rs`           | 263   | 165            | primary |
| `crates/sentinel/src/engine.rs`           | 430   | 206            | primary |
| `crates/sentinel/src/hashing.rs`          | 224   | 103            | primary |
| `crates/sentinel/src/action.rs`           | 46    | 46             | primary |
| `crates/sentinel/src/bindings.rs`         | 176   | 176            | primary |
| `crates/sentinel/src/config.rs`           | 135   | 59             | primary |
| `crates/sentinel/src/main.rs`             | 93    | 93             | primary |
| `crates/sentinel/src/metrics.rs`          | 143   | 143            | primary |
| `crates/sentinel/sentinel.sample.toml`    | 89    | n/a            | primary |
| `crates/sentinel/Dockerfile`              | 38    | n/a            | primary |
| `crates/sentinel/Dockerfile.dockerignore` | 4     | n/a            | primary |
| `crates/sentinel/Cargo.toml`              | 24    | n/a            | primary |

Checklist: Q47 to Q55. Also confirm INV-S1 to INV-S5.

Contract references: `contracts/src/SentinelOracle.sol:220-350`, `contracts/src/libraries/SentinelOracleRequests.sol:112-227, 380-416`, `contracts/src/libraries/SentinelOracleCommitments.sol:47-122`, `contracts/src/Consensus.sol:253-267`. Docs: `docs/sentinel-handbook.md`, `docs/sentinel-engine.md:7-37`, `docs/transaction-lifecycle.md`. Engine: treat `crates/sentinel-engine` as an opaque service under A3; do not file against it.

Finding IDs: `F3-SEN-001` to `F3-SEN-049`.

### R8: cross-cutting

14 files, 3,389 lines, 1,827 of them outside test modules. Cargo.lock (6,141 lines) is read with tools (`cargo tree`, `cargo audit`), not line by line.

| File | Lines | Non-test lines | Role |
| --- | --- | --- | --- |
| `Cargo.toml` | 26 | n/a | primary |
| `Cargo.lock` | 6,141 | n/a | primary |
| `crates/core/Cargo.toml` | 33 | n/a | primary |
| `crates/validator/Cargo.toml` | 26 | n/a | second reader |
| `crates/sentinel/Cargo.toml` | 24 | n/a | second reader |
| `crates/validator/Dockerfile` | 37 | n/a | second reader |
| `crates/validator/Dockerfile.dockerignore` | 4 | n/a | second reader |
| `crates/sentinel/Dockerfile` | 38 | n/a | second reader |
| `crates/sentinel/Dockerfile.dockerignore` | 4 | n/a | second reader |
| `crates/validator/validator.sample.toml` | 104 | n/a | second reader |
| `crates/sentinel/sentinel.sample.toml` | 89 | n/a | second reader |
| `crates/core/src/tx/bundle.rs` | 407 | 210 | second reader |
| `crates/sentinel/src/service.rs` | 2,214 | 1,051 | second reader |
| `crates/sentinel/src/state.rs` | 383 | 181 | second reader |

Checklist: Q56 to Q61; as second reader of the sentinel service also answer Q47 and Q48, and of `tx/bundle.rs` Q23, each independently. Censuses across all three crates: secrets in logs and `Debug`, panics and `expect`/`unwrap` on untrusted input, integer casts, `unsafe` (none in scope), CI gaps.

References: Recon3's `state/run3/baseline.md` (its summary reports 5 `cargo audit` vulnerabilities and 11 warnings, 9 of the 16 crates compiled into the in-scope packages, plus clippy 1.96 and 1.99 logs under `state/run3/logs/`); `.github/workflows/ci.yml`, `.github/workflows/integration.yml`, `Justfile`.

Finding IDs: `F3-XC-001` to `F3-XC-029`.

### L-crypto: cryptography lens across the three crates

15 files, 4,025 lines, 3,024 of them outside test modules.

| File | Lines | Non-test lines | Role |
| --- | --- | --- | --- |
| `crates/validator/src/frost/mod.rs` | 325 | 16 | lens |
| `crates/validator/src/frost/keygen.rs` | 515 | 515 | lens |
| `crates/validator/src/frost/sign.rs` | 236 | 212 | lens |
| `crates/validator/src/frost/ecdh.rs` | 316 | 241 | lens |
| `crates/validator/src/frost/marshal.rs` | 181 | 181 | lens |
| `crates/validator/src/frost/serialization.rs` | 85 | 85 | lens |
| `crates/validator/src/frost/participants.rs` | 33 | 20 | lens |
| `crates/validator/src/frost/error.rs` | 46 | 46 | lens |
| `crates/validator/src/merkle.rs` | 142 | 94 | lens |
| `crates/validator/src/consensus/group.rs` | 459 | 298 | lens |
| `crates/validator/src/consensus/hashing.rs` | 249 | 170 | lens |
| `crates/validator/src/state/sign.rs` | 836 | 836 | lens |
| `crates/core/src/kdf.rs` | 81 | 28 | lens |
| `crates/core/src/tx/signer.rs` | 297 | 179 | lens |
| `crates/sentinel/src/hashing.rs` | 224 | 103 | lens |

Checklist: Q62 to Q69, plus Q26 to Q28 and Q33 to Q36 from the crypto side.

References: `contracts/src/libraries/FROST.sol`, `contracts/src/libraries/Secp256k1.sol`, `contracts/src/libraries/FROSTSignatureShares.sol`, `contracts/src/libraries/ConsensusMessages.sol`, `contracts/src/libraries/SafeTransaction.sol`, `contracts/src/libraries/SentinelOracleCommitments.sol:47-56`; RFC 9591; frost-core 3.0.0 source in the cargo registry. Known: the ECDH pad (`settled.md` section 2).

Finding IDs: `F3-XC-100` to `F3-XC-129`.

### L-rust: Rust lens across the three crates

12 files, 4,491 lines, 2,543 of them outside test modules.

| File                                     | Lines | Non-test lines | Role |
| ---------------------------------------- | ----- | -------------- | ---- |
| `crates/core/src/driver.rs`              | 328   | 328            | lens |
| `crates/core/src/effects.rs`             | 267   | 112            | lens |
| `crates/core/src/utils.rs`               | 97    | 97             | lens |
| `crates/core/src/config.rs`              | 157   | 77             | lens |
| `crates/core/src/serialization.rs`       | 218   | 144            | lens |
| `crates/core/src/provider/mod.rs`        | 182   | 182            | lens |
| `crates/core/src/tx/storage.rs`          | 990   | 423            | lens |
| `crates/validator/src/secrets/store.rs`  | 1,056 | 426            | lens |
| `crates/validator/src/service/effect.rs` | 235   | 235            | lens |
| `crates/sentinel/src/effect.rs`          | 263   | 165            | lens |
| `crates/sentinel/src/engine.rs`          | 430   | 206            | lens |
| `crates/sentinel/src/verdicts.rs`        | 268   | 148            | lens |

Checklist: Q70 to Q75, plus Q10, Q15 and Q57 from the Rust side. Scan all three crates for `tokio::select!`, `spawn`, `JoinSet`, `#[derive(Debug)]` on secret-bearing types, `expect`/`unwrap`, `as` casts and `Drop`.

References: tokio and sqlx 0.9.0 semantics (registry source). Known: cancel safety of `Driver::next_input` and the warp header fetch (`settled.md` section 2).

Finding IDs: `F3-XC-130` to `F3-XC-159`.

### Load summary

| Reviewer | Files | Lines | Non-test lines | Notes |
| --- | --- | --- | --- | --- |
| R1 | 5 | 4,591 | 1,644 | test-heavy (64 % tests) |
| R2 | 15 | 3,969 | 2,168 | second reader of `tx/mod.rs` |
| R3 | 7 | 3,449 | 1,609 | owns the queue |
| R4 | 11 | 3,669 | 3,402 | `state/keygen.rs` has no tests |
| R5 | 11 | 3,759 | 2,669 | owns nonces and the secret store |
| R6 | 14 | 3,722 | 3,550 | second reader of `state/keygen.rs` |
| R7 | 15 | 4,530 | 2,526 | 44 % tests; owns the sentinel |
| R8 | 14 | 3,389 | 1,827 | plus censuses over all crates; Cargo.lock by tools |
| L-crypto | 15 | 4,025 | 3,024 | lens; second reader of signing and DKG crypto |
| L-rust | 12 | 4,491 | 2,543 | lens; second reader of stores and engine client |

R1 carries the most lines, 64 % of them tests; R7 follows with 44 % tests. R4 and R6 carry the most non-test code because `state/keygen.rs` has 1,447 lines and no tests.

## 7. Coverage check

75 files (64 `.rs` files with 23,319 lines, 11 other files with 6,526 lines). Every file has at least one reader; 42 files have two or more.

| # | File | Lines | Non-test lines | Readers |
| --- | --- | --- | --- | --- |
| 1 | `Cargo.lock` | 6,141 | n/a | R8 |
| 2 | `Cargo.toml` | 26 | n/a | R8 |
| 3 | `crates/core/Cargo.toml` | 33 | n/a | R8 |
| 4 | `crates/core/src/config.rs` | 157 | 77 | R2, L-rust |
| 5 | `crates/core/src/driver.rs` | 328 | 328 | R2, L-rust |
| 6 | `crates/core/src/effects.rs` | 267 | 112 | R2, L-rust |
| 7 | `crates/core/src/index/blocks.rs` | 1,429 | 610 | R1 |
| 8 | `crates/core/src/index/bloom.rs` | 523 | 41 | R1 |
| 9 | `crates/core/src/index/clock.rs` | 103 | 63 | R1 |
| 10 | `crates/core/src/index/events.rs` | 2,051 | 787 | R1 |
| 11 | `crates/core/src/index/mod.rs` | 485 | 143 | R1 |
| 12 | `crates/core/src/kdf.rs` | 81 | 28 | R2, L-crypto |
| 13 | `crates/core/src/lib.rs` | 26 | 26 | R2 |
| 14 | `crates/core/src/metrics.rs` | 107 | 107 | R2 |
| 15 | `crates/core/src/observability/logging.rs` | 21 | 21 | R2 |
| 16 | `crates/core/src/observability/metrics.rs` | 80 | 49 | R2 |
| 17 | `crates/core/src/observability/mod.rs` | 94 | 64 | R2 |
| 18 | `crates/core/src/provider/mod.rs` | 182 | 182 | R2, L-rust |
| 19 | `crates/core/src/serialization.rs` | 218 | 144 | R2, L-rust |
| 20 | `crates/core/src/state/mod.rs` | 731 | 326 | R2 |
| 21 | `crates/core/src/state/storage.rs` | 294 | 169 | R2 |
| 22 | `crates/core/src/tx/bundle.rs` | 407 | 210 | R3, R8 |
| 23 | `crates/core/src/tx/config.rs` | 194 | 136 | R3 |
| 24 | `crates/core/src/tx/fees.rs` | 109 | 57 | R3 |
| 25 | `crates/core/src/tx/mod.rs` | 1,286 | 438 | R2, R3 |
| 26 | `crates/core/src/tx/signer.rs` | 297 | 179 | R3, L-crypto |
| 27 | `crates/core/src/tx/storage.rs` | 990 | 423 | R3, L-rust |
| 28 | `crates/core/src/tx/types.rs` | 166 | 166 | R3 |
| 29 | `crates/core/src/utils.rs` | 97 | 97 | R2, L-rust |
| 30 | `crates/sentinel/Cargo.toml` | 24 | n/a | R7, R8 |
| 31 | `crates/sentinel/Dockerfile` | 38 | n/a | R7, R8 |
| 32 | `crates/sentinel/Dockerfile.dockerignore` | 4 | n/a | R7, R8 |
| 33 | `crates/sentinel/sentinel.sample.toml` | 89 | n/a | R7, R8 |
| 34 | `crates/sentinel/src/action.rs` | 46 | 46 | R7 |
| 35 | `crates/sentinel/src/bindings.rs` | 176 | 176 | R7 |
| 36 | `crates/sentinel/src/config.rs` | 135 | 59 | R7 |
| 37 | `crates/sentinel/src/effect.rs` | 263 | 165 | R7, L-rust |
| 38 | `crates/sentinel/src/engine.rs` | 430 | 206 | R7, L-rust |
| 39 | `crates/sentinel/src/hashing.rs` | 224 | 103 | R7, L-crypto |
| 40 | `crates/sentinel/src/main.rs` | 93 | 93 | R7 |
| 41 | `crates/sentinel/src/metrics.rs` | 143 | 143 | R7 |
| 42 | `crates/sentinel/src/service.rs` | 2,214 | 1,051 | R7, R8 |
| 43 | `crates/sentinel/src/state.rs` | 383 | 181 | R7, R8 |
| 44 | `crates/sentinel/src/verdicts.rs` | 268 | 148 | R7, L-rust |
| 45 | `crates/validator/Cargo.toml` | 26 | n/a | R6, R8 |
| 46 | `crates/validator/Dockerfile` | 37 | n/a | R6, R8 |
| 47 | `crates/validator/Dockerfile.dockerignore` | 4 | n/a | R6, R8 |
| 48 | `crates/validator/src/bindings.rs` | 245 | 245 | R6 |
| 49 | `crates/validator/src/config.rs` | 281 | 109 | R6 |
| 50 | `crates/validator/src/consensus/epoch.rs` | 95 | 77 | R4 |
| 51 | `crates/validator/src/consensus/group.rs` | 459 | 298 | R4, L-crypto |
| 52 | `crates/validator/src/consensus/hashing.rs` | 249 | 170 | R5, L-crypto |
| 53 | `crates/validator/src/consensus/mod.rs` | 5 | 5 | R4 |
| 54 | `crates/validator/src/frost/ecdh.rs` | 316 | 241 | R4, L-crypto |
| 55 | `crates/validator/src/frost/error.rs` | 46 | 46 | R4, L-crypto |
| 56 | `crates/validator/src/frost/keygen.rs` | 515 | 515 | R4, L-crypto |
| 57 | `crates/validator/src/frost/marshal.rs` | 181 | 181 | R4, L-crypto |
| 58 | `crates/validator/src/frost/mod.rs` | 325 | 16 | R5, L-crypto |
| 59 | `crates/validator/src/frost/participants.rs` | 33 | 20 | R4, L-crypto |
| 60 | `crates/validator/src/frost/serialization.rs` | 85 | 85 | R4, L-crypto |
| 61 | `crates/validator/src/frost/sign.rs` | 236 | 212 | R5, L-crypto |
| 62 | `crates/validator/src/main.rs` | 99 | 99 | R6 |
| 63 | `crates/validator/src/merkle.rs` | 142 | 94 | R5, L-crypto |
| 64 | `crates/validator/src/metrics.rs` | 173 | 173 | R6 |
| 65 | `crates/validator/src/secrets/mod.rs` | 5 | 5 | R5 |
| 66 | `crates/validator/src/secrets/store.rs` | 1,056 | 426 | R5, L-rust |
| 67 | `crates/validator/src/service/action.rs` | 353 | 353 | R6 |
| 68 | `crates/validator/src/service/effect.rs` | 235 | 235 | R5, R6, L-rust |
| 69 | `crates/validator/src/service/mod.rs` | 129 | 129 | R6 |
| 70 | `crates/validator/src/state/keygen.rs` | 1,447 | 1,447 | R4, R6 |
| 71 | `crates/validator/src/state/mod.rs` | 487 | 487 | R4, R5, R6 |
| 72 | `crates/validator/src/state/secrets.rs` | 86 | 86 | R5 |
| 73 | `crates/validator/src/state/sign.rs` | 836 | 836 | R5, L-crypto |
| 74 | `crates/validator/src/state/transactions.rs` | 102 | 102 | R5, R6 |
| 75 | `crates/validator/validator.sample.toml` | 104 | n/a | R6, R8 |

Highest-risk files and their readers: `state/sign.rs` (R5, L-crypto), `frost/sign.rs` (R5, L-crypto), `state/keygen.rs` (R4, R6), `frost/keygen.rs` and `frost/ecdh.rs` (R4, L-crypto), `secrets/store.rs` (R5, L-rust), `service/effect.rs` (R5, R6, L-rust), `tx/mod.rs` (R3, R2), `tx/storage.rs` (R3, L-rust), `tx/bundle.rs` (R3, R8), `tx/signer.rs` (R3, L-crypto), sentinel `service.rs` and `state.rs` (R7, R8), `verdicts.rs`, `effect.rs` and `engine.rs` (R7, L-rust).
