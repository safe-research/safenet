# F-SEN-004 The sentinel bonds on every proposal with no cap on concurrent engine checks, outstanding bonds or reveal throughput, so a proposal flood forces abstention and pushes reveals past their deadline

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | sentinel, service.rs / effect.rs (with core: effects.rs, tx/mod.rs, tx/storage.rs) |
| Location | crates/sentinel/src/service.rs:137-144, 226-242 (related: crates/core/src/effects.rs:54-62, crates/core/src/tx/mod.rs:202-219, crates/core/src/tx/storage.rs:144-168) |
| Severity | Medium / Medium |
| Certainty | 62% (set by Critic C-SEN; QA may raise) |
| Assumptions involved | A2, A3, A10 |
| Tags | dos, funds |

## Claim

Every `TransactionProposed` for the configured oracle spawns one unbounded-concurrency HTTP effect (`service.rs:137-144` → `core/effects.rs:54-62`), and every engine verdict that is not `Unknown` bonds unconditionally — there is no balance check, no cap on the number of simultaneously bonded requests and no back-pressure (`service.rs:226-242`). Three consequences follow from a burst of proposals in one or a few blocks, all reachable by any sponsor willing to pay the request fee (A2: proposal contents and volume are attacker-controlled):

1. **Abstention (liveness).** `N` proposals produce `N` simultaneous `POST /v1/security-check` calls to a single co-deployed engine that itself has no concurrency limit or server-side deadline. Once the engine saturates, the per-request `reqwest` timeout fires and every check resolves to `CheckOutcome::Unknown`, which drops the request unanswered (`service.rs:176-179`). No sentinel votes, the request times out onchain and the sponsor's fee is refunded (`contracts/src/SentinelOracle.sol:273-277`), so the attacker's marginal cost is only proposal gas.
2. **Non-reveal slashing (funds).** For the requests that _were_ bonded, every `Reveal` must be _submitted_ before `reveal_deadline`, but the queue only submits while fewer than `max_in_flight_transactions` (default 16) are in flight (`core/tx/mod.rs:202-219`) and picks queued rows strictly in insertion order, skipping expired ones (`core/tx/storage.rs:144-168`). A reveal that is still queued when `reveal_deadline` passes is silently dropped, leaving a `PENDING` commitment that is slashed `slashAmount` as soon as any sentinel reveals (`contracts/src/libraries/SentinelOracleRequests.sol:289-296`).
3. **Unbounded bond exposure.** Nothing bounds the sum of outstanding `bondTarget`s. A sustained flood locks the sentinel's whole fee-token balance for `COMMIT_WINDOW + REVEAL_WINDOW` blocks per request, after which further `commit`s revert for lack of balance (see F-SEN-007).

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | One engine effect per proposal, unconditionally | E2 | crates/sentinel/src/service.rs:137-144 | `        (`<br>`            state,`<br>`            vec![Command::Effect(effect::Effect::EngineCheck {`<br>`                request_id,`<br>`                transaction: event.transaction,`<br>`                block,`<br>`            })],`<br>`        )` |
| 2 | Effects are spawned into an unbounded `JoinSet` with no concurrency limit | E2 | crates/core/src/effects.rs:54-62 | `    pub fn spawn(&mut self, effect: Effect) {`<br>`        tracing::trace!(?effect, "spawning effect task");`<br>`        let handler = Arc::clone(&self.handler);`<br>`        self.tasks.spawn(async move {`<br>`            let resume = handler.perform_effect(effect).await;`<br>`            tracing::trace!(?resume, "effect task finished");`<br>`            resume`<br>`        });`<br>`    }` |
| 3 | The bond is posted unconditionally once a verdict exists; no cap, balance check or rate limit | E2 | crates/sentinel/src/service.rs:226-242 | `        let actions = vec![`<br>`            SentinelAction {`<br>`                kind: SentinelActionKind::ApproveToken {`<br>`                    bond: U256::from(bond_target),`<br>`                },`<br>`                expires_at: Some(commit_deadline),`<br>`            }`<br>`            .into,`<br>`            SentinelAction {`<br>`                kind: SentinelActionKind::Commit {`<br>`                    id: request_id,`<br>`                    hash,`<br>`                },`<br>`                expires_at: Some(commit_deadline),`<br>`            }`<br>`            .into,`<br>`        ];` |
| 4 | Submission is capped at `max_in_flight_transactions` (default 16) and is strictly FIFO by insertion id | E2 | crates/core/src/tx/mod.rs:204-216 | `        let in_flight = self.storage.count_in_flight.await?;`<br>`        for _ in in_flight..self.config.max_in_flight_transactions {`<br>`            let nonce = self.nonce.await?;`<br>`            let Some(transaction) = self`<br>`                .storage`<br>`                .next_transaction(Status { nonce, block })`<br>`                .await?`<br>`            else {`<br>`                break;`<br>`            };`<br>`            self.submit_transaction(transaction, block).await?;`<br>`        }` |
| 5 | An expired queued transaction is skipped and never submitted | E2 | crates/core/src/tx/storage.rs:150-155 | `             WHERE id = (`<br>`                 SELECT id FROM transactions`<br>`                 WHERE nonce IS NULL AND (expires_at IS NULL OR expires_at > ?)`<br>`                 ORDER BY id ASC`<br>`                 LIMIT 1`<br>`             )` |
| 6 | The reveal carries the reveal deadline as its expiry, so it is dropped rather than submitted late | E2 | crates/sentinel/src/service.rs:426-437 | `                actions.push(`<br>`                    SentinelAction {`<br>`                        kind: SentinelActionKind::Reveal {`<br>`                            id: *id,`<br>`                            approve,`<br>`                            salt,`<br>`                            reason,`<br>`                        },`<br>`                        expires_at: Some(reveal_deadline),`<br>`                    }`<br>`                    .into,`<br>`                );` |
| 7 | An engine failure or timeout drops the request unanswered — one abstention per saturated check | E2 | crates/sentinel/src/service.rs:173-180 | `        let (approve, reason) = match outcome {`<br>`            CheckOutcome::Approved => (true, String::new),`<br>`            CheckOutcome::Denied(rule) => (false, rule.to_string),`<br>`            CheckOutcome::Unknown => {`<br>`                tracing::warn!(%request_id, "engine check failed; dropping request unanswered");`<br>`                return (state, Vec::new);`<br>`            }`<br>`        };` |
| 8 | A total timeout refunds the sponsor's fee, so an abstention flood is nearly free for the attacker | E2 (Solidity reference, A7) | contracts/src/SentinelOracle.sol:273-277 | `        if (newState == SentinelOracleRequest.State.TIMED_OUT) {`<br>`            FEE_TOKEN.safeTransfer(sponsor, refundFee);`<br>`            emit RequestTimedOut(requestId);`<br>`            return;`<br>`        }` |

## Trigger

Using the integration script's parameters (`COMMIT_WINDOW = 5`, `REVEAL_WINDOW = 5`, `scripts/run_sentinel_integration_test.sh`) and the shipped queue defaults (`max_in_flight_transactions = 16`, `blocks_before_resubmit = 2`, `core/tx/mod.rs:85-93`):

- **Abstention:** a sponsor calls `Consensus.proposeTransaction` `N` times in one block, each carrying a `data` field large enough to make the engine's decoding and RPC-backed checkers expensive. All `N` engine checks are issued at once (basis 1, 2). Because the sentinel's per-request budget is a fixed three quarters of the voting window and the engine has no queueing discipline, past some `N` every check exceeds the budget and returns `Unknown` (basis 7). No commit is made for any of the `N` requests; each times out and refunds the sponsor (basis 8).
- **Non-reveal slash:** the same sponsor sizes `N` so that checks still succeed but the reveals cannot all be submitted. Each bonded request costs one `approve` + one `commit` before its commit deadline and one `reveal` after it; with a 5-block reveal window and 16 in-flight slots, at most `16 × 5 = 80` transactions can be submitted in the window even in the best case, and reveals compete with the `approve`/`commit` pairs of newer requests queued behind them in the same FIFO. Any reveal still queued at `reveal_deadline` is skipped (basis 5, 6); its commitment is slashed `slashAmount` as soon as one sentinel reveals.

Both variants need only a funded sponsor account and the per-request fee, which is refunded in the abstention case.

## Considered and rejected

- **"A3 makes engine overload out of scope."** A3 says the engine's API is only reachable by its co-deployed sentinel, which is exactly the point: the load is generated by the sentinel itself from chain input, not by an external caller. A3 explicitly leaves "the engine timed out or returned a 500" in scope.
- **"The per-request timeout bounds the damage."** It bounds each individual call (`effect.rs:62-68` always sets it), which is what converts saturation into abstention rather than a stall — but abstention is the liveness failure being claimed, and the timeout does nothing about bond exposure or reveal throughput.
- **"The engine would simply be slow, not wrong."** Slow _is_ wrong here: `CheckOutcome::Unknown` and a genuine `abstain` verdict are the same value (`engine.rs:170-188`), so a saturated engine is indistinguishable from a policy abstention in both the FSM and the `verdict` metric label (`metrics.rs:20-23` does separate `error` from `abstain`, which is the one useful signal).
- **"`voting_window` bounds the number of tracked requests."** It bounds how long a _pre-commit_ entry lives (`service.rs:393-400`), not how many exist, and not how many bonds are outstanding.
- **"The reveal is resubmitted until mined, so it cannot be lost."** Resubmission only applies once a transaction has been _allocated a nonce and submitted_ (`core/tx/mod.rs:224-237`). A reveal that never reached that stage before its expiry block is filtered out by the selection query (basis 5) and simply never runs.
- **"The FSM would notice and claim the remainder."** It would not: the entry is dropped at `service.rs:415-417` only when `self_committed` is false; when the commit _did_ land but the reveal did not, the entry sits in `CollectingVotes` and finalises at `reveal_deadline + 1` with `self_revealed == false` and `revealed_count > 0`, which is the silent-drop branch at `service.rs:631-633` (see F-SEN-002).
- **False positive check — is there really no throttle?** `grep -n "semaphore\|Semaphore\|limit\|throttle" crates/sentinel/src crates/core/src/effects.rs` returns nothing relevant; `EffectManager` has no bound (basis 2) and `SentinelTransition` has no counter of outstanding bonds (`state.rs:92-94` is a plain `HashMap`).

## Remediation options

1. **Bound concurrent effects.** Add an optional `max_concurrent_effects` to `core::effects::EffectManager` (a `tokio::sync::Semaphore` acquired inside the spawned task), so excess checks queue instead of stampeding the engine. Tradeoff: a queued check may exceed its commit window and become an abstention anyway — but a _predictable_ one, and the engine keeps answering.
2. **Give the engine call a deadline derived from the request, not the config.** Compute the per-check timeout from the request's own `commit_deadline` minus the current block (the `TODO` at `main.rs:45-49` already anticipates this), so a check that cannot possibly finish in time is abandoned early and does not hold a connection.
3. **Cap outstanding bonds.** Track the sum of outstanding `bondTarget` in `State` and decline to commit (drop the entry, increment a dedicated metric) above an operator-configured ceiling, ideally expressed as a fraction of the signer's fee-token balance. Tradeoff: deliberate non-participation, which must be visible in metrics so it is not mistaken for the engine failing.
4. **Prioritise reveals over new commitments.** Give the transaction queue a priority column, or have the sentinel emit reveals with a lower `id` ordering, so a reveal never queues behind the `approve`/`commit` pair of a newer request. This is the cheapest mitigation for the slashing half of the finding.
5. **Operational:** document a recommended `max_in_flight_transactions` relative to `REVEAL_WINDOW`, and alert on `safenet_sentinel_engine_check_verdicts_total{verdict="error"}` and on the gap between `requests_participated_total` and `requests_resolved_total`.

Tests to add: a queue test that enqueues more reveals than `max_in_flight_transactions × REVEAL_WINDOW` and asserts which are dropped. An integration-script scenario that proposes `N` transactions in one block and asserts the abstention rate and the number of slashed bonds.

## Trail

- Reviewer R7: drafted from lead SEN-H4, self-estimate 60%. All eight basis citations re-opened in this checkout. The code facts (no concurrency bound, no bond cap, FIFO submission, expiry drop) are E2; the value of `N` at which a real engine saturates and the exact reveal throughput are not measured, so the economic conclusion is inference. Severity kept at Medium rather than the map's High for that reason — QA measuring the saturation point on the Anvil script would justify raising it.

## Critic (C-SEN)

### Per-claim verdicts

| # | Verdict | Note |
| --- | --- | --- |
| 1 | **Supported** | `service.rs:137-144` verbatim; one effect per accepted proposal, unconditionally. |
| 2 | **Supported** | `core/effects.rs:54-62` verbatim; `JoinSet::spawn` with no permit, semaphore or queue. |
| 3 | **Supported** | `service.rs:226-242` verbatim; no balance, allowance, count or rate guard anywhere on the path. |
| 4 | **Supported** | `core/tx/mod.rs:204-216` verbatim; the loop is bounded by `max_in_flight_transactions`. |
| 5 | **Supported** | `core/tx/storage.rs:150-155` verbatim; `expires_at IS NULL OR expires_at > ?` with `ORDER BY id ASC`, so expired rows are skipped, never submitted, and selection is strictly FIFO. |
| 6 | **Supported** | `service.rs:426-437` verbatim; the reveal carries `expires_at: Some(reveal_deadline)`. |
| 7 | **Supported** | `service.rs:173-180` verbatim. |
| 8 | **Supported** | `SentinelOracle.sol:273-277` verbatim; the `TIMED_OUT` branch refunds the sponsor. |

### Assessment

The **code facts are all confirmed**: there is genuinely no bound on concurrent engine checks, no cap on simultaneously bonded requests, no balance gate, and the reveal path competes FIFO for 16 in-flight slots against the `approve`/`commit` pairs of newer requests. The mechanism is `E2`.

What is _not_ established, and cannot be from this checkout, is the **quantitative claim that makes it an attack**: the `N` at which the engine saturates, the engine's latency profile, the deployed `COMMIT_WINDOW`/`REVEAL_WINDOW`, and the per-proposal fee an attacker must front. R7 says so explicitly in its own log ("economics unproven"), and the trigger's arithmetic ("`16 × 5 = 80` transactions") is an illustration, not a measurement. Both variants therefore have a verified mechanism and an unproven trigger.

I checked one hedge R7 did not: the abstention variant's cost claim depends on the fee being refunded, which requires the request to reach `TIMED_OUT` — and it does, because with zero reveals `finalize` takes the `else` branch at `SentinelOracleRequests.sol:206-212` and sets `refundFee = prog.fee`. Basis 8 holds. But note this also means the attacker must still pay gas for `proposeTransaction` (which runs a FROST `sign` and an ERC-20 transfer), so "nearly free" is an inference, not a measured cost.

### Overlap

Consequence 1 (unbounded concurrent effects) is the sentinel instance of **F-CORE-033**, which is canonical for that mechanism and already names this crate. Consequences 2 and 3 (bond exposure and reveal-throughput starvation) are sentinel-specific and belong here. Consequence 2 shares its economic outcome with F-SEN-001 but by a different route — a reveal dropped for expiry rather than never emitted — so it is not a duplicate.

### Finding verdict

**Plausible. Certainty 62%. Severity Medium (unchanged).**

`E2` mechanism, unproven trigger, so the 40-69 band. Medium is correct and appropriately un-inflated: High under Section 8 would require the unbounded drain to be demonstrated rather than argued. If QA measures a saturation point that a single funded sponsor can reach, this becomes High.

### Notes for QA

The cheapest measurement is not an attack: instrument `EffectManager` task count during a warp over a busy range (`block_page_size` = 100 blocks in one batch, `state/mod.rs:213-223`) and record the peak. That number alone settles consequence 1 and bounds consequence 2.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

**Note for every sentinel finding whose "tests to add" list names a `service.rs` unit test:** `sentinel` is a **binary-only crate** — `crates/sentinel/src/main.rs` declares `mod service;` and there is no `lib.rs`, so the crate has no library target and `crates/sentinel/tests/` cannot compile against it. Every such test must live inside the existing `#[cfg(test)] mod tests` in the source file. If the team wants these as permanent regression tests reachable from an integration target, **the crate needs a `lib.rs` first**; that is an unstated prerequisite across F-SEN-001, -002, -003, -011, -012 and -015.

### Remediation check

**Sound: option 4 first (cheapest, biggest effect on the loss), then options 1 and 3.**

Option 4 (prioritise reveals over new commitments) is the cheapest mitigation for the half of this finding that costs money, and I would take it first. Note the implementation detail the option glosses: `next_transaction` selects `ORDER BY id ASC` (`crates/core/src/tx/storage.rs:151-153`), i.e. strict FIFO by insertion. A priority column is therefore a **core** change to `crates/core/src/tx/storage.rs`, not a sentinel change — the option's alternative ("emit reveals with a lower `id` ordering") is not achievable from the service side, because the service does not choose row ids. State that, or it will be scoped wrong.

Option 1 (bound concurrent effects with a semaphore in `EffectManager`) is sound and is literally **F-CORE-033 option 1**. One change, two findings. Its stated tradeoff is honest: a queued check may still miss its window, but a _predictable_ abstention beats a stampede that degrades every check.

Option 2 (derive the engine deadline from the request's own `commit_deadline` rather than from config) is sound, is what the `TODO` at `main.rs:45-49` already anticipates, and is **F-SEN-009 option 3** and **F-SEN-015**'s neighbourhood too. It has a second benefit this finding does not claim: a replayed check that cannot possibly finish in time is abandoned immediately, which shrinks F-SEN-015 variant 2's window.

Option 3 (cap outstanding bonds as a fraction of the signer's fee-token balance) is sound and is the only option that bounds the _funds_ exposure rather than the concurrency. Its stated requirement — that deliberate non-participation be visible in metrics — is essential, not optional: an invisible abstention cap is indistinguishable from the engine failing.

Option 5 (operational documentation and alerts) is worth taking regardless.

## Post-merge revalidation (RV-SEN)

Re-validated against merge commit `a7f3915` (baseline `2893917`).

### Verdict: **STILL VALID** — nothing cited was touched

**Merged-code citations (all byte-identical, same line numbers):**

- Unbounded engine-check spawn: `crates/sentinel/src/service.rs:137-144`.
- Unconditional bonding with no cap or back-pressure: `crates/sentinel/src/service.rs:226-242`.
- `CheckOutcome::Unknown` drops the request: `service.rs:175-179`.
- `crates/core/src/effects.rs:54-62`, `crates/core/src/tx/mod.rs:202-219`, `crates/core/src/tx/storage.rs:144-168` — `crates/core` is byte-identical across the merge (`git diff 2893917 HEAD -- crates/core` is empty).
- Solidity: the fee refund on timeout is unchanged; the slash rule moved to `contracts/src/libraries/SentinelOracleRequests.sol:286-293`.

No concurrency limit, outstanding-bond cap or reveal-throughput guard was added anywhere in the merge. The two new epics (`epics/2026_09_04_sentinel_verdict_composition.md`, `epics/2026_09_04_sentinel_batch_meta_transactions.md`) both scope themselves to `crates/sentinel-engine` and state "no change to `crates/sentinel`", so neither addresses the sentinel-side flooding path; if anything, per-call coverage and batch flattening make each engine check more expensive, which sharpens consequence 1.

**Certainty 62% and severity Medium / Medium unchanged.** Status left at `Critiqued`. No PoC exists for this finding.

## In-flight impact (AS-SEN)

**Pertains to unmerged branches, not to `main`.** Assessed against `origin/fix/sentinel_deadlines` (PR #914) and `origin/feat/optimistic_block_transition` (PR #915, tip `b2aad06`), built from a `git archive` extraction. Both apply cleanly onto `main` — `main`'s crates are byte-identical to their base `199629e`, so a verdict on the tip is also the verdict for the tip merged into `main`. **Effect: unchanged.**
