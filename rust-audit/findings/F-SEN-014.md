# F-SEN-014 Every participating sentinel submits `finalize` for every request, so all but one revert

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | sentinel, service.rs |
| Location | crates/sentinel/src/service.rs:635-641 (related: 372-384, 450-465, 723-730) |
| Severity | Informational / Informational |
| Certainty | 88% (set by Critic C-SEN; QA may raise) |
| Assumptions involved | A10 |
| Tags | dos |

## Claim

`finalize` unconditionally emits a `Finalize` action on every terminal path that is not the silent-drop branch (`service.rs:635-641`), and it is reached from two places that fire for every participating sentinel at roughly the same time: the early-finalise trigger in `handle_revealed` (`service.rs:372-384`, which fires on the last reveal — the same log for everyone) and the reveal-deadline branch in `handle_block_advance` (`service.rs:450-465`, which fires on the same block for everyone).

`SentinelOracle.finalize` may only run once — it requires `prog.state == State.PENDING` and then assigns a terminal state — so with `K` participating sentinels, `K - 1` of the submitted transactions revert with `RequestNotPending`. Each carries a 250,000 gas limit (`service.rs:723-730`), consumes a nonce, and occupies one of the sixteen in-flight slots that time-critical reveals compete for (F-SEN-004). There is also a thundering-herd effect on the RPC endpoint: every sentinel broadcasts within the same block.

This is a known and arguably deliberate design (someone must call `finalize`, and it is permissionless), so it is filed Informational. It is recorded because the cost scales with the sentinel-set size and with request volume, and because it interacts with the reveal-throughput problem in F-SEN-004.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | `Finalize` is emitted unconditionally on the non-drop paths | E2 | crates/sentinel/src/service.rs:629-641 | `        // If this sentinel did not participate and it was not a timeout`<br>`        // then no actions should be taken and the request should be dropped`<br>`        if !*self_revealed && !timed_out {`<br>`            return (None, Vec::new);`<br>`        }`<br>``<br>`        let mut actions = vec![`<br>`            SentinelAction {`<br>`                kind: SentinelActionKind::Finalize { id: request_id },`<br>`                expires_at: None,`<br>`            }`<br>`            .into,`<br>`        ];` |
| 2 | The early-finalise trigger fires on a log every participant sees at the same time | E2 | crates/sentinel/src/service.rs:372-375 | `        if *revealed_count < *committed_count {`<br>`            return (state, Vec::new);`<br>`        }`<br>`        let (update, actions) = self.finalize(entry, event.requestId);` |
| 3 | The deadline trigger fires on the same block for everyone | E2 | crates/sentinel/src/service.rs:450-457 | `            RequestState::CollectingVotes {`<br>`                reveal_deadline, ..`<br>`            } => {`<br>`                if block <= *reveal_deadline {`<br>`                    return true;`<br>`                }`<br>`                let (update, finalization) = self.finalize(entry, *id);`<br>`                actions.extend(finalization);` |
| 4 | Each carries a 250,000 gas limit and never expires | E2 | crates/sentinel/src/service.rs:723-730 | `            SentinelActionKind::Finalize { id } => Transaction {`<br>`                to: self.oracle,`<br>`                value: U256::ZERO,`<br>`                data: SentinelOracle::finalizeCall { requestId: id }`<br>`                    .abi_encode`<br>`                    .into,`<br>`                gas: 250_000,`<br>`            },` |
| 5 | Only the first `finalize` can succeed | E2 (Solidity reference, A7) | contracts/src/libraries/SentinelOracleRequests.sol:174-177 | `        require(prog.state == State.PENDING, RequestNotPending);`<br>`        bool everyoneRevealed = prog.committedCount > 0 && prog.revealedCount == prog.committedCount;`<br>`        bool nothingToReveal = prog.committedCount == 0 && block.number > self.terms.commitDeadline;`<br>`        require(block.number > self.terms.revealDeadline \|\| everyoneRevealed \|\| nothingToReveal, FinalizeTooEarly);` |

## Trigger

`K` sentinels participate in a request. All of them observe the last `Revealed` log in the same block (or all reach `reveal_deadline + 1` in the same block) and each emits a `Finalize` action (basis 1, 2, 3). All `K` transactions are broadcast within one or two blocks; the first mined one moves the request out of `PENDING` (basis 5) and the remaining `K - 1` revert. With the reference deployment's small sentinel set the waste is modest; it scales linearly with `K` and with the number of requests finalised per block.

## Considered and rejected

- **"Someone has to call `finalize`."** Yes — the finding is not that it is called, but that all `K` call it with no coordination and no way to notice the race was lost.
- **"`expires_at: None` means it is retried until it succeeds."** It means the queue never drops it before submission; once submitted and reverted the nonce is consumed and it is marked executed by nonce comparison (`crates/core/src/tx/storage.rs:222-232`), so it is not retried. That is correct behaviour here, but it also means the revert is invisible.
- **"A revert costs almost nothing."** It costs intrinsic gas plus the storage reads up to the failing `require`, a nonce, and — more importantly here — one of the sixteen in-flight slots (`crates/core/src/tx/mod.rs:204-216`) during exactly the window in which reveals for other requests need those slots.
- **"The claim has the same problem."** It does not: `claim` is genuinely per sentinel (`msg.sender`, `contracts/src/SentinelOracle.sol:286-304`), so every sentinel must submit its own.
- **False positive check — is there any deduplication or leader election?** No: `grep -n "Finalize" crates/sentinel/src/service.rs` shows the action is constructed only at `service.rs:637` and encoded at `723`, with no guard on who emits it.

## Remediation options

1. **Stagger by address.** Delay the `Finalize` action by `hash(request_id, self_address) mod K'` blocks (expiry-free actions already tolerate delay), so one sentinel usually goes first and the rest observe the resulting state change before their turn. Requires watching a terminal event to cancel the pending action — `OracleResult`, `DisputeTriggered` and `RequestTimedOut` are all emitted by `finalize` (`contracts/src/SentinelOracle.sol:268-283`) and none is currently watched.
2. **Only finalise when necessary.** Skip `Finalize` when the sentinel's own `Claim` would be blocked anyway, or emit it only from the reveal-deadline branch rather than the early-finalise one, halving the herd.
3. **Simulate before sending** (F-SEN-006 option 2 / F-SEN-007 option 3): an `eth_call` immediately before broadcast would drop a `Finalize` whose request is no longer `PENDING`, which fixes this and the duplicate-action waste in one place.
4. **Accept and measure.** Add a counter for submitted-versus-successful `Finalize` so the ratio is visible.

Tests to add: none required for the mechanism; an integration-script assertion counting reverted `finalize` transactions across the two sentinels would quantify it.

## Trail

- Reviewer R7: drafted from lead SEN-H13, self-estimate 95% on the mechanism, filed Informational because the cost is gas only and the behaviour is a reasonable default. All five basis citations re-opened in this checkout.

## Critic (C-SEN)

### Per-claim verdicts

All five rows re-opened; every quote is accurate and no claim is marked `H` (`service.rs:629-641`, `:372-375`, `:450-457`, `:723-730`, `SentinelOracleRequests.sol:174-177`). I confirmed independently that `Finalize` is constructed at exactly one site (`service.rs:636-640`) with `expires_at: None`, reached from both `handle_revealed`'s early trigger and `handle_block_advance`'s reveal-deadline branch, with no leader election, jitter or dedup anywhere in the crate.

### Severity: Informational is right. Certainty: 95% is inadmissible

Severity **Informational (unchanged)** — this is gas waste on a permissionless call that _someone_ must make, and the finding says so honestly. It is a hardening item under Section 8, not a defect.

The self-estimate is the problem. **95% claims the 90-100 band, which requires `E1` reproduction — unreachable in this read-only run (no toolchain; the critic brief §2 sets a hard 89% ceiling).** Nothing here was executed. I am setting **88%**, at the top of the `E2` band, which is what a fully-cited, uncontested mechanism deserves. This is not a criticism of the analysis, which is correct; it is a correction of the scale.

One factual nuance the report should carry: the two triggers do not fire equally. On the reveal-deadline path all `K` sentinels transition on the same block, so the herd is real and simultaneous. On the early-finalise path each sentinel fires on _its own_ tally reaching `committed_count`, and F-SEN-002 shows those tallies diverge between sentinels — so in practice that path produces a staggered, partially self-limiting herd rather than a synchronised one. The cost claim is therefore sound but its upper bound (`K - 1` reverts every time) is the worst case, not the expected case.

### Finding verdict

**Confirmed. Certainty 88% (corrected down from the 95% self-estimate). Severity Informational (unchanged).**

Recommend remediation option 3 (simulate before broadcast) be carried in the report as the shared fix for this, F-SEN-006 and F-SEN-007 — one `eth_call` immediately before submission drops every would-revert transaction in all three findings at a single site (`core/tx/mod.rs:258-265`).

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

**Note for every sentinel finding whose "tests to add" list names a `service.rs` unit test:** `sentinel` is a **binary-only crate** — `crates/sentinel/src/main.rs` declares `mod service;` and there is no `lib.rs`, so the crate has no library target and `crates/sentinel/tests/` cannot compile against it. Every such test must live inside the existing `#[cfg(test)] mod tests` in the source file. If the team wants these as permanent regression tests reachable from an integration target, **the crate needs a `lib.rs` first**; that is an unstated prerequisite across F-SEN-001, -002, -003, -011, -012 and -015.

### Remediation check

**Sound: option 3, which is the same change three other findings want.**

Option 3 (`eth_call` immediately before broadcast, drop on revert) is the right fix and is **F-SEN-006 option 2** and **F-SEN-007 option 3**. One core change to `submit_transaction` closes this finding, the replay-duplicate waste, and the unaffordable-commit waste. For an Informational finding that is not a reason to prioritise it here, but it is a strong reason to prioritise the change, and the report should aggregate the three rather than listing it three times.

Option 1 (stagger by `hash(request_id, self_address) mod K` blocks) is sound and its stated prerequisite is the interesting part: it needs a terminal event to cancel the pending action, and the finding correctly identifies that `OracleResult`, `DisputeTriggered` and `RequestTimedOut` are all emitted by `finalize` (`contracts/src/SentinelOracle.sol:268-283`) and **none is currently watched**. Adding `DisputeTriggered` is also **F-SEN-005 option 2**, so the event-set extension pays for itself twice. Without the cancellation the stagger only delays the herd, it does not thin it.

Option 2 (emit `Finalize` only from the reveal-deadline branch, not the early-finalise one) is sound and nearly free, and it composes with **F-SEN-002 option 2**, which proposes removing the early-finalise path entirely for correctness reasons. If F-SEN-002 option 2 is taken in its "wait for `reveal_deadline`" form, this finding's herd halves as a side effect.

Option 4 (a submitted-versus-successful `Finalize` counter) is the measurement that would tell an operator whether any of the above was worth doing.

## Post-merge revalidation (RV-SEN)

Re-validated against merge commit `a7f3915` (baseline `2893917`).

### Verdict: **STILL VALID** — same `Finalize` fan-out; one side effect removed

**Merged-code citations:**

| Cited at `2893917` | Now at | Changed? |
| --- | --- | --- |
| `service.rs:635-641` (unconditional `Finalize` emission) | `service.rs:639-645` | byte-identical text, moved |
| `service.rs:372-384` (early-finalise trigger in `handle_revealed`) | `service.rs:372-384` | byte-identical |
| `service.rs:450-465` (reveal-deadline branch) | `service.rs:450-465` | byte-identical |
| `service.rs:723-730` (250,000 gas for `Finalize`) | `service.rs:868-875` | moved only |

The set of sentinels that emit `Finalize` is unchanged: `finalize`'s guard is still `if !*self_revealed && !timed_out { return (None, Vec::new()); }` (`service.rs:633-637`), so every sentinel that revealed still submits one, on the same log or the same block. `SentinelOracle.finalize` still requires `prog.state == State.PENDING`, so `K - 1` still revert `RequestNotPending`, each burning a nonce, gas to the revert, and one of the sixteen in-flight slots.

**What changed:** the `K - 1` losers no longer delete their tracked entry. `finalize` now returns `Some(RequestState::WaitingForOutcome { … })` (`service.rs:647-653`), so a losing finalizer keeps the request and is recovered by the winner's `DisputeTriggered` / `RequestTimedOut` / `OracleResult` (`service.rs:669-706`, `:723-753`, `:775-817`). That removes the collateral state loss but not the cost this finding is about — and it introduces a parking hazard, since `WaitingForOutcome` is never expired by `handle_block_advance` (`service.rs:466`) and no losing finalizer ever retries. See **F-SEN-005**.

**Certainty 88% and severity Informational unchanged.** Status left at `Critiqued`.
