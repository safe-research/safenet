# Feature Proposal: Plausible Hash-Based Routing
Component: `explorer`

---

## Overview

Enable hash-based routing support in the Plausible Analytics integration so that pageviews are correctly tracked when the URL hash changes. The explorer uses hash-based routing; without this setting, Plausible only records one pageview per full-page load and misses subsequent in-app navigations.

Single-phase change: pass `hashBasedRouting: true` to `init()` in `Analytics.tsx`.

---

## Architecture Decision

The `@plausible-analytics/tracker` package exposes a `hashBasedRouting` flag on its `PlausibleConfig` interface. Setting it to `true` instructs the tracker to fire a pageview event on every `hashchange` event in addition to the initial load, matching Plausible's recommended configuration for hash-router SPAs (https://plausible.io/docs/hash-based-routing).

No new components, environment variables, or dependencies are required.

### Alternatives Considered

- **Manual `track()` calls in each route component** — more brittle, requires changes whenever routes are added, and duplicates logic the library already handles.

---

## Tech Specs

- **File changed**: `explorer/src/components/Analytics.tsx`
- **Config option added**: `hashBasedRouting: true` in the `init()` call
- **Test cases**: existing Vitest unit tests in `Analytics.test.tsx` have been updated to verify that `hashBasedRouting: true` is correctly passed to the Plausible `init()` call.

---

## Implementation Phases

### Phase 1 — Single PR
- Add `hashBasedRouting: true` to the `init()` call in `Analytics.tsx`.

---

## Open Questions / Assumptions

- Assumes the explorer will continue to use hash-based routing. If it migrates to the History API, this flag should be removed.
