# Feature Proposal: Explorer Transaction List — Refresh Controls & Row Redesign
Component: `explorer`

---

## Overview

Improve the transaction list on the Explorer home page with two focused changes:

1. **Refresh controls bar** — a visible control strip above the list that lets users manually trigger an immediate refresh, toggle auto-refresh on/off, and see when the data was last updated.
2. **Transaction row redesign** — replace the current border-color status indicator and verbose layout with a compact card-list row showing a status badge, a network badge, a shortened SafeTxHash, and a block-age "time ago" column.

These two changes are independent and can be developed and reviewed as separate PRs.

---

## Architecture Decision

### Phase 1 — Refresh Controls Bar

The existing `useRecentTransactionProposals` hook wraps TanStack Query's `useQuery`. The query already exposes:
- `refetch()` — triggers an immediate re-fetch.
- `dataUpdatedAt` — Unix timestamp (ms) of the last successful fetch.
- `isFetching` — loading indicator.

The auto-refresh toggle will be **ephemeral React state** (not persisted in localStorage). When toggled off, the hook disables `refetchInterval` for the component's lifetime; on page reload it resets to `ON`. This avoids coupling the toggle to the settings object which is intended for more durable configuration.

The "Last updated" timestamp will be derived from `dataUpdatedAt` and formatted in the user's local time with timezone offset (via `Intl.DateTimeFormat`).

### Phase 2 — Transaction Row Redesign

The current `RecentTransactionProposal` component delegates rendering to `SafeTxOverview`, which uses a generic title+body layout. The redesign will introduce a dedicated `TransactionListRow` component built as a table row, replacing `SafeTxOverview` usage in the list context. `SafeTxOverview` is still used on the detail page and should remain untouched.

New shared utilities and components:
- **`formatHashShort(hash: Hex): string`** — returns `0x` + first 8 hex chars + `…` + last 8 hex chars (= first and last 4 bytes).
- **`formatBlockAge(blockDiff: bigint, chain: ChainInfo): string`** — converts a block-count difference to a human-readable age string using the chain's block time. Reads `chain.blockTime` (milliseconds, optional on viem's `Chain` type) and divides by 1000 to get seconds. Falls back to 12,000 ms if undefined.
- **`NetworkBadge`** component — renders the chain short name (e.g. `ETH`, `GNO`, `BASE`) as a styled badge. Text-only; no external icon library added in this phase (see Open Questions).
- **`StatusBadge`** component — renders `PROPOSED` or `ATTESTED` as a styled badge.

The list will stay card-based but switch to a denser, structured layout. Each item becomes a compact row card (div/flexbox) with labelled columns: Network / Status | Safe | SafeTxHash | To / Summary | Block | When. The existing `Box` card chrome is replaced with a lighter row style, keeping visual consistency with the rest of the explorer.

The current `refetchInterval` setting in localStorage continues to control the underlying poll interval; the new toggle acts as an override layered on top.

### Alternatives Considered

- **Persisting the auto-refresh toggle in localStorage** — rejected to keep settings focused on durable configuration; the toggle is a transient UX preference.
- **Querying the current block number to compute real elapsed time** — rejected in favour of static block time constants to avoid extra RPC calls per chain on every render cycle.
- **Using a chain-icon library (e.g. `@web3icons/react`)** for network badges — deferred to an open question; text-only badges are sufficient for the initial redesign.
- **Reusing `SafeTxOverview` in the list** — the new compact row layout is sufficiently different that a dedicated `TransactionListRow` component is cleaner than parameterising `SafeTxOverview` further.

---

## User Flow

### Refresh Controls

```
┌──────────────────────────────────────────────────────────────────────────┐
│  Recent proposals (all networks)                                          │
│  [Refresh now]   Auto-refresh: [ON ●]   Last updated: 12:41:08 (UTC+1)   │
└──────────────────────────────────────────────────────────────────────────┘
```

- **[Refresh now]** — button, always visible. Triggers `refetch()`. Disabled (with spinner) while `isFetching` is true.
- **Auto-refresh toggle** — toggles between `ON` and `OFF`. When `OFF`, `refetchInterval` is set to `false` (no polling). Starts as `ON`.
- **Last updated** — shows the local time of `dataUpdatedAt` with UTC offset. Shows `—` before the first successful fetch.

### Transaction List

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│  Network   │  Safe            │  SafeTxHash        │  To / Summary   │  When    │
│  Status    │                  │                    │                 │          │
├─────────────────────────────────────────────────────────────────────────────────┤
│  [ETH]     │  0xA1b2c3d4…     │  0x9f123456…       │  to 0xBEEF1234… │  34s ago │
│  [ATTEST.] │  9CDe0102        │  789a7aBc          │  value 0.12 ETH │          │
├─────────────────────────────────────────────────────────────────────────────────┤
│  [GNO]     │  0x1111abcd…     │  0x0aa1beef…       │  to 0xCAFEbabe… │  12m ago │
│  [PROPOS.] │  22225678        │  bb221234          │  data len 196   │          │
├─────────────────────────────────────────────────────────────────────────────────┤
│  [BASE]    │  0x7E57d00d…     │  0x1234cafe…       │  to 0xF00Df00d… │  2026-   │
│  [ATTEST.] │  f00d9abc        │  abcdef12          │  (no data)      │  02-08   │
└─────────────────────────────────────────────────────────────────────────────────┘
```

**"When" column logic (block-age display):**

| Estimated age | Display |
|---|---|
| < 60 s | `Xs ago` |
| 60 s – 59 m | `Xm ago` |
| ≥ 1 h | `YYYY-MM-DD` (proposal block date, best-effort from block time) |

Block difference is computed as `currentBlock - proposal.proposedAt.block`. The current block is taken from the consensus state (`ConsensusState.currentBlock`) which is already fetched by the app. Elapsed seconds = `blockDiff × chainBlockTime`.

---

## Tech Specs

### Block Times via Viem

Viem's `Chain` type includes an optional `blockTime?: number` field (in milliseconds). Of the five chains in `SAFE_SERVICE_CHAINS`, three already define it:

| Chain | Chain ID | `chain.blockTime` |
|---|---|---|
| Ethereum Mainnet | 1 | 12,000 ms ✓ (defined in viem) |
| Gnosis Chain | 100 | 5,000 ms ✓ (defined in viem) |
| Base | 8453 | undefined — add `blockTime: 2_000` manually |
| Sepolia | 11155111 | undefined — add `blockTime: 12_000` manually |
| Gnosis Chiado | 10200 | 5,000 ms ✓ (defined in viem) |

For chains without a viem-provided value, `blockTime` is set explicitly when constructing `SAFE_SERVICE_CHAINS` (spread into the chain object alongside `shortName`). No separate static lookup table is needed — `ChainInfo` already extends `Chain`, and `Chain.blockTime` is part of the standard type.

`formatBlockAge` reads `chain.blockTime / 1000` to get seconds. If `blockTime` is still undefined (defensive fallback), 12 s is used.

### Hash Shortening

`formatHashShort("0x9f123456789a7aBcdeadbeef12345678deadbeef12345678deadbeef12345678")`
→ `"0x9f123456…12345678"` (first 4 bytes = 8 hex chars, last 4 bytes = 8 hex chars)

### Components

| Component | File | Notes |
|---|---|---|
| `NetworkBadge` | `components/common/NetworkBadge.tsx` | New. Accepts `chainId: bigint`. |
| `StatusBadge` | `components/common/StatusBadge.tsx` | New. Accepts `attested: boolean`. |
| `TransactionListRow` | `components/transaction/TransactionListRow.tsx` | New. Replaces `RecentTransactionProposal`. |
| `TransactionListControls` | `components/transaction/TransactionListControls.tsx` | New. Refresh button, toggle, last-updated. |
| `RecentTransactionProposals` | `components/transaction/RecentTransactionProposals.tsx` | Modified to use compact card list + controls. |

### Hook Changes

`useRecentTransactionProposals` is extended to accept an `autoRefresh: boolean` parameter so the parent can pass the toggle state down. Alternatively, the hook can remain unchanged and the `refetchInterval` override is applied inside `RecentTransactionProposals` by calling `query.refetch()` on a manual interval — but the cleaner approach is to pass `autoRefresh` and let the hook decide the `refetchInterval`.

The hook will expose `dataUpdatedAt` from the underlying `useQuery` result.

### Test Cases

| Test | Location |
|---|---|
| `formatHashShort` — correct truncation for standard 32-byte hash | `lib/safe/formatting.test.ts` |
| `formatHashShort` — handles short/invalid input gracefully | `lib/safe/formatting.test.ts` |
| `formatBlockAge` — seconds range | `lib/safe/formatting.test.ts` |
| `formatBlockAge` — minutes range | `lib/safe/formatting.test.ts` |
| `formatBlockAge` — date fallback for old proposals | `lib/safe/formatting.test.ts` |
| `formatBlockAge` — undefined `blockTime` uses 12 s fallback | `lib/safe/formatting.test.ts` |
| `NetworkBadge` — renders correct short name per chain | `components/common/NetworkBadge.test.tsx` |
| `StatusBadge` — renders PROPOSED and ATTESTED correctly | `components/common/StatusBadge.test.tsx` |
| `TransactionListControls` — refresh button calls refetch | `components/transaction/TransactionListControls.test.tsx` |
| `TransactionListControls` — toggle switches auto-refresh state | `components/transaction/TransactionListControls.test.tsx` |
| `TransactionListControls` — last updated displays formatted time | `components/transaction/TransactionListControls.test.tsx` |

---

## Implementation Phases

### Phase 1 — Refresh Controls Bar (independent PR)

**What this covers:**
- Refresh button that triggers an immediate data refetch.
- Auto-refresh toggle (ephemeral state; `ON` by default).
- "Last updated" timestamp displayed in local time with UTC offset.

**Files touched:**
- `explorer/src/components/transaction/TransactionListControls.tsx` — new component
- `explorer/src/components/transaction/RecentTransactionProposals.tsx` — add controls strip above the list, wire `autoRefresh` toggle and `refetch` callback
- `explorer/src/hooks/useRecentTransactionProposals.tsx` — accept `autoRefresh` param; expose `dataUpdatedAt`
- `explorer/src/routes/index.tsx` — pass `autoRefresh` state to the hook
- Tests for `TransactionListControls`

**Components implemented:**
- `TransactionListControls`

---

### Phase 2 — Transaction Row Redesign (independent PR)

**What this covers:**
- Status badge (`PROPOSED` / `ATTESTED`) replacing border colour.
- Network badge with chain short name.
- Shortened SafeTxHash (first + last 4 bytes).
- "When" column using block-age approximation.
- Compact card-list layout replacing the current stacked `Box` cards.

**Files touched:**
- `explorer/src/lib/chains.ts` — add explicit `blockTime` values for `base` and `sepolia` in `SAFE_SERVICE_CHAINS`
- `explorer/src/lib/safe/formatting.ts` — add `formatHashShort` and `formatBlockAge` utilities
- `explorer/src/components/common/NetworkBadge.tsx` — new component
- `explorer/src/components/common/StatusBadge.tsx` — new component
- `explorer/src/components/transaction/TransactionListRow.tsx` — new component
- `explorer/src/components/transaction/RecentTransactionProposals.tsx` — switch to compact card-list layout, use `TransactionListRow`
- Tests for utilities and new components

**Components implemented:**
- `NetworkBadge`
- `StatusBadge`
- `TransactionListRow`

---

## Open Questions / Assumptions

1. **Network icons**: The current design uses text-only badges (e.g. `ETH`, `GNO`). Adding chain logo icons (e.g. via `@web3icons/react`) is left for a follow-up if desired.
2. **Current block for block-age**: `ConsensusState.currentBlock` is assumed to be available in the component tree. If the consensus state query is not already exposed at the list level, Phase 2 needs to surface it — this should be verified before implementation.
3. **Responsive layout**: The compact card-list row may need to reflow on narrow screens (e.g. hide or stack the Block column). This is left to implementation judgement; follow the existing Tailwind responsive patterns used elsewhere in the explorer.
4. **Proposal date accuracy from block age**: For very old proposals the YYYY-MM-DD display is an approximation derived from block times. This is intentional and acceptable.
5. **Filtering interaction**: The existing network filter (via the `network` URL search param in `index.tsx`) is unaffected by these changes. The filter continues to work at the data level before proposals reach the list component.
