# F-CORE-011 The shared provider is built with no timeout, retry or rate-limit layer, so a stalled RPC connection stalls indexing indefinitely with no error, no metric and `/health` still `OK`

| Field | Value |
| --- | --- |
| Status | Draft (Critic-promoted) |
| Crate and module | core, `provider/mod.rs` (consumed by `index/blocks.rs` and `index/events.rs`) |
| Location | `crates/core/src/provider/mod.rs:127-137` (related: `crates/core/src/provider/mod.rs:66-117`; `crates/core/src/index/blocks.rs:393-416`; `crates/core/src/index/events.rs:402-469`; `crates/core/src/driver.rs:206-231`) |
| Severity | Medium / Medium |
| Certainty | 60% |
| Assumptions involved | A4, A6 |
| Tags | dos, config, input-validation |

## Claim

`Provider::connect` builds the JSON-RPC client with exactly one `tower` layer — the observability layer that records metrics and `trace`-level payloads. There is **no timeout layer, no retry layer and no rate limiter anywhere in the crate**. Every RPC call made by the indexer — `eth_getBlockByNumber` in the block watcher's poll loop, `eth_getLogs` in all three fetch strategies — therefore inherits whatever the underlying HTTP client's defaults are, and `core` sets none.

Two consequences, both on the indexing path, both distinct from the shutdown consequence already filed as F-CORE-039:

1. **A stalled connection stalls indexing silently and indefinitely.** A request that is accepted and never answered (a black-holed TCP session, a captive proxy, an overloaded or rate-limiting node — all explicitly in scope under A4) leaves the watcher parked inside an `await` that never resolves. Nothing above it has a deadline: `Driver::next_input`'s retry loop only runs _after_ an error is returned, and `Watcher::next` has no timeout of its own. Because the future never completes, the observability layer never records a result either — the `metrics::rpc_requests_total(&method, result)` increment happens after `inner.call(...).await` returns (`provider/mod.rs:90-114`) — so even the failure counter stays flat. `/health` keeps answering plain `OK` (`observability/metrics.rs:9-10`). The service is indistinguishable from an idle one.
2. **No backoff or jitter anywhere between the socket and the driver.** The only retry policy in the stack is `Driver::next_input`'s flat `STEP_RETRY_DELAY` sleep, which retries _everything_ at a fixed cadence. A rate-limited provider answering `429` therefore gets a steady 10 requests/second for as long as the condition lasts, which is the shape most likely to keep the rate limit engaged — and, per F-CORE-002, three of those `429`s are enough to strip the `use_client_filtering` integrity check off the next attempt.

This is the indexing half of lead **CORE-H12**. R2 filed the shutdown half as F-CORE-039 and stated there, explicitly, that "the other half — that a hung request also stalls _indexing_ — sits in R1's `provider`/`index` scope" and did not file it. R1 recorded the same citation in its coverage log (§8, "CORE-H12 (no RPC timeouts) not filed by me") and left it for the Critic. This file closes that gap so the lead is not lost between the two scopes.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The client is built with the observability layer and nothing else — no timeout, retry or rate-limit layer. | E2 | `crates/core/src/provider/mod.rs:127-137` | <pre>impl Provider {<br> /// Connects to `url`.<br> pub async fn connect(url: &Url) -> Result<Self, TransportError> {<br> let client = ClientBuilder::default<br> .layer(ObservabilityLayer)<br> .connect(url.as_str)<br> .await?;<br> let root = RootProvider::new(client);<br> let chain_id = root.get_chain_id.await?;<br> Ok(Self { root, chain_id })<br> }</pre> |
| 2 | No timeout is configured anywhere else either. | E2 | whole-crate grep | `grep -rn -i "timeout" crates/core/src/` returns exactly four lines and **none** is in `provider/`, `index/` or `driver.rs`: `effects.rs:160` (a `time::timeout` inside a `#[cfg(test)]` assertion) and `utils.rs:41,54,58` (sqlx pool `idle_timeout(None)`, which bounds a database connection, not an RPC call). The only RPC-adjacent timeout in the workspace is the sentinel's _engine-client_ timeout (`crates/sentinel/src/main.rs:50-62`), which bounds HTTP calls to the sentinel engine, not to the chain. |
| 3 | The metric that would reveal a hung call is only recorded once the call returns. | E2 | `crates/core/src/provider/mod.rs:90-114` | <pre>let response_packet = inner.call(request_packet).await;<br>…<br>for (_, (method, result)) in statuses {<br> metrics::rpc_requests_total(&method, result).increment(1);<br>}</pre> |
| 4 | The indexer awaits these calls with no deadline of its own. | E2 | `crates/core/src/index/blocks.rs:393-397` and `crates/core/src/index/events.rs:409` | <pre>let block = loop {<br> self.wait_for_pending_block.await;<br> if let Some(block) = self.get_block(BlockId::number(self.pending.number)).await? {<br> break block;<br> }</pre><br><pre>let logs = self.provider.get_logs(&filter).await?;</pre> |
| 5 | The only retry policy is a flat 100 ms driver sleep applied to every error class. | E2 | `crates/core/src/driver.rs:216-223` | <pre>Err(err) => {<br> tracing::warn!(<br> ?err,<br> "failed to get next blockchain update; retrying after delay"<br> );<br> tokio::time::sleep(STEP_RETRY_DELAY).await;<br>}</pre> |
| 6 | `/health` is a static liveness probe with no link to indexing progress. | E2 | `crates/core/src/observability/metrics.rs:9-10` | <pre>/// The listener serves Prometheus-formatted metrics on every path except<br>/// `/health`, which returns a plain `OK` for liveness probes.</pre> |
| 7 | Whether an un-timed request can hang indefinitely depends on the HTTP client's default, which cannot be read here. | I | `rust-audit/state/baseline.md` §1 | "Dependency sources are also absent … No dependency source (`frost-core`, `frost-secp256k1`, `alloy`, `sqlx`, `k256`, `sha2`, `hkdf`, `reqwest`) can be read in this checkout." |

## Trigger

1. A validator or sentinel is indexing normally against an endpoint behind a load balancer or proxy — the ordinary shape of a commercial RPC provider, and A4 explicitly admits a stale or rate-limited one.
2. The connection carrying the next `eth_getBlockByNumber` or `eth_getLogs` is accepted and then black-holed (an idle-connection reaper on the far side, a proxy holding the request open, a node that stops responding mid-request). No FIN, no RST, no HTTP status.
3. The watcher's `await` never resolves. No error is produced, so `Driver::next_input`'s retry branch is never entered; no response is produced, so `rpc_requests_total` is never incremented for that call; no block update is produced, so `safenet_core_block_number{status="processed"}` freezes.
4. `/health` answers `OK`. The process is alive, the log is silent, and the only visible symptom is a metric that has stopped moving — which a rate-based alert reads as "no chain activity", not "the indexer is wedged". The stall lasts until the operating system's TCP keepalive or the HTTP client's own default gives up, if either does.

Secondary, no stall required: a provider returning `429` under load gets retried at a fixed 10 Hz by `Driver::next_input` with no backoff and no jitter, which sustains the condition it is reacting to and, with `use_client_filtering` enabled, spends the three-attempt integrity budget of F-CORE-002 in 300 ms.

## Considered and rejected

- **"F-CORE-039 already covers this."** It covers the _shutdown_ consequence — that the uncancellable `update` window has no upper bound — and says so in its own overlap note: "this is CORE-H12's shutdown half. The other half — that a hung request also stalls _indexing_ — sits in R1's `provider`/`index` scope, and the fix (a timeout layer) is the same." Its claim, trigger and tests are all about `Driver::run`'s grace period; none of them mentions the indexing path, the frozen block metric or the flat-cadence retry. **The two findings share one remediation (a timeout layer in `Provider::connect`) and should be fixed together, but neither states the other's consequence.** F-CORE-039 should stay canonical for the shutdown half.
- **"The `alloy` default already bounds it."** Unknowable in this checkout (basis 7). This is the finding's one `I` component and it is load-bearing for consequence 1 only — consequence 2, the absence of any backoff, is `E2` and holds regardless. Note also that a default the crate does not set is a default the crate does not control: it can change with a dependency bump.
- **"A timeout would just turn a slow node into a retry storm."** True, which is why the remediation pairs a timeout with backoff rather than adding one alone.
- **"The driver's retry loop already handles RPC failure."** It handles failures that are _returned_. The gap is the case where nothing is returned, which no code in the crate can observe.
- **Not a false positive because** `Provider::connect` is eight lines, quoted in full above, and the layer stack is visibly a single element.

## Remediation options

1. **Add a request timeout to `Provider::connect`**, e.g. a `tower` timeout layer beneath `ObservabilityLayer` (so timed-out calls are still counted as failures by the metric), sized in the low tens of seconds and made configurable in the `[rpc]`/`[index]` table. Single change; also closes F-CORE-039. Tradeoff: too tight a value converts a slow node into a retry storm.
2. **Add a retry/backoff layer** (`RetryBackoffLayer` or equivalent) with exponential backoff and jitter for the transport-level classes (`429`, 5xx, connection errors), so the driver's flat `STEP_RETRY_DELAY` is not the only policy. This also stops F-CORE-002's integrity budget being spent by rate limiting.
3. **Export a liveness signal that a wedged indexer breaks.** A `safenet_core_last_update_timestamp` gauge, or making `/health` fail when no update has been processed for _k_ block times, turns a silent stall into a restartable one. Cheapest of the three and the only one that helps for failures nobody anticipated.
4. Bound the calls that are _not_ on the retry path at all — the startup scan in `BlockWatcher::initialize` (F-CORE-007) runs before `Driver::run` and would hang the same way.

Tests to add: a `provider` test using the mocked transport with a response that never arrives, asserting `get_block` returns `Err` within the configured timeout. No code is committed.

## Trail

- Critic C-CORE-A: **drafted by the Critic.** Lead CORE-H12 was assigned to R2 by `codebase-map.md` §9 but its mechanism lives in R1's `provider/mod.rs`. R2 filed the shutdown half as F-CORE-039 and explicitly excluded the indexing half; R1 recorded the citation in its coverage log §8 and asked for it to be promoted if R2 did not file it. I read F-CORE-039 in full and confirm it does not cover this consequence. Mechanism `E2`; the "hangs indefinitely" premise is `I` under A6 (no `reqwest`/`alloy` source on disk). Self-assessed **Plausible, 60%**, Medium.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 1, and it closes F-CORE-039 as well.**

Option 1 (a `tower` timeout layer beneath `ObservabilityLayer`) is sound, and the placement detail is right and easy to get wrong: _beneath_ the observability layer, so a timed-out call is still counted as a failure by the existing metric. Its tradeoff (too tight a value turns a slow node into a retry storm) is real and is precisely why it must land with **F-CORE-034 option 1** (exponential backoff with jitter and a ceiling), not before it. As shipped, the driver retries at a flat 100 ms forever.

Option 2 (a retry/backoff layer for `429`/5xx/connection errors) is sound and has a payoff the finding notes and I want to underline: it stops **F-CORE-002**'s integrity budget being consumed by rate limiting, which is that finding's cheaper trigger. Transport-layer backoff and F-CORE-002 option 3 (separate the budgets) attack the same problem from two sides; either helps, both is right.

Option 3 (a `last_update_timestamp` gauge, or `/health` failing after _k_ block times) is the cheapest of the three and the only one that helps for stalls nobody anticipated. Same consolidated health signal as F-CORE-004/-007/-030/-035.

Option 4 (bound the startup scan, which runs before `Driver::run` and is on no retry path) is a real gap and should not be dropped: a timeout layer on the provider covers it automatically, which is a further argument for option 1 over option 3 alone.

**Certainty note, not a change:** whether an un-timed request hangs _indefinitely_ rests on `reqwest`'s default, which is not on disk (A6). That is question 4 in `rust-audit/poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md`; answering it is what would move this finding out of 60%.
