# F3-CORE-032 A rollback to a snapshot taken before an effect resumed never performs that effect again

| Field | Value |
| --- | --- |
| Status | Final: Accepted |
| Crate and module | core, driver.rs and state/mod.rs (consequences in validator state/keygen.rs and sentinel service.rs) |
| Location | crates/core/src/driver.rs:261-279, crates/core/src/state/mod.rs:257, :278-290 (related: crates/core/src/index/blocks.rs:315-320, crates/core/src/state/storage.rs:124-143, crates/sentinel/src/service.rs:136-151, crates/validator/src/state/keygen.rs:41-54, :1012-1039) |
| Severity | Low / Medium |
| Certainty | 93% (CA3: evidence checked; the live-reorg route's consequence executed through F3-XC-160) |
| Assumptions involved | A5 |
| Tags | crash-consistency, reorg, known |

## Claim

`StateMachine::handle_update` commits the snapshot for a block inside the same call that applies the block's transitions, and `Driver::update` spawns the effects those transitions return only afterwards. So the snapshot of block `B` never holds the resume of an effect emitted by `B`'s events (nor, during a warp, of any effect emitted in the page `B` closes), and later snapshots hold it only once the resume has arrived. A rollback, either the restart's synthetic `Uncle { S + 1 }` or a live `Uncle { B + 1 }`, restores such a snapshot and replays from the next block, so the emitting transition never runs again and the effect is never re-spawned: the state waits for a resume that cannot arrive. The core docs say the opposite ("Effects may be performed more than once for the same chain message, for example after a crash or reorg replay", crates/core/src/state/mod.rs:64-65).

This is the core-level cause of the sentinel item `settled.md` already lists (#973, #961: in-flight checks lost on restart, a commit ignored while the request waits for its check). It adds two things: the check does not have to be in flight when the service stops (a check that finished and whose `Commit` already landed is lost the same way, so the sentinel's own commit is then ignored and never revealed), and validators are affected too.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The snapshot is committed inside `handle_update`, before the commands are returned | E2 | crates/core/src/state/mod.rs:257 | `self.snapshots.commit(blocks.last, &state).await?;` |
| 2 | Effects are spawned only after `handle_update` returned | E2 | crates/core/src/driver.rs:261, :272-279 | `let commands = self.state.handle_update(update).await?;` / `...` / `for command in commands {` / `match command {` / `...` / `state::Command::Effect(effect) => self.effects.spawn(effect),` |
| 3 | Resumes change live state only | E2 | crates/core/src/state/mod.rs:278-289 | `/// Handles an effect resume without committing a state snapshot.` / `...` / `.apply_transition(state, Message::Resume(resume));` / `*lock = Some((state, status));` |
| 4 | A restart restores the oldest retained snapshot `S` and replays from `S + 1` | E2 | crates/core/src/index/blocks.rs:315-320; crates/core/src/state/storage.rs:124-142 | `let uncle = indexed.safe.checked_add(1);` / `...` / `self.queue.push_back(BlockUpdate::Uncle { number: uncle });` ; `let parent = uncle.checked_sub(1).ok_or(Error::BlockNumberOverflow)?;` |
| 5 | Executed with the real `StateMachine` and `Driver`: an effect emitted by an event in block 1001 (the restore point) is performed 0 times after the restart and every later snapshot still waits; the same event in block 1002 is performed again and the state recovers | E1 | rust-audit/state/run3/logs/R2-lost-effect-on-restart.txt (harness rust-audit/state/run3/logs/R2-harness-main.rs.txt) | `===== event in block 1001` / `run 2: effects performed after the restart: 0` / `after run 2: snapshot 1008: waiting_for_resume = true` / `===== event in block 1002` / `run 2: effects performed after the restart: 1` / `after run 2: snapshot 1008: waiting_for_resume = false` |
| 6 | The same run shows that a snapshot after the event block can also lack the resume (the driver took the next queued block before the resume), so the exposed window can be longer than one block | E1 | rust-audit/state/run3/logs/R2-lost-effect-on-restart.txt | `===== event in block 1002` / `...` / `after run 2: snapshot 1003: waiting_for_resume = true` / `after run 2: snapshot 1004: waiting_for_resume = false` |
| 7 | Sentinel: the proposal event itself emits the engine check | E2 | crates/sentinel/src/service.rs:136-151 | `state.0.insert(` / `request_id,` / `RequestState::WaitingForEngineCheck {` / `...` / `vec![Command::Effect(effect::Effect::EngineCheck {` |
| 8 | Validator: the genesis `KeyGen` event starts key generation, whose setup is an effect; genesis has no deadline, and a rollover round whose setup never resumes is abandoned | E2 | crates/validator/src/state/keygen.rs:52-53, :1164-1168, :1012-1017 | `// The genesis group generation is not subject to a rollover deadline.` / `self.start_key_gen(state, EpochId::Genesis, &self.genesis, None)` ; `vec![Command::Effect(Effect::KeyGenSetup {` ; `// A commitment round every participant committed to, but whose own key` / `// generation setup never resumed, cannot be restarted: ...` / `// Skip the epoch instead.` |

## Trigger

Default config (`max_reorg_depth` 5). The restart restore point is `S = L - 5` for a service that last processed block `L`; a live one-block reorg `Uncle { B + 1 }` restores `B`.

- Sentinel (known mechanism, new trigger): a `TransactionProposed` for the configured oracle lands in block `S`. However fast the engine answers, the check starts only after snapshot `S` is committed, so `S` never holds its result; a slow check (up to 18.75 s with the sample `voting_window` 6) also leaves the next few snapshots without it. By the time the service has processed `L = S + 5` and is redeployed, its `Commit` has landed or sits in the persistent queue. After the restart the request is back in `WaitingForEngineCheck` with no check running; the replayed (or later landing) own `Committed` is "ignoring unexpected commitment" (crates/sentinel/src/service.rs:316-326); after the commit deadline the request is dropped (:402-407); no reveal, the bond waits for a manual `finalize` and `claim` (#961). Every restart has one such block `S`, so the chance per restart is the chance that a proposal for the oracle landed in that block.
- Validator genesis: a restart (or live reorg) whose restore point is the block with the genesis `KeyGen` event leaves `CollectingCommitments { secrets: None }` with no deadline. The validator's own commitment is replayed from chain data, but it can never produce its shares: genesis stalls. Genesis-only liveness, A16 (Informational).
- Validator rollover after a warp: since #1017 every warped block runs `NewBlock`, so a key generation that started inside the last warp page emits `KeyGenSetup` after that page's snapshot. A restart within about 5 blocks of the warp's end restores that snapshot; the setup never resumes, the round is abandoned at its deadline (keygen.rs:1012-1039) and the validator sits out that epoch.

## Considered and rejected

- Live effects emitted by the optimistic `NewBlock(B + 1)` are safe: they are spawned right after block `B`'s commit and resume about one block before `B + 1`'s snapshot, and a rollback to `B` replays `NewBlock(B + 1)` (crates/core/src/state/mod.rs:259-268).
- Validator nonce effects are mostly covered: a lost `GenerateNonces` or `UseNonces` already queued its action in the persistent transaction queue, and the validator's own `SignCommittedNonces` replays from chain data.
- Not A17: no out-of-band database access; restarts and reorgs within `max_reorg_depth` are in scope (A5).
- Pre-existing for event-emitted effects (`fe9e84c` has the same commit-then-spawn order, `driver.rs` is unchanged since then). The warp path for `NewBlock`-emitted effects is introduced by #1017.

## Remediation options

1. Core: keep the effects still waiting for a resume in the snapshot (for example a list next to the state, removed by the matching resume), and re-spawn them after a restore. Fixes both services and both triggers; effects are already idempotent by design (stored verdicts, first-stored DKG secrets, take-once nonces).
2. Services: re-emit the effect from `NewBlock` while the state still waits for it (sentinel `WaitingForEngineCheck` re-emits `EngineCheck`, which reuses the stored verdict; validator `CollectingCommitments { secrets: None }` re-emits `KeyGenSetup`, which reuses the stored secrets). This is the shape #973 proposes for the sentinel.
3. Document the replay boundary next to `Command::Effect` instead of the current claim, if the team keeps the behaviour.

Tests to add: the harness scenario as a `state` or driver test (event at the restore point, restart, assert the effect runs again). No code is committed.

## Trail

- 2026-10-08 Reviewer R2: drafted, self-estimate 85% for the core mechanism (executed), 70% for the service consequences (code-traced). Tagged `known` because the sentinel side is #973 and #961; severity Low because the validator side is either genesis (A16) or one skipped epoch.

## Critic (C3-CORE, 2026-10-08)

| Row | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | `crates/core/src/state/mod.rs:257`. |
| 2 | Supported | `crates/core/src/driver.rs:261, 272-279`. |
| 3 | Supported | `state/mod.rs:278-289`. |
| 4 | Supported | `crates/core/src/index/blocks.rs:315-320`; `crates/core/src/state/storage.rs:124-142`. |
| 5 | Supported | `rust-audit/state/run3/logs/R2-lost-effect-on-restart.txt`, same harness and source hash as F3-CORE-030. Run 1 applies the resume after the commit, which is what `Driver::update` does, since it spawns effects only after `handle_update` returns. |
| 6 | Supported | Same log, block 1002 case. |
| 7 | Supported | `crates/sentinel/src/service.rs:136-151`; the ignored own commit is `service.rs:316-326` and the drop after the deadline `service.rs:402-407`. |
| 8 | Supported | `crates/validator/src/state/keygen.rs:52-53, 1012-1017, 1164-1168`. |

Finding verdict: Confirmed. Severity: Low to Low, tag `known` (#973, #961). Certainty: 90, band 90 to 100 (the core mechanism is E1 through the real `StateMachine` and `Driver`; the service consequences are E2). Duplicate: canonical. Related, not a duplicate: F3-SEN-004 (C3-SEN's set) loses the sentinel's own commit for proposals newer than the restore point, because a warp page applies the commit before the replayed check resumes; this file covers the restore point itself, where the check is never re-run.

Confirmation, with two precisions.

- Live reorgs lose only resumes that already landed. An uncle does not touch the `EffectManager` (`driver.rs:237-297` has no cancel or clear), so an effect still running when `Uncle { B + 1 }` arrives resumes into the restored state and nothing is lost. The live trigger therefore needs the resume to land between the commit of `B` and the uncle, for example a fast engine verdict followed by a one-block reorg. A restart loses both kinds, since the process ends.
- The rollover window after a warp is shorter in wall-clock time than "about 5 blocks" suggests. The blocks between the warp's end (`L - 5`) and the startup head `L` are already mined at startup, and the warp-end snapshot stops being the restore point when the watcher's safe block moves past it, at the first block above `L` (`blocks.rs:509-518`, `driver.rs:263`, `storage.rs:151-161`). A second restart within about one block time of the catch-up is needed, for example a crash loop or a double deploy. When it happens, the cost is wider than the validator sitting out: its own commitment is replayed from chain data, so it skips the epoch (`keygen.rs:1012-1039`), while its peers wait for its shares until the share deadline and then restart without it (`keygen.rs:1062-1083`), one `key_gen_timeout` (120 blocks) of delay for the whole group.

QA recipe. The core mechanism is E1 (re-runnable with `r2-poc lost-effect 1001` and `1002`). For the sentinel consequence, a second reproduction inside `crates/sentinel/src/service.rs` tests: apply `TransactionProposed` at block `S` and keep that state as the snapshot; apply the engine resume, then the own `Committed` at `S + 2`; restore the `S` snapshot and replay blocks `S + 1` to `S + 7` (`NewBlock` plus the own `Committed`). Expected: the warn "ignoring unexpected commitment", the request dropped at `NewBlock(S + 7)` (commit deadline `S + 6`), and no `Reveal` action. Bite check: re-emitting `EngineCheck` from `NewBlock` while the request waits (remediation 2) produces the resume and then a `Reveal`.

## QA (QA3-CORE, 2026-10-08)

Result: Reproduced.

Route: (1) R2's harness binary re-run unchanged: with the effect-emitting event in block 1001 (the restore point) the effect is performed 0 times after the restart and snapshots 1003 to 1008 all still wait for it; with the event in 1002 it is performed again. This run left snapshots 1003 to 1006 waiting and recovered at 1007 (the finding's log recovered at 1004), so the exposed window varies with timing. (2) The sentinel consequence through the real core `StateMachine` (SQLite snapshots, a real `Uncle` rollback) with the real `SentinelTransition` and Aegis amounts (fee 0.40, bond 800, slash 0.40, in 6-decimal base units): `TransactionProposed` and `NewRequest` in block S, the engine verdict resumed after S's snapshot as `Driver::update` does, the sentinel's own `Committed` in S + 2. Without a rollback the run emits `Reveal` at S + 7. With `Uncle { S + 1 }` and a replay of S + 1 to S + 7, the only commands after the verdict are the original `ApproveToken` and `Commit`: no `Reveal`, and the request is gone from the newest snapshot while its bond stays committed.

Command and output: `poc/F3-CORE-032/` (`r2-harness-rerun.txt`, `sentinel-test-at-361aaf3.txt`).

Bite check: a minimal remediation 2 (a `ReplayVerdict` effect emitted on every block while a request waits for its check, answered from the `VerdictStore`, the driver's handling emulated in the test). When the replayed verdict resumes before the own commit is replayed, the request reaches `Reveal`. When the commit is replayed first, as a warp page or a fast replay does, it is still ignored and there is still no `Reveal`; after a restart with more than about two blocks of downtime, S + 1 onwards arrives in one warp update, so that is the usual case. Remediation 2 is therefore incomplete unless the transition also records an own commitment seen while the request waits (the F3-SEN-004 piece). It also changes the `NewBlock` command stream: the existing test `waiting_engine_checks_expire_using_the_available_deadline` fails with it. Remediation 1 (core) not tried: it needs a way to match a resume to its effect, which the core API does not have.

Certainty: 93 (core mechanism E1 twice, sentinel consequence E1 through the real state machine, Critic Confirmed).

## Practicality (PC3, 2026-10-09)

| Field | Value |
| --- | --- |
| Actor | Nobody hostile: a restart (or a live reorg) whose restore point holds an effect-emitting event |
| Cost | n/a |
| Condition | Sentinel: a proposal for the oracle in the restore-point block S = L - 5 of a restart (or within the next few blocks when the engine check is slow), with this sentinel's own commit landed (Batched mode on Aegis today, every mode once `F3-SEN-001` is fixed). Validator: a second restart within about one block of a warp that ended inside a key generation start (Critic), or the genesis case (A16) |
| Gate | Gates 1 and 2 pass (core mechanism E1 twice, sentinel consequence E1 through the real `StateMachine`). Gate 3: no attacker. Gate 4: per affected sentinel request 0.40 USDC.e slashed and 799.60 USDC.e locked until a manual `claim`; on the validator side one skipped epoch, or genesis only |
| Practicality | conditional (a restart whose restore point holds a proposal the sentinel committed to; for validators, two restarts in quick succession around a warp) |
| Recommended severity | Low, tag `known` (#961, #973) (reviewer, Critic and QA agree) |
| Intended route and parameters | Real core `StateMachine` with SQLite snapshots and a real `Uncle` rollback, the real sentinel transition and the Aegis amounts; the restart itself is modelled by the restore |
| Cheapest settling change | None needed |

Reason. Per restart, the sentinel is exposed only if a proposal it committed to sits in the one or few restore-point blocks, so the chance per restart is small at any realistic Aegis proposal rate; the wider window for the same lost reveal is `F3-SEN-004` (Medium). The run-1 row `F-CORE-031` (ledger High) and `F-SEN-001` describe this same mechanism; their band should be set from the same practicality, which on Aegis is a recoverable bond lock per rare restart, not a High.

## Confidence (CA3, 2026-10-09)

| Field | Value |
| --- | --- |
| Result | Evidence holds. Final: Accepted, Medium, `known`, certainty 93 |
| Evidence check | `poc/F3-CORE-032/r2-harness-rerun.txt` shows the snapshot at the restore point waiting for a resume that never comes after run 2; `sentinel-test-at-361aaf3.txt` shows the sentinel consequence. The live one-block route the Claim names runs through the same rollback: in `F3-XC-160`'s end-to-end run, which I re-ran, `Uncle { P + 1 }` restores P's snapshot, the resume is lost and the own commit is ignored |
| Final severity | Medium, `known`. I disagree with PC3's Low: its practicality weighs only the restart route. The Claim also covers a live `Uncle { B + 1 }`, and Gnosis orphans one-block slots every day (0.03 to 0.08 % per PC3's consult), so any request whose next block is orphaned at the node loses its resume, for every sentinel that committed, with no special endpoint: of the order of one request in two thousand. That is the consequence `F3-XC-160` and `F3-SEN-004` carry at Medium, and Section 8 names crash-consistency gaps with recoverable impact as Medium. Not High: recoverable by a manual `claim` |
| Certainty | 93 (unchanged) |
| Evidence | existing `poc/F3-CORE-032/`; `poc/F3-XC-160/ca3/f160-inject.txt` |

## Reconciliation (run 3)

| Field | Value |
| --- | --- |
| Relation | CONFIRMS `F-CORE-031` (run 2's `F2-CORE-030` is folded there). Its restore-point case also covers `F-SEN-011`'s in-flight check, which keeps its own Low row (a missed vote, no bond at stake) |
| Canonical | `F-CORE-031` |
| Ledger values | Medium, `known` (CA3's one band for `F-CORE-031`, `F-SEN-001`, this file and `F3-SEN-004`; the row was High), certainty 93 (this file's CA3 value) |
| Source | Ledger: [`report/RECONCILIATION.md`](../report/RECONCILIATION.md) sections 2 and 3 |
