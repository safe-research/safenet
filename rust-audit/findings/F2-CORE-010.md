# F2-CORE-010 The per-topic fallback multiplies request volume (x17 validator, x11 sentinel) exactly when the provider is failing, with a fixed 100 ms retry cadence and no retention of partial progress

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | safenet-core, index/events.rs |
| Location | crates/core/src/index/events.rs:369-380 and 412-440 (related: crates/core/src/driver.rs:28 and 218-224) |
| Severity | Low / Low |
| Certainty | 70% (Critic C2-CORE-A; reviewer self-estimate 55%) |
| Assumptions involved | A4 |
| Tags | dos |

Audited commit: `3ec8bc5`.

## Claim

After three failed single queries for a new block — on a public provider the usual cause is rate limiting or a brief outage — the watcher switches to `MultipleQueries`: `try_join_all` over every watched `topic0`, i.e. 17 concurrent `eth_getLogs` for the validator (5 `Consensus` + 11 `FROSTCoordinator` + 1 oracle event) and 11 for the sentinel (10 `SentinelOracle` + 1 `Consensus`), and the driver re-issues the whole batch every 100 ms until all succeed. `try_join_all` fails on the first error and discards the successful topics, so no progress is retained between attempts. Against a rate-limited provider this converts a transient 429 into a self-sustaining storm (up to ~170 requests/s for the validator versus the handbook's provisioning figure of 100,000 requests per day, about 1.2/s). The same fallback applies to single-block warp pages.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Fallback after the retry budget | E2 | crates/core/src/index/events.rs:369, 378-380 | `let fetch = if retries < self.config.block_single_query_retry_count.get() {` ... `} else {` / `Fetch::MultipleQueries(BlockFilter::Hash(block_hash))` / `};` |
| 2 | One concurrent query per topic; any failure fails all | E2 | crates/core/src/index/events.rs:412-421, 437-439 | `Fetch::MultipleQueries(blocks) => {` / `futures::future::try_join_all(self.topics.iter().map(\|topic\| async move {` ... `.event_signature(*topic);` / `let logs = self.provider.get_logs(&filter).await?;` ... `.await?` / `.concat()` |
| 3 | Single-block warp pages use the same fallback | E2 | crates/core/src/index/events.rs:318-322 | `let fetch = if page_size.get() > 1 {` / `Fetch::SingleQuery(blocks)` / `} else {` / `Fetch::MultipleQueries(blocks)` / `};` |
| 4 | Fixed 100 ms retry, no backoff | E2 | crates/core/src/driver.rs:28, 223 | `const STEP_RETRY_DELAY: Duration = Duration::from_millis(100);` ... `tokio::time::sleep(STEP_RETRY_DELAY).await;` |
| 5 | Topic counts (one selector per `event` declaration) | E2 | crates/validator/src/bindings.rs:113-146, 172-209, 245; crates/sentinel/src/bindings.rs:25-48, 110 | `event EpochProposed(` ... `event ValidatorStakerSet(address indexed validator, address staker);` ... `event KeyGen(` ... `event SignCompleted(` ... `event OracleResult(`; sentinel: `event NewRequest(` ... `event Claimed(` ... `event TransactionProposed(` |
| 6 | Handbook provisioning figure | E2 | docs/validator-handbook.md:25 | `you need a reliable Ethereum RPC node that can accommodate a peak of 100.000 requests per day.` |
| 7 | No batching layer is configured on the provider | I | crates/core/src/provider/mod.rs:129-134 | `let client = ClientBuilder::default()` / `.layer(ObservabilityLayer)` / `.connect(url.as_str())` — whether alloy coalesces concurrent calls into a batch was not verified |

## Trigger

Provider returns 429 (or times out) for at least three consecutive attempts on a new block, then keeps rate limiting: each 100 ms the validator issues 17 requests, most of which are rejected, restarting the cycle.

## Considered and rejected

- _The fallback is needed for size-capped nodes._ Yes; the concern is only its interaction with failure-driven retries.
- _Rate limiting is the operator's problem._ Partly; but the client's reaction to it makes recovery less likely.

## Remediation options

1. Exponential backoff with a ceiling in `Driver::next_input` (R2 seam).
2. Retain per-topic successes across attempts (retry only failed topics), or issue the per-topic queries sequentially in fallback mode.
3. Distinguish "response too large" errors (which justify the fallback) from rate-limit/transport errors (which do not).

Tests to add: mock 429s and assert the request count per unit time stays bounded.

## Trail

- Reviewer R1: drafted, self-estimate 55%. Seam: driver retry policy (R2).

## Critic (C2-CORE-A)

Independent recount of the topic lists from the `sol!` declarations behind each `watcher_events!` enum: validator `Event` (`crates/validator/src/service/mod.rs:71-79`) = `Consensus` 5 (`crates/validator/src/bindings.rs:113-146`) + `Coordinator` 11 (`172-209`) + `Oracle` 1 (`245`) = **17**; sentinel `SentinelEvents` (`crates/sentinel/src/bindings.rs:167-172`) = `SentinelOracle` 10 (`25-48`) + `Consensus` 1 (`110`) = **11**. No overloads, one selector per declaration, so `E::topics()` has exactly those lengths. Arithmetic: one attempt = 17 (11) concurrent `eth_getLogs` via `try_join_all` (`crates/core/src/index/events.rs:413-438`); the driver re-issues after 100 ms; per-attempt period = 100 ms + slowest round trip, so ~170 requests/s for the validator is the zero-latency upper bound (~110/s at 50 ms). `try_join_all` returns the first error and drops the remaining futures, so no partial progress is kept. Matches the reviewer.

Per-claim verdicts: 1-6 **Supported** (claim 6 at `docs/validator-handbook.md:25`, verbatim). Claim 7 upgraded from `I` to E2: `alloy-rpc-client` 2.0.5 (`Cargo.lock:428-429`) exposes batching only through the explicit `RpcClient::new_batch` (`~/.cargo/registry/src/index.crates.io-*/alloy-rpc-client-2.0.5/src/client.rs:172-175`); nothing coalesces concurrent calls, and `crates/core/src/provider/mod.rs:129-134` adds no such layer.

Verdict: **Confirmed**. Certainty **70** (E2). Severity **Low / Low**: self-inflicted load against the operator's own provider under failure (A4 rate limiting); it lowers the chance of recovery, but the process stays up and recovers when the provider does. Remediation 2 (retain per-topic successes) is the most targeted; remediation 1 belongs with F2-CORE-005's escalation (R2 seam). Distinct from F2-CORE-002 and F2-CORE-005. A16/A17 not applicable.

## Anchors at fe9e84c (Manager)

Anchors in `crates/sentinel/src/bindings.rs` moved with the `origin/main` merge (`fe9e84c`): 25–48 → 26–50 (the cited `NewRequest` event gained a `uint24 daoFeeShare` field at line 31), 167–172 → 169–174. Topic count and the ×11 sentinel multiplication are unchanged (`state/run2/baseline-delta.md` §3).

## Reconciliation (run 2)

**Final: EXTENDS `F-CORE-034` (canonical) — folded; combined Medium, 80 (E2).** Run 1's `F-CORE-034` (fixed 100 ms retry forever: storm, log flood, no escalation, handbook contradiction) already covers the policy; this file quantifies the storm (×17 / ×11, `try_join_all`, no batching layer) and is carried inside it. Also touches `F-CORE-011`'s "no backoff" horn (`state/run2/reconciliation/core.md` §1).
