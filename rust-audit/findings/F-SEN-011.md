# F-SEN-011 A restart orphans any in-flight engine check whose proposal is older than the rollback anchor: the request is never re-checked and silently expires without a vote

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | sentinel, state.rs / effect.rs / service.rs (with core: effects.rs, index/blocks.rs) |
| Location | crates/sentinel/src/state.rs:25-32, crates/sentinel/src/effect.rs:17-26, crates/sentinel/src/service.rs:393-399 (related: crates/core/src/effects.rs:28-36, crates/core/src/index/blocks.rs:255-278) |
| Severity | Low / Low |
| Certainty | 82% (set by Critic C-SEN; QA may raise) |
| Assumptions involved | A5, A10 |
| Tags | crash-consistency, reorg |

## Claim

`Effect::EngineCheck` carries the whole proposed `SafeTransaction` (`effect.rs:17-26`), but the persisted `WaitingForEngineCheck` state deliberately does not (`state.rs:25-32` holds only `deadline` and the optional onchain `Request` terms). In-flight effects live only in the `EffectManager`'s `JoinSet` and are aborted when it is dropped (`core/effects.rs:28-36`), so a restart destroys every outstanding check.

The only way a check is re-created is by re-applying the originating `TransactionProposed` log, which happens only for blocks inside the replayed range — from `indexed.safe + 1` upward (`core/index/blocks.rs:255-278`). A `WaitingForEngineCheck` entry restored from the anchor snapshot whose proposal was at or before `indexed.safe` therefore has **no effect in flight and no way to create one**: the FSM has forgotten the transaction bytes the engine needs. The entry then sits idle until `handle_block_advance` retires it at `commit_deadline` (or the local `deadline`) and drops it (`service.rs:393-399`), with no log line and no metric distinguishing it from an ordinary expiry.

Impact is missed participation only — no bond is at risk, because no commit was ever made. But it is systematic: on a busy oracle, every restart abandons whichever requests happened to be mid-check and older than the anchor, and the operator has no signal that it happened.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The effect needs the full transaction | E2 | crates/sentinel/src/effect.rs:16-26 | `#[derive(Debug, Clone, PartialEq, Eq)]`<br>`pub enum Effect {`<br>`    /// Defer the approve/deny decision for \`request_id\` (a proposed`<br>` /// \`transaction\` on \`safe\`) to the configured sentinel engine. \`block\` is`<br>` /// the block number the sentinel considers current.`<br>` EngineCheck {`<br>` request_id: B256,`<br>` transaction: SafeTransaction,`<br>` block: u64,`<br>` },`<br>`}` |
| 2 | The persisted state does not retain the transaction | E2 | crates/sentinel/src/state.rs:25-32 | `pub enum SentinelRequestState {`<br>`    /// The proposal is waiting for its sentinel engine check to complete. If`<br>`    /// the request opens onchain first, its voting data is retained in`<br>`    /// \`request\` until the check resumes.`<br>` WaitingForEngineCheck {`<br>` deadline: u64,`<br>` request: Option<Request>,`<br>` },` |
| 3 | In-flight effects are aborted when the manager is dropped, i.e. at process exit | E2 | crates/core/src/effects.rs:28-36 | `/// Executes effects concurrently and yields their resumes as they complete.`<br>`///`<br>`/// The manager owns all spawned effect tasks. Dropping it aborts any effects`<br>`/// that are still in progress.`<br>`pub struct EffectManager<Handler, Effect, Resume> {`<br>`    handler: Arc<Handler>,`<br>`    tasks: JoinSet<Resume>,`<br>`    effect: PhantomData<fn(Effect)>,`<br>`}` |
| 4 | A check is only ever created from a `TransactionProposed` event | E2 | crates/sentinel/src/service.rs:137-144 | `        (`<br>`            state,`<br>`            vec![Command::Effect(effect::Effect::EngineCheck {`<br>`                request_id,`<br>`                transaction: event.transaction,`<br>`                block,`<br>`            })],`<br>`        )` |
| 5 | Replay starts at the earliest retained snapshot, so older logs are never re-delivered | E2 | crates/core/src/index/blocks.rs:256-266 | `            // The earliest retained snapshot is the rollback anchor. Replay`<br>`            // everything after it, but only emit an uncle when there are newer`<br>`            // snapshots to discard. A pruned warp may retain only its latest`<br>`            // snapshot, in which case we can continue directly from the next`<br>`            // block without a synthetic reorg.`<br>`            let uncle = indexed.safe.checked_add(1);`<br>`            if let Some(uncle) = uncle`<br>`                && uncle <= indexed.latest`<br>`            {`<br>`                self.queue.push_back(BlockUpdate::Uncle { number: uncle });`<br>`            }` |
| 6 | The stranded entry is retired silently at its deadline | E2 | crates/sentinel/src/service.rs:390-399 | `    fn handle_block_advance(&self, mut state: State, block: u64) -> (State, Commands<State, Self>) {`<br>`        let mut actions = Vec::new;`<br>``<br>`        state.0.retain(\|id, entry\| match entry {`<br>`            RequestState::WaitingForEngineCheck { deadline, request } => {`<br>`                block`<br>`                    <= request`<br>`                        .as_ref`<br>`                        .map_or(*deadline, \|request\| request.commit_deadline)`<br>`            }` |

## Trigger

`max_reorg_depth = 5`, `COMMIT_WINDOW = 5`, 5 s blocks:

1. Block `b`: `TransactionProposed` + `NewRequest` for request R. The sentinel enters `WaitingForEngineCheck { request: Some(..), commit_deadline: b+5 }` and spawns the engine check. The snapshot at `b` records that state — without the transaction (basis 2).
2. Blocks `b+1 .. b+7`: the engine is slow (a legitimate case: the budget is deliberately three quarters of the voting window, `main.rs:45-62`) and the check is still running. Snapshots for `b+1 .. b+7` continue to record `WaitingForEngineCheck`.
3. Block `b+7`: the process restarts. The in-flight check is aborted (basis 3).
4. On startup `indexed.safe ≈ b+2` and `indexed.latest = b+7`, so `Uncle{b+3}` restores the snapshot at `b+2`, which still contains R in `WaitingForEngineCheck`.
5. Blocks `b+3 .. b+7` are replayed. R's `TransactionProposed` was at block `b`, outside the range, so no effect is created (basis 4, 5) and the FSM has no way to create one (basis 1, 2).
6. At `NewBlock(b+6)` the entry fails `block <= commit_deadline` and is dropped (basis 6). No vote, no log, no metric — `requests_proposed_total` was incremented back at step 1 and `requests_participated_total` simply never follows.

## Considered and rejected

- **"F-SEN-001 covers this."** It does not: F-SEN-001 is the case where the proposal _is_ inside the replayed range and the effect _is_ re-created (and the replayed `Committed` is then discarded, risking a slash). This finding is the complementary case where the proposal is outside the range, no effect exists, and the loss is participation rather than bond.
- **"The `deadline`/`commit_deadline` retirement is the intended behaviour."** Retiring an entry whose window has closed is correct; the defect is that the entry reached that point without the check ever being retried, which the FSM cannot even detect because `WaitingForEngineCheck` carries no "check outstanding" marker.
- **"An operator would see it in the engine's request log."** The engine sees fewer requests, not a request that stopped; and the sentinel's own `security_check` span (`engine.rs:126-130`) is only opened when a check is actually issued, so an abandoned one leaves no trace at all.
- **"Storing the transaction would be too large."** A `SafeTransaction` is bounded by the proposal's calldata, which is bounded by the block gas limit; snapshots already serialise the whole `State` as JSON per block (`core/state/mod.rs:236`), so persisting it would multiply snapshot size by the number of in-flight checks. This is a real tradeoff, which is why remediation option 2 (re-read the log) is offered as an alternative.
- **False positive check — could the resume from the aborted task still arrive?** No: the task is aborted with the `JoinSet` at process exit, and even in-process a stale resume for an entry that no longer matches is discarded (`service.rs:156-170`, tests at `service.rs:1622` and `1835`).

## Remediation options

1. **Persist what the effect needs.** Add `transaction: SafeTransaction` (or just its ABI encoding) to `WaitingForEngineCheck`, and have the state machine re-issue outstanding effects on startup. Tradeoff: larger snapshots, written once per block.
2. **Re-derive from the chain.** On startup, for every entry still in `WaitingForEngineCheck`, emit an effect that re-reads the originating `TransactionProposed` log by request id (via `eth_getLogs` over the proposal's block, which the entry would need to record — a single `u64`) and then runs the check. Keeps snapshots small; costs one log query per stranded request at startup.
3. **Widen the replay window for the state machine only.** Retain snapshots further back (a `snapshot_retention` independent of `max_reorg_depth`) so the anchor is old enough to include any proposal that could still be in `WaitingForEngineCheck` — i.e. at least `COMMIT_WINDOW` blocks. Simple, and it also shrinks the F-SEN-001 race window; costs storage.
4. **At minimum, make it visible.** Add a counter for entries retired from `WaitingForEngineCheck` without a verdict, and log at `warn` rather than dropping silently, so the participation gap is attributable.

Tests to add: a `StateMachine`-level test that commits a snapshot containing a `WaitingForEngineCheck` entry, restarts the machine, replays a range that excludes the proposal, and asserts an effect is (or is not) re-issued.

## Trail

- Reviewer R7: drafted from lead SEN-H10, self-estimate 75%. All six basis citations re-opened in this checkout. The mechanism is E2; the frequency depends on how often a check is still outstanding `max_reorg_depth` blocks after its proposal, which follows from the engine-timeout sizing (`main.rs:45-62`) but was not measured.

## Critic (C-SEN)

### Per-claim verdicts

All rows re-opened; every quote is accurate and no claim is marked `H`.

| # | Verdict | Note |
| --- | --- | --- |
| — | **Supported** | `state.rs:25-32`: `WaitingForEngineCheck { deadline, request: Option<Request> }` — the variant holds no `SafeTransaction`, and `Request` (`state.rs:7-20`) holds only bond and deadline fields. |
| — | **Supported** | `effect.rs:16-26`: `Effect::EngineCheck` carries `transaction: SafeTransaction`, which exists only in the live effect. |
| — | **Supported** | `core/effects.rs:28-36` verbatim, including the doc line "Dropping it aborts any effects that are still in progress" — so a restart destroys every in-flight check. |
| — | **Supported** | `blocks.rs:256-266`: replay starts at `indexed.safe + 1`, so the anchor block itself is never re-applied. |
| — | **Supported** | `service.rs:390-399`: the entry is reaped by `retain` with no log line and no metric. |

### This is the sentinel instance of F-CORE-031 — cross-referenced, not merged

The defect is exactly F-CORE-031's mechanism: an effect is spawned only after the snapshot that records it as pending, and the anchor block is not replayed, so an effect whose originating event sits at or below `indexed.safe` is performed zero times from the state machine's point of view. F-CORE-031 states this generically and already names the sentinel consequence in its own Claim.

**F-CORE-031 should be canonical**; this file should be kept as the crate-level consequence, because it adds one fact the core finding does not have: the sentinel _cannot_ recover even if it wanted to, since `WaitingForEngineCheck` deliberately does not persist the `SafeTransaction` the effect needs. That turns "the resume is lost" into "the check can never be re-issued from state", which is what determines the remediation (persist the transaction, or re-read it from the log).

Note the boundary with F-SEN-001, which is _not_ the same defect: F-SEN-001 lives in the range the rollback **does** replay, where a fresh effect is spawned correctly and the loss comes from the sentinel discarding its own replayed `Committed`. Different block, different mechanism, different fix, and — critically — different impact (bond slashed vs. no bond at risk).

### Finding verdict

**Confirmed. Certainty 82%. Severity Low (unchanged).**

`E2` mechanism, deterministic trigger (any restart, for whichever proposals happen to sit in the anchor block). Low is right per Section 8: missed participation only — no commit was ever made, so no funds are at risk. The substantive complaint is the invisibility, which the finding states correctly: no `warn`, no metric, and the reaped entry is indistinguishable from an ordinary expiry.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

**Note for every sentinel finding whose "tests to add" list names a `service.rs` unit test:** `sentinel` is a **binary-only crate** — `crates/sentinel/src/main.rs` declares `mod service;` and there is no `lib.rs`, so the crate has no library target and `crates/sentinel/tests/` cannot compile against it. Every such test must live inside the existing `#[cfg(test)] mod tests` in the source file. If the team wants these as permanent regression tests reachable from an integration target, **the crate needs a `lib.rs` first**; that is an unstated prerequisite across F-SEN-001, -002, -003, -011, -012 and -015.

### Remediation check

**Sound: option 3 is the cheapest and helps two other findings; option 1 is the direct fix.**

Option 3 (retain snapshots further back via a `snapshot_retention` independent of `max_reorg_depth`, at least `COMMIT_WINDOW` blocks) is the option I would take first, for a reason beyond the one given: it is the **same config change F-CORE-001 option 2 needs**, and that option is currently unsound without it — the retained window is exactly `max_reorg_depth + 1` snapshots (`state/storage.rs:145-161`, `blocks.rs:246`), so "walk back through the retained snapshots until one matches the chain" has nothing to walk back to. One knob, two findings. Its cost is storage, and snapshots are JSON blobs written per committed block, so the cost is real but linear and measurable.

Option 1 (persist `transaction: SafeTransaction` in `WaitingForEngineCheck` and re-issue outstanding effects on startup) is the direct fix and is sound. Two notes: the re-issue must go through the normal command path rather than a special startup path, or it will diverge; and it enlarges every snapshot for every tracked request, which interacts with **F-SEN-005**'s unbounded `WaitingForDisputeResolution` growth — bigger entries make that leak more expensive.

Option 2 (re-read the originating `TransactionProposed` by request id via `eth_getLogs` over the proposal's block, recording just a `u64`) is sound and keeps snapshots small. It costs one log query per stranded request at startup, which is bounded by the number of in-flight requests. Reasonable alternative to option 1 if snapshot size matters.

Option 4 (a counter for entries retired from `WaitingForEngineCheck` without a verdict, logged at `warn`) is the observability floor: today the participation gap is silent and unattributable.

## Post-merge revalidation (RV-SEN)

Re-validated against merge commit `a7f3915` (baseline `2893917`).

### Verdict: **STILL VALID** — the cited state shape and effect plumbing are unchanged

- `crates/sentinel/src/state.rs:25-32` — the enum still opens at `:25` and `WaitingForEngineCheck` still holds only `{ deadline: u64, request: Option<Request> }` at `:29-32`. `state.rs` did change in this merge, but only below the enum: a new `WaitingForOutcome` variant at `:80` and two helper methods at `:118-141` and `:157-165`. The transaction bytes the engine needs are still not persisted.
- `crates/sentinel/src/effect.rs:17-26` — untouched by the merge; `Effect::EngineCheck` still carries the whole `SafeTransaction` in memory only.
- `crates/sentinel/src/service.rs:393-399` — byte-identical; the orphaned entry is still retired silently, with no log line and no metric distinguishing it from an ordinary expiry.
- `crates/core/src/effects.rs:28-36` and `crates/core/src/index/blocks.rs:255-278` — `crates/core` is byte-identical across the merge.

The new `metrics.rs` label (`ResolvedOutcome::RevealMissed`, `metrics.rs:84-89`) does not help here: it is only recorded from `handle_oracle_result`, which requires a bond to have been posted, whereas this finding's entries expire before any commit.

**Certainty 82% and severity Low / Low unchanged.** Status left at `Critiqued`.

## In-flight impact (AS-SEN)

**Pertains to unmerged branches, not to `main`.** Assessed against `origin/fix/sentinel_deadlines` (PR #914) and `origin/feat/optimistic_block_transition` (PR #915, tip `b2aad06`), built from a `git archive` extraction. Both apply cleanly onto `main` — `main`'s crates are byte-identical to their base `199629e`, so a verdict on the tip is also the verdict for the tip merged into `main`. **Effect: unchanged.** The cited `effect.rs`, `state.rs`, `main.rs` and `config.rs` code is unchanged.
