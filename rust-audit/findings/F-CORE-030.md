# F-CORE-030 `Driver::run` discards its outcome, so every unrecoverable error exits the process with status 0 and the only other failure channel (`/health`) is liveness-only

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | core, `driver.rs` (with `observability/metrics.rs` as the other operator-facing signal) |
| Location | `crates/core/src/driver.rs:170-198` (related: `crates/core/src/observability/metrics.rs:6-16`; `crates/validator/src/main.rs:95-98`; `crates/sentinel/src/main.rs:85-88`) |
| Severity | Medium / Medium |
| Certainty | 85% |
| Assumptions involved | A1, A5 |
| Tags | crash-consistency, config, dos |

## Claim

`Driver::run` has return type ``. Every terminal condition — the shutdown signal, the deliberate `ExceededMaxReorgDepth` exit (assumption A5), a `state::Error` (`BadUpdate`, `MissingSnapshot`, `EndOfChain`, `Poisoned`, any SQLite or serde failure behind `storage::Error`), and any non-RPC `tx::Error` (storage or signing) — leaves the loop through the same `break` and returns the same ``. The caller cannot distinguish "operator asked me to stop" from "I cannot continue". Both binaries therefore `return Ok()` and the process exits with status **0** after a fatal error.

This matters because the exit status is in practice the _only_ failure channel these services have:

- `/health` is served by the Prometheus exporter and answers a constant `OK` for as long as the listener thread is alive; it has no view of the driver at all, and nothing in the repository consumes it (no `HEALTHCHECK`, no probe in any script or manifest — `grep -rn health` over Dockerfiles, `scripts/`, `*.yml`, `*.toml` returns nothing).
- The one metric that would move, `safenet_core_block_number{status="processed"}`, simply stops changing; there is no "driver exited" counter or state gauge.

The operational consequences are asymmetric and both bad. Under `Restart=on-failure` (systemd), `restart: on-failure` (Docker/Compose) or `restartPolicy: OnFailure` (Kubernetes) — the policy an operator picks precisely so that a _deliberate_ fail-stop is not restarted into a loop — the service stays down after a transient `BadUpdate` or a SQLite error and is never restarted. Under `Restart=always` the process is restarted, but the container terminates as `Completed` rather than `Error`, so exit-code-based alerting and `kube_pod_container_status_last_terminated_reason` report success for what is a hard failure, and the restart itself walks straight into the unverified-anchor resume documented in F-CORE-001.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | `run` returns ``; the shutdown signal, an unrecoverable watcher error and an unrecoverable driver error all leave through `break` and are indistinguishable to the caller. | E2 | `crates/core/src/driver.rs:170-197` | <pre>pub async fn run(mut self) {<br> let shutdown = utils::shutdown_signal;<br> tokio::pin!(shutdown);<br><br> loop {<br> let input = tokio::select! {<br> biased;<br> _ = shutdown.as_mut => {<br> tracing::info!("received shutdown signal; stopping service");<br> break;<br> },<br> input = self.next_input => input,<br> };</pre> |
| 2 | Both error paths only log and `break`. | E2 | `crates/core/src/driver.rs:186-196` | <pre>let result = match input {<br> Err(err) => {<br> tracing::error!(?err, "unrecoverable watcher error; exiting");<br> break;<br> }<br> Ok(input) => self.update(input).await,<br>};<br>if let Err(err) = result {<br> tracing::error!(?err, "unrecoverable driver error; exiting");<br> break;<br>}</pre> |
| 3 | Everything reachable from `update` is fatal-by-`break`: `state::Error` from `handle_update`/`prune`, and non-intermittent `tx::Error` lifted by `lift_intermittent_error`. | E2 | `crates/core/src/driver.rs:247-257` | <pre>let result = self.transactions.update_block_status(block_status).await;<br>if let Err(err) = tx::lift_intermittent_error(result)? {<br> tracing::warn!(<br> ?err,<br> "transaction queue failed to handle new block; will continue"<br> );<br>}<br><br>let commands = self.state.handle_update(update).await?;<br>recorder.processed;<br>self.state.prune(block_status.safe).await?;</pre> |
| 4 | The validator returns `Ok()` straight after `run`, so the process exit status is 0. | E2 | `crates/validator/src/main.rs:95-98` | <pre>tracing::info!("starting validator service");<br>driver.run.await;<br><br>Ok()</pre> |
| 5 | The sentinel does the same. | E2 | `crates/sentinel/src/main.rs:85-88` | <pre>tracing::info!("starting sentinel service");<br>driver.run.await;<br><br>Ok()</pre> |
| 6 | `/health` is a constant `OK` served by the metrics listener, with no coupling to the driver; the crate's own test asserts exactly that. | E2 | `crates/core/src/observability/metrics.rs:6-12` and `76-78` | <pre>/// Installs the global Prometheus recorder and starts an HTTP listener that<br>/// scrapers can poll for metrics.<br>///<br>/// The listener serves Prometheus-formatted metrics on every path except<br>/// `/health`, which returns a plain `OK` for liveness probes.</pre><br><pre>let (status, health) = http_get(addr, "/health").await;<br>assert_eq!(status, StatusCode::OK);<br>assert_eq!(health, "OK");</pre> |
| 7 | The deep-reorg regression test asserts process death and a log line, never the exit status, so the current behaviour is not pinned by any test. | E2 (reference material, context only) | `scripts/run_validator_deep_reorg_test.sh:86-94` | <pre>if ! grep -q "ExceededMaxReorgDepth" "$REPO_ROOT/validator_logs.txt"; then</pre> |

## Trigger

Any of these, all of which are reachable without a malicious RPC (assumption A4):

1. A reorg deeper than `max_reorg_depth` (default 5) while running: `Watcher::next` yields `index::Error::Blocks(ExceededMaxReorgDepth)`, `next_input` returns it un-retried (`driver.rs:211-215`), `run` logs and breaks, `main` returns `Ok()`, `echo $?` prints `0`. This is the _intended_ fail-stop of assumption A5 — the exit is correct, the status is not.
2. A node that returns a block's logs out of order or duplicated: `state::Error::BadUpdate` (`state/mod.rs:207-211`), same path, status 0.
3. `SQLITE_BUSY` or a full disk during `snapshots.commit`: `storage::Error::Database` → `state::Error` → status 0.
4. A signing failure or a SQLite failure in the transaction queue (`tx::Error::Signing`, `tx::Error::Storage` are explicitly _not_ intermittent, `tx/mod.rs:47-54`) → status 0.

With `Restart=on-failure` the service never comes back from cases 2-4; with `Restart=always` the alert that should say "validator crashed" says "validator completed".

## Considered and rejected

- **"`/health` will catch it."** It cannot: the endpoint is installed by `PrometheusBuilder::install` and answers `OK` from the listener alone (`observability/metrics.rs:16-47`); no driver state reaches it. It is also unused — no `HEALTHCHECK`, probe, or manifest in the repository references it.
- **"The process is gone, so any supervisor restarts it."** Only for `always`-style policies. `on-failure` is the policy an operator would reasonably choose given assumption A5's deliberate exit, and it keys off the exit status this finding is about.
- **"A panic would give a non-zero status anyway."** True for a panic in `apply_transition`, which unwinds the driver task, but not for any of the four triggers above — they are all `Err` returns, not panics. Note the asymmetry: a _panic_ is reported correctly, a _handled fatal error_ is not.
- **"The log line is enough."** `tracing::error!` output does go to stdout as JSON (`observability/logging.rs:14-20`), so a log-based alert can catch it. This finding is that the two cheaper, more reliable signals (exit status, health endpoint) both actively report success.
- **Not a duplicate of F-CORE-001**: that finding is about _what state_ the process resumes from; this one is about the process telling its supervisor that it succeeded. They compound (a status-0 exit under `restart: always` is the F-CORE-001 entry path) but the fixes are independent.

## Remediation options

1. Change the signature to `pub async fn run(mut self) -> Result<, Error>`: return `Ok()` only for the shutdown branch and propagate the error otherwise. Both `main`s already return `Result<_, Box<dyn Error>>` and would need only `driver.run.await?`. Tradeoff: a public API break for a crate that is `publish = false` and has exactly two callers in-tree; the deep-reorg integration script should then also assert a non-zero status.
2. Keep the signature and have `run` return an enum (`Stopped::Shutdown` / `Stopped::Fatal(Error)`) so the caller chooses the status. Same effect, no `?`-ergonomics.
3. Independently of 1/2, give the health endpoint something to report: a `Driver`-owned `AtomicBool`/watch channel that `observability::metrics::serve` consults, or at minimum a `safenet_core_driver_running` gauge set to 0 before `run` returns and a `safenet_core_driver_exits_total{reason}` counter. This also covers the stall case (F-CORE-034/F-CORE-035) where the process never exits at all.

Tests to add: a `driver.rs` unit test (the file currently has none — 0.0% line coverage per codebase-map §2) driving a mocked watcher that yields `ExceededMaxReorgDepth` and asserting the returned error; an assertion of `$?` in `scripts/run_validator_deep_reorg_test.sh`. No code is committed.

## Trail

- Reviewer R2: drafted from lead CORE-H3 / core checklist item 2, self-estimate 85%. Mechanism re-read in full at `driver.rs:170-198` and both `main.rs` tail sections; the "`/health` has no consumers" claim comes from a repo-wide grep run this session, not from the analysis file.

## Critic (C-CORE-B)

I derived the mechanism from `driver.rs:170-198` and both `main.rs` tails before reading the Claim, and reached the same conclusion: `run` returns ``, all four terminal conditions leave through the same `break`, and both binaries then `return Ok()` from an `async fn main -> Result<, Box<dyn Error>>` (`crates/validator/src/main.rs:33`, `crates/sentinel/src/main.rs:30`), so the process exit status is 0 after a fatal error.

### Per-claim verdicts

| # | Verdict | Note |
| --- | --- | --- |
| 1 | **Supported** | `driver.rs:170-182` verbatim. |
| 2 | **Supported** | `driver.rs:186-196` verbatim. |
| 3 | **Supported** | `driver.rs:247-257` verbatim; `handle_update`/`prune` use `?` and `lift_intermittent_error` re-raises the non-`Rpc` variants (`tx/mod.rs:47-54, 58-66`). |
| 4 | **Supported** | `crates/validator/src/main.rs:95-98`. |
| 5 | **Supported** | `crates/sentinel/src/main.rs:85-88`. |
| 6 | **Supported** | `observability/metrics.rs:9-11` and the test at `:76-78`. Independently confirmed that `/health` is served by `metrics_exporter_prometheus`'s own listener and receives no driver state: `serve` (`observability/metrics.rs:16-47`) takes only a `SocketAddr`. |
| 7 | **Supported** | `scripts/run_validator_deep_reorg_test.sh:86-94` re-read: it asserts the process is gone (`kill -0`) and that the log mentions `ExceededMaxReorgDepth`. It never inspects `$?`. Reference material, correctly labelled. |

I re-ran the "no consumer" grep myself: no `HEALTHCHECK` in any of the three Dockerfiles, and `grep -rn health` over `crates/`, `docs/` and `*.toml` returns only the four `observability` hits and the sample-config comment about `metrics_address`. The claim holds. I add one aggravating fact the reviewer did not use: `metrics_address` defaults to `127.0.0.1:0` (`observability/mod.rs:32-34`), an _ephemeral loopback_ port, so even a probe that wanted to use `/health` has no fixed port to reach.

### Finding verdict

**Confirmed — 85%.** `E2` throughout; mechanism and trigger both verified. Held at 85 rather than higher only because `E1` is unreachable in this read-only run (`state/baseline.md` §2).

**Severity: Medium (unchanged).** Not High: nothing here causes the stall, it only misreports one that has already happened, and under `Restart=always` the process does come back. Not Low: the exit status is the only machine-readable failure channel these binaries have, and `restartPolicy: OnFailure` is a defensible choice precisely because of A5's deliberate exit.

### Duplication

**This is the canonical file for the exit-status defect.** R6's `F-VAL-064` covers the same behaviour (its Claim bullet 1 and basis rows 4-5 cite the same `driver.rs:186-197` and `crates/validator/src/main.rs:95-99`) and says so itself: _"Merge the exit-code claim with R2's if both survive."_ The fix is in `crates/core/src/driver.rs`, which is R2's file, so F-CORE-030 is canonical for the mechanism and F-VAL-064 should retain only its deployment-artefact claims — no `USER` in the Dockerfile, `/health` unreachable under the sample config, no `HEALTHCHECK`, floating base-image tags, no `.dockerignore`. Neither file should be deleted.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 1, and it is a prerequisite for four other findings.**

Option 1 (`run(mut self) -> Result<, Error>`) is sound and the API-break objection is thin: the crate is `publish = false` with exactly two in-tree callers, both of which already return `Result<_, Box<dyn Error>>`. Option 2 (a `Stopped` enum) is equivalent and neither better nor worse.

What the finding understates is how much depends on this. A non-zero exit is the precondition for: **F-CORE-004 option 1** (propagate deterministic errors as fatal — pointless if the process then exits 0), **F-SEN-013 option 3** (same), **F-CORE-001 option 3** (a dirty-exit marker needs an exit signal to write from), and **F-CORE-035 option 2** (escalate after N repeated failures). It should be sequenced first among the whole fail-loud group, and the report should say so.

Option 3 (a `driver_running` gauge / `driver_exits_total{reason}` counter, and something for `/health` to report) is sound and covers the case option 1 cannot: the process that never exits at all. This is the same consolidated health/liveness signal F-CORE-004, -007, -011 and -035 each ask for separately.

The finding's own note that the deep-reorg integration script should assert a non-zero status is correct and cheap: `scripts/run_validator_deep_reorg_test.sh` currently cannot distinguish the intended fatal exit from a clean one.
