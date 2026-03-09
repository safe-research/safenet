# Feature Proposal: Search Transaction Proposals by Safe Address
Component: `explorer`

---

## Overview

Add a dedicated page that lists all `TransactionProposed` events associated with a given Safe address. The user can enter a Safe address in the existing `SearchBar` — when the input is detected as an Ethereum address, the explorer navigates to the new `/safe` route instead of the existing `/safeTx` route. Results are rendered with the existing `RecentTransactionProposals` component. Older data can be fetched block-range-by-block-range via a "Load More" button.

**Phases:**

1. **Phase 1** — Backend: extend `loadTransactionProposals` with an optional `safe` address topic filter and add a `useSafeTransactionProposals` hook backed by `useInfiniteQuery`.
2. **Phase 2** — Frontend: new `/safe` route + address auto-detection in `SearchBar`.

Phases are dependent (Phase 2 consumes Phase 1) but small enough to be shipped as a single PR or as two sequential PRs.

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

Accumulated results from all loaded pages are concatenated and passed to `RecentTransactionProposals`. A "Load More" button triggers `fetchNextPage()`, always fetching exactly one `maxBlockRange` window.

The current block used as the starting anchor is captured when the query is first mounted and stored as `initialBlock` in query state to keep page boundaries stable across re-fetches.

### Reuse of `RecentTransactionProposals`

`RecentTransactionProposals` already accepts `proposals`, `itemsToShow`, and `onShowMore`. The new route passes all loaded proposals with `itemsToShow` set to `proposals.length` (all loaded data is shown; "Show More" in this context means "fetch older blocks", not "reveal hidden rows"). Because fetching more data is async, the button will show a loading state while the next page is being fetched.

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
2. Matching proposals are shown using `RecentTransactionProposals` (same styling as home page).
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
| `explorer/src/routes/safe.tsx` | New route — renders header with Safe address, chain name, and `RecentTransactionProposals` |
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

### Phase 2 — Route & SearchBar update

**Files touched:**
- `explorer/src/routes/safe.tsx` — new file
- `explorer/src/components/search/SearchBar.tsx` — address auto-detection

**Scope:** Full user-visible feature. Depends on Phase 1.

**Tests:**
- Component test for `SearchBar` — address input navigates to `/safe`
- Component test for `safe.tsx` route — renders proposals list, "Load More" triggers next page

---

## Open Questions / Assumptions

- **Block anchor stability**: The `initialBlock` used as the starting point for pagination is captured at mount time. If the chain advances while the user is paginating, new proposals in the latest blocks will not appear until the query is invalidated (e.g. on refetch). This is acceptable for a history-oriented search.
- **`RecentTransactionProposals` label**: The component currently renders "N recent proposals". On the Safe search page this copy should read "N proposals" or be passed as a prop. Adjusting the label is a minor change that can be included in Phase 2 (either via a prop or by extracting a shared base component).
- **Checksum requirement**: The Safe address in the URL is expected to be checksummed. The `SearchBar` will call `getAddress()` before navigating to normalise any lowercase input.
- **Chain context**: The chain selector in `SearchBar` already provides the `chainId` needed for the route. No additional chain-selection UI is required on the `/safe` page itself.
