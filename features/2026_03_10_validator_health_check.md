# Feature Proposal: Validator Health Check Endpoint
Component: `validator`

---

## Overview

Add a `/health` HTTP endpoint to the validator on a dedicated port, separate from the existing `/metrics` endpoint. Metrics and health serve different audiences and have different operational requirements — keeping them on separate servers lets each be independently exposed, secured, and consumed.

Two phases, each a standalone PR:

1. **Liveness** — a new `HealthService` exposes `/health` on `HEALTH_PORT` (default `3556`), returning `200 OK` when the server is accepting connections.
2. **Readiness** — `/health` additionally checks whether the block watcher is advancing, returning `503` if it has stalled beyond a configurable threshold.

---

## Architecture Decision

### Why a dedicated endpoint, not just metrics?

Metrics require a Prometheus scraper or client library to interpret. A `/health` endpoint can be polled by Kubernetes `livenessProbe`/`readinessProbe`, Docker's `HEALTHCHECK` directive, an uptime monitor, or a simple `curl` in a runbook — with no tooling setup. The two concerns are complementary but serve different audiences.

### Why a separate server from metrics?

Metrics (`METRICS_PORT`) are typically internal-only: scraped by Prometheus within the same cluster, not exposed to the outside world. Health checks often need to be reachable externally — by a load balancer, an uptime monitor, or a container orchestrator that sits outside the internal network.

Sharing a port forces an all-or-nothing exposure decision. A dedicated `HEALTH_PORT` lets operators:
- Keep metrics private behind a firewall rule or network policy.
- Expose health publicly for infrastructure probes.
- Disable either independently (metrics-only mode, or health-only mode).
- Apply different access controls without hacks (path-based routing, auth middleware, etc.).

The implementation follows the same pattern as `MetricsService`: a minimal Node.js `http` server, started and stopped alongside the validator process, wired in `validator.ts`.

### Why not check RPC, peer connectivity, or FROST ceremony state?

A health check should reflect the validator's own process health, not its dependencies. RPC connectivity failures are already visible in `validator_rpc_requests{result="failure"}` metrics and will eventually surface as a stalled block number (Phase 2). Checking dependencies in the health endpoint risks false-positive degraded states and complicates the endpoint's semantics.

### Phase 1 — Liveness: new HealthService

A new `HealthService` (`validator/src/utils/health/index.ts`) mirrors the structure of `MetricsService`: it creates an HTTP server, handles `GET /health`, and exposes `start()` / `stop()` lifecycle methods. The server returns `{ "status": "ok" }` with `200` unconditionally — if the process is up and the event loop is healthy, the server responds.

This phase establishes the full infrastructure contract (port, env var, JSON shape, HTTP status codes, graceful shutdown) at near-zero complexity. The response is intentionally static so there is no risk of the health endpoint itself becoming unhealthy due to logic errors.

### Phase 2 — Readiness: block progress tracking

The state machine in `service/machine.ts` calls `metrics.blockNumber.set(...)` in the `finally` block of every transition. Block transitions are driven by `BlockWatcher`, which fires on every new chain block regardless of Safe transaction activity. A stalled block update therefore reliably indicates the validator has lost contact with the chain.

`HealthService` gains a `reportBlock(blockNumber: bigint)` method. The state machine calls it alongside the existing metrics update — the two services stay independent; neither knows about the other. `HealthService` stores:
- `lastBlockAt: Date | null` — wall-clock time of the last `reportBlock()` call.
- `lastBlockNumber: bigint | null` — the value passed in.

The `/health` handler compares `Date.now() - lastBlockAt` against `HEALTH_STALE_THRESHOLD_MS` (default `30_000` ms — roughly six Gnosis Chain blocks). Prometheus gauges don't expose a last-written timestamp, which is why `HealthService` tracks this independently rather than reading from the metrics registry.

The consensus layer operates on Gnosis Chain (~5 s block time), so the default is tuned to that cadence. 30 s allows a small buffer for transient network hiccups (6 missed blocks) while still detecting genuine connectivity loss promptly. Staking lives on Ethereum, but the block watcher tracking consensus progress is the more meaningful health signal here — Ethereum staking state is not time-sensitive in the same way.

Response shapes:

| State | HTTP | Body |
|---|---|---|
| Running, blocks current | `200` | `{ "status": "ok", "blockNumber": "12345", "blockAge": "3s" }` |
| Running, stale blocks | `503` | `{ "status": "degraded", "reason": "no new block for 75s", "blockNumber": "12300" }` |
| Service starting (no block yet) | `503` | `{ "status": "starting" }` |

### Alternatives Considered

- **Add `/health` to the existing metrics server** — Avoids a new port and service, but forces metrics and health to be exposed together. Rejected in favour of separate servers to give operators independent control over exposure and access.
- **Read staleness from the Prometheus registry** — `prom-client` gauges don't expose a last-written timestamp, so we'd have to store it separately regardless. The `reportBlock()` method on `HealthService` is the same complexity with a cleaner, self-contained API.
- **Check block number against chain head** — Requires an RPC call on every health probe, adding latency and an external dependency to the health path. The wall-clock staleness heuristic avoids this entirely.
- **Track last state machine transition instead of last block** — Transitions only fire when there is onchain activity relevant to the validator. During quiet periods (no pending Safe transactions) the machine may not transition for many minutes even on a healthy validator, causing false-positive degraded states. Block events are a better heartbeat signal.

---

## User Flow

Health check is not a human-facing UI flow. The primary consumers are:

**Kubernetes probes (example):**
```yaml
livenessProbe:
  httpGet:
    path: /health
    port: 3556
  initialDelaySeconds: 5
  periodSeconds: 10

readinessProbe:
  httpGet:
    path: /health
    port: 3556
  initialDelaySeconds: 10
  periodSeconds: 15
  failureThreshold: 3
```

**Docker HEALTHCHECK (example):**
```dockerfile
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
  CMD curl -sf http://localhost:3556/health || exit 1
```

**Operator runbook check:**
```sh
curl -s http://validator:3556/health | jq .
# { "status": "ok", "blockNumber": "22941088", "blockAge": "4s" }
```

**Firewall / network policy example (separate exposure):**
```
# Internal only
METRICS_PORT=3555  →  accessible within cluster

# Externally reachable
HEALTH_PORT=3556   →  exposed via load balancer health check target
```

---

## Tech Specs

### Endpoints

| Property | Metrics | Health |
|---|---|---|
| Path | `/metrics` | `/health` |
| Port env var | `METRICS_PORT` (default `3555`) | `HEALTH_PORT` (default `3556`) |
| Method | `GET` | `GET` |
| Content-Type | `text/plain` (Prometheus) | `application/json` |
| Auth | None | None |
| Audience | Prometheus scrapers | Infrastructure tooling |

### New environment variables

| Variable | Type | Default | Phase | Description |
|---|---|---|---|---|
| `HEALTH_PORT` | `number` | `3556` | 1 | Port for the health check HTTP server. |
| `HEALTH_STALE_THRESHOLD_MS` | `number` (ms) | `30000` | 2 | Max ms since last block update before `/health` returns `503 degraded`. Defaulted to 30 s (~6 Gnosis Chain blocks). |

Both added to `validatorConfigSchema` in `validator/src/types/schemas.ts` as optional fields, following the same `portSchema` pattern as `METRICS_PORT`.

### HealthService API

```typescript
// validator/src/utils/health/index.ts

interface HealthServiceOptions {
  logger: Logger;
  port: number;  // use 0 for OS-assigned port (tests only)
  staleThresholdMs?: number;  // Phase 2 only; ignored in Phase 1
}

function createHealthService(options: HealthServiceOptions): HealthService;

interface HealthService {
  start(): Promise<void>;
  stop(): Promise<void>;
  port: number;  // resolved port after start(); useful when port 0 is used in tests
  reportBlock(blockNumber: bigint): void;  // Phase 2 only
}
```

Port `0` delegates port selection to the OS, matching the same pattern used by `MetricsService`. This is only intended for test environments to avoid port conflicts; production deployments always set an explicit `HEALTH_PORT`.

### Response schema

```typescript
type HealthResponse =
  | { status: "ok" }                                            // Phase 1
  | { status: "ok"; blockNumber: string; blockAge: string }     // Phase 2
  | { status: "degraded"; reason: string; blockNumber: string } // Phase 2
  | { status: "starting" };                                     // Phase 2
```

`blockNumber` is serialised as a `string` (via `.toString()` on the `bigint`) to avoid precision loss — JSON numbers are IEEE 754 doubles, which cannot represent all 256-bit block numbers exactly.

### Wiring in validator.ts (Phase 1)

`HealthService` is created, started, and stopped alongside `MetricsService`:

```typescript
const health = createHealthService({ logger, port: validatorConfig.HEALTH_PORT });

Promise.all([service.start(), metrics.start(), health.start()]).catch(...);

// on SIGTERM/SIGINT:
await Promise.all([service.stop(), metrics.stop(), health.stop()]);
```

### Wiring in machine.ts (Phase 2)

The state machine already calls `metrics.blockNumber.set(Number(blockNumber))` in its `finally` block. Phase 2 adds a parallel call to `health.reportBlock(blockNumber)` in the same location. The two services remain independent — neither is aware of the other.

### Test cases

| Test | File | Phase |
|---|---|---|
| `GET /health` returns `200` with `{ status: "ok" }` when server is running | `utils/health/health.test.ts` | 1 |
| `GET /health` returns `404` for unknown paths | `utils/health/health.test.ts` | 1 |
| Health server starts on the configured port | `utils/health/health.test.ts` | 1 |
| Health server stops cleanly on `stop()` | `utils/health/health.test.ts` | 1 |
| `GET /health` returns `503 starting` before any block is reported | `utils/health/health.test.ts` | 2 |
| `GET /health` returns `200 ok` with `blockNumber`/`blockAge` after `reportBlock()` | `utils/health/health.test.ts` | 2 |
| `GET /health` returns `503 degraded` when `lastBlockAt` exceeds threshold | `utils/health/health.test.ts` | 2 |
| `blockAge` formats correctly (e.g. `"3s"`, `"75s"`) | `utils/health/health.test.ts` | 2 |
| Metrics server is unaffected: `GET /metrics` still returns Prometheus output | `utils/metrics/metrics.test.ts` | 1 |

---

## Implementation Phases

### Phase 1 — Liveness endpoint

**What this covers:**
- New `HealthService` on a dedicated `HEALTH_PORT` (default `3556`).
- `GET /health` returns `{ "status": "ok" }` with `200` unconditionally.
- Service lifecycle (start/stop) wired into `validator.ts` alongside metrics.

**Files touched:**
- `validator/src/utils/health/index.ts` — new file: `HealthService` implementation
- `validator/src/types/schemas.ts` — add optional `HEALTH_PORT` field
- `validator/src/validator.ts` — create, start, and stop `HealthService`
- `validator/src/utils/health/health.test.ts` — new test file

**No changes to `MetricsService` or `machine.ts`.**

---

### Phase 2 — Readiness via block progress

**What this covers:**
- `reportBlock(blockNumber)` method on `HealthService` that records the wall-clock timestamp.
- `/health` returns `503 starting` until the first block is reported.
- `/health` returns `503 degraded` if the last block is older than `HEALTH_STALE_THRESHOLD_MS`.
- `/health` returns `200 ok` with `blockNumber` and `blockAge` when healthy.

**Files touched:**
- `validator/src/utils/health/index.ts` — add `reportBlock()`, internal state, staleness logic
- `validator/src/service/machine.ts` — call `health.reportBlock(blockNumber)` alongside existing `metrics.blockNumber.set()`
- `validator/src/types/schemas.ts` — add optional `HEALTH_STALE_THRESHOLD_MS` field
- `validator/src/validator.ts` — pass `HEALTH_STALE_THRESHOLD_MS` to `createHealthService`
- `validator/src/utils/health/health.test.ts` — Phase 2 test cases

**No changes to `MetricsService`.**

---

## Open Questions / Assumptions

None.
