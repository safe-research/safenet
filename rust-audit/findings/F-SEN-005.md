# F-SEN-005 `WaitingForDisputeResolution` never expires and the sentinel never calls the permissionless `timeoutArbitration`, so an inactive arbitrator locks the bond and grows the snapshot forever

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | sentinel, service.rs / bindings.rs |
| Location | crates/sentinel/src/service.rs:466, 646-654 (related: crates/sentinel/src/bindings.rs:47-57, crates/sentinel/src/action.rs:9-25) |
| Severity | Medium / Medium |
| Certainty | 86% (set by Critic C-SEN; QA may raise) |
| Assumptions involved | A1, A10 |
| Tags | funds, dos |

## Claim

`finalize` parks a disputed request in `WaitingForDisputeResolution` (`service.rs:646-654`) and `handle_block_advance`'s arm for that state unconditionally returns `true` (`service.rs:466`), so the entry has no deadline and is only ever removed by an incoming `DisputeResolved`, `ArbitrationTimedOut` or `DisputeOutOfScope` event. All three of those are produced only by someone else calling `SentinelOracle.resolveDispute`, `timeoutArbitration` or `markOutOfScope`.

`timeoutArbitration` is deliberately permissionless precisely so a frozen request need not wait on the arbitrator forever (`contracts/src/SentinelOracle.sol:322-331`), and the bonded sentinel is the party with the incentive to call it — yet the sentinel has **no binding for it** (`bindings.rs:47-57` declares only `commit`, `reveal`, `hashCommitment`, `finalize`, `claim`) and **no action kind** for it (`action.rs:9-25`). If nobody else calls it, the bond stays locked in the oracle indefinitely and the request stays in `State` forever, so every per-block snapshot (`core/state/mod.rs:236`) carries it in perpetuity.

PR #883 introduced the two timeout handlers and explicitly deferred "adding a deadline to the waiting state" to a later change, so this is a known gap that is nonetheless unaddressed in the code at this commit.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The dispute-waiting state is retained on every block with no deadline check | E2 | crates/sentinel/src/service.rs:450-467 | `            RequestState::CollectingVotes {`<br>`                reveal_deadline, ..`<br>`            } => {`<br>`                if block <= *reveal_deadline {`<br>`                    return true;`<br>`                }`<br>`                let (update, finalization) = self.finalize(entry, *id);`<br>`                actions.extend(finalization);`<br>`                match update {`<br>`                    None => false,`<br>`                    Some(new_state) => {`<br>`                        *entry = new_state;`<br>`                        true`<br>`                    }`<br>`                }`<br>`            }`<br>`            RequestState::WaitingForDisputeResolution { .. } => true,`<br>`        });` |
| 2 | Entry into the state, with no timer of any kind | E2 | crates/sentinel/src/service.rs:643-654 | `        // In case of a dispute it is not a timeout, so this sentinel participated.`<br>`        // Finalize the request and wait for a dispute resolution by the arbitrator;`<br>`        // \`handle_resolved\` records the win/loss metric once that arrives.`<br>` if dispute {`<br>` return (`<br>` Some(RequestState::WaitingForDisputeResolution {`<br>` approve,`<br>` slash_amount,`<br>` }),`<br>` actions,`<br>` );`<br>` }` |
| 3 | No `timeoutArbitration` binding exists | E2 | crates/sentinel/src/bindings.rs:47-57 | `            function commit(bytes32 requestId, bytes32 commitHash) external;`<br>`            function reveal(bytes32 requestId, bool approve, bytes32 salt, string calldata reason) external;`<br>`            function hashCommitment(`<br>`                address sentinel,`<br>`                bytes32 requestId,`<br>`                bool approve,`<br>`                bytes32 salt,`<br>`                string calldata reason`<br>`            ) external pure returns (bytes32);`<br>`            function finalize(bytes32 requestId) external;`<br>`            function claim(bytes32 requestId) external;` |
| 4 | No action kind can encode such a call | E2 | crates/sentinel/src/action.rs:9-25 | `pub enum SentinelActionKind {`<br>`    /// Approve the fee token to be spent by the oracle (bond pre-authorisation).`<br>`    ApproveToken { bond: U256 },`<br>`    /// Lock a bond behind a blind commitment hash for the request with the given id.`<br>`    Commit { id: B256, hash: B256 },`<br>`    /// Reveal a previously committed vote and reasoning for the request with the given id.`<br>`    Reveal {`<br>`        id: B256,`<br>`        approve: bool,`<br>`        salt: B256,`<br>`        reason: String,`<br>`    },`<br>`    /// Finalise the committed vote for the request with the given id.`<br>`    Finalize { id: B256 },`<br>`    /// Claim the bond and reward for the request with the given id.`<br>`    Claim { id: B256 },`<br>`}` |
| 5 | `timeoutArbitration` is permissionless and exists for exactly this situation | E2 (Solidity reference, A7) | contracts/src/SentinelOracle.sol:322-331 | `    // Permissionless: a \`FROZEN\` request that outlives \`ARBITRATION_TIMEOUT\` should not wait on the`<br>` // arbitrator forever. Reuses the \`TIMED_OUT\` machinery, so every committed bond returns in`<br>` // full via \`claim\` -- identical to the no-reveal timeout path.`<br>` function timeoutArbitration(bytes32 requestId) external {`<br>` SentinelOracleRequest.T storage request = $requests.get(requestId);`<br>` address sponsor = request.terms.sponsor;`<br>` uint96 refundFee = request.timeoutArbitration;`<br>` FEE_TOKEN.safeTransfer(sponsor, refundFee);`<br>` emit ArbitrationTimedOut(requestId);`<br>` }` |
| 6 | The whole `State` map is serialised into a snapshot on every processed log batch, so a stuck entry is copied for every block | E2 | crates/core/src/state/mod.rs:236 | `                self.snapshots.commit(blocks.last, &state).await?;` |

## Trigger

1. A request is disputed: both sides revealed, `finalize` moves it to `WaitingForDisputeResolution` and emits `Finalize` (`service.rs:646-654`). Onchain the request becomes `FROZEN` with `arbitrationDeadline = block.number + ARBITRATION_TIMEOUT` (`contracts/src/libraries/SentinelOracleRequests.sol:183-185`).
2. The arbitrator does not rule — it is offline, the dispute is outside its remit and it never calls `markOutOfScope`, or governance has not yet appointed one.
3. `block.number` passes `arbitrationDeadline`. Anyone may now call `timeoutArbitration` and every bond would return in full, but the sentinel cannot: it has neither the binding nor the action.
4. No third party calls it. The sentinel's `bondTarget` stays in the oracle, and the request stays in `State` for the lifetime of the process, appearing in every snapshot written thereafter.

No attacker is required, but note that under A2 a sponsor can create disputes cheaply (propose a transaction that some engines rate `secure` and others `insecure`), so the number of parked entries is attacker-influenced.

## Considered and rejected

- **"`handle_arbitration_timeout` already covers this."** It only _reacts_ to the `ArbitrationTimedOut` event (`service.rs:539-576`, dispatched at `service.rs:774-776`); it never causes it. If nobody calls `timeoutArbitration`, the event is never emitted.
- **"The entry is small, so unbounded growth does not matter."** Each entry is two fields (`state.rs:76`), so the memory cost is trivial; the cost that matters is the locked `bondTarget` and the fact that the JSON snapshot written per block grows monotonically with the number of unresolved disputes (basis 6).
- **"The operator can claim by hand."** They can — but `claim` still requires the request to be resolved (`contracts/src/libraries/SentinelOracleRequests.sol:239-245`), so a manual `claim` does not help while the request is `FROZEN`; the operator would have to call `timeoutArbitration` themselves, which is exactly the missing automation.
- **"`ARBITRATION_TIMEOUT` is short, so this self-heals."** The timeout only makes the request _eligible_ for `timeoutArbitration`; it does not change the state by itself. `outOfScope`/`timeoutArbitration` are the only transitions out of `FROZEN` besides `resolveDispute` (`contracts/src/libraries/SentinelOracleRequests.sol:223-237`).
- **False positive check — is there any other expiry path?** `grep -n "WaitingForDisputeResolution" crates/sentinel/src` shows the state is constructed only at `service.rs:648` and matched at `service.rs:466`, `489`, `545` and `state.rs:76, 87`; none of those carries a block number.

## Remediation options

1. **Add the missing call.** Bind `function timeoutArbitration(bytes32 requestId) external;`, add `SentinelActionKind::TimeoutArbitration { id }` with the same 250,000 gas budget as the other oracle calls, extend `WaitingForDisputeResolution` with an `arbitration_deadline` field (derivable as `finalize_block + ARBITRATION_TIMEOUT`, which requires reading `ARBITRATION_TIMEOUT` once at startup, or from the `DisputeTriggered` event if it were watched), and emit the action once `block > arbitration_deadline`. Tradeoff: every bonded sentinel would race to submit it and all but one revert (same shape as F-SEN-013).
2. **Watch `DisputeTriggered` and derive the deadline from it.** The oracle already emits `DisputeTriggered(requestId)` at `contracts/src/SentinelOracle.sol:268-271`, which the sentinel does not consume. Adding it to the watched event set (`bindings.rs:24-46`) gives an exact freeze block without an extra RPC read.
3. **Bound the state without acting.** Give `WaitingForDisputeResolution` a generous deadline (e.g. `finalize_block + ARBITRATION_TIMEOUT + margin`) after which the entry is dropped, and record a dedicated metric/`warn` so the operator knows a bond needs manual recovery. Cheapest change; recovers snapshot growth but not the funds.
4. **Operational:** export a gauge for the number of entries parked in `WaitingForDisputeResolution` and their age, so an inactive arbitrator is visible before it becomes a balance problem.

Tests to add: a `service.rs` test asserting that a `WaitingForDisputeResolution` entry does not survive an unbounded number of `Message::NewBlock`s. An integration-script scenario that freezes a request and lets `ARBITRATION_TIMEOUT` elapse without the arbitrator acting, asserting the sentinel eventually recovers its bond.

## Trail

- Reviewer R7: drafted from lead SEN-H6, self-estimate 90%. All six basis citations re-opened in this checkout; the absence of a `timeoutArbitration` binding and of any deadline on the state were verified by grep over the whole crate.

## Critic (C-SEN)

I checked the contract first, as instructed: a missing binding to a function the contract already offers is a much stronger finding than a missing local timer, and that is exactly what this is.

### Per-claim verdicts

| # | Verdict | Note |
| --- | --- | --- |
| 1 | **Supported** | `service.rs:450-467` verbatim. The `WaitingForDisputeResolution { .. } => true` arm is unconditional — no block comparison, and the variant carries no block number to compare against (`state.rs:76`). |
| 2 | **Supported** | `service.rs:643-654` verbatim; the state is constructed with `approve` and `slash_amount` only. |
| 3 | **Supported** | `bindings.rs:47-57` verbatim. I re-read the whole `sol!` block: the declared functions are `commit`, `reveal`, `hashCommitment`, `finalize`, `claim` — no `timeoutArbitration`, no `markOutOfScope`. |
| 4 | **Supported** | `action.rs:9-25` verbatim; five variants, none of which can encode that call. `SentinelEncoder::encode_action_kind` (`service.rs:676-740`) is exhaustive over them. |
| 5 | **Supported** | `SentinelOracle.sol:322-331` verbatim. `timeoutArbitration` has no modifier and no `msg.sender` check — it is genuinely permissionless, and its own comment says it exists precisely so a `FROZEN` request "should not wait on the arbitrator forever". |
| 6 | **Supported** | `state/mod.rs:236` verbatim; the whole `State` map is re-serialised to JSON per processed log range (`state/storage.rs:105-116`), so a stuck entry is rewritten indefinitely. |

The PR-#883 claim in the Claim section is also **supported**, which I checked because it is the one assertion not backed by a code citation: commit `3b7e0cc` ("Claim on Arbitration timeout (#883)") contains verbatim "Adding a deadline to the waiting state is handled in a separate PR" and "Currently there is no deadline on the waiting for dispute resolution state". So this is a `known` item under A12 and should be tagged as such.

### What makes this worse than the finding says

Every sentinel in the set runs this same binary. `timeoutArbitration` is permissionless but nobody in the protocol's own software can call it, so if the arbitrator simply never rules, the escape hatch the contract deliberately provides is unreachable by the entire fleet: every bonded sentinel's `bondTarget` stays in the oracle, and every sentinel's snapshot carries the dead entry forever. Recovery requires a human sending a raw transaction. The sponsor has an incentive (the fee refund at `SentinelOracle.sol:329`) and `markOutOfScope` gives the arbitrator a second exit, which is what keeps this off High.

### Finding verdict

**Confirmed. Certainty 86%. Severity Medium (unchanged).**

This is the highest-certainty finding in the set: every claim is an _absence_ verified by reading the complete enumerations (`bindings.rs` `sol!` block, `SentinelActionKind`, the `retain` match), not by grep, and the contract side is quoted directly. The residual uncertainty is only whether an arbitrator would in practice leave a request unruled past `ARBITRATION_TIMEOUT`. Medium per Section 8: funds locked with recoverable impact, plus unbounded state growth.

Remediation option 1 should be the recommendation: add `timeoutArbitration(bytes32)` to `bindings.rs`, a `TimeoutArbitration { id }` action kind, and an `arbitration_deadline` on `WaitingForDisputeResolution`. The deadline is available — `finalize` sets `arbitrationDeadline = block.number + arbitrationTimeout` (`SentinelOracleRequests.sol:185`) — but it is not in any watched event, so it must either be read via a new effect or approximated from the finalising block; that tradeoff belongs in the finding.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

**Note for every sentinel finding whose "tests to add" list names a `service.rs` unit test:** `sentinel` is a **binary-only crate** — `crates/sentinel/src/main.rs` declares `mod service;` and there is no `lib.rs`, so the crate has no library target and `crates/sentinel/tests/` cannot compile against it. Every such test must live inside the existing `#[cfg(test)] mod tests` in the source file. If the team wants these as permanent regression tests reachable from an integration target, **the crate needs a `lib.rs` first**; that is an unstated prerequisite across F-SEN-001, -002, -003, -011, -012 and -015.

### Remediation check

**Sound: option 2 plus option 1; option 3 recovers the wrong thing.**

Option 2 (watch `DisputeTriggered`, already emitted at `contracts/src/SentinelOracle.sol:268-271` and currently unconsumed) is sound and is the prerequisite for option 1 done properly: it gives an exact freeze block with no extra RPC, so the arbitration deadline is derived from chain evidence rather than guessed. Take it first — it is one event added to `bindings.rs`'s watched set.

Option 1 (bind `timeoutArbitration`, add the action kind, emit once past the deadline) is sound and is the only option that recovers the **funds**. Its stated tradeoff (every bonded sentinel races and all but one reverts) is real and is exactly **F-SEN-014**'s herd; the same mitigation applies — stagger by `hash(request_id, self_address) mod K` blocks, or simulate before broadcasting (F-SEN-006 option 2). Since `timeoutArbitration` is permissionless, staggering is nearly free here: whoever goes first releases everyone.

Option 3 (drop the entry after a deadline, with a metric and a `warn`) recovers **snapshot growth but not the bond**, as the finding says. That is worth being blunt about in the report: it is a memory-leak fix presented alongside a funds fix, and a reader skimming the options could take the cheap one and believe the finding is closed. It is not — the bond stays locked and now nothing is tracking it.

Option 4 (a gauge for entries parked in `WaitingForDisputeResolution` and their age) is cheap and is the only thing here that makes an inactive arbitrator visible before it becomes a balance problem.

**Contract note:** none of these touch `apply_transition`'s purity. Adding `arbitration_deadline` to `WaitingForDisputeResolution` is a state-shape change, which serialises into the snapshot — see **F-CORE-037** on the absence of any snapshot versioning, since this is exactly the kind of field addition that a downgrade would silently discard.

## Post-merge revalidation (RV-SEN)

Re-validated against merge commit `a7f3915` (baseline `2893917`). This finding was the prime fix candidate this run — `6faa0b4`/`84facb2` add a waiting-for-outcome state, `874064c` adds an arbitration deadline to `DisputeTriggered`, `898a5c5` adds a handle-request-timed-out method. The code says otherwise.

### Verdict: **STILL VALID, and widened**

**Merged-code citations:**

| Cited at `2893917` | Now at | Changed? |
| --- | --- | --- |
| `service.rs:466` (`WaitingForDisputeResolution => true`, no deadline) | `service.rs:467` | +1, otherwise identical |
| `service.rs:646-654` (`finalize` parks the dispute) | moved to `handle_dispute_triggered`, `service.rs:742-750` | the state is now entered from the oracle's event, still with no deadline |
| `bindings.rs:47-57` (no `timeoutArbitration` binding) | `bindings.rs:50-60` | still only `commit`, `reveal`, `hashCommitment`, `finalize`, `claim` |
| `action.rs:9-25` (no action kind) | `action.rs:9-25` | `action.rs` is untouched by the merge; still `ApproveToken`, `Commit`, `Reveal`, `Finalize`, `Claim` |
| `contracts/src/SentinelOracle.sol:322-331` (`timeoutArbitration` permissionless) | `contracts/src/SentinelOracle.sol:327-333` | unchanged, still permissionless |

Neither half of the finding is closed: there is still no binding, no action kind, and no deadline on `WaitingForDisputeResolution`.

### The merge adds a _second_ never-expiring state

`handle_block_advance` now has two unconditional `=> true` arms (`service.rs:466-467`):

```rust
RequestState::WaitingForOutcome { .. } => true,
RequestState::WaitingForDisputeResolution { .. } => true,
```

and `WaitingForOutcome`'s own doc comment states the property as a design choice (`crates/sentinel/src/state.rs:71-80`):

> A `Finalize` action was submitted and is awaiting onchain confirmation. **No deadline: unlike every other state, advancement out of here is tied to our own `finalize()` call landing, not a block window, so `handle_block_advance` never expires it.**

That is load-bearing because the `finalize()` whose landing it waits on is not guaranteed to land. `finalize` is emitted by every participating sentinel (F-SEN-014), so all but one revert `RequestNotPending`; it is also emitted prematurely whenever the local tally is undercounted (F-SEN-002's mirror case) and reverts `FinalizeTooEarly`. In each of those cases the entry parks in `WaitingForOutcome` with nothing to retry it. Verified by execution on the merged tree — a `WaitingForOutcome` entry driven to `NewBlock(10_000)`, thousands of blocks past its `revealDeadline: 40`:

```
RV002 block10000 commands = []   entry = Some(WaitingForOutcome { approve: true, slash_amount: 500 })
```

### The arbitration deadline is now delivered and discarded

`874064c` puts the deadline on the wire. Contract (`contracts/src/SentinelOracle.sol:31-34, 261, 271`):

```solidity
// … `deadline` is the block number by which the
// arbitrator must rule before `timeoutArbitration` becomes callable.
event DisputeTriggered(bytes32 indexed requestId, uint64 deadline);
…
uint64 arbitrationDeadline = (block.number + ARBITRATION_TIMEOUT).toUint64();
…
emit DisputeTriggered(requestId, arbitrationDeadline);
```

The sentinel decodes it — `event DisputeTriggered(bytes32 indexed requestId, uint64 deadline);` (`crates/sentinel/src/bindings.rs:42`) — and then throws it away. `handle_dispute_triggered` (`service.rs:723-753`) reads only `event.requestId`:

```rust
if let Some((approve, slash_amount)) = entry.approve_and_slash_amount() {
    state.0.insert(
        event.requestId,
        RequestState::WaitingForDisputeResolution { approve, slash_amount },
    );
}
```

`event.deadline` is never referenced anywhere in `crates/sentinel` (a grep for `.deadline` in `service.rs`, excluding `commit_deadline`/`reveal_deadline`, returns nothing). So the exact value this finding's remediation option asked for is now available at zero cost and is not stored, and `WaitingForDisputeResolution` still carries only `{ approve, slash_amount }` (`crates/sentinel/src/state.rs:86`).

### Certainty and severity

**Certainty: 86% → 95%** — the mechanism is now execution-verified rather than read, and the "deferred to a later change" premise is settled: the later change landed and did not do it. **Severity unchanged (Medium / Medium)**, but the scope widens from one never-expiring state to two, and the second one is now reachable on every reverted `finalize()` — i.e. on `K-1` of every `K` participating sentinels per request. Status left at `Critiqued`.

## In-flight impact (AS-SEN)

**Pertains to unmerged branches, not to `main`.** Assessed against `origin/fix/sentinel_deadlines` (PR #914) and `origin/feat/optimistic_block_transition` (PR #915, tip `b2aad06`), built from a `git archive` extraction. Both apply cleanly onto `main` — `main`'s crates are byte-identical to their base `199629e`, so a verdict on the tip is also the verdict for the tip merged into `main`. **Effect: unchanged.** Both waiting states still never expire (`crates/sentinel/src/service.rs:478-479`: `WaitingForOutcome { .. } => true`, `WaitingForDisputeResolution { .. } => true`); `handle_dispute_triggered` (`:735-764`) reads only `event.requestId`, and no code in `crates/sentinel/src` reads `.deadline`. There is still no `timeoutArbitration` binding or action. #914 touches only the commit and engine-check comparisons (`:402-429`), not the dispute path.
