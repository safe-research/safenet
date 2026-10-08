# F-SEN-001 Replay after a restart or reorg discards the sentinel's own `Committed`, so it never reveals and its bond is slashed

| Field | Value |
| --- | --- |
| Status | Verified |
| Crate and module | sentinel, service.rs (with core: index/blocks.rs, state/mod.rs, driver.rs) |
| Location | crates/sentinel/src/service.rs:307-319, 413-417 (related: crates/core/src/index/blocks.rs:255-278, crates/core/src/state/mod.rs:246-258, crates/core/src/driver.rs:255-274) |
| Severity | High / High |
| Certainty | 98% (RW-CORE-SEN, Phase 8 real-world) |
| Assumptions involved | A2, A5, A10 |
| Tags | reorg, crash-consistency, funds |

## Claim

`self_committed` is only ever set by observing our own `Committed` log **while the FSM is in `CollectingCommitments`** (`service.rs:307-319`). Every restart, and every reorg within `max_reorg_depth`, rolls the FSM back to an earlier snapshot and replays the block range, re-spawning the `Effect::EngineCheck` for any proposal inside that range. The replayed `Committed(self)` log then arrives while the entry is back in `WaitingForEngineCheck` — because the engine's HTTP round trip is slower than replaying the next one or two already-mined blocks — and is discarded with a `warn`. When the engine finally resumes, `commit_vote` re-creates the entry with `self_committed: false`. At `commit_deadline + 1`, `handle_block_advance` sees `!self_committed`, believes "our own commit never landed onchain", and drops the request **without emitting a `Reveal`** (`service.rs:413-417`).

Onchain the commitment is real and stays `PENDING`. As soon as any side is established, `SentinelOracleRequest.slashAmountFor` charges the full governed `slashAmount` against that bond, and because the entry was dropped the sentinel never emits `Claim` for the unslashed remainder either. Net effect per affected request: `slashAmount` lost outright plus `bondTarget - slashAmount` locked in the oracle until an operator claims by hand. No attacker is required — an ordinary deploy restart is enough — and an attacker who can induce restarts (or who benefits from a 2-block reorg) can weaponise it.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | A `Committed` log seen in any state other than `CollectingCommitments` is discarded, so `self_committed` is never set from it | E2 | crates/sentinel/src/service.rs:307-319 | `        let RequestState::CollectingCommitments {`<br>`            committed_count,`<br>`            self_committed,`<br>`            ..`<br>`        } = entry`<br>`        else {`<br>`            tracing::warn!(`<br>`                request_id = %event.requestId,`<br>`                state = entry.name,`<br>`                "ignoring unexpected commitment"`<br>`            );`<br>`            return (state, Vec::new);`<br>`        };` |
| 2 | Past the commit deadline, `!self_committed` drops the entry with no `Reveal` action | E2 | crates/sentinel/src/service.rs:410-417 | `                if block <= *commit_deadline {`<br>`                    return true;`<br>`                }`<br>`                // Our own commit never landed onchain, so revealing would`<br>`                // just revert; drop the request instead.`<br>`                if !*self_committed {`<br>`                    return false;`<br>`                }` |
| 3 | Every restart rolls the state machine back to the earliest retained snapshot (about `max_reorg_depth` blocks) and replays the range | E2 | crates/core/src/index/blocks.rs:255-278 | `        if let Some(indexed) = indexed {`<br>`            // The earliest retained snapshot is the rollback anchor. Replay`<br>`            // everything after it, but only emit an uncle when there are newer`<br>`            // snapshots to discard. A pruned warp may retain only its latest`<br>`            // snapshot, in which case we can continue directly from the next`<br>`            // block without a synthetic reorg.`<br>`            let uncle = indexed.safe.checked_add(1);`<br>`            if let Some(uncle) = uncle`<br>`                && uncle <= indexed.latest`<br>`            {`<br>`                self.queue.push_back(BlockUpdate::Uncle { number: uncle });`<br>`            }` |
| 4 | A reorg `Uncle{n}` restores the snapshot at `n-1`, so the same replay happens live within `max_reorg_depth` | E2 | crates/core/src/state/mod.rs:182-189 | `            Update::Block(BlockUpdate::Uncle { number })`<br>`                if matches!(status, Status::BlockPending { pending } if number < pending)`<br>` |  | matches!(status, Status::BlockEvents { latest } if number <= latest) =>`<br>` {`<br>` let (_, state) = self.snapshots.reorg(number).await?;`<br>` let status = Status::BlockPending { pending: number };`<br>` (state, status, vec![])`<br>` }` |
| 5 | Replaying `TransactionProposed` re-spawns the engine effect, so the entry re-enters `WaitingForEngineCheck` | E2 | crates/sentinel/src/service.rs:127-144 | `        let deadline = block.saturating_add(self.voting_window);`<br>`        state.0.insert(`<br>`            request_id,`<br>`            RequestState::WaitingForEngineCheck {`<br>`                deadline,`<br>`                request: None,`<br>`            },`<br>`        );`<br>`        crate::metrics::requests_proposed_total.increment(1);`<br>`        (`<br>`            state,`<br>`            vec![Command::Effect(effect::Effect::EngineCheck {`<br>`                request_id,`<br>`                transaction: event.transaction,`<br>`                block,`<br>`            })],`<br>`        )` |
| 6 | The engine round trip is asynchronous and its resume competes with the next watcher update, so replayed log batches usually win the race | E2 | crates/core/src/driver.rs:227-230 | `        Ok(tokio::select! {`<br>`            update = update => Input::Update(update?),`<br>`            resume = self.effects.next => Input::Resume(resume)`<br>`        })` |
| 7 | An unrevealed commitment is slashed the governed `slashAmount` once either side is established | E2 (Solidity reference, A7) | contracts/src/libraries/SentinelOracleRequests.sol:289-296 | `        if (!isRevealedVote) {`<br>`            // A pending (never-revealed) vote is unrevealed griefing whenever either side ever got`<br>`            // established -- true for a directly-resolved request, and for a \`FROZEN\` request that`<br>` // later timed out via arbitration (that slash already happened back at \`finalize\``<br>` // time). A total timeout (nobody ever revealed) established neither side, so there is`<br>` // nothing left to slash.`<br>` bool wasEstablished = approveSentinelCount > 0 \|\| denySentinelCount > 0;`<br>` return wasEstablished ? self.terms.slashAmount : 0;`<br>` }` |
| 8 | The engine timeout is deliberately sized at three quarters of the voting window, confirming that engine checks are expected to take many blocks | E2 | crates/sentinel/src/main.rs:50-62 | `    let engine_timeout = {`<br>`        let block_time = config.driver.index.blocks.block_time.resolve(chain_id)?;`<br>`        Duration::from_millis(`<br>`            u64::try_from(`<br>`                u128::from(config.sentinel.voting_window.saturating_sub(1))`<br>`                    .saturating_mul(u128::from(block_time))`<br>`                    .saturating_mul(3)`<br>`                    / 4,`<br>`            )`<br>`            .unwrap_or(u64::MAX)`<br>`            .max(1_000),`<br>`        )`<br>`    };` |

## Trigger

Concrete restart sequence (Gnosis defaults, `max_reorg_depth = 5`, 5 s blocks):

1. Block `b`: `Consensus.proposeTransaction` emits `TransactionProposed` then, in the same transaction, `SentinelOracle.postRequest` emits `NewRequest` (`contracts/src/Consensus.sol:264-266`). The sentinel enters `WaitingForEngineCheck { request: Some(..) }` and spawns the engine check.
2. Block `b+1`: the engine answers, `commit_vote` runs, `approve`+`commit` are queued.
3. Block `b+2`: our `commit` is mined; `Committed(self)` sets `self_committed = true`. The snapshot at `b+2` records `CollectingCommitments { self_committed: true }`.
4. Block `b+3`: the operator restarts the process (deploy, config change, OOM kill, host reboot).
5. On startup `indexed.safe ≈ b-2`, so the watcher queues `Uncle{b-1}`; the state machine rolls back to the snapshot at `b-2`, in which the request does not exist yet.
6. Blocks `b-1 .. b+3` are re-delivered back to back — they are already mined, so there is no block-time pacing. At block `b` the replayed `TransactionProposed` re-inserts `WaitingForEngineCheck` and spawns a fresh HTTP engine check.
7. Block `b+2`'s replayed `Committed(self)` is processed a few milliseconds later, while the engine is still working, and hits `service.rs:307-319` → `"ignoring unexpected commitment"`, discarded.
8. The engine resumes; `commit_vote` re-creates `CollectingCommitments { committed_count: 0, self_committed: false }` and re-queues `approve`+`commit` (the duplicate `commit` reverts with `AlreadyCommitted`).
9. At `commit_deadline + 1` the entry is dropped by `service.rs:415-417`. No `Reveal` is ever emitted.
10. Another sentinel reveals; `finalize` establishes a side; our still-`PENDING` commitment is slashed `slashAmount` and the remainder is never claimed.

Reorg variant (no restart): a two-block reorg within `max_reorg_depth` that uncles the block containing `TransactionProposed` produces the same replay ordering — steps 5 to 10 are identical.

## Considered and rejected

- **"The edge guard at `service.rs:325-329` protects this."** It does not. The guard (`if !*self_committed && event.sentinel == self.signer.address`) only prevents _double counting_ our participation metric; it is inside the `CollectingCommitments` arm and is never reached when the outer `let ... else` at 307-319 has already returned.
- **"`state.0.get(&request_id)` at 119-126 stops the proposal being re-processed."** That guard only fires when an entry already exists. After the rollback the entry has been deleted from the restored snapshot, so the duplicate check passes and a fresh effect is spawned.
- **"The resume might arrive before the replayed `Committed`."** Possible, and then the behaviour is correct. But during a catch-up replay the watcher serves already-mined blocks with no block-time pacing, while the engine check is a full HTTP round trip whose budget is deliberately sized at three quarters of the voting window (`main.rs:50-62`, basis 8) because checks are slow. The losing case is the common one, not the exceptional one.
- **"The stale-resume guards at `service.rs:156-170` cover replay."** They cover the opposite direction — a resume arriving for an entry that has already advanced or disappeared (tests `stale_engine_check_resume_after_reorg_is_ignored`, `service.rs:1622`, and `stale_engine_check_resume_does_not_disturb_an_already_advanced_request`, `service.rs:1835`). Neither test replays a `Committed` log into `WaitingForEngineCheck`.
- **"The commitment can be recovered onchain."** `reveal` requires `block.number <= revealDeadline` (`contracts/src/libraries/SentinelOracleRequests.sol:130`), so once the window closes there is no recovery; only the post-slash remainder can be claimed, and only by hand.
- **False positive check — is the state really rolled back on a restart?** `SnapshotStore::status` returns `MIN(block_number)`/`MAX(block_number)` (`crates/core/src/state/storage.rs:86-101`) and `prune` retains everything from the watcher's `safe` block upward (`storage.rs:151-161`), so `indexed.safe` is about `max_reorg_depth` blocks behind the tip and `Uncle{indexed.safe + 1}` is emitted on every restart that saw at least one block since that anchor.
- **Not covered by any test.** No unit test in `service.rs` delivers a `Committed` event while the entry is in `WaitingForEngineCheck`; the happy-path test (`service.rs:1255-1276`) always commits after `commit_vote` has run. `scripts/run_sentinel_integration_test.sh` never restarts a sentinel.

## Remediation options

1. **Make `self_committed` derivable in every phase.** Record our own commitment as a field that survives phase changes: handle `Committed(self)` in `WaitingForEngineCheck` / `WaitingForRequest` by storing `self_committed: true` inside those variants (or in a small per-request `onchain` sub-struct) and carry it into `CollectingCommitments` when `commit_vote` runs. Cheap, local, and fixes the reorg case too. Tradeoff: two more places to keep in sync.
2. **Reconcile against the chain instead of trusting local tallies.** Before dropping a `CollectingCommitments` entry at `commit_deadline + 1`, emit an effect that reads `getCommitment(requestId, self)` (or `hashCommitment` equality) and only drop when the onchain commitment is genuinely absent. Strongest fix, and it also covers logs lost to an incomplete `eth_getLogs` (A4). Tradeoff: one extra RPC call per expiring request and a new effect/resume pair.
3. **Persist the engine verdict as soon as it is produced.** Have `handle_resume` commit a snapshot (or write the verdict to its own durable table) so a restart does not rewind past `commit_vote`. This narrows the restart window but does not fix the live-reorg variant. Tradeoff: changes a core invariant (`state/mod.rs:246-258`) that other services rely on.
4. **Defensive fallback:** when `!self_committed` at `commit_deadline + 1`, reveal anyway. A `reveal` without a matching commitment reverts with `NotCommitted` and costs only gas, whereas not revealing costs `slashAmount`. Tradeoff: wastes gas on genuinely-uncommitted requests; strictly worse than option 1 or 2 as a permanent design.

Tests to add: a `service.rs` flow test that feeds `proposed_event` → `new_request_event` → `committed_event(id, self_address, ..)` → `resolve_engine_check(Approved)` → `Message::NewBlock(commit_deadline + 1)` and asserts a `Reveal` action is emitted. A `StateMachine`-level test that commits a snapshot, calls `handle_update(Uncle)` and replays the range, asserting the reveal survives. An integration-script scenario that `SIGKILL`s and restarts sentinel A one block after its commit lands and asserts a `Revealed` event for A.

## Trail

- Reviewer R7: drafted from lead SEN-H2, self-estimate 80%. All eight basis citations re-opened in this checkout. Mechanism is E2; the win/lose ratio of the resume-versus-replay race is argued from code and configuration, not measured, so the _frequency_ is inference.

## Critic (C-SEN)

I re-derived this from `service.rs`, `state.rs`, `core/state/mod.rs`, `core/driver.rs` and `core/index/blocks.rs` before reading R7's argument, and arrived at the same mechanism plus one piece of evidence R7 did not have, which removes the finding's only soft spot.

### Per-claim verdicts

| # | Verdict | Note |
| --- | --- | --- |
| 1 | **Supported** | `service.rs:307-319` is quoted exactly. `handle_committed` binds `CollectingCommitments` in a `let ... else`; every other phase returns before `*self_committed = true` at `:326`. |
| 2 | **Supported** | `service.rs:410-417` verbatim. The `retain` closure returns `false` for `!self_committed` past `commit_deadline`, so the entry is removed and no `Reveal` is pushed. |
| 3 | **Supported, and stronger than stated** | `blocks.rs:255-266` verbatim. `initialize` computes `uncle = indexed.safe + 1` and pushes `BlockUpdate::Uncle{uncle}` whenever `uncle <= indexed.latest`. Since `prune` retains everything from the watcher's `safe` upward (`state/storage.rs:151-161`) and `safe = latest - max_reorg_depth` (`blocks.rs:246`, default 5 at `:83`), the anchor sits ~5 blocks behind the tip on every restart. This is not a corner case: it is the ordinary restart path. |
| 4 | **Supported** | `state/mod.rs:182-189` verbatim; `snapshots.reorg(number)` restores the snapshot at `number - 1` (`state/storage.rs:124-142`) and sets `BlockPending{pending: number}`, so `number` upward is re-delivered with its logs. |
| 5 | **Supported** | `service.rs:127-144` verbatim. After the rollback the duplicate guard at `:119-126` cannot fire, so a fresh `Command::Effect(EngineCheck)` is emitted. |
| 6 | **Supported as written, but the weakest row — and unnecessary** | The `tokio::select!` at `driver.rs:227-230` is real (and _not_ `biased`, unlike the shutdown select at `:175-182`). But arguing a _race_ understates the finding. See below. |
| 7 | **Supported** | `SentinelOracleRequests.sol:289-296` verbatim. I also confirmed the slash is applied eagerly inside `finalize` itself: `nonRevealerCount = committedCount - revealedCount; unrevealedBond = nonRevealerCount * slashAmount` (`:202-205`), transferred to the funds receiver at `SentinelOracle.sol:264-266`. The loss lands the moment any peer finalises, not later. |
| 8 | **Supported** | `main.rs:50-62` verbatim. |

### The race is not a race on the normal restart path

Basis 6 is the only inferential step, and it can be replaced with a deterministic one:

- On any restart after ≥1 block of downtime, `blocks.rs:271-278` also queues `Warp{from: indexed.safe + 1, to: safe}` (the condition `uncle <= safe` holds as soon as the chain advanced by one block while the process was down).
- A warp fetches logs in pages of `block_page_size`, default **100** blocks (`core/index/events.rs:97`, `:301-351`), and delivers each page as **one** `Update::Logs`.
- `StateMachine::handle_update` applies **every log in that page** in a single synchronous loop and only then returns the accumulated commands (`state/mod.rs:213-223`, `:243`).
- `Driver::update` spawns effects only _after_ `handle_update` returns (`driver.rs:255` then `:272`).

So when `TransactionProposed` (block `P`) and our own `Committed` (block `P+2`) fall in the same 100-block page — which they do for any realistic restart — the `Committed` log is applied while the entry is still `WaitingForEngineCheck` **before the engine effect has even been spawned**. There is no ordering under which the sentinel wins. Basis 6 should be reclassified `I` (frequency argument) and this paragraph added as a new `E2` row.

### Finding verdict

**Confirmed. Certainty 85%. Severity High (unchanged).**

`E2` throughout, with a concrete, operator-triggered sequence; 90+ is unreachable this run (no toolchain, so no `E1`). Severity is right under Section 8: recurring, unbounded drain of bonded funds through slashing, reachable by an ordinary deploy restart and by any reorg within `max_reorg_depth` (A5 says those must be handled). Not Critical: each incident costs the requests in one replay window rather than the whole bond pool.

### Relationship to F-CORE-031 — independent, not downstream

F-CORE-031 (effect spawned after the snapshot that records it; resume never snapshotted) is a real core defect, but it is **not** this finding's root cause. F-CORE-031's lossy case is a proposal in the _anchor block itself_, which is never replayed, so no second effect is ever spawned — that is F-SEN-011, not this. F-SEN-001 lives in the range the rollback **does** replay (`safe+1 ..= latest`), where a fresh effect _is_ spawned exactly as the runtime contract promises (`state/mod.rs:60-62`: "Effects may be performed more than once ... Transitions that emit effects must be prepared for the replayed effect to resume with a different result"). The defect is that `SentinelTransition` is _not_ prepared: it discards the chain evidence of its own bond because that evidence arrives in a phase its handler does not accept. Making resumes durable would not fix it; carrying `self_committed` across phases, or reconciling against `getCommitment` before dropping, would. **F-SEN-001 is canonical for this defect. Cross-reference F-CORE-031 as related context only.**

### Notes for QA

- The cheapest `E1` is a pure `SentinelTransition` unit test — no chain needed: `proposed_event` → `new_request_event` → `committed_event(id, self_address)` → `resolve_engine_check(Approved)` → `NewBlock(commit_deadline + 1)`; assert a `Reveal` is emitted. It will not be.
- A second test at `StateMachine` level should feed one `Update::Logs` batch containing both the proposal and the commit, to demonstrate the deterministic (non-race) form above.

## QA (QA-CORE-SEN)

**Outcome: Reproduced by inspection.** Nothing was executed — there is no Rust toolchain on this host (`state/baseline.md` §1) — so this does **not** move the finding into the 90-100 band, which requires `E1`.

**PoC written:** `rust-audit/poc/F-SEN-001/` — `poc_service.rs` (two tests) plus a README with the exact command, every fixture spelled out literally, and the pass/fail reading.

**Constraint the team must know before running it:** `sentinel` is a **binary-only crate**. `crates/sentinel/src/main.rs` declares `mod service;` and there is no `lib.rs`, so the crate has no library target and `crates/sentinel/tests/` cannot compile against it. Both tests have to be pasted into the existing `#[cfg(test)] mod tests` in `crates/sentinel/src/service.rs` — a temporary edit to a tracked file, reverted with `git checkout --`. Every sentinel PoC in this run has the same constraint. **If the team wants these as permanent regression tests, the crate needs a `lib.rs` first**; that is a real prerequisite for the "tests to add" list in this finding and in F-SEN-002, -003, -011, -012 and -015.

### Reproduced by inspection — traced end to end

1. `handle_committed` (`service.rs:296-322`) binds `RequestState::CollectingCommitments` in a `let … else`; every other phase logs `ignoring unexpected commitment` and returns before `*self_committed = true` at `:326`. I read the whole function: there is no other write to `self_committed` anywhere in the file (`grep -n self_committed` gives `:222` (init `false`), `:305`, `:325-326`, `:394`, `:415`).
2. `handle_block_advance`'s `CollectingCommitments` arm (`:410-417`) returns `false` from the `retain` closure when `!*self_committed` past `commit_deadline`, and the `Reveal` push at `:424-437` is below that early return. Entry removed, no action.
3. `handle_oracle_transaction_proposed` (`:119-144`) — the duplicate guard is `if let Some(entry) = state.0.get(&request_id)`, which cannot fire after a rollback deleted the entry, so a fresh `Effect::EngineCheck` is emitted.
4. `handle_engine_check_result` → `commit_vote` (`:198-225`) re-creates the entry with `committed_count: 0, self_committed: false`. Verified by reading the struct literal.
5. `BlockWatcher::initialize` (`index/blocks.rs:261-266`) pushes the synthetic `Uncle` on every restart retaining more than one snapshot; `SnapshotStore::prune` retains from `safe` upward (`state/storage.rs:151-161`) and `safe = latest - max_reorg_depth` (`blocks.rs:246`), so the anchor sits ~`max_reorg_depth` blocks behind the tip. This is the ordinary restart path.

**I confirm C-SEN's strengthening of basis 6, and the PoC's second test is built to demonstrate it.** On the restart path this is not a race. `StateMachine::handle_update`'s `Update::Logs` arm applies **every log in the batch** in one synchronous `for` loop (`state/mod.rs:200-223`) and returns the accumulated commands only afterwards (`:243`); `Driver::update` spawns effects only after `handle_update` returns (`driver.rs:255`, then `:272`). A warp page is up to `block_page_size` = 100 blocks (`index/events.rs:97`), so the proposal and the commit are in the same batch for any realistic restart and the `Committed` is consumed **before the engine effect is spawned at all**. There is no scheduling outcome under which the sentinel wins. Basis 6 should be reclassified `I` and this replaced as a new `E2` row, exactly as C-SEN says.

**Certainty: unchanged at 85%.** I add no evidence C-SEN did not already have, and 89% is the `E2` ceiling. Severity High is correct and I would not raise it to Critical: the loss is bounded by the requests in one replay window, not by the bond pool.

### Remediation check — at least one option is sound

**Option 1 (carry `self_committed` across phases) is sound and is the one I recommend.** It is a pure-state change: `handle_committed` would write a flag into `WaitingForEngineCheck` / `WaitingForRequest` and `commit_vote` would carry it forward. No effect, no I/O, no new failure mode, and it satisfies every clause of the documented `core::state` contract — transitions stay pure and infallible, nothing assumes exactly-once effect delivery, and it is insensitive to resume ordering. It fixes the reorg variant as well as the restart one. Its stated tradeoff ("two more places to keep in sync") is real but small.

**Option 2 (reconcile against `getCommitment` before dropping) is sound _only if implemented as an effect whose failure is encoded in the resume_, which the text does not say.** Two conditions:

- `apply_transition` is a pure, non-`async` `fn` returning `(State, Commands)`, so the read cannot happen inside `handle_block_advance`. It has to be `Command::Effect(Effect::ReadCommitment { .. })` plus a `Resume` arm — which means the entry survives at least one more block and the drop moves to the resume handler.
- **The resume handler must not drop the entry when the read fails.** If it does, this option recreates F-SEN-015 variant 2 exactly: `handle_engine_check_result` already removes the entry before inspecting the outcome and never re-inserts it on `CheckOutcome::Unknown` (`service.rs:156`, `:176-179`), and that is how a bonded request becomes untracked today. The effects contract is explicit about the right shape here — "for consumptive resources … handlers should encode outcomes like 'already used' in `Resume`" (`effects.rs:20-24`) — so the resume needs three states (`Committed`, `NotCommitted`, `Unknown`) and `Unknown` must mean "keep the entry and reveal anyway", not "drop it".

With those two conditions option 2 is the stronger fix, because it also covers the A4 case of a `Committed` log lost to an incomplete `eth_getLogs`, which option 1 does not.

**Option 3 (persist the engine verdict / snapshot resumes) is unsound and should not be taken.** It is F-CORE-031 option 1 under another name. Giving `handle_resume` its own commit at the current `latest` writes a snapshot for a block whose **logs have not been applied yet**: the status after `Update::Block(New{n})` is `BlockEvents { latest: n }` and the logs for `n` arrive in the _next_ update (`state/mod.rs:190-199` then `:200-239`). A crash in that window leaves `SnapshotStore::current` returning `(n, partial_state)`, so `StateMachine::with_init` resumes at `BlockPending { pending: n + 1 }` (`:132-137`) and **block `n`'s logs are never fetched**. That converts a bounded, visible replay problem into permanent silent log loss — the same class of defect as F-CORE-002. It also does not fix the live-reorg variant, which the option itself admits. See the fuller argument in my QA section on F-CORE-031.

**Option 4 (reveal anyway when `!self_committed`) is sound as a backstop but must not ship alone.** The economics are right — a `reveal` without a commitment reverts `NotCommitted` and costs gas, while not revealing costs `slashAmount`. Two caveats the text omits: it interacts with F-CORE-067, because on a replay the sentinel will now emit a _second_ `Reveal` for a request it already revealed, and the queue has no de-duplication; and it makes F-SEN-014's herd worse. Take it as a temporary mitigation while option 1 lands, not as the design.

**Recommendation:** option 1 now, option 2 next (it subsumes the A4 trigger and, per F-SEN-015 option 1, also closes that finding). Neither breaks the runtime contract.

## Verification (V-CORE-SEN, Phase 5)

**Executed. Reproduced. `E1`.**

`rust-audit/poc/F-SEN-001/poc_service.rs` was appended verbatim to the existing `#[cfg(test)] mod tests` block at the bottom of `crates/sentinel/src/service.rs` and run with

```
cargo test -p sentinel --bin sentinel service::tests::poc_f_sen_001
```

**No mechanical repair was needed**; the PoC compiled unmodified and no assertion was altered. The file was reverted with `git checkout -- crates/sentinel/src/service.rs`. Full output in `rust-audit/poc/F-SEN-001/RESULT-V-CORE-SEN.out`.

### Verbatim result

```
running 2 tests
test service::tests::poc_f_sen_001_replayed_own_commit_is_discarded_so_no_reveal_is_emitted ... FAILED
test service::tests::poc_f_sen_001_warp_page_applies_the_commit_before_the_effect_is_spawned ... ok

---- service::tests::poc_f_sen_001_replayed_own_commit_is_discarded_so_no_reveal_is_emitted stdout ----
thread '...' panicked at crates/sentinel/src/service.rs:2009:5:
assertion `left == right` failed: no Reveal was emitted for a commitment that is live onchain — the
`!self_committed` branch at service.rs:415-417 dropped the request, so slashAmount (500) is lost and
the remainder is never claimed
  left: []
 right: [Action(SentinelAction { kind: Reveal { id: 0x23d490d0…5225538d, approve: true,
          salt: 0x261eb185…3e0d398d, reason: "" }, expires_at: Some(40) })]
```

Both outcomes are exactly what the PoC README predicted for this checkout: **test 1 fails at the final assertion with `commands == []`** (the finding reproducing) and **test 2 passes** (the ordering claim holding).

### What is now established by execution rather than by reading

1. Every intermediate assertion in test 1 **held**, which is what makes the final failure diagnostic rather than merely negative:
   - the replayed `TransactionProposed` re-spawned `engine_check_effect(id, TO, 10)`;
   - after the sentinel's **own** `Committed(id, self_address, 500)` at block 12, the entry was still `RequestState::WaitingForEngineCheck { deadline: 20, request: Some(Request { bond_target: 500, slash_amount: 500, commit_deadline: 20, reveal_deadline: 40 }) }` — i.e. the commitment left **no trace at all** in the persisted state (`handle_committed`, `service.rs:307-319`);
   - when the engine verdict finally resumed, `commit_vote` re-created the entry as `CollectingCommitments { …, committed_count: 0, self_committed: false }`, forgetting a commitment that is live onchain.
2. At `NewBlock(commit_deadline + 1) == NewBlock(21)` the transition emitted **zero commands**. The sentinel does not reveal, and the entry is removed, so no later event (`Revealed`, `DisputeResolved`, `ArbitrationTimedOut`, `Claimed`) can rescue it.
3. Test 2 **passed**, establishing the ordering with no inferential step left: one `Update::Logs` page carrying the proposal (block 10) and the sentinel's own commit (block 12) returns exactly `[Command::Effect(Effect::EngineCheck { request_id, transaction, block: 10 })]`. The `Committed` was consumed — and discarded — inside `StateMachine::handle_update`'s synchronous per-log loop **before** `Driver::update` could spawn the effect. There is therefore **no race to win**: on the restart path the commit is always applied before the engine has been asked.

### Residual uncertainty

What remains unexecuted is the onchain consequence, which rests on the Solidity reference under A7: that a `PENDING` commitment is slashed `slashAmount` when a peer finalises (`contracts/src/libraries/SentinelOracleRequests.sol:202-205`, `:289-296`). No Foundry/anvil is available on this host, so that leg is still read rather than run. The Rust-side defect — a live bond with no reveal ever emitted — is now executed fact.

**Basis class:** `E1`. **Certainty: 85% → 96%. Status: Critiqued → Verified.** Severity unchanged (High / High).

## Real-world validation (Phase 8, RW-CORE-SEN)

**Verdict: Reproduced end-to-end.** The money moved on chain.

### Scenario

Local Anvil only (`http://127.0.0.1:8645`, chain 31337, 1 s blocks, binary copied to the session scratchpad so a sibling agent's `pkill anvil` could not reach it). Every service config was generated in the scratchpad and its effective `rpc` printed before start; all four read `rpc = "http://127.0.0.1:8645"`. No sample config was used.

Deployed with `forge script` against that Anvil: `MyToken` (fee token), the real `Consensus` backed by `MockCoordinator`, and the real `SentinelOracle` — `REQUEST_FEE = 1000`, `BOND_MULTIPLIER = 4` (`bondTarget = 4000`), `INITIAL_SLASHING_MULTIPLIER = 2` (`slashAmount = 2000`), commit window 30, reveal window 20, DAO fee share 0, protocol funds receiver `0x2222222222222222222222222222222222222222`.

Two registered sentinels, both the real `target/debug/sentinel` binary with the real `target/debug/sentinel-engine`, each on a **file-backed** SQLite database (the sample config's documented "must be backed up and restored across restarts" setting), signing with freshly generated funded keys. A is the sentinel that restarts; B is an untouched control.

Induced: a sponsor proposes a transaction; A commits its bond; **A's process is really killed and really restarted against the same database**, three blocks after its `Committed` landed. Nothing else is touched.

### Verbatim outcome

A's bond was posted (fee-token balance 1,000,000 → 996,000; `commit` at block `0x1d`, nonce 1). After the restart, `a2.log`:

```
{"level":"WARN","fields":{"message":"ignoring unexpected commitment","request_id":"0xeb49ce4b…","state":"waiting_for_engine_check"}}
{"level":"WARN","fields":{"message":"ignoring unexpected commitment","request_id":"0xeb49ce4b…","state":"waiting_for_engine_check"}}
```

Both `Committed` logs — A's own included — were discarded on the replay, exactly as the claim predicts. Every transaction A sent for the whole life of the request:

```
0x1d nonce=0x0 to=<feeToken>  fn=approve
0x1d nonce=0x1 to=<oracle>    fn=commit
0x20 nonce=0x2 to=<feeToken>  fn=approve     <- replay duplicate
0x20 nonce=0x3 to=<oracle>    fn=commit      <- replay duplicate, reverted AlreadyCommitted
```

**No `reveal`, no `finalize`, no `claim` — ever.** The control sentinel B revealed at block `0x3c`, finalised at `0x50` and claimed at `0x50`.

Final on-chain state (`getRequest`): `state = 3` (`RESOLVED_APPROVED`), `committedCount = 2`, `revealedCount = 1`, `approveSentinelCount = 1`. A's commitment: `vote = 1` (`PENDING`, never revealed), `claimed = false`.

**On-chain balance change, fee token:**

| Account                              | Before    | After     | Delta      |
| ------------------------------------ | --------- | --------- | ---------- |
| Sentinel A (restarted)               | 1,000,000 | 996,000   | **−4,000** |
| Sentinel B (control)                 | 1,000,000 | 1,001,000 | +1,000     |
| Protocol funds receiver (slash sink) | 0         | **2,000** | **+2,000** |
| Oracle (A's unclaimed remainder)     | 0         | 2,000     | +2,000     |

The 2,000 in the funds receiver is `finalize`'s `unrevealedBond` transfer (`nonRevealerCount × slashAmount`, `SentinelOracleRequests.sol:202-205`): the contract really slashed A. The remaining 2,000 is locked in the oracle because A's entry was dropped and it never calls `claim` — precisely the "slashAmount lost outright plus `bondTarget - slashAmount` locked" the claim describes. An ordinary deploy restart was the entire trigger; no attacker, no reorg.

Reproduced identically on three independent runs (one further run was aborted by an unrelated `pkill` from a sibling agent and is not counted).

**Certainty: 96% → 98%.** Severity unchanged (High / High). Raised because the loss was driven to an observable on-chain balance change against the real `SentinelOracle`, not an internal state assertion.

## Post-merge revalidation (RV-SEN)

Re-validated against merge commit `a7f3915` (`origin/main` merged into `rust-audit`); the audit baseline was `2893917`. In `crates/sentinel` only `bindings.rs`, `metrics.rs`, `service.rs` and `state.rs` changed; `crates/core` is byte-identical across the merge (`git diff 2893917 HEAD -- crates/core` is empty), so basis rows 3, 4 and 6 stand unchanged at their original line numbers.

### Verdict: **STILL VALID** — `[Part 7] Use oracle events over local inference` (`199629e`) does **not** close this

**Merged-code citations (line numbers unchanged — both cited hunks are byte-identical):**

- Basis 1, the discard: `crates/sentinel/src/service.rs:307-319`, still `let RequestState::CollectingCommitments { committed_count, self_committed, .. } = entry else { … "ignoring unexpected commitment" … }`.
- Basis 2, the no-reveal drop: `crates/sentinel/src/service.rs:410-417`, still `if !*self_committed { return false; }`.
- Basis 5, the effect respawn: `crates/sentinel/src/service.rs:127-144`, unchanged.
- Basis 7 moved with the Solidity: the slash rule is now `contracts/src/libraries/SentinelOracleRequests.sol:286-293` (three `import`/`using` lines were deleted at the top of the file); the quoted text is unchanged.
- The two tests named under _Considered and rejected_ moved: `stale_engine_check_resume_after_reorg_is_ignored` is now `service.rs:1929`, and `stale_engine_check_resume_does_not_disturb_an_already_advanced_request` is now `service.rs:2142`. Neither replays a `Committed` log into `WaitingForEngineCheck`; that is still untested.

What `199629e` actually changed is the _finalize_ path only: `finalize` no longer predicts `dispute`/`unanimous` from its local tally, and instead parks in a new `RequestState::WaitingForOutcome` until the oracle emits `DisputeTriggered` / `RequestTimedOut` / `OracleResult` (`service.rs:614-654`, handlers at `:669-706`, `:723-753`, `:775-817`). Nothing on the commit-tracking path — the only path this finding is about — was touched.

### Evidence (execution-verified on the merged tree)

`rust-audit/poc/F-SEN-001/poc_service.rs` was pasted **unmodified** into the tests module of the merged `crates/sentinel/src/service.rs` and run. It compiled against the merged APIs with no edits and failed exactly as it did at `2893917`:

```
test service::tests::poc_f_sen_001_replayed_own_commit_is_discarded_so_no_reveal_is_emitted ... FAILED
  assertion `left == right` failed: no Reveal was emitted for a commitment that is live onchain
  left: []
  right: [Action(SentinelAction { kind: Reveal { id: 0x23d4…538d, approve: true, salt: 0x261e…398d, reason: "" }, expires_at: Some(40) })]
test service::tests::poc_f_sen_001_warp_page_applies_the_commit_before_the_effect_is_spawned ... ok
```

The PoC therefore still compiles and still reproduces, as written.

### The merge propagates the same wrong inference to a second site

The new recovery helper `SentinelRequestState::approve_and_slash_amount` (`crates/sentinel/src/state.rs:118-141`) is what decides whether the three new terminal handlers claim a bond back. For `CollectingCommitments` it keys off exactly the flag this finding corrupts:

```rust
Self::CollectingCommitments { approve, slash_amount, self_committed, .. }
    => self_committed.then_some((*approve, *slash_amount)),
```

and its doc states the inference as fact — "`CollectingCommitments` while `self_committed` is still `false` (the request is open and its terms are known, but our own `Commit` hasn't landed onchain yet) … in every one of these there is nothing of ours to claim back" (`state.rs:110-113`). That is the precise proposition this finding disproves. Probed by execution on the merged tree, feeding each of the three new terminal events into the F-SEN-001 replay state:

```
RV001 pre-deadline entry = Some(CollectingCommitments { …, committed_count: 0, self_committed: false })
RV001 branchA OracleResult:     commands = [] entry = None
RV001 branchA RequestTimedOut:  commands = [] entry = None
RV001 branchA DisputeTriggered: commands = [] entry = None
RV001 block21 commands = [] entry = None          # dropped by service.rs:413-417, as before
RV001 branchB OracleResult:     commands = [] entry = None   # now untracked; ignored
RV001 branchB RequestTimedOut:  commands = [] entry = None
RV001 branchB DisputeTriggered: commands = [] entry = None
```

So neither branch emits a `Claim`: before the commit deadline the recovery declines because `self_committed` is falsely `false`, and after it the request has already been dropped and is untracked. The `bondTarget - slashAmount` remainder is still stranded, and the merge added a second place that makes the same assumption.

### Certainty and severity

**Certainty: 98% → 99%.** Severity unchanged (**High / High**). Raised only because the PoC was re-executed unmodified against the merged tree; the onchain reproduction from the Phase 8 run is unaffected, since the contract-side slash rule (`SentinelOracleRequests.sol:286-293`) is unchanged.

Status left at `Verified` — not fixed.

## In-flight impact (AS-SEN)

**Pertains to unmerged branches, not to `main`.** Assessed against `origin/fix/sentinel_deadlines` (PR #914) and `origin/feat/optimistic_block_transition` (PR #915, tip `b2aad06`), built from a `git archive` extraction. Both apply cleanly onto `main` — `main`'s crates are byte-identical to their base `199629e`, so a verdict on the tip is also the verdict for the tip merged into `main`. **Effect: unchanged — bond lost again, live.** `handle_committed` (`service.rs:307-319`) still returns on `"ignoring unexpected commitment"`, `:427-429` still drops the reveal when `!self_committed`, and the second site at `state.rs:121-126` is unchanged; the PoC fails unmodified.

**Live Anvil run with the #915 tip binaries** (local port 8845, engines 5873/5874, every config `rpc = "http://127.0.0.1:8845"`): sentinel A committed at block 31, was killed, and restarted at block 33 (`commitDeadline = 60`, `revealDeadline = 80`). It re-sent `approve` and `commit`; the second commit reverted in block 34 with `0xbfec5558` = `AlreadyCommitted()`. A never sent `reveal`, `finalize` or `claim`; B completed normally. Final state: A's commitment `[…, "4000", 1, false]` still pending, A's balance 996000 against B's 1001000, the slash receiver +2000. A's log after restart: `ignoring unexpected commitment` ×2 (`state: waiting_for_engine_check`) and `ignoring reveal for an untracked request`. Same outcome as the Phase 8 run on `main`.

## Scope note (run 2)

`crates/sentinel-engine` was placed out of scope after this finding was written (PROMPT.md Section 11); the engine-specific parts above are retained as run-1 history and are not part of the current audit's claims. The non-engine parts stand.

## Reconciliation (run 2)

**Final combined status: Confirmed, High, 99% (E1), canonical `F-SEN-001`.** Run-2 counterpart `F2-SEN-001` (High 92, E1 at `StateMachine` level, re-run at `fe9e84c`) **confirms** this finding and extends it with variant (a): the anchor snapshot holds `WaitingForEngineCheck`, the proposal block is not replayed, so no effect is re-spawned and the replayed `Committed(self)` is discarded all the same. Variant (c) of `F2-SEN-001` is `F-SEN-015`. The core root cause is `F2-CORE-030` (REC-CORE, canonical for core); this file stays canonical for the sentinel loss. At `fe9e84c` (PR #914 merged) the mechanism is untouched: `crates/sentinel/src/service.rs:307-319` (discard), `156-171`/`176-179` (resume path), `214-224` (`self_committed: false`) are byte-identical; the only change is that a rolled-back `WaitingForEngineCheck` entry is dropped at `NewBlock(commit_deadline)` instead of one block later (`407-412`, `block <`), same loss. The report carries run 1's number (on-chain `E1`); run 2's independent `E1` is the evidence that the merged tree behaves identically. Team disposition: none yet — this is the highest-certainty irreversible funds loss in the crate and is unassigned; the fix converges with `F-SEN-015` (three-state `getCommitment` effect, or handler-side durable verdict plus a `RecoverVerdict` effect, as `F2-SEN-001`'s QA reshaped its option 1). Both runs reject "snapshot resumes" as unsound. See `state/run2/reconciliation/sentinel.md` §1, §4.
