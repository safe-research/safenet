# F-CORE-035 The driver classifies _every_ RPC error as intermittent and swallows it forever, so a permanently failing node silently stops all onchain action while the service reports healthy progress

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | core, `driver.rs` (policy) with `tx/mod.rs` (classification) |
| Location | `crates/core/src/driver.rs:240-253` and `:276-284` (related: `crates/core/src/tx/mod.rs:44-66`; `crates/core/src/driver.rs:152-161`; `crates/core/src/metrics.rs:26-78`) |
| Severity | Medium / Medium |
| Certainty | 78% |
| Assumptions involved | A4, A1 |
| Tags | dos, config, crash-consistency |

## Claim

`Driver::update` runs the transaction queue's two entry points through `lift_intermittent_error`, which turns any `tx::Error::Rpc` into a `warn` line and continues. "Intermittent" is defined as _any_ `TransportError`, which in alloy includes a JSON-RPC **error response from the node**, not only a transport fault — the crate itself relies on that (`err.as_error_resp` is applied to a `TransportError` in `tx/mod.rs:363` and `index/mod.rs:139`). So a permanent, deterministic server-side condition — an endpoint that has disabled `eth_sendRawTransaction`, a provider returning a JSON-RPC rate-limit object on every call, an account rejected for insufficient funds, a proxy returning malformed JSON — is classified as "will naturally recover" and ignored on every block for as long as the process runs.

Meanwhile the state machine advances normally: `handle_update` is called after the swallowed error (`driver.rs:255`), snapshots commit, the block gauges keep climbing, and the transitions keep emitting actions that are durably enqueued and never submitted. The result is a validator or sentinel that is fully "healthy" by every signal it exports — `/health` is `OK` (F-CORE-030), `safenet_core_block_number{status="processed"}` tracks the head — while it has stopped participating onchain entirely. Core exports no transaction-queue metric of any kind (`metrics.rs` has exactly three metrics: RPC requests, block number, uncled blocks), and neither does the validator (`safenet_validator_transitions_total`, `safenet_validator_effects_total`). The only evidence is a `warn` line per block.

The same error class is treated inconsistently: `Driver::queue_action`, used for startup actions, does **not** lift the error, so an intermittent RPC failure there aborts start-up with a non-zero status. Identical failure, opposite policy, depending on whether the driver has started.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Queue reconciliation errors are logged and execution continues into the state machine. | E2 | `crates/core/src/driver.rs:242-257` | <pre>// Reconcile the transaction queue against the block watcher's<br>// current chain view before advancing the state machine, so<br>// freshly queued transactions are submitted against the latest<br>// known block. If we encounter an intermittent error, log and<br>// continue; things will naturally get a chance to recover.<br>let result = self.transactions.update_block_status(block_status).await;<br>if let Err(err) = tx::lift_intermittent_error(result)? {<br> tracing::warn!(<br> ?err,<br> "transaction queue failed to handle new block; will continue"<br> );<br>}<br><br>let commands = self.state.handle_update(update).await?;</pre> |
| 2 | Submission errors are treated the same way. | E2 | `crates/core/src/driver.rs:276-284` | <pre>if !transactions.is_empty {<br> let result = self.transactions.queue(transactions).await;<br> if let Err(err) = tx::lift_intermittent_error(result)? {<br> tracing::warn!(<br> ?err,<br> "transaction queue failed to queue transactions; will continue"<br> );<br> }<br>}</pre> |
| 3 | "Intermittent" is every `Rpc` variant, with no inspection of the payload. | E2 | `crates/core/src/tx/mod.rs:44-54` | <pre>impl Error {<br> /// Returns whether or not a transaction queue error is an intermittent<br> /// error that can be recovered from naturally.<br> fn is_intermittent(&self) -> bool {<br> // Note that we only consider RPC errors as transient - everything else<br> // including SQLite errors (which only happen if you are in a pretty<br> // borked FS situation or there is a bug in the SQL logic) and signing<br> // errors (which indicate some issue with the signer configuration) are<br> // considered more serious.<br> matches!(self, Self::Rpc(_))<br> }<br>}</pre> |
| 4 | A `TransportError` carries server-returned JSON-RPC error payloads — the crate depends on it elsewhere, so this is not an inference about alloy internals. | E2 | `crates/core/src/tx/mod.rs:360-368` | <pre>/// Whether `err` is a node rejection indicating that the transaction's fees<br>/// are too low for the mempool.<br>fn is_transaction_underpriced(err: &TransportError) -> bool {<br> err.as_error_resp.is_some_and(&#124;payload&#124; {<br> (iregex!("replacement transaction").is_match(&payload.message)<br> && iregex!("underpriced").is_match(&payload.message))<br> &#124;&#124; iregex!("INTERNAL_ERROR: could not replace existing tx").is_match(&payload.message)<br> })<br>}</pre> |
| 5 | The same class of error is fatal in `queue_action`, which does not lift it. | E2 | `crates/core/src/driver.rs:157-161` | <pre>pub async fn queue_action(&mut self, action: S::Action) -> Result<, Error> {<br> let transaction = self.actions.encode_action(action);<br> self.transactions.queue([transaction]).await?;<br> Ok()<br>}</pre> |
| 6 | Core exports no transaction-queue metric; the three it has are unrelated to submission health. | E2 | `crates/core/src/metrics.rs:26-36`, `62-70`, `72-78` | <pre>pub fn rpc_requests_total(method: &str, result: RpcRequestResult) -> Counter {</pre><br><pre>pub fn block_number(status: ProcessingStatus) -> Gauge {</pre><br><pre>pub fn uncled_blocks_total -> Counter {</pre> |
| 7 | Nothing is lost, which is why the failure is silent: rows are enqueued durably before submission is attempted. | E2 | `crates/core/src/tx/mod.rs:132-141` | <pre>pub async fn queue(<br> &mut self,<br> transactions: impl IntoIterator<Item = (Transaction, Option<u64>)>,<br>) -> Result<, Error> {<br> self.storage.enqueue(transactions).await?;<br> if let Some(status) = self.block_status {<br> self.submit_pending(status.latest).await?;<br> }<br> Ok()<br>}</pre> |

## Trigger

1. The configured RPC endpoint begins answering `eth_getTransactionCount` (or `eth_sendRawTransaction`) with a JSON-RPC error object rather than a transport failure — a provider-side rate limit expressed as `{"error":{"code":-32005,...}}`, a plan that disables the method, or a gateway returning an error body. No malicious RPC is required (assumption A4 admits stale and rate-limited providers).
2. On every block, `update_block_status` reaches `self.nonce.await?` (`tx/mod.rs:185-197`), fails, and returns `Err(Error::Rpc(..))`. `lift_intermittent_error` maps it to `Ok(Err(..))`; the driver logs `"transaction queue failed to handle new block; will continue"` and proceeds.
3. `handle_update` runs, the snapshot commits, `safenet_core_block_number{status="processed"}` advances, and any actions the transition produced are enqueued into SQLite. `submit_pending` fails the same way and is swallowed at step 2 of the next block.
4. This repeats indefinitely. The validator submits no `keyGenAndCommit`/`sign` transactions and the sentinel no `commit`/`reveal`, so both drop out of protocol participation, while every exported signal says the service is healthy and current.

## Considered and rejected

- **"It recovers naturally, as the comment says."** It does for a genuine blip: on the next block `previous.latest < status.latest` holds, so the full reconciliation path re-runs (`tx/mod.rs:185-197`). The finding is that there is no bound, no escalation and no signal for the case where it never recovers.
- **"Does the swallowed error corrupt queue state?"** Checked and rejected: `update_block_status` stores the new status _before_ the fallible work with an explicit comment (`tx/mod.rs:151-153`), so the only branch that could be skipped forever is the startup `unmark_executed(safe+1)`; reaching it requires a failure in `storage.prune`, which is a `storage::Error` and therefore **not** lifted — it kills the driver instead. Nothing is silently skipped, and enqueued rows survive (basis 7). No action is dropped or double-submitted by this path.
- **"The `warn` log is the signal."** It is the only signal, it is per-block rather than per-condition, and it is indistinguishable from the transient case that occurs routinely. There is no counter to alert on and no health effect.
- **"SQLite and signing errors are fatal, so serious failures are caught."** Serious _local_ failures are. A permanently failing remote is exactly the case that is not, and it is the more likely one.
- **Not a duplicate of F-CORE-034**: that is the watcher-side retry loop (which at least freezes the block gauge); this is the queue-side swallow, which keeps every gauge moving and is therefore strictly less observable.

## Remediation options

1. Count and expose: `safenet_core_transaction_queue_errors_total{stage}` plus a `safenet_core_transactions_outstanding` gauge (the queue already computes `count_outstanding`, `tx/mod.rs:186`). This alone converts a silent stall into an alertable one and is the smallest change.
2. Escalate on repetition: track consecutive lifted failures in the driver and treat N in a row (or a duration) as fatal, so the process exits — with a non-zero status once F-CORE-030 is fixed — and a supervisor can restart it against a healthy endpoint.
3. Narrow the classification: `is_intermittent` should exclude JSON-RPC error responses whose code indicates a permanent condition (method not found, invalid params) and treat `SerError`/`DeserError` as protocol faults rather than blips; `as_error_resp` is already used in this module for exactly this kind of inspection.
4. Make the two policies consistent: either `queue_action` should also lift (and log), or the run loop should not — the current split means the same provider fault is fatal at start-up and invisible thereafter.

Tests to add: a `driver.rs` test (none exist today) with a mocked provider that fails `eth_getTransactionCount` on every call, asserting the state machine still advances _and_ that the chosen signal (counter, or fatal exit after N) fires. No code is committed.

## Trail

- Reviewer R2: drafted from lead M10, self-estimate 75%. M10's own question ("can an action encoded against a stale chain view be dropped or double-submitted?") is answered _no_ in Considered and rejected; the residual defect is observability and the absence of any bound.

## Critic (C-CORE-B)

Read `driver.rs:240-253`, `:276-284` and `tx/mod.rs:44-66` before the Claim. `lift_intermittent_error` converts `Err(e)` into `Ok(Err(e))` exactly when `is_intermittent` is true, and `is_intermittent` is `matches!(self, Self::Rpc(_))` — the whole `TransportError` variant, with no inspection of the payload. Both driver call sites then log at `warn` and continue into `handle_update`. Independently confirmed.

The reviewer's key step — that a `TransportError` carries a _server-returned JSON-RPC error object_, not only a transport fault — is proved from inside this checkout rather than from alloy's sources: `is_transaction_underpriced` calls `err.as_error_resp` on a `TransportError` (`tx/mod.rs:363`) and `index/mod.rs:139` does the same. That is the right way to establish it under A6 and it is `E2`, not `I`.

### Per-claim verdicts

Rows 1-7 all **Supported**; every quote matches the cited range verbatim. Row 5 is the sharpest of them: `queue_action` (`driver.rs:157-161`) propagates the identical error class with `?`, so the same provider fault is fatal at start-up and invisible once the loop is running.

### A4 check

The trigger is a provider answering with a JSON-RPC error object — a plan that disables a method, a `-32005` rate-limit response, a gateway error body. That is a _stale or rate-limited_ provider, which A4 puts in scope; it is not the "malicious RPC lies to us" case A4 excludes. On the right side of the line.

### What I checked myself and agree with

The reviewer's most valuable negative result stands: nothing is dropped or double-submitted by this path. `update_block_status` assigns `self.block_status = Some(status)` at `tx/mod.rs:153` _before_ any fallible work; `prune`/`unmark_executed` are SQLite-only and a `storage::Error` is not intermittent, so it kills the driver rather than being swallowed; the first RPC call (`nonce`, `:188`) comes after both; and `enqueue` precedes submission (`:136`), so queued rows survive. I re-derived this and it holds. The residual defect really is the unbounded, unmetered swallow.

### Finding verdict

**Confirmed — 78%.** Mechanism `E2`; the trigger is a provider condition A4 admits, established from in-tree evidence rather than from dependency internals. Held below 85 because the specific JSON-RPC error codes a provider emits cannot be checked offline, and because the finding's substance is an observability absence rather than a state corruption.

**Severity: Medium (unchanged).** Correct. This is strictly _less_ observable than F-CORE-034 (every gauge keeps advancing) but its trigger is equally ordinary, and the outcome — a validator or sentinel that has silently stopped participating onchain while reporting healthy — is the same class of contained-but-serious failure. Not High because no attacker controls it and the queue recovers the instant the endpoint does.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 1 now, option 3 as the correct fix, option 4 to remove the inconsistency.**

Option 1 (`transaction_queue_errors_total{stage}` plus a `transactions_outstanding` gauge, which `count_outstanding` already computes at `tx/mod.rs:186`) is the smallest change and converts a silent stall into an alertable one. Take it first.

Option 3 (narrow `is_intermittent` so JSON-RPC error responses indicating a permanent condition, and `SerError`/`DeserError`, are not classified as blips) is the actual fix, and the finding correctly notes the machinery already exists — `as_error_resp` is used in this module for exactly this kind of inspection (`tx/mod.rs:363`). Sound.

Option 2 (escalate after N consecutive lifted failures) is sound but depends on **F-CORE-030**: today the escalation would exit with status 0 and a supervisor would restart into the same wedged state. Sequence it after.

Option 4 (make `queue_action` and the run loop agree — either both lift or neither) is worth doing on its own merits: the current split means the same provider fault is fatal at startup and invisible thereafter, which is the kind of asymmetry that makes an incident report impossible to write.

**Interaction to note:** narrowing `is_intermittent` (option 3) changes which failures reach the underpriced/generic branches in `submit_transaction`, which is the classification **F-CORE-061** is about. The two should be reviewed together so that widening one match and narrowing the other do not cancel out.
