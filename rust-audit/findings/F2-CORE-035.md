# F2-CORE-035 Effect concurrency is unbounded: one task per `Command::Effect`, so a catch-up warp fans out arbitrarily many concurrent effects

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | safenet-core, effects.rs (driver.rs) |
| Location | crates/core/src/effects.rs:60-69 (related: crates/core/src/driver.rs:272-279; crates/sentinel/src/effect.rs:55-75; crates/sentinel/src/main.rs:50-62; crates/sentinel/src/service.rs:127-139) |
| Severity | Low / Low |
| Certainty | 70% (Critic C2-CORE-B; reviewer self-estimate in Trail) |
| Assumptions involved | A2, A3 |
| Tags | dos |

Audited commit: `3ec8bc5`.

## Claim

`EffectManager::spawn` puts every effect into a `JoinSet` as its own tokio task with no concurrency limit, and the driver spawns every `Command::Effect` a transition returns. A log-range update during a `Warp` (restart after downtime, or a fresh `start_block`) applies up to `block_page_size` blocks of events in one call and hands all their effects to the manager at once. For the sentinel, each `TransactionProposed` produces an `EngineCheck` effect that opens an HTTP request to the engine with a timeout of roughly three quarters of the voting window (minutes on Gnosis), and the sentinel's handler has no semaphore, so a catch-up over a busy range issues that many concurrent requests. Because `Warp` delivers no `NewBlock` messages, entries whose commit deadline already passed during the downtime are not pruned before their check runs, so the fan-out includes work that can no longer lead to a vote. Memory is bounded only by the number of pending effects times the size of each `transaction` payload held in the task.

The exposure is governed by adversarially controllable input (anyone can propose Safe transactions, A2) and by operator downtime; the victim is the sentinel's own process and its co-deployed engine (A3). The validator's handler serialises its expensive effect (nonce generation) behind a `Semaphore::new(1)`, so it is less exposed.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Every effect becomes a task in an unbounded `JoinSet`. | E2 | crates/core/src/effects.rs:60-69 | `pub fn spawn(&mut self, effect: Effect) {` ... `self.tasks.spawn(async move {` `let resume = handler.perform_effect(effect).await;` ... `resume` `});` |
| 2 | The driver spawns every effect from a command batch without throttling. | E2 | crates/core/src/driver.rs:272-279 | `for command in commands { match command {` ... `state::Command::Effect(effect) => self.effects.spawn(effect),` `} }` |
| 3 | A warp page applies every log of the page in one `handle_update`, accumulating all their commands. | E2 | crates/core/src/state/mod.rs:213-223 | `for log in logs { let (new_state, new_commands) = self.transition.apply_transition(state, Message::Event(log)); state = new_state; commands.extend(new_commands); }` |
| 4 | Sentinel: each proposal spawns an engine check. | E2 | crates/sentinel/src/service.rs:127-139 | `state.0.insert(request_id, RequestState::WaitingForEngineCheck { deadline, request: None, },);` ... `vec![Command::Effect(effect::Effect::EngineCheck {` |
| 5 | Sentinel: the handler performs one HTTP request per effect with no concurrency bound. | E2 | crates/sentinel/src/effect.rs:55-75 | `async fn perform_effect(&self, effect: Effect) -> Resume {` `match effect { Effect::EngineCheck { request_id, transaction, block, } => { let outcome = self.engine.security_check(block, &transaction).request_id(request_id).timeout(self.engine_timeout).execute().await;` |
| 6 | Sentinel: the per-request timeout scales with the voting window in blocks. | E2 | crates/sentinel/src/main.rs:50-62 | `let engine_timeout = {` ... `u128::from(config.sentinel.voting_window.saturating_sub(1)).saturating_mul(u128::from(block_time)).saturating_mul(3) / 4,` ... `.max(1_000),` |
| 7 | Warps deliver events but no `NewBlock`, so deadline pruning does not run during catch-up. | E2 | crates/core/src/state/mod.rs:173-181, 200-239 | `Update::Block(BlockUpdate::Warp { from, to })` ... `(state, status, vec![])` ... (only `Message::Event(log)` is applied for the range) |
| 8 | Validator handler bounds its expensive effect; the sentinel does not. | E2 | crates/validator/src/secrets/nonces.rs:102, 118 | `pending: Arc<Semaphore>,` ... `pending: Arc::new(Semaphore::new(1)),` |

## Trigger

Sentinel down for longer than `max_reorg_depth` blocks while N `TransactionProposed` events for its oracle land (an attacker can generate N proposals at the cost of the request fee/bond, A2). On restart the watcher emits `Warp{safe+1, node_safe}`; the first page applies up to 100 blocks of proposals, `handle_update` returns N `EngineCheck` effects, the driver spawns N tasks, and the sentinel opens N concurrent connections to the engine, each allowed to run for `engine_timeout`. Not run this session.

## Considered and rejected

- "The watcher is pull-based, so it is bounded" - the watcher is, the effect side is not; one pull yields up to `block_page_size` blocks of effects.
- "The engine is out of scope" - the finding is about the sentinel's own unbounded task/socket fan-out; the engine is only the counterpart.
- "The pool of tokio tasks is cheap" - tasks are, but each holds the proposal payload and an open HTTP request; the unbounded part is the number of simultaneous outbound requests.

## Remediation options

1. Add a concurrency limit to `EffectManager` (a `Semaphore` acquired inside the spawned task before `perform_effect`, configurable per service); the state machine is unaffected because resumes are already order-independent.
2. Sentinel-side: bound `EngineCheck` with a semaphore in its handler, and skip checks whose `deadline`/`commit_deadline` is already below the current block (needs the block passed to the effect, which it already carries).
3. Deliver a synthetic `NewBlock(to)` at the end of a warp so deadline-based pruning runs before the next page's effects are spawned (also relevant to the observation on warp replay semantics in R2's log).

Tests to add: an `effects.rs` test spawning more effects than the limit and asserting the handler's peak concurrency; a sentinel test applying a warp page with expired proposals and asserting no `EngineCheck` is emitted for them.

## Trail

- Reviewer R2: drafted, self-estimate 60% (E2; resource impact not measured). Severity Low.
- Critic C2-CORE-B: Confirmed, 70%, severity Low (reviewer Low).

## Critic (C2-CORE-B)

Method: read title and Location only, traced `EffectManager::spawn`, the driver's dispatch loop, the warp/log-page path in `state/mod.rs` and the sentinel handler myself, then compared.

| # | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | effects.rs:61-69: `self.tasks.spawn(..)` into a `JoinSet` with no limit. |
| 2 | Supported | driver.rs:272-279. |
| 3 | Supported | state/mod.rs:213-223: all logs of the page are applied and their commands accumulated before returning. |
| 4 | Supported | sentinel service.rs:127-145. |
| 5 | Supported | sentinel effect.rs:55-75; no semaphore in the handler. `EngineClient` is built on `reqwest::Client::new()` (sentinel engine.rs:113), which caps idle pooling only, not concurrency. |
| 6 | Supported | sentinel main.rs:50-62; with the shipped `voting_window = 100` (sentinel.sample.toml:35) and 5 s blocks the per-request timeout is about 371 s. |
| 7 | Supported | state/mod.rs:173-181 and 200-239: a warp delivers `Message::Event` only. |
| 8 | Supported | validator secrets/nonces.rs:12, 102, 118: `Semaphore::new(1)`. |

Additions from my trace: (i) the fan-out is not warp-specific — a single `Logs{n}` carrying M proposals spawns M tasks through the same path, so the live head is exposed too; (ii) the sentinel-side outcome when the engine or the process's descriptors saturate is `CheckOutcome::Unknown` → "engine check failed; dropping request unanswered" (sentinel service.rs:176-179): missed votes and a transient stall, no crash, self-recovering once the checks time out.

Finding verdict: **Confirmed**. Certainty **70%** (mechanism E2; trigger concrete but the resource ceiling is not measured). Severity **Low / Low** from core's side: the exhaustion is bounded by the attacker's own transaction spend and recovers by itself; the protocol-level cost (fewer sentinels voting) depends on engine throughput, which is out of scope.

Overlap: `F2-SEN-005` (R7, reviewer Medium) frames the same fan-out as forced abstention under a proposal flood at low attacker cost. That severity argument belongs to the sentinel finding; the Manager should consolidate the two. Remediation 1 (a semaphore inside `EffectManager`) is sound because resumes are documented as order-independent (state/mod.rs:49-50).

## Reconciliation (run 2)

**Final: EXTENDS `F-CORE-033` (canonical) — combined Medium, 70 (E2; sentinel side E1 via `F2-SEN-005`).** Same unbounded fan-out; this file adds that warps deliver no `NewBlock`, so expired proposals are checked too — the fact that answers the team's question on `F-CORE-033` ("the transaction deadline should prevent this fan-out": it bounds the follow-up transactions at allocation, not the effects) (`state/run2/reconciliation/core.md` §4.1). Run 1 rated Medium and this file Low from core's side; Medium is carried with `F2-SEN-005` as the executed sentinel counterpart.
