# F-CORE-040 The driver's inner `select!` restarts the watcher's in-flight RPC request on every effect resume, so a wide effect fan-out is paid for in abandoned `eth_getLogs` calls

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | core, `driver.rs` (`next_input`'s select) + `index/events.rs` (the cancelled future) |
| Location | `crates/core/src/driver.rs:206-231` (related: `crates/core/src/index/events.rs:281-299` and `:303-354`; `crates/core/src/index/blocks.rs:385-416`; `crates/core/src/effects.rs:64-73`) |
| Severity | Critic / Low |
| Certainty | 65% |
| Assumptions involved | A4 |
| Tags | dos, config |

## Claim

`next_input` builds a fresh `update` async block on **every call** and races it against `self.effects.next` in an unbiased `tokio::select!` (`driver.rs:206-231`). `EffectManager::next` is documented and tested cancel-safe (`effects.rs:64-73`, test at `:165-179`); the watcher branch is neither documented nor considered. When the resume branch wins, the `update` future is dropped — and with it whatever RPC request `Watcher::next` was suspended inside.

No _state_ is lost: `EventWatcher::warp` mutates `self.step` only **after** `fetch_logs` resolves (`index/events.rs:324-348`), and `BlockWatcher::next` likewise mutates `self.recent`/`self.pending` only after `get_block` resolves, with no await between the mutation and the return (`index/blocks.rs:441-469`). So a cancelled fetch is simply re-issued. That is why the reviewer's cancel-safety analysis (R2 coverage log §4.2, hypothesis 2) correctly finds no data loss.

What is lost is the **work**. Because `join_next` on an already-finished task is immediately ready while the watcher branch needs a full RPC round trip, the select will take the resume branch on essentially every iteration for as long as completed resumes are queued. Each of those iterations issues a new `eth_getLogs` (or `eth_getBlockByNumber`) and abandons it unfinished. A fan-out of `N` resumes therefore costs up to `N` wasted RPC round trips and delays indexing by one round trip per resume, against a provider that A4 explicitly allows to be rate limited.

This is the second-order cost of F-CORE-033: that finding is about the peak number of concurrent effect tasks, this one is about what draining them does to the RPC budget. Neither is a leak and neither is fatal, but together they turn one 100-block warp page into a burst of concurrent engine calls _and_ a burst of abandoned log queries.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The `update` future is constructed inside `next_input` and raced unbiased against the resume branch, so it is dropped whenever the resume branch wins. | E2 | `crates/core/src/driver.rs:206-231` | <pre>async fn next_input(&mut self) -> Result&lt;Input&lt;S::Event, S::Resume&gt;, index::Error&gt; {<br> let update = async {<br> loop {<br> match self.watcher.next.await {<br> Ok(update) =&gt; return Ok(update),</pre><br><pre> Ok(tokio::select! {<br> update = update =&gt; Input::Update(update?),<br> resume = self.effects.next =&gt; Input::Resume(resume)<br> })<br>}</pre> |
| 2 | `next_input` is called once per run-loop iteration, so each iteration builds a new future rather than resuming the previous one. | E2 | `crates/core/src/driver.rs:174-182` | <pre>loop {<br> let input = tokio::select! {<br> biased;<br> _ = shutdown.as_mut =&gt; {<br> tracing::info!("received shutdown signal; stopping service");<br> break;<br> },<br> input = self.next_input =&gt; input,<br> };</pre> |
| 3 | The cancelled future is suspended inside a real RPC call: `warp` awaits `fetch_logs` before touching any state. | E2 | `crates/core/src/index/events.rs:324-330` | <pre>let result = self.fetch_logs(fetch).await;<br>self.step = if result.is_ok {<br> if query_to_block == to_block {<br> Step::Idle<br> } else {</pre> |
| 4 | `fetch_logs` issues the provider request directly, so cancelling it abandons an in-flight `eth_getLogs`. | E2 | `crates/core/src/index/events.rs:402-410` | <pre>async fn fetch_logs(&self, fetch: Fetch) -&gt; Result&lt;Vec&lt;EventLog&lt;E&gt;&gt;, Error&gt; {<br> let logs = match fetch {<br> Fetch::SingleQuery(blocks) =&gt; {<br> let filter = blocks<br> .into_filter<br> .address(self.addresses.clone)<br> .event_signature(self.topics.clone);<br> let logs = self.provider.get_logs(&filter).await?;</pre> |
| 5 | The resume branch is cheap to make ready — a finished task is reaped without any I/O — so it wins the race whenever completed resumes are queued. | E2 | `crates/core/src/effects.rs:74-81` | <pre>pub async fn next(&mut self) -&gt; Resume {<br> loop {<br> match self.tasks.join_next.await {<br> Some(Ok(resume)) =&gt; {<br> tracing::trace!(?resume, "effect resume collected");<br> return resume;<br> }</pre> |
| 6 | The cancel-safety of the _resume_ branch is documented and tested; the watcher branch has no such statement, so the asymmetry is unconsidered rather than deliberate. | E2 | `crates/core/src/effects.rs:69-73` | <pre>/// # Cancel Safety<br>///<br>/// This method is cancel safe. If `next` is used as the event in a<br>/// `tokio::select!` statement and some other branch completes first, it is<br>/// guaranteed that no effect tasks were consumed.</pre> |
| 7 | No request timeout or retry layer is configured, so an abandoned request costs the node its full work with no client-side bound. | E2 | `crates/core/src/provider/mod.rs:129-137` | <pre>pub async fn connect(url: &Url) -&gt; Result&lt;Self, TransportError&gt; {<br> let client = ClientBuilder::default<br> .layer(ObservabilityLayer)<br> .connect(url.as_str)<br> .await?;</pre> |

## Trigger

1. A sentinel or validator processes an `Update::Logs` covering a warp page (up to `block_page_size = 100` blocks, `index/events.rs:96-97`) whose logs produce `N` effects. The driver spawns all `N` in one pass (`driver.rs:266-274`; this is F-CORE-033).
2. The effects complete. Their tasks sit finished in the `JoinSet`.
3. On each of the next `N` run-loop iterations, `next_input` starts a fresh `Watcher::next`, which issues an `eth_getLogs` for the next page and suspends; `effects.next` reaps a finished task immediately and wins the select; the log query is dropped mid-flight.
4. Net effect: `N` abandoned queries, `N` iterations before the next page is actually fetched, and `N` counted-then-abandoned entries in `safenet_core_rpc_requests_total` — the observability layer records the request when it is issued (`provider/mod.rs:66-76`), so the metric shows traffic the node was asked for and never answered into anything.

No adversary is required. A denser log range increases `N` directly, and under A2 an attacker who can cheaply emit watched events chooses that density.

## Considered and rejected

- **"This is a data-loss bug."** It is not, and the finding does not claim it. I traced both cancel points: `EventWatcher::warp` and `BlockWatcher::next` both complete their state mutations synchronously after the last await, so a dropped future leaves the watcher exactly where it was. R2's coverage log reached the same conclusion (§4.2, hypothesis 2) and is right about it.
- **"It is starvation, and R2 already recorded it as observation O-3."** O-3 frames it as the watcher being starved by a continuous resume stream and dismisses it as self-limiting — which is correct as far as it goes: resumes are only produced by effects that updates spawned, so the stream drains. What O-3 misses is that draining it is not free. The cost is not a stall, it is `N` wasted RPC round trips, and that is a resource claim rather than a liveness one.
- **"Adding `biased` with the watcher first would fix it."** It would invert the problem: resumes would then be starved while the watcher had work, and the state machine would fall behind its own effects. The fix belongs in the future's lifetime, not in the poll order.
- **"The wasted requests are cheap."** An `eth_getLogs` over a 100-block range across the watched addresses is one of the more expensive requests the service makes, and A4 explicitly admits a rate-limited provider — the case where wasted requests are exactly what must not happen. It also compounds with F-CORE-034's fixed 100 ms retry loop, since a provider pushed into rate limiting by this traffic is then hit ten times a second.
- **Not a duplicate of F-CORE-033.** That finding is the peak concurrency of the effect tasks; this is the RPC cost of draining them. Capping concurrency (F-CORE-033's remediation 1) reduces `N` and therefore mitigates this, but does not remove it: any resume that wins the select still discards an in-flight request.
- **Not a duplicate of F-CORE-039.** That one is about `update` being uncancellable _after_ an input is selected; this one is about the watcher future being cancelled _before_ one is.

## Remediation options

1. **Hold the watcher future across iterations.** Keep a pinned, long-lived `next_input` future (or a `futures::future::Fuse` stored on the `Driver`) so a partially-completed `Watcher::next` is resumed rather than rebuilt on the next loop pass. This is the direct fix and changes no semantics. Tradeoff: the driver has to store a self-referential future over `&mut self.watcher`, which in practice means restructuring the watcher into a `Stream` or moving it behind a channel.
2. **Move the watcher into its own task** feeding a bounded channel, and select over the channel instead. The task owns its RPC request and is never cancelled; the channel gives the natural backpressure that F-CORE-033 also wants. Larger change, and it fixes both findings at once.
3. **Drain ready resumes before re-entering the select.** Have the driver reap all already-finished resumes (a `try_join_next` loop) and process them before starting a new watcher future, so a fan-out costs one abandoned request rather than `N`. Cheapest option, and it preserves the current structure exactly.
4. **At minimum, document it.** `EffectManager::next` carries an explicit `# Cancel Safety` section; `next_input` should carry the counterpart, stating that the watcher branch is cancel-_safe_ but not cancel-_free_, so the next person to add a branch to that select knows the cost.

Tests to add: a `driver.rs` test (the file has no tests today) with a mocked provider counting `eth_getLogs` requests and a handler that completes `N` effects instantly, asserting that draining them issues at most a constant number of log queries. No code is committed.

## Trail

- Critic C-CORE-B: **drafted by the Critic** while auditing R2's rejected hypothesis 2 and observation O-3 (`../state/coverage-logs.md#r2` §4.2, §5). R2's refutation of the _data-loss_ form of the hypothesis is sound and I confirm it; what was dismissed too quickly is the resource cost of the same cancellation, which O-3 records only as "starvation … self-limiting". Re-derived from `driver.rs:206-231`, `index/events.rs:281-354` and `index/blocks.rs:385-469` in this session.

## Critic (C-CORE-B)

This section is the filing verdict, since no Reviewer drafted the finding.

**Plausible — 65%.** Basis rows 1-7 are all `E2` and the mechanism is certain: the future is rebuilt per iteration, it is suspended inside a real RPC call, and the competing branch is ready without I/O. What holds it out of the Confirmed band is the magnitude. How often the resume branch actually wins depends on `tokio::select!`'s randomised poll order and on how many resumes are queued at once, and the number of effects a real chain range produces cannot be measured in this read-only run (`state/baseline.md` §2). The worst case is clear; the typical case is not.

**Severity: Low.** No state is lost, nothing stalls permanently, and the waste is proportional to a fan-out that F-CORE-033 already argues should be capped. It is filed rather than left as an observation because it is a concrete, citable resource cost with a cheap fix (remediation 3), because it lands on the one dependency A4 warns may be rate limited, and because the asymmetry it exposes — one branch of the select has a documented cancel-safety contract and the other has none — is the kind of thing that silently gets worse when a third branch is added.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 3 as the cheap fix, option 2 as the right one.**

Option 3 (drain already-finished resumes with a `try_join_next` loop before re-entering the select) is sound, preserves the current structure exactly, and turns a fan-out of `N` abandoned requests into one. For a 65% Low finding that is the correct cost/benefit and I would take it.

Option 2 (move the watcher into its own task feeding a bounded channel) is the structurally right fix and has a second payoff the finding names: the channel provides the backpressure **F-CORE-033** wants. Two findings, one change. It is also the only option that removes the cancellation entirely rather than reducing its frequency.

Option 1 (hold a pinned `next_input` future across iterations) is sound in principle but the finding correctly identifies the obstacle — a self-referential future over `&mut self.watcher` — and its own resolution ("restructuring the watcher into a `Stream` or moving it behind a channel") _is_ option 2. The report should probably fold 1 into 2 rather than list them separately.

Option 4 (document the cancel-safety asymmetry next to `next_input`, mirroring the `# Cancel Safety` section `EffectManager::next` already carries) is sound and should land whichever else is chosen: the distinction between cancel-_safe_ and cancel-_free_ is exactly the thing the next person adding a branch to that select will not think about.

No option affects `apply_transition` or effect semantics; the `core::state` contract is unaffected.
