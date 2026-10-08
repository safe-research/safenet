# F-CORE-032 A failed effect task is logged and skipped, so a panicking effect silently removes a resume the state machine is waiting for — the opposite of the fail-stop policy applied everywhere else

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | core, `effects.rs` |
| Location | `crates/core/src/effects.rs:64-89` (related: `crates/core/src/effects.rs:209-219`; `crates/core/src/driver.rs:186-196`; `crates/core/src/state/mod.rs:75-94`) |
| Severity | Low / Low |
| Certainty | 45% |
| Assumptions involved | A1 |
| Tags | crash-consistency, dos |

## Claim

`EffectManager::next` reaps `JoinSet::join_next` and, on `Some(Err(_))` — a panicked (or aborted) effect task — logs at `error` and loops to the next task. The resume is gone: the state machine is never told, the effect is never retried, and the driver keeps running as if nothing happened. A dedicated test pins this as intended behaviour.

That is inconsistent with the runtime's policy everywhere else. A panic inside `apply_transition` unwinds the driver task and kills the process (transitions are declared infallible, `state/mod.rs:76-78`); every `state::Error` and every non-intermittent `tx::Error` breaks the run loop (`driver.rs:186-196`). Only effects — the one place that runs arbitrary service code with I/O, locks and a shared SQLite pool — fail open. Because effect handlers are also declared infallible (`effects.rs:16-19`), a handler author has no in-band way to report a failure except by panicking, which is exactly the case that is discarded.

The result is the same terminal state as F-CORE-031 reached by a different route: a session parked in a `Waiting…` state until the service's own timeout reaps it, with no metric and a single log line. There is no counter for dropped effect tasks and no "effect outstanding" gauge, so the only evidence is one `error` line that says nothing about which effect died.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | A failed effect task is logged and skipped; the loop continues to the next completed task. | E2 | `crates/core/src/effects.rs:74-89` | <pre>pub async fn next(&mut self) -> Resume {<br> loop {<br> match self.tasks.join_next.await {<br> Some(Ok(resume)) => {<br> tracing::trace!(?resume, "effect resume collected");<br> return resume;<br> }<br> Some(Err(err)) => tracing::error!(?err, "unexpected effect task failure"),</pre> |
| 2 | The doc comment states the policy explicitly. | E2 | `crates/core/src/effects.rs:64-68` | <pre>/// Waits for and returns the next successfully completed effect.<br>///<br>/// Task failures are logged and skipped. If no effect tasks are in<br>/// progress, this remains pending until the caller cancels the future.</pre> |
| 3 | A test asserts that a panicking effect is skipped and the run continues. | E2 | `crates/core/src/effects.rs:209-219` | <pre>#[tokio::test(start_paused = true)]<br>async fn skips_panicked_tasks_without_losing_successful_resumes {<br> let mut manager = manager;<br> manager.spawn(TestEffect::Panic);<br> manager.spawn(TestEffect::Complete {<br> after: Duration::from_secs(1),<br> resume: 42,<br> });<br><br> assert_eq!(manager.next.await, 42);<br>}</pre> |
| 4 | Handlers cannot report failure any other way: `perform_effect` is infallible by contract. | E2 | `crates/core/src/effects.rs:15-19` | <pre>/// Performs an effect and returns its result to resume a state machine.<br>///<br>/// Note that this method explicitly does not fail, and is expected to return<br>/// some result that indicates an error so the state machine can handle the<br>/// effect error internally.</pre> |
| 5 | Everywhere else the runtime is fail-stop: any driver-side error breaks the loop. | E2 | `crates/core/src/driver.rs:193-196` | <pre>if let Err(err) = result {<br> tracing::error!(?err, "unrecoverable driver error; exiting");<br> break;<br>}</pre> |
| 6 | Nothing counts the failure: the crate's metrics are RPC requests, block numbers and uncled blocks only. | E2 | `crates/core/src/metrics.rs:80-90` | <pre>pub fn initialize {<br> for status in ProcessingStatus::variants {<br> block_number(status).set(0.0);<br> }<br> uncled_blocks_total.absolute(0);</pre> |

## Trigger

**none identified** for the panic itself. I read both in-tree handlers (`crates/validator/src/service/effect.rs`, `crates/sentinel/src/effect.rs`) and found no `unwrap`, `expect`, `panic!`, slice index or `unreachable!` in their non-test code, so I cannot name an input that panics one today. The mechanism is reachable through any of: an arithmetic overflow in a debug build; a `Mutex` poisoning or a panic inside a dependency called from a handler (`crates/validator/src/service/effect.rs:111` holds a `tokio::sync::Mutex<NonceGenerator>` and the nonce generator runs work on OS threads, `crates/validator/src/secrets/nonces.rs`); or any future handler. The severity is set on the basis that no _current_ trigger is proven.

## Considered and rejected

- **"The `error!` log is enough."** It is the only signal, and it carries `JoinError`'s `Debug`, which identifies the task id, not the effect. There is no way to tell from it which session will now hang, and no counter to alert on.
- **"A panicking effect should be fatal, so this is a design choice."** Possibly, but then it is undocumented: nothing in `EffectHandler`'s contract tells a service author that a panic degrades to a silently dropped resume rather than a crash. Given that `perform_effect` cannot return an error, a panic is the only failure channel a handler has.
- **"`JoinError` could be a cancellation, not a panic."** `EffectManager` never calls `abort`/`abort_all`, and the only drop is the `Driver`'s own drop, after which `next` is never called again — so in this codebase `Some(Err(_))` means a panic. That makes the behaviour _more_ clear-cut, not less.
- **Not a duplicate of F-CORE-031**: same end state (a resume that never arrives), different cause (task failure vs. rollback) and different fix. Remediation 2 of F-CORE-031 (a persisted pending- effect set) would also cover this one; remediation 1 would not.

## Remediation options

1. Make a task failure fatal: return the `JoinError` from `next` (changing its signature to `Result<Resume, JoinError>`) and let the driver break the loop as it does for every other unrecoverable error. Tradeoff: one panicking effect takes the process down — which is the existing policy for transitions, and pairs with F-CORE-030's non-zero exit status.
2. Keep skipping but make it visible and attributable: `spawn` into the `JoinSet` with a task key (`JoinSet::build_task.name(...)` or a `spawn` wrapper that returns the `Effect`'s discriminant alongside the resume), increment a `safenet_core_effect_failures_total{effect}` counter, and log the effect that died rather than the `JoinError`.
3. Catch it in the handler layer instead: wrap `perform_effect` in `AssertUnwindSafe(...).catch_unwind` inside `spawn` and require services to supply a `Resume` for the panic case (e.g. via a `Default`/`from_panic` bound). This keeps the "infallible handler" contract honest — the state machine always gets a resume — at the cost of a new trait bound.

Tests to add: rename/extend `skips_panicked_tasks_without_losing_successful_resumes` to assert whatever policy is chosen (currently it pins the silent-drop behaviour), plus a test that the failure is observable (counter incremented, or error returned). No code is committed.

## Trail

- Reviewer R2: drafted from the brief's "EffectManager JoinSet failure propagation" question, self-estimate 70% (mechanism certain, no current panic trigger found — hence Low).

## Critic (C-CORE-B)

I read `effects.rs:64-89` first. `next` loops on `join_next`, returns on `Some(Ok(resume))`, logs `Some(Err(err))` at `error` and continues, and blocks on `future::pending` when the set is empty. A panicked task's resume is therefore never delivered and the effect is never retried — the state machine is left waiting. That matches the Claim.

### Per-claim verdicts

| # | Verdict | Note |
| --- | --- | --- |
| 1 | **Supported** | `effects.rs:74-81` verbatim. |
| 2 | **Supported** | `effects.rs:64-67` verbatim. |
| 3 | **Supported** | the test at `effects.rs:209-219` exists and asserts exactly that. |
| 4 | **Supported** | `effects.rs:15-19` verbatim; `perform_effect` returns `Resume`, not `Result`. |
| 5 | **Supported** | `driver.rs:193-196`. |
| 6 | **Supported** | `metrics.rs:80-90`; the crate exports only `rpc_requests_total`, `block_number` and `uncled_blocks_total`. |

I also checked the reviewer's own "`JoinError` could be a cancellation" rebuttal and agree: `abort`/`abort_all`/`detach_all` appear nowhere in `effects.rs`, so in this codebase `Some(Err(_))` is a panic.

### Finding verdict

**Plausible — 45%.** The mechanism is fully verified `E2`, but the reviewer states `## Trigger`: **none identified**, and I could not find one either — the two in-tree handlers have no `unwrap`, `expect`, `panic!`, slice index or unchecked arithmetic in non-test code. Under PROMPT.md §8 a verified mechanism with an unproven trigger is the 40-69 band; 45 rather than higher because the residual paths named (debug-build overflow, a dependency panic, a future handler) are all conditional on code that does not exist today.

**Severity: Low (unchanged).** Correctly rated: no untrusted input reaches a panic today, and the impact is one stranded session bounded by a service timeout.

Note the reviewer's own cross-reference is right: F-CORE-031's remediation 2 (a persisted pending-effect set) would close this too, and F-CORE-032 is the mechanism F-VAL-061 reaches by a different route (the validator's handler swallowing errors into `Resume::Noop` rather than panicking). They are separate defects with a shared consequence; neither should be merged into the other.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 2. Option 1 is defensible but should not be first; option 3 has a hidden bound.**

Option 2 (spawn with a task key, count `effect_failures_total{effect}`, log the effect that died rather than the bare `JoinError`) is sound, cheap and the right first change: today the failure is invisible _and_ unattributable, and attribution is what a service needs to decide anything.

Option 1 (make a task failure fatal) is consistent with the fail-stop policy applied elsewhere and pairs naturally with F-CORE-030. My reservation is scope: a panicking effect is a service bug, and taking the process down for it is only proportionate once panics in effects are known to be rare — which nothing currently measures. Do option 2 first, then option 1 once the counter shows the rate.

Option 3 (`catch_unwind` in `spawn` plus a `from_panic` bound on the service) keeps the "the state machine always gets a resume" contract honest, which is genuinely attractive. The hidden bound: it requires `AssertUnwindSafe`, and a handler that panics mid-mutation of shared state (the validator's nonce generator holds a `Semaphore` and a SQLite pool) may leave that state inconsistent — so "always gets a resume" would be paid for with a possibly-corrupt handler. Sound only if handlers are audited for unwind safety, which is a larger job than the option implies.

**Contract note:** all three keep `apply_transition` pure. Option 3 changes the `Service` trait (a new bound), which is the same kind of change F-CORE-031 option 2 needs (`Effect: Serialize`); if both land, they should land together.

## In-flight impact (AS-PRUNE)

**Pertains to unmerged branches, not to `main`.** Assessed against the Scheduled Secret Pruning stack (`origin/prune/end`, PRs #906–#913), built from a `git archive` extraction; no branch was merged or checked out. **Effect: unchanged.** Effect spawning and panic handling only gained housekeeping code.

## Reconciliation (run 2)

**Final: Plausible, Low, 45 (E2, no trigger), canonical — missed by run 2.** Run 2's R2 rejected 9 covers cancel-safety of `join_next`, not task failure; no run-2 log considers a panicking effect task. Still valid at `fe9e84c`: `EffectManager::next` still documents "Task failures are logged and skipped" (`effects.rs:76-79` after the housekeeping drift). No run-2 counterpart (`state/run2/reconciliation/core.md` §2).
