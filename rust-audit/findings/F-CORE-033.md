# F-CORE-033 Effect concurrency is unbounded: one backfill page can spawn a task per matching log at once, with no cap, no queue and no backpressure

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | core, `effects.rs` + `driver.rs` |
| Location | `crates/core/src/effects.rs:53-62` (related: `crates/core/src/driver.rs:266-274`; `crates/core/src/state/mod.rs:213-223`; `crates/core/src/index/events.rs:94-97`) |
| Severity | Medium / Medium |
| Certainty | 70% |
| Assumptions involved | A1, A3, A4 |
| Tags | dos, crash-consistency |

## Claim

`EffectManager::spawn` pushes straight into an unbounded `JoinSet` and returns; there is no concurrency limit, no semaphore, no queue depth and no way for a handler to apply backpressure. The driver spawns _every_ effect returned by a single `handle_update` in one synchronous loop, and a single `handle_update` can carry the transitions of a whole warp page — `block_page_size` blocks, default **100** — with one command list concatenated across every log in the range (`state/mod.rs:213-223`). Resumes drain at one per driver loop iteration, so the set only shrinks after the fan-out is complete.

The transaction queue, by contrast, is explicitly bounded (`max_in_flight_transactions`, default 16, `tx/mod.rs:204-206`). Effects — which do the network I/O — are not bounded at all.

For the sentinel this is a direct amplifier: every `TransactionProposed` becomes an `Effect::EngineCheck` carrying the full proposed Safe transaction (`crates/sentinel/src/service.rs:139-143`), and each check is an HTTP request to the co-deployed engine, which in turn fans out to RPC-backed checkers. A backfill over a busy range therefore issues hundreds of concurrent engine requests — against a service with no rate limiting by design (assumption A3) — and simultaneously retains every in-flight `Effect`'s transaction payload (attacker-influenced size, assumption A2) in memory. The situation is reached by ordinary operation: a `start_block` backfill, or a restart after an outage long enough that the resume warps.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | `spawn` is unbounded and non-blocking: it clones the handler and pushes into the `JoinSet`. | E2 | `crates/core/src/effects.rs:53-62` | <pre>pub fn spawn(&mut self, effect: Effect) {<br> tracing::trace!(?effect, "spawning effect task");<br> let handler = Arc::clone(&self.handler);<br> self.tasks.spawn(async move {<br> let resume = handler.perform_effect(effect).await;<br> tracing::trace!(?resume, "effect task finished");<br> resume<br> });<br>}</pre> |
| 2 | The driver spawns every effect in the command list in one pass, with no yield or limit. | E2 | `crates/core/src/driver.rs:267-274` | <pre>for command in commands {<br> match command {<br> state::Command::Action(action) => {<br> transactions.push(self.actions.encode_action(action));<br> }<br> state::Command::Effect(effect) => self.effects.spawn(effect),<br> }<br>}</pre> |
| 3 | One update's command list spans every log in the update's block range, so a warp page contributes all of its logs' effects at once. | E2 | `crates/core/src/state/mod.rs:213-223` | <pre>let (state, commands) = {<br> let mut state = state;<br> let mut commands = Vec::new;<br> for log in logs {<br> let (new_state, new_commands) =<br> self.transition.apply_transition(state, Message::Event(log));<br> state = new_state;<br> commands.extend(new_commands);<br> }<br> (state, commands)<br>};</pre> |
| 4 | A warp page is 100 blocks by default. | E2 | `crates/core/src/index/events.rs:96-97` | <pre>Self {<br> block_page_size: NonZeroU64::new(100).expect("100 is nonzero"),</pre> |
| 5 | Resumes are consumed one per driver loop iteration, so the set drains far slower than it fills. | E2 | `crates/core/src/driver.rs:227-230` | <pre>Ok(tokio::select! {<br> update = update => Input::Update(update?),<br> resume = self.effects.next => Input::Resume(resume)<br>})</pre> |
| 6 | The comparable outbound path _is_ bounded, which shows the omission is not a deliberate symmetry. | E2 | `crates/core/src/tx/mod.rs:204-206` | <pre>async fn submit_pending(&mut self, block: u64) -> Result<, Error> {<br> let in_flight = self.storage.count_in_flight.await?;<br> for _ in in_flight..self.config.max_in_flight_transactions {</pre> |
| 7 | Each sentinel effect carries a full proposed transaction and becomes an engine HTTP call. | E2 | `crates/sentinel/src/service.rs:139-143` | <pre>vec![Command::Effect(effect::Effect::EngineCheck {<br> request_id,<br> transaction: event.transaction,<br> block,<br>})],</pre> |

## Trigger

1. A sentinel (or validator) is started against a database whose newest snapshot is far behind the head — a fresh deployment with `start_block` set, a restore from backup, or a restart after an outage. `BlockWatcher::initialize` queues `Warp{safe+1, node_safe}` (`blocks.rs:271-278`).
2. Each warp page delivers up to 100 blocks of logs in one `Update::Logs`. Every `TransactionProposed` in those 100 blocks produces one `Command::Effect`.
3. `Driver::update` spawns all of them before returning to the select loop, so all of them are in flight simultaneously; the next page is fetched as soon as one resume is consumed, so the set keeps growing across pages.
4. Each in-flight task holds its `Effect` (with the full transaction calldata) and one open HTTP connection to the engine. The engine has no rate limiting (A3) and its RPC-backed checkers fan out further.

No adversary is required; a busy oracle contract and a long backfill suffice. An adversary who can cheaply emit watched events (A2) increases the density of the range at will.

## Considered and rejected

- **"The watcher is pull-based, so it is naturally bounded."** It bounds _updates_, not effects: the watcher yields the next page as soon as the driver takes one input, and each input can add an unbounded number of tasks.
- **"`max_in_flight_transactions` covers it."** That caps onchain submissions only (`tx/mod.rs:204-206`); effects never touch that path.
- **"Tokio will schedule them fairly."** Scheduling is not the constraint — sockets, engine capacity, RPC quota and retained `Effect` payloads are. Nothing here is bounded by the runtime.
- **"The sentinel's `engine_timeout` bounds it."** It bounds each request's _duration_ (`crates/sentinel/src/main.rs:50-62`), which caps how long the fan-out lasts, not how wide it is; and a wide enough fan-out makes every request slower, so more of them time out — the failure amplifies itself.
- **Checked and not applicable:** the `JoinSet` is not leaked — completed tasks are reaped by `next` and the set is dropped with the driver (`effects.rs:30-31`). The growth is transient, not a leak; the finding is about peak concurrency, not unbounded memory retention over a long run.

## Remediation options

1. Cap concurrency in `EffectManager`: keep a `VecDeque<Effect>` of pending effects and only `tasks.spawn` while `tasks.len < config.max_concurrent_effects`, refilling in `next` after each reap. This preserves the `spawn` signature and keeps the manager the single owner. Tradeoff: a new config knob, and effects then queue in memory rather than running — which is fine as long as the queue is fed from durable chain state, but it does interact with F-CORE-031 (a queued but un-spawned effect is lost on shutdown just as an in-flight one is).
2. Hand the handler a `Semaphore` permit: `EffectHandler` acquires before doing I/O. Keeps core simple, but every service has to remember to do it, and the memory for pending `Effect` values is still retained.
3. Bound the input side instead: cap the number of logs per `Update::Logs` (a `max_logs_per_query` is already available but defaults to `None`, `index/events.rs:100`), which limits the fan-out per update without touching the effect layer. Weakest option — it does not stop many updates in a row from accumulating.

Tests to add: an `effects.rs` test that spawning N effects with a cap of K never has more than K tasks running (observable via an `AtomicUsize` in the test handler — the existing `TestHandler` already counts calls); a driver-level test that a 100-block warp page with many effect-producing logs does not exceed the cap. No code is committed.

## Trail

- Reviewer R2: drafted from the map's "backpressure/growth" note and core checklist item 6/7, self-estimate 70% (mechanism certain; the peak magnitude depends on chain history I cannot measure offline).

## Critic (C-CORE-B)

Derived independently: `EffectManager::spawn` (`effects.rs:53-62`) pushes into a `JoinSet` with no permit, no queue and no `await`, and `Driver::update` (`driver.rs:266-274`) drains the whole command list in one synchronous loop. `handle_update`'s `Update::Logs` arm concatenates the commands of every log in the range (`state/mod.rs:213-223`), and a warp page is `block_page_size` blocks wide, default 100. There is no cap anywhere on the effect path, while the outbound transaction path _is_ capped at `max_in_flight_transactions` (`tx/mod.rs:204-206`). All of that is exactly as claimed.

### Per-claim verdicts

Rows 1-3 and 5-7 **Supported**, verbatim at the cited ranges. Row 4 **Supported**: `crates/core/src/index/events.rs:96-97` reads `block_page_size: NonZeroU64::new(100).expect("100 is nonzero")`, and I confirmed `max_logs_per_query` defaults to `None` at `:100`, so nothing else bounds a page's log count either.

One refinement to row 5. Resumes are consumed one per driver loop iteration, but the drain is faster than the reviewer implies: `join_next` on an already-finished task is immediately ready while the watcher branch needs an RPC round trip, so the select at `driver.rs:227-230` will take the resume branch on essentially every iteration until the completed queue empties. That makes the _set_ drain quickly; it does not change the peak, which is what the finding is about. It does, however, produce a second-order cost the reviewer missed, which I have promoted separately as **F-CORE-040**.

### Finding verdict

**Confirmed — 70%.** Mechanism `E2` and complete; trigger (`Warp` on a restart or a `start_block` back-fill, `blocks.rs:271-278`) verified in code with no external assumption. Held at 70 because the peak magnitude is a function of chain history that cannot be measured in this run — a range with few `TransactionProposed` logs produces a harmless fan-out, and nothing in the repository pins the density.

**Severity: Medium (unchanged).** Not High: A3 puts the engine behind the co-deployed sentinel, so this is self-inflicted load rather than a remote DoS, and the growth is transient rather than a leak. Not Low: the amplifier is real, the payloads retained are attacker-sized under A2, and the asymmetry with the explicitly-bounded transaction queue shows the omission was not deliberate.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 1, with the interaction it names taken seriously.**

Option 1 (a pending `VecDeque` in `EffectManager`, spawn while `tasks.len < max_concurrent_effects`, refill in `next`) is sound and keeps the manager the single owner, which is what makes it maintainable. Its own caveat is the important part and I want to strengthen it: **a queued but un-spawned effect is lost on shutdown exactly as an in-flight one is**, so option 1 enlarges F-CORE-031's loss window rather than leaving it unchanged. If F-CORE-031 option 2 (a durable pending-effect set) lands, the two structures should be the same structure — a queue that is also the durable set — not two.

Option 2 (a `Semaphore` permit acquired inside the handler) is unsound as a _core_ fix: it moves the obligation to every service, where it can be forgotten, and it does not bound memory because the pending `Effect` values are still retained in spawned tasks. It is a service-local mitigation, not a remediation, and should be labelled as such.

Option 3 (cap logs per `Update::Logs` via `max_logs_per_query`) is the weakest and the finding says so; note it also interacts with F-CORE-012 option 2, which wants that same knob defaulted for a different reason. Setting it helps both, but only bounds one update at a time.

**Cross-finding:** for the sentinel this is F-SEN-004 option 1 by another name — that finding asks for `max_concurrent_effects` on `EffectManager` explicitly. One change, two findings.

## In-flight impact (AS-PRUNE)

**Pertains to unmerged branches, not to `main`.** Assessed against the Scheduled Secret Pruning stack (`origin/prune/end`, PRs #906–#913), built from a `git archive` extraction; no branch was merged or checked out. **Effect: unchanged.** Effect concurrency is unbounded as before; housekeeping itself runs inline, not as a spawned effect.
