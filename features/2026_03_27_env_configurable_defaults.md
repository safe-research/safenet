# Feature Proposal: Env-Configurable Explorer Default Settings
Component: `explorer`

---

## Overview

Make all `DEFAULT_SETTINGS` values in the explorer configurable at build time via `VITE_DEFAULT_*` environment variables. The existing hardcoded values remain as fallbacks when no env var is set. Also adds a `relayer` entry to `DEFAULT_SETTINGS` (empty by default) so operators can pre-configure a relayer URL via `VITE_DEFAULT_RELAYER`.

This is a single-PR change with no phases — the scope is limited to configuration plumbing and does not touch any user-facing UI.

---

## Architecture Decision

The explorer already uses Vite's `define` mechanism to inject build-time constants (e.g. `__DOCS_URL__`, `__BASE_PATH__`). This pattern is extended to the `DEFAULT_SETTINGS` constants:

- `vite.config.js` reads `VITE_DEFAULT_*` env vars via `loadEnv` and injects them as `__DEFAULT_*__` globals
- `vitest.config.ts` provides matching test values for the same globals
- `src/vite-env.d.ts` declares the constants for TypeScript
- `src/lib/settings.ts` uses the globals instead of hardcoded literals

`loadSettings` is updated to always merge stored user settings on top of `DEFAULT_SETTINGS` (rather than returning schema-default-filled objects). This ensures env-var defaults apply consistently for all users, including those with existing localStorage entries that predate a new field (e.g. `relayer`).

### Alternatives Considered

**`import.meta.env.VITE_*` directly in settings.ts** — This also works in Vite, but the existing codebase uses `define` globals for deployment-configurable constants. Following the existing pattern is more consistent and keeps the configuration surface in one place (vite.config.js).

---

## Tech Specs

### New environment variables

| Variable | Default | Description |
|---|---|---|
| `VITE_DEFAULT_CONSENSUS` | `0x223624cBF099e5a8f8cD5aF22aFa424a1d1acEE9` | Default consensus contract address |
| `VITE_DEFAULT_RPC` | `https://1rpc.io/gnosis` | Default RPC endpoint URL |
| `VITE_DEFAULT_DECODER` | `https://calldata.swiss-knife.xyz/decoder?calldata=` | Default calldata decoder URL |
| `VITE_DEFAULT_RELAYER` | *(empty)* | Default relayer URL; enables tx proposal submission when set |
| `VITE_DEFAULT_MAX_BLOCK_RANGE` | `10000` | Default max block range for log queries |
| `VITE_DEFAULT_VALIDATOR_INFO` | `https://raw.githubusercontent.com/…/validator-info.json` | Default validator info JSON URL |
| `VITE_DEFAULT_REFETCH_INTERVAL` | `10000` | Default UI refetch interval in ms (0 = disabled) |
| `VITE_DEFAULT_BLOCKS_PER_EPOCH` | `1440` | Default blocks per epoch |
| `VITE_DEFAULT_SIGNING_TIMEOUT` | `12` | Default signing timeout in blocks |

### Files changed

- `explorer/vite.config.js` — read env vars, inject as `__DEFAULT_*__` via `define`
- `explorer/vitest.config.ts` — add matching `define` entries with test values
- `explorer/src/vite-env.d.ts` — declare `__DEFAULT_*__` constants for TypeScript
- `explorer/src/lib/settings.ts` — use globals in `DEFAULT_SETTINGS`, add `relayer`, update `loadSettings`
- `explorer/.env.sample` — document all new variables

### Test cases

- `relayer` defaults to `undefined` when nothing is stored
- `relayer` is read from stored settings when present
- Stored settings override defaults; unset fields fall back to defaults
- Returns default settings when stored data is malformed JSON

---

## Implementation Phases

Single PR — all changes are contained in the explorer workspace configuration and settings module.

---

## Open Questions / Assumptions

- Users with existing localStorage settings will automatically receive env-var defaults for any fields they have not explicitly set (including the new `relayer` field), due to the `{ ...DEFAULT_SETTINGS, ...parsed }` merge in `loadSettings`. This is the desired behavior.
- Numeric env vars (`VITE_DEFAULT_MAX_BLOCK_RANGE` etc.) must be valid integers; invalid values silently fall back to the hardcoded default via `Number(val) || fallback`.
