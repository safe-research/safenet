# F2-CORE-033 `/health` is an unconditional `OK`; a stalled driver (indefinite watcher retry, hung RPC) stays healthy

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | safenet-core, observability/metrics.rs (driver.rs) |
| Location | crates/core/src/observability/metrics.rs:6-16 (related: crates/core/src/driver.rs:208-227, 237-297; crates/core/src/provider/mod.rs:129-136; Cargo.lock reqwest 0.13.4 defaults) |
| Severity | Low / Low |
| Certainty | 80% (Critic C2-CORE-B; reviewer self-estimate in Trail) |
| Assumptions involved | A4 |
| Tags | dos |

Audited commit: `3ec8bc5`.

## Claim

The only health surface the runtime exposes is the Prometheus exporter's built-in `/health` route, which returns the literal `OK` whenever the HTTP listener is up. It is not connected to the driver in any way. Two stall modes leave the process alive and "healthy" while it makes no progress:

1. `Driver::next_input` retries every watcher error except `ExceededMaxReorgDepth` forever, every 100 ms, with a `warn!` log and no escalation. A deterministic watcher error (R1 is classifying which exist; e.g. undecodable logs from a watched address, an RPC that rejects `blockHash` filters) or a permanently unreachable RPC therefore stalls indexing indefinitely with `/health` = `OK`.
2. `Driver::update` awaits JSON-RPC calls (`update_block_status`, `queue` -> `submit_pending`) through an alloy HTTP transport built on `reqwest::Client::new()`, whose default has no request or read timeout. An RPC endpoint that accepts the connection but never answers hangs the driver at that await indefinitely, again with `/health` = `OK`.

The only progress indicator is the `safenet_core_block_number{status="processed"}` gauge, which requires a Prometheus rule to alert on. Liveness probes that use `/health` (the natural choice, since the doc comment advertises it "for liveness probes") will never restart a stalled service.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | `/health` is advertised for liveness and returns a plain `OK`; nothing in core feeds it. | E1 (test run this session) | crates/core/src/observability/metrics.rs:9-12, 76-78 | `/// The listener serves Prometheus-formatted metrics on every path except` `/// `/health`, which returns a plain `OK` for liveness probes.` ... test: `let (status, health) = http_get(addr, "/health").await;` `assert_eq!(status, StatusCode::OK);` `assert_eq!(health, "OK");` |
| 2 | The route is implemented inside the pinned exporter and is unconditional. | E2 | ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/metrics-exporter-prometheus-0.18.3/src/exporter/http_listener.rs:141-145 | `if req.uri().path() == "/health" {` `let mut response = Response::new("OK".into());` ... `return Ok(response);` |
| 3 | All watcher errors but one are retried forever at a fixed 100 ms. | E2 | crates/core/src/driver.rs:28, 210-226 | `const STEP_RETRY_DELAY: Duration = Duration::from_millis(100);` ... `Err(err) => { tracing::warn!(?err, "failed to get next blockchain update; retrying after delay"); tokio::time::sleep(STEP_RETRY_DELAY).await; }` |
| 4 | The provider is built without a timeout layer. | E2 | crates/core/src/provider/mod.rs:129-134 | `let client = ClientBuilder::default()` `.layer(ObservabilityLayer)` `.connect(url.as_str())` `.await?;` |
| 5 | The pinned alloy HTTP transport uses `reqwest::Client::new()`. | E2 | ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/alloy-transport-http-2.0.5/src/reqwest_transport.rs:28 | `Ok(BoxTransport::new(Http::with_client(Client::new(), self.url.clone())))` |
| 6 | The pinned reqwest default client has no request or read timeout. | E2 | ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/reqwest-0.13.4/src/async_impl/client.rs:299, 313-314 | `connect_timeout: None,` ... `read_timeout: None,` `timeout: None,` |
| 7 | `update` awaits those RPC calls inline on the driver task. | E2 | crates/core/src/driver.rs:249, 283 | `let result = self.transactions.update_block_status(block_status).await;` ... `let result = self.transactions.queue(transactions).await;` |

## Trigger

(a) Point the service at an RPC URL that resolves but refuses connections: the log fills with "failed to get next blockchain update; retrying after delay" at 10 Hz, `safenet_core_block_number` freezes, `curl :port/health` keeps returning `OK`. (b) Put a TCP sink (accepts, never responds) in front of the RPC: the first `eth_getTransactionCount`/`eth_sendRawTransaction` inside `update` never returns; the process is idle and `/health` is `OK`. Not run this session.

## Considered and rejected

- "The exporter's `/health` is for the exporter, not the service" - the core doc comment (row 1) advertises it as the service's liveness endpoint and no other health surface exists.
- "Process exit covers fatal cases" - yes (and see F2-CORE-031 for the exit status); this finding is about non-fatal stalls.
- "reqwest has a default timeout" - refuted at the pinned version (row 6); only the connection-pool idle timeout is set.

## Remediation options

1. Add a request timeout (alloy `ClientBuilder::layer` with a tower timeout, or `reqwest::Client::builder().timeout(..)`) so every RPC call fails within a bounded time; a failure in `update` is already handled as intermittent for RPC errors.
2. Make `/health` reflect progress: serve it from core (a tiny axum/hyper route beside the exporter, or the exporter's `with_http_listener` replaced by a handler that checks `now - last_processed_update < k * block_time`), returning 503 when stale. Tradeoff: needs a shared "last progress" timestamp updated by the driver.
3. Bound the retry loop: exponential backoff with a cap and, after N consecutive failures of the same deterministic error, exit (non-zero, per F2-CORE-031).

Tests to add: a `driver.rs` test with a mocked watcher that always errors, asserting the loop backs off / gives up per the chosen policy; an observability test asserting `/health` returns 503 when the progress timestamp is stale.

## Trail

- Reviewer R2: drafted, self-estimate 70% (E2; the stall modes are concrete, the impact depends on how operators probe). Severity Low: robustness/monitoring gap with no direct security impact; note that map lead CORE-H8 (deterministic errors) would make mode 1 reachable from chain data.
- Critic C2-CORE-B: Confirmed, 80%, severity Low (reviewer Low).

## Critic (C2-CORE-B)

Method: read title and Location only, traced `observability/metrics.rs`, `driver.rs:208-227` and `provider/mod.rs` myself, then re-opened every citation including the three registry sources at the `Cargo.lock` pins (`metrics-exporter-prometheus 0.18.3`, `alloy-transport-http 2.0.5`, `reqwest 0.13.4`).

| # | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | Doc at observability/metrics.rs:9-12; test at 76-78 (present; executed by R2, not re-run by me). |
| 2 | Supported | `metrics-exporter-prometheus-0.18.3/src/exporter/http_listener.rs:141-145`: `if req.uri().path() == "/health" { let mut response = Response::new("OK".into()); ... return Ok(response); }` — unconditional. |
| 3 | Supported | driver.rs:28 and 218-224. |
| 4 | Supported | provider/mod.rs:130-134: `ClientBuilder::default().layer(ObservabilityLayer).connect(url.as_str())`; no timeout layer anywhere in the file. |
| 5 | Supported | `alloy-transport-http-2.0.5/src/reqwest_transport.rs:28`: `Http::with_client(Client::new(), self.url.clone())`. |
| 6 | Supported, with a nuance | `reqwest-0.13.4/src/async_impl/client.rs:299` `connect_timeout: None`, 313-314 `read_timeout: None,` `timeout: None,`. The same defaults block also sets `tcp_keepalive: Some(15 s)`, `tcp_keepalive_retries: Some(3)` and, on Linux, `tcp_user_timeout: Some(30 s)` (303-307): a peer that stops acknowledging is torn down within roughly a minute and surfaces as an RPC error. The indefinite hang is real only for a peer that keeps the TCP session alive and never answers — which is exactly the reviewer's trigger (b), so the claim stands with that qualification. |
| 7 | Supported | driver.rs:249 and 283. |

Finding verdict: **Confirmed**. Certainty **80%** (row 1 is E1 via the existing test; the rest E2 with a concrete trigger). Severity **Low / Low** — a monitoring gap; no security impact by itself.

Overlaps: `F2-VAL-066` (C2-VAL-B) bundles the `/health` half with the exit status (F2-CORE-031). `F2-SEN-008` covers the sentinel's own `reqwest::Client::new()` toward the engine (sentinel `engine.rs:113`); that path is bounded by a per-request `.timeout(..)` (`engine.rs:155-158`), unlike the RPC path here. Remediation 1 (a request timeout on the provider) also fixes F2-CORE-034.

## Reconciliation (run 2)

**Final: CONFIRMS and narrows `F-CORE-011` (canonical) — combined Low, 80 (E2).** This file settled run 1's open dependency question from the pinned sources (`reqwest-0.13.4`: no request/read timeout, but `tcp_user_timeout` 30 s on Linux), which turns `F-CORE-011`'s Medium 60 Plausible into Low 80 Confirmed for the live-but-silent-peer case. The `/health` half restates `F-CORE-030`; the retry-forever mode is `F-CORE-004` / `F-CORE-034` (`state/run2/reconciliation/core.md` §1.1).
