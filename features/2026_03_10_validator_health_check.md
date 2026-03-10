# Feature Proposal: Validator Health Check Endpoint
Component: `validator`

---

## Overview

Add a `/health` route to the existing metrics HTTP server (`METRICS_PORT`, default `3555`). The endpoint provides a simple liveness signal for infrastructure tooling (container orchestrators, load balancers, monitoring scripts) without requiring a Prometheus client to interpret. Detailed observability — including readiness signals such as block progress — is already available via `/metrics` and does not need to be duplicated here.

---

## Architecture Decision

### Why not just use `/metrics`?

Metrics require a Prometheus scraper or client library to interpret. A `/health` endpoint can be polled by Kubernetes `livenessProbe`, Docker's `HEALTHCHECK` directive, an uptime monitor, or a simple `curl` in a runbook — with no tooling setup.

### Why liveness only, not readiness?

Readiness signals (e.g. whether the block watcher is advancing) are already expressed precisely by existing Prometheus metrics such as `validator_block_number`. Consumers that need readiness information can scrape `/metrics` directly. Duplicating that logic in the health endpoint adds complexity for no additional capability.

### Why on the same server as metrics?

The metrics HTTP server already handles per-path routing. Adding `/health` there requires no new port, no new env var, no new service, and no additional wiring in `validator.ts`. The two endpoints serve different consumers but there is no operational requirement to expose them independently — operators who need to restrict access to `/metrics` can do so at the proxy/ingress layer.

### Alternatives Considered

- **Separate `HealthService` on its own port** — Provides independent exposure control but adds a new service, a new `HEALTH_PORT` env var, and lifecycle wiring for a single route that returns a static response. Rejected as disproportionate complexity.
- **Readiness check via block staleness** — Block progress is already visible in `validator_block_number` on `/metrics`. A parallel implementation in the health endpoint would duplicate state tracking. Rejected in favour of keeping `/metrics` as the single source of truth for readiness.

---

## User Flow

Health check is not a human-facing UI flow. The primary consumers are:

**Kubernetes probes (example):**
```yaml
livenessProbe:
  httpGet:
    path: /health
    port: 3555
  initialDelaySeconds: 5
  periodSeconds: 10
```

**Docker HEALTHCHECK (example):**
```dockerfile
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
  CMD curl -sf http://localhost:3555/health || exit 1
```

**Operator runbook check:**
```sh
curl -s http://validator:3555/health | jq .
# { "status": "ok" }
```

---

## Tech Specs

### Endpoint

| Property | Value |
|---|---|
| Path | `/health` |
| Port | Same as `METRICS_PORT` (default `3555`) |
| Method | `GET` |
| Content-Type | `application/json` |
| Auth | None |

### Response

```json
{ "status": "ok" }
```

Returns `200` unconditionally while the HTTP server is accepting connections. No new environment variables.

### Test cases

| Test | File |
|---|---|
| `GET /health` returns `200` with `{ "status": "ok" }` | `utils/metrics/metrics.test.ts` |
| `GET /metrics` continues to return Prometheus output unaffected | `utils/metrics/metrics.test.ts` |
| `GET /unknown` returns `404` | `utils/metrics/metrics.test.ts` |

---

## Implementation

**What this covers:**
- `/health` route added to the existing metrics HTTP server.
- Returns `{ "status": "ok" }` with `200` while the server is accepting connections.

**Files touched:**
- `validator/src/utils/metrics/index.ts` — add `/health` branch to the request handler
- `validator/src/utils/metrics/metrics.test.ts` — new test cases for the health route

**No new env vars. No new files. No changes to any other files.**

---

## Open Questions / Assumptions

None.
