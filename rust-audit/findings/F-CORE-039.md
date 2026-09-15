# F-CORE-039 Graceful shutdown is bounded only by the RPC's own patience: the shutdown branch is unreachable while an input is being processed, and no request timeout is configured anywhere

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | core, `driver.rs` (with `provider/mod.rs` as the missing-timeout site, R1's scope) |
| Location | `crates/core/src/driver.rs:174-198` and `:235-287` (related: `crates/core/src/provider/mod.rs:127-137`; `crates/core/src/utils.rs:13-36`; `crates/core/src/tx/mod.rs:202-219`) |
| Severity | Low / Low |
| Certainty | 55% |
| Assumptions involved | A4, A1, A6 |
| Tags | dos, crash-consistency, config |

## Claim

The run loop's `biased` select gives the shutdown signal priority, but only _between_ inputs: once an input is selected, `self.update(input).await` runs to completion with no cancellation point, by explicit design ("Once selected, an input is processed to completion before the run loop can stop; this prevents partial state applies", `driver.rs:184-185`). That is the right invariant. The problem is what `update` contains: up to a nonce fetch, a fee estimate and up to `max_in_flight_transactions` (default 16) `eth_sendRawTransaction` calls, over a provider built with no timeout layer and no retry layer — `ClientBuilder::default.layer(ObservabilityLayer).connect(url)` and nothing else.

So the upper bound on a graceful stop is whatever the underlying HTTP client decides, and core sets nothing. A stalled connection (a black-holed TCP session, a proxy holding the request open, an overloaded provider) therefore delays SIGTERM handling for as long as that request hangs. Container orchestrators do not wait: Docker sends SIGKILL 10 s after SIGTERM by default, Kubernetes after `terminationGracePeriodSeconds` (30 s). The graceful path then degrades into exactly the crash the design was avoiding — with the resume-loss consequences of F-CORE-031 and the duplicate-submission replay of CORE-H5 — while the code believes it prevented a partial apply.

The window is not the whole of `update`: the state-machine and snapshot work is local SQLite. It is specifically the transaction-queue calls, which bracket the state machine on both sides (`driver.rs:247` before, `driver.rs:277` after).

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Shutdown only wins between inputs; processing an input is deliberately uncancellable. | E2 | `crates/core/src/driver.rs:174-192` | <pre>loop {<br> let input = tokio::select! {<br> biased;<br> _ = shutdown.as_mut => {<br> tracing::info!("received shutdown signal; stopping service");<br> break;<br> },<br> input = self.next_input => input,<br> };<br><br> // Once selected, an input is processed to completion before the run<br> // loop can stop; this prevents partial state applies.</pre> |
| 2 | `update` makes network calls on both sides of the state machine. | E2 | `crates/core/src/driver.rs:247` and `:276-278` | <pre>let result = self.transactions.update_block_status(block_status).await;</pre><br><pre>if !transactions.is_empty {<br> let result = self.transactions.queue(transactions).await;</pre> |
| 3 | Those calls can be many: submission loops up to `max_in_flight_transactions` (default 16), each with its own broadcast. | E2 | `crates/core/src/tx/mod.rs:204-215` | <pre>async fn submit_pending(&mut self, block: u64) -> Result<, Error> {<br> let in_flight = self.storage.count_in_flight.await?;<br> for _ in in_flight..self.config.max_in_flight_transactions {<br> let nonce = self.nonce.await?;<br> let Some(transaction) = self<br> .storage<br> .next_transaction(Status { nonce, block })<br> .await?<br> else {<br> break;<br> };<br> self.submit_transaction(transaction, block).await?;</pre> |
| 4 | No timeout, retry or rate-limit layer is configured on the provider; the observability layer is the only one. | E2 | `crates/core/src/provider/mod.rs:127-137` | <pre>impl Provider {<br> /// Connects to `url`.<br> pub async fn connect(url: &Url) -> Result<Self, TransportError> {<br> let client = ClientBuilder::default<br> .layer(ObservabilityLayer)<br> .connect(url.as_str)<br> .await?;<br> let root = RootProvider::new(client);<br> let chain_id = root.get_chain_id.await?;<br> Ok(Self { root, chain_id })<br> }</pre> |
| 5 | Whether an un-timed request can hang indefinitely depends on the HTTP client's default, which is not on disk in this checkout. | I | `state/baseline.md` §1 ("Dependency sources are also absent") | — |
| 6 | The signal handler itself is only installed when `run` starts, so nothing is graceful before that either. | E2 | `crates/core/src/driver.rs:171-172` and `crates/core/src/utils.rs:17-23` | <pre>let shutdown = utils::shutdown_signal;<br>tokio::pin!(shutdown);</pre><br><pre>pub async fn shutdown_signal {<br> let sigterm = async {<br> unix::signal(unix::SignalKind::terminate)<br> .unwrap<br> .recv<br> .await<br> };</pre> |

## Trigger

1. A validator is running with 16 in-flight transactions and a provider whose connection has stalled (accepted, never answered — routine with load balancers, captive proxies and overloaded nodes; assumption A4 explicitly admits a stale or rate-limited provider).
2. A block update arrives. `update` calls `update_block_status`, which reaches `self.nonce.await?` and blocks on the stalled connection.
3. The operator deploys, and the orchestrator sends SIGTERM. The shutdown future is ready, but the run loop is inside `self.update(...).await` and never returns to the select.
4. After the grace period (10 s Docker, 30 s Kubernetes) the process is SIGKILLed mid-update. Any resume applied since the last log commit is lost (F-CORE-031); any effect in flight dies; the restart replays the retained window and re-queues its actions (CORE-H5).

## Considered and rejected

- **"`biased` fixes this."** `biased` only fixes the ordering _at the select_, guaranteeing the shutdown wins over a simultaneously-ready input. It has no effect once a branch has been entered.
- **"Uncancellable processing is wrong."** It is right, and this finding does not propose cancelling it: a half-applied update is worse than a slow stop. The fix belongs at the RPC layer (bound each request) and in the driver's willingness to stop _before_ starting more submissions.
- **"`next_input` is cancellable, so most of the time we are fine."** True — the common state is waiting in `next_input`, which the shutdown pre-empts cleanly, and `EffectManager::next` is documented and tested cancel-safe (`effects.rs:69-73`, test `effects.rs:165-179`). I re-checked the inner select for cancel-safety: `join_next` is only consumed on the `Some(Ok(_))` path, which returns without an intervening await (`effects.rs:76-80`), so no resume can be dropped by cancellation. The exposure is exactly the `update` window.
- **"The SQLite writes are the risk."** They are local and short; the pool's own acquire timeout bounds them (sqlx default, not verifiable here). The unbounded part is the network.
- **Overlap:** this is CORE-H12's shutdown half. The other half — that a hung request also stalls _indexing_ — sits in R1's `provider`/`index` scope, and the fix (a timeout layer) is the same.

## Remediation options

1. Add a request timeout to the provider (a `tower` timeout layer, or the HTTP client's own) sized well below the deployment's grace period, so every `await` inside `update` has a bound. Single change, fixes both halves of CORE-H12. Tradeoff: a timeout that is too tight turns a slow node into a retry storm — pair it with F-CORE-034's backoff.
2. Make the driver stop _starting_ work once shutdown is pending: keep the current uncancellable apply, but check a shutdown flag before the second transaction-queue call (`driver.rs:276`) and before spawning effects, so the loop finishes the state-machine work and exits without opening new submissions.
3. Document the intended grace period next to the run loop and in the handbooks, so operators set `terminationGracePeriodSeconds`/`stop_grace_period` above the worst case rather than the default.

Tests to add: a `driver.rs` test (none exist) with a mocked provider whose response never arrives, asserting that `run` returns within a bounded time after the shutdown future resolves. No code is committed.

## Trail

- Reviewer R2: drafted from lead CORE-H12 and core checklist item 11, self-estimate 65%. The driver-side mechanism is `E2`; the "hangs forever" premise is `I` because the HTTP client's default timeout cannot be read in this checkout (A6).

## Critic (C-CORE-B)

Read `driver.rs:174-198` and `:235-287` first. The `biased` select gives shutdown priority _at the select_, but the comment at `:184-185` is explicit that an input, once taken, is processed to completion; `update` then makes RPC calls on both sides of the state machine (`:247` and `:277`), and `submit_pending` can issue up to `max_in_flight_transactions` broadcasts, each preceded by a nonce fetch and a fee estimate (`tx/mod.rs:204-216`). `Provider::connect` installs `ObservabilityLayer` and nothing else (`provider/mod.rs:129-137`). All confirmed.

### Per-claim verdicts

| # | Verdict | Note |
| --- | --- | --- |
| 1-4 | **Supported** | verbatim at the cited ranges. |
| 5 | **Supported as an `I` claim, correctly classed** | whether an un-timed request can hang indefinitely depends on the HTTP client's default, and no dependency source is on disk. I confirmed the in-tree half independently: `grep -rn timeout crates/core/src` finds nothing but `utils.rs`'s two sqlx pool knobs and one test, so the title's "no request timeout is configured anywhere" is accurate for this workspace. |
| 6 | **Supported** | `driver.rs:171-172` and `utils.rs:17-23`; the handler is installed inside `run`, so nothing before it is graceful. |

The reviewer's cancel-safety re-check is also right and worth keeping: `EffectManager::next` consumes a resume only on the `Some(Ok(_))` arm and returns without an intervening await (`effects.rs:76-80`), pinned by `next_is_cancel_safe` (`effects.rs:165-179`). I separately traced the _watcher_ side of the same select and found no data loss either — `EventWatcher::warp` mutates `self.step` only after `fetch_logs` resolves (`index/events.rs:324-348`), so a cancelled fetch is simply re-issued. That is not free, however, and I have filed the cost as **F-CORE-040**.

### Finding verdict

**Plausible — 55%.** The driver-side mechanism is `E2` and complete; the premise that turns it into a real stall — that an un-timed HTTP request can hang past a container grace period — is `I` and unverifiable in this run. By the rubric that is squarely the 40-69 band. I set 55 rather than the reviewer's 65 because the whole harm depends on that single unverified premise and on a deployment's grace-period setting, neither of which is in this checkout.

**Severity: Low (unchanged).** Correct. The worst case is a SIGKILL mid-`update`, whose consequences (a lost resume, a replayed action) are already filed as F-CORE-031 and F-SEN-006 / F-VAL-065; this finding is the enabling condition, not an independent harm. Remediation 1 (a `tower` timeout layer) is the same one-line fix that F-CORE-034 needs and should land with it.

## Cross-reference (Critic C-CORE-A, covering R1's `core` findings)

Not a critique of this finding — its assigned Critic owns that. Recorded here because the Manager asked me to decide whether the _provider-side_ absence of any timeout or retry layer (`provider/mod.rs:129-137`, which sits in R1's scope) is covered by this file.

**It is not, and I have filed the remainder as F-CORE-011.** This finding's claim, trigger, remediation and proposed test are all about the shutdown path: that `Driver::run`'s uncancellable `update` window has no upper bound, so SIGTERM handling is delayed past the orchestrator's grace period. Your own overlap note says as much — "this is CORE-H12's shutdown half. The other half — that a hung request also stalls _indexing_ — sits in R1's `provider`/`index` scope". R1 likewise recorded the citation in its coverage log (§8, "CORE-H12 (no RPC timeouts) not filed by me") and left it for the Critic, so without a promotion the indexing half would have fallen between the two scopes.

F-CORE-011 states the two consequences this file does not: (a) a stalled request parks the _indexer_ in an `await` that never resolves, and because `metrics::rpc_requests_total` is incremented only after `inner.call(...).await` returns (`provider/mod.rs:90-114`), not even the failure counter moves while `/health` answers `OK`; and (b) there is no backoff or jitter anywhere between the socket and `Driver::next_input`'s flat `STEP_RETRY_DELAY`, so a `429` is retried at a fixed 10 Hz — which, with `use_client_filtering` on, spends F-CORE-002's three-attempt integrity budget in 300 ms.

**Neither file subsumes the other and both should survive**; this one stays canonical for the shutdown half. They share one remediation — a timeout layer in `Provider::connect`, paired with backoff — so they should be fixed together, and I have said so in F-CORE-011's _Considered and rejected_. I also endorse your basis 5 caveat: the "hangs forever" premise is `I` in this checkout (no `alloy`/`reqwest` source on disk, A6), and I carried that limitation into F-CORE-011's certainty rather than papering over it.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 1, which is the same change as F-CORE-011 option 1.**

Option 1 (a request timeout on the provider, sized well below the deployment's grace period) is sound and closes both halves. It is literally the same layer F-CORE-011 option 1 asks for — the two findings should be resolved by one change, and the report should present them that way. Its tradeoff (too tight a value turns a slow node into a retry storm) is real and is why it must land with **F-CORE-034 option 1**'s backoff.

Option 2 (check a shutdown flag before the second transaction-queue call at `driver.rs:276` and before spawning effects, so the loop finishes the state-machine work and exits without opening new submissions) is sound and is the more precise fix, because it preserves the deliberate invariant that an input is processed to completion (`driver.rs:184-185`) while removing the part of `update` that is genuinely optional at shutdown. I would take both: option 1 bounds the worst case, option 2 removes the common case.

Option 3 (document the grace period in the handbooks) is necessary and currently absent; operators setting `terminationGracePeriodSeconds` have nothing to size it against.

**Certainty note, not a change:** basis 5 — whether an un-timed request can hang indefinitely — rests on `reqwest`'s default, which is not on disk (A6). Question 4 in `rust-audit/poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md`. Answering it is what moves this out of 55%, and the same answer moves F-CORE-011.
