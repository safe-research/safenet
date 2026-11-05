# F3-CORE-001 Watcher failures escalate to one eth_getLogs per event every 100 ms with no backoff, so a rate-limited RPC can stall the indexer for good

| Field | Value |
| --- | --- |
| Status | Final: Accepted |
| Crate and module | core, index/events.rs (with driver.rs) |
| Location | crates/core/src/index/events.rs:453-464, 474-484, 504-531 (related: events.rs:378-382, 654-667; crates/core/src/driver.rs:26-28, 208-227) |
| Severity | Medium / Medium |
| Certainty | 85% (CA3: reproduced again by both routes) |
| Assumptions involved | A4 |
| Tags | dos, liveness |

## Claim

Every watcher error except a too-deep reorg is retried by the driver after a flat 100 ms, with no backoff and no classification of rate-limit errors. The event watcher counts failed attempts and, after `block_single_query_retry_count` (3) failures for a new block, or after 7 failures that shrink a 100-block warp page to one block, switches to one `eth_getLogs` per watched event, issued concurrently: 16 requests per attempt for the validator and 11 for the sentinel. It never switches back for that block (the retry counter only grows) and the warp page size is reset only on success.

Under an RPC rate limit (A4 says the provider "may be rate limited"), this escalation works against recovery: each attempt needs 16 (or 11) requests to succeed together, and the 100 ms retry keeps the provider's budget drained. A token-bucket or sliding-window limit that refills fewer than 16 requests per retry cycle can then never satisfy an attempt, and the indexer stays on that block or warp page indefinitely. The process does not exit (only `ExceededMaxReorgDepth` ends the loop) and `/health` keeps returning `OK`, so neither a restart policy nor a liveness probe reacts. While it spins, the loop sends roughly 100 to 160 requests per second, against the handbook's sizing of 100,000 requests per day for the whole validator. The original TypeScript watcher backed off exponentially (1 s to 64 s) on HTTP 429 and `LimitExceeded` errors.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | After `block_single_query_retry_count` failures a new block is fetched with one query per event | E2 | crates/core/src/index/events.rs:453-464 | `let fetch = if retries < self.config.block_single_query_retry_count.get() {` … `} else {`<br>`Fetch::MultipleQueries(BlockFilter::Hash(block_hash))` |
| 2 | A failure keeps the block with `retries + 1`; nothing resets it, so the block stays on per-event queries | E2 | crates/core/src/index/events.rs:474-484 | `self.step = if result.is_ok() {`<br>`Step::Idle`<br>`} else {`<br>`Step::Block {` … `retries: retries + 1,` |
| 3 | Per-event mode issues one concurrent `eth_getLogs` per topic and fails the whole attempt if any one fails (no fallible events by default) | E2 | crates/core/src/index/events.rs:504-511, 529-530 | `futures::future::try_join_all(self.topics.iter().map(\|topic\| async move {` … `let logs = self.provider.get_logs(&filter).await?;` … `}))`<br>`.await?` |
| 4 | Warp pages halve on failure (100, 50, 25, 13, 7, 4, 2, 1) and a one-block page uses per-event queries | E2 | crates/core/src/index/events.rs:378-382, 656-660 | `let fetch = if page_size.get() > 1 {`<br>`Fetch::SingleQuery(blocks)`<br>`} else {`<br>`Fetch::MultipleQueries(blocks)`; `let page_size = NonZeroU64::new(page_size.get().div_ceil(2))` |
| 5 | The page size is reset only after a successful page | E2 | crates/core/src/index/events.rs:416-431 | `self.step = if result.is_ok() {` … `page_size: self.config.block_page_size,` … `} else {`<br>`narrowed_warp(from_block, to_block, page_size)` |
| 6 | Every watcher error except a too-deep reorg is retried after a flat 100 ms; the service never exits for it | E2 | crates/core/src/driver.rs:28, 211-224 | `const STEP_RETRY_DELAY: Duration = Duration::from_millis(100);`; `Err(err) => {`<br>`tracing::warn!(` … `tokio::time::sleep(STEP_RETRY_DELAY).await;` |
| 7 | The provider adds no retry or backoff layer of its own | E2 | crates/core/src/provider/mod.rs:145-149 | `let client = ClientBuilder::default()`<br>`.layer(ObservabilityLayer)`<br>`.connect(url.as_str())` |
| 8 | The validator watches 16 events (Consensus 5, Coordinator 10, Oracle 1) | E2 | crates/validator/src/service/mod.rs:71-79; crates/validator/src/bindings.rs:112-146, 171-208, 242-243 | `Consensus(Consensus::ConsensusEvents),`<br>`Coordinator(Coordinator::CoordinatorEvents),`<br>`Oracle(Oracle::OracleEvents),` |
| 9 | The sentinel watches 11 events (SentinelOracle 10, Consensus 1) | E2 | crates/sentinel/src/bindings.rs:170-176, 25-50, 112-121 | `Oracle(oracle::SentinelOracle::SentinelOracleEvents),`<br>`Consensus(consensus::Consensus::ConsensusEvents),` |
| 10 | The escalation itself runs as described: existing tests `new_block_falls_back_to_multiple_queries_after_retries`, `warp_narrows_the_page_on_failure_and_resets_on_success` and `warp_queries_each_event_for_single_block_pages` pass at the audit commit | E1 | rust-audit/state/run3/logs/R1-index-tests.txt | `test index::events::tests::new_block_falls_back_to_multiple_queries_after_retries ... ok` |
| 11 | `/health` returns a constant `OK`, so a stalled indexer passes liveness probes | E2 | crates/core/src/observability/metrics.rs:10-11 | ``/// `/health`, which returns a plain `OK` for liveness probes.`` |
| 12 | The handbook sizes the RPC for a peak of 100,000 requests per day | I | docs/validator-handbook.md:25 | `a reliable Ethereum RPC node that can accommodate a peak of 100.000 requests per day` |
| 13 | The TypeScript watcher throttled every step and backed off 1 s to 64 s on 429, `LimitExceeded` and bloom mismatches | I | `git show e69fdc8^:validator/src/watcher/backoff.ts` (DEFAULT_CONFIG, `#isRateLimitError`), `git show e69fdc8^:validator/src/watcher/index.ts` (`#run`) | `backoffDelays: [1, 2, 4, 8, 16, 32, 64].map((seconds) => seconds * 1000),`; `await this.#backoff.throttled(() => this.#next());` |

## Trigger

1. A validator uses an RPC endpoint with a sustained per-second limit, enforced as a token bucket or sliding window (A4: "may be rate limited").
2. Any burst pushes three consecutive single-query attempts for one new block into the limit, for example the catch-up after a restart, where the block watcher, the event watcher and the transaction queue's per-block `eth_getProof` run back to back with no pacing. The three attempts are only about 100 ms apart, so a limit that is exhausted for 300 ms is enough. The block is now in per-event mode (Basis 1, 2).
3. From then on each attempt sends 16 concurrent `eth_getLogs` (Basis 3, 8). Between attempts the limiter refills about `L x (0.1 s + RTT)` requests, where `L` is its per-second rate, and the attempt consumes them. With `L x (0.1 s + RTT) < 16`, which holds for any limit below about 107 requests per second at a 50 ms round trip (160 per second at zero round trip), no attempt ever sees all 16 succeed.
4. The driver logs a warning and retries every 100 ms forever (Basis 6). The block watcher is never polled again (the event watcher must drain first), so no further block, event or action is processed. A restart recovers, since the event watcher state is in memory and the block is retried with a single query, but nothing triggers one (Basis 6, 11).

The warp path reaches the same state after 7 failures, which take about one second at 100 ms each (Basis 4, 5); warps run on every restart. The sentinel behaves the same with 11 requests per attempt.

While stuck, the loop sends about 16 requests per 0.1 to 0.15 s, so 100 to 160 per second, or 9 to 14 million per day. A plan sized by the handbook (Basis 12) would also hit any daily quota within minutes, after which every RPC call of the service fails.

## Considered and rejected

- The provider has a retry layer: no, only the observability layer (Basis 7).
- `fallible_events` limits the blast radius: it is empty by default (`crates/core/src/index/events.rs:112`) and dropping events is not acceptable for the Safenet events anyway.
- A fixed-window limiter with at least 16 requests per window recovers on the first attempt of a new window; the claim is limited to limits that refill fewer than 16 requests per retry cycle.
- This is not the settled engine HTTP timeout item and not one of the known items in `settled.md`.
- Pre-existing at run 2's commit `fe9e84c`: the same fallback (`git show fe9e84c:crates/core/src/index/events.rs`, `block()` at lines 362-384) and the same `STEP_RETRY_DELAY` (`git show fe9e84c:crates/core/src/driver.rs:28`).

## Remediation options

1. Back off on watcher errors: exponential delay with a cap (as the TypeScript watcher did), at least for HTTP 429 and JSON-RPC `-32005`, reset on success. Cheap and keeps the current structure.
2. Make the per-event fallback bounded: return to the single query after a success or after a number of per-event failures, and cap the concurrency of per-event queries (or run them sequentially), so one attempt never needs 16 simultaneous successes.
3. Expose the stall: count consecutive watcher failures and make `/health` fail (or the process exit) after a configurable period without progress.

Tests to add: a mocked provider that rejects more than N concurrent requests within a window, asserting that the watcher still makes progress; a test that per-event mode returns to a single query after recovery.

## Trail

- 2026-10-08 Reviewer R1: drafted, self-estimate 70% (escalation and retry behaviour certain from code and tests; the stall depends on the provider's limiter type and rate).

## Critic (C3-CORE, 2026-10-08)

| Row | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | `crates/core/src/index/events.rs:453-464` as quoted. |
| 2 | Supported | `events.rs:474-484`. The only other way out of `Step::Block` is `on_block_invalidated` (`events.rs:299-310`), reached only for JSON-RPC `-32001` (`crates/core/src/index/mod.rs:109-126, 135-142`), never for HTTP 429 or `-32005`. |
| 3 | Supported | `events.rs:505-531`. `try_join_all` returns the first error and drops the other futures, so one rejected request fails the whole attempt. |
| 4 | Supported | `events.rs:378-382, 656-660`: 100, 50, 25, 13, 7, 4, 2, 1 is seven failures. |
| 5 | Supported | `events.rs:416-431`. |
| 6 | Supported | `crates/core/src/driver.rs:28, 211-224`. |
| 7 | Supported | `crates/core/src/provider/mod.rs:145-149`. alloy-rpc-client 2.0.5 `ClientBuilder::connect` adds no retry layer (`builder.rs:135-141`, `connect_with` at `:186-195`). |
| 8 | Supported | 5 + 10 + 1 = 16 distinct events (`crates/validator/src/bindings.rs:113-146, 172-208, 243`). |
| 9 | Supported | 10 + 1 = 11 (`crates/sentinel/src/bindings.rs:25-50, 112-121, 170-176`). |
| 10 | Supported | `rust-audit/state/run3/logs/R1-index-tests.txt:56, 58, 60`. The test shows the fallback, not that it persists; persistence is row 2, from code. |
| 11 | Supported | The constant `OK` comes from `metrics-exporter-prometheus-0.18.3/src/exporter/http_listener.rs:141-145`. |
| 12 | Supported | `docs/validator-handbook.md:25`. |
| 13 | Supported | `git show e69fdc8^:validator/src/watcher/backoff.ts` lines 16 and 41-55. |

Finding verdict: Plausible. Severity: Medium to Medium. An honest validator or sentinel stops processing until someone restarts it; the trigger is a provider property that A4 allows ("may be rate limited"), not attacker input, so not High. Certainty: 60, band 40 to 69 (mechanism E2, the fallback itself E1 through the existing test; the stall needs a provider limit that nobody measured). Duplicate: canonical. Related to F3-CORE-002 (same fixed retry schedule in `block()`), not a duplicate.

Confirmation and limits. The mechanism holds as written: once a block or a one-block warp page is in per-event mode, an attempt succeeds only if all 16 (sentinel: 11) concurrent `eth_getLogs` succeed within that attempt, and only success returns the step to a single query. Two precisions for QA and the practicality check:

- Entry. Under a pure token bucket each single-query retry needs one token, so three failures in a row need about 0.3 s plus three round trips with no token at all. That happens with an exhausted fixed window or quota, a short outage (a 502, a reset or a DNS failure lasting about half a second), or a burst. Restarts produce bursts: a failing warp page narrows to one block after 7 failures, and on the validator every warped block emits `ReconcileGroupSecrets` (`crates/validator/src/state/secrets.rs:78-84`), whose ready resumes can win the unbiased `select!` in `next_input` (`crates/core/src/driver.rs:229-232`) and drop the in-flight watcher future, so the same page query is sent again (R2 observation O1; the cancel safety itself is a known item).
- Stall. Per-event mode never completes when the provider cannot admit 16 requests within one retry cycle: a fixed window below 16, a token bucket whose burst or refill per cycle is below 16 (each failing attempt drains what refilled), or a cap on concurrent requests below 16. A fixed window of 16 or more recovers at the next window. No Gnosis provider limit was checked (no live RPC), which is why this stays Plausible.

QA recipe. In a scratch copy of `crates/core`, add a test-only constructor to `provider/mod.rs` that wraps a custom transport: a tower service over `RequestPacket` that answers `eth_getLogs` with an empty array for the first 10 requests of each 150 ms window and with an HTTP 429 error after that. Drive `EventWatcher::next` for one `BlockUpdate::New`, with tokio time paused and a 100 ms sleep between calls as `next_input` does. Expected: three failed single queries, then no `Ok` in 1,000 attempts, each sending 16 requests. Control: the same limiter with `block_single_query_retry_count = 1000` succeeds on a later window. Bite check: returning to a single query after a failed per-event attempt removes the stall.

## QA (QA3-CORE, 2026-10-08)

Result: Reproduced.

Route: the real `Driver` (real 100 ms `STEP_RETRY_DELAY`, real time) follows a 1 s chain through an RPC mocked at the transport boundary only: an in-process tower service behind `Provider`, a 20 ms round trip, HTTP errors built the way the reqwest transport builds them for a 429 (`alloy-transport-http-2.0.5/src/reqwest_transport.rs:63-65`). The service watches 16 events, as the validator does. Each run opens with a 0.5 s HTTP 502 outage on `eth_getLogs`, which takes the queued new block through its three single queries into per-event mode; then a provider limit applies. For 30 s, under a fixed window of 10 requests per 150 ms, a cap of 8 requests in flight, a token bucket with a burst of 12 (100 per second), or a token bucket with a burst of 20 (50 per second) that the first per-event attempt drained, no block's logs are processed (no snapshot at all) while the chain moves from 1000 to 1030, and the driver sends 122 to 123 `eth_getLogs` per second. The restart path stalls the same way: a warp from block 800 halves to single-block pages after 7 failures and stays on block 800 for the whole run, at 120 requests per second. Controls: the same window with `block_single_query_retry_count = 1000`, and the burst-20 bucket entered with its tokens refilled, both follow the chain at about 1 request per second. A 3 s outage gives the same results.

Command and output: `poc/F3-CORE-001/` (`README.md`; `driver-test-at-361aaf3.txt`, `driver-test-at-361aaf3-outage3s.txt`).

Bite check: remediation 2 in its smallest form, back to the single query after a failed per-event attempt (`retries % (count + 1)` in `block()`), removes the stall on the new-block path for all four limits under both outages (processed to the head at about 1 request per second). The warp variant still stalls, because single-block warp pages use per-event queries unconditionally (`crates/core/src/index/events.rs:378-382`), so the remediation must cover `warp` as well. The other 151 core tests pass with it. Remediation 1 alone (backoff from 100 ms doubling to 6.4 s, reset on success) cuts the rate to about 3 requests per second and lets the drained burst-20 bucket recover, but the window, in-flight and burst-12 limits still stall once per-event mode is entered (3 s outage), because every attempt still needs 16 requests admitted at once. Verdict: remediation 2 is the fix, at both call sites; remediation 1 is worth adding for the request rate but is not enough alone. Remediation 3 (a health signal) not tried.

Certainty: 80 (mechanism and stall E1 through the real driver; no Gnosis provider limit was measured, and `/health` staying `OK` during the stall comes from code, not from this run).

## QA second reproduction (QA3-2B, 2026-10-08)

Result: Reproduced. Route: Real `sentinel` binary behind a loopback JSON-RPC proxy in front of Anvil (chain 100, 5 s blocks), a different route from the Critic's mocked-transport unit test. The proxy serves logs during warmup, then a short eth_getLogs blackout forces three single-query failures on a new block so the event watcher escalates to one eth_getLogs per watched event (11 for the sentinel), after which the proxy caps concurrent eth_getLogs at 4 (< 11, a limit that would serve a single query but never 11 at once). The indexer sticks at block 11 while the chain reaches 20, firing ~90 eth_getLogs/s in 11-wide per-event bursts with a flat 100 ms retry, 518 "failed to get next blockchain update; retrying after delay" warns, `/health` constant 200 OK, process alive. Command and output: `poc/F3-CORE-001/repro-2/` (`core001.py`, `h.py`, `README.md`). `python3 -I core001.py <binary> <label>`. Bite check: remediation option 2 (run the per-event queries sequentially instead of concurrently) in `crates/core/src/index/events.rs`; re-run shows the indexer recovers after the blackout (seen advances 10 -> 21 -> 23 tracking head, no >=8-wide bursts, no stall). Sound. Certainty: 82 (E1 stall on the real binary; Critic rated Plausible, and the specific Gnosis provider concurrency/refill limit below 11 remains an unmeasured provider property).

## Practicality (PC3, 2026-10-09)

| Field | Value |
| --- | --- |
| Actor | Nature plus the operator's choice of RPC plan: any short provider failure or burst (three failed single queries about 100 ms apart, or seven failed warp pages, about one second) followed by a limit that cannot admit 16 concurrent `eth_getLogs` (11 for the sentinel) every retry cycle. No outside party has to target the node; a shared endpoint latches every node on it at once |
| Cost | n/a |
| Condition | The plan admits fewer than about 133 `eth_getLogs` per second (16 per 120 ms) or caps concurrency below 16. Per the researcher consult, no published free or public Gnosis tier reaches that: Alchemy Free about 5 `eth_getLogs`/s (300 CU/s at 60 CU), QuickNode trial 15 RPS, Chainstack Developer 25 RPS, Ankr about 30 req/s, BlockPI Public about 4/s, dRPC free 100/s at best and 42/s at worst; `rpc.gnosischain.com` and PublicNode publish no limit. Paid tiers that do (Alchemy PAYG 166/s, QuickNode Scale, Chainstack Growth, BlockPI Elementary) and self-hosted nodes do not stall |
| Gate | Gates 1 and 2 pass (real `Driver` with four limiter shapes, QA3-CORE; real sentinel binary behind a proxy capping concurrent `eth_getLogs` at 4, QA3-2B; both stall for good with `/health` OK). Gate 3: no attacker needed; the trigger is a provider property A4 allows, and the amplifier is that a transient failure latches into a permanent stall. Gate 4: the node stops processing until someone restarts it, and while stuck it sends about 120 `eth_getLogs` per second, which empties a handbook-sized daily quota (100,000 requests, `docs/validator-handbook.md:25`) in under 15 minutes |
| Practicality | conditional (an RPC plan below about 133 `eth_getLogs`/s, which covers every published free or public Gnosis tier, plus any transient failure; self-hosted nodes and large paid plans are not affected) |
| Recommended severity | Medium (reviewer Medium, Critic Medium) |
| Intended route and parameters | Real `Driver` and the real sentinel binary; only the provider's limiter is modelled, which is the trigger itself |
| Cheapest settling change | None needed for the band. If the team wants a provider-specific number: run QA3-2B's `core001.py` with the proxy set to Alchemy Free's published shape (300 CU/s over a 10 s bucket, 60 CU per call) |

Reason. Medium under Section 8: incorrect behaviour under a reachable provider condition, with recoverable impact (a restart). It is the common case for any operator who follows the handbook's sizing ("a peak of 100.000 requests per day") with a free or entry plan, and nothing signals it: the process stays up and `/health` says OK. The correlated case matters most for Aegis: if three of the five validators share one rate-limited endpoint and it hiccups, all three latch at once and attestations stop until operators restart them; that still needs no attacker, so the band stays Medium, but it is the reason to prefer remediation 2 (sequential or bounded per-event queries in both `block()` and `warp`) over backoff alone. Run 1's `F-CORE-034` and run 2's `F2-CORE-005` (both Medium) cover the flat retry; this row adds the escalation that makes the stall permanent.

Sources (researcher consult, read 2026-10-09): https://www.alchemy.com/docs/reference/pricing-plans, https://www.alchemy.com/docs/reference/compute-unit-costs, https://www.quicknode.com/pricing, https://docs.chainstack.com/docs/rps-plan-limits, https://www.ankr.com/docs/rpc-service/service-plans/, https://docs.blockpi.io/pricing/pricing-and-rate-limit, https://docs.drpc.org/howitworks/ratelimiting, https://docs.drpc.org/pricing/compute-units, https://docs.gnosischain.com/tools/RPC%20Providers/.

## Confidence (CA3, 2026-10-09)

| Field | Value |
| --- | --- |
| Result | Reproduced. Final: Accepted, Medium, certainty 85 |
| Reproductions | QA3-CORE's driver test (`driver-rate-limit-test.patch`): 1 of 1; variants A, C, D, E never process a block after the outage and F stays on block 800, each at 117 to 119 `eth_getLogs` per second; controls B and G follow the chain at about 1 per second. QA3-2B's real-binary route: 1 of 1 (seen block stuck at 11 while the chain moves, `/health` OK, 11-wide per-event bursts, process alive) |
| Route and parameters | Real `Driver` and real sentinel binary; only the provider's limiter is modelled, which is the trigger |
| Final severity | Medium, agree with PC3 |
| Certainty | 85, up from 80: two routes E1 again, and PC3's consult places every published free tier below the threshold. Untested: how a real provider's limiter treats 16 concurrent requests every 120 ms (burst allowances could admit some) |
| Evidence | `poc/F3-CORE-001/ca3/driver-test.txt`, `poc/F3-CORE-001/ca3/core001-ca3.txt` |

## Reconciliation (run 3)

| Field | Value |
| --- | --- |
| Relation | EXTENDS `F-CORE-034` (every watcher error retried at a flat 100 ms, no backoff). Cross-references: `F-CORE-004` (a failing event watcher starves the block watcher; run 2's `F2-CORE-005` is folded there) and run 2's `F2-CORE-010` (the per-event fan-out, folded into `F-CORE-034`) |
| Canonical | `F-CORE-034`; this file is its run-3 counterpart and holds the only execution of the permanent stall |
| Ledger values | Medium, certainty 85 (this file's CA3 value; basis E1 through this file) |
| What it adds | Under a limiter that admits fewer than 16 (sentinel 11) concurrent `eth_getLogs` per retry cycle, one transient failure latches the watcher into per-event mode for good while `/health` answers OK. Present at `fe9e84c` |
| Source | Ledger: [`report/RECONCILIATION.md`](../report/RECONCILIATION.md) sections 2 and 3 |
