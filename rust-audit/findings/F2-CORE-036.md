# F2-CORE-036 A log range's actions and effects are dispatched only after its snapshot is committed and pruned; when the prune leaves a single retained snapshot (every catch-up warp page, or `max_reorg_depth = 0`) a crash in that window loses them permanently

| Field | Value |
| --- | --- |
| Status | QA'd |
| Crate and module | safenet-core, driver.rs (state/mod.rs, state/storage.rs, index/blocks.rs) |
| Location | crates/core/src/driver.rs:261-290 (related: crates/core/src/state/mod.rs:236; crates/core/src/state/storage.rs:151-161; crates/core/src/index/blocks.rs:255-278, 453-462; test crates/core/src/state/mod.rs:584-644) |
| Severity | Low / Low |
| Certainty | 92% (QA2-XC, mirrored from canonical F2-XC-050; Critic C2-CORE-A set 75%) |
| Assumptions involved | A17 (service-caused crash consistency stays in scope) |
| Tags | crash-consistency |

Audited commit: `3ec8bc5`. Promoted by Critic C2-CORE-B from reviewer observations R2 O4 (45%) and R3 O2 (35%), both routed to the Critic and not filed.

## Claim

`StateMachine::handle_update` commits the snapshot of a log range _inside_ the call (`state/mod.rs:236`) and returns the commands; the driver then prunes snapshots (`driver.rs:263`) and only afterwards encodes and enqueues the actions (`driver.rs:272-290`, durable at `tx/storage.rs:93-103`) and spawns the effects (`driver.rs:278`). In steady state with `max_reorg_depth >= 1` the prune leaves at least two snapshots, so a restart rolls back to `safe` and replays the range (`index/blocks.rs:261-266`); the commit-to-enqueue window is covered. During a catch-up warp the watcher's `safe` is above every persisted snapshot, so the prune keeps only the page's own row (`state/storage.rs:152-159`; asserted by the existing test: one row, `latest == safe`), and on the next start no `Uncle` is emitted because `safe + 1 > latest` (`blocks.rs:262-264`) — the page is never replayed. The same holds in steady state with `max_reorg_depth = 0`, where the watcher's `safe` equals `latest` (`blocks.rs:453-462`) and the prune at `Logs{n}` leaves only snapshot `n`.

Consequently a crash (SIGKILL, OOM, power loss) between `SnapshotStore::prune` and the `COMMIT` of `TransactionStorage::enqueue` loses every action of that page permanently, and every effect spawned for it (or still in flight) as well — the state says the work was done, the queue never heard of it. Warp pages occur on every restart with at least one block of downtime (`blocks.rs:271-278`) and on every fresh start from `start_block` (`blocks.rs:279-289`). The window is milliseconds and not attacker-controlled; the loss is permanent and silent. A lost action is a missed on-chain step (a validator's nonce registration, a sentinel commit or reveal) whose event is never re-delivered; a lost effect produces the stuck state described in F2-CORE-030.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The snapshot is committed before the commands are returned to the driver. | E2 | crates/core/src/state/mod.rs:236-238 | `self.snapshots.commit(blocks.last, &state).await?;` `(state, status, commands)` |
| 2 | The driver prunes before it enqueues actions or spawns effects. | E2 | crates/core/src/driver.rs:261-263, 272-290 | `let commands = self.state.handle_update(update).await?;` `recorder.processed();` `self.state.prune(block_status.safe).await?;` ... `state::Command::Effect(effect) => self.effects.spawn(effect),` ... `let result = self.transactions.queue(transactions).await;` |
| 3 | When `safe` is above every snapshot, prune keeps only the newest row. | E1 | crates/core/src/state/storage.rs:152-159; test crates/core/src/state/mod.rs:596-612 (passes: `cargo test -p safenet-core --lib -- state::`, this session) | `"DELETE FROM snapshots WHERE block_number < ? AND block_number < (SELECT MAX(block_number) FROM snapshots)"` ... test: `machine.prune(6).await.unwrap();` ... `assert_eq!(snapshot_count(&pool).await, 1);` `assert_eq!(machine.block_status().await.unwrap(), Some(BlockStatus { latest: 3, safe: 3 }));` |
| 4 | With a single retained snapshot the restart emits no `Uncle` and does not replay the page. | E2 | crates/core/src/index/blocks.rs:261-266 | `let uncle = indexed.safe.checked_add(1);` `if let Some(uncle) = uncle` `&& uncle <= indexed.latest` `{` `self.queue.push_back(BlockUpdate::Uncle { number: uncle });` `}` |
| 5 | A warp page is emitted on every restart with at least one block of downtime and on fresh starts from `start_block`. | E2 | crates/core/src/index/blocks.rs:271-278, 279-289 | `if let Some(uncle) = uncle` `&& uncle <= safe` `{` `self.queue.push_back(BlockUpdate::Warp { from: uncle, to: safe });` `}` ... `} else if let Some(start_block) = self.config.start_block` `&& start_block <= safe` |
| 6 | The watcher's `safe` is the block evicted from `recent`, so with `max_reorg_depth = 0` it equals `latest`. | E2 | crates/core/src/index/blocks.rs:453-462 | `if self.recent.len() as u64 > self.config.max_reorg_depth {` `let evicted = self.recent.pop_front().expect("checked len > max_reorg_depth above");` `self.safe = SafeBlock { number: evicted.number, hash: evicted.hash };` `}` |
| 7 | Enqueue is durable only at its own `COMMIT`, after the snapshot. | E2 | crates/core/src/tx/storage.rs:93-103 | `let mut tx = self.pool.begin().await?;` ... `tx.commit().await?;` |

## Trigger

A service catching up (a `Warp` page in progress after downtime, or a fresh start from `start_block`) is killed between `SnapshotStore::prune` and `TransactionStorage::enqueue`'s `COMMIT` for a page whose events produced actions. On restart `block_status()` reports `latest == safe` (single row), no `Uncle` is queued, the warp resumes at `latest + 1`, and the page's actions never reach the queue. Same with `max_reorg_depth = 0` on any `Logs{n}` update. Deterministic under fault injection (a panic after `prune` in a scratch copy, then a second `Driver` over the same pool); probabilistic in production (millisecond window). Not run.

## Considered and rejected

- "The queue's own transactionality covers it" — `enqueue` is atomic, but it starts after the snapshot commit and after the prune (`driver.rs:263`, `283`); atomicity within the queue does not order it with the snapshot.
- "The restart replay covers it" — only when at least two snapshots remain; `blocks.rs:262-264` skips the `Uncle` when `safe + 1 > latest`, and the existing test documents the single-row state after a pruned warp page.
- "Effects are at-least-once by contract" — the contract (`state/mod.rs:60-62`) applies to replayed messages; a page that is never replayed re-spawns nothing (see F2-CORE-030 for the same gap on the resume side).
- "Reordering `prune` after `queue` would fix it" — for steady state with `max_reorg_depth >= 1` there is no gap to begin with; for warps, keeping the _previous_ page's snapshot alive until the current page's actions are durable is exactly what a reorder achieves, so the fix is small (see remediation 2).
- Severity above Low — no attacker control, millisecond window; kept Low because the loss is permanent and invisible.

## Remediation options

1. Make the snapshot commit and the action enqueue one SQLite transaction: both stores share the pool, so `handle_update` can take the encoder (or return a transaction handle) and `enqueue` can run on the same `tx` before `commit`. Tradeoff: couples the state machine to the queue's storage.
2. Move `self.state.prune(..)` after the `queue`/`spawn` block in `Driver::update`, and have `SnapshotStore::prune` retain the two newest rows rather than one, so a restart always has a rollback anchor below the last committed page. Cheap; leaves a (now replayable) window instead of a lost one, at the cost of one duplicate submission (F2-CORE-063).
3. For effects, the `pending_effects`/restore hook proposed in F2-CORE-030 remediation 1 also covers this window.

Tests to add: a driver-level test is not possible today (no harness); a QA PoC with an injected panic after `prune` in a scratch copy, followed by a second `Driver` over the same pool asserting the `transactions` table is empty for the page; a `state/mod.rs` test asserting that after `prune` during a warp `block_status()` yields `latest == safe` already exists (mod.rs:609-612) and should be cross-referenced.

## Trail

- Critic C2-CORE-B: drafted as a promotion of R2 O4 / R3 O2, Plausible, 45% (E1 for the single-snapshot state via the existing test; E2 for the ordering; crash not executed). Severity Low.
- QA2-XC: Reproduced through the canonical F2-XC-050 PoC (warp page and `max_reorg_depth = 0` variants executed; `start_block` fresh start not executed); certainty 75 → 92 mirroring F2-XC-050; Status QA'd.

## Critic (C2-CORE-A)

Method: title and `Location` only, then `crates/core/src/driver.rs` (`update`, `new`), `crates/core/src/state/{mod,storage}.rs`, `crates/core/src/index/blocks.rs` (`initialize`, `status`, eviction) and `crates/core/src/tx/{mod,storage}.rs`, before reading the Claim. My own trace: for a `Logs` update the driver task runs `commit(blocks.last)` (autocommit upsert, `state/mod.rs:236`) → `prune(block_status.safe)` (`driver.rs:263`) → `spawn` effects in memory (`driver.rs:278`, `effects.rs:61-69`) → `enqueue` in its own `BEGIN … COMMIT` (`driver.rs:283`, `tx/storage.rs:93-102`, durable only at `:102`). The restart safety net is `Uncle { MIN+1 }`, emitted only while `MIN(block_number) + 1 <= MAX` (`blocks.rs:261-266`; `Driver::new` at `driver.rs:122-152` restores metrics only, `with_init` replays nothing). `prune` deletes `< safe AND < MAX` (`storage.rs:152-157`), so exactly one row survives whenever `safe >= blocks.last`: (a) every page of a warp, because the watcher's `safe` is fixed at the init anchor `latest - max_reorg_depth` (`blocks.rs:246, 333-340, 376-381`) which is the warp's `to` (`:274-277, 285-288`) and nothing moves it while queued updates drain (`:387-389`; eviction only runs in the `New` path, `:453-462`); (b) `max_reorg_depth = 0`, where eviction fires at `len > 0` so `safe == latest` after every `New`; (c) the first `Logs` after an empty store. Same picture as the author's before I read the argument.

| # | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | `crates/core/src/state/mod.rs:236-238`; `Logs` is the only arm that commits. |
| 2 | Supported | `crates/core/src/driver.rs:261-263, 272-290`, order as quoted. |
| 3 | Supported; E1 label now backed | `storage.rs:152-159`; test `state/mod.rs:596-612`. C2-CORE-B's run was not saved under `rust-audit/` (no C2-CORE-B file in `state/run2/logs/`), so as filed the row was E2 by Section 2's definition. Re-executed by me: `cargo test -p safenet-core --lib -- state::tests::warps_and_prunes_intermediate_snapshots state::storage::tests::prune` → 3 passed, log `state/run2/logs/c2-core-a-036-state-test.txt` (HEAD `92a0610`; `crates/` tree identical to `3ec8bc5` per `baseline.md`). |
| 4 | Supported | `blocks.rs:261-266`; with one row `uncle = latest + 1 > latest`, and `with_init` leaves the machine in `BlockPending { latest + 1 }`, which the following `Warp { from: latest + 1 }` matches (`state/mod.rs:173-175`). |
| 5 | Supported | `blocks.rs:271-278, 279-289`. Derivation: at shutdown `MIN = L - d`; restart warp exists iff `L - d + 1 <= (L + t) - d`, i.e. `t >= 1` new block. The `start_block` branch warps over an empty store, so its first page is single-row without any prune of older rows. |
| 6 | Supported | `blocks.rs:453-462`; `max_reorg_depth = 0` is exercised by the crate's own tests (`blocks.rs:595, 819, 1258`), so it is a supported configuration. |
| 7 | Supported | `tx/storage.rs:93-103`; `TransactionQueue::queue` calls `enqueue` first and only then `submit_pending` (`tx/mod.rs:132-141`), so the durability point is the `COMMIT` at `:102` and the action window is bounded by two SQLite statements, not by RPC. |

Considered-and-rejected entries hold. Two additions from my trace that neither author states: (i) a non-RPC `enqueue` error (`is_intermittent` is `matches!(self, Self::Rpc(_))`, `tx/mod.rs:47-54`) is propagated by `lift_intermittent_error(result)?` at `driver.rs:284` after the prune, so a `SQLITE_BUSY` past the 5 s `busy_timeout` or an I/O error in that window is the same loss without any kill — F2-XC-050 row 8 has this, F2-CORE-036 mentions it only in the title. (ii) The _effect_ half has a much wider window than "milliseconds": an effect spawned for page `k` is durable only once its resume has been applied and carried by a later `Logs` commit (`state/mod.rs:246-249`), and every intervening page's prune again leaves a single row, so during a warp a crash at any point between the spawn and that later commit loses it — the effect's whole duration (an engine HTTP call, a FROST preprocess), multiplied by the number of pages in a long catch-up. That is the F2-CORE-030 mechanism (effects of a block that is never replayed) with a different reason the block is not replayed; it belongs with F2-CORE-030 / F2-XC-050 remediation 3, not with the action fix.

Remediation check: 036's option 2 is stronger than written — moving `self.state.prune(..)` below the `queue` block is _by itself_ sufficient for the action half in all three single-row states (a crash between `commit` and `enqueue` then finds two rows and the restart `Uncle` replays the page; after `enqueue` the actions are durable), so the "retain the two newest rows" clause is redundant. It is the least invasive fix and turns the loss into the duplicate that F2-CORE-032/063 already show is tolerated. Option 1 (one transaction) is the clean fix; neither closes the effect half.

**A16/A17.** In scope under A17: the loss is produced by the service's own write ordering, no out-of-band database access. A16 lowers only a genesis-DKG instance; warp pages covering a rollover key generation and every sentinel `Claim`/`Finalize` keep their severity.

**Severity: Low / Low.** Impact is permanent and silent, but the action window is two fsync'd statements on the driver task, reachable only by a hard kill, OOM, power loss or a storage fault — the `biased` select at `driver.rs:177-198` finishes the current input on an orderly shutdown — and case (b) is non-default. Medium would be right only if the effect-side window above were judged under this ID rather than under F2-CORE-030 (High), where it already lives.

**Verdict: Confirmed. Certainty: 75%.** Mechanism traced end to end and the single-row precondition executed; the crash itself is not injected (a panic after `driver.rs:263` in a scratch copy, then a second `Driver` over the same pool, is a Phase-3 PoC), and permanence at the service layer (no re-emission of `Claim`/`Finalize`, of the DKG round actions) is taken from F2-SEN-002/003 and F2-VAL-003/005 rather than re-verified by me.

**Duplicate — same defect as F2-XC-050; canonical F2-XC-050** (agreeing with C2-XC: it carries the fatal-storage variant, the traced per-service consequences and a saved test log). Reconciliation should fold into F2-XC-050 from this file: (1) the `start_block` fresh-start variant (`blocks.rs:279-289`), a warp over an empty store; (2) the one-line remediation "prune after `queue`" as sufficient on its own for actions; (3) the effect-window precision in (ii) above, cross-referenced to F2-CORE-030; (4) the trivial third single-row state, the first `Logs` after an empty store with no `start_block`. Not edited F2-XC-050.

## QA (QA2-XC)

**Reproduced** — same defect as **F2-XC-050 (canonical)**; the outcome is recorded here for completeness and the details live there. PoC `poc/F2-XC-050/qa2_xc_state_tests.rs` (module `qa2_xc_050`, three tests; `run.txt` at `3ec8bc5`, `rerun.txt` at `fe9e84c`, `crates/core` identical): the driver's `commit → prune → (fatal) enqueue` sequence loses the page's action and the restart does not replay it in exactly the two single-row states this file names — a catch-up warp page (`restart: persisted BlockStatus { latest: 3, safe: 3 }; watcher queued [Warp { from: 4, to: 998 }, …]` … `snapshot 998 says events [10] were handled; transactions rows = []`) and `max_reorg_depth = 0` (`… watcher queued [Warp { from: 4, to: 1000 }]` … `state events [30]; transactions rows = []`), while the depth-2 control with two retained rows recovers the action through `Uncle { 6 }`. The sentinel-side consequence is executed in `poc/F2-XC-050/coverage-7.4/` (three `Claim`s emitted from one warp page, one snapshot left after `prune`, nothing re-emitted after the restart). Not executed: the `start_block` fresh-start variant this file adds (a warp over an empty store — single-row without any prune, so it follows a fortiori) and C2-CORE-A's effect-window precision (ii), which belongs with F2-CORE-030.

**Certainty: 92%**, mirrored from the canonical finding (E1, Critic Confirmed). Severity Low / Low stands.

**Remediation check.** Option 1 (one SQLite transaction for snapshot and queue rows) is sound. Option 2 as C2-CORE-A sharpened it — `prune` after the `queue` block, retention clause redundant — is sound for the warp-page and depth-0 states but does **not** cover the fresh-store first page (nothing older exists to keep), so on its own it leaves this file's own `start_block` variant open; pair it with, or prefer, F2-XC-050's option 1/2 (enqueue before or together with the commit). Option 3 is F2-CORE-030's effect hook. Nothing here breaks the `state/mod.rs` contract or touches the Solidity.

## Reconciliation (run 2)

**Final: folded into `F2-XC-050` (canonical, REC-XC's ledger row) — Low, 92 (E1).** No run-1 core counterpart: `F-CORE-031` is the resume half, `F-CORE-067` the duplicate half, and `F-CORE-039` names "SIGKILL mid-`update`" only as their enabling condition. Points from this file that REC-XC should fold into `F2-XC-050`: the `start_block` fresh-start variant, "`prune` after `queue`" as sufficient on its own for the action half, and C2-CORE-A's effect-window precision (cross-referenced to `F-CORE-031` / `F2-CORE-030`) (`state/run2/reconciliation/core.md` §1).
