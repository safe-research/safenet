# Plan: Non-Blocking Effects

Component: `crates/core` (state machine, driver), `crates/sentinel` (Rust).

---

## Overview

Every effect a [`StateTransition`](../crates/core/src/state/mod.rs) emits today runs fully in-band and blocking: `TransitionBatch::apply` (`crates/core/src/state/mod.rs:313-339`) `await`s `EffectHandler::perform_effect` directly, and the whole call chain — from `StateMachine::handle_update` down — runs under `&mut self`/a single `Mutex` guard held for the duration (`mod.rs:196-286`). A slow effect stalls everything: no other log in the current batch, no later block, nothing.

This is not hypothetical — it already bit `crates/sentinel`. Its recently-shipped `Effect::DynamicCheck` (`crates/sentinel/src/effect.rs`) resolves via either an `eth_getLogs` RPC call (`address_poisoning.rs`) or an operator-configured HTTPS POST (`dynamic_checker.rs`), either of which can legitimately take hundreds of milliseconds to seconds — and for that whole window the sentinel cannot advance on anything else, across every request it's tracking. `crates/validator`'s own direct FROST-signing path stays out of this epic entirely: the validator's consensus path is meant to stay 100% deterministic and local (`safe_tx::checks::check_transaction`); statistical/time-varying judgment belongs in the Sentinel. Validator therefore only needs to *opt into* the new mechanism with a no-op background handler — it has no dynamic check of its own to run, now or as a goal of this epic.

This epic adds a second class of effect — **background effects** — that a `StateTransition` can emit to run concurrently with, and without blocking, the transition loop, and migrates Sentinel's existing `Effect::DynamicCheck` onto it. Migrating it is not just a mechanical swap: today's implementation deliberately tracks no state at all while the check is outstanding (`crates/sentinel/src/service.rs:107-109`, `156-174`), relying on the fact that, under full blocking, nothing else can ever race a request's own resume. That assumption becomes false the moment effects run concurrently — the oracle's onchain `NewRequest` event (opening the request for voting) can legitimately land *before* the dynamic check resolves, and today's `handle_new_request` silently ignores a request id it doesn't recognize (`service.rs:228-233`). Fixing this is the epic's second deliverable: explicit `WaitingForDynamicCheck` and `PreparingToVote` states, so a proposal's dynamic check and the oracle's own request-opening event — whichever arrives first — both get folded into the eventual commit correctly, regardless of order.

Two things have to land together for this to work, since one is useless without the other:
- **The mechanism**: a way to run an effect off the transition loop, and a way for its result to come back in — "fetched by the driver," per the motivating request — without the driver polling or the state machine blocking.
- **A real consumer**: Sentinel's own request FSM, fixed to track the race explicitly now that blocking can no longer paper over it.

---

## Architecture Decision

**Add a second effect class, not a flag on the existing one.** `StateTransition` gains two new associated types, `BackgroundEffect`/`BackgroundResume`, alongside the existing `Effect`/`Resume`. `Command<Action, Effect, BackgroundEffect>` gains a third variant, `Command::Background(BackgroundEffect)`. `Message<Event, Resume, BackgroundResume>` gains `Message::BackgroundResume(BackgroundResume)` alongside `Message::Resume(Resume)`. The existing `Command::Effect`/`Message::Resume` pair, and the existing `EffectHandler` trait (`&mut self`, awaited inline), are **untouched** — every existing blocking effect in `crates/validator` (`TopupNonces`, `RevealNonceCommitments`, `UseNonce`, ...) keeps running exactly as it does today: those effects mutate a shared local secret store and must stay strictly ordered (`UseNonce` after `TopupNonces`, never interleaved), so they are effects that need to block, not ones that merely happen to today. Reusing one enum for both blocking and non-blocking commands was considered and rejected (see Alternatives) precisely because it would force the same handler to also answer for variants it must never run out of order.

**A new `BackgroundEffectHandler` trait takes `&self`, not `&mut self`, and requires `Send + Sync + 'static`:**
```rust
pub trait BackgroundEffectHandler<Effect, Resume>: Send + Sync + 'static {
    fn perform_effect(&self, effect: Effect) -> impl Future<Output = Resume> + Send + 'static;
}
```
This is the property that actually buys concurrency: `EffectHandler::perform_effect(&mut self, ..)` can only ever have one call in flight (Rust's aliasing rules forbid two live `&mut` borrows into the same handler, regardless of the async runtime), so it fundamentally cannot run two effects at once even if we stopped awaiting it inline. A `&self` handler wrapped in `Arc` can: `StateMachine` holds `background: Arc<B>`, and each `Command::Background(effect)` spawns `tokio::spawn({ let background = background.clone(); async move { background.perform_effect(effect).await } })` onto an internal `tokio::task::JoinSet<T::BackgroundResume>`, so any number of dynamic checks for distinct in-flight requests run in parallel rather than queuing behind each other. `crates/sentinel`'s existing `effect::Handler` (`RemoteChecker` + `AddressPoisoningChecker`) already holds nothing but a cloneable HTTP client and an RPC `Provider` — no mutable state that ever required `&mut self` in the first place, so this is a natural fit, not a workaround.

**The driver polls a `JoinSet`, not a channel it has to remember to drain.** `StateMachine` exposes `next_background_resume(&mut self) -> T::BackgroundResume`, implemented as a loop over `self.background_tasks.join_next().await` that falls through to `std::future::pending()` when the set is empty — so it composes directly as a `tokio::select!` arm in `Driver::step` (`crates/core/src/driver.rs:187-215`) alongside the existing shutdown/watcher-update arms, and simply never wins when nothing is outstanding. `handle_background_resume` runs the resumed `Message::BackgroundResume` through the same `TransitionBatch` machinery as any other message and returns the actions it produces, to be queued exactly like `handle_update`'s.

**A background resume that never lands (crash, restart) is accepted as a lost effect, not persisted for exactly-once redelivery.** Applying a background resume updates the in-memory `(state, status)` but does **not** force an immediate snapshot commit — that stays tied to `Update::Logs` processing as it is today (`mod.rs:275`). If the process crashes before the next natural commit, the resume (and, if its own emitting block/log batch was also never committed, the emission itself) is simply gone on restart. This is not treated as a new correctness gap: every state this epic introduces carries a deadline and is swept by the existing per-block timeout mechanism (`handle_block_advance`'s `retain`, `crates/sentinel/src/service.rs:349-420`) that already drops a request whose expected next step never arrives, for any reason. Building persistence for outstanding background effects (a durable "requests in flight" table, replayed on restart) was considered and rejected as unjustified complexity given that safety net already exists (see Alternatives).

### Alternatives Considered

- **Reuse the existing `Effect`/`Resume` enum for background commands, distinguished only by which `Command` variant wraps them.** Rejected: `EffectHandler::perform_effect`'s match would need to stay exhaustive over variants it must never actually receive (since those are always emitted via `Command::Background`), forcing either dead `unreachable!()` arms or weakening the type-level guarantee that a mutating, must-stay-ordered effect (e.g. `UseNonce`) can never accidentally be dispatched through the concurrent path. A wholly separate `BackgroundEffect`/`BackgroundResume` pair per service keeps that impossible by construction.
- **Model "non-blocking" as bounding to one background effect in flight at a time (a single `Option<Pin<Box<dyn Future>>>` slot on `StateMachine`, still built on `&mut self`).** Would unblock the driver's own loop from a single slow effect, but two requests proposed close together would still serialize their dynamic checks behind each other — exactly the kind of stall this epic exists to remove, and exactly the failure mode already observed in Sentinel. The `&self` + `Arc` + `JoinSet` design costs little extra and removes the limitation outright.
- **Persist outstanding background effects (a "requests in flight" table) for exactly-once redelivery across restarts.** Rejected for this epic: every consumer state already has a deadline and an existing timeout sweep that tolerates "the expected response never came," for any reason. Layering durable redelivery on top would solve a problem the state machine already has a general answer for, at real implementation cost (a new snapshot-adjacent store, replay-ordering rules) for a narrower class of failure (process crash) than what the sweep already covers (crash, slow network, unreachable endpoint, buggy handler...).
- **Give `crates/validator` a dynamic-check consumer of its own as part of this epic.** Rejected: the validator's direct-signing path is deliberately kept fully deterministic and local (see Overview); inventing a dynamic check for it here would be scope in search of a justification, not something the codebase or the motivating request calls for. Validator's only change in this epic is opting into the new `Service::Background` associated type with a no-op handler, so the driver-level plumbing (Phase 2) has more than one real implementor to compile against.
- **Leave Sentinel's `Effect::DynamicCheck` on the new mechanism's day-one behavior (spawn it in the background, but keep today's "track nothing while outstanding" state shape).** Rejected: that shape's safety depended entirely on effects being fully sequential (documented explicitly at `service.rs:156-174`); moving it to run concurrently without also adding `WaitingForDynamicCheck`/`PreparingToVote` would silently drop any `NewRequest` that lands before its check resolves — a real regression, not a hypothetical one. The migration and the FSM fix are not separable into an "inert first step" the way `crates/sentinel`'s original reference epic could split its Phase 5a/5b (that mechanism was net-new there; here, an already-shipped mechanism's safety invariant is what's changing), so this epic's Phase 3 lands them together.

---

## Tech Specs

### Phase 1 — Core: background effect mechanism (infra only, no consumers)

- `crates/core/src/state/mod.rs`:
  - `StateTransition<S>` gains `type BackgroundEffect;` / `type BackgroundResume;`.
  - `Command<Action, Effect, BackgroundEffect>` gains `Background(BackgroundEffect)`; `Commands<S, T>`'s alias picks up the extra parameter automatically from the trait's associated types.
  - `Message<Event, Resume, BackgroundResume>` gains `BackgroundResume(BackgroundResume)`.
  - New `BackgroundEffectHandler<Effect, Resume>` trait (per Architecture Decision).
  - `StateMachine<S, T, E, B = NoBackgroundEffects>` gains a fourth type parameter, `background: Arc<B>` and `background_tasks: tokio::task::JoinSet<T::BackgroundResume>` fields; `new`/`with_init` take a `background: B` argument.
  - `TransitionBatch` (or its constructor) gets access to `background`/`background_tasks` so a `Command::Background(effect)` encountered mid-batch spawns onto the `JoinSet` instead of awaiting inline; the batch itself does not wait on it.
  - `StateMachine::next_background_resume(&mut self) -> T::BackgroundResume` and `handle_background_resume(&mut self, resume: T::BackgroundResume) -> Result<Vec<T::Action>, Error>`, per Architecture Decision (no snapshot commit forced by the latter).
  - A `NoBackgroundEffects` marker type (sibling to the existing `Pure` `EffectHandler` marker), implementing `BackgroundEffectHandler<Infallible, Infallible>`, for services that don't emit background effects.
- Unit tests (extending `mod.rs`'s existing `#[cfg(test)]` module, same style as its current block/reorg/warp tests): a background effect spawned from one message does not block the next message in the same batch; `next_background_resume`/`handle_background_resume` correctly feed a resume back through `apply_transition`; multiple concurrently-spawned background effects can resolve out of order; a panicking background task is logged and does not take down the state machine.
- No behavior change for any existing service — nothing yet constructs a real `BackgroundEffectHandler` other than `NoBackgroundEffects`.

### Phase 2 — Core: wire background resumes into the driver

- `crates/core/src/driver.rs`:
  - `Service` trait gains `type Background: BackgroundEffectHandler<Effect, Resume>` (named consistently with `Effects`/`Actions`); `components()` returns the extra component.
  - `Driver::step` (`driver.rs:187-215`) restructured so its `tokio::select!` races a third arm, `self.state.next_background_resume()`, alongside `shutdown` and `self.watcher.next()`; when it wins, `handle_background_resume` runs and its actions are queued exactly like `handle_update`'s, mirroring the existing `Update::Block` housekeeping-then-transition-then-queue shape.
- `crates/validator/src/main.rs` and `crates/sentinel/src/main.rs`: both `Service` impls add `type Background = NoBackgroundEffects;` and pass `NoBackgroundEffects` into `components()` — mechanical, zero behavior change. (Validator stops here for this epic; see Alternatives for why it gets no dynamic-check consumer of its own.)
- Acceptance bar: every existing validator/sentinel/core test passes unmodified. This phase adds no new observable behavior; it only proves the plumbing compiles and runs end-to-end with a no-op background handler on both services before Phase 3 gives Sentinel's something real to do.

### Phase 3 — Sentinel: migrate `Effect::DynamicCheck` to a background effect, fix the request race

Lands as one PR (see Alternatives for why this isn't split into an inert-plumbing-first step the way the mechanism itself was in Phases 1-2).

- `crates/sentinel/src/effect.rs`: `Effect`/`Resume`/`Handler` are unchanged in shape, but `Handler` now implements `BackgroundEffectHandler<Effect, Resume>` instead of `EffectHandler`. `crates/sentinel/src/main.rs`'s `Service` impl swaps `type Effects = Pure` (sentinel now has no blocking effect at all) and `type Background = effect::Handler`.
- `crates/sentinel/src/state.rs`'s `SentinelRequestState` gains two variants:
  - `WaitingForDynamicCheck { deadline: u64 }` — the proposal passed `StaticChecker`; the dynamic check was handed off to run in the background; the request hasn't opened onchain yet. `deadline` is the same "our own guessed cutoff" the existing `WaitingForRequest` doc comment describes.
  - `PreparingToVote { bond_target: U256, commit_deadline: u64, reveal_deadline: u64 }` — the oracle's `NewRequest` opened the request onchain while the dynamic check was still outstanding; `bond_target` and the real onchain deadlines are captured from that event so the eventual commit can be built the moment the check resolves, without needing to re-derive or re-fetch anything.
- `handle_oracle_transaction_proposed` (`service.rs:110-154`): on passing `StaticChecker`, now inserts `RequestState::WaitingForDynamicCheck { deadline }` *before* emitting `Command::Background(effect::Effect::DynamicCheck{..})` (replacing today's `Command::Effect` and its deliberate "track nothing" gap).
- `handle_new_request` (`service.rs:223-280`): its commit-building logic (salt, `commit_hash`, building `CollectingCommitments` plus the `ApproveToken`/`Commit` actions) is extracted into a shared helper, e.g. `commit_vote(&self, request_id, approve, reason, bond_target, commit_deadline, reveal_deadline) -> (RequestState, Commands<State, Self>)`, since Phase 3 needs to reach it from two call sites. Match on the tracked state broadens:
  - `WaitingForRequest { approve, reason, .. }` (check already resolved) → unchanged: calls `commit_vote` immediately, as today.
  - `WaitingForDynamicCheck { .. }` (check still outstanding) → insert `PreparingToVote { bond_target: event.bondTarget, commit_deadline, reveal_deadline }`; no actions yet.
  - Anything else (unknown request id, or one already past this point) → no-op, as today.
- `handle_dynamic_check_result` (`service.rs:175-219`): the "already tracked ⇒ no-op" guard (today's stand-in for "nothing else could have raced this," per the doc comment being replaced) is replaced with an explicit match on the tracked state, mirroring the replay-safety idiom `crates/validator/src/state/sign.rs` already uses for its own effect resumes:
  - `WaitingForDynamicCheck { deadline }`, `Approved`/`Denied(rule)` → insert `WaitingForRequest { approve, reason, deadline }`, exactly as today (still waiting on `NewRequest`).
  - `PreparingToVote { bond_target, commit_deadline, reveal_deadline }`, `Approved`/`Denied(rule)` → call `commit_vote` immediately with the stored fields — this is "submit the vote" from the motivating request, reached the moment the check resolves rather than waiting for another event.
  - `Failed`, in either waiting state → drop the request unanswered (as today), logged at `warn`; a live onchain request whose bond was never committed against is the same accepted outcome as a request that never got this far at all.
  - Anything else (already advanced past either waiting state, or genuinely unknown) → no-op, logged at `warn` — a stale/replayed resume that lost a race or arrived late.
- `handle_block_advance`'s `retain` (`service.rs:349-420`) gains two arms: `WaitingForDynamicCheck { deadline }` drops past `deadline` (mirrors `WaitingForRequest`'s existing arm); `PreparingToVote { commit_deadline, .. }` drops once `block > commit_deadline` (mirrors `CollectingCommitments`'s first check — if the dynamic check hasn't resolved by the real onchain commit deadline, there is no reveal to fall back to, since no commit was ever built).
- `state.rs`'s serde-roundtrip test extended to cover both new variants.
- Flow tests (extending the existing whole-lifecycle style in `service.rs`'s `#[cfg(test)]` module, `service.rs:674+`) proving the race both ways: `NewRequest` lands before the dynamic check resolves, and after; approve and deny outcomes in each ordering; a stale/replayed `Message::BackgroundResume` against an already-advanced or already-dropped request id is a no-op.

### Phase 4 — Remove this plan

Delete `epics/2026_07_24_nonblocking_effects.md` once Phases 1–3 are merged.

---

## Implementation Phases

| Phase | Summary | Depends on | Own PR |
|---|---|---|---|
| 1 | Core: `Command::Background`/`Message::BackgroundResume`, `BackgroundEffectHandler` trait, `StateMachine` spawn/`JoinSet`/resume-fetch plumbing | — | ✅ |
| 2 | Core: wire background resumes into `Driver::step`; `Service` trait gains `Background`; validator + sentinel opt in with a no-op handler (zero behavior change) | 1 | ✅ |
| 3 | Sentinel: migrate `Effect::DynamicCheck` to `BackgroundEffectHandler`; add `WaitingForDynamicCheck`/`PreparingToVote`; fix the `NewRequest`-vs-check race in `handle_new_request`/`handle_dynamic_check_result`; deadline-sweep arms | 2 | ✅ |
| 4 | Remove this plan | 3 | ✅ |

---

## Assumptions

- **`crates/validator` gets no dynamic-check consumer of its own in this epic.** Its direct-signing path is deliberately kept fully deterministic per the Sentinel reference epic's own architecture decision; it only needs to compile against the new `Service::Background` associated type (a no-op) so Phase 2 has more than one real implementor. Introducing a validator-side dynamic check is a different, unrelated epic if one is ever wanted.
- **No persistence for outstanding background effects across restarts.** A crash loses any background effect that hasn't resumed and been committed; this is accepted as equivalent to "the expected response never arrived," already handled by each consumer state's existing deadline/timeout sweep. See Architecture Decision.
- **`handle_oracle_transaction_proposed`'s local `StaticChecker` denial path is unchanged** — it still decides immediately and never touches the background mechanism, since a local, deterministic denial needs no deferral.
- **Exact Rust shapes (the `commit_vote` helper's exact signature, whatever field ordering reads best) are intentionally left loose**, to be nailed down during implementation/PR review rather than gated on this planning doc — consistent with how the Sentinel reference epic scoped its own spec.
