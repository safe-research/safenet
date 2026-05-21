# Feature Proposal: DAppSpec RPC URL Param + Default RPC Update
Component: `explorer`

---

## Overview

Two small related changes to the explorer's RPC configuration:

1. Change the default RPC endpoint from `https://1rpc.io/gnosis` to `https://rpc.gnosischain.com/`.
2. Support the [DAppSpec](https://ethereum-magicians.org/t/new-erc-best-practices-for-dapps-dappspec/24407) `ds-rpc-{chainId}` URL query parameter so users and integrators can override the RPC endpoint via URL. For this Gnosis Chain explorer the relevant parameter is `ds-rpc-100`.

Both changes ship in a single PR.

---

## Architecture Decision

The `ds-rpc-100` value is read from `window.location.search` (the real query string, before the hash) using a new `getRpcUrlParam()` utility in `src/lib/settings.ts`. This is the correct location because the app uses hash-based routing (`createHashHistory`), so TanStack Router search params live inside the hash and cannot carry the DAppSpec params.

The URL param takes priority over the stored settings RPC in `useProvider`. The settings form disables the RPC field and shows an inline note when a URL param is active, so users are not confused by their saved value being silently ignored.

A phishing-risk warning modal is shown on app load whenever `ds-rpc-100` is detected, as recommended by the DAppSpec. The user must acknowledge it before continuing.

### Alternatives Considered

- **Store URL param dismissal in sessionStorage** — unnecessary complexity; dismissal state is ephemeral per page load and component-local `useState` is sufficient.
- **Block the app until acknowledged** — the modal already achieves this by sitting in a full-screen overlay.

---

## User Flow

### URL param present

1. User opens `https://explorer.example.com/?ds-rpc-100=https://my-rpc.example.com/#/`
2. Warning modal appears: explains a custom RPC was provided via URL and warns about phishing risk.
3. User clicks "I understand, continue" — modal closes, custom RPC is used.
4. On the Settings page the RPC field is greyed out with a note: _"Ignored — RPC provided via URL param: https://my-rpc.example.com/"_

### No URL param

- Default RPC is `https://rpc.gnosischain.com/` (or whatever was saved in localStorage).
- Settings RPC field is editable as before.

### Page Layout (warning modal)

```
┌─────────────────────────────────────┐
│  ⚠ Custom RPC Endpoint Detected     │
│                                     │
│  This page was opened with a custom │
│  RPC endpoint provided via URL      │
│  parameter:                         │
│                                     │
│  ┌─────────────────────────────┐    │
│  │ https://my-rpc.example.com/ │    │
│  └─────────────────────────────┘    │
│                                     │
│  Only open links from sources you   │
│  trust. A malicious RPC can return  │
│  false data about transactions and  │
│  balances.                          │
│                                     │
│  [ I understand, continue ]         │
└─────────────────────────────────────┘
```

---

## Tech Specs

### New files
- `src/components/common/RpcUrlParamWarning.tsx` — warning modal component

### Modified files
- `vite.config.js` — default RPC changed to `https://rpc.gnosischain.com/`
- `.env.sample` — comment updated
- `src/lib/settings.ts` — `getRpcUrlParam()` added
- `src/hooks/useProvider.tsx` — URL param priority
- `src/components/settings/ConsensusSettingsForm.tsx` — disabled RPC field + note
- `src/components/settings/ConsensusSettingsForm.test.tsx` — new test cases
- `src/routes/__root.tsx` — `<RpcUrlParamWarning />` rendered in root layout

### Query parameter
| Param | Value | Example |
|-------|-------|---------|
| `ds-rpc-100` | URL-encoded RPC endpoint URL | `?ds-rpc-100=https%3A%2F%2Frpc.ankr.com%2Fgnosis` |

Other `ds-rpc-*` params (e.g. `ds-rpc-1` for Mainnet) are ignored since this is a Gnosis Chain-only explorer.

---

## Implementation Phases

Single PR — all changes are small and tightly coupled.

---

## Open Questions / Assumptions

- We only read `ds-rpc-100` (Gnosis Chain). If the app ever supports multiple chains this function would need to be generalised.
- The warning modal is dismissed in component state only (no persistence). Each fresh page load with the param shows the modal once.
