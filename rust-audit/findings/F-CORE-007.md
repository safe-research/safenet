# F-CORE-007 A node that keeps disagreeing with itself during startup puts `BlockWatcher::initialize` in an unbounded, undelayed RPC loop that is invisible at the default log level while `/health` already answers OK

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | core, `index/blocks.rs` |
| Location | `crates/core/src/index/blocks.rs:291-327` (related: `244-246`; context: `crates/core/src/observability/mod.rs:29-36`, `crates/validator/src/main.rs:41-79`) |
| Severity | Low / Low |
| Certainty | 60% |
| Assumptions involved | A4 |
| Tags | dos, config, input-validation |

## Claim

`BlockWatcher::initialize` scans `safe..=latest` block by block and checks that each header's `parent_hash` matches the previous header's hash. When the check fails it discards everything and restarts the whole scan from `safe`. That restart has **no attempt limit, no delay and no backoff**: the loop simply re-issues `eth_getBlockByNumber` for every block in the window as fast as the endpoint will answer, indefinitely, for as long as the node keeps returning a mutually inconsistent range.

Every other retry loop in the crate at least sleeps — the block-poll loop uses `block_retry_delays`/`block_time` (`blocks.rs:392-416`), the driver sleeps 100 ms (`driver.rs:216-223`), the warp path halves its page size (`events.rs:337-348`). This one does not.

Two properties make the failure hard to notice:

- It happens inside `Driver::new`, i.e. _before_ `Driver::run`, so the service simply never starts. By that point `observability::init` has already installed the Prometheus listener (`crates/validator/src/main.rs:41`, `crates/sentinel/src/main.rs`), so `/health` answers `OK` for a process that has not begun indexing anything.
- The only trace it emits is `tracing::debug!`, and the default `log_filter` is `info` (`observability/mod.rs:29-36`), so at the shipped default the loop produces **no log output at all**. An operator sees a healthy process, no logs after "configuration loaded", and no metrics movement.

The amount of work per restart is `max_reorg_depth + 1` RPC round trips, and `max_reorg_depth` is an unvalidated `u64` (see F-CORE-009), so a large configured depth makes each restart proportionally more expensive and the window in which a fresh disagreement can appear proportionally wider — the loop becomes more likely to sustain itself the larger the window is.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | On a parent-hash mismatch the scan resets to the start of the range with no counter, no delay and no bound. | E2 | `crates/core/src/index/blocks.rs:311-327` | <pre>if parent_hash.is_none_or(&#124;hash&#124; hash == block.parent_hash) {<br> parent_hash = Some(block.hash);<br> self.recent.push_back(block);<br> number += 1;<br>} else {<br> // Reorg observed mid-init: discard and re-query the range. We<br> // will also need to re-fetch `latest`, as it may have been uncled.<br> tracing::debug!(<br> number,<br> "reorg observed during initialization, restarting range scan"<br> );<br> parent_hash = None;<br> canonical_latest = None;<br> self.recent.clear;<br> number = safe;<br>}</pre> |
| 2 | Each pass issues one `eth_getBlockByNumber` per block in `safe..=latest`, and after the first restart the cached `latest` is dropped so even that block is re-fetched. | E2 | `crates/core/src/index/blocks.rs:294-309` | <pre>let latest_number = latest.number;<br>let mut parent_hash = None;<br>let mut canonical_latest = Some(latest);<br>let mut number = safe;<br>while number <= latest_number {<br> // Avoid an additional RPC request for the latest block, but only if<br> // we know for sure it is still canonical.<br> let cached_block = if number == latest_number {<br> canonical_latest.take<br> } else {<br> None<br> };<br> let block = match cached_block {<br> Some(block) => block,<br> None => self.require_block(BlockId::number(number)).await?,<br> };</pre> |
| 3 | The window scanned is `max_reorg_depth + 1` blocks wide, and the depth is taken straight from configuration with no bound. | E2 | `crates/core/src/index/blocks.rs:244-246` | <pre>async fn initialize(&mut self, indexed: Option<BlockStatus>) -> Result<, Error> {<br> let latest = self.require_block(BlockId::latest).await?;<br> let safe = latest.number.saturating_sub(self.config.max_reorg_depth);</pre> |
| 4 | The default log level hides the only signal the loop emits. | E2 | `crates/core/src/observability/mod.rs:29-36` | <pre>impl Default for Config {<br> fn default -> Self {<br> Self {<br> log_filter: EnvFilter::new("info"),<br> metrics_address: SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),<br> }<br> }<br>}</pre> |
| 5 | Observability (and therefore `/health`) is installed before the driver is constructed, so a wedged `Driver::new` reports healthy. | E2 | `crates/validator/src/main.rs:41-45` and `71-79` | <pre>observability::init(config.observability)?;<br>metrics::initialize;<br>tracing::debug!(config_file = %options.config_file.display, "validator configuration loaded");<br><br>let provider = Provider::connect(&config.rpc).await?;</pre><br><pre>let mut driver = Driver::new(<br> service,<br> provider.clone,<br> config.signer,<br> pool,<br> watched,<br> config.driver,<br>)<br>.await?;</pre> |
| 6 | The existing test proves only the happy path: one mismatch, then a consistent range. Nothing exercises a repeating mismatch. | E2 | `crates/core/src/index/blocks.rs:839-879` | <pre>async fn handles_reorgs_during_initialization {<br> let asserter = Asserter::new;<br> asserter.push_success(&block_with(1000, &#124;header&#124; {<br> header.hash = keccak256("bad1000");<br> header.parent_hash = keccak256("bad999");<br> }));</pre> |

## Trigger

1. A validator or sentinel starts against a load-balanced RPC endpoint whose backends are briefly on different forks, or against a single node during a period of repeated shallow reorgs. Both are in scope under assumption A4 (stale / inconsistent, but not malicious, RPC).
2. `initialize` fetches `latest` (block `L`) from backend A and scans `L-5 ..= L`.
3. The request for `L-1` is routed to backend B, which is on a different fork, so its `parent_hash` does not chain to the header the scan already accepted for `L-2`. The scan clears `recent` and restarts at `L-5` (basis 1).
4. The next pass distributes its six requests across the same backends and hits the same disagreement. Because there is no delay, the passes run back to back at the endpoint's full service rate; because there is no counter, they run for as long as the disagreement lasts.
5. The process is stuck inside `Driver::new`. It has not started indexing, has queued no transactions, and emits no `info`-level log; the Prometheus listener installed in step 0 keeps answering `OK` on `/health` and reports `safenet_core_block_number` still at its initialised `0` (`crates/core/src/metrics.rs:81-90`). An orchestrator using an HTTP liveness probe sees a healthy pod; one using a startup probe on `/health` sees the same.

A second reachable trigger with no reorg at all: any node whose `eth_getBlockByNumber` answers are served from different snapshots of a fast-moving chain — for example a provider that fans requests across regions — can produce a mismatch on most passes during a period of high reorg activity.

## Considered and rejected

- **"The mismatch resolves after one restart in practice."** Usually, yes, which is why this is Low rather than Medium. The defect is that nothing _bounds_ it: a persistent disagreement produces a hot loop rather than an error, and the code has no way to distinguish "one reorg landed mid-scan" from "this endpoint is permanently inconsistent".
- **"`require_block` would error out."** Only if the node returns `null` for a block at or below the head (`Error::MissingBlock`, `blocks.rs:238-242`). A backend on a different fork returns a valid header for every height in the window, so the error path is not reached.
- **"`Driver::new`'s error path handles it."** There is no error path from this loop — it either completes or spins. Both binaries do propagate a `Driver::new` failure with `?` (`crates/validator/src/main.rs:79`), so an actual error would surface correctly; that is precisely what this loop never produces.
- **"SIGTERM will kill it."** It will: no shutdown handler is installed until `Driver::run`, so the default disposition applies and the process dies on the signal. That makes this a startup-liveness problem rather than an unkillable hang — but it still requires an operator to notice, and basis 4 and 5 are why they would not.
- **"The `debug!` line is enough to diagnose it."** Not at the shipped level (basis 4). Both service sample configs pin the level to the same value as the default — `crates/validator/validator.sample.toml:59` and `crates/sentinel/sentinel.sample.toml:43` both read `log_filter = "info"` — so an operator following the samples gets no output from this loop at all.
- **Not a false positive because** the absence of a counter, a sleep and an `info`-level log are all visible in the quoted block, and the ordering that makes `/health` lie is quoted from the binary.

## Remediation options

1. **Bound the restarts and fail with a distinct error.** Count restarts (a handful is generous — one real reorg mid-scan needs at most one) and return a new `Error::InconsistentChainDuringInit { attempts }` once exceeded. `Driver::new` already propagates errors to `main`, so this becomes a normal non-zero-exit startup failure. Tradeoff: a node in a genuinely reorg-heavy minute would now fail to start rather than eventually succeeding — mitigate with a generous bound plus option 2.
2. **Delay between restarts.** Sleep one `block_time` (or reuse `block_retry_delays`) before re-scanning, so a transient reorg has resolved by the next pass and the loop cannot saturate the endpoint. Cheap and independently worthwhile.
3. **Raise the log level and add a metric.** Log the restart at `warn` rather than `debug`, and expose a counter (for example `safenet_core_index_init_restarts_total`) so the condition is visible at the default configuration.
4. **Do not report healthy before the driver is running.** Make `/health` reflect driver state rather than process liveness. This is the same gap as the stalled-indexer case in F-CORE-004 and belongs to `observability/metrics.rs` (R2's file); noted here because it is what converts this loop from "slow start" into "silent non-start".
5. **Scan incrementally instead of restarting.** On a mismatch, only re-fetch from the mismatching height backwards until the chain links up, rather than discarding the whole window. Tradeoff: more code for a rare path; option 1 plus 2 is the better cost/benefit.

Tests to add:

- `blocks.rs`: push a repeating inconsistent range (the same mismatching triple three times) and assert `BlockWatcher::new` returns an error rather than consuming responses forever. The existing `handles_reorgs_during_initialization` (basis 6) can be extended, since the `Asserter` queue makes "how many passes happened" directly observable via `read_q`.

## Trail

- Reviewer R1: found while reading `initialize` line by line for lead CORE-H1; it is mentioned in passing in `rust-audit/analysis/analysis-core.md:138` ("persistent parent mismatch during init → unbounded re-scan loop") but was never raised as a hypothesis, so it is drafted here. All citations re-opened at commit `2893917`. Kept at Low: the trigger needs a _persistently_ inconsistent endpoint, and the process remains killable. Self-estimate 70% that the loop is unbounded as described (high confidence, it is plainly visible), 45% that a real endpoint sustains it long enough to matter. No `E1`: read-only run.

## Critic (C-CORE-A)

### Per-claim verdicts — all Supported

Re-derived from `blocks.rs:291-327`: the scan is `while number <= latest_number`, and the mismatch branch (`:315-326`) sets `parent_hash = None; canonical_latest = None; self.recent.clear; number = safe;` — no counter, no `sleep`, no backoff, no error return. The only output is `tracing::debug!(number, "reorg observed during initialization, restarting range scan")`, and the default `log_filter` is `EnvFilter::new("info")` (`observability/mod.rs:31-36`), so at shipped defaults the loop is **silent**. `observability::init` runs at `crates/validator/src/main.rs:41`, before `Driver::new`, so the Prometheus listener — and therefore `/health` returning plain `OK` (`observability/metrics.rs:9-10`) — is already up while `initialize` spins. Every element of the claim checks out.

I also confirm the reviewer's cost arithmetic: each pass is `max_reorg_depth + 1` sequential `eth_getBlockByNumber` calls (`blocks.rs:298-309`), minus at most one saved by `canonical_latest`, which the mismatch branch discards.

### Finding verdict

**Plausible** — the mechanism is certain (an unbounded, undelayed, invisible retry loop before the service starts), but the trigger requires a node or endpoint that keeps returning a _mutually inconsistent_ `safe..=latest` range for a sustained period. A single disagreement costs one extra pass and resolves; the finding needs the disagreement to persist, and that is asserted rather than shown. **Certainty 60%.** **Severity Low, unchanged.** No consensus impact, no attacker, and the failure is "the service does not start" rather than divergence. The part that deserves the team's attention is not the loop but the observability gap it exposes — a process that has not begun indexing answers `OK` on `/health` and logs nothing above `debug` — which is a general property of `Driver::new`, not of this loop alone, and is worth fixing once for all startup paths.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: options 1 and 2 together; option 3 unconditionally.**

Option 1 (bound the restarts, return a distinct error) is sound and integrates with an existing path — `Driver::new` propagates to `main`, so this becomes a normal startup failure. Its tradeoff is correctly identified and correctly mitigated by option 2.

Option 2 (sleep one `block_time` between restarts) is sound, independently worthwhile, and is the part that actually stops the RPC saturation; option 1 alone bounds the loop but still lets it run at full speed until the bound is hit.

Option 3 (log at `warn`, add a restart counter) is the change that makes the condition exist for an operator at all, and should land regardless.

Option 4 (`/health` should reflect driver state, not process liveness) is sound and is the same change F-CORE-004 option 3, F-CORE-011 option 3, F-CORE-030 option 3 and F-CORE-035 option 1 all ask for. **Five findings, one health/liveness signal.** The report should consolidate them into one recommendation rather than five, or it will read as five separate observability asks and be deferred five times.

Option 5 (incremental re-scan) is sound but is the wrong cost/benefit for a rare path; the finding says so and I agree.
