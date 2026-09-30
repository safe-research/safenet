> Generated at commit 82b3e0d by a read-only analysis agent (Claude Fable 5.1) with no toolchain available. Every statement is class I or E2 by inspection; nothing here was executed. The Manager spot-checked the citations of the top hypotheses only. Treat every hypothesis as a lead to confirm or refute, never as a finding.

# `sentinel` crate — technical map and risk analysis

Scope: `crates/sentinel` (all 10 `.rs` files, 3,348 LOC, read in full), plus the parts of `safenet-core` the crate builds on (driver, state machine + snapshot store, effect manager, transaction queue + storage, signer/KDF, block/event watchers — read as cited). Solidity files were read ONLY to check hashing/encoding parity and to learn what the oracle does on a call the sentinel makes (they were not reviewed). Nothing was built or executed; every claim below cites lines that were actually read. Where I did not read something, it is marked "not read".

Files read in full: `main.rs`, `action.rs`, `effect.rs`, `config.rs`, `metrics.rs`, `state.rs`, `bindings.rs`, `hashing.rs`, `engine.rs`, `service.rs`, `sentinel.sample.toml`, `Dockerfile`, `Cargo.toml`; core `driver.rs`, `effects.rs`, `state/mod.rs`, `state/storage.rs`, `tx/mod.rs`, `tx/storage.rs`, `tx/types.rs`, `tx/signer.rs`, `tx/fees.rs`, `kdf.rs`, `provider/mod.rs`, `index/mod.rs`, `utils.rs`, `observability/*`; core `index/blocks.rs` lines 1-470 and `index/events.rs` lines 1-140 and 395-600 (rest not read); contracts `SentinelOracle.sol`, `libraries/SentinelOracleCommitments.sol`, `libraries/SentinelOracleRequests.sol`, `libraries/ConsensusMessages.sol`, `libraries/SentinelMap.sol`, `interfaces/IOracle.sol`, `libraries/SafeTransaction.sol` lines 1-140, `Consensus.sol` lines 140-160 and 245-300, `interfaces/IConsensus.sol` lines 60-100; `docs/sentinel-handbook.md`, `docs/sentinel-engine.md`, `crates/sentinel-engine/openapi.yaml`, `scripts/run_sentinel_integration_test.sh`, `AGENTS.md`.

---

## 1. Purpose and runtime shape

### 1.1 What the binary does

A sentinel is a bonded voter in the `SentinelOracle` commit/reveal game. It indexes two contracts (`SentinelOracle`, `Consensus`), forwards every `TransactionProposed` Safe transaction to an HTTP "engine" (`POST /v1/security-check`), and maps the verdict onto an onchain vote backed by an ERC-20 bond: `approve(fee_token)` → `commit(requestId, hash)` → `reveal(requestId, approve, salt, reason)` → `finalize(requestId)` → `claim(requestId)`.

### 1.2 `main.rs` wiring (`main.rs:29-89`)

1. `Config::load(--config-file)` (`main.rs:37`, `config.rs:61-67`), `observability::init` (`main.rs:38`), `Provider::connect(rpc)` (`main.rs:41`; reads `eth_chainId` once, core `provider/mod.rs:129-137`), `utils::connect_sqlite(database)` (`main.rs:42`; core `utils.rs:56-62`).
2. **Engine timeout derivation** (`main.rs:45-62`): `engine_timeout = max(1 s, ((voting_window − 1) · block_time_ms · 3) / 4)`. `block_time` comes from `[index].block_time` (explicit ms or `auto` = 5 000 ms on chain 100, 12 000 ms on Sepolia; core `index/blocks.rs:34-43`). Note it is derived from the **config** `voting_window`, not from the onchain `COMMIT_WINDOW`/`REVEAL_WINDOW` (the TODO at `main.rs:45-49` acknowledges this).
3. `SentinelService::new(oracle, fee_token, consensus, signer, U256(chain_id), voting_window, EngineClient::new(engine_url)?, engine_timeout)` (`main.rs:64-73`, `service.rs:70-93`).
4. `Driver::new(service, provider, signer, pool, vec![oracle, consensus], config.driver)` (`main.rs:75-83`; core `driver.rs:120-150`): the watcher filters logs by those two addresses and the union of both contracts' event topics (`bindings.rs:164-170`, core `index/events.rs:568-592`).
5. `driver.run()` (`main.rs:86`; core `driver.rs:170-198`): single loop, `select!` between watcher updates and completed effects, processes one input to completion, exits on shutdown signal or unrecoverable error.

### 1.3 `Service` impl (`service.rs:799-833`)

`SentinelService::components()` splits into three pure-ish parts consumed by the core driver:

- `SentinelTransition` (`service.rs:44-57`, `StateTransition` impl at `743-791`) — the FSM; owns the `Signer` only to (a) derive reveal salts and (b) compare `event.sentinel == self.signer.address()`.
- `effect::Handler` (`effect.rs:40-76`) — performs the single effect `Effect::EngineCheck` by calling `EngineClient`.
- `SentinelEncoder` (`service.rs:61-68`, `ActionEncoder` impl at `793-797`, encoding at `676-740`) — turns `SentinelAction`s into raw `Transaction { to, value: 0, data, gas }` with hard-coded gas limits (approve 55 000; commit/reveal/finalize/claim 250 000) plus an `expires_at` block.

### 1.4 Request lifecycle FSM (real names, `state.rs:25-77`)

State is `State(HashMap<B256 /*requestId*/, SentinelRequestState>)` (`state.rs:94`).

| State | Fields | Entered by | Left by |
| --- | --- | --- | --- |
| `WaitingForEngineCheck { deadline, request: Option<Request> }` | `deadline = proposal_block + voting_window` (`service.rs:127`); `request` filled when `NewRequest` arrives first (`service.rs:264-276`) | `TransactionProposed` for our oracle (`service.rs:99-145`) | engine resume (`150-194`); block > `commit_deadline` (or > `deadline` if no request) (`394-399`); `ArbitrationTimedOut`/`DisputeOutOfScope` (`544-556`) |
| `WaitingForRequest { approve, reason, deadline }` | engine verdict known, request not yet open | resume before `NewRequest` (`185-193`) | `NewRequest` → `commit_vote` (`261-263`); block > `deadline` (`400`) |
| `CollectingCommitments { approve, reason, slash_amount, commit_deadline, reveal_deadline, committed_count, self_committed }` |  | `commit_vote` (`198-244`) — emits `ApproveToken{bond}` + `Commit{id,hash}` both expiring at `commit_deadline` | block > `commit_deadline`: if `self_committed` → emit `Reveal` (expires `reveal_deadline`) and go to `CollectingVotes`, else drop (`401-449`) |
| `CollectingVotes { approve, slash_amount, reveal_deadline, committed_count, revealed_count, approve_count, deny_count, self_revealed }` |  | above | `Revealed` with `revealed_count >= committed_count` (`372-383`) or block > `reveal_deadline` (`450-465`) → `finalize()` (`607-672`) |
| `WaitingForDisputeResolution { approve, slash_amount }` |  | `finalize()` when both `approve_count>0 && deny_count>0` (`646-654`); emits `Finalize` | `DisputeResolved` (`483-529`, emits `Claim`), `ArbitrationTimedOut`/`DisputeOutOfScope` (`539-576`, emits `Claim`). **No deadline** (`466` retains forever). |

`finalize()` (`service.rs:607-672`) semantics: `dispute = approve_count>0 && deny_count>0`; `timed_out = revealed_count == 0`; if `!self_revealed && !timed_out` → drop entry, no action (`631-633`); else emit `Finalize` (`635-641`); if `dispute` → `WaitingForDisputeResolution` (`646-654`); else also emit `Claim` and drop (`664-671`).

Timeouts:

- `voting_window` (config, blocks): only bounds the two pre-commit states and feeds `engine_timeout`. In practice `TransactionProposed` and `NewRequest` are emitted by the same `Consensus.proposeTransaction` call (`Consensus.sol:264-266`), so both logs land in the same batch (core applies a whole log batch before any effect is spawned, `state/mod.rs:213-223`, `driver.rs:255-274`) and `request` is already `Some` when the engine resumes — `WaitingForRequest` is effectively unreachable with this Consensus; `voting_window` matters mainly through `engine_timeout`.
- `commit_deadline`/`reveal_deadline`: from `NewRequest` (`service.rs:254-259`), compared with `block <= deadline` mirroring the contract (`SentinelOracleRequests.sol:120,129-130`).
- Arbitration timeout: onchain only (`ARBITRATION_TIMEOUT`); the sentinel merely reacts to `ArbitrationTimedOut`/`DisputeOutOfScope` (PR #883, `service.rs:539-576`, `774-779`) and never calls `timeoutArbitration` (no binding for it, `bindings.rs:47-57`).

### 1.5 Engine verdict → action (`engine.rs:170-188`, `service.rs:173-180`)

| Engine | `CheckOutcome` | FSM |
| --- | --- | --- |
| 200 `{"verdict":"secure"}` | `Approved` | `approve=true, reason=""` → commit |
| 200 `{"verdict":"insecure","rule":"R-a.b"}` | `Denied(RuleId(a,b))` | `approve=false, reason="R-a.b"` → commit |
| 200 `{"verdict":"abstain"}` | `Unknown` | entry removed, no vote (`176-179`) |
| non-2xx, transport error, timeout, invalid JSON, malformed rule | `Unknown` (+ `error` metric) | same: request dropped, never retried |

---

## 2. Module map

| File | LOC | Responsibility | Key pub types / fns | Main deps |
| --- | --- | --- | --- | --- |
| `main.rs` | 89 | CLI (`--config-file`, `--version`), wiring, engine-timeout derivation | `Options`, `main` | `safenet_core::{Driver, observability, provider, utils}`, `argh`, `tokio` |
| `config.rs` | 144 | TOML schema (`deny_unknown_fields`) | `Config{rpc, signer, database, oracle, consensus, sentinel, observability, driver(flatten)}`, `SentinelConfig{fee_token, voting_window, engine}`, `Config::load` | `serde`, `toml`, `sqlx::SqliteConnectOptions`, `url` |
| `service.rs` | 1 851 (835 non-test) | FSM, action encoding, `Service` impl | `SentinelService`, `SentinelTransition`, `SentinelEncoder` | `safenet_core::{driver, state, tx}`, `alloy::sol_types::SolCall` |
| `state.rs` | 167 | Persisted per-request FSM state (serde JSON) | `Request`, `SentinelRequestState`, `State` | `alloy::aliases::U96`, `serde` |
| `action.rs` | 43 | Onchain action enum + expiry | `SentinelActionKind{ApproveToken, Commit, Reveal, Finalize, Claim}`, `SentinelAction{kind, expires_at}` | `safenet_core::state::Command` |
| `effect.rs` | 134 | Single effect (engine check) + handler | `Effect::EngineCheck{request_id, transaction, block}`, `Resume::EngineCheckResult`, `Handler` | `safenet_core::effects::EffectHandler`, `engine` |
| `engine.rs` | 392 (196 non-test) | HTTP client to engine | `CheckOutcome`, `RuleId`, `EngineClient::new/security_check`, `SecurityCheck::{request_id,timeout,execute}` | `reqwest`, `serde`, `tracing`, `url` |
| `hashing.rs` | 224 (103 non-test) | Commitment hash, reveal salt, request-id (EIP-712) | `commit_hash`, `RevealSalt::reveal_salt`, `oracle_tx_proposal_hash`, `safe_tx_hash` (test-only) | `alloy::{Keccak256, Eip712Domain, SolStruct}`, `safenet_core::tx::Signer::derive_key` |
| `bindings.rs` | 170 | `sol!` ABI bindings + event set | `oracle::{SentinelOracle, ERC20, RequestState}`, `consensus::{Consensus, SafeTransaction, TransactionProposal, Operation}`, `safe::SafeTx`, `SentinelEvents` | `alloy::sol`, `safenet_core::watcher_events!` |
| `metrics.rs` | 134 | Prometheus counters/histograms | `engine_check_verdicts_total`, `requests_proposed_total`, `requests_participated_total`, `bond_amount`, `requests_resolved_total`, `dispute_bond_slashed_amount`, `fee_reward_amount` | `metrics` |

---

## 3. External interfaces

### 3.1 Contracts (bindings.rs)

Events consumed (`bindings.rs:24-46, 106-115`): `SentinelOracle.{NewRequest, Committed, Revealed, DisputeResolved, ArbitrationTimedOut, DisputeOutOfScope, Claimed}` and `Consensus.TransactionProposed`. Not consumed although emitted by the oracle: `DisputeTriggered`, `RequestTimedOut`, `OracleResult` (`SentinelOracle.sol:33,45,283`) — the FSM infers freeze/timeout locally instead. Calls made (`bindings.rs:47-57, 61-64`; encoded at `service.rs:676-740`): `ERC20.approve(oracle, bondTarget)` (to `fee_token`, gas 55 000), `SentinelOracle.commit/reveal/finalize/claim` (gas 250 000 each). `hashCommitment` and `ERC20.allowance` are bound but never called (dead bindings). No `timeoutArbitration` binding. Dispatch: `service.rs:749-790`. `event.address` is never inspected by the FSM (the watcher's address filter is the only guard).

### 3.2 HTTP client to the engine (engine.rs)

- URL: base URL from config; `path_segments_mut().pop_if_empty().extend(["v1","security-check"])` (`engine.rs:104-115`) → `http://host:5473` → `http://host:5473/v1/security-check`; a base path prefix is preserved; query/fragment on the base URL are preserved; cannot-be-a-base URLs rejected at startup.
- Client: `reqwest::Client::new()` (`engine.rs:113`) — defaults: follows redirects (up to 10), honours `HTTP(S)_PROXY` env, no global timeout; workspace features `json`, `rustls` (`Cargo.toml` workspace deps).
- Request: `POST` JSON `{ "block": <quantity hex>, "transaction": <SafeTransaction> }` (`engine.rs:71-76, 131-134`). `block` via `alloy::serde::quantity` (minimal hex). `SafeTransaction` is the `sol!` struct with `#[derive(Serialize)]` (`bindings.rs:77-91`): field idents are already camelCase, `U256`/`Address`/`Bytes` serialize as hex strings, `Operation` custom-serialized as `0`/`1` (`bindings.rs:118-130`). Matches `openapi.yaml` `SafeTransaction` by construction; no unit test asserts the body shape (the integration test does exercise the real wire against the reference engine, whose `deny_unknown_fields, rename_all="camelCase"` deserializer would reject a mismatch — `crates/sentinel-engine/src/engine/transaction.rs:55-56`, grep only).
- Headers: `x-request-id: 0x<64 hex>` (`engine.rs:148-152`), `x-request-timeout: <ms>` (`155-161`); per-request `reqwest` timeout set to the same value. Both only when the builder methods are called; `effect.rs:62-67` always calls both.
- Response parsing: `send()?.error_for_status()?.json::<Response>()` (`engine.rs:168`); `Response` is internally tagged on `verdict` (`78-84`), unknown extra fields tolerated, `rule` parsed by `RuleId::parse` = `^R-<u32>.<u32>$` (`37-42`) — any rule code matching the OpenAPI regex is accepted (open set), out-of-`u32` or malformed → deserialization error → `Unknown`.
- Error handling: **any** failure (5xx, 4xx, timeout, connection refused, invalid JSON, malformed rule) → `CheckOutcome::Unknown` + `tracing::error!` + `error` metric (`181-187`). **No retries**, no backoff, one attempt per proposal.

### 3.3 Config (config.rs)

`Config` (`config.rs:17-39`): `rpc: Url` (required), `signer: Signer` (required, 32-byte hex; deserializer zeroizes its temporary, core `tx/signer.rs:84-94`), `database: SqliteConnectOptions` via `from_str` (required), `oracle: Address` (required), `consensus: Address` (required), `sentinel: SentinelConfig` (required), `observability` (default: `info`, metrics on `127.0.0.1:0`), flattened `driver` (`index`/`transactions`, all defaulted: `max_reorg_depth 5`, `block_time auto`, `max_in_flight_transactions 16`, `blocks_before_resubmit 2` — core `index/blocks.rs:78-86`, `tx/mod.rs:85-93`). `SentinelConfig` (`config.rs:49-59`): `fee_token: Address`, `voting_window: u64`, `engine: Url` — all required. The TODO (`config.rs:44-48`) says a default for `voting_window` is pending and explicitly keeps `fee_token/oracle/consensus` required "so a missing value fails loudly rather than silently using the wrong window/zero address" — enforced by the tests at `config.rs:118-134`. The sample TOML ships zero addresses as placeholders (`sentinel.sample.toml:24-31`) and is only checked for parseability (`config.rs:137-143`). Validation gaps: no check that `voting_window >= 2` (0/1 ⇒ `engine_timeout = 1 s`, `main.rs:54-60`), no check that `fee_token == oracle.FEE_TOKEN()`, `oracle.PROPOSER == consensus`, or that the RPC chain id matches the deployment.

### 3.4 SQLite (owned by core, used by the sentinel)

The sentinel defines no tables. Core creates `snapshots(block_number INTEGER PK, state TEXT)` (core `state/storage.rs:50-57`) storing the whole `State` as JSON per committed block, and `transactions(id, request TEXT, expires_at, nonce, submitted_at, executed_at)` (core `tx/storage.rs:69-80`). Sample config points at `/var/lib/safenet/sentinel/data/storage.db`; the integration test uses `sqlite::memory:`.

### 3.5 Metrics (metrics.rs)

`safenet_sentinel_engine_check_verdicts_total{verdict}` (`39-46`), `..._requests_proposed_total` (`51-56`), `..._requests_participated_total` (`60-65`), `..._bond_amount` histogram (`68-73`), `..._requests_resolved_total{outcome}` (`104-111`), `..._dispute_bond_slashed_amount` (`115-120`), `..._fee_reward_amount` (`129-134`). Recorded at `service.rs:135, 327-328, 513-516, 566, 590, 663`; `engine.rs:189`. Served on loopback ephemeral port by default (core `observability/mod.rs:29-35`).

---

## 4. Trust boundaries and untrusted inputs

1. **Proposal contents from chain** (`Consensus.TransactionProposed.transaction`, `oracleData`, `epoch`, `safeTxHash`): fully attacker-controlled (any sponsor can propose). Used to (a) compute the request id (`service.rs:108-115`) — `safeTxHash` is taken from the indexed topic, not recomputed from `transaction` (the `safe_tx_hash` helper is test-only, `hashing.rs:57-78`), and (b) forwarded verbatim to the engine as JSON (`effect.rs:62-67`). `data` may be large (block-gas-limit calldata) → large JSON body; bounded by chain. The FSM never inspects the transaction.
2. **`NewRequest` terms** (`bondTarget`, `slashAmount`, deadlines): set by the oracle from governance parameters (`SentinelOracle.sol:206-223`), not by the sponsor; trusted as-is (`service.rs:254-259`). `U256::from(bondTarget)` is the exact approval amount (`229`).
3. **Other sentinels' `Committed`/`Revealed` events**: drive `committed_count`/`revealed_count`/`approve_count`/`deny_count` (`service.rs:320, 363-368`) and thereby the early-finalize trigger and the dispute/timeout classification (`372, 626-627`). A registered sentinel can therefore influence _when_ we finalize and which branch we take (see H1, H3). `Revealed.reason` (arbitrary `string`) is decoded by alloy but never used by the FSM.
4. **Arbitrator/oracle outcomes** (`DisputeResolved.outcome`, `ArbitrationTimedOut`, `DisputeOutOfScope`): trusted; the sentinel always claims regardless of side (`service.rs:519-527, 568-574`).
5. **Engine responses**: the engine has total authority over the vote (`service.rs:173-180`). A compromised/buggy engine can (a) make the sentinel approve everything or deny everything — bounded loss of `slashAmount` per disputed request plus gas, (b) make it abstain (no loss), (c) NOT make it reveal something other than what it committed (reason is bounded to `R-u32.u32`; salt is local), (d) NOT touch the key or funds directly. There is no local sanity policy, loss budget, rate limit or kill switch — see H9. The docs' "trust boundary" (`docs/sentinel-engine.md`) isolates custody, not economic exposure.
6. **RPC data**: core's watcher handles reorgs up to `max_reorg_depth` (default 5) and can verify log completeness via bloom (`use_client_filtering`). A malicious RPC can withhold logs (missed participation), feed stale nonces/fees, or drop transactions; it cannot forge signatures. The FSM trusts block numbers for deadline math.
7. **Config file**: contains the raw private key; `Config::load` reads the whole file into a `String` that is not zeroized (`config.rs:63-64`).

---

## 5. Invariants

| Invariant | Where enforced | Notes |
| --- | --- | --- |
| A bond is committed at most once per request | Onchain `AlreadyCommitted` (`SentinelOracleCommitments.sol:91-101`). Client-side only by FSM state (`commit_vote` reachable solely from `WaitingForEngineCheck`/`WaitingForRequest`, `service.rs:181-183, 261-263`) | **Not enforced under replay**: reorg/restart re-emits `ApproveToken`+`Commit` (see §6, H7); the duplicate reverts onchain. |
| Reveal matches the commitment | `commit_hash(sentinel, id, approve, salt, reason)` (`hashing.rs:24-38`) and the `Reveal` action use the same `approve`/`reason` carried in state (`state.rs:38-55` doc "must never be re-derived"; `service.rs:213, 418-433`) and the same deterministic salt `signer.reveal_salt(id)` (`212, 425`). | Salt is **not stored**; it is re-derived from the private key by HKDF (`hashing.rs:49-53`, core `tx/signer.rs:59-64`). Survives restarts/reorgs as long as the key is unchanged. Key rotation between commit and reveal ⇒ `InvalidReveal` (assumed operational invariant, not enforced). |
| Commitment matches Solidity `computeHash` | `hashing.rs:31-37` = `keccak(approve‖salt‖sentinel‖requestId‖reason)` vs `abi.encodePacked(approve, salt, sentinel, requestId, reason)` (`SentinelOracleCommitments.sol:47-56`). Parity test `hashing.rs:177-192` mirrored by `contracts/test/SentinelOracle.t.sol:1188` (name grep only). | Match confirmed by reading both encodings (bool = 1 byte, address = 20 bytes, packed). |
| Request id computed identically to the contract | `oracle_tx_proposal_hash` (`hashing.rs:82-102`): EIP-712 domain `{chainId, verifyingContract=consensus}` (alloy omits absent fields ⇒ typehash `EIP712Domain(uint256 chainId,address verifyingContract)` = `ConsensusMessages.sol:16-18`), struct `TransactionProposal(uint64 epoch,address oracle,bytes oracleData,bytes32 safeTxHash)` (`bindings.rs:93-103` vs `ConsensusMessages.sol:27-30, 95-114`; `bytes` hashed per EIP-712 = `keccak256(oracleData)` at `Consensus.sol:261`). Parity vector `hashing.rs:149-166` ↔ `contracts/test/libraries/ConsensusMessages.t.sol:47`. | Match confirmed. Depends on config `consensus` and RPC chain id being right — not validated. |
| Never votes outside the window | Client: `Commit` expires at `commit_deadline`, `Reveal` at `reveal_deadline` (`service.rs:231, 239, 434`); core only _submits_ while `expires_at > latest` (`tx/storage.rs:150-156`). | Once submitted, a tx is resubmitted with bumped fees until mined even past expiry (`tx/mod.rs:224-237`, test `541-596`) — the contract, not the client, enforces the window; late txs revert and cost gas. |
| Fee-token approvals bounded | `ApproveToken { bond: bondTarget }` (`service.rs:228-231`) encoded as `approve(oracle, bond)` (`678-688`). | Exactly one bond per approval, overwriting (not accumulating) the allowance. Residual allowance after a reverted commit ≤ one `bondTarget`, spendable only via `commit` by `msg.sender == sentinel` (`SentinelOracle.sol:231-237`). |
| Effects idempotent under replay | Resume for unknown/advanced request ignored (`service.rs:156-170`, tests `1622, 1835`). | The _action_ side is not idempotent (H7). Replay can also _lose_ the effect (H10) or mis-order it against our own `Committed` (H2). |
| `self_committed` set at most once | Edge-triggered (`service.rs:325-329`). | Only while in `CollectingCommitments` — a `Committed(self)` seen in any other state is dropped (`307-319`) — root of H2. |
| Counts are complete | Assumed, **not enforced**: `Committed`/`Revealed` arriving in a phase the FSM does not expect are discarded with a warning (`307-319`, `347-362`). | Root of H1/H3. |
| Sentinel is registered/active | Not checked client-side; onchain `SentinelNotActive` (`SentinelOracle.sol:232`, `SentinelMap.sol:48-51`). | An unregistered sentinel burns gas on every request (approve succeeds, commit reverts). |
| No panics in transitions | `StateTransition` doc requires infallibility (core `state/mod.rs:76-78`); no `unwrap/expect/panic` in non-test sentinel code (§8). | ✔ |

---

## 6. Persistence, restarts and reorgs

**What is stored.** (a) `snapshots`: full `State` JSON after every processed log batch (core `state/mod.rs:236`), pruned to `[watcher.safe, latest]` (`driver.rs:257`, `state/storage.rs:151-161`); (b) `transactions`: every emitted action with `expires_at`, nonce, submission block, fee floor (`tx/storage.rs:69-104, 174-202`). Not stored: reveal salts (derived), engine results (folded into state at resume), in-flight effects.

**Crash consistency.** Order in `Driver::update` (`driver.rs:235-287`): tx-queue block reconcile → `state.handle_update` (commits the snapshot for a log batch) → prune → encode actions → `transactions.queue`. Hence:

- Log-batch actions: snapshot first, enqueue second. Crash in between ⇒ state says e.g. `CollectingVotes` but the `Reveal` was never enqueued ⇒ no reveal ⇒ **non-reveal slash** if a side gets established (`SentinelOracleRequests.sol:202-205, 289-297`). Narrow window, but real.
- Resume-driven actions (`ApproveToken`+`Commit`): enqueued immediately, state persisted only at the next log batch (`state/mod.rs:246-258`). Crash in between ⇒ txs are durable and will be sent, but state rolls back to `WaitingForEngineCheck` ⇒ the effect re-runs and our own `Committed` is replayed _before_ the new resume ⇒ ignored ⇒ H2.
- Tx queue itself: nonce reserved before sending; a send whose `record_submission` never ran is resubmitted with the same nonce (`tx/storage.rs:281-311`) — no double-spend of nonces.

**Restart = rollback + warp (always).** On startup the block watcher queues `Uncle{indexed.safe+1}` and, whenever `indexed.safe+1 <= latest_now − max_reorg_depth`, a `Warp{indexed.safe+1 ..= latest_now − max_reorg_depth}` (core `index/blocks.rs:255-278`). Since `indexed.safe` is the earliest retained snapshot ≈ `latest_at_shutdown − max_reorg_depth`, **any restart after ≥ 1 new block replays from the anchor and warps over the blocks that were already processed before shutdown**. During a warp the state machine receives **only `Message::Event`s, never `Message::NewBlock`** (`state/mod.rs:173-181, 200-239`), so every deadline-driven transition in `handle_block_advance` (`service.rs:390-470`) is deferred to the first post-warp block, while events emitted in the warped range are applied against the stale phase and, if "unexpected", discarded (`307-319`, `347-362`, `493-501`). Consequences are H2 (slash) and H3 (lockup). The unit tests never exercise a warp; the integration test never restarts a sentinel.

**Reorg (live).** `Uncle{n}` restores snapshot `n−1` (`state/storage.rs:124-143`) and re-applies blocks with proper `NewBlock` messages ⇒ FSM phases stay consistent. But the durable tx queue is not rolled back: re-emitted `Approve/Commit/Reveal/Finalize/Claim` are enqueued again ⇒ duplicate txs that revert onchain (H7). Reorg deeper than `max_reorg_depth` ⇒ driver exits (`driver.rs:206-215`).

**Duplicate transaction risk.** No idempotency key in the queue; identical calldata is queued as a new row. Reverting duplicates consume nonce + gas but cannot double-commit (contract guards).

**Cleanup / growth.** Pre-commit states expire at `commit_deadline`/`deadline`; `CollectingCommitments` without `self_committed` is dropped; `CollectingVotes` finalizes at `reveal_deadline+1`. `WaitingForDisputeResolution` never expires (`466`) — grows with the number of unresolved disputes (H6). Snapshots are O(#tracked requests) JSON per block; the `transactions` table keeps rows until executed/expired below `safe`.

---

## 7. Concurrency and cancellation

- The driver is single-threaded over state: one `select!` loop (`driver.rs:174-197`); `StateMachine` holds a `tokio::Mutex<Option<..>>` and a transition that panics would poison it (`state/mod.rs:171, 252`).
- Engine calls do **not** block the indexer: `Command::Effect` is spawned into a `JoinSet` (`effects.rs:54-62`) and its `Resume` is consumed later via an **unbiased** `select!` against the next watcher update (`driver.rs:227-230`). There is **no concurrency limit** on effects: N proposals in a block ⇒ N parallel HTTP requests to the engine (H4).
- Ordering: a whole log batch (one block, or up to `block_page_size = 100` blocks in a warp) is applied atomically before any effect is spawned or any resume is processed (`state/mod.rs:213-223`, `driver.rs:255-274`). Resume order relative to later blocks is unspecified (`state/mod.rs:46-51`); the FSM copes via per-state guards.
- Timeouts: per-request `reqwest` timeout = `engine_timeout` (`effect.rs:66`), which is a _fixed_ fraction of the configured window and ignores how much of the commit window has already elapsed (TODO `main.rs:45-49`). A resume arriving after `commit_deadline` finds the entry gone (`394-399`) and is ignored — safe (no vote).
- Cancellation: dropping the `EffectManager` aborts in-flight checks (`effects.rs:29-31`); on shutdown the loop breaks after finishing the current input (`driver.rs:184-186`). Nothing is done about queued-but-unsent transactions at shutdown; they persist and are sent on restart if not expired.
- The sentinel's own tx throughput is bounded by `max_in_flight_transactions = 16` per block (`tx/mod.rs:88, 204-219`).

---

## 8. Error handling

Non-test sentinel code contains **no** `unwrap()`, `expect()`, `panic!`, `unreachable!` or slice indexing (grep over all files, tests excluded). Items to note:

| Location | Construct | Safe? |
| --- | --- | --- |
| `main.rs:53-60` | `u64::try_from(u128).unwrap_or(u64::MAX)`, `saturating_sub/mul` | yes |
| `engine.rs:158` | `u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX)` | yes |
| `hashing.rs:68-69` | `SafeOperation::try_from(u8).unwrap_or(CALL)` | test-only code (`#[cfg_attr(not(test), expect(dead_code))]`); silently maps an invalid operation to `CALL` — harmless today |
| `service.rs:127` | `block.saturating_add(voting_window)` | yes |
| `service.rs:320, 363-368` | `u64 += 1` counters | cannot overflow (onchain counts are `uint16`) |
| `service.rs:328, 516, 590` | `U96.to::<u128>() as f64` | `U96→u128` lossless; `as f64` lossy but metrics-only |
| `service.rs:229` | `U256::from(U96)` | lossless |
| `bindings.rs:118-130` | `Operation` serialize `_ => Err` | unreachable for decoded events; returns an error rather than panicking |
| core `kdf.rs:20, 24-25` | `assert!(!domain.is_empty())`, `expect(..)` | domain is a non-empty constant (`hashing.rs:14`) |
| core `utils.rs:19-28` | `unix::signal(..).unwrap()` | startup only |

Engine-client error paths (`engine.rs:164-194`): every `Err` collapses to `Unknown`; the error is logged with `%err` (reqwest `Display` includes the URL) and counted. Transition-level: `Unknown` removes the entry (`service.rs:156-157, 176-179`) — there is no retry path. Driver-level: watcher errors are retried every 100 ms forever except deep reorgs (`driver.rs:206-225`); tx-queue RPC errors are logged and skipped, storage/signing errors are fatal (`tx/mod.rs:44-66`, `driver.rs:247-253, 276-284`). A log that fails ABI decoding makes the watcher return `DecodeLog` and the driver retry indefinitely (`index/events.rs:491-519`, `driver.rs:216-223`) — I found no realistic trigger for the watched contracts (see rejected list).

---

## 9. Cryptography and secrets

- **Request id**: EIP-712 (`hashing.rs:82-102`), matches contract (§5). Domain-separated by chain id and Consensus address.
- **Commitment**: `keccak256(approve(1B) ‖ salt(32B) ‖ sentinel(20B) ‖ requestId(32B) ‖ reason)` (`hashing.rs:31-37`). Binding to `sentinel` and `requestId` prevents cross-sentinel/cross-request replay (documented `16-22`). Hiding relies on the 32-byte secret salt: with `approve ∈ {0,1}` and `reason` from a tiny set, an observer without the salt cannot brute-force; an observer _with_ the salt for request X learns nothing about request Y's salt (independent HKDF `info`).
- **Salt**: `HKDF-SHA256(ikm = private key, salt = "safenet-sentinel-reveal-salt", info = requestId)` (`hashing.rs:14, 49-53`; core `kdf.rs:19-27`, `tx/signer.rs:59-64`). Deterministic, 32 bytes, key copy zeroized after use. No persistence needed (design decision cited at `hashing.rs:43-46`). Independently computed vector at `hashing.rs:199-208`. Weakness only if the key changes between commit and reveal, or if the key is shared with another use that also derives HKDF with the same domain (none in the crate).
- **Signer exposure**: `Signer`'s `Debug` prints only the address (core `tx/signer.rs:96-100`); `Config` derives `Debug` but is never logged (`main.rs:39` logs the path only). `tracing::trace!` in the driver logs full updates/resumes (`driver.rs:238, 261`) — these contain proposal data, not keys. The raw TOML text (with the key) lives un-zeroized in memory during `Config::load` (`config.rs:63-64`) and the key is on disk in plaintext (documented in the handbook).
- **Transport**: HTTP to the engine (plain by default, TLS possible via rustls); no auth, no pinning — per docs the engine must be on a private network. RPC URL may contain an API key; alloy transport errors logged with `?err` could include it (core, not read in depth).
- `effect.rs`: purely plumbing; no crypto.

---

## 10. Economic / security logic

- **Bond sizing**: `bondTarget = fee × bondMultiplier`, `slashAmount = fee × slashingMultiplier` are read from `NewRequest` (`service.rs:254-259`); the sentinel approves exactly `bondTarget` per request (`228-231`) to `oracle` (`681-684`) — never unlimited, never to a third party.
- **What the sentinel relies on** (observed in Solidity, not reviewed): commit pulls `bondTarget` (`SentinelOracle.sol:234-236`); non-revealers are slashed `slashAmount` when any side is established (`SentinelOracleRequests.sol:192-205, 289-297`); dispute losers are slashed `slashAmount` (`298-304`); winners get `fee/winningSideCount` (`252-269`); timeouts (`TIMED_OUT`, incl. arbitration timeout/out-of-scope) return bonds in full and **refund the fee to the sponsor** (`SentinelOracle.sol:273-277, 325-343`). Claims are per sentinel and only by the sentinel itself (`claim` uses `msg.sender`, `286-305`).
- **Attacker spamming proposals** (pays `fee` per request, sponsor-funded): (a) locks `bondTarget` per request from every sentinel for `COMMIT_WINDOW + REVEAL_WINDOW` blocks — the sentinel commits to _every_ request unconditionally (`181-183, 261-263`) with no cap on outstanding bonds; (b) costs each sentinel ≈ approve + commit + reveal + finalize + claim gas (hard-coded limits, `676-740`) — offset by the fee share only if the sentinel wins; (c) once a sentinel's fee-token balance is exhausted, each further request still triggers `approve` (succeeds) + `commit` (reverts) ⇒ pure gas burn with no local pre-check (H5); (d) if _no_ sentinel can vote, requests time out and the fee is refunded, making the flood nearly free for the attacker (inference, H4); (e) if the flood exceeds the tx queue's reveal throughput (16 in flight/block), late reveals are slashed (H4).
- **Griefing via engine latency**: a slow engine ⇒ resume after `commit_deadline` ⇒ no vote (safe). A slow engine ⇒ other sentinels' commits arrive while we are still `WaitingForEngineCheck` ⇒ undercount ⇒ H1 (claim lost). An overloaded engine (flood) ⇒ `Unknown` for everything ⇒ abstain.
- **Claim/slash conditions the sentinel assumes**: it always claims after `DisputeResolved` (partial slash leaves `bond − slash`, `service.rs:472-477`), after `ArbitrationTimedOut`/`DisputeOutOfScope` (`531-538`), after unanimous/timeout finalize (`656-671`). It never claims when it decided it "did not participate" (`631-633`) — which is wrong whenever its local counts are incomplete (H1/H3) or its reveal failed while others revealed (bond remainder `bond − slash` is claimable but never claimed).
- **"Approve nested transaction (#882)"**: this PR touched only `crates/sentinel-engine` (`checkers/nested.rs`, `bindings.rs`, `main.rs`, `checkers/mod.rs` per `git show --stat`): a new engine checker returns `secure` for any well-formed `execTransaction` call on another Safe, deferring value/refund/aggregation concerns to later PRs. For the sentinel it simply means more `secure` verdicts flow into approving votes; the sentinel code was not changed by it. (Engine code itself not read beyond the commit stat.)
- **"Claim on Arbitration timeout (#883)"**: added the `ArbitrationTimedOut`/`DisputeOutOfScope` bindings and `handle_arbitration_timeout` (`service.rs:539-576`), which claims for **any** tracked entry regardless of state (`550-556`), and left "adding a deadline to the waiting state" for a separate PR (PR description).

---

## 11. Test coverage

37 tests total (grep of `#[test]`/`#[tokio::test]`): `service.rs` 15, `engine.rs` 10, `hashing.rs` 5, `config.rs` 4, `state.rs` 2, `effect.rs` 1.

**service.rs (15, lines 1157-1850)** — pure FSM flow tests driving `apply_transition` with synthetic `EventLog`s and hand-fed `Message::Resume` (`resolve_engine_check`, `958-971`); the engine is never called (`876-893`):

1. `flow_unanimous_approve_finalizes_via_early_reveal_and_claims` (1158): both commits counted _after_ we enter `CollectingCommitments`, early finalize, `Finalize`+`Claim`. 2-3. `flow_dispute_claims_when_arbitration_matches_our_vote` (1425) / `..._contradicts_our_vote` (1448): claim on either `DisputeResolved` outcome. 4-5. `flow_claims_on_arbitration_timeout` (1471) / `flow_claims_on_dispute_out_of_scope` (1494) (#883).
2. `claimed_event_never_mutates_state` (1517).
3. `flow_finalizes_and_claims_on_genuine_reveal_timeout` (1549): nobody reveals ⇒ `Finalize`+`Claim`.
4. `stale_engine_check_resume_after_reorg_is_ignored` (1622).
5. `engine_check_denial_finalizes_with_the_engine_rule` (1636): `reason = "R-4.6"` carried into the commit hash. 10-11. `new_request_before_engine_check_*_starts_voting_on_resume` (1713, 1723): `request: Some` path. 12-13. `engine_check_failure_drops_*` (1737, 1759): `Unknown` drops the entry.
6. `waiting_engine_checks_expire_using_the_available_deadline` (1791).
7. `stale_engine_check_resume_does_not_disturb_an_already_advanced_request` (1835).

**engine.rs (10, 242-391)**: header forwarding, secure/insecure/unknown-rule/malformed-rule/abstain/503/unreachable/timeout (paused-time), `RuleId` JSON round trip. Uses a raw one-shot TCP responder (`211-240`). **hashing.rs (5)**: parity vectors for `safe_tx_hash`, `oracle_tx_proposal_hash`, `commit_hash`, `reveal_salt`; salt bound to request id. **config.rs (4)**: defaults, missing `oracle`, missing `engine`, sample TOML parses. **state.rs (2)**: serde round trips. **effect.rs (1)**: handler resumes with the engine outcome.

**Mocking seams**: the `Service`/`StateTransition`/`EffectHandler`/`ActionEncoder` split lets tests drive the FSM without core; `EngineClient` is only mockable via a real socket; `safenet-core`'s `test-util` feature is enabled in dev-deps (`Cargo.toml`) but no sentinel test I read uses `Provider::mocked`.

**Notable untested paths**: commits/reveals arriving in an "unexpected" phase (`307-319`, `347-362`); `finalize()` with `self_revealed=false && revealed_count>0` (`631-633`); `handle_arbitration_timeout` on a non-dispute state (`550-556`); any restart/warp or reorg replay through the real `StateMachine`; duplicate action emission; `SentinelEncoder` output (gas/calldata) has no unit test; JSON request body shape vs OpenAPI; `voting_window` edge values; `handle_new_request` duplicate/unexpected arms; `WaitingForRequest` expiry.

**Bash integration test** (`scripts/run_sentinel_integration_test.sh`): builds sentinel + engine; Anvil 1 s blocks; deploys ERC-20 fee token, `TestConsensus`, `SentinelOracle` with `COMMIT_WINDOW=5`, `REVEAL_WINDOW=5`, `BOND_MULTIPLIER=4`, `INITIAL_SLASHING_MULTIPLIER=2`, `ARBITRATION_TIMEOUT=100`; two sentinels (`voting_window = 10`, `database = sqlite::memory:`, `block_time = 1000`) each with its own reference engine (B's engine blocklists the tx token). Scenario 1: benign proposal ⇒ `RESOLVED_APPROVED`, no `DisputeResolved`, both claimed, bonds returned, fee split ≈ whole fee. Scenario 2: disputed proposal ⇒ `FROZEN`, arbitrator rules for approve, both claim, winner nets `+fee`, loser nets `−fee×2` (partial slash). Not covered: restarts, reorgs, more than two sentinels, engine latency skew, out-of-funds, arbitration timeout/out-of-scope, non-reveal slashing, spam.

---

## 12. Hypotheses

Evidence classes: **E2** = defect visible in cited code with a concrete input described; **I** = inference without a concrete trigger. Severity scale from the task statement.

### H1 — Commits seen before the engine answers are discarded, so early finalize fires too early and the claim is lost (bond + reward locked)

- `service.rs:307-319` (guard), `372-375` (trigger), `631-633` (drop).

```rust
let RequestState::CollectingCommitments { committed_count, self_committed, .. } = entry
else { tracing::warn!(..., "ignoring unexpected commitment"); return (state, Vec::new()); };
...
if *revealed_count < *committed_count { return (state, Vec::new()); }
let (update, actions) = self.finalize(entry, event.requestId);
...
if !*self_revealed && !timed_out { return (None, Vec::new()); }
```

- Reasoning: `committed_count` starts at 0 when `commit_vote` runs (`222`) and only counts `Committed` logs that arrive while in `CollectingCommitments`. Any sentinel that commits before our engine resumes (a block or two is enough; the reference engine does multi-block `eth_getLogs`) is not counted. Concrete input: `TransactionProposed`+`NewRequest` @b1; `Committed(OTHER)` @b2 (ignored); `Resume(Approved)`; `Committed(self)` @b3 ⇒ `committed_count=1`; `NewBlock(commit_deadline+1)` ⇒ `Reveal` queued; `Revealed(OTHER)` ⇒ `revealed_count=1 >= 1` ⇒ `finalize()` with `self_revealed=false` ⇒ entry removed, no actions. Our reveal still lands (already queued), but nothing ever emits `Claim`. Variant: if we reveal first, `Finalize`+`Claim` are emitted immediately; onchain `finalize` reverts `FinalizeTooEarly` (`SentinelOracleRequests.sol:175-177`) and `claim` reverts `RequestNotResolved`; entry already dropped ⇒ same outcome. Variant: if the request later becomes a dispute, `DisputeResolved` hits an untracked id ⇒ ignored.
- Confidence: 85%. Confirm: add a flow test inserting `committed_event(id, OTHER)` between `proposed_event` and `resolve_engine_check`; or on devnet give one engine artificial latency. Refute: show `Committed` cannot precede the resume in production (it can: `NewRequest` and other sentinels' commits are independent of our engine).
- Severity: **High** (funds locked, manual `claim` from the key recovers them; no theft; scales with request volume).

### H2 — Restart replay re-runs the engine check and discards our own replayed `Committed`, so the sentinel never reveals and is slashed

- `service.rs:307-319`, `325-329`, `415-417`; core `index/blocks.rs:255-278`, `state/mod.rs:182-189, 246-258`, `driver.rs:255-274`.

```rust
// service.rs:415-417
if !*self_committed { return false; }   // "Our own commit never landed onchain … drop"
// blocks.rs:261-278 (restart): queue Uncle{indexed.safe+1}; if uncle <= safe { Warp{uncle..=safe} }
```

- Reasoning: a resume's state is not snapshotted until the next log batch (`state/mod.rs:246-258`); the rollback anchor on restart is ≈ `max_reorg_depth` blocks back. If the process restarts within ~`max_reorg_depth` blocks (25 s on Gnosis) of the engine answering, the restored snapshot is `WaitingForEngineCheck`; replay re-applies `TransactionProposed` (effect re-spawned, HTTP round trip) and, in the same or next log page, our own `Committed(self)` — applied while still `WaitingForEngineCheck` ⇒ discarded (`307-319`). The new resume then runs `commit_vote` with `self_committed=false`, its duplicate `commit` reverts (`AlreadyCommitted`), and at `commit_deadline+1` the entry is dropped without a `Reveal`. Onchain our commitment stays `PENDING` ⇒ slashed `slashAmount` when any side is established (`SentinelOracleRequests.sol:202-205, 289-297`) and the remainder is never claimed. The same happens if the re-run check returns `Unknown` (entry removed at `176-179`).
- Confidence: 75% (depends on my reading that every restart warps/replays from the anchor and that a log page is applied before any resume — both cited). Confirm: integration test that restarts a sentinel 1-2 blocks after its commit lands; expect a `Revealed`-less request and `denySentinelCount/approveSentinelCount` with our bond slashed. Refute: show the replay orders the resume before `Committed(self)`.
- Severity: **Critical** (actual bond loss; no attacker needed; an attacker who can force restarts/crashes weaponises it).

### H3 — No `NewBlock` during warps: reveals in the replayed range are ignored, `finalize()` takes the wrong branch, and a disputed request's claim is lost

- `service.rs:347-362`, `456-465`, `626-633`, `664-671`; core `state/mod.rs:173-181, 200-239`.

```rust
// state/mod.rs:173-181 — Warp: no transition applied
Update::Block(BlockUpdate::Warp { from, to }) if ... => { let status = Status::WarpEvents{..}; (state, status, vec![]) }
// service.rs:626-627
let dispute = *approve_count > 0 && *deny_count > 0;
let timed_out = *revealed_count == 0;
```

- Reasoning: after any restart the FSM stays in whatever phase the anchor snapshot had for the whole warped range; `Revealed` logs in that range hit `CollectingCommitments` and are dropped; `DisputeResolved` in that range hits a non-`WaitingForDisputeResolution` state and is dropped (`493-501`). At the first post-warp `NewBlock` the FSM emits a (duplicate) `Reveal` and moves on; at `reveal_deadline+1` it finalizes with counts that miss everything from the warp: (a) `revealed_count==0` ⇒ `timed_out` branch ⇒ `Finalize` (may revert) + `Claim`; the claim succeeds if the request is already resolved but **reverts if it is `FROZEN`** (`RequestNotResolved`), and the entry is dropped, so the eventual `DisputeResolved`/`ArbitrationTimedOut` is ignored ⇒ bond (± slash/reward) locked; (b) partial counts (our reveal in the warp, theirs after) ⇒ `!self_revealed` ⇒ drop without claim.
- Confidence: 70%. Confirm: restart a sentinel during the reveal window of a request that ends disputed; observe no `Claimed` for it after `resolveDispute`. Refute: show `NewBlock` is delivered for warped blocks (it is not per `state/mod.rs:173-181`).
- Severity: **High** (fund lockup, manual recovery; plus duplicate/reverting txs).

### H4 — No bound on concurrent engine checks or on outstanding bonds; a proposal flood makes the sentinel abstain (cheap liveness DoS) and can push reveals past the deadline (non-reveal slash)

- `service.rs:139-143` (one effect per proposal), `226-242` (unconditional bond), core `effects.rs:54-62` (unbounded `JoinSet`), `tx/mod.rs:88, 204-219` (16 in flight/block), `tx/storage.rs:150-156` (expired reveals are silently dropped).

```rust
pub fn spawn(&mut self, effect: Effect) { ... self.tasks.spawn(async move { handler.perform_effect(effect).await }); }
```

- Reasoning: N proposals in one block ⇒ N parallel HTTP checks ⇒ engine/RPC saturation ⇒ timeouts ⇒ `Unknown` for all ⇒ no votes. If no sentinel votes, the request times out and (per `SentinelOracle.sol:273-277`, not reviewed) the sponsor's fee is refunded, so the attacker's marginal cost is proposal gas. Separately, if the sentinel does commit to N requests, it must land N reveals within `REVEAL_WINDOW` blocks at ≤ 16 tx/block; every reveal not _submitted_ by `reveal_deadline` is dropped (`expires_at`), leaving a `PENDING` commitment that is slashed once any sentinel reveals.
- Confidence: 60% (class I for the economics; the code facts are E2). Confirm: devnet flood of > 16×`REVEAL_WINDOW` proposals; measure abstain rate and slashed bonds. Refute: show the engine/RPC comfortably handles hundreds of concurrent lookbacks and that the queue drains faster than assumed.
- Severity: **High** (liveness of the oracle at low attacker cost; bounded bond loss under sustained flood).

### H5 — No fee-token balance/allowance/registration pre-check: every request costs gas even when the commit is doomed

- `service.rs:226-242`, `676-704`; core `tx/mod.rs:241-296` (no `eth_call`/gas estimation before send).

```rust
SentinelAction { kind: SentinelActionKind::ApproveToken { bond: U256::from(bond_target) }, expires_at: Some(commit_deadline) }.into(),
SentinelAction { kind: SentinelActionKind::Commit { id: request_id, hash }, expires_at: Some(commit_deadline) }.into(),
```

- Reasoning: with an empty bond balance, an unregistered/deactivated key, or a wrong `fee_token`, `approve` succeeds (55 k gas) and `commit` reverts (`SentinelNotActive`/`safeTransferFrom`), per request, forever; the handbook only says "logs will show". Combined with H4's refund mechanics, an attacker can keep every under-funded sentinel burning gas.
- Confidence: 75%. Confirm: run a sentinel with zero fee-token balance against the integration script; count reverted `commit`s. Severity: **Medium** (gas drain; no bond loss).

### H6 — `WaitingForDisputeResolution` has no deadline and the sentinel never calls `timeoutArbitration`

- `service.rs:466`, `646-654`; `bindings.rs:47-57` (no `timeoutArbitration` binding); PR #883 description.

```rust
RequestState::WaitingForDisputeResolution { .. } => true,
```

- Reasoning: if the arbitrator never rules and no third party calls the permissionless `timeoutArbitration`, the bond stays locked onchain and the entry stays in every snapshot forever. A sentinel is the party with the incentive to call it, yet it does not. Acknowledged as follow-up in #883.
- Confidence: 90% (E2). Severity: **Medium** (fund lockup contingent on arbitrator inactivity; unbounded local growth is slow).

### H7 — Action emission is not idempotent under replay: reorgs and restarts enqueue duplicate `approve/commit/reveal/finalize/claim` transactions that revert

- `service.rs:226-242, 426-437, 635-641, 664-671, 519-527`; core `driver.rs:266-284`, `tx/storage.rs:89-104` (no dedup), `tx/mod.rs:241-296` (no simulation).
- Reasoning: the tx queue persists across the rollback while the FSM is rewound, so every re-derived action is queued again. Duplicates always revert (`AlreadyCommitted`, `AlreadyRevealed`, `RequestNotPending`, `AlreadyClaimed`) but consume nonces and gas (`250_000` limit each, actual usage lower). Every sentinel also submits `Finalize` although only one can succeed (`635-641`).
- Confidence: 90%. Severity: **Low/Medium** (gas waste proportional to restarts × in-flight requests; not a safety issue).

### H8 — Hard-coded 55 000 gas for `approve` may be insufficient for the real fee token

- `service.rs:678-688`.

```rust
SentinelActionKind::ApproveToken { bond } => Transaction { to: self.fee_token, ..., gas: 55_000 },
```

- Reasoning: a plain OZ ERC-20 `approve` fits; a proxied token, an ERC-777-style hook, or a non-zero→non-zero-reverting token (USDT-style; a leftover allowance after a reverted commit would then brick all later approves) would fail every commit. Only a simple `MyToken` is exercised by the integration test.
- Confidence: 55% (I; depends on the deployed token). Severity: **Medium** if it hits (total non-participation + gas burn), else Info.

### H9 — The engine has unbounded authority over bond exposure; the sentinel has no local policy, loss budget or kill switch

- `service.rs:173-180`; `docs/sentinel-engine.md` trust-boundary section.
- Reasoning: a compromised/buggy engine (e.g. a false-positive checker such as the new nested-transaction rule from #882, or a `secure`-for-everything backdoor) converts directly into votes; loss is `slashAmount` per lost dispute, unbounded across requests, with no local rate/loss limit or "stop after N losses". The documentation claims the split "keeps transaction-verification compromise separate from custody" — true for the key, not for the funds.
- Confidence: 60% (I). Severity: **Medium** (bounded per request, unbounded in aggregate; requires engine compromise).

### H10 — Orphaned engine check after restart: a proposal older than the rollback anchor with an unresolved check is never re-checked and the request silently expires

- core `state/mod.rs:129-151`, `index/blocks.rs:255-278`; `service.rs:394-399`.
- Reasoning: in-flight effects die with the process; only proposals inside the replayed range re-spawn them. Snapshots older than the anchor may hold `WaitingForEngineCheck` with nothing in flight ⇒ dropped at `commit_deadline` ⇒ missed vote.
- Confidence: 70%. Severity: **Low** (missed participation only).

### H11 — Single-attempt engine client; any transient failure = no vote for that request

- `engine.rs:164-194`, `service.rs:176-179`.
- Confidence: 95% (E2, by design). Severity: **Low** (liveness; documented in handbook). Worth a bounded retry within the commit window.

### H12 — Configuration is not cross-validated; wrong values fail silently at runtime

- `config.rs:49-59`, `main.rs:50-62`, `service.rs:105-115`.
- Reasoning: wrong `consensus` ⇒ request ids never match `NewRequest` ⇒ never votes (debug-level log only, `287`); wrong `fee_token`/unregistered signer ⇒ gas burn per request (H5); `voting_window ∈ {0,1}` ⇒ 1 s engine timeout ⇒ all `Unknown`; `voting_window` far above `COMMIT_WINDOW` ⇒ engine timeout longer than the commit window (harmless but wasteful). No startup read of `oracle.FEE_TOKEN()`, `oracle.PROPOSER()`, `oracle.sentinelActiveAt(signer)`.
- Confidence: 85%. Severity: **Low** (operator error surface).

### H13 — Every sentinel submits `Finalize` for every request it participated in (K−1 reverts)

- `service.rs:635-641`. Confidence 95%. Severity **Info** (gas).

### H14 — `reqwest::Client::new()` defaults: redirects followed, proxy env honoured, no TLS pinning

- `engine.rs:113, 131-134`. A misconfigured proxy or a redirecting engine could route proposals elsewhere (public data) or delay checks. Confidence 50% (I). Severity **Info**.

### H15 — Private key material lingers in the un-zeroized config `String`

- `config.rs:63-64` vs core `tx/signer.rs:89-91`. Confidence 40% (I). Severity **Info**.

### Considered and rejected

- **Commit-hash / request-id mismatch with Solidity**: encodings match by inspection and both parity vectors exist on both sides (§5).
- **Unlimited ERC-20 approval or approval to a wrong spender**: bounded to `bondTarget`, spender is `oracle` (`service.rs:681-684`).
- **Salt entropy / predictability**: HKDF-SHA256 keyed by the private key, domain-separated, request-bound; not persisted so cannot be lost on restart or leaked from the DB.
- **Engine can inflate `reason` to blow the reveal gas limit**: `reason` is `RuleId(u32,u32).to_string()` (`engine.rs:37-49`, `service.rs:175`), ≤ 24 bytes.
- **Engine can alter the vote after commit**: the verdict is consumed once at resume; reveal uses state carried from that moment (`state.rs:38-55`).
- **Key printed in logs / Debug**: `Signer` Debug prints the address only; `Config` is never logged.
- **Panics / unchecked arithmetic**: none in non-test code (§8).
- **Invalid UTF-8 in `Revealed.reason` / `DisputeResolved.context` stalling the indexer** (a `DecodeLog` error is retried forever, `driver.rs:216-223`): alloy-sol-types 1.6.0 (`Cargo.lock`) is, to my recollection, lossy for `string` (`from_utf8_lossy`), which would make this a non-issue; **source not read locally — unverified**. Worth a 5-minute check by a reviewer with the registry available.
- **Duplicate `TransactionProposed` resets progress**: guarded (`service.rs:119-126`, test 1190-1204).
- **Stale resume creating state after a reorg**: guarded (`156-170`, tests 1622/1835).
- **Late engine resume committing after the deadline**: entry is gone by then (`394-399`) and the tx queue never submits past `expires_at`.
- **Metrics `as f64` casts**: metrics only.

---

## 13. Suggested review checklist (ordered by risk)

1. **Phase-lag robustness** (`service.rs:295-385, 390-470, 607-672`; core `state/mod.rs:166-258`, `index/blocks.rs:244-368`): Can `Committed(self)`/`Committed(other)`/`Revealed`/`DisputeResolved` ever be discarded for a request we have bonded on? Trace a restart 1-2 blocks after our commit (H2) and a restart in the reveal window of a disputed request (H3). Should the FSM reconcile against `getRequest`/`getCommitment` instead of local tallies?
2. **Early-finalize trigger** (`service.rs:372-375, 631-633`): is `revealed_count >= committed_count` sound when `committed_count` can be undercounted (H1)? Should `finalize()` ever drop an entry for which `self_committed` is true without emitting `Claim`?
3. **Non-reveal exposure**: enumerate every path where the sentinel has committed but emits no `Reveal` (crash between snapshot and enqueue §6; H2; expired reveal in a backlog H4). Each is a `slashAmount` loss.
4. **Claim completeness**: enumerate every terminal path and confirm a `Claim` is emitted whenever `getCommitment(id, self).bondAmount > 0` (`service.rs:519-527, 568-574, 664-671`; missing: `631-633`, H3 revert-then-drop, H6).
5. **Flood behaviour** (core `effects.rs:54-62`, `tx/mod.rs:88, 204-219`; `service.rs:139-143, 226-242`): what bounds concurrent engine checks, outstanding bonds, and reveal throughput? Does the sentinel ever decline to commit?
6. **Pre-flight checks before spending gas** (`service.rs:676-740`; core `tx/mod.rs:241-296`): balance/allowance/`sentinelActiveAt` at startup and per request; gas limits vs the real fee token (H5, H8); simulate before send?
7. **Replay idempotency of actions** (core `driver.rs:266-284`, `tx/storage.rs:89-104`): dedup key or `eth_call` guard for re-emitted `commit/reveal/finalize/claim` (H7); is `Finalize` from every sentinel intended (H13)?
8. **Engine trust and policy** (`engine.rs:164-194`, `service.rs:173-180`, docs): retry/backoff within the window (H11); loss budget / kill switch (H9); confirm the JSON body matches `openapi.yaml` with a unit test; confirm `block = event.block` semantics with the engine's expectation.
9. **Timeouts** (`main.rs:45-62`, `config.rs:49-59`): derive `engine_timeout` from onchain `COMMIT_WINDOW` (read once at startup) rather than `voting_window`; validate `voting_window`; add a deadline to `WaitingForDisputeResolution` and a `timeoutArbitration` action (H6).
10. **Config validation at startup** (`main.rs:37-73`): cross-check `oracle.FEE_TOKEN()`, `oracle.PROPOSER()`, chain id, registration (H12).
11. **Hashing parity** (`hashing.rs`, `bindings.rs:93-103`; `ConsensusMessages.sol:95-114`, `SentinelOracleCommitments.sol:47-56`): re-run both parity vectors after any change; confirm alloy's `Eip712Domain` field omission behaviour is stable across alloy upgrades.
12. **Decode robustness** (core `index/events.rs:491-519`, `driver.rs:216-223`): verify alloy 1.6.0 string decoding is lossy; otherwise any committed sentinel can stall every other sentinel with an invalid-UTF-8 `reason`.
13. **Secrets hygiene** (`config.rs:61-67`, `engine.rs:113`): zeroize config text; consider disabling proxies/redirects for the engine client; ensure RPC URLs with keys are not logged by transport errors.
