# F-CORE-034 Every watcher error is retried at a fixed 100 ms forever, with one warning line per attempt: a rate-limited or deterministically-failing node becomes a self-sustaining retry storm and a log flood

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | core, `driver.rs` |
| Location | `crates/core/src/driver.rs:200-231` and `:24-26` (related: `crates/core/src/index/blocks.rs:401-416`; `crates/core/src/metrics.rs:62-70`; `docs/validator-handbook.md:110`) |
| Severity | Medium / Medium |
| Certainty | 80% |
| Assumptions involved | A4, A1 |
| Tags | dos, config |

## Claim

`Driver::next_input` retries _every_ watcher error except `ExceededMaxReorgDepth` after a constant `STEP_RETRY_DELAY` of 100 ms, in an unbounded loop, emitting a `warn` line with the full error on every attempt. There is no backoff, no jitter, no attempt cap, no error classification and no metric.

Three consequences follow, all reachable with a merely unhealthy — not malicious — RPC provider (assumption A4):

1. **Retry storm.** A provider that rate-limits us is hit ten times a second, indefinitely, which is the behaviour most likely to keep the limit engaged. In `MultipleQueries` mode each attempt is one `eth_getLogs` per watched topic (`index/events.rs:412-440`), multiplying the rate. Nothing in core sheds load; the only other retry paths — the block poller's `[200, 100, 100]` cycle and the event pager's page halving — are also fixed-delay, so the composite behaviour under a sustained outage is a constant-rate request flood.
2. **Log flood.** ~10 `warn` records per second is ~864,000 lines per day, each carrying the error's full `Debug` and, in JSON mode (the default whenever stdout is not a TTY, `observability/logging.rs:16-19`), the full structured envelope. For a node down over a weekend that is millions of records, on the same disk as the SQLite database whose write failures are fatal (F-CORE-030 basis 3).
3. **Deterministic errors never escalate.** `Error::DecodeLog`, `TooManyLogs` and an unsupported `blockHash` filter fail identically on every attempt, so a permanent condition is retried forever at 10 Hz while `safenet_core_block_number{status="processed"}` freezes. The process stays alive, `/health` keeps answering `OK` (F-CORE-030), and there is no counter that distinguishes "retrying" from "idle".

The validator handbook tells operators the opposite ("the validator implements exponential backoff for some RPC requests", `docs/validator-handbook.md:110`); nothing in `core` implements exponential backoff.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The retry delay is a single fixed constant. | E2 | `crates/core/src/driver.rs:24-26` | <pre>/// How long to wait after a failed step before retrying, to avoid spinning on a<br>/// persistent failure (such as an unreachable RPC node).<br>const STEP_RETRY_DELAY: Duration = Duration::from_millis(100);</pre> |
| 2 | Every error except `ExceededMaxReorgDepth` is warned and retried in an unbounded loop; no attempt counter exists. | E2 | `crates/core/src/driver.rs:207-225` | <pre>let update = async {<br> loop {<br> match self.watcher.next.await {<br> Ok(update) => return Ok(update),<br> Err(<br> err @ index::Error::Blocks(index::blocks::Error::ExceededMaxReorgDepth(_)),<br> ) => {<br> return Err(err);<br> }<br> Err(err) => {<br> tracing::warn!(<br> ?err,<br> "failed to get next blockchain update; retrying after delay"<br> );<br> tokio::time::sleep(STEP_RETRY_DELAY).await;<br> }<br> }<br> }<br>};</pre> |
| 3 | The doc comment frames the policy as "retry everything else", with no mention of a bound. | E2 | `crates/core/src/driver.rs:200-205` | <pre>/// Reads the next state machine input to process.<br>///<br>/// Watcher failures are retried after a short delay while completed<br>/// effects remain eligible for selection, except a reorg deeper than the<br>/// configured `max_reorg_depth`: the watcher cannot recover from that on<br>/// its own, so it is returned instead of retried.</pre> |
| 4 | The block poller's own retry ladder is a fixed cycling list, not a backoff. | E2 | `crates/core/src/index/blocks.rs:404-408` | <pre>let index = retry_count % (self.config.block_retry_delays.len + 1);<br><br>if let Some(delay) = self.config.block_retry_delays.get(index).copied {</pre> |
| 5 | The only progress signal is a gauge that silently stops moving; there is no retry or error counter. | E2 | `crates/core/src/metrics.rs:62-70` | <pre>/// The block-number component of the chain-processing cursor, by `status`.<br>pub fn block_number(status: ProcessingStatus) -> Gauge {<br> let status = status.label;<br> metrics::gauge!(<br> description: "Block number by chain-processing status.",<br> "safenet_core_block_number",<br> "status" => status,<br> )<br>}</pre> |
| 6 | Logs are JSON on stdout in any non-TTY deployment, so each retry line is a full record. | E2 | `crates/core/src/observability/logging.rs:14-20` | <pre>pub fn init(filter: EnvFilter) -> Result<, TryInitError> {<br> let registry = tracing_subscriber::registry.with(filter);<br> if std::io::stdout.is_terminal {<br> registry.with(fmt::layer).try_init<br> } else {<br> registry.with(fmt::layer.json).try_init<br> }<br>}</pre> |

## Trigger

1. The configured RPC endpoint starts returning HTTP 429 / JSON-RPC rate-limit errors — the exact failure the validator handbook lists first under "Common Problems" (`docs/validator-handbook.md:110`).
2. `Watcher::next` returns `Err` on every call. `next_input` warns, sleeps 100 ms, and repeats: ten attempts a second, forever, with no upper bound and no widening delay.
3. The provider's quota window never clears, so the outage persists for as long as the process runs; operator-visible symptoms are a frozen `safenet_core_block_number{status="processed"}` and a log stream growing at ~1 line per 100 ms.

The deterministic variant needs no provider fault at all: a single log whose `topic0` collides with a watched event but whose payload does not decode makes `EventWatcher` return `Error::DecodeLog` for that block on every attempt (`index/events.rs:491-519`), so step 2 runs forever on a condition that cannot resolve. That path is R1's `index` scope; the unbounded retry that turns it into a permanent silent stall is this one.

## Considered and rejected

- **"100 ms is deliberate — see the constant's comment."** The comment justifies _having_ a delay ("to avoid spinning on a persistent failure"), which is a different problem from _escalating_ on one. A fixed delay bounds CPU spin, not request rate or log volume.
- **"An operator can alert on the frozen gauge."** They can, and should; but the gauge is also frozen when the chain is idle, and there is nothing to distinguish "stalled retrying" from "nothing happening". A `safenet_core_watcher_errors_total{kind}` counter would.
- **"The RPC layer retries anyway."** It does not: `Provider::connect` installs only an observability layer — no retry layer, no timeout (`provider/mod.rs:129-137`, R1's scope). The 100 ms loop is the entire retry policy.
- **"`ExceededMaxReorgDepth` shows errors are classified."** That is the only classification: one variant is fatal, everything else — transport faults, decode failures, `TooManyLogs`, malformed responses — shares one policy.
- **Rejected as a separate finding:** log-volume alone. It is included here because it shares the cause and the fix (bounded/backed-off retries with rate-limited logging).

## Remediation options

1. Exponential backoff with jitter and a ceiling (e.g. 100 ms → 30 s), reset on the first success. Smallest change, fixes the storm and most of the log flood; matches what the handbook already claims. Tradeoff: recovery from a brief blip is a little slower — mitigable by keeping the first few retries at 100 ms.
2. Classify the error before retrying: transport/timeout/429 → backoff; `DecodeLog`, `TooManyLogs`, an unsupported filter → escalate after N attempts (fatal, as `ExceededMaxReorgDepth` already is, or a distinct "degraded" state). This is the only option that fixes consequence 3.
3. Independently: emit `safenet_core_watcher_errors_total{kind}` and log at `warn` only on the first failure and on a change of error kind (then at `debug`, or every Nth attempt), so a sustained outage is one alertable counter rather than a million log lines.

Tests to add: `driver.rs` has no tests at all (0.0% line coverage, map §2) — a `start_paused` tokio test with a mocked watcher that fails K times then succeeds, asserting the elapsed virtual time matches the intended schedule, would pin whichever policy is chosen. No code is committed.

## Trail

- Reviewer R2: drafted from lead M9, self-estimate 80%. The handbook's backoff claim was re-read this session (`docs/validator-handbook.md:110`); docs are reference-only, cited as context.

## Critic (C-CORE-B)

Read `driver.rs:200-231` and `:24-26` first. `STEP_RETRY_DELAY` is a single `Duration::from_millis(100)` constant; `next_input`'s inner loop matches exactly one error variant as fatal (`index::Error::Blocks(blocks::Error::ExceededMaxReorgDepth(_))`) and warns-sleeps-repeats on every other, unbounded, with no counter and no widening delay. Confirmed independently.

### Per-claim verdicts

| # | Verdict | Note |
| --- | --- | --- |
| 1 | **Supported** | `driver.rs:24-26` verbatim. |
| 2 | **Supported** | `driver.rs:207-225` verbatim. |
| 3 | **Supported** | `driver.rs:200-205` verbatim. |
| 4 | **Supported** | `blocks.rs:404-408`; the modulo cycles the list rather than escalating, and once exhausted the code adds a whole `block_time` instead — still not a backoff. |
| 5 | **Supported** | `metrics.rs:62-70`; there is no error or retry counter in the crate. |
| 6 | **Supported** | `observability/logging.rs:14-20` verbatim: JSON whenever stdout is not a TTY. |
| 7 (handbook) | **Supported** | `docs/validator-handbook.md:110` reads "While the validator implements exponential backoff for some RPC requests, rate limits can still prevent full participation". Reference-only material, correctly cited as context; the contradiction with `core` is real. |

I also verified the reviewer's "the RPC layer retries anyway" rebuttal: `Provider::connect` installs `ObservabilityLayer` and nothing else (`provider/mod.rs:129-137`), and `grep -rn timeout crates/core/src` finds no timeout outside `utils.rs`'s sqlx pool knobs and one test. The 100 ms loop really is the entire retry policy.

### A4 check — the trigger is on the right side of the line

Consequences 1 and 2 rest on a **rate-limited or erroring** provider, which the brief puts explicitly **in** scope; no malicious behaviour is needed and none is claimed. Consequence 3's `DecodeLog` variant is weaker than presented: the event watcher filters by the watched contract addresses, so a log with a matching `topic0` and an undecodable payload would have to be emitted by the audited Solidity itself (A7), which makes that instance `I` rather than `E2`. The `TooManyLogs` instance is reachable from a provider that truncates, which A4 admits. I therefore accept consequences 1 and 2 as `E2`-triggered and treat the `DecodeLog` half as an unproven sub-case.

### Finding verdict

**Confirmed — 80%.** Mechanism and the rate-limit trigger both verified; only the deterministic-error sub-case is unproven, and it is not load-bearing.

**Severity: Medium (unchanged).** Not High: the process stays alive and recovers the moment the provider does, and the block gauge does freeze, so an operator watching metrics has _a_ signal. Not Low: 10 Hz against a rate-limited endpoint is the behaviour most likely to keep the limit engaged, ~864k log lines a day land on the same disk as the SQLite file whose write failures are fatal (F-CORE-030 basis 3), and the handbook actively misinforms operators about the policy.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 1 plus option 3; option 2 is the only one that fixes the third consequence.**

Option 1 (exponential backoff with jitter and a ceiling, reset on success, first few retries kept at 100 ms) is sound and is the smallest change that stops the storm. It is also a **prerequisite for F-CORE-011 option 1**: adding a request timeout to the provider without backoff converts a slow node into a faster retry storm.

Option 2 (classify before retrying — transport/429 → backoff, `DecodeLog`/`TooManyLogs`/unsupported filter → escalate) is the same change as **F-CORE-004 option 1** and is the only option here that addresses a deterministically failing node rather than a slow one. The two findings should be fixed by one classification, not two.

Option 3 (`watcher_errors_total{kind}`, and log at `warn` only on the first failure and on a change of error kind) is sound and is what turns a million log lines into one alertable counter. Take it regardless.

**Contract note:** none of these touch `apply_transition` or effect delivery. Backoff does lengthen the window in which a rollback replay has not yet happened, which very slightly reduces **F-CORE-067**'s frequency and does nothing to its mechanism — not a reason to choose or avoid any option, but worth not mistaking for a fix.

## In-flight impact (AS-PRUNE)

**Pertains to unmerged branches, not to `main`.** Assessed against the Scheduled Secret Pruning stack (`origin/prune/end`, PRs #906–#913), built from a `git archive` extraction; no branch was merged or checked out. **Effect: unchanged.** `next_input` (`crates/core/src/driver.rs:208-233`) and its fixed retry are untouched.
