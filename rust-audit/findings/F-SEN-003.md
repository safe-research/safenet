# F-SEN-003 A warp replay delivers no `NewBlock`, so reveals in the replayed range are discarded, `finalize` takes the timeout branch, and a frozen request's bond is never claimed

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | sentinel, service.rs (with core: state/mod.rs, index/blocks.rs) |
| Location | crates/sentinel/src/service.rs:347-362, 450-465, 493-501, 626-671 (related: crates/core/src/state/mod.rs:173-181, crates/core/src/index/blocks.rs:268-278) |
| Severity | Medium / Medium |
| Certainty | 72% (set by Critic C-SEN; QA may raise) |
| Assumptions involved | A2, A5, A10 |
| Tags | reorg, crash-consistency, funds |

## Claim

When the sentinel restarts after being down for more than `max_reorg_depth` blocks, the block watcher emits a `Warp{from, to}` covering the whole missed range (`core/index/blocks.rs:268-278`). The state machine handles a warp by switching to `Status::WarpEvents` and **applying no transition at all** (`core/state/mod.rs:173-181`); the logs in that range are then delivered as ordinary `Update::Logs` batches, but no `Message::NewBlock` is ever produced for a warped block. `SentinelTransition::handle_block_advance` — the only place the FSM advances phases on deadlines — is therefore not run for the entire replayed range.

Consequently a request whose anchor snapshot says `CollectingCommitments` stays in that phase for the whole warp, so every `Revealed` log in the range hits `service.rs:347-362` and is discarded, and every `DisputeResolved` hits `service.rs:493-501` and is discarded. At the first post-warp `NewBlock` the FSM finally moves to `CollectingVotes { revealed_count: 0 }` and, one block later (the reveal deadline is long past), `finalize` computes `timed_out = *revealed_count == 0` → `true` and takes the "nobody revealed" branch: it emits `Finalize` + `Claim` and **deletes the entry**.

If the request is `FROZEN` at that moment — a real possibility, because arbitration runs for up to `ARBITRATION_TIMEOUT` blocks after the reveal window closes — both transactions revert (`RequestNotPending`, `RequestNotResolved`) and the entry is gone. The eventual `DisputeResolved` / `ArbitrationTimedOut` / `DisputeOutOfScope` then finds no tracked entry and is ignored, so the sentinel never claims its bond.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | A warp applies no transition, so no `NewBlock` is delivered for the replayed range | E2 | crates/core/src/state/mod.rs:173-181 | `            Update::Block(BlockUpdate::Warp { from, to })`<br>`                if matches!(status, Status::Initialized)`<br>`                    \|\| matches!(status, Status::BlockPending { pending } if pending == from) =>`<br>`            {`<br>`                let status = Status::WarpEvents {`<br>`                    range: block_range(from, to)?,`<br>`                };`<br>`                (state, status, vec![])`<br>`            }` |
| 2 | Logs inside the warp are still applied to the FSM, in pages, each committing a snapshot | E2 | crates/core/src/state/mod.rs:213-236 | `                let (state, commands) = {`<br>`                    let mut state = state;`<br>`                    let mut commands = Vec::new;`<br>`                    for log in logs {`<br>`                        let (new_state, new_commands) =`<br>`                            self.transition.apply_transition(state, Message::Event(log));`<br>`                        state = new_state;`<br>`                        commands.extend(new_commands);`<br>`                    }`<br>`                    (state, commands)`<br>`                };` |
| 3 | A restart that missed more than `max_reorg_depth` blocks queues a warp | E2 | crates/core/src/index/blocks.rs:268-278 | `            // If possible, warp up to the reorg-safe block to allow bulk log`<br>`            // queries. We cannot warp to the latest block, as a range query`<br>`            // could then return data for a block that later gets uncled.`<br>`            if let Some(uncle) = uncle`<br>`                && uncle <= safe`<br>`            {`<br>`                self.queue.push_back(BlockUpdate::Warp {`<br>`                    from: uncle,`<br>`                    to: safe,`<br>`                });`<br>`            }` |
| 4 | A `Revealed` log arriving in a phase other than `CollectingVotes` is discarded | E2 | crates/sentinel/src/service.rs:347-362 | `        let RequestState::CollectingVotes {`<br>`            committed_count,`<br>`            revealed_count,`<br>`            approve_count,`<br>`            deny_count,`<br>`            self_revealed,`<br>`            ..`<br>`        } = entry`<br>`        else {`<br>`            tracing::warn!(`<br>`                request_id = %event.requestId,`<br>`                state = entry.name,`<br>`                "ignoring unexpected reveal"`<br>`            );`<br>`            return (state, Vec::new);`<br>`        };` |
| 5 | A `DisputeResolved` arriving in any other phase is discarded and the entry is left as-is | E2 | crates/sentinel/src/service.rs:493-501 | `            Some(entry) => {`<br>`                tracing::warn!(`<br>`                    request_id = %event.requestId,`<br>`                    state = entry.name,`<br>`                    "ignoring unexpected dispute resolution"`<br>`                );`<br>`                state.0.insert(event.requestId, entry);`<br>`                return (state, Vec::new);`<br>`            }` |
| 6 | `timed_out` is derived purely from the local (now empty) reveal tally and drives the terminal branch | E2 | crates/sentinel/src/service.rs:626-627, 656-671 | `        let dispute = *approve_count > 0 && *deny_count > 0;`<br>`        let timed_out = *revealed_count == 0;`<br>`        // ...`<br>`        let outcome_metric = if timed_out {`<br>`            ResolvedOutcome::Timeout`<br>`        } else {`<br>`            ResolvedOutcome::Unanimous`<br>`        };`<br>`        crate::metrics::requests_resolved_total(outcome_metric).increment(1);`<br>`        actions.push(`<br>`            SentinelAction {`<br>`                kind: SentinelActionKind::Claim { id: request_id },`<br>`                expires_at: None,`<br>`            }`<br>`            .into,`<br>`        );`<br>`        (None, actions)` |
| 7 | `claim` on a still-`FROZEN` request reverts, and the entry has already been deleted | E2 (Solidity reference, A7) | contracts/src/libraries/SentinelOracleRequests.sol:239-245 | `    function requireResolved(T storage self) internal view returns (State) {`<br>`        State state = self.progress.state;`<br>`        require(`<br>`            state == State.RESOLVED_APPROVED \|\| state == State.RESOLVED_DENIED \|\| state == State.TIMED_OUT,`<br>`            RequestNotResolved`<br>`        );`<br>`        return state;`<br>`    }` |
| 8 | A `FROZEN` request stays unresolved for up to `ARBITRATION_TIMEOUT` blocks after finalisation | E2 (Solidity reference, A7) | contracts/src/libraries/SentinelOracleRequests.sol:183-185 | `            if (approveMet && denyMet) {`<br>`                newState = State.FROZEN;`<br>`                self.progress.arbitrationDeadline = (block.number + arbitrationTimeout).toUint64;` |

## Trigger

`max_reorg_depth = 5`, `COMMIT_WINDOW`/`REVEAL_WINDOW` as in `scripts/run_sentinel_integration_test.sh` (5/5), `ARBITRATION_TIMEOUT = 100`:

1. Block `b`: request opened; the sentinel commits. Snapshot at `b+2` records `CollectingCommitments { self_committed: true, commit_deadline: b+5, reveal_deadline: b+10 }`.
2. Block `b+3`: the process stops (crash, node drain, host reboot). Retained snapshots span roughly `[b-2, b+3]`, so `indexed.safe ≈ b-2`, `indexed.latest ≈ b+3`.
3. The process is down for 60 blocks (about five minutes on Gnosis). Meanwhile the commit window closes, other sentinels reveal on both sides, `finalize` is called by one of them, and the request becomes `FROZEN` awaiting the arbitrator.
4. Restart at block `b+63`. `safe_now = b+58`, `uncle = b-1`. `Uncle{b-1}` rolls back to the snapshot at `b-2`; `uncle <= safe_now`, so `Warp{b-1 ..= b+58}` is queued.
5. During the warp the FSM receives only `Message::Event`. The request's anchor state is `CollectingCommitments`, so:
   - `Committed(..)` logs are tallied (harmless),
   - every `Revealed(..)` log — including the sentinel's own, if its reveal had been mined before step 2's crash — hits `service.rs:347-362` and is **discarded**,
   - the block-driven transition to `CollectingVotes` never happens, because no `NewBlock` is delivered.
6. First post-warp `NewBlock(b+59)`: `block > commit_deadline` and `self_committed` → a (now useless) `Reveal` action is emitted with `expires_at = b+10`, which the transaction queue never submits (`crates/core/src/tx/storage.rs:150-156`), and the entry moves to `CollectingVotes { revealed_count: 0 }`.
7. Next `NewBlock`: `block > reveal_deadline` → `finalize` with `revealed_count == 0` → `timed_out == true` → `Finalize` + `Claim` emitted, **entry deleted**.
8. Onchain the request is `FROZEN`: `finalize` reverts with `RequestNotPending` and `claim` with `RequestNotResolved` (basis 7). Both consume gas and a nonce.
9. Block `b+100`+: the arbitrator rules, `DisputeResolved` is emitted. `handle_resolved` finds no entry (`service.rs:502-508`) and logs at `debug`. The sentinel never claims; its bond (minus any slash) stays in the oracle.

## Considered and rejected

- **"Reveals during the warp would move the FSM forward anyway."** They cannot: `handle_revealed` requires `CollectingVotes` (basis 4), and the only transition into `CollectingVotes` is in `handle_block_advance` (`service.rs:438-447`), which is only reached from `Message::NewBlock`.
- **"The `Uncle` before the warp restores a snapshot that is already in `CollectingVotes`."** Only if the phase change happened at or before `indexed.safe`. The anchor is about `max_reorg_depth` blocks behind the tip at shutdown, so any phase change in the final few blocks is rolled back; and after the rollback there are no `NewBlock` messages until the warp ends.
- **"This is just the downtime, not a bug."** Steps 7 and 9 are the bug: the FSM concludes "nobody revealed" from evidence it deliberately threw away, and then _deletes_ an entry with a live bonded commitment. Had the entry survived, step 9's `DisputeResolved` would have produced a valid `Claim` (`service.rs:519-527`) and recovered the bond regardless of how much downtime there was.
- **"`handle_arbitration_timeout` claims even from an unexpected state, so the timeout path is covered."** It does (`service.rs:544-556`), but only while an entry still exists; step 7 deleted it (`service.rs:557-563` returns early for an untracked id). The same asymmetry is why `handle_resolved` has no such fallback.
- **"A warp only happens on a long outage."** A warp is also emitted on a _fresh_ start with a configured `start_block` (`core/index/blocks.rs:279-289`), and the `to` bound is always `latest - max_reorg_depth`, so any outage longer than `max_reorg_depth` blocks (25 s at the default) warps.
- **False positive check — are the logs really delivered during a warp?** Yes: `Status::WarpEvents` accepts `Update::Logs` batches whose range starts at the warp's `from` (`core/state/mod.rs:200-202`), and the event watcher pages the warp in `block_page_size` (default 100) chunks (`core/index/events.rs:301-346`). Only the _block_ messages are suppressed.
- **Not covered by any test.** No sentinel unit test issues a warp, and `scripts/run_sentinel_integration_test.sh` never stops a sentinel.

## Remediation options

1. **Deliver deadline progress after a warp.** Have the state machine synthesise a single `Message::NewBlock(to)` at the end of a warp (or before each warp page's logs), so deadline-driven phase transitions still run in order. This is a `safenet-core` change and would benefit every service; the tradeoff is that a service would see one `NewBlock` for a range rather than one per block, so transitions must be written to tolerate gaps (the sentinel's already are — all its comparisons are `block > deadline`).
2. **Do not conclude "timed out" from an empty local tally.** Gate the `timed_out` branch on evidence, e.g. only treat a request as timed out when the FSM actually observed the reveal window pass block by block; otherwise emit `Finalize` + `Claim` but keep the entry (see option 3) instead of deleting it.
3. **Keep bonded entries until a terminal onchain event is observed.** Replace the delete in `finalize` with a `WaitingForClaim` state cleared only by our own `Claimed` log (`service.rs:584-593` already receives it). Any later `DisputeResolved` / `ArbitrationTimedOut` then still finds the entry. Needs a bound to avoid the growth problem in F-SEN-005.
4. **Reconcile on startup.** After the warp completes, run one effect per tracked request that reads the oracle's `getRequest`/`getCommitment` and rebuilds the phase from chain state rather than from replayed logs. Most robust; costs one RPC round trip per tracked request at startup.

Tests to add: a `StateMachine`-level test that drives `Update::Block(Warp)` followed by `Update::Logs` containing `Revealed` events and asserts the sentinel still claims. A `service.rs` unit test asserting `finalize` does not delete an entry whose bond is still outstanding. An integration-script scenario that stops sentinel A for the whole reveal window of a disputed request and asserts a `Claimed` event for A after `resolveDispute`.

## Trail

- Reviewer R7: drafted from lead SEN-H3, self-estimate 70%. All eight basis citations re-opened in this checkout. The mechanism (no `NewBlock` during a warp, reveals discarded, timeout branch taken) is E2; whether a real deployment's `ARBITRATION_TIMEOUT` and outage distribution actually leave a request `FROZEN` at step 7 is inference, and it is that overlap which turns the wasted transactions into a lost claim.

## Critic (C-SEN)

Working from the location alone, I confirmed the core fact first: the `Warp` arm of `StateMachine::handle_update` returns `(state, status, vec![])` and calls no transition (`state/mod.rs:173-181`), while `Message::NewBlock` is produced only by the `BlockUpdate::New` arm (`:190-199`). `handle_block_advance` is the sentinel's only deadline engine (`service.rs:390-470`), so it does not run for a single warped block. That matches R7's claim exactly.

### Per-claim verdicts

| # | Verdict | Note |
| --- | --- | --- |
| 1 | **Supported** | Verbatim at `state/mod.rs:173-181`. |
| 2 | **Supported** | Verbatim at `state/mod.rs:213-236`; one snapshot per page, no per-block transition. |
| 3 | **Supported** | Verbatim at `blocks.rs:268-278`. The `uncle <= safe` guard holds whenever downtime exceeded the reorg window, as the trigger assumes. |
| 4 | **Supported** | Verbatim at `service.rs:347-362`. |
| 5 | **Supported** | Verbatim at `service.rs:493-501`. |
| 6 | **Supported** | Verbatim at `service.rs:626-627` and `:656-671`. |
| 7 | **Supported** | `SentinelOracleRequests.sol:239-245` verbatim; `FROZEN` is not in the accepted set. I also confirmed `finalize` itself requires `PENDING` (`:174`), so the companion `Finalize` reverts too. |
| 8 | **Supported** | `SentinelOracleRequests.sol:183-185` verbatim (read `:166-215` in full). |

### Where I agree, and the one conjunct that keeps this out of the top band

Steps 5 to 7 of the trigger are deterministic, not probabilistic: during the warp the FSM receives only `Message::Event`, so an anchor state of `CollectingCommitments` cannot advance however many blocks pass, and every `Revealed` in the range is discarded. The post-warp `finalize` then reads `revealed_count == 0` and takes the `timed_out` branch — that much is certain.

The **loss**, however, needs one further conjunct that R7 states honestly but cannot cite: the request must still be `FROZEN` at the moment of the post-warp finalise. If it has already resolved, the `Claim` at `service.rs:664-671` succeeds and nothing is lost; if it timed out, likewise. Only the arbitration-pending window makes both transactions revert into a deleted entry. Two independent things must hold at once — a >`max_reorg_depth` outage, and a disputed request whose arbitration is still open when the sentinel comes back. That is reachable (`ARBITRATION_TIMEOUT` is a long window by construction), but it is a conjunction, so this does not sit at the top of the `E2` band.

I checked and rejected the obvious way out: nothing else advances the phase during a warp. The only other `Reveal` emission site is `handle_block_advance` (`service.rs:426-437`), and there is no per-log deadline check anywhere in `apply_transition` (`service.rs:749-790`).

I also note the emitted `Reveal` is not merely useless but **actively destructive**: `std::mem::take(reason)` at `service.rs:424` moves the reason out of state before the action is queued, and `expires_at = reveal_deadline` (long past) means the queue skips it forever (`core/tx/storage.rs:150-155`). Even if the reveal window were somehow still open, the reason is now gone from state and could not be re-emitted.

### Root cause is core-level, but no core finding covers it

The defect that makes this possible — a warp advances the chain cursor by up to `block_page_size` blocks while delivering no `NewBlock`, so every service's deadline logic is skipped over the whole range — lives in `core/state/mod.rs:173-181`, not in the sentinel. I searched the finding set: `F-CORE-001`, `F-CORE-004`, `F-CORE-031` and `F-CORE-033` all touch warps, but **none** files the missing-`NewBlock` gap. It should be raised with the Coverage Critic / R2 as a core finding, with this file kept as the sentinel consequence. It is **not** downstream of `F-CORE-031`: that finding is about effect/resume durability, whereas this one loses nothing but block ticks.

### Finding verdict

**Confirmed. Certainty 72%. Severity Medium (unchanged).**

Every step is `E2`; the certainty sits near the floor of the Confirmed band because the lossy outcome requires the `FROZEN` conjunct. Medium is right per Section 8 — a crash-consistency gap with recoverable impact (the bond is claimable by hand, and the sentinel's participation resumes).

### Notes for QA

Reproducible at `StateMachine` level with no chain: seed a snapshot in `CollectingCommitments`, feed `Update::Block(Warp{from, to})` then a single `Update::Logs` covering the whole range including a peer `Revealed`, then `Update::Block(New{to+1})`. Assert the emitted actions are `Finalize` + `Claim` (the timeout branch) rather than the dispute branch.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

**Note for every sentinel finding whose "tests to add" list names a `service.rs` unit test:** `sentinel` is a **binary-only crate** — `crates/sentinel/src/main.rs` declares `mod service;` and there is no `lib.rs`, so the crate has no library target and `crates/sentinel/tests/` cannot compile against it. Every such test must live inside the existing `#[cfg(test)] mod tests` in the source file. If the team wants these as permanent regression tests reachable from an integration target, **the crate needs a `lib.rs` first**; that is an unstated prerequisite across F-SEN-001, -002, -003, -011, -012 and -015.

### Remediation check

**Sound: option 1 (a core change) or option 4; option 2 as stated is the weakest.**

Option 1 (have the state machine synthesise a `Message::NewBlock(to)` at the end of a warp) is sound and is the fix I would take: it is a `safenet-core` change that benefits every service, and the sentinel's own transitions already tolerate the gap because every comparison is `block > deadline` rather than `block == deadline`. I verified that: `handle_block_advance`'s four arms all use `<=`/`>` against a deadline (`crates/sentinel/src/service.rs:383-465`). The validator would need the same check before this lands.

**One caveat the option does not state:** a synthesised `NewBlock` inside `handle_update` would run the block transition and emit actions _within the same update as the warp page's logs_, which is fine — but it must be emitted **before** the page's logs, not after, or a deadline-driven transition would see reveals that logically follow it. `state/mod.rs`'s `Warp` arm currently emits no commands at all (`:171-178`), so this is a new command path and its ordering must be chosen deliberately.

Option 4 (reconcile from chain state after the warp completes, one effect per tracked request) is the most robust and is the same mechanism **F-SEN-001 option 2**, **F-SEN-002 option 2** and **F-SEN-015 option 1** all want. Four findings, one reconciliation effect — that is the strongest argument in the sentinel's whole finding set for building it. It carries the same two conditions I attached elsewhere: it must be a `Command::Effect` + `Resume` (transitions are pure and non-`async`), and a **failed** read must not drop the entry.

Option 2 ("do not conclude 'timed out' from an empty local tally") is sound in intent but vague as written — "only treat a request as timed out when the FSM actually observed the reveal window pass block by block" would require the FSM to record which blocks it saw, which is new state with no bound. Option 1 gives the same result without it.

Option 3 (`WaitingForClaim` until a terminal onchain event) is sound and is F-SEN-002 option 4; it needs F-SEN-005's deadline or it converts a funds bug into unbounded snapshot growth.

## Post-merge revalidation (RV-SEN)

Re-validated against merge commit `a7f3915` (baseline `2893917`). `crates/core` is byte-identical, so the warp mechanism itself (`core/index/blocks.rs:268-278`, `core/state/mod.rs:173-181` — no `Message::NewBlock` for a warped block) is unchanged.

### Verdict: **PARTIALLY ADDRESSED** — the funds impact in the title is FIXED; the replay defect remains

**Merged-code citations:**

| Cited at `2893917` | Now at | Changed? |
| --- | --- | --- |
| `service.rs:347-362` (`Revealed` in another phase discarded) | `service.rs:347-362` | byte-identical |
| `service.rs:450-465` (reveal-deadline branch) | `service.rs:450-465` | byte-identical |
| `service.rs:493-501` (`DisputeResolved` in an unexpected state discarded) | `service.rs:494-502` | +1, otherwise identical |
| `service.rs:626-671` (`finalize`, timeout branch emitting `Finalize` + `Claim` and deleting) | `service.rs:614-654` | **rewritten**: emits `Finalize` only and returns `Some(WaitingForOutcome { … })` |

#### What `6df6fb9` / `898a5c5` / `30d79de` close

The three new terminal handlers each recover a request found in an unexpected state, keyed on `SentinelRequestState::approve_and_slash_amount` (`state.rs:118-141`), which returns `Some(..)` for `CollectingCommitments` as long as `self_committed` is `true` — which it is on the F-SEN-003 path, because the anchor snapshot recorded our own commit before the outage. `handle_dispute_triggered` (`service.rs:742-750`) then promotes the stranded entry straight into `WaitingForDisputeResolution`.

Executed on the merged tree: anchor state `CollectingCommitments { committed_count: 2, self_committed: true }`, then a warp page delivering both reveals, the `DisputeTriggered` and the `DisputeResolved` with **no `NewBlock` at any point**:

```
RV003 anchor entry             = Some(CollectingCommitments { …, committed_count: 2, self_committed: true })
RV003 after self reveal        : commands=[] entry=Some(CollectingCommitments { … })   # still discarded
RV003 after peer reveal        : commands=[] entry=Some(CollectingCommitments { … })   # still discarded
RV003 after DisputeTriggered   : commands=[] entry=Some(WaitingForDisputeResolution { approve: true, slash_amount: 500 })
RV003 after DisputeResolved    : commands=[Action(SentinelAction { kind: Claim { … }, expires_at: None })] entry=None
```

The bond **is** claimed. The finding's headline impact — "a frozen request's bond is never claimed" — is closed. `handle_request_timed_out` (`service.rs:669-706`) and `handle_oracle_result` (`service.rs:775-817`) give the same recovery for the timed-out and directly-resolved variants.

#### What remains

1. **The reveals are still discarded.** `service.rs:347-362` is unchanged, so the local tally in a warped range is still wrong. This is what the two rows above show.
2. **The bogus timeout finalize still fires when no terminal event falls inside the warp.** If the warp ends before anyone finalized, the entry is still `CollectingCommitments`; the first post-warp `NewBlock` still builds `CollectingVotes { revealed_count: 0 }` and pushes a duplicate `Reveal` (reverts `AlreadyRevealed`), and the next block still takes `finalize`'s `timed_out = *revealed_count == 0` branch (`service.rs:631`) and emits a `Finalize` that is wrong about the outcome. The difference is that the entry now parks in `WaitingForOutcome` instead of being deleted, so the eventual real oracle event still recovers the claim — at the cost of two reverting transactions, and of parking in a state that never expires (**F-SEN-005**).
3. **New: the recovered request is mis-metered.** `SentinelRequestState::self_revealed()` (`state.rs:157-165`) returns `false` unconditionally for `CollectingCommitments`, justified by "revealing only starts once `CollectingVotes` is reached, and this state hasn't gotten there yet" (`state.rs:148-151`). On the warp path that is false — the reveal is onchain, only the _local_ phase is stale. So `handle_oracle_result` records `ResolvedOutcome::RevealMissed` (`service.rs:801-806`, `metrics.rs:84-89`) for a request the sentinel actually won. The bond is still claimed correctly; only the win-rate metric is wrong, and it is wrong in the direction that makes a real warp look like a slashing incident.

### Certainty and severity

**Certainty: 72% → 80%** — the mechanism is now executed rather than read. **Severity: Medium → Low**, since the funds loss is gone and what remains is two reverting transactions per affected request plus a metric that lies on this path. Status left at `Critiqued`.

## In-flight impact (AS-SEN)

**Pertains to unmerged branches, not to `main`.** Assessed against `origin/fix/sentinel_deadlines` (PR #914) and `origin/feat/optimistic_block_transition` (PR #915, tip `b2aad06`), built from a `git archive` extraction. Both apply cleanly onto `main` — `main`'s crates are byte-identical to their base `199629e`, so a verdict on the tip is also the verdict for the tip merged into `main`. **Effect: unchanged.** The warp path still runs no block transitions (`crates/core/src/state/mod.rs:295-305`), so reveals are still discarded.
