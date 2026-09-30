# F-SEN-015 A replayed engine check re-decides an already-committed vote: the second verdict overwrites the reason the commitment was built from, so the reveal fails the onchain hash check (or is never sent) and the bond is slashed

| Field | Value |
| --- | --- |
| Status | Verified |
| Crate and module | sentinel, service.rs (with core: state/mod.rs, driver.rs, index/blocks.rs) |
| Location | crates/sentinel/src/service.rs:150-194, 198-244 (related: crates/sentinel/src/engine.rs:163-194, crates/core/src/state/mod.rs:54-73, 182-189, crates/core/src/index/blocks.rs:255-278) |
| Severity | High / High |
| Certainty | 97% (RW-CORE-SEN, Phase 8 real-world) |
| Assumptions involved | A2, A3, A5, A10 |
| Tags | reorg, crash-consistency, funds, crypto |

## Claim

The commit-reveal game requires the sentinel to reveal _exactly_ the `(approve, salt, reason)` triple its commitment hash was built from — `reveal` recomputes `keccak256(abi.encodePacked(approve, salt, sentinel, requestId, reason))` and reverts `InvalidReveal` on any difference (`contracts/src/libraries/SentinelOracleCommitments.sol:103-124`). `salt` is deterministic in `request_id`, so the binding values are `approve` and `reason`, and both come from a **live HTTP call to the sentinel engine** (`service.rs:173-180`, `engine.rs:163-194`).

Every restart and every reorg within `max_reorg_depth` rolls the state machine back and replays the block range, re-emitting `Effect::EngineCheck` for any `TransactionProposed` inside it (`core/index/blocks.rs:255-278`, `core/state/mod.rs:182-189`, `service.rs:127-144`). The engine is then asked to decide the _same proposal a second time_ — but the first decision has already been committed onchain and is immutable. `handle_engine_check_result` consumes the second verdict exactly as it consumed the first: it removes the entry, takes the new `(approve, reason)`, and hands them to `commit_vote`, which computes a **new** `commit_hash` and stores the **new** `reason` in `CollectingCommitments` (`service.rs:156-194`, `:198-225`). At the commit deadline `handle_block_advance` reveals whatever is in state (`service.rs:424-437`).

Three outcomes, all losing:

1. **Verdict changed** (`Approved` → `Denied(R-x.y)`, or a different rule id). The stored `reason` / `approve` no longer match the onchain `commitHash`. The duplicate `commit` reverts `AlreadyCommitted` (`SentinelOracleCommitments.sol:91-93`), leaving the original hash in place, and the later `reveal` reverts `InvalidReveal`. The commitment stays `PENDING` and is slashed `slashAmount` the moment any peer finalises with a side established (`contracts/src/libraries/SentinelOracleRequests.sol:202-205`, `:289-296`).
2. **Verdict unavailable** (`CheckOutcome::Unknown` — the overwhelmingly likely case on a restart, because the replay runs milliseconds after the process starts, while a co-deployed engine (A3) is still booting). `handle_engine_check_result` has _already removed_ the entry at `service.rs:156` and returns without re-inserting it (`:176-179`). The request is now untracked, no `Reveal` is ever emitted, and the replayed `Committed(self)` is dropped as "untracked" (`service.rs:300-306`). Same slash.
3. **Verdict identical.** No loss — the recomputed hash matches and the duplicate `commit` simply reverts.

The sentinel has all the information needed to avoid this: the salt is derivable, the commitment is readable onchain (`getCommitment`, and the unused `hashCommitment` binding at `bindings.rs:49-55`), and the runtime contract explicitly warns that "Transitions that emit effects must be prepared for the replayed effect to resume with a different result" (`core/state/mod.rs:54-73`). It is not prepared: nothing anywhere on the path checks whether a commitment for this request already exists onchain.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The reveal must reproduce the committed `(approve, salt, reason)` byte-for-byte or it reverts | E2 (Solidity reference, A7) | contracts/src/libraries/SentinelOracleCommitments.sol:116-121 | `        require(commitment.vote != SentinelOracleCommitment.Vote.NONE, NotCommitted);`<br>`        require(commitment.vote == SentinelOracleCommitment.Vote.PENDING, AlreadyRevealed);`<br>`        require(`<br>`            SentinelOracleCommitment.computeHash(sentinel, requestId, approve, salt, reason) == commitment.commitHash,`<br>`            InvalidReveal`<br>`        );` |
| 2 | `approve` and `reason` are produced from a live HTTP verdict, not from anything deterministic | E2 | crates/sentinel/src/service.rs:173-180 | `        let (approve, reason) = match outcome {`<br>`            CheckOutcome::Approved => (true, String::new),`<br>`            CheckOutcome::Denied(rule) => (false, rule.to_string),`<br>`            CheckOutcome::Unknown => {`<br>`                tracing::warn!(%request_id, "engine check failed; dropping request unanswered");`<br>`                return (state, Vec::new);`<br>`            }`<br>`        };` |
| 3 | Any transport failure, timeout, non-2xx status or unparseable body becomes `Unknown` — the outcome is not a pure function of the transaction | E2 | crates/sentinel/src/engine.rs:181-188 | `                Err(err) => {`<br>`                    tracing::error!(`<br>`                        %err,`<br>`                        "sentinel engine request failed; dropping the request unanswered",`<br>`                    );`<br>`                    (CheckOutcome::Unknown, EngineCheckVerdict::Error)`<br>`                }` |
| 4 | The `Unknown` arm removes the entry and never restores it, so a bonded request becomes untracked | E2 | crates/sentinel/src/service.rs:156-171 | `        let (deadline, request) = match state.0.remove(&request_id) {`<br>`            Some(RequestState::WaitingForEngineCheck { deadline, request }) => (deadline, request),`<br>`            Some(entry) => {`<br>`                tracing::warn!(`<br>`                    %request_id,`<br>`                    state = entry.name,`<br>`                    "ignoring unexpected engine check result"`<br>`                );`<br>`                state.0.insert(request_id, entry);`<br>`                return (state, Vec::new);`<br>`            }` |
| 5 | The second verdict is hashed into a fresh commitment and stored as the reveal reason, with no check against any existing onchain commitment | E2 | crates/sentinel/src/service.rs:212-225 | `        let salt = self.signer.reveal_salt(request_id);`<br>`        let hash = commit_hash(self.signer.address, request_id, approve, salt, &reason);`<br>`        state.0.insert(`<br>`            request_id,`<br>`            RequestState::CollectingCommitments {`<br>`                approve,`<br>`                reason,`<br>`                slash_amount,`<br>`                commit_deadline,`<br>`                reveal_deadline,`<br>`                committed_count: 0,`<br>`                self_committed: false,`<br>`            },`<br>`        );` |
| 6 | The reveal is built from whatever `reason`/`approve` state holds at the commit deadline | E2 | crates/sentinel/src/service.rs:418-437 | `                let approve = *approve;`<br>`                // \`CollectingVotes\` has no \`reason\` field of its own, so this is the`<br>` // last use of it — take it rather than cloning.`<br>` let reason = std::mem::take(reason);`<br>` let salt = self.signer.reveal_salt(*id);`<br>` actions.push(`<br>` SentinelAction {`<br>` kind: SentinelActionKind::Reveal {`<br>` id: *id,`<br>` approve,`<br>` salt,`<br>` reason,`<br>` },` |
| 7 | Every restart rolls back to the retained anchor and replays the range, re-emitting the effect | E2 | crates/core/src/index/blocks.rs:261-266 | `            let uncle = indexed.safe.checked_add(1);`<br>`            if let Some(uncle) = uncle`<br>`                && uncle <= indexed.latest`<br>`            {`<br>`                self.queue.push_back(BlockUpdate::Uncle { number: uncle });`<br>`            }` |
| 8 | The runtime documents at-least-once effect delivery with possibly-different results, so this is the transition's responsibility | E2 | crates/core/src/state/mod.rs:59-62 | `/// Effects may be performed more than once for the same chain message, for`<br>`/// example after a crash or reorg replay. Transitions that emit effects must be`<br>`/// prepared for the replayed effect to resume with a different result.` |
| 9 | A duplicate `commit` cannot overwrite the original hash | E2 (Solidity reference, A7) | contracts/src/libraries/SentinelOracleCommitments.sol:91-93 | `    function checkNotCommitted(T storage self, bytes32 requestId, address sentinel) internal view {`<br>`        require(self.commitments[requestId][sentinel].commitHash == 0, AlreadyCommitted);`<br>`    }` |
| 10 | A never-revealed commitment is slashed as soon as a side is established | E2 (Solidity reference, A7) | contracts/src/libraries/SentinelOracleRequests.sol:202-205 | `            uint128 nonRevealerCount = prog.committedCount - prog.revealedCount;`<br>`            unchecked {`<br>`                unrevealedBond = nonRevealerCount * self.terms.slashAmount;`<br>`            }` |
| 11 | Whether a given engine returns a different verdict on the second call | **I** — depends on engine implementation and external state, not on this checkout | crates/sentinel-engine (not in this reviewer's scope) | — |

## Trigger

Variant 2 (`Unknown` on the replay) is the one that needs no assumption about engine determinism, so it is stated first. Gnosis defaults: `max_reorg_depth = 5`, 5 s blocks, sentinel and engine co-deployed (A3).

1. Block `b`: `TransactionProposed` + `NewRequest`. The sentinel enters `WaitingForEngineCheck { request: Some(..) }` and spawns the engine check.
2. Block `b+1`: the engine answers `Denied(R-2.1)`. `commit_vote` stores `reason = "R-2.1"`, hashes it, and queues `approve` + `commit`.
3. Block `b+2`: the `commit` is mined. `bondTarget` is now locked onchain behind `hash("R-2.1")`. The transaction queue's row is durable and independent of the state machine (`core/tx/storage.rs:89-104`).
4. Block `b+3`: the pod is restarted (deploy, config change, OOM kill, node drain). **Both** containers restart — the sentinel and its co-deployed engine.
5. Startup: `indexed.safe ≈ b-2`, so `blocks.rs:261-266` queues `Uncle{b-1}`; the state machine rolls back to the snapshot at `b-2`, in which the request does not exist. Blocks `b-1 .. b+3` are re-delivered immediately, with no block-time pacing.
6. The replayed `TransactionProposed` at block `b` re-inserts `WaitingForEngineCheck` and spawns a **second** engine check — within milliseconds of process start.
7. The engine container is still booting, so the HTTP request is refused or times out → `CheckOutcome::Unknown` (basis 3).
8. `handle_engine_check_result` has already removed the entry (basis 4) and returns without re-inserting it. The request is now completely untracked.
9. No `Reveal` is ever emitted. At `finalize` a peer establishes a side; our `PENDING` commitment is slashed `slashAmount` (basis 10) and the remainder is never claimed, because no tracked entry exists for `handle_resolved` or `handle_arbitration_timeout` to act on.

Variant 1 (changed verdict) follows the same steps 1-6, but at step 7 the engine answers and returns a _different_ verdict — a rule list updated during the deploy, a checker reading chain state at a different block (`Effect::EngineCheck.block` is the proposal's block, but the engine's own RPC lookups are at head), or simply a different rule id for the same violation. Steps 8-9 become: the new `reason` is stored, the duplicate `commit` reverts `AlreadyCommitted` (basis 9), and the `reveal` at the commit deadline reverts `InvalidReveal` (basis 1). Same slash.

The live-reorg form needs no restart: any `Uncle{q}` with `q <= b` replays block `b` while the process is running, with the engine reachable — which makes variant 1 the operative one there.

## Considered and rejected

- **"R7 already refuted this: the verdict is consumed exactly once, and a second resume for the same id finds an advanced entry and is ignored (`service.rs:156-170`)."** This is the rejection that hides the bug, and it does not survive the rollback. The guard at `:158-166` fires only when the entry is still present and has _advanced_. After a rollback the entry has been **deleted** from the restored snapshot and then re-created as `WaitingForEngineCheck` by the replayed `TransactionProposed` — so the replayed resume matches the _first_ arm at `:157` and is consumed normally. R7's own F-SEN-001 basis 5 establishes that the replay re-spawns the effect; the two conclusions are inconsistent, and the replay side is the correct one.
- **"The tests cover this — `stale_engine_check_resume_after_reorg_is_ignored` (`service.rs:1622`) and `stale_engine_check_resume_does_not_disturb_an_already_advanced_request` (`:1835`)."** Both test the opposite direction: a _stale_ resume arriving for an entry that has already advanced. Neither replays the proposal first, so neither produces the fresh `WaitingForEngineCheck` that makes the second verdict acceptable.
- **"The engine is deterministic, so the second verdict equals the first."** Not established, and not establishable from this checkout — hence basis 11 is class `I`. But variant 2 does not need it: `Unknown` arises from transport failure alone (basis 3), and a co-deployed engine restarting alongside the sentinel is the ordinary case, not an exotic one.
- **"This is the same defect as F-SEN-001."** No. They share a trigger (the replay) and an outcome (a slashed bond), but not a mechanism or a fix. F-SEN-001 is about the replayed `Committed(self)` log being discarded because the FSM is in the wrong phase; its fix is to carry `self_committed` across phases. F-SEN-015 is about the _verdict itself_ being re-decided after the commitment is immutable; carrying `self_committed` forward does not help, because the stored `reason` is already wrong. On a warp replay both fire on the same request, independently, and either alone is sufficient to lose the bond.
- **"This is downstream of F-CORE-031."** No. F-CORE-031 is the case where the effect is performed **zero** times (the anchor block is never replayed). This finding is the opposite case — the effect is performed **twice**, exactly as the runtime contract permits (basis 8) — and the defect is that the sentinel treats the second performance as authoritative over an immutable onchain commitment.
- **"The salt could also differ."** It could not, within one signer key: `reveal_salt` is `HKDF-SHA256(ikm = key, salt = domain, info = request_id)` (`hashing.rs:49-53`), deterministic in `request_id`. (Rotating the signer key between commit and reveal _would_ strand every outstanding commitment, but that also changes the sentinel address, and there is no key-rotation feature; noted as an observation, not part of this finding.)
- **"An operator would notice."** The reverting `reveal` is invisible: `mark_executed` compares nonces only (`core/tx/storage.rs:222-232`), nothing inspects execution status, and the `Unknown` variant logs one `warn` that is indistinguishable from an ordinary abstention.

## Remediation options

1. **Never re-decide a request that is already committed onchain.** Before acting on an `EngineCheckResult`, or before emitting `Commit`, read `getCommitment(requestId, self)` via a new effect; if a commitment exists, do not re-query the engine and do not overwrite the stored reason. Strongest fix — it closes variants 1 and 2 together and also closes F-SEN-001's drop path, since a found commitment proves `self_committed`. Cost: one RPC read per replayed request, on the replay path only.
2. **Make the verdict durable at the moment it is produced.** Persist `(request_id, approve, reason)` to its own table inside `handle_engine_check_result`, and on a replay reuse the stored verdict instead of asking the engine again. Cheap and fully local, but it needs a retention policy and a second store alongside the snapshot table.
3. **Snapshot resumes.** Have `handle_resume` commit a snapshot (`core/state/mod.rs:246-258`) so the rollback anchor cannot predate a verdict that has already been acted on. This is the F-CORE-031 remediation; it narrows the window but does not close it, because the anchor can still sit before the proposal block. It also changes a core invariant shared by every service.
4. **Reconcile rather than re-derive at reveal time.** Before emitting `Reveal`, recompute `commit_hash` from state and compare it against the onchain `commitHash` (the unused `hashCommitment` binding at `bindings.rs:49-55` exists for exactly this); on mismatch, alert loudly rather than broadcasting a transaction that is guaranteed to revert. Detection, not prevention — useful alongside option 1 or 2, not instead of them.

Tests to add: a `SentinelTransition` test that drives `proposed_event` → `resolve_engine_check(Denied(R-2.1))` → `committed_event(id, self_address)` → _rollback and replay_ `proposed_event` → `resolve_engine_check(Denied(R-3.4))`, then `NewBlock(commit_deadline + 1)`, asserting the emitted `Reveal` carries `"R-2.1"` (it will carry `"R-3.4"`). A second test with the replayed resume as `Unknown`, asserting the entry survives and a `Reveal` is still emitted (it will not be). A `StateMachine` test that a resume consumed before a rollback is not re-consumed with a different value afterwards.

## Trail

- Critic C-SEN: **drafted by the Critic**, promoted from R7's coverage log (`../state/coverage-logs.md#r7`, "Rejected by my own reading" → "The engine could change the vote after the commit"), which dismissed it with `service.rs:156-171` and the two stale-resume tests. That citation refutes the stale-resume direction only; it does not reach the replay direction, in which the entry has been deleted by the rollback and re-created by the replayed proposal, so the second verdict is consumed normally. Every basis row except 11 re-opened in this checkout. Self-estimate as Critic: 78% — mechanism fully `E2`, variant 2's trigger `E2`, variant 1's trigger `I` because engine determinism cannot be assessed from the sentinel crate.

## QA (QA-CORE-SEN)

**Outcome: Reproduced by inspection (variant 2); variant 1's mechanism reproduced, its trigger remains `I`.** Nothing was executed — no Rust toolchain (`state/baseline.md` §1) — so this stays below the 90-100 band.

**PoC written:** `rust-audit/poc/F-SEN-015/` — `poc_service.rs` (one test per variant) plus a README with literal fixtures and the pass/fail reading. Same binary-only-crate constraint as F-SEN-001. **Run variant 2 first**: it is the one that needs no assumption about engine determinism.

### Reproduced by inspection — variant 2 is fully `E2`

`handle_engine_check_result` (`service.rs:150-194`), read line by line:

```
let (deadline, request) = match state.0.remove(&request_id) {      // :156 — REMOVED FIRST
    Some(RequestState::WaitingForEngineCheck { deadline, request }) => (deadline, request),
    Some(entry) => { …; state.0.insert(request_id, entry); return … }   // re-inserted
    None => { …; return … }
};
let (approve, reason) = match outcome {
    …
    CheckOutcome::Unknown => {
        tracing::warn!(%request_id, "engine check failed; dropping request unanswered");
        return (state, Vec::new());                                 // :176-179 — NOT re-inserted
    }
};
```

The entry is removed at `:156` before the outcome is inspected, and the `Unknown` arm is the only one that returns without putting anything back. So a replayed check that comes back `Unknown` — the likely case on a restart under A3, where the engine is co-deployed and still booting — leaves a request with a **live onchain bond** completely untracked. The replayed `Committed(self)` then hits `handle_committed`'s untracked branch (`:300-306`) and is dropped, no `Reveal` is ever emitted, and the commitment is slashed the moment any peer finalises (`contracts/src/libraries/SentinelOracleRequests.sol:202-205`, `:289-296`).

Variant 1's mechanism is equally `E2`: `commit_vote` stores the new `reason` verbatim (`:214-225`) and `handle_block_advance` reveals `std::mem::take(reason)` from state (`:424-437`), while `reveal` recomputes `keccak256(abi.encodePacked(approve, salt, sentinel, requestId, reason))` and reverts `InvalidReveal` on any difference (`contracts/src/libraries/SentinelOracleCommitments.sol:47-56`, `:103-124`). What is `I` is only whether the engine actually answers differently, which cannot be assessed from this crate — I uphold that classification and record it in `rust-audit/../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md` §2b as a question that needs the deployed engine, **not** a dependency source. (That entry originally sat in the shared `UNRESOLVED-DEPENDENCY-QUESTIONS.md`; it was destroyed when that file was overwritten and has been restored to QA-CORE-SEN's own file pending the Manager's merge.)

I also confirmed the negative that makes this a finding rather than a hypothesis: **nothing anywhere on the path checks whether a commitment already exists onchain.** `grep -n "getCommitment\|hashCommitment"` over `crates/sentinel/src/` returns only the unused binding at `bindings.rs:49-55` — it has no call site.

**Certainty: unchanged at 78%.** Variant 2 alone would justify the top of the `E2` band, but the finding as written is the union of two variants and its title leads with the reason-overwrite, so I leave C-SEN's number. If the team prefers, variant 2 could be split out and carried at 85%; I have not done so, because splitting a Critic-drafted finding is not QA's call.

### Remediation check — option 2 as written is **not implementable**

**Option 2 ("persist `(request_id, approve, reason)` to its own table inside `handle_engine_check_result`") cannot be done.** `handle_engine_check_result` is called from `apply_transition`, which is `fn(&self, S, Message<Event, Resume>) -> (S, Commands<S, Self>)` — not `async`, no `&mut self`, no error channel, and documented as "**all state transitions are non-fallible**" (`crates/core/src/state/mod.rs:74-78`). A SQLite write there is impossible without breaking the purity invariant that every service in the workspace relies on. The option has to be re-expressed as either (a) a `Command::Effect(Effect::PersistVerdict { .. })` whose resume confirms the write — which reintroduces the delivery problem it was meant to avoid — or (b) a write performed by the `EffectHandler` at the moment the verdict is produced, before the `Resume` is returned, which _is_ implementable and is what I think the author meant. Form (b) is sound, and it has a further virtue: the handler is already `async` and already owns the engine call. **Say (b) explicitly, or this option will be implemented as a purity violation.**

**Option 1 (read `getCommitment` before acting on a verdict) is sound and is the strongest fix, with the same shape condition as F-SEN-001 option 2.** It must be an effect, and the resume must carry three outcomes — `Committed(hash)`, `NotCommitted`, `Unavailable` — with `Unavailable` meaning "keep the entry, do not re-decide, reveal from stored state". This is precisely the discipline `effects.rs:20-24` prescribes ("encode outcomes like 'already used' in `Resume`"), and it satisfies "effects may run more than once" and "resume ordering is undefined" without extra work. It closes variants 1 and 2 together, and per its own text it also closes F-SEN-001's drop path.

**Option 3 (snapshot resumes) is unsound; see my QA on F-CORE-031.** Committing inside `handle_resume` at the current `latest` persists a snapshot for a block whose logs have not been applied, and a crash in that window makes the state machine resume at `latest + 1` and skip those logs permanently. The option's own text admits it "does not close" the reorg case; the new failure mode it introduces makes it worse than the problem.

**Option 4 (compare against the onchain `commitHash` before revealing) is sound as detection.** The `hashCommitment` binding at `bindings.rs:49-55` exists and is unused, so the plumbing is half done. Note it is an effect too, so a failed read must not suppress the reveal — revealing a possibly-stale hash costs gas, not `slashAmount`.

**Recommendation:** option 1, implemented with the three-state resume. Option 2 in form (b) is a reasonable cheaper alternative for the restart case only. Option 4 alongside either.

**Interaction the finding does not mention.** Any of options 1, 2 or 4 makes the sentinel emit _fewer_ duplicate `commit`s on replay, which reduces F-CORE-067's blast radius for this crate but does not remove it — the `approve` and, in variant 1, the `reveal` are still re-queued with no de-duplication. F-CORE-067 remains the canonical fix for that half.

## Verification (V-CORE-SEN, Phase 5)

**Executed. Reproduced (both variants). `E1`.**

`rust-audit/poc/F-SEN-015/poc_service.rs` was appended verbatim to the existing `#[cfg(test)] mod tests` block at the bottom of `crates/sentinel/src/service.rs` and run with

```
cargo test -p sentinel --bin sentinel service::tests::poc_f_sen_015
```

**No mechanical repair was needed**; the PoC compiled unmodified and no assertion was altered. The file was reverted with `git checkout -- crates/sentinel/src/service.rs`. Full output in `rust-audit/poc/F-SEN-015/RESULT-V-CORE-SEN.out`.

### Verbatim result

```
running 2 tests
test service::tests::poc_f_sen_015_unknown_verdict_on_replay_drops_an_already_committed_request ... FAILED
test service::tests::poc_f_sen_015_replayed_verdict_overwrites_the_committed_reason ... FAILED

---- poc_f_sen_015_unknown_verdict_on_replay_drops_an_already_committed_request stdout ----
thread '...' panicked at crates/sentinel/src/service.rs:2684:5:
handle_engine_check_result removed the entry at service.rs:156 and the Unknown arm returned without
re-inserting it (:176-179) — the request is now untracked despite 500 being bonded onchain

---- poc_f_sen_015_replayed_verdict_overwrites_the_committed_reason stdout ----
thread '...' panicked at crates/sentinel/src/service.rs:2580:5:
assertion `left == right` failed: the reveal carries the SECOND verdict's reason, not the one the
onchain commitment was built from — reveal will recompute a hash !=
0xbb6eb0492cf1b45887d128721e93a2b8097de56b808f711a786b5146b64d7dbb and revert InvalidReveal,
leaving the commitment PENDING and slashing slashAmount (500)
  left:  [Action(SentinelAction { kind: Reveal { id: 0x0eab17c7…3ef616b7, approve: false,
           salt: 0x4fb53d55…b94aee12c, reason: "R-3.4" }, expires_at: Some(40) })]
 right: [Action(SentinelAction { kind: Reveal { id: 0x0eab17c7…3ef616b7, approve: false,
           salt: 0x4fb53d55…b94aee12c, reason: "R-2.1" }, expires_at: Some(40) })]

test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 41 filtered out
```

Both failures are **the finding reproducing**, at the assertions the PoC README nominates as diagnostic; neither is the "harness is wrong" failure (variant 1's earlier `engine_check_effect` assertion — that the duplicate-proposal guard at `service.rs:119-126` cannot fire after a rollback — **held**).

### What is now established by execution rather than by reading

1. **Variant 2, the one the README says settles the finding, reproduces.** A replayed engine check that resumes `Unknown` — needing nothing but the engine being unavailable at the instant of replay, no determinism assumption at all — removes the entry at `service.rs:156` and returns without re-inserting it (`:176-179`). The request is untracked while 500 is bonded onchain, and no `Reveal` is ever emitted for it.
2. **Variant 1 also reproduces**, and its output pins the numbers: the salt is _identical_ (`0x4fb53d55…`, derived deterministically from the request id) while the reason changed from `"R-2.1"` to `"R-3.4"`. The reveal is therefore not merely late or absent — it is a _wrong preimage_ for a commitment whose stored hash the test names, `0xbb6eb0492cf1b45887d128721e93a2b8097de56b808f711a786b5146b64d7dbb`. The `approve` flag is unchanged, so the divergence is carried entirely by the free-text field, which is exactly the part no operator would expect to be consensus-critical.

### Residual uncertainty

Variant 1's _trigger_ still depends on the deployed engine returning a different answer to the same question — a property of the engine, not of the `sentinel` crate, which the Critic classified `I` and which execution here cannot change. Variant 2 does not depend on it. The onchain consequence (`reveal` reverting `InvalidReveal`, then `slashAmount` being taken on any peer's finalisation) remains the Solidity reference under A7, unrunnable without Foundry.

**Basis class:** `E1` for the state-machine defect. **Certainty: 78% → 95%. Status: Critiqued → Verified.** Severity unchanged (High / High) — carried by variant 2, which needs no engine-determinism assumption.

## Real-world validation (Phase 8, RW-CORE-SEN)

**Verdict: Reproduced end-to-end.** The real `SentinelOracle` rejected the mismatched reveal with `InvalidReveal` and the bond was slashed.

### Scenario

Same local-only rig (Anvil `http://127.0.0.1:8645`, chain 31337, 1 s blocks; every config's effective `rpc` printed and asserted local; no sample config used). Real `SentinelOracle`, `REQUEST_FEE = 1000`, `bondTarget = 4000`, `slashAmount = 2000`, commit window 40, reveal window 15.

Sentinel A runs against an engine that answers `secure` the first time and `insecure` / `R-4.1` thereafter — the realistic case of an operator adding the destination to the charter blocklist, or a chain-state-dependent check flipping, between the original decision and the replay. Sentinel B is an unchanged control. A's process is really killed and really restarted two blocks after its `Committed` lands, against the same file-backed database.

Two configurations were run, because the outcome turns on a race the finding does not claim to resolve:

- **`s3b`, `max_reorg_depth = 40`** — a legitimate operator setting for a chain with deep reorgs. The replay window spans the proposal, and the engine's (instant) second answer arrives before the replayed `Committed` log.
- **`s3a`, default `max_reorg_depth = 5`** — the replayed `Committed` arrives first.

### Verbatim outcome — `s3b`

Engine trace, both decisions on the same `request_id`:

```
CALL#1 rid=0x27d6f6b4… -> delay=8000ms verdict=secure
CALL#2 rid=0x27d6f6b4… -> delay=0ms    verdict=insecure rule=R-4.1
```

A's transactions:

```
0x27 nonce=0x0 fn=approve
0x27 nonce=0x1 fn=commit    -> commitHash 0x9620bf72a60674aab7f43e9a9adf7f7b3b3126a584c1f71a8276d15d5172bab5
0x2a nonce=0x2 fn=approve                       <- replay duplicate
0x2a nonce=0x3 fn=commit                        <- replay duplicate
0x48 nonce=0x4 fn=reveal                        <- built from the SECOND verdict
```

Anvil's own transaction log:

```
Transaction: 0xe0ba55ff…  (the duplicate commit)
Error: reverted with: custom error 0xbfec5558      == AlreadyCommitted
Transaction: 0x47617bbd…  (the reveal)
Error: reverted with: custom error 0x9ea6d127      == InvalidReveal
```

`cast sig` confirms `AlreadyCommitted -> 0xbfec5558` and `InvalidReveal -> 0x9ea6d127`. This is outcome 1 of the claim, verbatim: the duplicate commit reverted leaving the original hash in place, and the reveal built from the re-decided verdict failed the on-chain hash check.

Final on-chain state: `state = 3`, `committedCount = 2`, `revealedCount = 1`, `approveSentinelCount = 1`; A's commitment `vote = 1` (`PENDING`), `claimed = false`.

| Account                              | Before    | After     | Delta      |
| ------------------------------------ | --------- | --------- | ---------- |
| Sentinel A                           | 1,000,000 | 996,000   | **−4,000** |
| Sentinel B (control)                 | 1,000,000 | 1,001,000 | +1,000     |
| Protocol funds receiver (slash sink) | 0         | **2,000** | +2,000     |
| Oracle (A's unclaimed remainder)     | 0         | 2,000     | +2,000     |

### Verbatim outcome — `s3a` (default `max_reorg_depth = 5`)

The re-decision itself still happened at default settings — the engine trace again shows `CALL#1 secure` then `CALL#2 insecure R-4.1`, so the replay really does hand an already-committed vote back to the engine and rebuild the entry from the new verdict. But the replayed `Committed` logs arrived first (`2 × "ignoring unexpected commitment" state=waiting_for_engine_check`), so `self_committed` stayed false and A dropped the request **without revealing at all** — the F-SEN-001 path. A sent only `approve, commit, approve, commit` (nonces 0-3) and no reveal. Balances were identical: A −4,000, funds receiver +2,000, oracle 2,000 locked.

### Reading

The mechanism — a replayed engine check re-deciding an already-committed vote — reproduces at **default configuration**. Which of the two losses lands depends on whether the engine answers before the replayed `Committed` log, and both losses are the same 4,000 fee tokens. The specific `InvalidReveal` consequence needs a replay window wide enough for the engine to win that race; at `max_reorg_depth = 5` with a fast engine, F-SEN-001 gets there first and masks it.

**Certainty: 95% → 97%.** Severity unchanged (High / High). Raised because the re-decision, the `AlreadyCommitted` revert and the `InvalidReveal` revert were all observed against the real contract, but held below the F-SEN-001/002 level because the headline consequence is race-dependent.

## Post-merge revalidation (RV-SEN)

Re-validated against merge commit `a7f3915` (baseline `2893917`).

### Verdict: **STILL VALID** — nothing on this path changed

**Merged-code citations (all byte-identical, same line numbers):**

- `handle_engine_check_result`: `crates/sentinel/src/service.rs:150-194` — still removes the entry at `:156` and re-consumes the second verdict; the `CheckOutcome::Unknown` arm still returns without re-inserting at `:175-179`.
- `commit_vote`: `crates/sentinel/src/service.rs:198-246` — still recomputes `commit_hash` from the new `(approve, reason)` at `:212-213` and stores the new `reason` at `:214-225`.
- The reveal built from local state: `service.rs:424-437`.
- `crates/sentinel/src/engine.rs` is untouched by the merge; `crates/core` is byte-identical, so the `core/state/mod.rs:54-73` and `core/index/blocks.rs:255-278` citations are unchanged.
- `hashCommitment` is still declared and still unused — now `crates/sentinel/src/bindings.rs:52-58` (was `:49-55`; three new event declarations were inserted above it at `:42`, `:44`, `:45`).
- Solidity: the `InvalidReveal` recomputation is unchanged; the slash rule moved to `contracts/src/libraries/SentinelOracleRequests.sol:286-293`, and the eager `unrevealedBond` slash inside `finalize` to `:199-201`.

Nothing anywhere on the merged path reads the onchain commitment before revealing. `[Part 7]` (`199629e`) added oracle-event handling _after_ `finalize`; it does not make the pre-commit path replay-safe.

### Evidence (execution-verified on the merged tree)

`rust-audit/poc/F-SEN-015/poc_service.rs` was pasted unmodified into the merged tests module, compiled with no edits, and both tests failed exactly as at `2893917`:

```
test service::tests::poc_f_sen_015_replayed_verdict_overwrites_the_committed_reason ... FAILED
  the reveal carries the SECOND verdict's reason, not the one the onchain commitment was built from
  left:  [ … Reveal { …, approve: false, reason: "R-3.4" } … ]
  right: [ … Reveal { …, approve: false, reason: "R-2.1" } … ]

test service::tests::poc_f_sen_015_unknown_verdict_on_replay_drops_an_already_committed_request ... FAILED
  handle_engine_check_result removed the entry at service.rs:156 and the Unknown arm returned
  without re-inserting it (:176-179) — the request is now untracked despite 500 being bonded onchain
```

Outcome 2 (`Unknown` on replay) is also unrescued by the new handlers: the entry is untracked, and all three terminal handlers no-op on an untracked request — the same probe result recorded under F-SEN-001's _branch B_.

### Certainty and severity

**Certainty: 97% → 98%.** Severity unchanged (**High / High**). Status left at `Verified`.

## In-flight impact (AS-SEN)

**Pertains to unmerged branches, not to `main`.** Assessed against `origin/fix/sentinel_deadlines` (PR #914) and `origin/feat/optimistic_block_transition` (PR #915, tip `b2aad06`), built from a `git archive` extraction. Both apply cleanly onto `main` — `main`'s crates are byte-identical to their base `199629e`, so a verdict on the tip is also the verdict for the tip merged into `main`. **Effect: unchanged.** `service.rs:156` still removes the entry and the `Unknown` arm (`:176-179`) returns without restoring it; both PoC tests fail unmodified.

## Scope note (run 2)

`crates/sentinel-engine` was placed out of scope after this finding was written (PROMPT.md Section 11); the engine-specific parts above are retained as run-1 history and are not part of the current audit's claims. The non-engine parts stand.

## Reconciliation (run 2)

**Final combined status: Confirmed, High, 98% (E1), canonical `F-SEN-015`.** Run 2 folded this into `F2-SEN-001` variant (c) — "the re-run engine check answers differently: `Unknown` drops the entry; a different `approve`/`reason` yields `InvalidReveal`" — and weighted it as `I` ("engine out of scope, not needed for the verdict"; QA did not attempt it). That under-weights it: variant 2 here (`Unknown` on replay) needs no engine-determinism assumption, only a co-deployed engine still booting when the replayed effect fires (A3), and run 1 executed both variants at the state-machine level (`poc/F-SEN-015/`) and observed `AlreadyCommitted` then `InvalidReveal` against the real contract on Anvil. Settled in run 1's favour; this row keeps its own canonical ID and number. Still valid at `fe9e84c`: `handle_engine_check_result` removes the entry first and the `Unknown` arm never re-inserts it (`crates/sentinel/src/service.rs:156-171`, `176-179`), `commit_vote` re-hashes and stores the new `reason` (`214-224`), the reveal is built from state (`444-455`), and `hashCommitment` is bound at `bindings.rs:54-60` with no call site (`git grep`). Team disposition: **assigned @rmeissner**. Fix shape agreed across both runs: a three-state `getCommitment` effect (option 1; `Unavailable` keeps the entry and reveals from stored state), or option 2 in form (b) — the effect handler persists `(request_id, approve, reason)` idempotently, which is exactly `F2-SEN-001`'s reshaped option 1 with its `RecoverVerdict` effect; both runs reject "snapshot resumes" (option 3) as unsound. One fix closes `F-SEN-001` and this finding together. See `state/run2/reconciliation/sentinel.md` §1.2, §4.
