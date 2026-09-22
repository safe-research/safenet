# F-SEN-002 Commitments seen before the engine verdict are discarded, so early finalisation fires with `self_revealed == false` and the bond and reward are never claimed

| Field | Value |
| --- | --- |
| Status | Verified |
| Crate and module | sentinel, service.rs |
| Location | crates/sentinel/src/service.rs:307-319, 372-384, 626-633 (related: 415-417, 466) |
| Severity | High / High |
| Certainty | 98% (RW-CORE-SEN, Phase 8 real-world) |
| Assumptions involved | A2, A4, A10 |
| Tags | funds, input-validation |

## Claim

`committed_count` starts at `0` when `commit_vote` runs (`service.rs:222`) and only counts `Committed` logs that arrive **after** that moment, because every `Committed` seen in another phase is discarded (`service.rs:307-319`). The FSM's early-finalise trigger is `revealed_count >= committed_count` (`service.rs:372-374`). Any commitment from another sentinel that lands before our engine answers is therefore invisible, the trigger fires one or more reveals too early, and `finalize` is entered while `self_revealed` is still `false`.

`finalize` then hits `if !*self_revealed && !timed_out { return (None, Vec::new); }` (`service.rs:631-633`): the entry is deleted with **no `Finalize` and no `Claim`**, even though the sentinel has a live bonded commitment and (once its already-queued `Reveal` lands) is on a revealed side entitled to `bondTarget` back plus its share of the fee. Nothing later re-creates the entry — `Revealed`, `DisputeResolved`, `ArbitrationTimedOut` and `Claimed` for an untracked id are all no-ops (`service.rs:340-346`, `502-508`, `557-563`) — so the sentinel never calls `claim`.

The mirror-image case is just as bad: if _our_ reveal lands first, the undercounted tally makes `finalize` emit `Finalize` + `Claim` before the request is finalisable onchain. Both revert (`FinalizeTooEarly`, `RequestNotResolved`) and the entry is dropped anyway, so a subsequent `DisputeResolved` also finds nothing.

Because bonds are pulled per request and this failure leaves them unclaimed, the loss compounds: a sentinel whose engine is consistently slower than its peers' progressively drains its fee-token balance into unclaimed oracle bonds until it can no longer commit at all.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | `Committed` outside `CollectingCommitments` is discarded, so it never reaches `committed_count` | E2 | crates/sentinel/src/service.rs:307-319 | `        let RequestState::CollectingCommitments {`<br>`            committed_count,`<br>`            self_committed,`<br>`            ..`<br>`        } = entry`<br>`        else {`<br>`            tracing::warn!(`<br>`                request_id = %event.requestId,`<br>`                state = entry.name,`<br>`                "ignoring unexpected commitment"`<br>`            );`<br>`            return (state, Vec::new);`<br>`        };` |
| 2 | The tally starts at zero when we commit, so anything earlier is lost | E2 | crates/sentinel/src/service.rs:214-225 | `        state.0.insert(`<br>`            request_id,`<br>`            RequestState::CollectingCommitments {`<br>`                approve,`<br>`                reason,`<br>`                slash_amount,`<br>`                commit_deadline,`<br>`                reveal_deadline,`<br>`                committed_count: 0,`<br>`                self_committed: false,`<br>`            },`<br>`        );` |
| 3 | Early finalisation is driven purely by the local tally | E2 | crates/sentinel/src/service.rs:363-384 | `        *revealed_count += 1;`<br>`        if event.approved {`<br>`            *approve_count += 1;`<br>`        } else {`<br>`            *deny_count += 1;`<br>`        }`<br>`        if event.sentinel == self.signer.address {`<br>`            *self_revealed = true;`<br>`        }`<br>`        if *revealed_count < *committed_count {`<br>`            return (state, Vec::new);`<br>`        }`<br>`        let (update, actions) = self.finalize(entry, event.requestId);` |
| 4 | `finalize` drops the entry with no action when we have not (yet) been seen to reveal | E2 | crates/sentinel/src/service.rs:626-633 | `        let dispute = *approve_count > 0 && *deny_count > 0;`<br>`        let timed_out = *revealed_count == 0;`<br>``<br>`        // If this sentinel did not participate and it was not a timeout`<br>`        // then no actions should be taken and the request should be dropped`<br>`        if !*self_revealed && !timed_out {`<br>`            return (None, Vec::new);`<br>`        }` |
| 5 | A later `Revealed` / `DisputeResolved` for an untracked id is a silent no-op, so nothing recovers the entry | E2 | crates/sentinel/src/service.rs:340-346 | `        let Some(entry) = state.0.get_mut(&event.requestId) else {`<br>`            tracing::debug!(`<br>`                request_id = %event.requestId,`<br>`                "ignoring reveal for an untracked request"`<br>`            );`<br>`            return (state, Vec::new);`<br>`        };` |
| 6 | Claiming is per sentinel and is the only way a bond comes back | E2 (Solidity reference, A7) | contracts/src/SentinelOracle.sol:286-304 | `    function claim(bytes32 requestId) external {`<br>`        SentinelOracleRequest.T storage request = $requests.get(requestId);`<br>`        SentinelOracleRequest.State state = request.requireResolved;`<br>`        SentinelOracleCommitment.Commitment storage commitment = $commitments.get(requestId, msg.sender);`<br>`        SentinelOracleCommitment.Vote vote = commitment.vote;`<br>`        commitment.markClaimed;`<br>`        uint96 feeReward = request.calcFeeReward(state, vote);`<br>`        uint96 bondReturn = commitment.bondAmount - request.slashAmountFor(state, vote);` |
| 7 | Onchain finalisation requires the _real_ committed count, so an undercounted early finalise reverts | E2 (Solidity reference, A7) | contracts/src/libraries/SentinelOracleRequests.sol:175-177 | `        bool everyoneRevealed = prog.committedCount > 0 && prog.revealedCount == prog.committedCount;`<br>`        bool nothingToReveal = prog.committedCount == 0 && block.number > self.terms.commitDeadline;`<br>`        require(block.number > self.terms.revealDeadline \|\| everyoneRevealed \|\| nothingToReveal, FinalizeTooEarly);` |

## Trigger

Two registered sentinels, A (this one) and B. No restart, no reorg, no attacker needed — only a slower engine on A:

1. Block `b`: `proposeTransaction` emits `TransactionProposed` and `NewRequest`. A enters `WaitingForEngineCheck { request: Some(..) }` and spawns its engine check.
2. Block `b+1`: B's engine is faster; B's `commit` is mined. A processes `Committed(B)` while still in `WaitingForEngineCheck` → discarded at `service.rs:307-319` (`"ignoring unexpected commitment"`).
3. Block `b+2`: A's engine resumes → `commit_vote` → `CollectingCommitments { committed_count: 0, self_committed: false }`.
4. Block `b+3`: A's `Committed(self)` → `committed_count = 1`, `self_committed = true`. Onchain `committedCount` is 2.
5. Block `commit_deadline + 1`: A emits its `Reveal` and moves to `CollectingVotes { committed_count: 1, revealed_count: 0, self_revealed: false }`.
6. Block `commit_deadline + 2`: `Revealed(B)` → `revealed_count = 1 >= committed_count = 1` → `finalize` with `self_revealed == false`, `timed_out == false` → **entry deleted, no actions**.
7. Block `commit_deadline + 3`: A's own reveal is mined. `Revealed(self)` finds no entry (`service.rs:340-346`) and is ignored.
8. The request resolves. A is on a revealed side with a full bond and (if it won) a fee share, and never submits `claim`. The funds stay in the oracle.

Variant (A reveals first): at step 6 `Revealed(self)` fires the same undercounted trigger with `self_revealed == true`, so `Finalize` + `Claim` are emitted while the request is still `PENDING`. `finalize` reverts with `FinalizeTooEarly` (basis 7) and `claim` with `RequestNotResolved` (`SentinelOracleRequests.sol:239-245`). The entry is still deleted, so if B later reveals the opposite vote and the request freezes, the eventual `DisputeResolved` is ignored (`service.rs:502-508`) and the bond is never claimed.

A second, independent way to undercount: A4 permits an RPC to return incomplete `eth_getLogs` results, so a missing `Committed` log has exactly the same effect without any latency skew.

## Considered and rejected

- **"`committed_count` is only used as a lower bound, so an undercount is harmless."** It is used as an equality-style threshold at `service.rs:372-374` (`if *revealed_count < *committed_count { return ... }`), so an undercount makes the condition true too early. There is no second guard.
- **"The reveal-deadline path would catch it later."** No — the entry is already deleted at step 6, so `handle_block_advance`'s `CollectingVotes` arm (`service.rs:450-465`) never sees it again.
- **"`self_revealed` will be true by the time we finalise."** Only if our own `Revealed` log has already been processed. The FSM emits its `Reveal` as a _queued transaction_; between queuing and inclusion, other sentinels' reveals arrive. The tests only exercise the ordering where our reveal lands last (`service.rs:1332-1351`) or where nobody reveals (`service.rs:1598-1615`).
- **"Dropping the entry is correct when we did not participate."** The comment at `service.rs:629-630` says exactly that, but `self_revealed == false` does not mean "did not participate": it means "we have not observed our own reveal yet". The FSM already knows it participated — `self_committed` was true one phase earlier, and it is discarded when `CollectingVotes` is constructed (`service.rs:438-447` carries `committed_count` but not `self_committed`).
- **"`Claimed` could be used to notice we forgot."** `handle_claimed` (`service.rs:584-593`) only records a metric and explicitly never touches state.
- **"An operator would see it."** The discarded commitment logs at `warn` level, but the dropped-entry path at `service.rs:631-633` logs nothing at all, and no metric distinguishes it (`requests_resolved_total` is only incremented on the branches that _do_ act, `service.rs:663`).
- **False positive check — is `WaitingForEngineCheck` really still the state when B commits?** Yes: `handle_new_request` for an entry in `WaitingForEngineCheck { request: None }` only stores the terms and stays in that state (`service.rs:264-276`); only the engine resume leaves it (`service.rs:181-183`).

## Remediation options

1. **Carry participation forward and never drop a bonded entry silently.** Add `self_committed` (or a `bonded: bool`) to `CollectingVotes` and change `service.rs:631-633` to emit `Claim` whenever the sentinel is known to have bonded, regardless of `self_revealed`. `claim` is idempotent-by-revert (`markClaimed` guards `AlreadyClaimed`, `SentinelOracleCommitments.sol:43-45`) and pays `0` harmlessly on a losing side, so an unnecessary claim costs only gas.
2. **Stop early-finalising on a local tally.** Trigger finalisation only at `reveal_deadline + 1`, or gate the early path on an effect that reads the oracle's own `committedCount`/`revealedCount` for the request. Removes the undercount dependency entirely; costs one round trip or one extra reveal-window wait.
3. **Count commitments in every pre-commit phase.** Let `handle_committed` tally into `WaitingForEngineCheck`/`WaitingForRequest` (a `committed_count` field on those variants, carried into `CollectingCommitments` by `commit_vote`). Fixes the undercount at its source and also helps F-SEN-001.
4. **Retain the entry until a terminal onchain event is seen.** Keep a lightweight `WaitingForClaim { .. }` state after finalisation instead of deleting, and clear it only on our own `Claimed` event. Bounds the "we forgot to claim" class in general, at the cost of unbounded growth if a request never resolves (see F-SEN-005).

Tests to add: a flow test that inserts `committed_event(id, OTHER, ..)` _between_ `proposed_event` and `resolve_engine_check`, then drives the full lifecycle and asserts a `Claim` is emitted. A test for `finalize` with `self_revealed = false, revealed_count > 0` asserting it does not silently drop a bonded entry. An integration-script scenario that delays sentinel A's engine by two blocks.

## Trail

- Reviewer R7: drafted from lead SEN-H1, self-estimate 85%. All seven basis citations re-opened in this checkout; the Solidity references were read directly.

## Critic (C-SEN)

Derived independently before reading R7's text: `commit_vote` seeds `committed_count: 0` (`service.rs:222`), `handle_committed` accepts only `CollectingCommitments` (`:307-319`), and `handle_revealed`'s early trigger is `revealed_count < committed_count` (`:372-374`) — so any peer commit that lands before our engine answers is invisible and the trigger fires early. I reached the same conclusion R7 did, including the `!self_revealed && !timed_out` silent drop at `:631-633`.

### Per-claim verdicts

| # | Verdict | Note |
| --- | --- | --- |
| 1 | **Supported** | Verbatim at `service.rs:307-319`. |
| 2 | **Supported** | Verbatim at `service.rs:214-225`; `committed_count: 0`, `self_committed: false`. |
| 3 | **Supported** | Verbatim at `service.rs:363-384`. Note `revealed_count` counts reveals from _any_ sentinel, so it can reach the undercounted `committed_count` on someone else's log. |
| 4 | **Supported** | Verbatim at `service.rs:626-633`. `finalize` returns `(None, Vec::new)`, and both call sites (`:376-383`, `:456-464`) treat `None` as "remove the entry". |
| 5 | **Supported** | Verbatim at `service.rs:340-346`. I also confirmed the same shape for `handle_resolved` (`:502-508`) and `handle_arbitration_timeout` (`:557-563`), so nothing re-creates the entry. |
| 6 | **Supported** | `SentinelOracle.sol:286-297` verbatim; `claim` is keyed on `msg.sender`, so no peer can claim on our behalf. |
| 7 | **Supported** | `SentinelOracleRequests.sol:174-177` verbatim (I read `:166-215` in full). |

One overstatement in the Claim, not in the Basis: "entitled to `bondTarget` back **plus** its share of the fee" — `calcFeeReward` (`SentinelOracleRequests.sol:252-270`) pays a fee share only on the winning/established side. The bond return is unconditional for a revealed, non-losing vote; the fee is not. Immaterial to the finding, worth correcting in the report.

### The two failure modes are asymmetric, and both are real

- **Peer reveals first** (steps 1-8): entry deleted with `self_revealed == false`; our reveal is already queued and lands; onchain we are a revealed voter owed `bondTarget` (+ fee if we won), and we never call `claim`. Funds are **locked, not slashed** — an operator can recover them by calling `claim` by hand, if they ever notice.
- **We reveal first** (the variant): `Finalize` + `Claim` are emitted while the request is still `PENDING`. `finalize` reverts `FinalizeTooEarly` (`SentinelOracleRequests.sol:177`) and `claim` reverts `RequestNotResolved` (`:239-245`); the entry is deleted regardless, so a later `DisputeResolved` is ignored and the bond is again never claimed.

I checked the one path that could rescue it and it does not exist: `handle_claimed` (`service.rs:584-593`) is metrics-only by design, and no `Claimed`, `OracleResult`, `DisputeTriggered` or `RequestTimedOut` handler re-creates state.

I also checked whether the ordering is safe in normal operation and it is **not**, for a reason worth recording: `applyReveal` requires `block.number > commitDeadline` (`SentinelOracleRequests.sol:129`), and the state machine delivers `Message::NewBlock(n)` before the logs of block `n` (`state/mod.rs:190-199` then `:200-239`), so our own `CollectingCommitments → CollectingVotes` transition always precedes any peer reveal in normal block-by-block operation. The defect therefore does **not** need a warp or a reorg — the undercount alone is sufficient, exactly as R7's trigger says.

### Finding verdict

**Confirmed. Certainty 84%. Severity High (unchanged).**

Mechanism and trigger are both `E2` and need no attacker, no restart and no reorg — only a peer whose engine is faster, which is the expected steady state for any sentinel that is not the fastest in the set. Severity High per Section 8: the loss is silent, systematic and compounding — every affected request permanently parks one `bondTarget` in the oracle until the fee-token balance can no longer fund a commit, which converts a funds problem into a total participation outage. It is one notch below F-SEN-001 only because the funds are recoverable by a manual `claim`.

A4 gives this a second, independent trigger with no latency assumption at all: an incomplete `eth_getLogs` response that drops one `Committed` log produces the same undercount.

### Notes for QA

`resolve_engine_check` in the existing flow tests already makes this reproducible without a chain: insert `committed_event(id, OTHER, ..)` between `proposed_event` and `resolve_engine_check`, then run the lifecycle and assert on the emitted actions. Expect no `Claim`.

## QA (QA-CORE-SEN)

**Outcome: Reproduced by inspection.** Nothing was executed (no Rust toolchain, `state/baseline.md` §1), so this stays below the 90-100 band, which needs `E1`.

**PoC written:** `rust-audit/poc/F-SEN-002/` — `poc_service.rs` (two tests, one per failure mode) plus a README with literal fixtures and the pass/fail reading. Same binary-only-crate constraint as F-SEN-001: the tests must be pasted into `crates/sentinel/src/service.rs`'s existing `mod tests`.

### Reproduced by inspection

- `commit_vote` seeds `committed_count: 0` (`service.rs:214-225`, read in full).
- `handle_committed` accepts only `CollectingCommitments` (`:296-322`), so a peer commit in any earlier phase is discarded — and `handle_new_request` for `WaitingForEngineCheck { request: None }` stores the terms and **stays in that phase** (`:264-276`), so the window is the entire engine round trip, not a sliver of it.
- The early-finalise trigger is `if *revealed_count < *committed_count { return … }` (`:372-374`), an equality-style threshold with no second guard. `revealed_count` counts reveals from _any_ sentinel, so an undercounted `committed_count` is reached on someone else's log.
- `finalize`'s `if !*self_revealed && !timed_out { return (None, Vec::new); }` (`:626-633`) and both call sites treat `None` as "remove the entry" (`:376-383`, `:456-464`).
- Nothing re-creates it: I checked all four terminal handlers and each is a no-op for an untracked id — `handle_revealed` `:340-346`, `handle_resolved` `:502-508`, `handle_arbitration_timeout` `:557-563`, `handle_claimed` `:584-593` (metrics only, touches no state).

**I confirm C-SEN's ordering point, which is what makes this need no reorg.** `applyReveal` requires `block.number > commitDeadline` (`contracts/src/libraries/SentinelOracleRequests.sol:129`), and `StateMachine::handle_update` delivers `Message::NewBlock(n)` in one update and block `n`'s logs in the next (`state/mod.rs:190-199` then `:200-239`), so the sentinel's own `CollectingCommitments → CollectingVotes` transition always precedes any peer reveal in normal block-by-block operation. The undercount alone is sufficient.

**Certainty: unchanged at 84%.** No new evidence; 89% is the ceiling. Severity High is right.

### Remediation check — at least one option is sound, but the obvious one is incomplete

**Option 3 (count commitments in every pre-commit phase) is sound and is the natural pair with F-SEN-001 option 1 — but it does not close the finding.** It fixes the _latency_ trigger, which is the one in the Trigger section. It does **not** fix the A4 trigger the Critic identified: a `Committed` log genuinely missing from an incomplete `eth_getLogs` response produces the same undercount no matter which phase is willing to count it. A team that takes only option 3 will believe the finding is closed and will still lose bonds to F-CORE-002's log loss.

**Option 1 (carry a `bonded` flag into `CollectingVotes` and always `Claim`) is sound and is the cheapest complete fix for the _loss_.** It is a pure-state change, it needs no effect, and the economics check out: `claim` is idempotent-by-revert (`markClaimed` guards `AlreadyClaimed`, `contracts/src/libraries/SentinelOracleCommitments.sol:43-45`) and pays `0` harmlessly on a losing side. Note it does **not** fix failure mode B — with `bonded` set, an undercounted tally still emits `Finalize` + `Claim` while the request is `PENDING`, and both still revert. It converts a permanent loss into wasted gas, which is the right direction but not the whole job.

**Option 2 (stop early-finalising on a local tally) is the only option that closes both triggers and both failure modes, and it is sound.** Two implementable forms, and the text conflates them:

- _Wait for `reveal_deadline + 1`._ Fully local, no effect, no new failure mode, satisfies the runtime contract trivially. Cost: the sentinel stops finalising early, so `finalize` is delayed by up to the reveal window. Given F-SEN-014 (every participating sentinel submits `finalize` and all but one revert), losing the early path is close to free.
- _Gate the early path on an effect reading the oracle's `committedCount`/`revealedCount`._ Stronger, but subject to the same two conditions as F-SEN-001 option 2: the read must be a `Command::Effect` + `Resume` (transitions are pure and non-`async`), and a **failed** read must leave the entry in place rather than dropping it.

**Option 4 (`WaitingForClaim` until our own `Claimed` is seen) is sound in principle and unbounded in practice.** The finding says so and cross-references F-SEN-005; I agree, and I would add that it must not be taken without F-SEN-005 option 3's deadline, or it converts a funds bug into the snapshot-growth bug F-SEN-005 already documents. Since snapshots are serialised as JSON on every committed block (`state/storage.rs:104-113`), unbounded state is a per-block write cost, not just a disk cost.

**Recommendation:** option 2 in its "wait for `reveal_deadline`" form, plus option 1 as defence in depth. Option 3 is worth taking anyway because it also helps F-SEN-001, but **it must not be reported as closing this finding**.

**One correction, already noted by C-SEN and worth carrying into the report:** the Claim's "entitled to `bondTarget` back **plus** its share of the fee" overstates it — `calcFeeReward` (`contracts/src/libraries/SentinelOracleRequests.sol:252-270`) pays a fee share only on the established side. The bond return is the unconditional part.

## Verification (V-CORE-SEN, Phase 5)

**Executed. Reproduced (both variants). `E1`.**

`rust-audit/poc/F-SEN-002/poc_service.rs` was appended verbatim to the existing `#[cfg(test)] mod tests` block at the bottom of `crates/sentinel/src/service.rs` and run with

```
cargo test -p sentinel --bin sentinel service::tests::poc_f_sen_002
```

**No mechanical repair was needed**; the PoC compiled unmodified and no assertion was altered. The file was reverted with `git checkout -- crates/sentinel/src/service.rs`. Full output in `rust-audit/poc/F-SEN-002/RESULT-V-CORE-SEN.out`.

### Verbatim result

```
running 2 tests
test service::tests::poc_f_sen_002_peer_commit_before_engine_verdict_loses_the_claim ... FAILED
test service::tests::poc_f_sen_002_undercount_finalizes_before_the_request_is_finalisable ... FAILED

---- poc_f_sen_002_peer_commit_before_engine_verdict_loses_the_claim stdout ----
thread '...' panicked at crates/sentinel/src/service.rs:2294:5:
the request was silently dropped at service.rs:631-633 while this sentinel had 500 bonded onchain
and an already-queued Reveal — nothing re-creates the entry, so claim is never called and
bondTarget stays locked in the oracle

---- poc_f_sen_002_undercount_finalizes_before_the_request_is_finalisable stdout ----
thread '...' panicked at crates/sentinel/src/service.rs:2388:5:
Finalize/Claim were emitted at block 22 while the oracle still has one unrevealed commitment:
finalize reverts FinalizeTooEarly and claim reverts RequestNotResolved, and the entry is deleted
anyway — commands = [Action(SentinelAction { kind: Finalize { id: 0x8df07240…2b67d278 },
expires_at: None }), Action(SentinelAction { kind: Claim { id: 0x8df07240…2b67d278 },
expires_at: None })]

test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 41 filtered out
```

Per the PoC README both failures are **the finding reproducing**, and neither is a "harness is wrong" failure: test A reached the post-block-22 `contains_key` assertion, meaning its earlier `committed_count: 1` assertion — the one that makes the undercount visible (local tally 1 against an onchain `committedCount` of 2) — **held**.

### What is now established by execution rather than by reading

1. A peer `Committed` seen while this sentinel's entry is still pre-verdict is discarded, so the local tally is **1** where the oracle's is **2**. This is executed, not inferred.
2. On the peer's `Revealed`, the early-finalise trigger fires and `finalize` takes the `!self_revealed && !timed_out` branch (`service.rs:626-633`): the entry is **deleted with no actions emitted at all**, while this sentinel has 500 bonded onchain and a `Reveal` already queued. Nothing re-creates the entry, so `claim` is never called and `bondTarget` plus the winning-side fee share stay locked in the oracle. The drop is silent — `requests_resolved_total` is only incremented on branches that do act (`service.rs:663`).
3. The mirror form is also executed: at block 22 the undercounted tally makes the sentinel emit `Finalize` **and** `Claim` for a request that still has an unrevealed commitment, then delete the entry regardless — so the later `DisputeResolved` is ignored too.

### Residual uncertainty

Two legs remain read-not-run because no Foundry/anvil exists on this host: that `finalize` reverts `FinalizeTooEarly` and `claim` reverts `RequestNotResolved` in this state (Solidity reference under A7, `contracts/src/libraries/SentinelOracleRequests.sol:175-177`, `:239-245`). The Rust-side loss — a bonded, revealed, winning vote whose entry is deleted without a `Claim` — needs neither.

**Basis class:** `E1`. **Certainty: 84% → 96%. Status: Critiqued → Verified.** Severity unchanged (High / High).

## Real-world validation (Phase 8, RW-CORE-SEN)

**Verdict: Reproduced end-to-end.** 4,500 fee tokens left unclaimed on a real `SentinelOracle`.

### Scenario

Same local-only rig as F-SEN-001 (Anvil `http://127.0.0.1:8645`, chain 31337, 1 s blocks; effective `rpc` printed and asserted local before every process start; no sample config used). Real `SentinelOracle` with `REQUEST_FEE = 1000`, `bondTarget = 4000`, `slashAmount = 2000`, commit window 30, reveal window 15, DAO share 0.

Two real `sentinel` binaries, both voting _approve_. The only asymmetry is engine latency, which is what the claim is about: sentinel A's engine answers the `POST /v1/security-check` after **8 s** (a slow engine — a cold cache, a loaded host, a lookback query against a busy node); sentinel B's answers immediately. Both engines were HTTP stand-ins speaking the real `openapi.yaml` contract so the latency was controllable; nothing else was substituted.

### Verbatim outcome

B committed at block `0x1e`; A only at block `0x26`, eight blocks later. In between, `a.log` contains exactly one warning:

```
{"level":"WARN","fields":{"message":"ignoring unexpected commitment","request_id":"0xfef56620…","state":"waiting_for_engine_check"}}
```

That is B's `Committed`, discarded because A was still in `WaitingForEngineCheck`. A's `committed_count` therefore started at 1 against a true on-chain `committedCount` of 2.

Both sentinels revealed in block `0x3d`. A's transactions, complete:

```
0x26 nonce=0x0 fn=approve
0x26 nonce=0x1 fn=commit
0x3d nonce=0x2 fn=reveal
```

**A never sent `finalize` and never sent `claim`.** The early-finalise trigger (`revealed_count >= committed_count`, 1 >= 1) fired on the first `Revealed` A saw, `finalize` hit `if !*self_revealed && !timed_out { return (None, Vec::new) }`, and the entry was deleted while A's bond was live. B, whose count was correct, finalised at `0x3e` and claimed.

Final on-chain state: `state = 3` (`RESOLVED_APPROVED`), `committedCount = 2`, `revealedCount = 2`, `approveSentinelCount = 2`. A's commitment: `vote = 2` (`APPROVED` — its reveal _did_ land), `claimed = false`.

**On-chain balance change, fee token:**

| Account                              | Before    | After     | Delta      |
| ------------------------------------ | --------- | --------- | ---------- |
| Sentinel A (slow engine)             | 1,000,000 | 996,000   | **−4,000** |
| Sentinel B (fast engine)             | 1,000,000 | 1,000,500 | +500       |
| Oracle (A's unclaimed bond + reward) | 0         | **4,500** | +4,500     |
| Protocol funds receiver              | 0         | 0         | 0          |

No slash occurred — A revealed on the winning side and was entitled to its whole 4,000 bond back plus a 500 fee share. It simply never asked for either. The loss is **4,500 per request**, and it is not recoverable by the running process: nothing re-creates the entry. This is the compounding-drain the claim describes, and it needs nothing more exotic than one sentinel's engine being slower than its peers'.

**Certainty: 96% → 98%.** Severity unchanged (High / High).

## Post-merge revalidation (RV-SEN)

Re-validated against merge commit `a7f3915` (baseline `2893917`). `crates/core` is byte-identical; in `crates/sentinel` only `bindings.rs`, `metrics.rs`, `service.rs`, `state.rs` changed.

### Verdict: **PARTIALLY ADDRESSED** — the primary loss path is STILL VALID; the mirror-image case has CHANGED SHAPE

**Merged-code citations:**

| Cited at `2893917` | Now at | Changed? |
| --- | --- | --- |
| `service.rs:307-319` (every `Committed` in another phase discarded) | `service.rs:307-319` | byte-identical |
| `service.rs:372-384` (early-finalise trigger `revealed_count >= committed_count`) | `service.rs:372-384` | byte-identical |
| `service.rs:626-633` (the silent drop) | `service.rs:633-637` | byte-identical text, moved |
| `service.rs:415-417` | `service.rs:415-417` | unchanged |
| `service.rs:466` (`WaitingForDisputeResolution => true`) | `service.rs:467` | +1; `:466` is now the new `WaitingForOutcome => true` |
| `service.rs:502-508` (`DisputeResolved` for an untracked id is a no-op) | `service.rs:503-509` | +1 |

#### Primary case — STILL VALID

The drop is untouched (`service.rs:633-637`):

```rust
// If this sentinel did not participate and it was not a timeout
// then no actions should be taken and the request should be dropped
if !*self_revealed && !timed_out {
    return (None, Vec::new());
}
```

`rust-audit/poc/F-SEN-002/poc_service.rs` was pasted unmodified into the merged tests module and compiled with no edits. Test 1 failed identically:

```
test service::tests::poc_f_sen_002_peer_commit_before_engine_verdict_loses_the_claim ... FAILED
  the request was silently dropped at service.rs:631-633 while this sentinel had 500 bonded
  onchain and an already-queued Reveal — nothing re-creates the entry, so claim() is never
  called and bondTarget stays locked in the oracle
```

The three new terminal handlers do not rescue it: once the entry is gone the request is untracked, and `handle_request_timed_out` / `handle_dispute_triggered` / `handle_oracle_result` all return `(state, Vec::new())` on the untracked path (`service.rs:674-680`, `:728-734`, `:780-786`) — verified by execution under F-SEN-001's _branch B_ probe.

#### Mirror-image case — CHANGED SHAPE (improved)

`finalize` no longer emits `Claim` and no longer deletes the entry. It emits only `Finalize` and parks in the new state (`service.rs:639-653`):

```rust
let actions = vec![
    SentinelAction { kind: SentinelActionKind::Finalize { id: request_id }, expires_at: None }.into(),
];

(
    Some(RequestState::WaitingForOutcome { approve, slash_amount }),
    actions,
)
```

Executed on the merged tree with PoC test 2's fixture (peer commits before our verdict; our reveal lands first at block 22):

```
RV002 commands at block22 = [Action(SentinelAction { kind: Finalize { … }, expires_at: None })]
RV002 entry after         = Some(WaitingForOutcome { approve: true, slash_amount: 500 })
RV002 dispute commands    = []   entry = Some(WaitingForDisputeResolution { approve: true, slash_amount: 500 })
RV002 block10000 commands = []   entry = Some(WaitingForOutcome { approve: true, slash_amount: 500 })
```

So two of the three sub-claims are now closed by `dbc963a` / `6df6fb9`: the guaranteed-to-revert `claim()` is no longer sent, and the entry survives, so a later real `DisputeTriggered` recovers it into `WaitingForDisputeResolution` and `handle_resolved` does claim.

**What remains in the mirror case:** the premature `Finalize` is still emitted and still reverts `FinalizeTooEarly` (`SentinelOracleRequests.sol:174`), burning gas and a nonce — and, new with this merge, the entry is now parked in a state that `handle_block_advance` never expires (`service.rs:466`; the last probe line above: `NewBlock(10_000)` emits nothing and leaves the entry in place). Nothing re-attempts `Finalize`, so if no other party finalizes the request, the sentinel waits forever. That parking hazard is tracked under **F-SEN-005**.

### Certainty and severity

**Certainty 98% (unchanged); severity unchanged (High / High)** — carried entirely by the primary case, which is intact and re-executed. Status left at `Verified`.

## In-flight impact (AS-SEN)

**Pertains to unmerged branches, not to `main`.** Assessed against `origin/fix/sentinel_deadlines` (PR #914) and `origin/feat/optimistic_block_transition` (PR #915, tip `b2aad06`), built from a `git archive` extraction. Both apply cleanly onto `main` — `main`'s crates are byte-identical to their base `199629e`, so a verdict on the tip is also the verdict for the tip merged into `main`. **Effect: unchanged on the #915 tip; worsened by #914 merged alone.** `service.rs:307-319` and the `finalize` drop at `:647-649` are unchanged, and both PoC tests fail. With #914 alone, `:418` becomes `block < *commit_deadline`, so a peer's valid `Committed` mined in the commit-deadline block is discarded and the request is deleted with a bond posted. Filed as `F-SEN-016` D1 — **#914 should not be merged without #915.**

## Reconciliation (run 2)

**Final combined status: Confirmed, High, 98% (E1), canonical `F-SEN-002`.** Run-2 counterpart `F2-SEN-002` (High 90, E1, both outcomes re-run at `fe9e84c`) **confirms** the undercount and the silent drop, and extends the mirror case into an attacker-usable stall: a reverted `Finalize` now parks the entry in `WaitingForOutcome` with no deadline and no retry (`crates/sentinel/src/service.rs:657-671`, `484`), so one registered sentinel that commits early and never reveals parks every honest sentinel until a third party finalizes. The in-flight note above ("#914 alone worsens this") and run 2's `F2-SEN-010` (High 93) are the same fact: the `CollectingVotes` switch now runs at `NewBlock(commit_deadline)` (`423-425`) before that block's logs, so commits mined in block `commitDeadline` are a second, engine-independent undercount source. It is filed as a distinct canonical finding because its root cause (tally frozen at the phase switch) and remediation (keep tallying in `CollectingVotes`, or stop early-finalizing on local counts) are disjoint from this file's (counter born at zero on the resume; count from `NewRequest` onward). Severity here is unchanged — the trigger set grew, the rating did not. Team disposition: **assigned @rmeissner**; the fix must close both sources — `F2-SEN-010` option 1 was compiled in-tree (47/47 tests), and `F2-SEN-003` option 1 (park instead of drop) makes outcome 1 recoverable (compiled, 42/42 existing tests pass). Run 1's QA caveat stands: counting in every pre-commit phase alone must not be reported as closing this finding (A4 log-loss trigger). See `state/run2/reconciliation/sentinel.md` §1.1, §4.
