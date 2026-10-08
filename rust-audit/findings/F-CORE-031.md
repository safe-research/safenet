# F-CORE-031 Effects are spawned only after the snapshot that records them as pending, so every rollback that lands on the spawning block reverts the resume and never re-runs the effect

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | core, `state/mod.rs` + `effects.rs` + `driver.rs` (the runtime's effect/resume contract) |
| Location | `crates/core/src/state/mod.rs:246-258` and `182-189` (related: `crates/core/src/driver.rs:255-274`; `crates/core/src/effects.rs:28-36`; `crates/core/src/state/storage.rs:145-161`; `crates/core/src/index/blocks.rs:255-266`) |
| Severity | Medium / Medium |
| Certainty | 78% |
| Assumptions involved | A5, A4, A1 |
| Tags | reorg, crash-consistency |

## Claim

The runtime documents that an effect "may be performed more than once for the same chain message" (`state/mod.rs:60-62`, `effects.rs:21-24`) — an at-least-once contract that every service is written against. The implementation does not provide it. There is a class of rollbacks after which an effect is performed **zero** times from the state machine's point of view: its result is reverted and it is never re-spawned.

The mechanism is an ordering property, not a race:

1. A snapshot is committed **inside** `handle_update` (`state/mod.rs:236`), before the driver has even seen the returned commands. The effect is spawned afterwards, in the driver (`driver.rs:267-274`). So the snapshot for block `n` — the one that records "effect E is pending" — is always durable _before_ E starts.
2. A resume never commits a snapshot of its own (`handle_resume`, `state/mod.rs:246-258`). E's result is persisted only when the _next_ log range commits, i.e. in a snapshot for some block `m > n`.
3. A rollback restores the snapshot at `uncle - 1` and replays from `uncle` upward (`state/mod.rs:182-189`). Block `n` itself is replayed only when the rollback goes strictly below it.

Put together: for any anchor in `[n, m-1]` the state reverts to "E is pending" while E has already run and will never run again. The state machine waits for a resume that no longer exists.

The two ways to reach an anchor in that interval are both ordinary operation, not corner cases:

- **Any reorg.** `Uncle{q}` anchors at `q-1`. Every effect spawned by an event in block `q-1` whose resume landed during block `q`'s processing — i.e. any effect faster than one block, ~5 s on Gnosis — is in exactly this interval. Reorgs up to `max_reorg_depth` are the case assumption A5 says must be handled gracefully.
- **Every restart.** `BlockWatcher::initialize` emits a synthetic `Uncle{safe+1}` (`blocks.rs:261-266`), so the anchor is `MIN(block_number)` — pruning holds that at about `head - max_reorg_depth` (`state/storage.rs:151-161`). Every effect spawned by an event in that anchor block loses its resume, and every effect still in flight when the process stopped (the `JoinSet` is aborted with the `Driver`, `effects.rs:30-31`) is both un-run and un-recorded.

The rollback is safe for the case that looks most dangerous — an effect _still in flight_ across the rollback keeps running and resumes into the restored state, which is consistent — and that is precisely why the failing case is invisible in testing.

Concretely for the sentinel: a lost `EngineCheck` resume leaves the request in `WaitingForEngineCheck` until its deadline, at which point `handle_block_advance` silently `retain`s it away (`crates/sentinel/src/service.rs:393-399`). The sentinel never commits and never votes on that transaction proposal, with one `warn`-free, metric-free hole in its participation. For the validator the equivalent is a signing or key-generation session that sits in a `Waiting…` state until `handle_signing_timeouts`/`handle_key_gen_timeouts` reap it (`crates/validator/src/state/mod.rs:464-476`).

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The runtime promises at-least-once effect execution; services are told to prepare for replay, not for loss. | E2 | `crates/core/src/state/mod.rs:59-62` | <pre>/// Effects may be performed more than once for the same chain message, for<br>/// example after a crash or reorg replay. Transitions that emit effects must be<br>/// prepared for the replayed effect to resume with a different result.</pre> |
| 2 | The snapshot recording "E pending" is committed inside `handle_update`, i.e. before the commands (including the effect) are returned to the driver. | E2 | `crates/core/src/state/mod.rs:236-243` | <pre>self.snapshots.commit(blocks.last, &state).await?;<br><br> (state, status, commands)<br> }<br> _ => return Err(Error::BadUpdate),<br> };<br> *lock = Some((state, status));<br> Ok(commands)</pre> |
| 3 | Only then does the driver spawn the effect. | E2 | `crates/core/src/driver.rs:266-274` | <pre>let mut transactions = Vec::with_capacity(commands.len);<br>for command in commands {<br> match command {<br> state::Command::Action(action) => {<br> transactions.push(self.actions.encode_action(action));<br> }<br> state::Command::Effect(effect) => self.effects.spawn(effect),<br> }<br>}</pre> |
| 4 | A resume mutates live state and commits nothing; it is persisted only with the next log range, i.e. in a later block's snapshot. | E2 | `crates/core/src/state/mod.rs:246-257` | <pre>/// Handles an effect resume without committing a state snapshot.<br>///<br>/// Resume transitions update the live state immediately. Their state is<br>/// persisted with the next successfully processed log range.<br>pub async fn handle_resume(&mut self, resume: T::Resume) -> Result<Commands<S, T>, Error> {<br> let mut lock = self.inner.lock.await;<br> let (state, status) = mem::take(&mut *lock).ok_or(Error::Poisoned)?;<br> let (state, commands) = self<br> .transition<br> .apply_transition(state, Message::Resume(resume));<br> *lock = Some((state, status));<br> Ok(commands)</pre> |
| 5 | The crate's own test pins that behaviour: after `handle_resume(777)` the committed snapshot at block 1 has `resumes: []`, and 777 only appears in the snapshot at block 2. | E2 | `crates/core/src/state/mod.rs:450-464` | <pre>assert_eq!(<br> machine.handle_resume(777).await.unwrap,<br> vec![Command::Action(Action::Resume(777)),]<br>);<br>assert_eq!(<br> committed(&pool).await,<br> Some((<br> 1,<br> TestState {<br> blocks: vec![1],<br> events: vec![42],<br> resumes: vec![],<br> },<br> ))<br>);</pre> |
| 6 | A rollback replaces live state with the snapshot at `uncle - 1`; replay then starts at `uncle`, so the spawning block is not re-applied. | E2 | `crates/core/src/state/mod.rs:182-189` | <pre>Update::Block(BlockUpdate::Uncle { number })<br> if matches!(status, Status::BlockPending { pending } if number < pending)<br> &#124;&#124; matches!(status, Status::BlockEvents { latest } if number <= latest) =><br>{<br> let (_, state) = self.snapshots.reorg(number).await?;<br> let status = Status::BlockPending { pending: number };<br> (state, status, vec![])<br>}</pre> |
| 7 | `reorg` deletes every snapshot at or above the uncle, so any snapshot that did hold the resume is destroyed. | E2 | `crates/core/src/state/storage.rs:129-133` | <pre>let mut tx = self.pool.begin.await?;<br>sqlx::query("DELETE FROM snapshots WHERE block_number >= ?")<br> .bind(i64::try_from(uncle)?)<br> .execute(&mut *tx)<br> .await?;</pre> |
| 8 | Every restart performs exactly this rollback, anchored at the oldest retained snapshot. | E2 | `crates/core/src/index/blocks.rs:255-266` | <pre>if let Some(indexed) = indexed {<br> // The earliest retained snapshot is the rollback anchor. Replay<br> // everything after it, but only emit an uncle when there are newer<br> // snapshots to discard. A pruned warp may retain only its latest<br> // snapshot, in which case we can continue directly from the next<br> // block without a synthetic reorg.<br> let uncle = indexed.safe.checked_add(1);<br> if let Some(uncle) = uncle<br> && uncle <= indexed.latest<br> {<br> self.queue.push_back(BlockUpdate::Uncle { number: uncle });<br> }</pre> |
| 9 | In-flight effects are lost outright on shutdown: the `JoinSet` is owned by the manager and dropping it aborts them. Nothing persists the pending-effect set. | E2 | `crates/core/src/effects.rs:28-36` | <pre>/// Executes effects concurrently and yields their resumes as they complete.<br>///<br>/// The manager owns all spawned effect tasks. Dropping it aborts any effects<br>/// that are still in progress.<br>pub struct EffectManager<Handler, Effect, Resume> {<br> handler: Arc<Handler>,<br> tasks: JoinSet<Resume>,<br> effect: PhantomData<fn(Effect)>,<br>}</pre> |
| 10 | Consequence in a real service: the sentinel spawns one `EngineCheck` per `TransactionProposed` and drops the request when the deadline passes without a resume. | E2 | `crates/sentinel/src/service.rs:139-143` and `393-399` | <pre>vec![Command::Effect(effect::Effect::EngineCheck {<br> request_id,<br> transaction: event.transaction,<br> block,<br>})],</pre><br><pre>state.0.retain(&#124;id, entry&#124; match entry {<br> RequestState::WaitingForEngineCheck { deadline, request } => {<br> block<br> <= request<br> .as_ref<br> .map_or(*deadline, &#124;request&#124; request.commit_deadline)<br> }</pre> |

## Trigger

**Restart variant (deterministic, no reorg needed).** Defaults `max_reorg_depth = 5`, Gnosis 5 s blocks.

1. Head is `H`; the `snapshots` table holds `H-5 … H` (`prune` is called with the watcher's `safe` on every update, `driver.rs:257`).
2. At block `A = H-5`, a `TransactionProposed` log makes the sentinel emit `Effect::EngineCheck{request_id}`; snapshot `A` is committed with the request in `WaitingForEngineCheck`, then the effect is spawned.
3. The engine answers within a second; `handle_resume` moves the request to `WaitingForRequest`. That change is persisted in snapshot `A+1`.
4. `SIGTERM` (a deploy, an OOM kill, or the status-0 exit of F-CORE-030). Restart.
5. `SnapshotStore::status` = `{safe: A, latest: H}` → `Uncle{A+1}` → `reorg(A+1)` deletes `A+1 … H` and restores snapshot `A`: the request is back in `WaitingForEngineCheck`.
6. Replay runs `A+1 … H`. Block `A` is never re-applied, so no second `EngineCheck` is emitted and no resume ever arrives. At `deadline` the request is silently dropped; the sentinel never votes.

Step 3's timing is not a race: by construction (basis rows 2 and 3) a resume can only ever be recorded in a snapshot strictly after the one that spawned the effect.

**Reorg variant.** Same steps 1-3 at block `q-1` while running, then a one-block reorg: `Uncle{q}` anchors at `q-1`, the resume state is deleted with snapshot `q`, and block `q-1`'s event is not replayed. Any effect that completes in under one block time is in the window.

## Considered and rejected

- **"The service replays the event on restart, so the effect is re-emitted."** True only for blocks strictly above the anchor. The anchor block itself is restored, not replayed (basis row 6, and the crate's own `reorg_rolls_back_to_the_common_ancestor` test, `state/mod.rs:551-581`, shows block 1 keeping its original event after `uncle(2)`).
- **"An in-flight effect survives the rollback and resumes."** It does, and that case is correct. The finding is about a resume that has _already been applied_, or an effect aborted with the process. Both leave the restored state saying "pending" with nothing running.
- **"The state machine guards against stale resumes, so it must guard against missing ones."** The guard is the opposite direction: `handle_engine_check_result` ignores a resume whose entry has moved on (`crates/sentinel/src/service.rs:156-171`) — good hygiene for CORE-H10, and no help here.
- **"The `Uncle` guard makes the shallow case unreachable."** Checked: after committing block `p` the status is `BlockPending{pending: p+1}` and `Uncle{q}` is only accepted for `q < p+1` (`state/mod.rs:182-184`), so the shallowest legal anchor is `p-1`. That still lands on or above the spawning block for every effect spawned at `p-1` or earlier — the interval in the claim is non-empty for every effect, not just deep reorgs.
- **"Timeouts make it self-healing."** They bound the damage, they do not repair it: the sentinel drops the request without voting (basis row 10), the validator abandons the round (`handle_signing_timeouts`, `crates/validator/src/state/sign.rs:512-517`). Neither recomputes the lost effect. A service with a state that has no timeout would stall permanently.
- **Not a duplicate of F-CORE-001** (which is about the _identity_ of the anchor snapshot) or of **F-SEN-001** (the sentinel's own `Committed` event being replayed away). This one is about the effect/resume half of the state, which no snapshot ever covers at the moment the effect starts.
- **Not the same as CORE-H5/H10.** H5 is duplicate _actions_ from replay (too many); H10 is _stale_ resumes (arriving late). This is the missing third case: resumes that are neither replayed nor delivered.

## Remediation options

1. **Commit the resume.** Give `handle_resume` its own commit at the current `latest` block — the status already carries it (`Status::BlockPending{pending}` / `BlockEvents{latest}`). Then a rollback to any block at or after the effect's block preserves the resume, and a rollback below it replays the event and re-spawns the effect. Tradeoff: one extra SQLite write per resume, and `Status::Initialized`/`WarpEvents` need a defined block to commit at.
2. **Persist the pending-effect set.** Store spawned-but-unresumed effects in a table keyed by `(block_number, effect)`, delete on resume, and re-spawn everything still present at startup and after each rollback. Requires `Effect: Serialize` (a new bound on `Service`), but it is the only option that also covers effects lost to a crash mid-flight, and it makes the at-least-once contract real rather than aspirational.
3. **Cheapest, weakest:** re-emit effects from the anchor block by replaying it — i.e. anchor the rollback at `uncle - 2` rather than `uncle - 1` — which converts loss into duplication, the case services are already required to tolerate. Costs one extra retained snapshot and one extra block of replay per rollback; does nothing for effects older than the window.
4. If none of the above is taken, the contract text in `state/mod.rs:59-62` and `effects.rs:21-24` must be corrected to say that an effect's result may be discarded without the effect being re-performed, so services know they must reconcile pending work from chain state on every `NewBlock` rather than trusting a resume to arrive.

Tests to add: a `state/mod.rs` test that commits block 1, applies a resume, commits block 2, then `uncle(2)` and asserts the resume is gone from the live state while block 1's event is not replayed (the existing `resume_updates_live_state_without_committing_a_snapshot` test already builds two thirds of it); a driver-level test that a restart across a resume re-emits nothing. No code is committed.

## Trail

- Reviewer R2: drafted from leads CORE-H5/CORE-H10 and core checklist item 10, self-estimate 80%. The ordering property (basis rows 2-4) was derived from the code in this session, not from the analysis file, which treats only the stale-resume direction (H10, 40%).

## Critic (C-CORE-B)

Per the brief I re-derived the snapshot/spawn/resume ordering from `state/mod.rs`, `state/storage.rs`, `effects.rs`, `driver.rs` and `index/blocks.rs` **before** reading the reviewer's Claim. My independent derivation:

1. `handle_update`'s `Update::Logs` arm applies every log's transition and then commits, `self.snapshots.commit(blocks.last, &state).await?` (`state/mod.rs:236`), **before** returning `commands`. The driver only reaches `self.effects.spawn(effect)` afterwards (`driver.rs:266-274`). So the snapshot recording "effect pending" is durable before the effect starts.
2. `handle_resume` (`state/mod.rs:250-258`) mutates the live state and commits nothing; the resume first becomes durable in the snapshot for the next committed log range.
3. `snapshots.reorg(uncle)` deletes every row `>= uncle` and restores the row at `uncle - 1` (`state/storage.rs:124-143`); the state machine then replays from `uncle` (`state/mod.rs:182-189`), so the block at the anchor is _restored_, never _re-applied_.

Therefore, for an effect emitted at block `n` whose resume first lands in the snapshot for block `m > n`, any rollback anchored in `[n, m-1]` restores "pending" with nothing running and no re-emission. That is exactly the reviewer's mechanism, reached independently. The end state in the sentinel is real: `handle_block_advance` `retain`s a `WaitingForEngineCheck` entry away at its deadline with no action, no `warn` and no metric (`crates/sentinel/src/service.rs:393-399`, re-read), so the request is dropped without a commit and without a vote.

### Per-claim verdicts

All ten basis rows **Supported**; I re-opened every citation and every quote matches this checkout byte-for-byte, including the two test quotes (`state/mod.rs:450-464` and the `reorg_rolls_back_to_the_common_ancestor` test at `:550-581`) and the `blocks.rs:255-266` synthetic-uncle block. `max_reorg_depth`'s default really is 5 (`crates/core/src/index/blocks.rs:69-70, 83`), so the Trigger's parameters are right. Nothing here is `H`.

### Correction to the Trigger's framing — the "every restart" claim

The **reorg variant is Confirmed as written**. `Uncle{q}` anchors at `q-1`; an effect emitted at `q-1` whose resume was first persisted in the snapshot for `q` is lost, and `q-1` is not replayed. Single-block reorgs on Gnosis are inside A5's must-handle set, so no adversary is needed.

The **restart variant is narrower than "Every restart"**, and the finding should not be read as claiming a loss on each restart. `BlockWatcher::initialize` does emit `Uncle{safe+1}` on every restart with more than one retained snapshot (`blocks.rs:261-266`) — that part is exact — but the same rollback then **replays blocks `safe+1 … latest`, which re-runs their transitions and therefore re-emits their effects**. Loss requires the effect's emission block to be at or _below_ the anchor while its resume landed above it, i.e. `n <= MIN(snapshots) < m`. With snapshots pruned to roughly `head - max_reorg_depth` and a resume that lands within a block or two, that is a one-to-two-block coincidence per effect, not a per-restart certainty. The reviewer's own step 2 pins the effect to `A = H-5` (the anchor) and is honest about it; the section heading "deterministic, no reorg needed" and the Claim's "**Every restart**" bullet overstate the frequency and should be re-worded. Basis row 9's "every effect still in flight when the process stopped … is both un-run and un-recorded" is likewise true of the _record_, but such an effect is normally re-emitted by the replay unless it too was emitted at or below the anchor.

The finding is not weakened by this — an unrecoverable, silent, per-request vote loss on a routine one-block reorg is the substance — but the certainty must reflect the corrected trigger.

### Finding verdict

**Confirmed — 78%.** Mechanism `E2` and independently re-derived; the reorg trigger is verified in code; the impact chain into the sentinel is verified in code. Held below 85 by the overstated restart framing and by the fact that the coincidence window (`m - n` blocks) cannot be measured without running the services (`E1` unreachable, `state/baseline.md` §2).

**Severity: Medium (unchanged).** Not High: the loss is per-request, not per-node, no attacker can steer which effect sits on the anchor block, and both services bound the damage with a deadline. Not Low: A5 makes shallow reorgs a case the system must handle correctly, the runtime's own contract (`state/mod.rs:59-62`, `effects.rs:21-24`) promises at-least-once and does not deliver it, and the failure is completely silent.

### Dependence of the downstream findings — they do **not** depend on this one

I read the four claims. None rests on F-CORE-031's ordering property, so a refutation here would not have moved any of them:

- **F-SEN-001** requires the replay to _re-spawn_ `Effect::EngineCheck` so the replayed `Committed(self)` arrives while the entry is back in `WaitingForEngineCheck`. That is the case where the effect **is** re-run — the opposite of this finding. The two are independent and, on the same block, mutually exclusive.
- **F-SEN-002** is about `committed_count` starting at 0 and `Committed` logs seen in other phases being discarded (`service.rs:307-319, 372-374`). No effect/resume ordering involved.
- **F-SEN-003** is about the warp arm applying no transition and emitting no `NewBlock` (`state/mod.rs:173-181`), which I re-read and confirm. Unrelated mechanism.
- **F-VAL-061** is about the validator's own handler mapping every effect error to `Resume::Noop`. It is the near neighbour of **F-CORE-032** (a resume that never arrives), not of this finding.

So the "four findings weaken with it" premise is wrong in both directions: F-CORE-031 carries its own weight and nothing else is propped up by it.

### Cross-reference the reviewer left open

R2's log (§4.1, CORE-H5) defers the duplicate-action half of replay to R3, and R3's log (item 28) defers the replay half back to R2 — neither filed the core-side gap that `TransactionStorage::enqueue` is an unconditional `INSERT` with no idempotency key (`tx/storage.rs:96-100`). The service-side consequences are filed as F-SEN-006 and F-VAL-065; I have promoted the core-side gap as **F-CORE-067** so the fix has a home in `core`.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No PoC written for this finding — my PoC budget went to the seven findings I was assigned in priority order — but the remediation check below is the reason this section exists, and it is the one C-CORE-B asked for.

**Certainty: unchanged at 78%.** I add no evidence for or against the mechanism.

### Remediation check — option 1 is unsound and introduces permanent log loss

**Option 1 ("give `handle_resume` its own commit at the current `latest`") should not be implemented.** It breaks a documented invariant and, worse, creates a new failure mode strictly more severe than the one it fixes.

The invariant it breaks is stated on the method itself (`crates/core/src/state/mod.rs:246-249`):

> Handles an effect resume **without committing a state snapshot**. Resume transitions update the live state immediately. Their state is persisted with the next successfully processed log range.

The new failure mode is the reason that invariant exists. A snapshot is only ever committed at `blocks.last` **after** every log in that range has been applied (`:200-239`, the commit at `:236` follows the per-log loop at `:213-223`). But `handle_resume` can be called while the status is `Status::BlockEvents { latest: n }` — i.e. after `Update::Block(New { number: n })` has run the block transition and **before** `Update::Logs(n..=n)` has delivered block `n`'s logs. Committing at `latest` in that window writes a snapshot for block `n` whose state is missing block `n`'s events.

Now crash. On restart, `StateMachine::with_init` reads `snapshots.current` → `(n, partial_state)` and sets `Status::BlockPending { pending: n + 1 }` (`:129-137`). `BlockWatcher::initialize` resumes from `indexed.latest = n` and **never re-fetches block `n`'s logs**. They are lost permanently, and the snapshot records the block as fully processed — which is exactly F-CORE-002's failure signature, reached from a different direction. Trading a bounded, replayable resume loss for unbounded silent event loss is not an improvement.

If option 1 is wanted anyway, it is only safe when committed at a block whose logs are known to be applied — i.e. gated on `Status::BlockPending { pending }` and committed at `pending - 1`, never under `BlockEvents` or `WarpEvents`. The option's own aside that "`Status::Initialized`/`WarpEvents` need a defined block to commit at" is half of this; the `BlockEvents` case is the dangerous half and is not mentioned.

**Because option 1 is unsound, three other findings that reach for it must be redirected:** F-SEN-001 option 3, F-SEN-015 option 3 and F-VAL-061's neighbourhood all cite "snapshot the resume" as a fix. Each has a better local option (F-SEN-001 option 1, F-SEN-015 option 1); the report should say so rather than letting three findings point at the same unsound change.

**Option 2 (persist the pending-effect set) is the sound one**, and it is the only option that makes the at-least-once contract in `state/mod.rs:59-62` and `effects.rs:21-24` true rather than aspirational. Two costs the text names and one it does not:

- `Effect: Serialize` becomes a bound on `Service`. Both in-tree services can satisfy it, but note the sentinel's `Effect::EngineCheck` carries a `SafeTransaction` (a `sol!`-generated type), so the bound reaches into generated code.
- The unstated cost: **re-spawning on every rollback multiplies effects**, and while the contract permits that for effects, several service handlers are only _nearly_ idempotent — see F-VAL-030 and F-VAL-061, whose whole subject is what a re-run or a failed `NonceTree` does. Option 2 must land together with F-VAL-061 option 1 (a `Resume::Failed` variant), or a re-spawned effect that fails is silently converted to `Resume::Noop` and the re-spawn buys nothing.

**Option 3 (anchor the rollback at `uncle - 2`) is unsound in this codebase and should be withdrawn.** Its premise is that it "converts loss into duplication, the case services are already required to tolerate". That premise is false for **actions**: F-CORE-067 establishes that the at-least-once language covers effects only, that `Command::Action` has no replay contract, and that `enqueue` has no de-duplication and no unique constraint. Option 3 would add one more replayed block to **every** rollback, i.e. strictly more duplicate onchain transactions, on every restart and every reorg. It trades a core defect for a worse one in the same crate.

**Option 4 (correct the contract text) is right and should land regardless**, exactly as F-CORE-067 option 4 says for the action half. The two doc changes are the same paragraph and should be written together.

### The three-way check C-CORE-B asked for: F-CORE-031 / F-VAL-030 / F-VAL-061

C-CORE-B's note is that these are three distinct defects sharing one hazard — anticipatory state written before an unguaranteed effect — each needing a different fix. I checked the three remediation sets against each other. **They do not contradict, but the report must say two things or the team will get it wrong.**

**1. Fixing F-CORE-031 does not fix the other two, and the natural reading is that it does.** The three fixes act at different layers:

| Finding | Layer | What its fix guarantees |
| --- | --- | --- |
| F-CORE-031 | **delivery** | the effect is re-spawned / its resume survives a rollback |
| F-VAL-030 | **state shape** | a `None` chunk reservation stops being counted as 1024 usable nonces |
| F-VAL-061 | **failure policy** | a _failed_ effect reports failure instead of becoming `Resume::Noop` |

F-VAL-061's trigger is deterministic and is **not** a delivery problem: `Effect::NonceTree` reaches an unstarted `NonceGenerator` and returns `Error::Unavailable` on the first `NewBlock` after every restart, because `handle_nonce_topup` is ordered before `handle_group_reconciliation` and the driver spawns both concurrently. Re-spawning a deterministically failing effect fails again. F-VAL-030's trigger B is the same. **So F-CORE-031 option 2, taken alone, leaves both validator findings fully open** — and because it would re-spawn `NonceTree` on every rollback, it would generate a stream of `Resume::Noop`s that look like successes in `effects_total{result}` (whose own docs say `Success` covers "an expected no-op", `metrics.rs:91-92`). It would make the validator defect _harder to see_.

**2. The one fix that is sound under the documented contract, and subsumes the most, is F-VAL-061 option 2 (self-healing state).** "On `NewBlock`, re-emit the effect for any epoch whose state still shows the placeholder" assumes nothing about delivery, nothing about ordering, and nothing about failure reporting — it re-derives the need from durable state on every block, which is exactly what the contract's "effects may be performed more than once … resume ordering is undefined" leaves a service free to do. It subsumes F-VAL-030 option 1 (the same re-emission, stated for one effect). Neither assumes exactly-once delivery, and both explicitly note the target effects are idempotent.

**No contradiction found, with one ordering constraint.** F-VAL-030 option 2 (exclude `None` reservations from `available`) and F-VAL-061 option 2 (re-emit on `NewBlock`) interact: with both, the shortfall is visible _and_ repaired, which is correct. F-CORE-031 option 2 layered on top adds a third re-emission path for the same effect; for `NonceTree` that means an occasional wasted chunk, which F-VAL-030 option 1 already accepts as a cost. Redundant, not contradictory.

**Recommended sequencing for the shared hazard:**

1. **F-VAL-061 option 1** (`Resume::Failed`, or `Result<Resume, _>` from the handler) — nothing else is diagnosable until a failed effect stops being indistinguishable from a successful no-op. It keeps transitions pure, because the resume value still carries the whole outcome.
2. **F-VAL-061 option 2 / F-VAL-030 option 1** (self-healing state), plus **F-VAL-030 option 2** (stop counting a placeholder as capacity).
3. **F-CORE-031 option 4** (correct the contract text), merged with F-CORE-067 option 4.
4. **F-CORE-031 option 2** (durable pending-effect set) only if the at-least-once contract is genuinely wanted as a core guarantee — and only after step 1, or it amplifies silent failures.

**Not recommended at any point:** F-CORE-031 option 1 (unsound, see above) and option 3 (worsens F-CORE-067).

## In-flight impact (AS-PRUNE)

**Pertains to unmerged branches, not to `main`.** Assessed against the Scheduled Secret Pruning stack (`origin/prune/end`, PRs #906–#913), built from a `git archive` extraction; no branch was merged or checked out. **Effect: unchanged by this stack.** `crates/core/src/state` is untouched; the effect spawn and `next` paths only gained housekeeping code. (PR #915 does change this finding's shape — see the `AS-SEN` section.)

## In-flight impact (AS-SEN)

**Pertains to unmerged branches, not to `main`.** Assessed against `origin/fix/sentinel_deadlines` (PR #914) and `origin/feat/optimistic_block_transition` (PR #915, tip `b2aad06`), built from a `git archive` extraction. Both apply cleanly onto `main` — `main`'s crates are byte-identical to their base `199629e`, so a verdict on the tip is also the verdict for the tip merged into `main`. **Effect: changed shape (#915).** The snapshot is now committed before the early transition (`crates/core/src/state/mod.rs:283-287`), so a rollback re-runs `NewBlock(N+1)` and effects from **block transitions** are re-emitted. Effects from **logs** are still lost: a probe after `Uncle(3)` shows `resumes: []` with no `Effect(10)` re-emitted. #915 also adds a second case on the warp path — filed as `F-SEN-016` D2.

## Reconciliation (run 2)

**Final: Confirmed, High, 92 (E1), canonical for the core mechanism** (`F-SEN-001` / `F2-SEN-001` canonical for the sentinel loss). Rediscovered and extended by run 2 as `F2-CORE-030` (EXTENDS): same commit-before-spawn ordering, plus an executed extension that overturns this file's Critic note that the replay case is "mutually exclusive" with the loss — when the emitting block _is_ replayed, the replayed own `Committed` is applied synchronously before the re-spawned effect can resume (`driver.rs:261` vs `:278`), so the replayed case loses too, and with ≥ 25 s of downtime the warp page covers the whole pre-shutdown window (`poc/F2-CORE-030`, restart variant with the real `BlockWatcher`; sentinel PoC re-run at `fe9e84c` by QA2-SEN-Δ, same result). The narrowed "one-to-two-block coincidence" restart framing therefore does not hold; the loss is deterministic for every request whose proposal and own commit landed in the last `max_reorg_depth` blocks before a restart. Severity raised Medium → High: irreversible bond loss under routine operation with no attacker (`state/run2/reconciliation/core.md` §1.1). Remediation: both runs converge on a `pending_effects`/restored hook after every restore (this file's option 1 = `F2-CORE-030` option 1); run 2 adds that it must be paired with the sentinel accepting its own `Committed` from `WaitingForEngineCheck`. Anchor note: `driver.rs`/`effects.rs` lines shifted by the housekeeping hook; mechanism unchanged. Counterparts: `F2-CORE-030`, `F2-XC-050` (the same block-not-replayed gap reached through a single retained snapshot).
