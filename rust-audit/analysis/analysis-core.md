> Generated at commit 82b3e0d by a read-only analysis agent (Claude Fable 5.1) with no toolchain available. Every statement is class I or E2 by inspection; nothing here was executed. The Manager spot-checked the citations of the top hypotheses only. Treat every hypothesis as a lead to confirm or refute, never as a finding.

# `safenet-core` — technical map and risk analysis

Crate: `crates/core` (version 0.2.0, edition 2024, 7,644 LOC incl. tests). Method: every `.rs` file in the crate was read in full (`cat -n`); the consumer crates were only grepped/spot-read where cited. `cargo`/`rustc` are not installed and no dependency sources (alloy 2.0.5, sqlx 0.9.0, hkdf 0.13.0, tokio 1.52.3 per `Cargo.lock`) are on disk, so every statement about dependency behaviour is marked "not verified on disk". All line numbers are from the current checkout (`main` @ 82b3e0d).

Legend for evidence: **E2** = defect visible in cited code with a concrete input described; **I** = inference without a concrete trigger.

---

## 1. Purpose and runtime shape

**What it is.** The shared runtime for the `validator` and `sentinel` binaries (the `sentinel-engine` uses only `observability`, `provider::Provider` and `utils`, see `crates/sentinel-engine/src/main.rs:17`). It provides: (a) a chain indexer that emits an ordered stream of block/log updates with reorg detection (`index/`), (b) a pure, snapshot-persisted, rollback-able state machine (`state/`), (c) an async effect runner (`effects.rs`), (d) a durable transaction submission queue with nonce/fee management (`tx/`), (e) an alloy provider wrapper with request metrics (`provider/`), (f) HKDF key derivation (`kdf.rs`), (g) logging/metrics bootstrap (`observability/`, `metrics.rs`), (h) SQLite pool construction and a shutdown signal (`utils.rs`).

**Entry points.**

- `Driver::new(service, provider, signer, pool, addresses, config)` — `driver.rs:120-150`. Order: `StateMachine::new` (creates `snapshots` table, loads tip snapshot) → `state.block_status()` (MIN/MAX of snapshots) → `Watcher::new(provider, config.index, addresses, block_status)` (does RPC: latest block + `safe..=latest` headers) → `TransactionQueue::new` (creates `transactions` table). Failure returns `Err` and both binaries exit with a non-zero code (`crates/validator/src/main.rs:71-79`, `crates/sentinel/src/main.rs:75-83`).
- `Driver::queue_action(action)` — `driver.rs:157-161`; bypasses the state machine (validator uses it for `SetValidatorStaker`, `crates/validator/src/main.rs:82-93`).
- `Driver::run(self)` — `driver.rs:170-198`.
- `observability::init(config)` — `observability/mod.rs:56-63` installs tracing (`logging.rs:14-21`, JSON when stdout is not a TTY), Prometheus exporter (`observability/metrics.rs:16-48`), and `crate::metrics::initialize()` (`metrics.rs:81-90`, which `tokio::spawn`s the tokio-metrics reporter — must be called inside a runtime).
- `utils::connect_sqlite(options)` — `utils.rs:56-62`.

**Async task structure of `Driver::run`** (`driver.rs:170-198`). There are no channels and the driver spawns nothing itself. It is a single loop on the caller's task:

```
loop {
  select! { biased;
    _ = shutdown => break,                       // utils::shutdown_signal(): SIGTERM|SIGINT (utils.rs:17-36)
    input = self.next_input() => input,          // driver.rs:206-231
  }
  // then, NOT cancellable by shutdown:
  match input { Err(_) => break /* only ExceededMaxReorgDepth */, Ok(i) => self.update(i).await }
  if update returned Err => break
}
```

- `next_input` (`driver.rs:206-231`) is an inner `select!` (unbiased) between (1) an infinite retry loop around `watcher.next()` that sleeps `STEP_RETRY_DELAY = 100 ms` (`driver.rs:26`) after any watcher error except `index::Error::Blocks(ExceededMaxReorgDepth)` which is returned (`driver.rs:211-215`), and (2) `effects.next()` (`effects.rs:74-89`), which is `JoinSet::join_next` and stays pending forever when no effect is running (`effects.rs:86`).
- `update` (`driver.rs:235-287`) for a watcher update: read `watcher.block_status()` → `transactions.update_block_status(status)` (RPC errors are logged and swallowed, `driver.rs:247-253`) → `state.handle_update(update)` (fatal on error) → `state.prune(status.safe)` (fatal on error) → encode `Command::Action`s via `ActionEncoder` and `spawn` `Command::Effect`s on the `EffectManager` (`driver.rs:266-274`) → `transactions.queue(txs)` (RPC errors swallowed, others fatal, `driver.rs:276-284`). For a resume: `state.handle_resume(resume)` then the same command dispatch.
- Effects run as detached tokio tasks inside a `JoinSet<Resume>` owned by the `EffectManager` (`effects.rs:54-62`); dropping the driver aborts them (`effects.rs:30-31`). Panicked effect tasks are logged and skipped (`effects.rs:81`).
- Other background tasks: the tokio-metrics reporter (`metrics.rs:89`) and the Prometheus HTTP listener installed by `PrometheusBuilder::install()` (`observability/metrics.rs:31-33, 43-45`; internal spawning not verified on disk).

**Exit behaviour.** `run` returns `()` in every case: shutdown signal, `ExceededMaxReorgDepth`, any `state::Error`, any non-RPC `tx::Error` (storage/signing). Both binaries then `return Ok(())` (`crates/validator/src/main.rs:96-98`, `crates/sentinel/src/main.rs:86-88`), i.e. **the process exit code is 0 after a fatal error** (see H3). A panic inside `apply_transition` (service code) unwinds the main task instead and yields a non-zero exit.

**How services plug in** (all in `driver.rs:65-93`):

- `Service` (`driver.rs:73-93`): associated types `State: Default + Serialize + DeserializeOwned`, `Event: Events`, `Action`, `Effect`, `Resume`, `Transition: StateTransition<State>`, `Effects: EffectHandler<Effect, Resume>`, `Actions: ActionEncoder<Action>`; `components(self) -> (Transition, Effects, Actions)`.
- `StateTransition::apply_transition(&self, state, Message) -> (state, Vec<Command>)` (`state/mod.rs:79-94`) — infallible and pure by contract (`state/mod.rs:76-78`).
- `EffectHandler::perform_effect(&self, Effect) -> impl Future<Output = Resume> + Send` (`effects.rs:14-26`) — infallible; "may be performed more than once for the same chain message" (`effects.rs:21-24`).
- `ActionEncoder::encode_action(&self, Action) -> (Transaction, Option<u64 /*expires_at*/>)` (`driver.rs:65-70`).
- `Events` (`index/events.rs:28-39`) generated by `watcher_events!` (`index/events.rs:536-594`) from alloy `sol!` `*Events` enums; decoding tries each enum in order and returns the first successful `decode_raw_log` (`events.rs:581-590`). Note it is **address-agnostic** (see H4).
- Implementations: validator `crates/validator/src/service/mod.rs:71-117`, `state/mod.rs:402-501`, `service/effect.rs:243-256`, `service/action.rs:81+`; sentinel `crates/sentinel/src/service.rs:743-831`, `effect.rs:54-76`, `bindings.rs:164`.

---

## 2. Module map

| File | LOC | Responsibility | Key pub types / fns | Main deps |
| --- | --- | --- | --- | --- |
| `src/lib.rs` | 25 | module tree, `pub use Driver` | — | — |
| `src/driver.rs` | 318 | wires watcher + state machine + effects + tx queue; run loop; error policy | `Config`, `Error`, `ActionEncoder`, `Service`, `Driver::{new,queue_action,run}` | tokio `select!`, all sibling modules |
| `src/effects.rs` | 220 | concurrent effect execution via `JoinSet` | `EffectHandler`, `EffectManager::{new,spawn,next}`, `Pure` | tokio `JoinSet` |
| `src/index/mod.rs` | 468 | composes block + event watcher; recovery for "resource not found" logs | `Config`, `Error`, `Update`, `Watcher::{new,next,block_status}` | alloy `EthRpcErrorCode` |
| `src/index/blocks.rs` | 1330 | head following, polling cadence, reorg detection with bounded history | `BlockTime`, `Config`, `BlockUpdate`, `BlockStatus`, `Error`, `InvalidatedBlock`, `BlockWatcher::{new,next,status,ready,revalidate_last_block}` | alloy `Provider::get_block`, `Clock`, metrics |
| `src/index/events.rs` | 1516 | log fetching strategies (single/multi/client-filtered), warp paging, decode+sort, `watcher_events!` | `Events`, `EventLog`, `EventUpdate`, `Config`, `Error`, `EventWatcher::{new,on_block_update,on_block_invalidated,next}` | alloy `get_logs`, `Filter`, `bloom` |
| `src/index/bloom.rs` | 523 | bloom helpers (`may_contain_log` unused in prod: module is `#[allow(dead_code)]`, `index/mod.rs:5-6`) | `may_contain_log`, `compute_logs_bloom` | alloy `Bloom` |
| `src/index/clock.rs` | 103 | wall clock (prod) / tokio clock (test) | `Clock::{start,now_ms,sleep_until}` | `SystemTime` / tokio time |
| `src/state/mod.rs` | 644 | ordered update validation, transition application, rollback, resume handling | `Error`, `Message`, `Command`, `StateTransition`, `Commands`, `StateMachine::{new,with_init,block_status,handle_update,handle_resume,prune}` | `SnapshotStore`, tokio `Mutex` |
| `src/state/storage.rs` | 294 | per-block JSON snapshots in SQLite | `Error`, `SnapshotStore::{new,current,status,commit,reorg,prune}` | sqlx, serde_json |
| `src/tx/mod.rs` | 719 | submission queue: nonce/fee caches, in-flight cap, stale resubmission, underpriced detection | `Error`, `lift_intermittent_error` (crate), `Config`, `TransactionQueue::{new,queue,update_block_status}` | alloy provider, `Signer`, `TransactionStorage`, regex |
| `src/tx/storage.rs` | 507 | `transactions` table: enqueue, nonce allocation, submission/execution markers, pruning | `Error`, `Submission`, `Status`, `TransactionStorage::{new,enqueue,count_in_flight,next_transaction,record_submission,count_outstanding,mark_executed,prune,unmark_executed,stale_submissions}` | sqlx, serde_json (`json_set`) |
| `src/tx/fees.rs` | 109 | priority-fee cap and ≥10 % replacement bump | `cap_priority_fee`, `bump` | alloy `Eip1559Estimation` |
| `src/tx/signer.rs` | 118 | local EOA signer, HKDF derivation over the key | `Signer::{new,address,sign_transaction,derive_key}`, `SignedTransaction`, `SigningError` | alloy `PrivateKeySigner`, k256, zeroize, `kdf` |
| `src/tx/types.rs` | 87 | queued transaction shapes and EIP-1559 build | `Transaction`, `AllocatedTransaction::build` | `fees::bump` |
| `src/provider/mod.rs` | 166 | alloy `RootProvider<AnyNetwork>` + request metrics/trace layer; cached chain id | `Provider::{connect,chain_id}` (+`mocked*` under `test-util`) | alloy `ClientBuilder`, tower |
| `src/kdf.rs` | 81 | HKDF-SHA256 with domain salt | `derive_key` | hkdf, sha2 0.11 |
| `src/serialization.rs` | 34 | serde via `FromStr`/`Display` | `from_str::{serialize,deserialize}` | serde |
| `src/metrics.rs` | 90 | crate metrics + tokio runtime metrics | `rpc_requests_total`, `block_number`, `uncled_blocks_total`, `initialize` (crate-private module) | metrics, tokio-metrics |
| `src/observability/mod.rs` | 94 | config + init | `Config`, `InitError`, `init` | tracing-subscriber |
| `src/observability/logging.rs` | 21 | subscriber setup | `init` | tracing-subscriber |
| `src/observability/metrics.rs` | 80 | Prometheus HTTP exporter, `/health` | `serve` | metrics-exporter-prometheus |
| `src/utils.rs` | 97 | shutdown signal, SQLite pool, `Json` formatter | `shutdown_signal`, `connect_sqlite`, `Json` | tokio signal, sqlx |

---

## 3. External interfaces

### 3.1 JSON-RPC methods used by core

| Method | Where | Notes |
| --- | --- | --- |
| `eth_chainId` | `provider/mod.rs:135` once at connect; afterwards `get_chain_id` returns the cached value (`provider/mod.rs:163-165`) | chain id used for tx signing (`tx/mod.rs:246-248`) |
| `eth_getBlockByNumber(n \| "latest", false)` via `get_block(id).hashes()` | `blocks.rs:227-236`; callers `initialize` (`245`, `308`), `next` (`395`), `revalidate_last_block` (`504`) | only `number`, `hash`, `parent_hash`, `timestamp`, `logs_bloom` are kept (`blocks.rs:150-156`) |
| `eth_getLogs {blockHash, address[], topics[[t0…]]}` | `Fetch::SingleQuery(Hash)` `events.rs:404-411` | default new-block path |
| `eth_getLogs {blockHash, address[], topics[[t0]]}` ×N topics, concurrently (`try_join_all`) | `Fetch::MultipleQueries` `events.rs:412-440` | fallback after `block_single_query_retry_count` failures; also single-block warp pages |
| `eth_getLogs {blockHash}` (no address/topic filter) | `Fetch::ClientFiltered` `events.rs:441-466` | `use_client_filtering = true`; bloom equality check `events.rs:450` |
| `eth_getLogs {fromBlock,toBlock,address[],topics[[…]]}` | warp pages `events.rs:303-354` via `BlockFilter::Range` (`events.rs:147`) | page size halves on failure, resets on success |
| `eth_getTransactionCount(signer, <blockNumber>)` | `tx/mod.rs:304-312` (block = `block_status.latest`, or `latest` tag before any status) | cached per block (`tx/mod.rs:156-159`) |
| `eth_feeHistory` via alloy `estimate_eip1559_fees()` | `tx/mod.rs:326` | alloy's default estimator (not on disk); tests consume exactly one `FeeHistory` mock per estimate (`tx/mod.rs:407-415, 446`) giving `max_fee = 2·base + reward`. The handbook's RPC table lists `eth_maxPriorityFeePerGas` (`docs/validator-handbook.md:29-31`); core does not call it directly — not verified whether alloy's estimator falls back to it. |
| `eth_sendRawTransaction` | `tx/mod.rs:265` | error payload string-matched (`tx/mod.rs:362-368`) |
| filters / subscriptions | **none** — polling only |  |
| `eth_call` | **none in core**; services call contracts directly (`crates/validator/src/main.rs:49-52, 83-86`) |  |

### 3.2 Retry / timeout behaviour (`provider/mod.rs`)

- `Provider::connect` = `ClientBuilder::default().layer(ObservabilityLayer).connect(url)` (`provider/mod.rs:129-137`). **No retry layer, no request timeout, no rate limiting** is configured in core. The `ObservabilityLayer` (`provider/mod.rs:57-118`) only traces the full request/response JSON at `trace` level (`86-94`) and counts `safenet_core_rpc_requests_total{method,result}` (`112-114`). Batch packets are supported by id-matching (`67-83, 95-111`).
- Retries live in callers: driver `next_input` (100 ms, unbounded, `driver.rs:216-223`), block polling (`block_retry_delays` then `block_time`, `blocks.rs:392-416`), event pages (halving, `events.rs:337-348`), new-block logs (`retries` counter, `events.rs:383-392`), tx queue (per block, `tx/mod.rs:185-197`).
- The validator handbook claims "exponential backoff for some RPC requests" (`docs/validator-handbook.md:108`); nothing in core implements exponential backoff.

### 3.3 SQLite

- Tables are created with `CREATE TABLE IF NOT EXISTS`; there is no `sqlx::migrate!` and no schema versioning anywhere in core (grep `migrate` → none).
- `snapshots (block_number INTEGER PRIMARY KEY, state TEXT NOT NULL)` — `state/storage.rs:50-57`. State is JSON (`serde_json`, `storage.rs:106, 140`).
- `transactions (id INTEGER PRIMARY KEY, request TEXT NOT NULL, expires_at INTEGER NULL, nonce INTEGER NULL, submitted_at INTEGER NULL, executed_at INTEGER NULL)` — `tx/storage.rs:69-80`. `request` is the `Transaction` JSON; `nonce`/fees are injected with `json_set` on read (`tx/storage.rs:144-156, 293-299`) and fees written back with `json_set` as hex QUANTITY strings (`tx/storage.rs:177-193`). **No UNIQUE constraint on `nonce`, no indexes.**
- Queries of note: nonce allocation `UPDATE … SET nonce = MAX(?, COALESCE((SELECT MAX(nonce)+1 FROM transactions),0)) WHERE id = (oldest queued, unexpired) RETURNING json_set(...)` (`tx/storage.rs:145-156`); rollback `DELETE … WHERE block_number >= ?` then `SELECT … = parent` inside one transaction (`state/storage.rs:129-141`); prune keeps `>= safe` and always the MAX row (`state/storage.rs:152-159`).
- Services share the same pool/file: validator `SecretStore::new(pool)` (`crates/validator/src/service/mod.rs:50`) — plaintext secrets, per handbook (`docs/validator-handbook.md:77-79`).
- `connect_sqlite` (`utils.rs:56-62`): `SqlitePoolOptions::new().idle_timeout(None).max_lifetime(None)`; everything else is the sqlx 0.9 default. Per sqlx documentation (not verified on disk): `journal_mode = WAL`, `synchronous = FULL`, `foreign_keys = ON`, `busy_timeout = 5 s`, `locking_mode = NORMAL`, `create_if_missing = false`, pool `max_connections = 10`, `acquire_timeout = 30 s`. The options string comes straight from config via `FromStr` (`crates/validator/src/config.rs:28-29`), so operators can pass `?mode=rwc` etc.; the sample configs use `sqlite:/var/lib/...` **without** `mode=rwc` (`crates/validator/validator.sample.toml:22`), while the integration scripts add it (`scripts/lib/shared_test_scripts.sh:142`).

### 3.4 Configuration (all `#[serde(default)]`)

- `driver::Config { index, transactions }` — `driver.rs:30-37`; flattened into the service configs so tables are `[index]`/`[transactions]` (`crates/validator/src/config.rs:36-38`).
- `index::Config` — `index/mod.rs:20-29`, `deny_unknown_fields`, flattens:
  - `blocks::Config` (`blocks.rs:47-87`): `block_time` = `"auto"` (chain 100 → 5000 ms, 11155111 → 12000 ms, else startup error `UnknownBlockTime`, `blocks.rs:34-43`) or integer ms; `block_propagation_delay` 500; `block_retry_delays` [200,100,100]; **`max_reorg_depth` 5** (0 = any reorg is fatal, `blocks.rs:66-68`); `start_block` None. No range validation (e.g. `block_time = 0` is accepted and makes `next` poll in a tight loop after the retry delays, `blocks.rs:413-415`).
  - `events::Config` (`events.rs:73-104`): `block_page_size` 100 (NonZero); `block_single_query_retry_count` 3 (NonZero); **`use_client_filtering` false**; `max_logs_per_query` None (check disabled); `fallible_events` {} (topic0 set whose _query failures_ are dropped, `events.rs:426-436`; no service sets it — grep outside core finds none).
- `tx::Config` (`tx/mod.rs:71-93`): `max_in_flight_transactions` 16; `blocks_before_resubmit` 2; **`priority_fee_cap_percentage` None** (`f64`; ≤ 0 disables tips, ≥ 100 no-op, `fees.rs:11-19`; `NaN` → clamped to 0 → tips disabled).
- `observability::Config` (`observability/mod.rs:18-36`): `log_filter` "info"; `metrics_address` 127.0.0.1:0.
- Validation is limited to serde types (`NonZero*`, `deny_unknown_fields` on the index/tx/observability structs). No cross-field validation (e.g. `blocks_before_resubmit = 0` resubmits every block; `max_in_flight_transactions = 0` never submits).

### 3.5 Metrics exported by core (`metrics.rs`)

`safenet_core_rpc_requests_total{method,result=success|failure}` counter (`28-36`); `safenet_core_block_number{status=seen|processed}` gauge (`63-70`, driven from `driver.rs:136-141, 291-318`); `safenet_core_uncled_blocks_total` counter (`73-78`, incremented at `blocks.rs:430, 520`); tokio runtime metrics via `tokio_metrics::RuntimeMetricsReporterBuilder` (`89`, names not verified). HTTP: Prometheus text on every path except `/health` → `OK` (`observability/metrics.rs:9-10`, test `62-79`). `/health` is liveness only; it keeps answering `OK` after `Driver::run` has returned (nothing tears the exporter down).

---

## 4. Trust boundaries and untrusted inputs

| Input | Parsed by | Validation performed | Behaviour on malformed / inconsistent data |
| --- | --- | --- | --- |
| Block headers (`eth_getBlockByNumber`) | alloy → `BlockHeader` copy `blocks.rs:229-235` | Parent-hash linkage against the previous header during init (`blocks.rs:311`) and against `recent.back()`/`safe` on each new block (`blocks.rs:421-439`); hash equality on revalidation (`blocks.rs:505`). **Not** validated: returned `number == requested number`; `timestamp` plausibility (feeds sleep arithmetic `blocks.rs:538-551`); header hash is trusted as given (cannot be recomputed from `.hashes()` data). | `None` for the pending block → retry/wait (`395-416`); `None` during init → `MissingBlock` → `Driver::new` fails (`240-242`); mismatched number → state machine `BadUpdate` → driver exits (`state/mod.rs:190-199, 240`); persistent parent mismatch during init → unbounded re-scan loop (`blocks.rs:315-326`). |
| Logs (`eth_getLogs`) | alloy `Log` → `decode_and_sort` `events.rs:491-519` | `block_number` and `log_index` must be present (`501-502`) else `DecodeLog`; ABI decode via `SolEventInterface::decode_raw_log`; sort by `(block, index)`; the state machine re-checks strict ordering and range membership (`state/mod.rs:207-210`). Client-filtered mode additionally requires `compute_logs_bloom(logs) == header.logs_bloom` (`events.rs:450`) and filters address/topic0 client-side (`458-465`). **Not** validated: `log.block_hash` vs requested hash, `removed` flag, emitting address in node-filtered modes, completeness in node-filtered modes (`max_logs_per_query` optional). | Missing logs: undetected except in client-filtered mode (and see H2). Duplicate or out-of-order logs: `BadUpdate` → driver exits (fail-stop). Undecodable log matching the filter: `DecodeLog` → retried forever (H8). Logs outside the requested block: `BadUpdate` → exit. |
| Receipts | not used | — | execution is inferred solely from `eth_getTransactionCount` (`tx/mod.rs:185-194`). |
| Fee estimates (`eth_feeHistory` via alloy) | `Eip1559Estimation` `tx/mod.rs:326` | optional cap (`fees.rs:12-31`); bump ≥ 10 % over the last _accepted_ fees (`fees.rs:39-56`, `types.rs:64-66`). No upper bound; no sanity vs. balance. | garbage estimates propagate into signed txs (H6). |
| Account nonce (`eth_getTransactionCount`) | `u64` `tx/mod.rs:308-312` | none; trusted for both execution marking (`tx/storage.rs:224-235`) and allocation floor (`145-149`). | a forked/lagging node can mark unexecuted txs executed or open a nonce gap (H11). |
| RPC error payloads | string regexes `tx/mod.rs:362-368`; code −32001 `index/mod.rs:135-142` | — | unmatched messages fall into "retry without bump" (H7). |
| DB rows (`snapshots`, `transactions`) | `serde_json::from_str` (`state/storage.rs:76, 140`; `tx/storage.rs:166, 309`), `i64→u64` via `try_from` (`Overflow`/`BlockNumberOverflow` errors) | no integrity/hash binding, no chain-id/contract binding | any deserialization failure is fatal (driver exits, or `Driver::new` fails). A DB from another chain/deployment or an orphaned fork is accepted silently (H1). |
| Service-provided actions | `Transaction { to, value, data, gas }` `types.rs:20-29` + `expires_at` | none; `gas` is used verbatim (no estimation) | a wrong `gas` yields a permanently failing tx that still holds its nonce (see §6.5). |
| Effect resumes | `Resume` values from `EffectHandler` | none; applied to whatever the current state is (`state/mod.rs:250-258`) | stale resumes after rollback are the service's problem (H10). |
| Config file | serde/TOML | `deny_unknown_fields` on nested structs; `Signer` from 32-byte hex (`signer.rs:84-94`) | numeric edge values accepted (§3.4). |
| Process signals | `utils.rs:17-36` | — | SIGTERM/SIGINT break the loop only between inputs (§7). |

---

## 5. Invariants

| # | Property | Enforced where | Status |
| --- | --- | --- | --- |
| I1 | Block updates arrive in order and each `New{n}` builds on the previously emitted block | `blocks.rs:421-439` (running); `state/mod.rs:172-199` status guards reject out-of-order updates with `BadUpdate` | enforced while running; **not enforced across restarts** — snapshots carry no block hash (`state/storage.rs:50-54`) and `initialize` never compares persisted vs. chain (`blocks.rs:255-278`) (H1) |
| I2 | Logs handed to transitions are strictly sorted `(block, index)` and inside the announced range | `events.rs:517`; `state/mod.rs:207-210` | enforced (fail-stop) |
| I3 | Exactly one process/driver writes `snapshots`/`transactions` | — | assumed, not enforced (no lock file/lease; SQLite locking only serialises statements) |
| I4 | For every `Uncle{n}` the watcher can emit, snapshot `n-1` exists | prune keeps rows `>= safe` plus MAX (`state/storage.rs:152-159`); watcher never uncles at or below `safe` (`blocks.rs:435-439, 453-462`); the driver prunes with the watcher's `safe` (`driver.rs:257`) | enforced by coupling of two modules, not asserted; violation → `MissingSnapshot` → fatal |
| I5 | Allocated nonces are unique and ≥ chain nonce | `MAX(?, MAX(nonce)+1)` in one `UPDATE … RETURNING` (`tx/storage.rs:145-156`) | enforced for this queue; no `UNIQUE(nonce)` constraint; external senders using the key are tolerated by design (`tx/storage.rs:126-128`) but silently drop the displaced action (§6.5) |
| I6 | A transaction that holds a nonce is eventually mined or replaced (never dropped) | `stale_submissions` includes `submitted_at IS NULL` (`tx/storage.rs:297`); expiry applies only to `nonce IS NULL` rows (`tx/storage.rs:152, 261`) | enforced; consequence: head-of-line blocking is permanent (§6.5, H7) |
| I7 | Effects are idempotent / replay-safe | contract only (`effects.rs:21-24`, `state/mod.rs:59-62`); validator burns nonces atomically (`crates/validator/src/secrets/store.rs:205-218`) | assumed by core, enforced by services |
| I8 | Transitions do not depend on resume ordering | documented `state/mod.rs:49-50` | assumed |
| I9 | Rollback restores exactly snapshot `n-1` and discards live state | `state/mod.rs:186-188` replaces the live state | enforced for state; in-flight effects/resumes are not rolled back (H10) |
| I10 | Bloom semantics | `may_contain_log` (`bloom.rs:23-35`) is **unused** in production; the only bloom use is exact equality of recomputed vs. header bloom (`events.rs:450`) | equality is stricter than "no false negatives" and is correct iff the node returned exactly the block's logs; correctness of `compute_logs_bloom` is anchored by a real Gnosis block vector (`bloom.rs:72-522`) |
| I11 | Monotonic/synchronised clock | `Clock::now_ms` is `SystemTime` (`clock.rs:33-38`) | assumed roughly aligned with block timestamps; not enforced (H13) |
| I12 | `BlockStatus.safe` never decreases while running | `safe` only changes by eviction (`blocks.rs:453-462`) or revalidation (never touches `safe`, `blocks.rs:481-482`) | enforced while running; on restart it is recomputed as `node_latest − depth` (`blocks.rs:246`) which may be lower or higher than the persisted `safe` |
| I13 | Chain id is constant for the process lifetime | cached once (`provider/mod.rs:135`) | assumed (RPC endpoint switching chains is undetected) |
| I14 | At most `max_in_flight_transactions` unexecuted allocated txs | `tx/mod.rs:205-206` loop bound; resubmissions do not add rows | enforced |
| I15 | State transitions never observe `Uncle` while warping | guards `state/mod.rs:182-185` (`BadUpdate` otherwise); watcher only calls `blocks.next()` when the event watcher is idle (`index/mod.rs:85-97`) and `revalidate_last_block` refuses when the queue front is not `New` (`blocks.rs:486-490`) | enforced |
| I16 | `Status::BlockPending{pending}` after commit equals `blocks.last + 1` | `state/mod.rs:225-234` with `checked_add` (`EndOfChain`) | enforced |

---

## 6. Persistence, restarts and reorgs

### 6.1 Reorg detection (`index/blocks.rs`)

- State: `safe: SafeBlock{number,hash}` (the anchor, considered final) and `recent: VecDeque<BlockHeader>` holding up to `max_reorg_depth` blocks after it (`blocks.rs:177-191`). Exactly one block is pushed per `next()`; when `recent.len() > max_reorg_depth` the oldest is evicted **into** `safe` (`blocks.rs:445-462`).
- On each fetched block: if `recent.back().hash != block.parent_hash`, pop it, rewind `pending` to its number, emit `Uncle{number}` (`blocks.rs:421-434`) — one uncle per call, so an N-block reorg produces N `Uncle`s followed by N `New`s (test `blocks.rs:975-1031`). If `recent` is empty and `safe.hash != parent_hash` → `Err(ExceededMaxReorgDepth(depth))` (`blocks.rs:435-439`), returned repeatedly on every subsequent call (test `blocks.rs:1078-1086`).
- Revalidation path (`blocks.rs:483-535`), triggered only by a JSON-RPC −32001 on the logs query (`index/mod.rs:106-130`): re-fetch the last emitted block by number; if the hash differs **or the node returns `None`** (`blocks.rs:504-507`), truncate `recent` at it, rewind `pending`, clear the queue and push `Uncle`.

### 6.2 What `max_reorg_depth` does and what happens when exceeded

`max_reorg_depth` = size of the mutable window = number of `recent` headers kept (`blocks.rs:59-69`). Exceeding it while running → `ExceededMaxReorgDepth` propagates through `Watcher::next` → `Driver::next_input` returns it un-retried (`driver.rs:211-215`) → `run` logs "unrecoverable watcher error; exiting" and returns (`driver.rs:186-190`) → **process exits with code 0** (§1). Integration test: `scripts/run_validator_deep_reorg_test.sh:77-97` (asserts process death + log line, not exit code). On restart, nothing remembers the failure: `initialize` re-anchors on the node's current `latest − depth` and replays from the persisted `MIN(snapshots)` (`blocks.rs:255-278`), so the service resumes from a snapshot that may belong to the orphaned fork (H1; acknowledged in PR #834's message: "the block hash of the last safe block is not persisted").

### 6.3 Startup sequence (resume)

`indexed = {safe: MIN(block_number), latest: MAX(block_number)}` (`state/storage.rs:86-101`). If `safe+1 <= latest` a synthetic `Uncle{safe+1}` is queued (`blocks.rs:261-266`) → state rolls back to snapshot `safe` (`state/mod.rs:182-189`, `state/storage.rs:124-143`). If `safe+1 <= node_safe` a `Warp{safe+1, node_safe}` is queued (`blocks.rs:271-278`); then `New` for every `recent` block `>= safe+1` (`blocks.rs:342-365`). With `start_block` and no snapshots: `Warp{start_block, node_safe}` without a fake uncle (`blocks.rs:279-289`). Consequences: (a) every restart replays at least the last `max_reorg_depth` blocks of events, re-emitting their actions and effects (H5); (b) a node lagging more than `latest − safe` blocks behind the DB makes the first real `New` mismatch `BlockPending` → `BadUpdate` → exit (`state/mod.rs:190-199`) — a fail-stop with an unhelpful message.

### 6.4 State machine and snapshot lifecycle

- `Update::Block(New{n})` → `Message::NewBlock(n)` applied, status `BlockEvents{n}`, **no commit** (`state/mod.rs:190-199`).
- `Update::Logs{blocks, logs}` → each log applied, then `snapshots.commit(blocks.last, &state)` (`state/mod.rs:200-239`). Commit is an upsert (`state/storage.rs:105-116`).
- `Update::Block(Uncle{n})` → `snapshots.reorg(n)` (transactional delete `>= n`, read `n-1`) → live state replaced (`state/mod.rs:182-189`).
- `Update::Block(Warp{from,to})` → status only (`state/mod.rs:173-181`); each page commits at its last block.
- `handle_resume` mutates live state without committing (`state/mod.rs:246-258`); it is persisted with the next log-range commit. Actions produced by a resume are queued immediately (`driver.rs:260-284`).
- `prune(safe)` after every update (`driver.rs:257`) deletes snapshots `< safe` except the newest (`state/storage.rs:151-161`).
- Any error inside `handle_update` after `mem::take` leaves `inner = None` → `Poisoned` on next use (`state/mod.rs:170-171, 252`); moot because the driver exits on the first error.

### 6.5 Transaction queue: nonces, fees, expiry, restart

- **Nonce management.** `nonce()` fetches `eth_getTransactionCount(signer, latest_known_block)` once per block (`tx/mod.rs:300-317`). Allocation: `MAX(chain_nonce, MAX(nonce)+1)` (`tx/storage.rs:145-156`). Execution marking: every allocated nonce `< chain_nonce` is marked `executed_at = latest` (`tx/storage.rs:224-235`) — a nonce consumed by _any_ sender counts as executed, so an action can be marked executed without its call ever running (documented intent `tx/storage.rs:126-128`; the handbook warns operators never to reuse the key, `docs/validator-handbook.md:59`).
- **Ordering per block** (`tx/mod.rs:145-200`): cache invalidation → prune at `safe` → `unmark_executed(safe+1)` on the first status or `unmark_executed(latest+1)` when `latest` went backwards → (only if `latest` advanced and outstanding > 0) fetch nonce → `mark_executed` → `resubmit_stale(latest)` → `submit_pending(latest)`.
- **Fee replacement.** Fresh estimate (capped, `tx/mod.rs:322-345`) then `bump(fresh, previous)` raises each component to ≥ `prev + ceil(prev/10)` (`fees.rs:39-56`, `types.rs:64-66`). The floor advances only on mempool acceptance or an "underpriced replacement" rejection (`tx/mod.rs:265-283`; PR #686). Stale = `submitted_at <= latest − blocks_before_resubmit` or `submitted_at IS NULL` (`tx/storage.rs:293-305`). **The cap is not applied to bumps** (`fees.rs:38`) and there is no ceiling (H6).
- **Expiry.** `expires_at` only prevents _allocation_ (`tx/storage.rs:152`); expired queued rows are pruned once `safe >= expires_at` (`tx/storage.rs:259-265`). Once a nonce is allocated the row is never dropped; a permanently rejected tx (e.g. insufficient funds, wrong `gas`) blocks every later nonce until it succeeds (I6).
- **Crash consistency.** Sequence per tx: allocate nonce (DB commit) → sign → `eth_sendRawTransaction` → `record_submission` (DB). Crash after broadcast and before recording leaves `submitted_at NULL` → on restart it is resubmitted with `bump(fresh, None)` = fresh estimate (`tx/storage.rs:297`, `types.rs:81-86`); same nonce and payload, so it either replaces, is rejected ("already known"/underpriced → floor recorded), or the original mines. **A tx cannot be executed twice** (same nonce) and **cannot be lost** (row persists). A tx _can_ be broadcast more than once (by design). Across the driver, snapshot commit and action enqueue are separate transactions (`driver.rs:255-284`); a crash between them loses the queued action, but the restart replay (§6.3) re-emits it — which is also why duplicates are routine (H5).
- **In-flight txs on restart.** They remain `nonce NOT NULL, executed_at NULL`; the first status update unmarks everything above `safe`, re-marks with the live nonce, resubmits those older than `blocks_before_resubmit` with bumped fees (fees are persisted in `request` JSON, `tx/storage.rs:177-193`), and counts them against `max_in_flight_transactions`.
- **Reorg.** `latest` decreasing → `unmark_executed(latest+1)`; the next advancing block re-marks from the live nonce before resubmitting (`tx/mod.rs:166-197`). `prune` uses `safe`, so an executed tx is deleted only after it is reorg-final.

---

## 7. Concurrency and cancellation

- **Ownership.** `Driver` owns everything by value; `StateMachine` wraps its state in a `tokio::sync::Mutex<Option<…>>` (`state/mod.rs:102`) although all callers hold `&mut self` — the lock is redundant. The only real concurrency is between the driver task and effect tasks (`effects.rs:54-62`), which share `Arc<Handler>` and — in the validator — the same SQLite pool (`crates/validator/src/service/mod.rs:50`). Validator's handler serialises its nonce generator with a `tokio::sync::Mutex` (`crates/validator/src/service/effect.rs:111, 148-158`).
- **Cancel-safety of the `next_input` select** (`driver.rs:227-230`, unbiased): `EffectManager::next` is `JoinSet::join_next` — cancel-safe (`effects.rs:69-73`, test `166-179`). The `update` arm can be dropped mid-await: `BlockWatcher::next` mutates only after its awaits, except `self.pending.timestamp_ms += self.block_time` between polls (`blocks.rs:414`) — a dropped future during the "slot skipped" wait therefore pushes the next poll one `block_time` later; `EventWatcher::{warp,block}` set `self.step` only after `fetch_logs` completes (`events.rs:325, 383`); `revalidate_last_block` mutates after its await (`blocks.rs:504-533`). A cancelled RPC request is simply repeated. No queued `BlockUpdate` can be lost: `next()` returns queued updates synchronously (`blocks.rs:387-389`) and `Watcher::next` calls `on_block_update` synchronously after the await (`index/mod.rs:93-95`).
- **Shutdown.** `select!{biased; shutdown, next_input}` (`driver.rs:175-182`) makes the signal win whenever the loop is at the top, but `self.update(input).await` is not cancellable (`driver.rs:184-192`): with no RPC timeouts (§3.2) a hung `eth_sendRawTransaction`/`eth_getTransactionCount` inside `update` blocks shutdown until the orchestrator kills the process. Before `run` (during `Driver::new`) no handler is installed, so SIGINT terminates immediately.
- **Error propagation from spawned tasks.** Effect panics are logged and dropped (`effects.rs:81`), so the state machine never receives the resume — a silent loss for that effect. Tokio-metrics and Prometheus tasks are fire-and-forget.
- **Ordering assumptions.** Resumes interleave arbitrarily with block/log updates and are applied to whatever state exists (`state/mod.rs:250-258`); after a rollback, resumes spawned before the rollback still arrive (H10). Effects spawned during a warp for historical events run against the live world (e.g. sentinel `EngineCheck` HTTP calls for every historical `TransactionProposed`, `crates/sentinel/src/service.rs:137-144`).
- **Backpressure / growth.** The watcher is pull-based (bounded). The `JoinSet` is unbounded: a warp over a busy range spawns one task per triggering event concurrently. `MultipleQueries` issues one `eth_getLogs` per watched topic concurrently (`events.rs:413`; validator watches three contracts' full event sets). `transactions` rows: executed rows pruned at `safe`, expired queued rows pruned at `safe`, never-expiring queued rows and in-flight rows persist until executed. `recent` ≤ `max_reorg_depth`; `queue` ≤ `depth + 2`.
- **Timeouts.** None in core (RPC, SQLite acquire uses sqlx default 30 s, effect futures untimed; the sentinel adds its own engine timeout, `crates/sentinel/src/main.rs:50-62`).
- **Locks in SQLite.** With sqlx's default 5 s busy timeout (not verified), a long effect-side write transaction can make a driver-side `commit` fail with `SQLITE_BUSY`, which is fatal (`driver.rs:193-196`).

---

## 8. Error handling — every unwrap/expect/panic/index/cast/unchecked arithmetic in non-test code

(Test modules start at: `effects.rs:101`, `index/mod.rs:144`, `blocks.rs:554`, `events.rs:596`, `bloom.rs:42`, `clock.rs:64`, `state/mod.rs:283`, `state/storage.rs:170`, `tx/mod.rs:370`, `tx/storage.rs:320`, `fees.rs:58`, `signer.rs:102`, `kdf.rs:29`, `observability/mod.rs:65`, `observability/metrics.rs:50`. `Provider::mocked*` (`provider/mod.rs:139-150`) is gated on `test-util`/`cfg(test)`; `Clock` has `cfg(test)` variants.)

| Location | Construct | Safe? |
| --- | --- | --- |
| `blocks.rs:314` `number += 1` | unchecked add on `u64` bounded by `latest_number` | safe unless the node reports `latest == u64::MAX` (absurd) |
| `blocks.rs:336` `.expect("the range scan always includes the safe block")` | `recent.pop_front()` | provably safe: loop runs at least once because `safe <= latest_number` via `saturating_sub` (`246`) and restarts keep `number <= latest_number` |
| `blocks.rs:405` `retry_count += 1` | `usize` add | safe (would need 2^64 iterations) |
| `blocks.rs:406` `.get(index).copied()` | bounds-checked | safe |
| `blocks.rs:414` `self.pending.timestamp_ms += self.block_time` | unchecked `u64` add | overflow only if an RPC timestamp is ~u64::MAX/1000; wraps in release (no `overflow-checks` profile set; workspace `Cargo.toml` has no `[profile]`) |
| `blocks.rs:427`, `525` `last.timestamp * 1000` | unchecked mul on RPC-supplied timestamp | not provably safe (untrusted input); wraps in release, panics in debug; practical impact: mis-timed polling only |
| `blocks.rs:453` `self.recent.len() as u64` | widening cast | safe |
| `blocks.rs:457` `.expect("checked len > max_reorg_depth above")` | pop after `len > depth` | provably safe |
| `blocks.rs:502` `&self.recent[last_index]` | index from `rposition` | safe |
| `blocks.rs:541` `number + 1`, `timestamp * 1000 + self.block_time` | unchecked | as above; `number + 1` overflows only at `u64::MAX` |
| `blocks.rs:549` `timestamp_ms + block_propagation_delay` | unchecked | as above |
| `events.rs:97-98` `NonZeroU64::new(100/3).expect` | constants | safe |
| `events.rs:310` `page_size.get() - 1` | NonZero | safe |
| `events.rs:332` `query_to_block + 1` | reached only when `query_to_block != to_block` and `query_to_block <= to_block` | provably safe |
| `events.rs:342` `.expect("halving a nonzero page size stays nonzero")` | `div_ceil(2)` of ≥1 | provably safe |
| `events.rs:512-513` `unwrap_or(…)` | defaults for error message | safe |
| `index/mod.rs:140` `.code() as i64` | cast | safe |
| `tx/mod.rs:354` `.expect("valid regex")` | static literals, `LazyLock` | safe |
| `tx/storage.rs:304` `.unwrap_or(-1)` | sentinel so `submitted_at <= -1` never matches | safe, intentional |
| `fees.rs:16` `(… * PRECISION as f64).round() as u128` | float→int `as` saturates; `NaN.max(0.0)` = 0 | safe (NaN silently disables tips) |
| `fees.rs:24` `/ (PRECISION - scaled_percent)` | divisor > 0 because `scaled_percent < PRECISION` checked at `17` | provably safe |
| `fees.rs:29` `base_fee + max_priority_fee_per_gas` | unchecked `u128` add | safe: `base_fee = max_fee − prio` (saturating) and `prio' <= prio`, so sum ≤ original `max_fee` |
| `fees.rs:54` `saturating_add`, `div_ceil` | — | safe |
| `kdf.rs:20` `assert!(!domain.is_empty())` | documented panic | callers pass constants (`crates/sentinel/src/hashing.rs:14`) |
| `kdf.rs:25` `.expect("32 bytes is far below…")` | HKDF max 8160 bytes | safe |
| `clock.rs:36-37` `unwrap_or_default()`, `.as_millis() as u64` | truncating cast | safe for ~584 M years |
| `utils.rs:20`, `26` `unix::signal(...).unwrap()` | panics if signal registration fails | safe inside tokio; would panic outside a runtime |
| `metrics.rs:89` `tokio::spawn` | panics outside a runtime | called from `observability::init` inside `#[tokio::main]` in all three binaries |
| `driver.rs:139`, `308`, `315` `as f64` | precision loss above 2^53 | safe |
| `driver.rs:301` `saturating_sub(1)` | — | safe |
| `state/mod.rs:171`, `252` `mem::take(..).ok_or(Poisoned)?` | explicit | safe (poisoning is fail-stop) |
| `state/mod.rs:139`, `227`, `231` `checked_add` → `EndOfChain` | — | safe |
| `state/storage.rs:125` `checked_sub` → `BlockNumberOverflow` | — | safe |
| all `i64::try_from`/`u64::try_from`/`usize::try_from` in storage | `?` into `Overflow`/`BlockNumberOverflow` | safe |

No `panic!`, `unreachable!`, `todo!` or `unimplemented!` exist in non-test code. `match effect {}` on `Infallible` (`effects.rs:97`) is sound.

---

## 9. Cryptography and secrets

- **`kdf::derive_key(ikm, domain, message_parts) -> B256`** (`kdf.rs:19-27`): HKDF-SHA256 (RFC 5869) with **salt = `domain`** (must be non-empty, asserted), **IKM = `ikm`**, **info = concatenation of `message_parts`** via `expand_multi_info`, L = 32 bytes. Domain separation relies on distinct salts; the info is a plain concatenation so `["foo","bar"]` ≡ `["foobar"]` (documented by test `kdf.rs:66-74`) — callers must use fixed-length or self-delimiting parts. Reference vector `kdf.rs:36-46` was "independently computed" per the comment (not re-verified here).
- **`Signer::derive_key(domain, message)`** (`signer.rs:59-64`): IKM is the **secp256k1 transaction-signing private key** (`self.0.to_bytes()`), zeroized after use (`signer.rs:62`). The only consumer is the sentinel's deterministic reveal salt: domain `b"safenet-sentinel-reveal-salt"`, message = 32-byte `request_id` (`crates/sentinel/src/hashing.rs:14, 49-53`). The validator does not use it (grep). Because the salt is a PRF output of the key, publishing it on reveal leaks nothing about the key; determinism means duplicate commits for the same request are identical.
- **`Signer`** (`signer.rs:23-48`): wraps alloy `PrivateKeySigner`; `sign_transaction` signs `TxEip1559` synchronously and returns EIP-2718 bytes. `Deserialize` reads a `B256` hex string and zeroizes the buffer (`signer.rs:84-94`). `Debug` prints only the address (`signer.rs:96-100`), so `Config: Debug` derives (`crates/validator/src/config.rs:20`) cannot leak the key. `Clone` duplicates the key in memory (sentinel clones it, `crates/sentinel/src/main.rs:68, 78`). Whether alloy/k256 zeroize the key on drop is not verified on disk. The TOML file contents (`fs::read_to_string`, `crates/validator/src/config.rs:44`) and toml intermediates are not zeroized.
- **Logging exposure.** `provider/mod.rs:86-94` traces the full JSON-RPC request and response at `trace` level (integration configs enable `safenet_core=trace`, `scripts/lib/shared_test_scripts.sh:156`); this includes signed raw transactions and all chain data, no key material. `tx/mod.rs:259-264` logs nonce/hash at debug. No code path formats the private key.
- **Persisted secrets.** Core's `snapshots.state` JSON contains whatever the service state holds; the validator keeps taken nonces in that state (`crates/validator/src/secrets/store.rs:203-204`), so the `snapshots` table contains FROST nonce secrets in plaintext. No encryption at rest in core; the handbook says so (`docs/validator-handbook.md:79`).
- **`serialization.rs`** (`5-34`): generic `FromStr`/`Display` serde adapter; no security semantics. It is what parses the SQLite URL and the log filter.
- **Randomness.** None in core (services draw their own RNG).

---

## 10. Test coverage

Counts (`#[test]`/`#[tokio::test]`): `blocks.rs` 23, `events.rs` 19, `tx/mod.rs` 9, `state/storage.rs` 7, `tx/storage.rs` 7, `effects.rs` 6, `state/mod.rs` 6, `index/mod.rs` 5, `kdf.rs` 4, `fees.rs` 3, `bloom.rs` 2, `clock.rs` 2, `observability/mod.rs` 2, `observability/metrics.rs` 1, `signer.rs` 1; **0** in `driver.rs`, `provider/mod.rs`, `tx/types.rs`, `utils.rs`, `metrics.rs`, `serialization.rs`, `logging.rs`.

Mocking seams: `Provider::mocked`/`mocked_with_chain` over alloy's `Asserter` (`provider/mod.rs:139-150`, feature `test-util` or `cfg(test)`; consumers enable it only in dev-deps, `crates/validator/Cargo.toml:27-28`). The `Asserter` pops responses in FIFO order regardless of method, so tests pin the _number and order_ of RPC calls (`assert!(asserter.read_q().is_empty())`). `Clock` switches to tokio's paused clock under `cfg(test)` (`clock.rs:8-11, 44-47`); 14 tests use `start_paused = true`. SQLite tests use `sqlite::memory:`.

What is covered: block init/resume/start_block/warp combinations, retry cadence, deep and mid-init reorgs, `ExceededMaxReorgDepth`, revalidation (`blocks.rs:619-1276`); every `Fetch` strategy, `TooManyLogs`, `fallible_events`, bloom mismatch, paging/halving, invalidation errors (`events.rs:784-1515`); watcher composition incl. the −32001 recovery (`index/mod.rs:248-394`); state apply/commit/resume/restart/reorg/warp+prune (`state/mod.rs:409-643`); storage reorg atomicity and prune retention (`state/storage.rs:189-293`); tx queue: dedup of identical statuses, startup reconciliation, reorg unmark, expiry, failed vs. underpriced replacements (`tx/mod.rs:437-718`); storage nonce allocation/expiry (`tx/storage.rs:348-506`); fee cap/bump edge values; HKDF vectors; signature round-trip; effect manager cancel-safety and panic skipping.

Notable untested paths:

- `Driver::run/next_input/update` entirely (error policy, select interleaving, shutdown) — only integration scripts.
- `use_client_filtering` **after** `block_single_query_retry_count` failures (falls back to unverified `MultipleQueries`, `events.rs:369-380`) — the existing test `events.rs:1385-1448` covers the fallback without client filtering.
- State-machine defensive rejections (`BadUpdate` for unsorted/out-of-range logs, `Uncle` while warping, wrong `New` number) — no tests.
- `BlockWatcher::next` when `get_block` returns an RPC _error_ (not `None`); `initialize` when the persisted range is ahead of the node.
- `is_transaction_underpriced` negative cases (e.g. plain "transaction underpriced", Nethermind messages) — only positives tested (`tx/mod.rs:424-435`).
- `stale_submissions` with `submitted_at IS NULL` after a crash; `mark_executed` with nonces consumed externally; `prune` of executed rows.
- `cap_priority_fee` with `NaN`/`inf`; `Provider` observability layer; `connect_sqlite`; `Clock` under wall-clock skew (prod variant is `cfg(not(test))`).
- Integration scripts exercise: two-validator happy path, deep reorg exit, restart across a reorg within `max_reorg_depth = 10` (`scripts/run_validator_reorg_nonce_test.sh:75, 148-152`).

---

## 11. Hypotheses

### H1 — Reorg-depth safety is not persisted: a restart resumes from an unverified, possibly orphaned snapshot

- **Where:** `index/blocks.rs:244-289` (init uses only numbers), `state/storage.rs:50-57, 86-101` (no hash column), `blocks.rs:435-439` (check exists only while running).
- **Excerpt** (`blocks.rs:255-266`):
  ```rust
  if let Some(indexed) = indexed {
      // The earliest retained snapshot is the rollback anchor. Replay
      // everything after it, but only emit an uncle when there are newer
      // snapshots to discard. ...
      let uncle = indexed.safe.checked_add(1);
      if let Some(uncle) = uncle && uncle <= indexed.latest {
          self.queue.push_back(BlockUpdate::Uncle { number: uncle });
      }
  ```
- **Reasoning:** snapshots are keyed by block number only. On restart the watcher anchors on the node's current `latest − depth` and the state machine rolls back to snapshot `MIN(block_number)` — a block that was "final" for the previous run but is never compared to the chain. If a reorg deeper than the retained window happened while the process was down (or the process exited on `ExceededMaxReorgDepth` and was restarted, or a backup was restored, or `max_reorg_depth = 0` and any reorg occurred), the service continues from state derived from orphaned blocks, applies canonical events on top, and never notices. PR #834's description explicitly records this gap.
- **Evidence class:** E2. Trace: run a validator with `max_reorg_depth = 2` against Anvil; stop it; `anvil_reorg 5`; start it; observe no error and `safenet_core_block_number` advancing (the deep-reorg script does the reorg while running and never restarts).
- **Confidence:** 85 %. **Severity:** High (state divergence of a consensus participant; nonce reuse is still prevented by the validator's burn-on-use, but selection/epoch views can be wrong, leading to invalid shares or stalled ceremonies).
- **Confirm/refute:** unit test: build `BlockWatcher::new(indexed = Some{safe:900,latest:905})` against a mocked chain where block 900's hash differs from what was persisted — there is nowhere to persist it, which is the finding. Fix direction: store `(block_number, block_hash)` in `snapshots` and verify the anchor in `initialize` (fail loudly on mismatch).

### H2 — `use_client_filtering` integrity protection silently degrades to unverified node-filtered queries after 3 failures (~300 ms)

- **Where:** `index/events.rs:362-398`; retry cadence `driver.rs:216-223`.
- **Excerpt** (`events.rs:369-380`):
  ```rust
  let fetch = if retries < self.config.block_single_query_retry_count.get() {
      if self.config.use_client_filtering {
          Fetch::ClientFiltered { block_hash, logs_bloom }
      } else {
          Fetch::SingleQuery(BlockFilter::Hash(block_hash))
      }
  } else {
      Fetch::MultipleQueries(BlockFilter::Hash(block_hash))
  };
  ```
- **Reasoning:** the flag exists for nodes that return empty/incomplete logs when queried too soon after a block (`docs/validator-handbook.md:33-41`). Each bloom mismatch returns `IncompleteLogs`, the driver retries after 100 ms, and after `block_single_query_retry_count = 3` attempts the watcher switches to per-topic node-filtered queries **with no bloom check**. Any transient failure (HTTP 429, timeout) counts too. If the node still serves incomplete logs at attempt 4, the empty result is accepted, the snapshot is committed at that block, and the events are lost for good (no later re-fetch). The default `block_single_query_retry_count` therefore bounds the protection to roughly 0.3–0.5 s after the block is first seen.
- **Evidence class:** E2. Mock: `on_block_update(New{bloom=B})`, push three responses whose bloom ≠ B, then two per-topic empty responses; observe `Some(EventUpdate{logs: []})`.
- **Confidence:** 80 % for the mechanism; practical impact depends on node lag. **Severity:** High for validators on such RPCs (missed `Sign`/`KeyGen*` events → exclusion or divergent state), otherwise Medium.
- **Confirm/refute:** add the unit test above; check whether the fallback should keep `ClientFiltered` (or at least apply `may_contain_log` on the header bloom before accepting an empty result).

### H3 — Fatal errors terminate with exit code 0

- **Where:** `driver.rs:170-198` (`run` returns `()`), `crates/validator/src/main.rs:96-98`, `crates/sentinel/src/main.rs:86-88`.
- **Excerpt** (`driver.rs:186-196`):
  ```rust
  let result = match input {
      Err(err) => { tracing::error!(?err, "unrecoverable watcher error; exiting"); break; }
      Ok(input) => self.update(input).await,
  };
  if let Err(err) = result { tracing::error!(?err, "unrecoverable driver error; exiting"); break; }
  ```
- **Reasoning:** `ExceededMaxReorgDepth`, `BadUpdate`, `MissingSnapshot`, SQLite and signing errors all `break` and the binaries return `Ok(())`. Orchestrators using `restart: on-failure` will not restart; alerting on exit codes sees success; conversely `restart: always` would immediately re-enter H1. The deep-reorg integration test checks only process death + a log line (`scripts/run_validator_deep_reorg_test.sh:86-94`).
- **Evidence class:** E2 (run the deep-reorg script and `echo $?` of the validator). **Confidence:** 95 %. **Severity:** Medium (operational; amplifies H1).

### H4 — Event decoding is address-agnostic, so any watched address can inject any watched event type (cross-crate)

- **Where:** `index/events.rs:404-411` (one filter with all addresses × all topics), `events.rs:458-465`, `events.rs:491-519` (decode ignores address), `watcher_events!` `events.rs:577-591`; consumers: `crates/validator/src/main.rs:56-57` (watches operator-configured `oracles`), `crates/validator/src/state/mod.rs:415-460` (only `OracleResult` receives `log.address`).
- **Excerpt** (`events.rs:405-409`):
  ```rust
  let filter = blocks.into_filter()
      .address(self.addresses.clone())
      .event_signature(self.topics.clone());
  let logs = self.provider.get_logs(&filter).await?;
  ```
- **Reasoning:** core offers no per-address topic sets and `EventLog.address` is the only hint. The validator dispatches `Coordinator::*`/`Consensus::*` events without checking the emitter, so a third-party oracle contract listed in `[validator].oracles` can emit e.g. `Sign(...)`, `SignRevealedNonces(...)`, `KeyGenComplained(...)`-shaped logs and drive the validator's state machine (burning nonces, failing groups, opening bogus signing sessions). Oracles are trusted for _results_, not for coordinator messages. The sentinel watches only protocol contracts, so it is unaffected.
- **Evidence class:** I for core (design), E2 at the validator (deploy a contract at an oracle address that emits `Coordinator.Sign` with a fresh sid; the validator will run `handle_sign`). **Confidence:** 70 %. **Severity:** Medium–High (liveness/nonce exhaustion of the affected validator; consensus safety still protected on-chain).
- **Confirm/refute:** read `handle_sign`/`handle_key_gen_complained` for any address check; consider a `Vec<(Address, Vec<B256>)>` filter in `EventWatcher` or an address check in the `Events` decode.

### H5 — Restart (and any rollback) replays events and re-queues their actions; the queue never de-duplicates

- **Where:** `blocks.rs:261-266` (synthetic uncle every restart), `state/mod.rs:182-199` (rollback then re-apply), `driver.rs:266-284`, `tx/storage.rs:89-104` (unconditional insert), `tx/storage.rs:145-156` (each row gets its own nonce).
- **Excerpt** (`tx/storage.rs:96-100`):
  ```rust
  sqlx::query("INSERT INTO transactions (request, expires_at) VALUES (?, ?)")
      .bind(request)
      .bind(expires_at.map(i64::try_from).transpose()?)
      .execute(&mut *tx)
  ```
- **Reasoning:** the first emission of an action is persisted in `transactions` and typically already submitted. After a restart the last `max_reorg_depth` (default 5) blocks are re-applied from the `safe` snapshot, which predates the submission, so identical actions are inserted again, allocated new nonces and broadcast. On-chain the second call must revert or be idempotent; either way gas is spent and an in-flight slot consumed. Spurious revalidation uncles (H9) trigger the same path.
- **Evidence class:** E2: queue an action at block N, SIGTERM before N leaves the reorg window, restart, observe two rows with different nonces for the same `request`. **Confidence:** 75 %. **Severity:** Medium (fee loss, contract-side assumptions).
- **Confirm/refute:** integration run with `max_reorg_depth = 5`, restart during keygen, count `keyGenAndCommit` txs from the validator. Mitigation idea: dedup on `request` hash among non-executed rows, or persist "actions already queued at block n" with the snapshot.

### H6 — Replacement fee bumps compound without a ceiling; the priority-fee cap does not apply to bumps

- **Where:** `tx/fees.rs:33-56` (doc: "fee bumps can cause priority fee caps to not be observed"), `tx/mod.rs:224-237, 265-283`, `tx/types.rs:64-66`.
- **Excerpt** (`fees.rs:53-56`):
  ```rust
  fn bump_fee(fresh: u128, previous: u128) -> u128 {
      let bumped = previous.saturating_add(previous.div_ceil(10));
      fresh.max(bumped)
  }
  ```
- **Reasoning:** every `blocks_before_resubmit` (2) blocks an unexecuted-but-accepted tx is rebroadcast with both `max_fee` and `max_priority_fee` ≥ 1.1× the last _accepted_ values. On Gnosis (5 s blocks) that is ×1.77 per minute, ×8·10¹⁴ per hour. The only brakes are a non-underpriced rejection (floor frozen) or the tx becoming invalid for the account balance. Plausible stuck-but-accepted conditions: an RPC that acknowledges `eth_sendRawTransaction` without propagating, a nonce gap in front of it (each later in-flight tx is "accepted" as queued and keeps bumping), or a load-balanced provider whose backends have separate mempools. Because fees are persisted in `request`, switching RPC later broadcasts the inflated fees. PR #686 addressed compounding only for _failed_ RPC calls.
- **Evidence class:** I (requires a stuck-but-accepted tx). **Confidence:** 55 %. **Severity:** Medium (bounded by account balance, but the whole gas budget can go to tips).
- **Confirm/refute:** mock: accept every submission, never advance the nonce, drive 60 block statuses, assert `max_priority_fee_per_gas` growth; decide on an absolute cap (e.g. `max_fee_per_gas` ceiling or bump the cap with `cap_priority_fee` after `bump`).

### H7 — Initial "transaction underpriced" and non-geth rejection messages are not recognised, so the tx retries forever without a bump and blocks later nonces

- **Where:** `tx/mod.rs:362-368` (regexes require both "replacement transaction" and "underpriced", or the Ankr string), `tx/mod.rs:284-294` (other errors: retry without bump), `tx/storage.rs:293-305` (`submitted_at IS NULL` rows are retried every block), `tx/storage.rs:152, 261` (expiry never applies once a nonce is allocated).
- **Excerpt** (`tx/mod.rs:362-368`):
  ```rust
  fn is_transaction_underpriced(err: &TransportError) -> bool {
      err.as_error_resp().is_some_and(|payload| {
          (iregex!("replacement transaction").is_match(&payload.message)
              && iregex!("underpriced").is_match(&payload.message))
              || iregex!("INTERNAL_ERROR: could not replace existing tx").is_match(&payload.message)
      })
  }
  ```
- **Reasoning:** geth's first-submission rejection is `"transaction underpriced"` (no "replacement"); Nethermind's replacement rejection is not `INTERNAL_ERROR: could not replace existing tx` (exact Nethermind wording not verified here). Either lands in the generic branch: the row keeps its nonce, `submitted_at` stays NULL, and every block it is re-signed with `bump(fresh, None)` = the same fresh estimate, which stays below the node's floor. All subsequently allocated nonces are queued behind it indefinitely. Trigger: RPC node txpool price floor above the 20th-percentile reward estimate (e.g. geth `--txpool.pricelimit`, Nethermind `MinGasPrice`) — plausible when the RPC provider is stricter than block producers.
- **Evidence class:** E2 for the code path (mock `push_failure_msg("transaction underpriced")` repeatedly and observe identical fees each block). **Confidence:** 55 % (mechanism 85 %, real-world trigger lower). **Severity:** Medium (silent-ish stall of all onchain actions; logged as warnings).

### H8 — Deterministic event errors are retried forever (indexing stalls with only warn logs)

- **Where:** `index/events.rs:491-519` (`DecodeLog` when a filtered log fails ABI decode or lacks `blockNumber`/`logIndex`), `events.rs:474-486` (`TooManyLogs`), `driver.rs:206-225` (unbounded retry), `events.rs:383-392` (`retries` only changes the strategy, never gives up).
- **Excerpt** (`events.rs:498-506`):
  ```rust
  E::decode_log(log.topics(), &log.data().data)
      .and_then(|data| Some(EventLog {
          block: log.block_number?,
          index: log.log_index?,
          address: log.inner.address,
          data,
      }))
      .ok_or_else(|| Error::DecodeLog { ... })
  ```
- **Reasoning:** a watched address emitting an event whose topic0 collides with a watched event but whose layout differs (the crate's own test demonstrates this with ERC-20 vs ERC-721 `Transfer`, `events.rs:750-753`), a node returning `logIndex: null`, a block with ≥ `max_logs_per_query` matching logs of one topic, or a provider that does not support `blockHash` filters, all produce the same error on every attempt. The driver retries at 100 ms forever; `safenet_core_block_number{status="processed"}` freezes but the process stays "healthy". For the validator the collision case is reachable through an operator-configured oracle contract.
- **Evidence class:** E2 (mock a log with a colliding topic0 from the watched address). **Confidence:** 70 %. **Severity:** Medium (silent stall; deterministic across all validators watching the same contract).

### H9 — Load-balanced or lagging RPC backends can produce spurious uncles via `revalidate_last_block`

- **Where:** `index/mod.rs:106-130` (−32001 on logs → revalidate), `blocks.rs:504-507` (`None` ⇒ invalidated).
- **Excerpt** (`blocks.rs:504-507`):
  ```rust
  let current = self.get_block(BlockId::number(last.number)).await?;
  if current.map(|block| block.hash) == Some(last.hash) {
      return Ok(None);
  }
  ```
- **Reasoning:** a backend that has not yet imported block n answers the by-hash logs query with "resource not found" and the by-number header query with `null`; the watcher then emits `Uncle{n}`, rolls the state back to `n−1`, and re-fetches n — usually the same block — replaying its events (H5 duplicates, effect re-execution such as `KeyGenSetup`). Also increments `safenet_core_uncled_blocks_total`.
- **Evidence class:** I. **Confidence:** 45 %. **Severity:** Low–Medium.
- **Confirm/refute:** mock sequence: `New{n}` → −32001 → `None` → `block(n)` again; assert `Uncle{n}` then `New{n}` with the same hash. Consider treating `None` as "not yet available" (retry) rather than "uncled" unless the hash differs.

### H10 — In-flight effects are not invalidated on rollback; stale resumes are applied to the rolled-back state

- **Where:** `state/mod.rs:182-189` (rollback), `state/mod.rs:246-258` (resume applied unconditionally), `effects.rs:74-89`, `driver.rs:227-230` (unbiased select).
- **Reasoning:** core carries no epoch/fork tag on effects or resumes. A resume computed from an orphaned block can arrive after the rollback and after the canonical replacement was applied. Whether that is harmful is up to the service: the validator keys `handle_nonces` by `message` and requires `CollectSigningShares` (`crates/validator/src/state/sign.rs:365-375`), which drops most stale resumes; the sentinel keys by `request_id`. New services could easily get this wrong.
- **Evidence class:** I. **Confidence:** 40 % (as a latent hazard). **Severity:** Low (design), potentially High in a future service.

### H11 — Nonce reconciliation trusts `eth_getTransactionCount` absolutely; a forked/lagging node can mark unexecuted transactions executed or open a permanent nonce gap

- **Where:** `tx/mod.rs:185-197`, `tx/storage.rs:224-235`, `tx/mod.rs:166-179` (unmarking only on observed reorgs).
- **Reasoning:** if the node answering the nonce query is on a different fork than the node feeding the block watcher (common with load-balanced providers), a nonce that only advanced on the other fork marks a tx `executed_at = latest`; since the watcher never sees a reorg, `unmark_executed` never runs, the row is pruned at `safe`, and the action is lost. Subsequent allocations start at `MAX(nonce)+1`, leaving a gap the canonical chain never fills, so every later tx is "queued" forever (and bumps, H6).
- **Evidence class:** I. **Confidence:** 35 %. **Severity:** Medium.
- **Confirm/refute:** mock the nonce jumping from 0 to 2 while only nonce 0 was allocated; assert what `mark_executed` and the next allocation do (no sanity check today).

### H12 — No RPC timeouts anywhere; a hung request stalls the driver and blocks graceful shutdown

- **Where:** `provider/mod.rs:129-137`, `driver.rs:174-197`.
- **Reasoning:** alloy's default HTTP client is used without a timeout layer (client construction not on disk; no timeout configured in core). A stalled `eth_sendRawTransaction` inside `update` prevents the `biased` shutdown branch from being reached; a stalled `eth_getBlockByNumber` inside `next_input` is at least cancellable by the signal.
- **Evidence class:** I. **Confidence:** 60 %. **Severity:** Low–Medium (silent stall until the orchestrator kills the process).

### H13 — Wall-clock dependence of block polling

- **Where:** `clock.rs:33-38, 52-57`, `blocks.rs:538-551`.
- **Reasoning:** the next poll time is `header.timestamp·1000 + block_time + propagation_delay` compared against `SystemTime::now()`. A host clock behind chain time by Δ delays every poll by Δ (indexing lags by Δ, silently); a header with a bogus far-future timestamp (buggy/malicious RPC) sleeps until then (tokio caps the deadline; no panic). Clock ahead is harmless (retries).
- **Evidence class:** I. **Confidence:** 50 %. **Severity:** Low.

### H14 — `max_reorg_depth = 0` plus a node that cannot serve logs for a just-uncled block loops forever

- **Where:** `blocks.rs:494-501` (`recent` empty ⇒ `Ok(None)`), `index/mod.rs:124-125` (error returned), `driver.rs:216-223` (retry).
- **Reasoning:** with depth 0 `recent` is always empty after eviction, so revalidation can never invalidate; the −32001 error is retried indefinitely and `blocks.next()` — which would raise `ExceededMaxReorgDepth` — is never reached. Contradicts the "fail loudly on any reorg" promise of `blocks.rs:66-68`.
- **Evidence class:** E2 (mock: depth 0, `New{n}`, then −32001 forever). **Confidence:** 60 %. **Severity:** Low (non-default config).

### H15 — `fallible_events` silently discards logs without marking the gap (Info)

- `events.rs:426-436` drops a failed per-topic query's logs; the snapshot at that block is committed as if complete. No service uses the option today. Reviewers should treat enabling it as accepting permanent event loss.

### H16 — HKDF info concatenation ambiguity (Info)

- `kdf.rs:24` `expand_multi_info(message, …)`; only fixed-size callers exist (`crates/sentinel/src/hashing.rs:51`). Safe today; document the requirement.

### H17 — Persistence has no chain-id / deployment binding (Info)

- `snapshots`/`transactions` carry no chain id or contract address; pointing an existing DB at another RPC/chain resumes silently (`provider/mod.rs:135` is never persisted). Operator-error class.

### Considered and rejected (non-findings)

- **Concurrent nonce allocation race:** single driver task; allocation is one `UPDATE … RETURNING` (`tx/storage.rs:144-161`).
- **Double execution after crash between broadcast and `record_submission`:** the row keeps its nonce and `submitted_at IS NULL`, so the retry reuses the same nonce (`tx/storage.rs:297`); only one can mine.
- **`bump` producing `max_priority > max_fee`:** both components are bumped from a consistent previous pair; fresh estimates satisfy `max_fee ≥ prio`; `max` per component preserves it (`fees.rs:39-50`).
- **Overflow/div-by-zero in `cap_priority_fee`:** guarded (`fees.rs:16-24`) and `base + prio' ≤ original max_fee` (`fees.rs:29`).
- **`expect`s at `blocks.rs:336, 457`, `events.rs:342`:** provably unreachable (§8).
- **Rollback to a missing snapshot corrupting the store:** `reorg` is transactional and errors without deleting (`state/storage.rs:124-143`, test `234-248`); the driver then exits.
- **Prune deleting a needed rollback anchor:** prune keeps `>= safe` and the newest (`state/storage.rs:152-159`) while the watcher never uncles at or below `safe` (`blocks.rs:435-439`) — consistent, given the driver prunes with the watcher's own `safe` (`driver.rs:257`).
- **Out-of-order/duplicate logs from a node corrupting state:** rejected as `BadUpdate` before any transition runs (`state/mod.rs:207-210`); fail-stop, not corruption.
- **Effect replay causing FROST nonce reuse:** the validator burns the nonce with `DELETE … RETURNING` before returning it (`crates/validator/src/secrets/store.rs:205-218`) and drops resumes whose message is not in `CollectSigningShares` (`crates/validator/src/state/sign.rs:365-375`); core's documented contract is upheld there.
- **Private key leakage via `Debug`/logs:** `Signer`'s `Debug` prints the address only (`signer.rs:96-100`); deserialization zeroizes (`signer.rs:89-92`); trace logging contains only RPC payloads.
- **`on_block_update` while not idle (`UnexpectedBlockUpdate`) losing a block:** impossible via `Watcher::next` ordering (`index/mod.rs:85-97`).
- **Warp page arithmetic overflow (`events.rs:332`):** unreachable (§8).
- **`Uncle` arriving during a warp:** the watcher cannot emit it (`blocks.rs:486-490`, `index/mod.rs:85-97`); guard `state/mod.rs:182-185` would exit otherwise.
- **`max_in_flight` bypass via resubmission:** resubmits reuse rows; the cap applies to allocation only, which is the intended semantics (`tx/mod.rs:204-219`).
- **`SnapshotStore::status()` `safe == latest` after warp pruning breaking resume:** handled by the `uncle <= indexed.latest` guard (`blocks.rs:261-266`, test `blocks.rs:681-707`).

---

## 12. Suggested review checklist (ordered by risk)

1. **Fork binding of persisted state** — `index/blocks.rs:244-368`, `state/storage.rs:50-101`: can a service ever resume from a snapshot whose block is no longer canonical without erroring? What should an operator do after `ExceededMaxReorgDepth` (wipe? which tables?) and is that documented?
2. **Exit codes and health** — `driver.rs:170-198`, `crates/*/src/main.rs`, `observability/metrics.rs`: does every fatal path yield a non-zero exit and an unhealthy `/health`?
3. **Log completeness guarantees** — `index/events.rs:362-469`: enumerate every path that commits a block's events without a completeness check (node-filtered single query, per-topic fallback, warp pages, `fallible_events`). Should `logs_bloom` (`bloom.rs:23-35`, currently dead code) gate empty results in the default mode? Is the retry-count fallback acceptable when `use_client_filtering` is on?
4. **Per-address event authority** — `index/events.rs:402-411, 491-519` and each service's `apply_transition`: which handlers verify `EventLog.address`? Can an operator-configured oracle contract emit coordinator/consensus-shaped events?
5. **Replay-induced duplicate submissions** — `blocks.rs:255-278`, `state/mod.rs:182-199`, `tx/storage.rs:89-104`: for every `Action`, is the on-chain call idempotent or does it revert cheaply? Is `expires_at` set tightly enough to bound duplicates after restarts?
6. **Fee safety** — `tx/fees.rs`, `tx/mod.rs:224-296`, `tx/types.rs:64-77`: derive the worst-case fee for a tx stuck for T hours; decide whether an absolute `max_fee_per_gas` ceiling or cap-after-bump is required; verify the underpriced regexes against Nethermind and geth messages (initial _and_ replacement) since Gnosis is Nethermind-heavy.
7. **Nonce reconciliation under inconsistent RPC views** — `tx/mod.rs:145-200, 300-317`, `tx/storage.rs:131-168, 224-235`: what happens when the nonce query and the block watcher observe different forks; can a gap or a silently dropped action occur; is a sanity check (`chain_nonce <= MAX(nonce)+1`) warranted?
8. **Permanent retry loops** — `driver.rs:206-225`, `index/events.rs:474-519`, `index/mod.rs:106-130`: classify every watcher error as transient vs. deterministic; add a bounded retry/alert for deterministic ones (`DecodeLog`, `TooManyLogs`, unsupported `blockHash` filter, depth-0 revalidation).
9. **Spurious uncles** — `blocks.rs:483-535`: should `None` on revalidation be treated as "unavailable" rather than "uncled"? Impact of load-balanced providers on `uncled_blocks_total` and on effect re-execution.
10. **Effect/resume semantics after rollback** — `state/mod.rs:246-258`, `effects.rs`: is there a need for a fork/epoch tag on resumes, or an explicit "invalidate in-flight effects" hook on `Uncle`? Audit each service's resume handler for stale-resume safety.
11. **Timeouts and shutdown** — `provider/mod.rs:129-137`, `driver.rs:174-197`: add request timeouts; confirm SIGTERM semantics under a hung RPC; confirm no partial state can be observed on kill (snapshot commit vs. action enqueue vs. effect side effects).
12. **SQLite configuration** — `utils.rs:56-62` and the config-supplied URL: verify sqlx 0.9 defaults (WAL, `synchronous=FULL`, `busy_timeout`), the `create_if_missing` behaviour vs. the sample configs, single-writer assumptions with effect tasks on the same pool, and behaviour under `SQLITE_BUSY` in `commit` (fatal).
13. **Clock assumptions** — `index/clock.rs`, `blocks.rs:538-551`: effect of host clock skew and of untrusted header timestamps on polling; whether `block_time = 0`/tiny values should be rejected.
14. **Secrets in `snapshots.state`** — `state/storage.rs`: confirm what each service serialises into the state JSON (validator: taken nonces) and the file-permission guidance.
15. **Configuration validation** — `driver.rs:30-37`, `blocks.rs:47-87`, `events.rs:73-104`, `tx/mod.rs:71-93`: reject nonsensical values (`block_time = 0`, `max_in_flight_transactions = 0`, `priority_fee_cap_percentage = NaN`).
16. **Dependency-level assumptions not verifiable offline** — alloy 2.0.5 `estimate_eip1559_fees` RPC pattern and estimator, reqwest timeout defaults, `Filter::at_block_hash` encoding, `EthRpcErrorCode::ResourceNotFound == -32001`, sqlx defaults, k256/alloy key zeroization on drop.
