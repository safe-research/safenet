# Plan: Non-Blocking Effects

Component: `crates/core` (state machine, effects, driver), `crates/sentinel` and `crates/validator` (Rust).

---

## Overview

Every [`Command::Effect`](../crates/core/src/state/mod.rs) a [`StateTransition`](../crates/core/src/state/mod.rs) emits today is executed inline by `TransitionBatch::apply`. The state machine owns the effect handler, awaits `EffectHandler::perform_effect`, and immediately applies the resulting `Message::Resume` before it continues through the rest of the command queue. A slow effect therefore blocks all later indexer updates and shutdown handling.

This epic makes the existing effect model uniformly asynchronous. There is no separate "immediate" and "background" effect class: every `Command::Effect` is returned to the [`Driver`](../crates/core/src/driver.rs) and handed to an `EffectManager` that schedules it in an encapsulated `tokio::task::JoinSet`. The driver selects completed effects from the manager alongside indexer updates and shutdown. A completed effect is given back to the state machine as `Message::Resume`, producing another ordinary state transition and zero or more new commands.

For the validator this changes when resume transitions run, and therefore when their state is next captured by a snapshot, but not the intended final protocol state after those effects have been handled. Its transition code already models effect dependencies explicitly by entering a waiting state and emitting the next dependent effect only from the preceding resume. Independent effects may finish in any order, which is already part of the `Message::Resume` contract.

Sentinel needs one additional correctness change. Its `Effect::DynamicCheck` currently relies on inline execution and deliberately records no state while the check is outstanding. Once all effects are asynchronous, the oracle's onchain `NewRequest` event can arrive before the dynamic check completes and would be silently ignored. The epic therefore introduces a `WaitingForDynamicCheck` state whose optional request data records whether `NewRequest` has already arrived, so either ordering reaches the same vote state while the FSM remains explicit that the dynamic check is still outstanding.

The work lands as three independently mergeable PRs that keep `main` compiling, tested and behaviorally safe after each merge. First add the unused effect-management abstraction while execution remains inline; then make Sentinel's FSM correct for either resume/event ordering while inline execution still preserves today's ordering; finally switch the state machine and driver to asynchronous execution. The concurrency change is therefore activated only after every consumer is ready for it.

---

## Architecture Decision

**Keep one effect model.** `StateTransition` retains its existing `Effect` and `Resume` associated types, `Command` retains `Command::Effect`, and `Message` retains `Message::Resume`. There is no `BackgroundEffect`, `BackgroundResume`, `Command::Background`, `Message::BackgroundResume`, no second handler trait, and no no-op background component. Whether an effect usually resolves immediately or performs network or database work has no bearing on how it is dispatched.

**Lift effect execution out of `StateMachine` into a dedicated effects abstraction coordinated by `Driver`.** `StateMachine<S, T>` owns only the transition, reorg-aware state and snapshots. It no longer owns an effect handler and no longer recursively executes effect commands. Both indexer updates and effect resumes return `Commands<S, T>` to the driver. The driver dispatches each returned command: actions go to the transaction queue and effects go to its `EffectManager`.

**Introduce `crates/core/src/effects.rs`.** This module owns the complete effect-execution abstraction: the handler trait and an `EffectManager` that encapsulates handler sharing, task spawning, the `JoinSet`, task completion and task failures. The handler API becomes shareable across concurrent tasks:

```rust
pub trait EffectHandler<Effect, Resume>: Send + Sync + 'static {
    fn perform_effect(&self, effect: Effect) -> impl Future<Output = Resume> + Send;
}
```

Conceptually, the manager has this shape (exact generic ordering and method names remain implementation details):

```rust
pub struct EffectManager<Handler, Effect, Resume> {
    handler: Arc<Handler>,
    tasks: JoinSet<Resume>,
    // Effect may require a PhantomData marker.
}

impl<Handler, Effect, Resume> EffectManager<Handler, Effect, Resume>
where
    Handler: EffectHandler<Effect, Resume>,
    Effect: Send + 'static,
    Resume: Send + 'static,
{
    pub fn new(handler: Handler) -> Self;
    pub fn spawn(&mut self, effect: Effect);
    pub async fn next(&mut self) -> Resume;
}
```

For each effect, `EffectManager::spawn` creates a task that owns an `Arc` clone and awaits `perform_effect(effect)`. The future returned directly by `perform_effect` does not itself need to be `'static`; the spawned outer future owns the `Arc` for the duration of the borrow. `EffectManager::next` yields the next successful resume, logs and skips `JoinError`s, and remains pending when the task set is empty rather than repeatedly yielding `None`. Dropping the manager drops the `JoinSet` and aborts outstanding work.

**The driver has one input loop.** `Driver::step` uses `tokio::select!` over:

- shutdown;
- `self.watcher.next()` for the next indexer update; and
- `self.effects.next()` for the next completed effect.

The manager hides the empty-set and join-error behavior from the driver, so this select arm is always safe to poll. Shutdown retains priority. A successful watcher update follows the existing block-status reconciliation, state update, snapshot pruning and action-queueing path. A completed effect calls `StateMachine::handle_resume`; commands produced by either path go through the same driver dispatch helper, so a resume can emit more actions and effects. A panicked or cancelled effect task is logged and discarded by the manager without poisoning the driver; ordinary effect failures remain values encoded in the service's `Resume` type.

**Resume transitions are not committed.** `StateMachine::handle_resume(resume)` applies `Message::Resume(resume)` to the current in-memory state and returns its commands, but does not write a snapshot. Snapshot commits remain tied exclusively to successfully processed `Update::Logs` ranges. The next log-range commit captures any resume transitions that have occurred since the previous commit. A crash or shutdown before that commit can therefore lose both an outstanding effect and an uncommitted resume, as described under Assumptions.

**State transitions remain serialized even though effect work is concurrent.** Only the driver mutates the state machine, one selected input at a time. The manager's task set permits effect futures to overlap, but their results enter the transition function individually. State transitions must not rely on completion order and must ignore stale resumes, including a task emitted before a reorg that completes afterward.

### Alternatives Considered

- **Keep immediate effects and add a background effect class.** Rejected: duration is not a useful semantic distinction, every transition must already tolerate replayed and unordered resumes, and two command/resume pairs plus two handler traits add ongoing API and implementation complexity without changing the validator's intended final state.
- **Keep effect execution in `StateMachine`.** Rejected: it mixes pure transition/snapshot responsibilities with task orchestration and forces the driver to reach into the state machine to poll completed work. The driver already owns the event loop and is the natural place to select indexer, effect and shutdown inputs.
- **Store the handler and `JoinSet` directly on `Driver`.** Rejected: the driver should orchestrate service inputs, not implement task-spawning, shared-handler, empty-set and join-error semantics itself. `EffectManager` gives those details one reusable, independently testable home.
- **Run one effect at a time without blocking indexer updates.** Rejected: a single pending future avoids blocking the event loop but still serializes unrelated requests. A `JoinSet` lets independent effects make progress concurrently and directly supplies the next completed resume.
- **Use a completion channel inside `EffectManager` instead of `JoinSet`.** Rejected: it requires separate task lifecycle tracking and channel closure handling. `JoinSet` owns the spawned tasks, exposes completion directly, and aborts remaining work when the manager is dropped.
- **Persist outstanding effects for exactly-once redelivery.** Rejected for this epic: handlers and transitions already need replay-safe semantics, and protocol states use block deadlines to recover when an expected resume never arrives. Durable effect orchestration would be a separate design with its own replay and reorg rules.
- **Add a validator dynamic-check consumer.** Rejected: the validator's direct signing decision remains deterministic and local. This epic changes how its existing secret-store effects are scheduled; it does not add a new effect.

---

## Tech Specs

### Phase 1 — Core: introduce the effect manager without using it

- `crates/core/src/effects.rs` (new):
  - Move `EffectHandler` and the `Pure` no-effect handler out of the state module. Update `EffectHandler` to the shared `&self`/`Send + Sync + 'static` shape above.
  - Add `EffectManager`, owning `Arc<Handler>` and `JoinSet<Resume>` and exposing construction, effect submission and next-resume methods.
  - Keep task construction, required `Send + 'static` bounds, empty-set pending behavior and `JoinError` logging inside the manager.
  - Add focused tests for concurrent/out-of-order completion, an empty manager remaining pending, handler sharing, chained submissions and panicking tasks being skipped without losing later successful resumes.
- `crates/core/src/lib.rs`: export the new `effects` module.
- Update `crates/core/src/state/mod.rs` and all other call sites to import `EffectHandler`/`Pure` from `core::effects`; do not re-export them from their old state-module path. `StateMachine` still owns the handler and `TransitionBatch` still awaits every effect inline in this phase.
- `crates/validator/src/service/effect.rs` and `crates/sentinel/src/effect.rs`:
  - Update handler implementations to take `&self`. Validator's `SecretStore` methods already take `&self` and use database constraints/transactions for replay-safe mutation; Sentinel's checker trait is already `Send + Sync`.
  - Move imports to `safenet_core::effects::EffectHandler` and update handler unit tests for the revised receiver.
- `Driver`, `StateMachine`, command dispatch and snapshot semantics are otherwise untouched. `EffectManager` is production-ready but deliberately unused.
- Merge gate: existing core, validator and Sentinel tests pass with the same observable inline effect behavior, plus the new `EffectManager` unit tests.

### Phase 2 — Sentinel: model the async-effect waiting states while effects are still inline

- `crates/sentinel/src/state.rs`'s `SentinelRequestState` gains:
  - A serializable `Request { bond_target: U256, commit_deadline: u64, reveal_deadline: u64 }` value containing the onchain data needed to construct the vote after `NewRequest` arrives.
  - `WaitingForDynamicCheck { deadline: u64, request: Option<Request> }` — the proposal passed `StaticChecker` and its dynamic check was dispatched. `request: None` means `NewRequest` has not arrived; `Some(request)` retains its onchain voting data while making clear that the dynamic check is still the unresolved condition.
- `crates/sentinel/src/effect.rs` removes `deadline` from `Effect::DynamicCheck` and `Resume::DynamicCheckResult`; the deadline now lives in the request FSM instead of being carried through impure work solely to avoid tracking intermediate state.
- `handle_oracle_transaction_proposed` inserts `WaitingForDynamicCheck { deadline, request: None }` before returning `Command::Effect(Effect::DynamicCheck { .. })`. The command uses the same universal effect path as every validator and Sentinel effect.
- Extract `handle_new_request`'s vote construction into a shared helper, for example `commit_vote(request_id, approve, reason, bond_target, commit_deadline, reveal_deadline)`, which produces `CollectingCommitments` plus the `ApproveToken`/`Commit` actions.
- `handle_new_request` handles both valid orderings:
  - `WaitingForRequest { approve, reason, .. }` means the check completed first, so it calls `commit_vote` immediately.
  - `WaitingForDynamicCheck { request: None, .. }` means the dynamic check is still outstanding, so it stores `Some(Request { .. })`, remains in `WaitingForDynamicCheck`, and emits no action yet.
  - Unknown, already-advanced, or already-populated request IDs remain no-ops.
- `handle_dynamic_check_result` matches the currently tracked state:
  - `WaitingForDynamicCheck { deadline, request: None }` plus `Approved`/`Denied(rule)` becomes `WaitingForRequest { approve, reason, deadline }`.
  - `WaitingForDynamicCheck { request: Some(request), .. }` plus `Approved`/`Denied(rule)` calls `commit_vote` immediately with the stored request fields.
  - `Unknown` drops the waiting request unanswered and logs a warning, matching today's failure outcome.
  - An unknown, expired or already-advanced request is a stale resume and remains a logged no-op.
- `handle_block_advance` expires `WaitingForDynamicCheck { request: None, .. }` at its guessed deadline and `WaitingForDynamicCheck { request: Some(request), .. }` after `request.commit_deadline`.
- Extend state serde tests for `WaitingForDynamicCheck` with both `None` and `Some(Request)`. Flow tests cover approve and deny outcomes with `NewRequest` both before and after the dynamic-check resume, failures with and without stored request data, timeout cleanup, and stale resumes after advancement, expiry or reorg.
- Effect execution remains inline in this phase. In production, `WaitingForDynamicCheck { request: None, .. }` is inserted and immediately consumed by the effect resume before the next indexer event, preserving today's behavior; the `Some(Request)` path is exercised through direct transition tests and becomes reachable from the live driver only in Phase 3.
- Merge gate: the full Sentinel lifecycle remains unchanged under inline execution, while transition tests prove the state machine is already safe for both orderings that asynchronous execution will enable.

### Phase 3 — Core: hand effect execution from the state machine to the driver

- `crates/core/src/state/mod.rs`:
  - Remove the effect-handler type parameter and field from `StateMachine`; it becomes `StateMachine<S, T>`.
  - Remove effect execution from `TransitionBatch`. Applying a message is synchronous apart from the surrounding snapshot operations, advances the state once, and appends all emitted `Command`s without recursively resolving `Command::Effect`.
  - Change `handle_update` to return `Commands<S, T>` rather than only `Vec<T::Action>`. Its reorg validation, update ordering and `Update::Logs` snapshot behavior remain unchanged.
  - Add `handle_resume(resume) -> Result<Commands<S, T>, Error>`. It takes the current `(state, status)`, applies `Message::Resume(resume)`, stores the resulting in-memory state with the same status, and deliberately does not call `SnapshotStore::commit`.
- `crates/core/src/driver.rs`:
  - Keep the existing `Service::Effects` associated type and three-part `components()` tuple. Do not add a background-associated type or component.
  - `Driver` stores `StateMachine<S::State, S::Transition>` and an `EffectManager` parameterized by the service's handler, effect and resume types; it does not directly own an `Arc` or `JoinSet` for effects.
  - `Driver::new` constructs the state machine without a handler and constructs the already-tested effect manager from `S::Effects`.
  - Add a single command-dispatch helper used after watcher updates and resumes. It encodes and queues every `Command::Action` and passes every `Command::Effect` to `EffectManager::spawn`; it never awaits an effect result.
  - Restructure `Driver::step` around the three-way select described in Architecture Decision. Preserve the existing watcher retry and transaction-queue intermittent-error policies.
  - Select `EffectManager::next` without reaching into its task set or handling `JoinError` directly.
- Core state tests cover that effects are returned without being run, multiple log messages still transition in order and accumulate commands, a resume can emit both an action and another effect, and `handle_resume` changes live state without changing the committed snapshot.
- Audit every validator `Message::Resume` arm for stale or out-of-order completion. Existing waiting-state matches should remain the ordering mechanism; add focused regression tests wherever an unexpected resume is not already a no-op.
- Driver-level tests prove that a pending effect does not block later watcher updates, commands emitted by resumes are dispatched through the manager, and shutdown wins with tasks in flight. Concurrency, empty-set and task-failure mechanics remain covered directly by the Phase 1 `EffectManager` tests.
- Merge gate: all core, validator, Sentinel and relevant integration tests pass with universally asynchronous effect execution. This is the only phase that changes live scheduling behavior.

### Phase 4 — Remove this plan

Delete `epics/2026_07_24_nonblocking_effects.md` once Phases 1–3 are merged.

---

## Implementation Phases

Each phase is an independently mergeable PR. Every row must leave `main` compiling and all relevant tests passing; later phases build only on behavior and APIs already merged by earlier phases.

| Phase | Summary                                                                                                                 | Depends on | Own PR |
| ----- | ----------------------------------------------------------------------------------------------------------------------- | ---------- | ------ |
| 1     | Add and test `effects::EffectManager`; relocate the handler abstraction while retaining inline execution                | —          | ✅     |
| 2     | Add Sentinel's `WaitingForDynamicCheck { request: Option<Request>, .. }` FSM and race tests while effects remain inline | 1          | ✅     |
| 3     | Make `StateMachine` return effects and have `Driver` execute them through `EffectManager`                               | 1, 2       | ✅     |
| 4     | Remove this plan                                                                                                        | 3          | ✅     |

---

## Assumptions

- **There is no immediate/background distinction.** Every `Command::Effect`, including validator secret-store work and Sentinel's dynamic check, is submitted through the same `EffectManager` path.
- **Only effect work is concurrent.** The driver continues to feed one watcher update or completed resume at a time into the state machine, so transition execution itself stays serialized.
- **Resume ordering is unspecified.** Causal dependencies must be expressed by emitting a later effect from an earlier effect's resume transition, not by relying on task insertion order. Independent effects and stale post-reorg tasks may resume in any order.
- **Outstanding effects and uncommitted resumes are not durable.** A process exit drops the `EffectManager` and aborts its task set; restart begins from the latest log snapshot and does not recreate tasks that were outstanding there. Existing deadline sweeps are the recovery mechanism. Durable redelivery is out of scope.
- **No concurrency limit is introduced here.** `EffectManager` may contain one task per emitted effect. Backpressure can be added inside the manager later without changing the driver or adding a second effect type.
- **Sentinel's local `StaticChecker` denial remains synchronous transition logic.** It is not a `Command::Effect`; only the impure checker chain dispatched as `Effect::DynamicCheck` moves through the manager's task set.
- **Exact helper names, generic ordering and field ordering remain implementation details.** The ownership boundary, one effect model, `EffectManager`-encapsulated `JoinSet` scheduling, non-committing resume semantics and Sentinel's optional request data while waiting for the dynamic check are the required design.
