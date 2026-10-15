# L-rust coverage log (run 3, blind discovery)

| Field | Value |
| --- | --- |
| Agent | L-rust (rust-expert department), Rust lens across core, validator, sentinel |
| Date | 2026-10-08 |
| Audit commit | `361aaf3` (main checkout; `git diff --stat 361aaf3 -- crates Cargo.toml Cargo.lock` empty). Builds and runs in `wt/base` at `361aaf3`, `git status --short` empty before and after |
| Finding IDs used | F3-XC-130 (range F3-XC-130 to F3-XC-159) |
| Independence | Did not open, list or grep `rust-audit/findings/` (other than writing my own file and `test -e` on my own six IDs), `report/`, `poc/`, `codebase-map.md`, `analysis/`, `state/` outside `state/run3/`, `state/run3/revalidation/`, or PR #905. Accidental exposure, recorded as the brief requires: after my finding, this log and my report were written, a final repo-wide `git status --short` (run to confirm I had changed no tracked file) printed the names of other files under `rust-audit/findings/` and `rust-audit/poc/`. I opened none of them and nothing I wrote depends on them |

## Files

All twelve assigned files were read completely with the Read tool, tests included.

| File                                     | Lines | Read  |
| ---------------------------------------- | ----: | ----- |
| `crates/core/src/driver.rs`              |   328 | 100 % |
| `crates/core/src/effects.rs`             |   267 | 100 % |
| `crates/core/src/utils.rs`               |    97 | 100 % |
| `crates/core/src/config.rs`              |   157 | 100 % |
| `crates/core/src/serialization.rs`       |   218 | 100 % |
| `crates/core/src/provider/mod.rs`        |   182 | 100 % |
| `crates/core/src/tx/storage.rs`          |   990 | 100 % |
| `crates/validator/src/secrets/store.rs`  | 1,056 | 100 % |
| `crates/validator/src/service/effect.rs` |   235 | 100 % |
| `crates/sentinel/src/effect.rs`          |   263 | 100 % |
| `crates/sentinel/src/engine.rs`          |   430 | 100 % |
| `crates/sentinel/src/verdicts.rs`        |   268 | 100 % |

Context read in part (not owned): `crates/core/src/index/mod.rs:1-145`, `index/blocks.rs:140-300, 424-610`, `index/events.rs:116-500, 650-720`, `index/clock.rs:60-103`, `state/mod.rs:1-330`, `state/storage.rs:40-170`, `tx/mod.rs:1-104`, `tx/bundle.rs:1-130`, `tx/signer.rs:1-60, 110-180`, `kdf.rs:1-30`, `metrics.rs:80-107`, `observability/mod.rs:1-64`, `observability/logging.rs` (all), `observability/metrics.rs` (all); `crates/validator/src/main.rs` (all), `frost/keygen.rs:1-60, 140-170, 420-515`, `frost/ecdh.rs:1-90, 200-245`, `frost/sign.rs:15-60`, `state/sign.rs:21-112 (grep), 233-260, 318-390`, `state/transactions.rs:1-80`, `state/mod.rs` (grep for nonces); `crates/sentinel/src/main.rs` (all), `action.rs` (all), `state.rs:15-100`, `service.rs:159-205, 265-300, 580-620, 706-760, 819-862` and grep for `salt` and log calls; `crates/validator/Dockerfile` (all); `Cargo.toml`, `crates/core/Cargo.toml` (all); `contracts/src/Consensus.sol:240-275`, `contracts/src/SentinelOracle.sol` (grep for fee transfers); `docs/` grep for health and restart guidance.

Registry sources read (versions pinned in `Cargo.lock`): sqlx-sqlite 0.9.0 `src/options/mod.rs:170-240`, `src/options/parse.rs` (grep); frost-core 3.0.0 `src/keys.rs:110-140, 631-650`, `src/keys/dkg.rs:71-75, 188-205, 226-230, 334-351`, `src/round1.rs:212-216, 265-285`; alloy-rpc-client 2.0.5 `src/builtin.rs:58-300`, `src/builder.rs:133-147`; alloy 2.0.5 `Cargo.toml:85-215`; alloy-transport-http 2.0.5 `src/reqwest_transport.rs:28-35`; reqwest 0.13.4 `src/async_impl/client.rs:172-349`; alloy-transport-ws 2.0.5 `src/native.rs` (grep); alloy-pubsub 2.0.5 `src/service.rs` (grep); tungstenite and tokio-tungstenite 0.28.0 (grep `UnableToConnect`); alloy-provider 2.0.5 `src/provider/root.rs:31-35`; alloy-transport 2.0.5 `src/boxed.rs:62-66`; hkdf 0.13.0 (grep zeroize).

## Commands

| Command | Result | Log |
| --- | --- | --- |
| `git diff --stat 361aaf3 -- crates Cargo.toml Cargo.lock`; `wc -l` on the twelve files | empty diff; counts match map section 6 |  |
| `grep` scans over `crates/{core,validator,sentinel}/src` for `select!`, spawns, `JoinSet`, `begin(`, `begin_with`, `Debug` on secret types, `Json(`, `serde_json::to_string`, `expect`/`unwrap`/`panic!`, `as` casts, `Drop`, std locks and blocking calls, `.remove(`/`mem::take` in state machines | see checklist |  |
| `cargo tree --offline -p safenet-core -e features -i alloy-rpc-client` (wt/base) | `ipc`, `ws`, `reqwest` features on | `logs/L-rust-provider-connect-url-leak.txt` |
| `source env/base.sh && cargo build -p validator -p sentinel --bins` | rc 0, rebuilt from clean wt/base at `361aaf3` |  |
| `holdtarget bash -c '... $CARGO_TARGET_DIR/debug/validator --config-file <scratch cfg>'` for `htps://` literal, `htps://` through `${LRUST_RPC_URL}`, `https://127.0.0.1:1` control, and three malformed URLs | wrong scheme prints full URL and key; control and malformed URLs do not | `logs/L-rust-provider-connect-url-leak.txt` |
| same, 14 configs with multibyte characters at the parse-error position | no panic, no secret in output, spans on char boundaries | `logs/L-rust-config-multibyte-spans.txt` |
| `cargo test -p safenet-core --lib -- effects:: config:: serialization:: tx::storage:: provider::`; `cargo test -p validator --bins -- secrets::store service::`; `cargo test -p sentinel --bins -- verdicts:: effect:: engine::` | 33, 12 and 19 passed, 0 failed | `logs/L-rust-tests.txt` |
| `git show fe9e84c:crates/core/src/provider/mod.rs`, `git show fe9e84c:crates/validator/src/main.rs`, `git show fe9e84c:Cargo.lock`, `git log fe9e84c..361aaf3 -- crates/core/src/provider/mod.rs`, `git show 55d811e` | pre-existence of F3-XC-130; #1046 scope and "Not covered" list |  |

Scratch configs live in the session scratchpad; their full text is copied into the two log files.

## Checklist answers

**Q70 (cancel safety beyond the known item).** Each watcher future commits its state synchronously after its last await, so dropping it when an effect wins the `select!` loses or duplicates no update.

- `BlockWatcher::next` (`crates/core/src/index/blocks.rs:440-527`): awaits at 449, 450 and 467. Before an await, only `self.pending.timestamp_ms += self.timings.block_time` (469) mutates state, on purpose, once retries run out; `retry_count` is local and restarts on re-entry. The queue pop (442) and all later mutations (476-519) run after the last await and return at once.
- `revalidate_last_block` (540-592): one await (561); every mutation follows it.
- `EventWatcher::next` (`index/events.rs:318-347`) works on `self.step.clone()`; `warp` (356-437) and `block` (445-490) assign `self.step` only after their awaits. A cancel throws away fetched logs, which are fetched again. The warp header refetch is the known item.
- `Watcher::next` (`index/mod.rs:85-97, 106-130`): no await between `blocks.next()` and `events.on_block_update`, or between `revalidate_last_block` and `on_block_invalidated`.
- `EffectManager::next` (`effects.rs:86-101`) wraps `JoinSet::join_next`, which is cancel safe; documented and tested (`next_is_cancel_safe` passes, `logs/L-rust-tests.txt`).
- `StateMachine::handle_update` (`state/mod.rs:171-276`) is not cancel safe (state moved out by `mem::take` at 174, awaits at 192 and 257) and has no `# Cancel Safety` doc, but it is never raced: `Driver::update` runs to completion (`driver.rs:186-194`). Latent only.

**Q71 (panics).** No panic ends the process or a task without a log line or a non-zero exit.

- Effect task panic: logged at `error` with the `JoinError` (`effects.rs:93`) and printed by the default panic hook; the resume is lost and the process continues (test `skips_panicked_tasks_without_losing_successful_resumes` passes). No reachable panic site on the effect paths: validator effects reach only `ecdh.rs:238-239` (single-output hash-to-field; a zero hash), both unreachable; the sentinel effect path has none.
- Runtime metrics task (`crates/core/src/metrics.rs:106`): detached; a panic would print only through the default hook (no `tracing` event) and freeze the runtime metrics. No custom panic hook exists (`grep panic::set_hook`: none).
- Signal handlers (`utils.rs:17-36`): `unwrap()` runs inside the future that `Driver::run` polls on the main task, so a failure panics the main task and `#[tokio::main]` exits with 101. tokio `full` includes the signal driver.
- Main task panic: exit 101 (restartable), while a handled fatal driver error exits 0 (known item; asymmetry noted as observation O9).
- No `[profile]` section in any manifest: release builds use `panic = "unwind"` and no overflow checks.

**Q72 (SQLite under concurrency).** Correct. sqlx-sqlite 0.9.0 leaves `journal_mode` unset (`sqlx-sqlite-0.9.0/src/options/mod.rs:179-183`), so the file uses the rollback journal, with `busy_timeout` 5 s (`:205`). Every deferred transaction on the shared pool starts with a write: `tx/storage.rs:116-123` (INSERT), `:324-338` (DELETE), `secrets/store.rs:178-190` (UPDATE ... RETURNING), `:227-239` (INSERT ... ON CONFLICT), `:329-340` (INSERT ... ON CONFLICT ... RETURNING), `:364-369` (DELETE), `crates/core/src/state/storage.rs:129-133` (DELETE). The two read-first transactions use `BEGIN IMMEDIATE` (`tx/storage.rs:165`, `secrets/store.rs:273`). The verdict store uses single autocommit writes (`verdicts.rs:79-131`). So no transaction upgrades a read lock to a write lock, the only case where SQLite returns `SQLITE_BUSY` without calling the busy handler. `SQLITE_BUSY` then needs a lock held for more than 5 s, and every in-service holder is short. When it happens in the queue or the snapshot store it is fatal: only RPC errors are intermittent (`tx/mod.rs:62-84`), state errors propagate (`driver.rs:250, 261, 263, 284`), and the process exits 0 (known). In an effect it resumes `Noop` (validator, `service/effect.rs:174-180`) or `Unknown` (sentinel, `effect.rs:85-117`).

**Q73 (secret copies, error types).** Copies left in memory (Informational under A1): the config `String` (`config.rs:67`) and toml's parse buffers, freed without zeroizing; the RPC `Url` with its key for the process lifetime; snapshot JSON (`state/storage.rs:106`) holding `KeyShare` and DKG secrets, and secret-store JSON (`secrets/store.rs:198, 236`), freed without zeroizing; `EncryptionKey::generate` leaves the 32-byte entropy, the key's preimage, on the stack (`frost/ecdh.rs:34-36`); the HKDF PRK in `kdf::derive_key` (`kdf.rs:22`) is not zeroized (hkdf 0.13.0 has no zeroize support). A taken nonce stays in free SQLite pages (no `secure_delete`), but the same file already holds the `KeyShare` in plain text (A1), so this adds nothing. Error types: `Provider::connect` with an unsupported scheme quotes the full URL: **F3-XC-130**. Checked clean: toml errors (`config.rs:32-49`), `url::ParseError` (executed), sqlx connect-option errors (quote only query parameter names and values, `sqlx-sqlite-0.9.0/src/options/parse.rs:63-108`), `Signer::from_str` (`tx/signer.rs:152-163`), `${NAME}` substitution errors (`serialization.rs:99-109`), runtime reqwest errors (`provider/mod.rs:124-134`, executed control), WebSocket errors, store errors.

**Q74 (growth from chain input).** No unbounded growth found; growth is linear in what an attacker pays. The `JoinSet` holds one task per effect and is drained by `next`; there is no concurrency cap (observation O2). Sentinel entries are small (`state.rs:22-96`) and every pre-commit state has a deadline. Validator `signing` entries are created only for allow-listed oracles in a participating epoch (`state/transactions.rs:22-41`); each clones the full `SafeTransaction`, `data` included, and the `KeyShare` into snapshotted state (`:52-70`) and is cleared by timeouts. Each such proposal costs the proposer the SentinelOracle fee (0.40 USDC.e, `contracts/src/SentinelOracle.sol:238`) plus gas, so snapshot size per log range grows with attacker-paid calldata.

**Q75 (`mem::take` across awaits).** The awaits after the take (`state/mod.rs:174, 280`) are `snapshots.reorg` (192) and `snapshots.commit` (257); the `BadUpdate` returns (222, 272) also leave the machine poisoned. The driver already exits on any state error (`driver.rs:195-198, 261`), so poisoning has no extra effect today. A transient `SQLITE_BUSY` in `commit` is fatal, matching the policy stated at `tx/mod.rs:65-71`.

**Q10 (Rust side).** Panics are unreachable in both effect handlers. Failures resume `Noop` (validator) or `Unknown` (sentinel), with no retry. The sentinel's `WaitingForEngineCheck` has a deadline that `handle_block_advance` enforces (`state.rs:29-32`, `service.rs:403`). Validator states waiting on a resume rely on the key-gen and signing timeouts (consequences are R4 and R5 territory; observation O10).

**Q15 (SIGTERM aborts).** `run` breaks only between inputs (`driver.rs:177-199`); dropping the `Driver` drops the `JoinSet`, which aborts running effects (`effects.rs:37-38`). An effect aborted after its database commit costs liveness only: a taken nonce pair is burned and the replay gets `None` and resumes `Noop`, so there is no share and no reuse (INV-V10 by design); stored DKG secrets and recorded verdicts are reused on replay.

**Q57 (Debug and Serialize).** Every secret-bearing type redacts in `Debug`: frost-core `SigningShare` (`keys.rs:126-133`), round-1 and round-2 `SecretPackage` (`keys/dkg.rs:191-204, 337-350`), round-2 `Package` and `KeyPackage` through their `SigningShare` field, `round1::SigningNonces` (`round1.rs:271-281`), Safenet `EncryptionKey` (`frost/ecdh.rs:71-75`), `SigningNonces` (`frost/sign.rs:49-53`) and `Signer` (`tx/signer.rs:174-178`). So the `Effect` and `Resume` logs at `effects.rs:62, 66, 90`, `driver.rs:267` and `service/effect.rs:177` (warn level) print no secret. `SentinelActionKind::Reveal { salt }` derives `Debug`, but no action is logged and the salt is derived at use (`service.rs:221, 435`). No secret is serialized into a log (`Json(` is used only for JSON-RPC packets, `provider/mod.rs:51-52, 87`).

**Scans.** `tokio::select!`: `driver.rs:177, 229`, `utils.rs:32` (non-test). Spawns: `effects.rs:64` (JoinSet), `metrics.rs:106` (detached); `clock.rs:74`, `engine.rs:229`, `effect.rs:188` are test-only. `Drop`: only `frost/ecdh.rs:77-81` (zeroize). No `unsafe`, no std `Mutex`/`RwLock`, no blocking calls in async code. `expect`/`unwrap` census matches map section 4.3. `as` casts: 31 in non-test code; none on my paths truncates untrusted input (the `len() as u16` comparisons in `state/keygen.rs` count contract-gated members).

## Rejected hypotheses

| # | Hypothesis | Refutation |
| --- | --- | --- |
| R-1 | Malformed RPC URLs are quoted in the startup error, as #1046's "Not covered" list says | `url` crate messages are fixed; executed with a bad port, no scheme, bad host (`logs/L-rust-provider-connect-url-leak.txt`) |
| R-2 | `ParseError::new` slices `&contents[..span.start]` (`config.rs:39`) off a char boundary, panics, and the panic message quotes up to 256 bytes of the config | 14 multibyte probes: every span on a char boundary, no panic (`logs/L-rust-config-multibyte-spans.txt`); the column counts `chars()` (`config.rs:41-44`) |
| R-3 | WebSocket transport logs the URL on reconnect | alloy-transport-ws logs `%err` (`native.rs:232, 256, 274`); `UnableToConnect(uri)` exists only in tungstenite's sync client (`client.rs:137`), not used by tokio-tungstenite |
| R-4 | `Provider`'s derived `Debug` prints the URL | `BoxTransport` `Debug` is opaque (`alloy-transport-2.0.5/src/boxed.rs:62-66`) |
| R-5 | A dead RPC connection hangs the driver forever | reqwest 0.13.4 defaults: TCP keepalive 15 s, 3 probes, `tcp_user_timeout` 30 s (`client.rs:303-307`); only a live peer that never answers hangs (outside A4; observation O8) |
| R-6 | A deferred read-then-write transaction gets an immediate `SQLITE_BUSY` | none exists (Q72 list) |
| R-7 | In Batched mode an oversized first transaction blocks the queue forever | the first transaction is always taken (`tx/bundle.rs:90-93, 129`) |
| R-8 | A cancelled watcher future loses or duplicates an update | Q70 |
| R-9 | `Effect`/`Resume` logs leak secrets | Q57 |
| R-10 | Concurrent `KeyGenSetup` effects store two different secrets for one group | first statement is a write (`secrets/store.rs:182-190`), so the second waits and reads the stored row |
| R-11 | Concurrent `take_signing_nonces` returns the same pair twice | `BEGIN IMMEDIATE` (`store.rs:273`); a deferred variant would also fail closed |
| R-12 | Reordered reconcile and setup effects delete freshly stored DKG secrets | each block's reconcile re-applies the retained set and deletion waits for the safe block to pass the schedule (`store.rs:24-41, 312-352`) |
| R-13 | Remove-then-match arms drop entries on unexpected events | sentinel `service.rs:165-179, 269-298` re-insert; `586-605, 715-747, 824-861` drop deliberately with a stated reason; validator `state/sign.rs:29-112` (re-insert at 100), `243-331` (re-insert at 327) |
| R-14 | SIGTERM before signal registration loses state | nothing of consequence is in memory before `run`; SQLite commits are atomic |
| R-15 | `utils::Json` returns `fmt::Error` and tracing's `format!` panics | serde_json writes UTF-8 slices split at ASCII escapes; packets hold valid JSON |
| R-16 | A cast truncates chain data on the lens paths | cast scan above |
| R-17 | The effect `JoinSet` grows without bound | drained continuously; bounded by paid proposals (Q74) |

## Observations (plausible, unproven or hardening)

- O1. `next_input` retries every watcher error except `ExceededMaxReorgDepth` every 100 ms, forever (`driver.rs:209-227`), including permanent ones (`DecodeLog`, `UnexpectedBlockUpdate`, `UnexpectedBlockInvalidation`, `index/events.rs:122-145`). A permanent error would stall the service without exiting, logging a warning every 100 ms, and `/health` is static (`observability/metrics.rs:9-11`), so a liveness probe would not catch it. No trigger found here: the one route I checked, an out-of-range `DisputeResolved.outcome`, cannot happen because the binding enum matches the contract's six variants (`crates/sentinel/src/bindings.rs:14-22`, `contracts/src/libraries/SentinelOracleRequests.sol:15-22`). See R1's Q3 and Q6.
- O2. No cap on concurrent effects: the sentinel opens one engine request per tracked proposal at once (`effects.rs:61-69`, `crates/sentinel/src/effect.rs:98-105`). A burst of fee-paying proposals (fees refunded on timeout, #549) could push honest proposals' checks past the 18.75 s timeout if the engine saturates. Engine capacity is out of scope (A3).
- O3. `/health` returns `OK` from the exporter task regardless of driver progress.
- O4. Secret copies not zeroized (Q73 list), A1.
- O5. `ObservabilityTransport::call` calls a clone of the inner service without `poll_ready` (`provider/mod.rs:62-64, 84-90`); harmless while alloy's transports are always ready.
- O6. The validator and sentinel `Config` types derive `Debug` with `rpc: Url` (`crates/validator/src/config.rs:8-19`, `crates/sentinel/src/config.rs:8-19`). Nothing logs them today; a future `?config` would leak the key.
- O7. The images run the binary as PID 1 with no init (`crates/validator/Dockerfile` `ENTRYPOINT`), which ignores SIGTERM until tokio installs its handler on the first poll in `Driver::run`; a stop during startup waits for the grace period and ends with SIGKILL. Harmless.
- O8. RPC calls inside `Driver::update` have no request timeout (`driver.rs:249, 283`); a peer that accepts a request and never answers stalls the driver and its SIGTERM handling while `/health` stays `OK`. Outside A4.
- O9. Exit status asymmetry: a driver panic exits 101 (restartable), a handled fatal error exits 0 (adds to the known item).
- O10. Failed validator effects are not retried (`service/effect.rs:174-180`); a transient store error in `KeyGenSetup` costs that key generation (liveness). For R4 and R5.
- O11. No concurrent-call tests for the secret store's single-use paths or the verdict store's first-wins path; correctness rests on statement order and SQLite locking.
- O12. The reconcile `UPDATE` rewrites every row of both secret tables on each `NewBlock` (`secrets/store.rs:389-419`, no `WHERE`), so the known per-warped-block performance item also applies to live blocks and grows with nonce rows per epoch.
- O13. The engine response body is read without a size cap (`engine.rs:178`); the engine is trusted (A3).
