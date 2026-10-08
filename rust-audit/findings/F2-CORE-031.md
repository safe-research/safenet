# F2-CORE-031 Fatal driver errors terminate the process with exit status 0

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | safenet-core, driver.rs |
| Location | crates/core/src/driver.rs:172-200 (related: crates/validator/src/main.rs:95-98; crates/sentinel/src/main.rs:85-88; scripts/run_validator_deep_reorg_test.sh:86-94) |
| Severity | Medium / Low |
| Certainty | 85% (Critic C2-CORE-B; reviewer self-estimate in Trail) |
| Assumptions involved | A1 |
| Tags | reorg, crash-consistency, config |

Audited commit: `3ec8bc5`.

## Claim

`Driver::run` has the signature `pub async fn run(mut self)` and returns `()` on every exit path: the shutdown signal, an unrecoverable watcher error (`ExceededMaxReorgDepth`) and any error from `update` (`state::Error::{BadUpdate, EndOfChain, Poisoned}`, `storage::Error::{Database, Serialization, MissingSnapshot, BlockNumberOverflow}`, non-intermittent `tx::Error` such as SQLite or signing failures). The error is only logged. Both binaries then fall through to `Ok(())`, so the process exits with status 0 after an unrecoverable failure. Orchestrators with `restart: on-failure` semantics will not restart the service, alerting keyed on exit status sees a clean exit, and `podman`/`systemd` unit status reads "success". The in-repo integration test for the deep-reorg exit verifies process death and a log line only, so the exit status is untested. The API also makes it impossible for a caller to distinguish an operator-requested shutdown from a fatal error without parsing logs.

Under Kubernetes' default `restartPolicy: Always` the impact is limited to observability (the pod restarts anyway and, for `ExceededMaxReorgDepth`, immediately re-enters the unverified-anchor resume that R1 is assessing); under `OnFailure`/`systemd Restart=on-failure` an honest validator or sentinel stays down until a human notices.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | `run` returns `()`; watcher and driver errors only `break` after logging. | E2 | crates/core/src/driver.rs:172, 186-198 | `pub async fn run(mut self) {` ... `Err(err) => { tracing::error!(?err, "unrecoverable watcher error; exiting"); break; }` `Ok(input) => self.update(input).await,` `};` `if let Err(err) = result { tracing::error!(?err, "unrecoverable driver error; exiting"); break; }` |
| 2 | The validator binary returns `Ok(())` after `run`. | E2 | crates/validator/src/main.rs:95-98 | `tracing::info!("starting validator service");` `driver.run().await;` `Ok(())` |
| 3 | The sentinel binary returns `Ok(())` after `run`. | E2 | crates/sentinel/src/main.rs:85-88 | `tracing::info!("starting sentinel service");` `driver.run().await;` `Ok(())` |
| 4 | Only `ExceededMaxReorgDepth` reaches `run` from the watcher; every other watcher error is retried, so this path is specifically the deep-reorg exit. | E2 | crates/core/src/driver.rs:211-224 | `Err(err @ index::Error::Blocks(index::blocks::Error::ExceededMaxReorgDepth(_))) => { return Err(err); }` `Err(err) => { tracing::warn!(?err, "failed to get next blockchain update; retrying after delay"); tokio::time::sleep(STEP_RETRY_DELAY).await; }` |
| 5 | The integration test asserts death and a log line, not the exit status. | E2 | scripts/run_validator_deep_reorg_test.sh:86-94 | `if kill -0 "$VALIDATOR_PID" 2>/dev/null; then` ... `if ! grep -q "ExceededMaxReorgDepth" "$REPO_ROOT/validator_logs.txt"; then` `EXIT_MESSAGE="FAILURE: the validator exited, but its logs do not mention the expected max-reorg-depth error."` |
| 6 | Storage errors are classified as non-intermittent and therefore fatal through `?` in `update`. | E2 | crates/core/src/tx/mod.rs:47-54; crates/core/src/driver.rs:250, 284 | `fn is_intermittent(&self) -> bool {` ... `matches!(self, Self::Rpc(_))` ... driver: `if let Err(err) = tx::lift_intermittent_error(result)? {` |

## Trigger

Run either binary with `max_reorg_depth = N` and produce a reorg deeper than `N` (the deep-reorg script does this on Anvil), or corrupt/lock the SQLite file so `SnapshotStore::commit` fails, or feed the state machine an out-of-order update (`BadUpdate`). Observe `echo $?` = 0 after the "unrecoverable ... error; exiting" log line. Not run this session (no Anvil in this phase).

## Considered and rejected

- "A panic gives a non-zero exit anyway" - true for panics inside `apply_transition`, but none of the enumerated fatal paths panic; they `break`.
- "The metrics endpoint signals the failure" - the process exits, so `/health` simply disappears; nothing distinguishes it from a clean stop (see F2-CORE-033 for the stall case).
- "Operators use logs" - `docs/devnet.md:150` indeed points at logs for liveness; that does not help restart policies or exit-status-based alerting.

## Remediation options

1. Change `Driver::run` to return `Result<(), Error>` (or an enum `Exit::{Shutdown, Fatal(Error)}`), and make both `main`s propagate it (`driver.run().await?;`), yielding a non-zero status for every fatal path. Tradeoff: a small API change; the integration scripts should additionally assert the status.
2. Keep the signature and call `std::process::exit(1)` after logging a fatal error. Simpler, but skips destructors (the `EffectManager` abort, SQLite pool drop) and loses the ability to unit-test the outcome.

Tests to add: extend `scripts/run_validator_deep_reorg_test.sh` with `wait "$VALIDATOR_PID"; [ $? -ne 0 ]`; a `driver.rs` unit test (there are none today) that drives `run` with a mocked watcher error and asserts the returned error once option 1 lands.

## Trail

- Reviewer R2: drafted, self-estimate 90% (E2, all paths cited; runtime check deferred to QA). Severity Medium: operational, but it silences every fatal condition in the runtime, including the deep-reorg exit that the rest of the design relies on being loud.
- Critic C2-CORE-B: Confirmed, 85%, severity Low (reviewer Medium).

## Critic (C2-CORE-B)

Method: read title and Location only, traced `Driver::run` (`crates/core/src/driver.rs:172-200`) and both `main` tails myself, then compared with the reviewer's text. Every citation re-opened at `3ec8bc5`.

| # | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | `pub async fn run(mut self)` returns `()`; both error arms only `break` (driver.rs:172, 188-198). |
| 2 | Supported | `driver.run().await;` then `Ok(())` (validator/main.rs:96-98); `main` already returns `Result<(), Box<dyn Error>>` (line 33). |
| 3 | Supported | Same shape (sentinel/main.rs:30, 86-88). |
| 4 | Supported | driver.rs:213-217 returns only `ExceededMaxReorgDepth`; every other watcher error is retried after 100 ms (218-224). |
| 5 | Supported | The script checks `kill -0` and `grep -q "ExceededMaxReorgDepth"` only (scripts/run_validator_deep_reorg_test.sh:86-94). |
| 6 | Supported | `is_intermittent` matches `Rpc` only (tx/mod.rs:47-54); `lift_intermittent_error(..)?` at driver.rs:250 and 284. |

Finding verdict: **Confirmed** (mechanism and trigger verified by trace; not executed, since Anvil is out of bounds for Critics). Certainty **85%** (E2 band).

Severity: reviewer Medium, mine **Low**. The defect is an error-handling and operations weakness: it needs a condition that is already fatal (the A5 deep-reorg exit, a SQLite or signing failure, `BadUpdate`) together with an exit-status-keyed restart or alerting policy; the `error!` line is loud; under `Restart=always` / `restartPolicy: Always` the impact is nil; nothing here is attacker-controlled. The reviewer's point that the A5 exit is designed to be loud is fair, and the Manager may weigh it toward Medium.

Overlap: `F2-VAL-066` (R6, owned by C2-VAL-B) restates the same exit-status defect from `validator/main.rs:95-98` and bundles the `/health` half (which is F2-CORE-033). One defect: the root is `Driver::run`'s `()` return in core, which both binaries inherit, so **F2-CORE-031 is the canonical finding for the exit status**; I have not edited F2-VAL-066.

Remediation check: option 1 is a two-line change per binary (`driver.run().await?;`) because both `main`s already return `Result`; option 2 skips the `EffectManager` abort and pool drop as the reviewer says.

## Reconciliation (run 2)

**Final: CONFIRMS `F-CORE-030` (canonical) — combined Medium, 85 (E2).** Same exit-status defect; this file adds the storage-error path and the missing status assertion in the deep-reorg script. Run 1 rated Medium and this file Low; Medium is carried (this file's Critic offered it; #820 asked for an observable failure) (`state/run2/reconciliation/core.md` §1). Cross-crate: canonical over `F-VAL-064` / `F2-VAL-066` for the exit status.
