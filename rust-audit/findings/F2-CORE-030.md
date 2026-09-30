# F2-CORE-030 Rollback and restart discard effect resumes applied since the restored snapshot, and the orphaned effects are never re-issued

| Field | Value |
| --- | --- |
| Status | QA'd |
| Crate and module | safenet-core, state/mod.rs (driver.rs, effects.rs) |
| Location | crates/core/src/state/mod.rs:182-189, 236, 246-258 (related: crates/core/src/driver.rs:261-279; crates/core/src/index/blocks.rs:256-266; crates/core/src/state/storage.rs:152-159; crates/sentinel/src/service.rs:127-139, 264-276, 307-319, 393-399; contracts/src/libraries/SentinelOracleRequests.sol:199-202) |
| Severity | High / High |
| Certainty | 92% (QA2-CORE; Critic C2-CORE-B set 80%) |
| Assumptions involved | A4, A17 (service-caused reorg and crash consistency stays in scope) |
| Tags | reorg, crash-consistency |

Audited commit: `3ec8bc5`.

## Claim

The persistence model in `crates/core/src/state/mod.rs` has a structural gap between effects and snapshots. An effect is spawned by the driver only after the snapshot of the block whose event emitted it has been committed (`handle_update` commits at `state/mod.rs:236`, the driver spawns at `driver.rs:278` afterwards). Its resume mutates only the live state (`handle_resume`, `state/mod.rs:246-258`) and is persisted with the next log-range commit, i.e. in the snapshot of a **later** block. A rollback (`Uncle{n}`, `state/mod.rs:182-189`) replaces the live state with snapshot `n-1` and emits no commands; a restart emits `Uncle{safe+1}` and replays only blocks `> safe` (`blocks.rs:256-266`), where `safe` is the oldest retained snapshot (`storage.rs:152-159`).

Consequently, **every rollback to block `k` discards every resume for effects triggered by events in block `k`, and those effects are never performed again**, because block `k` itself is not replayed. The documented contract ("Effects may be performed more than once for the same chain message, for example after a crash or reorg replay", `state/mod.rs:60-62`) covers replayed messages only; it says nothing about messages that are _not_ replayed, and core gives services no hook to re-issue pending effects after a restore. Actions the lost resume had already produced were durably inserted into the transaction queue (`tx/storage.rs:93-103`), which is not rolled back, so the service's persisted state and its on-chain footprint diverge.

This happens on (1) **every restart**, for the effects triggered in the retained safe-anchor block (with the default `max_reorg_depth = 5` on Gnosis, the block roughly 25 s before shutdown), and (2) **every one-block reorg** of block `k+1` for the effects triggered in block `k` whose resume was already applied.

Sentinel consequence (traced, `E2`): for a request proposed in block `k`, the sentinel's `EngineCheck` resumes, `commit_vote` queues `ApproveToken` + `Commit` (`service.rs:214-242`) and the commit is mined. After the rollback the request is back in `WaitingForEngineCheck`; its own `Committed` event is rejected as "unexpected commitment" in that state (`service.rs:307-319`); `handle_new_request` only stores the request terms (`service.rs:264-276`); `handle_block_advance` keeps the entry until the commit deadline and then drops it without ever re-issuing the check (`service.rs:393-399`); no `Reveal` is ever produced. The oracle then treats the sentinel as a non-revealer and slashes `slashAmount` from its bond on `finalize` (`SentinelOracleRequests.sol:199-202`, `SentinelOracle.sol:262-268`). The loss is not recoverable and recurs with every restart and every adjacent short reorg while requests are flowing.

Validator consequence (`I`, for R4/R5 to settle): `KeyGen` -> `Effect::KeyGenSetup` -> `Resume::Setup` has the same shape; the state keeps `secrets: None` until the resume arrives (`crates/validator/src/state/keygen.rs:65-78`). The signing path is robust to the loss because the validator consumes its own on-chain `SignRevealedNonces` (`crates/validator/src/state/sign.rs:278-285`), which is the pattern the sentinel lacks.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | A resume mutates the live state only; it is persisted with the next log range. Verified by the existing test, run this session (28 passed). | E1 | crates/core/src/state/mod.rs:246-258; test at 444-479 | `/// Handles an effect resume without committing a state snapshot.` ... `pub async fn handle_resume(&mut self, resume: T::Resume) -> Result<Commands<S, T>, Error> {` ... `let (state, commands) = self.transition.apply_transition(state, Message::Resume(resume));` `*lock = Some((state, status));` |
| 2 | The snapshot for a log range is committed inside `handle_update`, before the driver dispatches the commands (so effects always start after their block's snapshot exists). | E2 | crates/core/src/state/mod.rs:236; crates/core/src/driver.rs:261-279 | `self.snapshots.commit(blocks.last, &state).await?;` ... driver: `let commands = self.state.handle_update(update).await?;` ... `state::Command::Effect(effect) => self.effects.spawn(effect),` |
| 3 | A rollback replaces the live state with the parent snapshot and emits no commands. | E2 | crates/core/src/state/mod.rs:182-189 | `Update::Block(BlockUpdate::Uncle { number })` ... `let (_, state) = self.snapshots.reorg(number).await?;` `let status = Status::BlockPending { pending: number };` `(state, status, vec![])` |
| 4 | A restart rolls back to the oldest retained snapshot and replays only the blocks after it. | E2 | crates/core/src/index/blocks.rs:256-266 | `// The earliest retained snapshot is the rollback anchor. Replay` `// everything after it, but only emit an uncle when there are newer` `// snapshots to discard.` ... `let uncle = indexed.safe.checked_add(1);` `if let Some(uncle) = uncle && uncle <= indexed.latest {` `self.queue.push_back(BlockUpdate::Uncle { number: uncle });` |
| 5 | The oldest retained snapshot is the watcher's `safe` block (prune keeps `>= safe`), so the anchor is exactly `latest - max_reorg_depth` at shutdown. | E2 | crates/core/src/state/storage.rs:152-159; crates/core/src/driver.rs:263 | `"DELETE FROM snapshots WHERE block_number < ? AND block_number < (SELECT MAX(block_number) FROM snapshots)"` ... driver: `self.state.prune(block_status.safe).await?;` |
| 6 | Actions are inserted durably and are never removed by a rollback (nothing in `state/mod.rs` touches the queue). | E2 | crates/core/src/tx/storage.rs:93-103 | `let mut tx = self.pool.begin().await?;` `for (transaction, expires_at) in transactions {` ... `sqlx::query("INSERT INTO transactions (request, expires_at) VALUES (?, ?)")` ... `tx.commit().await?;` |
| 7 | Sentinel: a proposal enters `WaitingForEngineCheck` and spawns the engine check effect. | E2 | crates/sentinel/src/service.rs:127-139 | `let deadline = block.saturating_add(self.voting_window);` `state.0.insert(request_id, RequestState::WaitingForEngineCheck { deadline, request: None, },);` ... `vec![Command::Effect(effect::Effect::EngineCheck {` |
| 8 | Sentinel: the resume moves the entry to `CollectingCommitments` and queues the commit; both actions are durable once queued. | E2 | crates/sentinel/src/service.rs:214-242 | `RequestState::CollectingCommitments { approve, reason, slash_amount, commit_deadline, reveal_deadline, committed_count: 0, self_committed: false, },` ... `kind: SentinelActionKind::Commit { id: request_id, hash, }, expires_at: Some(commit_deadline),` |
| 9 | Sentinel: after the rollback the sentinel's own `Committed` event is ignored because the entry is not `CollectingCommitments`. | E2 | crates/sentinel/src/service.rs:307-319 | `let RequestState::CollectingCommitments { committed_count, self_committed, .. } = entry else {` `tracing::warn!(request_id = %event.requestId, state = entry.name(), "ignoring unexpected commitment");` `return (state, Vec::new());` `};` |
| 10 | Sentinel: `NewRequest` only records terms in `WaitingForEngineCheck`; it does not restart the check. | E2 | crates/sentinel/src/service.rs:264-276 | `Some(RequestState::WaitingForEngineCheck { deadline, request: None, }) => {` `state.0.insert(request_id, RequestState::WaitingForEngineCheck { deadline, request: Some(request), },);` `(state, Vec::new())` |
| 11 | Sentinel: the entry is kept until the commit deadline and then dropped; `NewBlock` never re-issues `EngineCheck`. | E2 | crates/sentinel/src/service.rs:393-399 | `state.0.retain(\|id, entry\| match entry {` `RequestState::WaitingForEngineCheck { deadline, request } => {` `block <= request.as_ref().map_or(*deadline, \|request\| request.commit_deadline)` `}` |
| 12 | Contract: a committed-but-unrevealed sentinel forfeits `slashAmount` to the protocol funds receiver at `finalize`. | E2 | contracts/src/libraries/SentinelOracleRequests.sol:199-202; contracts/src/SentinelOracle.sol:262-268 | `uint128 nonRevealerCount = prog.committedCount - prog.revealedCount;` `unchecked { unrevealedBond = nonRevealerCount * self.terms.slashAmount; }` ... `if (unrevealedBond > 0) { FEE_TOKEN.safeTransfer(fundsReceiver, unrevealedBond); }` |
| 13 | Validator: the keygen setup resume has the same shape (state waits with `secrets: None`); whether a later handler re-issues the effect is not verified here. | I | crates/validator/src/state/keygen.rs:65-78 | `pub(super) fn handle_key_gen_setup(` ... `RolloverState::CollectingCommitments { next_epoch, group, secrets: KeyGenCommitment::Participating { poap, secrets: None, },` |

## Trigger

Restart variant (deterministic): default `max_reorg_depth = 5`, sentinel running normally. (1) Block `k` contains `TransactionProposed`/`NewRequest` for request `R`; snapshot `k` = `WaitingForEngineCheck`. (2) The engine answers; `handle_engine_check_result` -> `commit_vote`; `Commit` is queued and mined; the state `CollectingCommitments{self_committed: true}` is committed in snapshot `k+1` (or later). (3) Operator restarts the sentinel while `latest == k + 5` (`safe == k`, so `MIN(snapshots) == k`). (4) On start: `Uncle{k+1}` -> state restored to snapshot `k` (`WaitingForEngineCheck`); blocks `k+1..` replayed; `Committed` for `R` from the sentinel's own address is logged as "ignoring unexpected commitment"; at `block > commit_deadline` the entry is dropped; no `Reveal`. (5) `finalize` slashes the sentinel's bond for `R`.

Reorg variant: same as steps 1-2, then block `k+1` is uncled after the resume was applied (any engine check that resolves before block `k+1` is orphaned by a `k+1` uncle); rollback to snapshot `k`; identical outcome.

The in-flight sub-case (resume not yet applied at shutdown/uncle) also loses the effect for the restart variant, but no commit was sent, so the cost is a missed vote rather than a slashed bond. Note that a resume arriving _after_ a rollback to `k` for an effect from block `k` is applied correctly (the restored state expects it); the loss is specific to resumes applied _before_ the rollback.

## Considered and rejected

- "Replay re-runs the effect" - only blocks `> k` are replayed (`blocks.rs:261-266`); the event in block `k` that emitted the effect is inside the restored snapshot and is never re-delivered. The map's lead CORE-H10 (stale resumes applied after rollback) is the opposite direction and is harmless here.
- "The sentinel re-issues the check on `NewBlock`" - `handle_block_advance` only drops or advances entries (`service.rs:390-471`); no branch emits `Effect::EngineCheck`.
- "The sentinel recovers from its own `Committed` log" - rejected at `service.rs:307-319`; the `self_committed` edge is only handled inside `CollectingCommitments` (`service.rs:325-326`).
- "`expires_at` or the queue prevents the on-chain commit" - the commit was already submitted in the first run; the queue is durable (`tx/storage.rs:93-103`) and rollback never touches it. The commit is exactly what makes the later non-reveal costly.
- "Committing resumes immediately would fix it" - only partially: a resume applied while the status is `BlockEvents{n}` (after `New{n}`, before `Logs{n}`) still cannot be attributed to a committed block; see remediation.
- "The validator is equally exposed" - not shown: the signing path consumes its own `SignRevealedNonces` (`sign.rs:278-285`); the keygen path is flagged for R4 (row 13).

## Remediation options

1. Give the transition a way to re-issue pending effects after a restore: add e.g. `StateTransition::pending_effects(&S) -> Vec<Effect>` (or a `Message::Restored` transition) invoked by `StateMachine` after `Uncle` and after the restart rollback, and spawn the returned effects. Tradeoff: a small API addition; services must be able to enumerate outstanding effects from state (the sentinel can: every `WaitingForEngineCheck` entry).
2. Persist resumes eagerly: in `handle_resume`, when the status is `BlockPending{pending}`, upsert snapshot `pending-1` with the new live state. Cheap, closes the restart case for resumes that land between blocks, but does not cover resumes applied during `BlockEvents`/`WarpEvents`; combine with option 1 or document the residual window.
3. Service-side mitigation (sentinel, R7): in `handle_block_advance` re-emit `EngineCheck` for `WaitingForEngineCheck` entries that are older than one block, and accept the sentinel's own `Committed` from `WaitingForEngineCheck`/`WaitingForRequest` by moving the entry to `CollectingCommitments` with `self_committed = true` (the reveal salt is deterministic, `hashing.rs:51`, so the reveal can still be produced; the `reason`/`approve` would have to be re-derived from a re-run check, which the state comments warn against - hence option 1 is preferable).

Tests to add: a `state/mod.rs` test that (a) applies an event emitting an effect at block 1, (b) applies its resume, (c) commits block 2, (d) uncles block 2, and asserts that the restored state still awaits the resume and that no command re-issues the effect (documents the gap; flips to asserting the re-issue once fixed); a sentinel flow test for "proposal at `k`, commit, uncle `k+1`, own `Committed` replayed, block advances past the commit deadline" asserting a `Reveal` action is still produced.

## Trail

- Reviewer R2: drafted, self-estimate 75% (mechanism E1/E2; sentinel consequence E2 by trace; validator consequence unverified). Severity High because the loss is fund-bearing (bond slashed), non-recoverable and recurs under routine operation (every restart, every adjacent one-block reorg); the Critic may prefer Medium if restarts and short reorgs are considered rare relative to request volume.
- Critic C2-CORE-B: Confirmed, 80%, severity High (reviewer High). Canonical for the core mechanism; F2-SEN-001 is the canonical statement of the sentinel loss.
- QA2-CORE: Reproduced (reorg, restart and ordering variants; pure state machine and real `BlockWatcher`); certainty 80 → 92; PoC `poc/F2-CORE-030/`.

## Critic (C2-CORE-B)

Method: read title and Location only, then traced the rollback and restart paths through `state/mod.rs`, `state/storage.rs`, `driver.rs`, `effects.rs` and `index/blocks.rs:244-300` myself, and the sentinel handlers (`service.rs:99-471`) before reading the reviewer's argument. Every citation re-opened at `3ec8bc5`, including `SentinelOracleRequests.sol:199-202` and `SentinelOracle.sol:262-268`. I re-ran `cargo test -p safenet-core --lib -- tx:: state:: effects::` (41 passed), which covers the tests cited in row 1.

| # | Verdict | Note |
| --- | --- | --- |
| 1 | Supported (E1) | state/mod.rs:251-258 applies the resume and writes only `*lock`; test 444-479 asserts the committed tip is unchanged until the next log range. |
| 2 | Supported | commit at state/mod.rs:236 happens inside `handle_update`; the driver spawns at driver.rs:278 only after 261 returns. |
| 3 | Supported | state/mod.rs:183-189: `reorg(number)` then `(state, status, vec![])`. |
| 4 | Supported | index/blocks.rs:261-266 (`Uncle{safe+1}` when `safe+1 <= latest`), 271-278 (`Warp{safe+1..node_safe}`). |
| 5 | Supported | state/storage.rs:152-159 keeps `>= safe` plus `MAX`; driver.rs:263 prunes with the watcher's `safe`; `status()` reports `MIN` as `safe` (storage.rs:86-101). |
| 6 | Supported | tx/storage.rs:93-103; nothing in `state/` or `driver.rs` deletes queue rows. |
| 7 | Supported | sentinel service.rs:127-145. |
| 8 | Supported | sentinel service.rs:198-243. |
| 9 | Supported | sentinel service.rs:307-319. |
| 10 | Supported | sentinel service.rs:264-276. |
| 11 | Supported | sentinel service.rs:393-399; no arm of `handle_block_advance` (390-471) or `handle_new_request` (248-291) emits `Effect::EngineCheck`. |
| 12 | Supported | SentinelOracleRequests.sol:199-202 and SentinelOracle.sol:262-268 both re-opened; quotes match. |
| 13 | `I`, left as filed | validator keygen path is R4/C2-VAL-A's; not judged here. |

**Which resumes are discarded (my own derivation).** Let `t` be the rollback target: `n - 1` for `Uncle{n}` (state/mod.rs:186-188), or the oldest retained snapshot for a restart (`MIN(block_number)`, storage.rs:86-101, which after steady-state pruning equals the watcher's `safe`, i.e. `latest - max_reorg_depth`). A resume is persisted only by the next `Logs` commit (mod.rs:236, 251-258), so **every resume applied after `commit(t)` and before the rollback is discarded**, and the effect that produced it is re-spawned only if its emitting event lies in a replayed block `> t` (mod.rs:213-223 → driver.rs:278). Effects emitted by blocks `<= t` are therefore orphaned in two forms: (a) resume already applied — discarded by the restore; (b) effect still in flight at shutdown — the `JoinSet` dies with the process (effects.rs:35-38) and nothing re-creates it. **Nothing re-issues them**: `Uncle` returns `vec![]`, `with_init` (mod.rs:130-152) restores without emitting commands, and `StateTransition` (mod.rs:79-94) has only `apply_transition` — there is no "restored" hook. During a catch-up warp the page snapshot is the only retained row after `prune` (storage.rs:152-159; test mod.rs:596-612), so `t` is the previous page's end and every in-flight effect at a crash is lost, not only those older than `max_reorg_depth`.

**Extension of the trigger.** The reviewer's restart variant covers proposals in block `safe`. When the proposal block _is_ replayed (`b > t`), the effect is re-spawned, but the replayed own `Committed` is applied synchronously inside the same or an earlier `Logs` update — before the driver even spawns the effect (driver.rs:261 vs 278) — and is ignored at service.rs:307-319; the later resume then produces `CollectingCommitments { self_committed: false }` (service.rs:214-225) plus a duplicate `Commit` that reverts (`AlreadyCommitted`, SentinelOracleCommitments.sol:91-96), and the entry is dropped at service.rs:415-417 without a `Reveal`. After `d` blocks of downtime the warp page covers `safe+1 .. L+d-5` (blocks.rs:271-278), i.e. the whole pre-shutdown window once `d >= 5`, so **every request whose proposal and own commit both landed in the last `max_reorg_depth` blocks before a restart with ~25 s of downtime is slashed deterministically**; without downtime the same ordering is the likely outcome of the race between queued watcher updates and the engine round-trip. R7 reached the same three variants independently (`F2-SEN-001` (a)/(b)/(c)); the two reviews converge.

Finding verdict: **Confirmed** (mechanism E1/E2; both triggers verified by trace; no end-to-end reproduction). Certainty **80%**.

Severity **High / High**. A5 requires reorgs within `max_reorg_depth` to be handled and a restart is a designed operation; both convert into an irreversible loss of bonded funds per affected request, with no attacker needed, recurring under routine operation, and the exposure is the whole reorg window (extension above), not one block. Not Critical: per-request `slashAmount`, not loss "at scale", and no key or nonce material involved. Medium would require request volume to be negligible; I do not adopt it. A16 not applicable (sentinel); A17: service-caused, in scope.

Same defect, two files: **F2-CORE-030 is canonical for the core defect** (non-persisted resumes plus no re-issue hook; it also shapes the validator's keygen path, row 13). **F2-SEN-001 is canonical for the sentinel loss** and already carries the wider trigger surface; I cannot edit it (owned by C2-SEN) — the Manager should mirror this note there. F2-CORE-032/063 (duplicate actions) interact: the replayed duplicate `Commit` is what pins `self_committed = false` in the extension.

Remediation check: the reviewer's own caveat on option 2 is correct (a resume applied during `BlockEvents{n}` cannot be attributed to a committed block); option 1 (`pending_effects` hook after every restore) is the sound fix and also closes the in-flight sub-case and F2-CORE-036's effect half. The proposed `state/mod.rs` test is straightforward with the in-crate `TestTransition` (E1 for QA).

## QA (QA2-CORE)

**Outcome: Reproduced** — both variants and the Critic's ordering extension, at the state-machine level and with the real `BlockWatcher` driving the restart.

Command: paste `poc/F2-CORE-030/state_mod_tests.rs` into the `mod tests` of `crates/core/src/state/mod.rs` and `poc/F2-CORE-030/blocks_tests.rs` into the `mod tests` of `crates/core/src/index/blocks.rs`, then `cargo test -p safenet-core --lib qa_f2_core_030 -- --nocapture --test-threads=1` (4 tests; they pass, i.e. the defect is present). Both files reverted afterwards.

Decisive output (verbatim from `poc/F2-CORE-030/output.txt`):

- Reorg variant: `committed after block 2: block 2, TestState { blocks: [1, 2], events: [10], resumes: [777] }` → `Uncle{2} -> []` → `replayed block 2 -> [Action(Block(2))] []` → `committed after rollback + replay: block 2, TestState { blocks: [1, 2], events: [10], resumes: [] }`. The rollback returns no command, block 1's event (which emitted effect 10) is not replayed, and the applied resume is gone.
- Restart variant with the real watcher (`max_reorg_depth = 2`; effect emitted by block 1000; resume applied before block 1001 committed; shutdown at 1002 with the store at `{1000..=1002}`): `restart updates: [Uncle { number: 1001 }, New { number: 1001, .. }, New { number: 1002, .. }]` → `after restart replay: commands []; tip 1002 = QaState { blocks: [999, 1000, 1001, 1002], events: [10], resumes: [] }`.
- Ordering extension: `replayed block 2 -> [Action(Event(10)), Effect(10)] (the driver spawns the effect only now)` → `replayed block 3 committed at 3: TestState { blocks: [1, 2, 3], events: [10, 11], resumes: [] }` — the follow-up event of block k+1 is applied before the re-spawned effect can resume (`driver.rs:261` applies the update, `:278` spawns afterwards, and the next replayed update is already queued by the watcher).

The sentinel-specific consequence (own `Committed` ignored, bond slashed) is not exercised here; it belongs to F2-SEN-001. Nothing in `StateMachine`, `Driver` or `EffectManager` re-issues effects after an `Uncle` or a restore (`effects.rs:35-38`: dropping the manager aborts in-flight tasks).

Certainty: 80 → **92**. Critic Confirmed plus `E1` for the mechanism, both triggers and the ordering extension; not higher because the fund-loss consequence is traced, not executed.

Remediation check: option 1 (a `pending_effects`/`Message::Restored` hook invoked after `Uncle` and after the startup restore, with the returned effects spawned) is sound: it is a pure function of the restored state, compatible with "effects may run more than once" and "resume ordering undefined", and it also covers effects still in flight at shutdown. Option 2 is partial, as the reviewer says, and would make `handle_resume` IO-bearing; a complement at most. Option 1 alone does not close the Critic's ordering extension (a replayed own `Committed` applied before the resume): that needs the sentinel to accept its own `Committed` from `WaitingForEngineCheck` (F2-SEN-001), so both are required for the sentinel loss.

## Anchors at fe9e84c (Manager)

Anchors in `crates/sentinel/src/service.rs` after the `origin/main` merge (`fe9e84c`): 127–139, 214–242, 264–276, 307–319 unchanged; **393–399 → 406–412 with changed content** at old line 396 (`<= request` → `< request`, PR #914: the un-self-committed entry is now dropped on the deadline block itself, one block earlier). Basis row 11 ("kept until the commit deadline and then dropped") describes the pre-merge timing; the core mechanism (resume discarded on rollback/restart, nothing re-issues) lives in `crates/core`, which is byte-identical. Re-read of row 11 assigned to Critic C2-SEN-Δ together with `F2-SEN-001`; QA2-SEN-Δ re-ran the sentinel-side PoC at `fe9e84c` with the same result (`state/run2/baseline-delta.md` §3).

## Critic (C2-SEN-Δ, re-validation)

Basis row 11 against `fe9e84c` (`crates/sentinel/src/service.rs:406-412`): the verbatim quote is stale — the arm now reads `block < request.as_ref().map_or(*deadline, |request| request.commit_deadline)` (`408-411`), not `block <=`. The prose ("kept until the commit deadline and then dropped") is if anything more literal now: the un-self-committed `WaitingForEngineCheck` entry is dropped at `NewBlock(commit_deadline)` instead of `NewBlock(commit_deadline + 1)`. The second half of the row, "`NewBlock` never re-issues `EngineCheck`", is unchanged: no arm of `handle_block_advance` (`403-490`) emits an effect. So the row is inaccurate in its quote and anchor only; re-anchor to `406-412` and refresh the quote to `<`.

The verdict does not depend on the row's timing. The loss needs (i) the resume discarded by rollback/restart (`crates/core`, byte-identical), (ii) `Committed(self)` rejected in `WaitingForEngineCheck` (`service.rs:307-319`, unchanged), and (iii) no re-issue of the check before the entry is dropped — true one block earlier or later. The deferred-drop branch added by PR #914 (`433-435`) guards `CollectingCommitments { self_committed: false }` only and never applies to the rolled-back entry, which stays in `WaitingForEngineCheck`. QA2-SEN-Δ's re-run of the sentinel-side PoC at `fe9e84c` (recorded in F2-SEN-001: the stranded entry is now dropped at `NewBlock(120)` instead of `121`, no `Reveal`) is executed evidence of exactly that. High / 92% stands.

- C2-SEN-Δ: row 11 quote and anchor stale (`<=` → `<`, `393-399` → `406-412`), prose still true, verdict independent of the drop block; header unchanged.

## Reconciliation (run 2)

**Final: EXTENDS `F-CORE-031` (canonical) — combined High, 92 (E1); this file supplies the number.** Run 1 filed the same commit-before-spawn ordering at Medium 78 and its Critic narrowed the restart variant on the premise that replayed blocks re-spawn their effects harmlessly, calling the replay case "mutually exclusive" with the loss; this file's executed ordering extension overturns that premise (`poc/F2-CORE-030`), so the combined finding is raised to High and the restart variant is deterministic for the last `max_reorg_depth` blocks before a restart with ≥ 25 s downtime (`state/run2/reconciliation/core.md` §1.1). `F-SEN-001` / `F2-SEN-001` stay canonical for the sentinel loss. Anchor note: run 1's `driver.rs:255/257/267-274` are this file's `261/263/272-279` after the housekeeping hook.
