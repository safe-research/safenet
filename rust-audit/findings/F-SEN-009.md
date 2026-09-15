# F-SEN-009 The engine timeout is derived from an unvalidated config value instead of the oracle's real commit window, so `voting_window` silently controls whether the sentinel can vote at all

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | sentinel, main.rs / config.rs |
| Location | crates/sentinel/src/main.rs:45-62 (related: crates/sentinel/src/config.rs:41-59, crates/sentinel/src/service.rs:127, 393-399) |
| Severity | Low / Low |
| Certainty | 82% (set by Critic C-SEN; QA may raise) |
| Assumptions involved | A1, A10 |
| Tags | config, known |

## Claim

`engine_timeout` is computed at startup as `max(1 s, (voting_window - 1) × block_time × 3 / 4)` from the operator-supplied `[sentinel].voting_window`, which is never validated and has no relation to the oracle's onchain `COMMIT_WINDOW`. The same value also sets `WaitingForEngineCheck`'s pre-`NewRequest` deadline (`service.rs:127`). Three consequences:

1. `voting_window ∈ {0, 1}` collapses the engine budget to the 1-second floor. Any engine check slower than one second returns `CheckOutcome::Unknown` and the request is dropped unanswered (`service.rs:176-179`). The sentinel silently never votes on anything, and this is a `deny_unknown_fields` config the schema accepts without complaint.
2. `voting_window` much larger than `COMMIT_WINDOW` gives the engine a budget longer than the window in which a commit is even possible. The late resume finds the entry already expired at `service.rs:393-399` and is discarded (`service.rs:167-170`), so the check was pure waste — and, again, the sentinel silently never votes.
3. The budget is a fixed fraction of a static configured window and ignores how much of the _actual_ request's commit window has already elapsed. This is the acknowledged `TODO` at `main.rs:45-49`; it matters most on the replay paths of F-SEN-001 and F-SEN-003, where a check is re-spawned for a proposal whose commit deadline may already have passed and is still given the full budget.

There is also a documentation mismatch that makes misconfiguration more likely: `main.rs:47-49` describes the value as "the configured time until the block before the **reveal** deadline", while `config.rs:54-55` and `sentinel.sample.toml:33-34` describe `voting_window` as "the number of blocks a `Preparing` request is kept alive for before being cleaned up". The two readings differ by `COMMIT_WINDOW + REVEAL_WINDOW`.

This finding covers the `known` item recorded at `codebase-map.md` Section 4 for `crates/sentinel/src/main.rs:45` and is filed at reduced priority accordingly.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The timeout is derived from the config value, with a 1-second floor and a saturating `-1` | E2 | crates/sentinel/src/main.rs:45-62 | `    // TODO: Derive this from effect lifecycle data so time elapsed before the`<br>`    // effect starts can be deducted from the request's actual reveal deadline.`<br>`    // For now, use three quarters of the configured time until the block`<br>`    // before the reveal deadline to leave some wiggle room for delays, with a`<br>`    // one-second minimum for practical deployments.`<br>`    let engine_timeout = {`<br>`        let block_time = config.driver.index.blocks.block_time.resolve(chain_id)?;`<br>`        Duration::from_millis(`<br>`            u64::try_from(`<br>`                u128::from(config.sentinel.voting_window.saturating_sub(1))`<br>`                    .saturating_mul(u128::from(block_time))`<br>`                    .saturating_mul(3)`<br>`                    / 4,`<br>`            )`<br>`            .unwrap_or(u64::MAX)`<br>`            .max(1_000),`<br>`        )`<br>`    };` |
| 2 | `voting_window` is a plain `u64` with no validation and a differing doc comment | E2 | crates/sentinel/src/config.rs:49-59 | `#[derive(Debug, Deserialize)]`<br>`#[serde(deny_unknown_fields)]`<br>`pub struct SentinelConfig {`<br>`    /// The ERC-20 fee token approved for bonds.`<br>`    pub fee_token: Address,`<br>`    /// The number of blocks a \`Preparing\` request is kept alive for before`<br>` /// being cleaned up.`<br>` pub voting_window: u64,`<br>` /// Base URL of the transaction-verification engine used by this sentinel.`<br>` pub engine: Url,`<br>`}` |
| 3 | The same value is the pre-`NewRequest` deadline | E2 | crates/sentinel/src/service.rs:127 | `        let deadline = block.saturating_add(self.voting_window);` |
| 4 | A resume that arrives after the entry expired is discarded, so an over-long budget means no vote | E2 | crates/sentinel/src/service.rs:167-170 | `            None => {`<br>`                tracing::warn!(%request_id, "ignoring stale engine check result");`<br>`                return (state, Vec::new);`<br>`            }` |
| 5 | The tracked entry expires at the real onchain commit deadline once `NewRequest` is known | E2 | crates/sentinel/src/service.rs:393-399 | `        state.0.retain(\|id, entry\| match entry {`<br>`            RequestState::WaitingForEngineCheck { deadline, request } => {`<br>`                block`<br>`                    <= request`<br>`                        .as_ref`<br>`                        .map_or(*deadline, \|request\| request.commit_deadline)`<br>`            }` |
| 6 | The timeout is applied to every engine call, so it directly decides success or `Unknown` | E2 | crates/sentinel/src/effect.rs:62-68 | `                let outcome = self`<br>`                    .engine`<br>`                    .security_check(block, &transaction)`<br>`                    .request_id(request_id)`<br>`                    .timeout(self.engine_timeout)`<br>`                    .execute`<br>`                    .await;` |
| 7 | The real commit deadline is set by the oracle from `COMMIT_WINDOW`, which the sentinel never reads | E2 (Solidity reference, A7) | contracts/src/SentinelOracle.sol:217-221 | `        uint256 commitDeadlineWide = block.number + COMMIT_WINDOW;`<br>`        uint256 revealDeadlineWide = commitDeadlineWide + REVEAL_WINDOW;`<br>`        uint64 commitDeadline = commitDeadlineWide.toUint64;`<br>`        uint64 revealDeadline = revealDeadlineWide.toUint64;` |

## Trigger

- Set `voting_window = 1` in an otherwise valid TOML. `Config::load` accepts it (the only validation is TOML shape, `config.rs:61-67`, and the `deserializes_required_fields_and_defaults_the_rest` test at `config.rs:87-115` asserts nothing about the value). `engine_timeout` becomes `max(1 s, 0)` = 1 s. Every check against a real engine that performs RPC-backed lookbacks exceeds it, returns `Unknown`, and the request is dropped with a `warn` (`service.rs:176-179`). The sentinel runs, indexes, logs normally, and never commits.
- Set `voting_window = 1000` against an oracle whose `COMMIT_WINDOW` is 5. `engine_timeout` becomes `(999 × 5000 × 3) / 4 ≈ 62 minutes`. A slow engine will now be waited on long past the commit deadline; the entry is dropped at `service.rs:393-399` and the eventual resume is discarded at `service.rs:167-170`, again with no vote. Meanwhile the effect task stays alive holding an HTTP connection, adding to the concurrency pressure in F-SEN-004.

## Considered and rejected

- **"An over-long timeout is safe because a late resume cannot vote."** Correct as a _safety_ statement — that is why this is Low and not Medium — but it is a liveness and resource problem: the sentinel abstains, and the abandoned checks accumulate.
- **"The 1-second floor protects against `voting_window = 0`."** The floor is what _causes_ the problem in case 1: it guarantees a non-zero but uselessly small budget instead of failing loudly.
- **"The arithmetic could overflow."** It cannot: `saturating_sub`, `saturating_mul` on `u128` and `u64::try_from(..).unwrap_or(u64::MAX)` are all saturating (basis 1). Verified as safe.
- **"`block_time` could be wrong."** `BlockTime::resolve(chain_id)` returns an error for an unknown chain when set to `auto` (`crates/core/src/index/blocks.rs:34-43`), and `main.rs:51` propagates it with `?`, so that path does fail loudly.
- **"The TODO means it is already tracked."** It is, which is why this is filed as `known` at reduced priority — but the `TODO` only covers point 3 (deriving the budget from effect lifecycle data). Points 1 and 2, and the doc mismatch, are not mentioned by it.
- **False positive check — is `voting_window` used anywhere else?** `grep -n "voting_window" crates/sentinel/src` shows only `config.rs:56`, `main.rs:54,70`, `service.rs:35,56,78,88,817,827,863,887` (the last few are the struct field and the test constant) and `service.rs:127`. There is no validation site.

## Remediation options

1. **Read the window from the chain.** Bind `SentinelOracle.COMMIT_WINDOW` (and `REVEAL_WINDOW`), read them once at startup, and derive `engine_timeout` from `COMMIT_WINDOW × block_time`. Removes the whole class of mismatch and makes `voting_window` unnecessary as a timing knob. Tradeoff: one extra `eth_call` at startup and a hard dependency on the oracle being deployed when the sentinel starts.
2. **Validate `voting_window`.** Reject values below 2 (or below whatever floor makes the derived timeout meaningful) in a `Config::load` validation pass, and warn when `voting_window × block_time` far exceeds a plausible commit window. Cheap; catches case 1 at startup instead of in production silence.
3. **Deadline per request, not per process.** Carry the request's own `commit_deadline` into `Effect::EngineCheck` and compute the timeout as `(commit_deadline - current_block) × block_time × 3 / 4` inside the handler. This is what the `TODO` asks for and also stops replayed checks from being given a budget they cannot use.
4. **Fix the documentation drift.** Make `main.rs:47-49`, `config.rs:54-55` and `sentinel.sample.toml:33-34` agree on what `voting_window` measures, and say explicitly that it is a _local_ value with no onchain counterpart.

Tests to add: a `config.rs` test asserting `voting_window = 0` and `= 1` are rejected (or that the derived timeout is at least some floor). A `main`-level unit for the timeout derivation covering `0`, `1`, a normal value and `u64::MAX`.

## Trail

- Reviewer R7: drafted from lead SEN-H12 plus the `known` `TODO` at `main.rs:45` (codebase-map Section 4), self-estimate 80%. All seven basis citations re-opened in this checkout. Tagged `known` and filed at reduced priority per A12.

## Critic (C-SEN)

### Per-claim verdicts

All rows re-opened against `main.rs:45-62`, `config.rs:49-59`, `effect.rs:62-68`, `service.rs:127`, `:167-170`, `:393-399`. Every quote is accurate; no claim marked `H`. I confirmed by reading `config.rs` in full that `SentinelConfig` applies no validation of any kind to `voting_window` — it is a bare `pub voting_window: u64` with `deny_unknown_fields` only, so `0` is accepted.

I re-derived the arithmetic: `voting_window = 0` gives `saturating_sub(1) = 0`, so the product is `0` and the `.max(1_000)` floor makes the engine budget exactly 1 s; the same `0` also makes `deadline = block.saturating_add(0) = block` at `service.rs:127`, so a `WaitingForEngineCheck` entry created at block `b` is dropped at `b+1` by the `block <= *deadline` test at `:393-399` — before almost any real engine could answer. Consequence 1 is therefore stronger than stated: the check is not merely likely to time out, the entry is _also_ reaped a block later.

The documentation mismatch is real and is the practical risk: `main.rs:47-49` says "the block before the reveal deadline" while `config.rs:54-55` and `sentinel.sample.toml:33-34` say "blocks a `Preparing` request is kept alive for". Those are different quantities and an operator has no way to know which one to size against, since neither is `COMMIT_WINDOW`.

### Finding verdict

**Confirmed. Certainty 82%. Severity Low (unchanged).**

`E2` mechanism and a concrete trigger (`voting_window ∈ {0,1}`, or any value uncorrelated with the oracle's real `COMMIT_WINDOW`). Low is right per Section 8 — a configuration weakness with limited impact, and under A1 the operator provisioning it is trusted. Correctly tagged `known` (A12) against the `TODO` at `main.rs:45-49`; that reduces priority but not certainty.

The remediation worth naming first is not a range check on `voting_window` but reading `COMMIT_WINDOW`/`REVEAL_WINDOW` off the oracle at startup, which also closes half of F-SEN-007.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

**Note for every sentinel finding whose "tests to add" list names a `service.rs` unit test:** `sentinel` is a **binary-only crate** — `crates/sentinel/src/main.rs` declares `mod service;` and there is no `lib.rs`, so the crate has no library target and `crates/sentinel/tests/` cannot compile against it. Every such test must live inside the existing `#[cfg(test)] mod tests` in the source file. If the team wants these as permanent regression tests reachable from an integration target, **the crate needs a `lib.rs` first**; that is an unstated prerequisite across F-SEN-001, -002, -003, -011, -012 and -015.

### Remediation check

**Sound: option 1 is the right fix; option 2 is the right first change.**

Option 1 (bind `COMMIT_WINDOW` / `REVEAL_WINDOW`, read once at startup, derive `engine_timeout` from `COMMIT_WINDOW × block_time`) removes the whole class of mismatch and makes `voting_window` unnecessary as a timing knob. Sound. Its stated cost — a hard dependency on the oracle being deployed when the sentinel starts — is the same dependency **F-SEN-007 option 1** already introduces, so if both land it is paid once.

Option 2 (reject `voting_window` below a floor in `Config::load`) is the right _first_ change: it is two lines, it needs no chain read, and it catches the case at startup instead of in production silence. Combine with **F-SEN-010 option 1**, which wants a validation pass in the same place.

Option 3 (carry the request's own `commit_deadline` into `Effect::EngineCheck` and compute the timeout per request) is what the `TODO` at `main.rs:45-49` asks for and is the most correct of the three. It has a benefit neither the finding nor the `TODO` claims: a **replayed** engine check for a request whose commit deadline has already passed would be given a non-positive budget and abandoned immediately, which materially shrinks **F-SEN-015 variant 2**'s window. Worth recording as a reason to prefer option 3 over option 1.

Option 4 (make `main.rs:47-49`, `config.rs:54-55` and `sentinel.sample.toml:33-34` agree, and say `voting_window` is a purely local value with no onchain counterpart) is unconditionally correct and costs nothing.

## Post-merge revalidation (RV-SEN)

Re-validated against merge commit `a7f3915` (baseline `2893917`).

### Verdict: **STILL VALID** — none of the cited files changed

`crates/sentinel/src/main.rs`, `crates/sentinel/src/config.rs` and `crates/sentinel/src/engine.rs` are all untouched by the merge (`git diff 2893917 HEAD --` over them is empty), so `main.rs:45-62` and `config.rs:41-59` stand verbatim. On the `service.rs` side both citations are byte-identical at their original line numbers:

- `service.rs:127` — `let deadline = block.saturating_add(self.voting_window);`
- `service.rs:393-399` — the `WaitingForEngineCheck` expiry arm in `handle_block_advance`
- `service.rs:175-179` — the `CheckOutcome::Unknown` drop (consequence 1)
- `service.rs:167-170` — the stale-resume discard (consequence 2)

The `voting_window` value is still unvalidated, still has no relation to the oracle's onchain `COMMIT_WINDOW`, and the `TODO` at `main.rs:45-49` is still open. The documentation mismatch between `main.rs:47-49` and `config.rs:54-55` / `sentinel.sample.toml:33-34` is unchanged.

Consequence 3 is if anything sharper after the merge: it names the replay paths of F-SEN-001 and F-SEN-003, and F-SEN-001 was re-confirmed unchanged this run.

**Certainty 82% and severity Low / Low unchanged.** Status left at `Critiqued`; the `known` tag still applies.

## In-flight impact (AS-SEN)

**Pertains to unmerged branches, not to `main`.** Assessed against `origin/fix/sentinel_deadlines` (PR #914) and `origin/feat/optimistic_block_transition` (PR #915, tip `b2aad06`), built from a `git archive` extraction. Both apply cleanly onto `main` — `main`'s crates are byte-identical to their base `199629e`, so a verdict on the tip is also the verdict for the tip merged into `main`. **Effect: unchanged.** `service.rs:127` is still `block.saturating_add(self.voting_window)`, unvalidated.
