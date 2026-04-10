# Feature Proposal: Fix IPFS deep links via hash-based routing
Component: `explorer`

---

## Overview

Direct deep links into the explorer fail when accessed through the IPFS/ENS gateway (`explorer.safenet-beta.eth.limo`). The gateway tries to resolve the URL path as a file inside the IPFS content tree, but because the explorer is a SPA, only `index.html` exists — no file named `safeTx`, `safe`, `epoch`, etc.

Fix: switch TanStack Router from History API (path-based) routing to hash-based routing. With hash routing the path is encoded in the URL fragment (`/#/safeTx?...`), which is never sent to the gateway, so `index.html` is always served and the React app handles routing client-side.

This is a single, self-contained change (one PR).

---

## Architecture Decision

Replace `basepath: __BASE_PATH__` with `history: createHashHistory()` in the router constructor. TanStack Router's `<Link>` components and `navigate` helpers automatically generate hash URLs when hash history is active — no other code changes are needed.

The `__BASE_PATH__` build-time variable is no longer consumed by the router, but Vite still uses `base:` for asset paths, so the Vite config is left unchanged.

### Alternatives Considered

- **`_redirects` / `404.html` trick**: Redirect all 404s to `index.html` via a special file in the build output. Not supported by IPFS gateways in a standard way; depends on gateway implementation.
- **Static pre-rendering**: Pre-render each route to its own `index.html`. Significant build complexity; routes with dynamic params (e.g. `safeTx?chainId=...&safeTxHash=...`) cannot be exhaustively pre-rendered.
- **Hash routing (chosen)**: Minimal change, zero gateway dependency, universally supported.

---

## User Flow

No new pages or interactions are introduced. All existing user flows remain identical; only the URL shape changes:

| Before | After |
|---|---|
| `/safeTx?chainId=1&safeTxHash=0x...` | `/#/safeTx?chainId=1&safeTxHash=0x...` |
| `/safe?chainId=1&safeAddress=0x...` | `/#/safe?chainId=1&safeAddress=0x...` |
| `/epoch` | `/#/epoch` |
| `/settings` | `/#/settings` |

---

## Tech Specs

- **Changed import**: add `createHashHistory` from `@tanstack/react-router`.
- **Router config**: replace `basepath: __BASE_PATH__` with `history: createHashHistory()`.
- **`__BASE_PATH__`**: still defined by Vite for asset path resolution; no longer passed to the router.
- **External links**: any deep links from external sites (e.g. `safe.dev`) that use the old path-based format will need to be updated to the hash format.

---

## Implementation Phases

### Phase 1 — Switch to hash routing (this PR)

Files touched:
- `explorer/src/main.tsx` — swap `basepath` for `history: createHashHistory()`
- `features/2026_04_10_fix_ipfs_deep_links.md` — this spec

---

## Open Questions / Assumptions

- External sites linking into the explorer with path-based URLs will break. Assumed acceptable given those links already break on IPFS.
- The `safe.dev` deployment is a regular web server. The root URL (`safe.dev/safenet/`) will continue to work. However, existing path-based deep links (e.g. `safe.dev/safenet/safeTx?...`) will load `index.html` correctly but the router (now in hash mode) ignores the URL path and renders the root route. If backward compat is needed, `safe.dev` should handle it server-side with a redirect from `/safenet/<path>` → `/safenet/#/<path>`.
