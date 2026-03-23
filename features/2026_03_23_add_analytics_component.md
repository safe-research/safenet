# Feature Proposal: Add Analytics Component to Explorer
Component: `explorer`

---

## Overview

Add a stub `Analytics` component to the explorer that serves as the designated integration point for analytics tooling. The component is rendered once in the root layout so it is present on every page. It intentionally does nothing in this repository — forks that need analytics replace this single file with their own implementation (e.g. Google Analytics, Plausible, Mixpanel).

A `README.md` for the explorer workspace is added at the same time to document the integration point and general workspace usage.

This is a single-phase change delivered in one PR.

---

## Architecture Decision

The component is placed in `explorer/src/components/Analytics.tsx` and imported in `explorer/src/routes/__root.tsx` (the TanStack Router root layout). It is rendered first in the layout fragment so it is treated as infrastructure rather than UI, consistent with how analytics scripts are typically loaded before page content.

The component exports a single default function that returns `null`. No props, no context, no side effects in the base implementation. This keeps the contract minimal: forks only need to drop in a replacement file.

### Alternatives Considered

**Environment-variable-driven lazy loading** — conditionally loading an analytics script via `VITE_ANALYTICS_*` env vars. Rejected because it requires the base repo to anticipate specific analytics providers, which is the opposite of the goal.

**Context/plugin slot** — exposing an analytics context that consumers inject into. More flexible but significantly more complex for what amounts to a one-file swap.

---

## User Flow

No user-visible change. The component renders nothing and has no UI.

---

## Tech Specs

### New file

`explorer/src/components/Analytics.tsx` — stub component, returns `null`.

### Modified file

`explorer/src/routes/__root.tsx` — adds `<Analytics />` as the first child of the root layout fragment.

### New documentation

`explorer/README.md` — workspace-level README covering setup, development, build, environment variables, and analytics integration.

---

## Implementation Phases

### Phase 1 (this PR)

- Create `explorer/src/components/Analytics.tsx`
- Update `explorer/src/routes/__root.tsx`
- Create `explorer/README.md`
- Create this feature spec

All changes are in the `explorer` workspace and are trivially small, so a single PR is appropriate.

---

## Open Questions / Assumptions

- Forks are expected to fork the entire repository and replace `Analytics.tsx` directly. No dynamic import or plugin mechanism is provided.
- Route-change tracking (SPA navigation events) is left entirely to the fork's implementation; the component is re-rendered on every route change via the root layout, so implementations that call `analytics.page()` in a `useEffect` will work without any additional wiring.
