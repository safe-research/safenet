# Feature Proposal: Search Transaction Proposals by Safe Address
Component: `explorer`

---

## Overview

Add a dedicated page that lists all `TransactionProposed` events associated with a given Safe address. The user can enter a Safe address in the existing `SearchBar` — when the input is detected as an Ethereum address, the explorer navigates to the new `/safe` route instead of the existing `/safeTx` route. Older data can be fetched block-range-by-block-range via a "Load More" button.

`RecentTransactionProposals` is refactored into a generic `TransactionProposalsList` component that both the home page and the new Safe search page consume.

**Phases:**

1. **Phase 1** — Backend: extend `loadTransactionProposals` with an optional `safe` address topic filter and add a `useSafeTransactionProposals` hook backed by `useInfiniteQuery`.
2. **Phase 2** — Shared component: refactor `RecentTransactionProposals` into a generic `TransactionProposalsList` and keep `RecentTransactionProposals` as a thin wrapper.
3. **Phase 3** — Frontend: new `/safe` route + address auto-detection in `SearchBar`, consuming `TransactionProposalsList` directly.

Phase 1 and Phase 2 are independent and can be parallelised. Phase 3 depends on both.

---

## Architecture Decision

### Filtering by Safe address in `loadTransactionProposals`

The `TransactionProposed` event already indexes the `safe` address as its third indexed topic:

```
event TransactionProposed(
  bytes32 indexed transactionHash,
  uint256 indexed chainId,
  address  indexed safe,
  ...
)
```

`loadTransactionProposals` builds a raw `eth_getLogs` topic filter. Adding a `safe` parameter requires placing it in topic position 3 (index 2):

```typescript
topics: [transactionEventSelectors, safeTxHash ?? null, null, safe ?? null],
```

This extends the existing signature with an optional `safe?: Address` parameter — no breaking change.

### Block-range pagination with `useInfiniteQuery`

The recent proposals page fetches a single `maxBlockRange`-sized window and then paginates _client-side_ (all data is already in memory). That is fine for a live feed of recent activity, but for a Safe-specific search the user may want to look further back in time.

The new hook (`useSafeTransactionProposals`) uses TanStack Query's `useInfiniteQuery`. Each page corresponds to one `maxBlockRange`-sized block window:

- **Page 0** — `[currentBlock - maxBlockRange, currentBlock]`
- **Page 1** — `[currentBlock - 2 × maxBlockRange, currentBlock - maxBlockRange - 1]`
- etc.

Accumulated results from all loaded pages are concatenated and passed to `TransactionProposalsList`. A "Load More" button triggers `fetchNextPage()`, always fetching exactly one `maxBlockRange` window.

The current block used as the starting anchor is captured when the query is first mounted and stored as `initialBlock` in query state to keep page boundaries stable across re-fetches.

### `TransactionProposalsList` — shared list component

`RecentTransactionProposals` today hardcodes the label `"N recent proposals"`, computes `hasMore` itself from `proposals.length > itemsToShow`, and has no concept of an async loading state on its button. The Safe search page needs different behaviour on all three counts:

- Label: `"N proposals"` (no "recent")
- `hasMore`: driven externally by whether more block pages exist, not by hidden in-memory rows
- Button: shows a loading indicator while `fetchNextPage()` is in flight

Rather than papering over this with ad-hoc props on the existing component, `RecentTransactionProposals` is refactored into two layers:

**`TransactionProposalsList`** (new shared primitive, `components/transaction/TransactionProposalsList.tsx`):

```typescript
{
  proposals: TransactionProposal[];
  label: string;          // e.g. "recent proposals" | "proposals"
  hasMore: boolean;       // whether to render the load/show-more button
  onShowMore: () => void;
  isLoadingMore?: boolean; // shows spinner on button when true
  showMoreLabel?: string;  // defaults to "Show More"
}
```

**`RecentTransactionProposals`** becomes a thin wrapper that keeps the existing home-page contract unchanged:

```typescript
// derives hasMore from proposals.length > itemsToShow
// passes label="recent proposals", showMoreLabel="Show More"
// slices proposals to itemsToShow before passing to TransactionProposalsList
```

The Safe search route uses `TransactionProposalsList` directly, passing `hasMore={hasNextPage}`, `isLoadingMore={isFetchingNextPage}`, and `showMoreLabel="Load More"`.

### Address auto-detection in `SearchBar`

When the user submits the search input, the component checks whether the trimmed string is a valid EVM address using viem's `isAddress()`. If it is, navigation goes to `/safe`; otherwise the existing `/safeTx` behaviour is preserved.

### Alternatives Considered

| Alternative | Reason rejected |
|---|---|
| Separate search input component | Duplicates UI; auto-detection is a natural UX improvement to the existing bar |
| Client-side filter over `useRecentTransactionProposals` data | Only covers the already-loaded block range; cannot page further back |
| Cursor stored in URL params | Complex to keep URL in sync with accumulated multi-page state; `useInfiniteQuery` handles this cleanly in memory |

---

## User Flow

### Auto-detection in search bar

1. User types or pastes a Safe address (`0x…`) into the `SearchBar`.
2. On submit (Enter or click), `isAddress(input)` returns `true`.
3. Navigator pushes `/safe?safeAddress=<checksummed>&chainId=<selected>`.
4. The Safe Proposals page loads.

### Safe Proposals page

1. Page mounts, fetches the most recent `maxBlockRange` blocks for the given Safe address.
2. Matching proposals are shown using `TransactionProposalsList` (same styling as home page).
3. If no proposals are found, an empty-state message is shown.
4. User clicks "Load More" → next `maxBlockRange` block window is fetched and results are appended.
5. When the chain's genesis block is reached, the "Load More" button is hidden.

### Page Layout

```
┌─────────────────────────────────────────────────────────┐
│  Safenet Explorer                                        │
│                                                          │
│  ┌──────────────────────────────────────────────────┐   │
│  │ [Network ▾]  0xAbCd…1234 (Safe address)     [🔍] │   │
│  └──────────────────────────────────────────────────┘   │
│                                                          │
│  < Back                                                  │
│                                                          │
│  Proposals for Safe: 0xAbCd…1234                        │
│  (Ethereum)                   3 proposals found          │
│                                                          │
│  ┌──────────────────────────────────────────────────┐   │
│  │ Safe Tx Hash: 0xabc…                             │   │
│  │ To: 0xdef… | Value: 1 ETH | Nonce: 7            │   │
│  │ Block: 21500000                         [green]  │   │
│  └──────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────┐   │
│  │ Safe Tx Hash: 0x123…                             │   │
│  │ To: 0x456… | Value: 0 ETH | Nonce: 6            │   │
│  │ Block: 21499800                        [yellow]  │   │
│  └──────────────────────────────────────────────────┘   │
│                                                          │
│            [ Load More ]                                 │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

---

## Tech Specs

### New route

| Property | Value |
|---|---|
| Path | `/safe` |
| File | `explorer/src/routes/safe.tsx` |
| Search params | `safeAddress: Address`, `chainId: bigint` (default `1n`) |

### New / modified components

| File | Change |
|---|---|
| `explorer/src/lib/consensus.ts` | Add optional `safe?: Address` param to `loadTransactionProposals`; extend topics array accordingly |
| `explorer/src/hooks/useSafeTransactionProposals.ts` | New hook — `useInfiniteQuery` over block-range pages for a given Safe address |
| `explorer/src/components/transaction/TransactionProposalsList.tsx` | New shared primitive extracted from `RecentTransactionProposals`; accepts `label`, `hasMore`, `isLoadingMore`, `showMoreLabel` |
| `explorer/src/components/transaction/RecentTransactionProposals.tsx` | Refactored to be a thin wrapper over `TransactionProposalsList` |
| `explorer/src/routes/safe.tsx` | New route — renders header with Safe address, chain name, and `TransactionProposalsList` |
| `explorer/src/components/search/SearchBar.tsx` | Auto-detect address input; navigate to `/safe` when matched |

### Data source

Consensus contract logs via `eth_getLogs`, same RPC provider as the rest of the explorer. No new environment variables or external APIs required.

### Schemas / validation

- `safeAddress` route param — validated with the existing `checkedAddressSchema` (viem checksum).
- `chainId` route param — validated with the existing `bigIntSchema`.
- Invalid params fall back to sensible defaults (matching `safeTx.tsx` pattern).

### Hook: `useSafeTransactionProposals`

```typescript
type PageParam = bigint; // fromBlock for this page

useInfiniteQuery<TransactionProposal[], Error, TransactionProposal[], unknown[], PageParam>({
  queryKey: ["safeProposals", safeAddress, chainId, settings.consensus, settings.maxBlockRange],
  queryFn: ({ pageParam: fromBlock }) =>
    loadTransactionProposals({
      provider,
      consensus: settings.consensus,
      safe: safeAddress,
      fromBlock,
      toBlock: fromBlock + BigInt(settings.maxBlockRange) - 1n,
      maxBlockRange: BigInt(settings.maxBlockRange),
    }),
  initialPageParam: initialFromBlock,       // currentBlock - maxBlockRange
  getNextPageParam: (_lastPage, _pages, lastFromBlock) => {
    const nextFrom = lastFromBlock - BigInt(settings.maxBlockRange);
    return nextFrom >= 0n ? nextFrom : undefined;   // undefined = no more pages
  },
});
```

The accumulated list exposed to the component is `data.pages.flat()`.

### Test cases

| Scenario | Expected |
|---|---|
| Valid Safe address with proposals in range | List rendered; proposal count shown |
| Valid Safe address with no proposals | Empty-state message shown |
| "Load More" clicked | Next block window fetched; results appended |
| Genesis block reached | "Load More" button hidden |
| Invalid address in URL param | Falls back / shows error state |
| Address input in `SearchBar` | Routes to `/safe` instead of `/safeTx` |
| Non-address input in `SearchBar` | Existing behaviour unchanged |

---

## Implementation Phases

### Phase 1 — Data layer & hook (can be reviewed independently)

**Files touched:**
- `explorer/src/lib/consensus.ts` — extend `loadTransactionProposals` signature
- `explorer/src/hooks/useSafeTransactionProposals.ts` — new file

**Scope:** No visible UI change. Only adds the data-fetching capability and its unit tests.

**Tests:**
- Unit test for `loadTransactionProposals` with `safe` filter (mock `provider.request`)
- Unit test for `useSafeTransactionProposals` hook (Vitest + TanStack Query test utilities)

### Phase 2 — `TransactionProposalsList` shared component (parallel with Phase 1)

**Files touched:**
- `explorer/src/components/transaction/TransactionProposalsList.tsx` — new shared primitive
- `explorer/src/components/transaction/RecentTransactionProposals.tsx` — refactored to thin wrapper

**Scope:** Pure refactor; the home page behaviour is unchanged. No new user-visible feature.

**Tests:**
- Unit test for `TransactionProposalsList` — label rendering, `hasMore` gating, `isLoadingMore` state on button
- Verify existing `RecentTransactionProposals` tests still pass

### Phase 3 — Route & SearchBar update (depends on Phase 1 + Phase 2)

**Files touched:**
- `explorer/src/routes/safe.tsx` — new file
- `explorer/src/components/search/SearchBar.tsx` — address auto-detection

**Scope:** Full user-visible feature.

**Tests:**
- Component test for `SearchBar` — address input navigates to `/safe`
- Component test for `safe.tsx` route — renders proposals list, "Load More" triggers next page fetch

---

## Open Questions / Assumptions

- **Block anchor stability**: The `initialBlock` used as the starting point for pagination is captured at mount time. If the chain advances while the user is paginating, new proposals in the latest blocks will not appear until the query is invalidated (e.g. on refetch). This is acceptable for a history-oriented search.
- **Checksum requirement**: The Safe address in the URL is expected to be checksummed. The `SearchBar` will call `getAddress()` before navigating to normalise any lowercase input.
- **Chain context**: The chain selector in `SearchBar` already provides the `chainId` needed for the route. No additional chain-selection UI is required on the `/safe` page itself.
