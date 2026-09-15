# F-SEN-012 The engine client makes exactly one attempt per proposal, so any transient failure inside a window that still has blocks left is a permanent abstention

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | sentinel, engine.rs / service.rs |
| Location | crates/sentinel/src/engine.rs:163-194 (related: crates/sentinel/src/service.rs:176-179, crates/sentinel/src/effect.rs:54-76) |
| Severity | Low / Low |
| Certainty | 85% (set by Critic C-SEN; QA may raise) |
| Assumptions involved | A3 |
| Tags | dos |

## Claim

`SecurityCheck::execute` collapses every failure mode — connection refused, DNS failure, a 500 or 503, a request timeout, a truncated or unparseable body, a rule code outside `R-<u32>.<u32>` — into a single `CheckOutcome::Unknown` (`engine.rs:163-194`). `handle_engine_check_result` treats `Unknown` by removing the tracked entry outright (`service.rs:176-179`), and nothing re-issues the effect. There is no retry, no backoff, and no distinction between "the engine says it cannot decide" and "the engine did not answer".

Refusing to _vote_ on a failed check is the right call and is documented as deliberate (`engine.rs:86-91`). Refusing to _ask again_ is a separate decision that the code makes implicitly. An engine restart, a rolling deployment, a momentary connection reset, or one slow check that just exceeded a budget deliberately sized at three quarters of the window costs the sentinel every request in flight at that instant, even when several blocks of the commit window remain. Because the metric label for a failed check (`EngineCheckVerdict::Error`) is recorded but the sentinel takes no action on it, a fleet-wide engine blip produces a fleet-wide abstention, which is the condition under which a request times out and the sponsor's fee is refunded (`contracts/src/SentinelOracle.sol:273-277`) — i.e. an attacker who can degrade engines gets free proposals.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Every error path collapses to `Unknown`; one attempt, no retry | E2 | crates/sentinel/src/engine.rs:166-190 | `        async move {`<br>`            let result: Result<Response, reqwest::Error> =`<br>`                async { request.send.await?.error_for_status?.json.await }.await;`<br>``<br>`            let (outcome, verdict) = match result {`<br>`                Ok(Response::Secure) => (CheckOutcome::Approved, EngineCheckVerdict::Secure),`<br>`                Ok(Response::Insecure { rule }) => {`<br>`                    (CheckOutcome::Denied(rule), EngineCheckVerdict::Insecure)`<br>`                }`<br>`                Ok(Response::Abstain) => (CheckOutcome::Unknown, EngineCheckVerdict::Abstain),`<br>`                Err(err) => {`<br>`                    tracing::error!(`<br>`                        %err,`<br>`                        "sentinel engine request failed; dropping the request unanswered",`<br>`                    );`<br>`                    (CheckOutcome::Unknown, EngineCheckVerdict::Error)`<br>`                }`<br>`            };` |
| 2 | `Unknown` removes the entry, so no later path can retry | E2 | crates/sentinel/src/service.rs:173-180 | `        let (approve, reason) = match outcome {`<br>`            CheckOutcome::Approved => (true, String::new),`<br>`            CheckOutcome::Denied(rule) => (false, rule.to_string),`<br>`            CheckOutcome::Unknown => {`<br>`                tracing::warn!(%request_id, "engine check failed; dropping request unanswered");`<br>`                return (state, Vec::new);`<br>`            }`<br>`        };` |
| 3 | The handler performs exactly one call and resumes with its outcome | E2 | crates/sentinel/src/effect.rs:55-73 | `    async fn perform_effect(&self, effect: Effect) -> Resume {`<br>`        match effect {`<br>`            Effect::EngineCheck {`<br>`                request_id,`<br>`                transaction,`<br>`                block,`<br>`            } => {`<br>`                let outcome = self`<br>`                    .engine`<br>`                    .security_check(block, &transaction)`<br>`                    .request_id(request_id)`<br>`                    .timeout(self.engine_timeout)`<br>`                    .execute`<br>`                    .await;`<br>`                Resume::EngineCheckResult {`<br>`                    request_id,`<br>`                    outcome,`<br>`                }`<br>`            }`<br>`        }`<br>`    }` |
| 4 | Transport failure, non-2xx and malformed rule are all covered by the same collapse, and the tests confirm it | E2 | crates/sentinel/src/engine.rs:306-321 | `    #[tokio::test]`<br>`    async fn fails_on_a_malformed_rule_code {`<br>`        let url = respond_once(`<br>`            "200 OK",`<br>`            r#"{"verdict":"insecure","rule":"not-a-real-rule"}"#,`<br>`        )`<br>`        .await;`<br>`        let engine = EngineClient::new(url).unwrap;`<br>`        assert_eq!(`<br>`            engine`<br>`                .security_check(1, &SafeTransaction::default)`<br>`                .execute`<br>`                .await,`<br>`            CheckOutcome::Unknown`<br>`        );`<br>`    }` |
| 5 | The `error` metric label exists, so the condition is observable but not acted on | E2 | crates/sentinel/src/metrics.rs:20-23 | `    /// The engine had no trustworthy verdict either way.`<br>`    Abstain,`<br>`    /// The check itself failed -- a transport failure, timeout, non-2xx`<br>`    /// status, or unparseable response.`<br>`    Error,` |
| 6 | The entry would still have been valid to commit for the rest of the commit window | E2 | crates/sentinel/src/service.rs:393-399 | `        state.0.retain(\|id, entry\| match entry {`<br>`            RequestState::WaitingForEngineCheck { deadline, request } => {`<br>`                block`<br>`                    <= request`<br>`                        .as_ref`<br>`                        .map_or(*deadline, \|request\| request.commit_deadline)`<br>`            }` |

## Trigger

1. Block `b`: request R opens; the sentinel spawns its engine check. `commit_deadline = b + COMMIT_WINDOW`.
2. Block `b` (milliseconds later): the operator's rolling deployment restarts the co-deployed engine, or the engine's own process is briefly unavailable. `request.send` fails with a connection error.
3. `execute` returns `Unknown` after one attempt (basis 1). `handle_engine_check_result` deletes R's entry (basis 2).
4. Blocks `b+1 .. b+COMMIT_WINDOW`: the engine is healthy again, and R is still fully commitable (basis 6) — but the sentinel has forgotten it. No vote.

Fleet variant: because sentinels typically deploy from the same artefact, a coordinated engine restart abstains every sentinel at once, and the request times out with the sponsor's fee refunded.

## Considered and rejected

- **"A failed check must not become a vote."** Agreed, and that is not what this asks for. The remediation is to re-issue the _check_, not to guess the verdict; the `Unknown → no vote` mapping stays exactly as it is.
- **"The state machine could retry on the next block."** It cannot as written: the entry is deleted at step 3, so `handle_block_advance` never sees it again and there is nothing left holding the transaction bytes (see F-SEN-011 basis 2).
- **"A retry would double the load on a struggling engine."** It would if unbounded, which is why the remediation options below are all bounded (fixed attempts, or bounded by the remaining commit window) and why option 3 pairs the retry with the concurrency cap from F-SEN-004.
- **"Abstaining is the safe default."** For a single sentinel, yes. Across the fleet, universal abstention is the failure mode that makes a proposal flood free (F-SEN-004 basis 8), so it is a liveness property of the oracle, not just of one operator.
- **"`abstain` and `error` are indistinguishable to the FSM."** They are (basis 1: both map to `CheckOutcome::Unknown`), which is the reason the FSM cannot implement a retry policy even if it wanted to — the metric label is the only place the distinction survives (basis 5). Splitting `CheckOutcome::Unknown` into `Abstained` and `Failed` is a prerequisite for option 1.
- **False positive check — is there a retry anywhere in the stack?** `core::effects` spawns once and never re-spawns (`crates/core/src/effects.rs:54-62`); the driver retries only _watcher_ errors (`crates/core/src/driver.rs:206-225`), not effects. `grep -n "retry\|backoff" crates/sentinel/src` returns nothing.

## Remediation options

1. **Split the outcome and retry the failures.** Change `CheckOutcome::Unknown` into `Abstained` (a real verdict — drop the request) and `Failed` (no verdict). On `Failed`, re-emit `Effect::EngineCheck` if the request's commit deadline has not passed, up to a small bounded number of attempts, with a short delay. Requires carrying the transaction in the state (F-SEN-011 option 1) or re-reading it (option 2). Tradeoff: the state grows, and a persistently broken engine now generates N times the traffic — bound the attempts and pair with F-SEN-004's concurrency cap.
2. **Retry inside the client instead.** Add a bounded retry loop with jittered backoff inside `SecurityCheck::execute`, subordinate to the same overall `timeout` budget, so the FSM is unchanged and no state grows. Cheapest option; it cannot survive an engine outage longer than one budget, but it does cover restarts and connection resets. Tradeoff: the per-attempt timeout must be derived from the overall budget rather than equal to it.
3. **Retry only the cheap failures.** Distinguish connection-level errors (`err.is_connect`, `err.is_request`) from a timeout, and retry only the former — a timeout has already consumed the budget and should not be repeated.
4. **Operational:** alert on `safenet_sentinel_engine_check_verdicts_total{verdict="error"}` and make engine deployments drain rather than restart abruptly.

Tests to add: an `engine.rs` test where the first connection is refused and the second succeeds, asserting the outcome is `Approved`. A `service.rs` test asserting a `Failed` outcome leaves the entry in place for another attempt while an `Abstained` outcome removes it.

## Trail

- Reviewer R7: drafted from lead SEN-H11, self-estimate 90% on the mechanism (it is by construction, and the existing tests at `engine.rs:345-391` assert exactly this behaviour). Severity is Low because the consequence is missed participation rather than fund loss, and because the single-attempt design is a documented choice — the finding is that the choice conflates "no verdict" with "no answer". All six basis citations re-opened in this checkout.

## Critic (C-SEN)

### Per-claim verdicts

All rows re-opened (`engine.rs:166-190`, `:306-321`, `service.rs:173-180`, `:393-399`, `effect.rs:55-73`, `metrics.rs:20-23`). Every quote is accurate; no claim marked `H`.

I verified the collapse is total by reading `SecurityCheck::execute` in full: the inner `async { request.send.await?.error_for_status?.json.await }` folds transport errors, non-2xx statuses and body-parse failures into one `reqwest::Error`, which maps to `(CheckOutcome::Unknown, EngineCheckVerdict::Error)` at `engine.rs:181-187`. A malformed rule code takes a different route — `RuleId::deserialize` fails inside `.json`, so it also arrives as `Unknown` (confirmed by the test at `engine.rs:306-321`). `Response::Abstain` maps to the _same_ `CheckOutcome::Unknown` at `:175`, so the state machine genuinely cannot distinguish "the engine declined to rule" from "the engine did not answer" — only the metric label differs, and nothing acts on it. `grep -rn "retry\|backoff" crates/sentinel/src/` returns nothing; I re-ran it.

### Assessment

The finding's framing is the right one and I want to keep it: refusing to _vote_ on a failed check is correct and documented (`engine.rs:86-91`); refusing to _ask again_ is a separate decision the code makes implicitly, and it is the defect. A rolling engine deployment during a busy block costs the sentinel every request in flight even when most of the commit window remains.

One correction to the Claim's last sentence: "an attacker who can degrade engines gets free proposals" overstates it under **A3**, which puts the engine on a co-deployed, non-attacker-reachable network path. Absent a bypass inside that deployment, engine degradation is an availability event, not an attack surface — so the fee-refund economics belong to F-SEN-004 (where the attacker's lever is proposal volume, not engine access), not here.

### Finding verdict

**Confirmed. Certainty 85%. Severity Low (unchanged).**

`E2` mechanism, certain trigger (any transient engine failure). Low per Section 8: an error-handling weakness with contained impact — one abstention per affected request, no funds at risk, and under A3 no attacker lever. It matters most as a multiplier on F-SEN-001 and the new F-SEN-015, where a replayed check that returns `Unknown` does not merely abstain but _deletes_ a request whose bond is already posted.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

**Note for every sentinel finding whose "tests to add" list names a `service.rs` unit test:** `sentinel` is a **binary-only crate** — `crates/sentinel/src/main.rs` declares `mod service;` and there is no `lib.rs`, so the crate has no library target and `crates/sentinel/tests/` cannot compile against it. Every such test must live inside the existing `#[cfg(test)] mod tests` in the source file. If the team wants these as permanent regression tests reachable from an integration target, **the crate needs a `lib.rs` first**; that is an unstated prerequisite across F-SEN-001, -002, -003, -011, -012 and -015.

### Remediation check

**Sound: option 1 is the correct fix; option 2 is the cheap one and covers less than it appears.**

Option 1 (split `CheckOutcome::Unknown` into `Abstained` — a real verdict — and `Failed` — no verdict — and re-emit the effect on `Failed` while the commit deadline allows) is sound and is the right shape. It also **directly fixes F-SEN-015 variant 2**: today the `Unknown` arm removes the entry and never re-inserts it (`crates/sentinel/src/service.rs:156`, `:176-179`), which is how a request with a live onchain bond becomes untracked. A `Failed` outcome that leaves the entry in place removes that loss. That is a stronger argument for option 1 than the one the finding makes, and the report should carry it.

Its stated prerequisite is real: re-emitting needs the transaction, so option 1 depends on **F-SEN-011 option 1 or 2**. And its stated risk — a persistently broken engine generating N times the traffic — needs **F-SEN-004 option 1**'s concurrency cap, exactly as the text says.

Option 2 (a bounded retry with jittered backoff inside `SecurityCheck::execute`, subordinate to the same overall budget) is the cheapest and needs no state change, but it covers less than it looks: it cannot survive an engine outage longer than one budget, and — the case that matters — it does **not** help on the restart path, where the co-deployed engine is still booting and the whole budget may elapse before it is ready. Its own caveat about deriving a per-attempt timeout from the overall budget is correct and easy to get wrong.

Option 3 (retry only connection-level errors, not timeouts) is sound and is a sensible refinement of either 1 or 2 — a timeout has already consumed the budget and repeating it just consumes another.

Option 4 (alert on `engine_check_verdicts_total{verdict="error"}`, drain rather than restart engine deployments) is operational and worth taking regardless.

## Post-merge revalidation (RV-SEN)

Re-validated against merge commit `a7f3915` (baseline `2893917`).

### Verdict: **STILL VALID** — no retry, no backoff, no re-issue was added

- `crates/sentinel/src/engine.rs` is untouched by the merge, so `engine.rs:163-194` (every failure mode collapsed to `CheckOutcome::Unknown`) and the deliberate-abstention comment at `:86-91` stand verbatim.
- `crates/sentinel/src/service.rs:175-179` is byte-identical — the `Unknown` arm still returns after `handle_engine_check_result` has already removed the entry at `:156`, and nothing re-issues `Effect::EngineCheck`.
- `crates/sentinel/src/effect.rs:54-76` is untouched.

The merge added three new _oracle-event_ handlers; none of them can re-create a request that `handle_engine_check_result` dropped, because the entry is gone and all three no-op on an untracked request (`service.rs:674-680`, `:728-734`, `:780-786`) — verified by execution under F-SEN-001's _branch B_ probe this run.

**Certainty 85% and severity Low / Low unchanged.** Status left at `Critiqued`.
