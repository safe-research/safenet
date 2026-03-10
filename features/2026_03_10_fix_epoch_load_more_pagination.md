# Feature Proposal: Fix Epoch Load More Pagination Bugs
Component: `explorer`

---

## Overview

Two bugs in the epoch rollover history pagination make it impossible to navigate all the way back through epoch history:

1. **Empty page disables "Load more"** — when no `EpochStaged` events are found in the searched block range, the pagination cursor is `undefined`, which TanStack Query interprets as "no more pages", hiding the "Load more" button even though earlier epochs may exist.
2. **Last entry is duplicated** — the cursor for the next page is the block number of the oldest entry on the current page; that block is included (inclusive) in both the current and next fetch, causing the boundary entry to appear twice.

Both bugs are fixed in a single PR touching `loadEpochRolloverHistory` and `useEpochRolloverHistory`.

---

## Architecture Decision

### Bug 2 Fix: Cursor Offset

`loadEpochRolloverHistory` currently calls `getBlockRange(provider, maxBlockRange, cursor)` when a cursor is provided, making `toBlock = cursor`. Since `cursor = oldest.stagedAt` (the block of the last entry of the previous page), and the previous page fetched up to `cursor` inclusive, the event at that block is returned again.

**Fix:** pass `cursor - 1n` to `getBlockRange` so the next page fetches `[cursor - 1 - maxBlockRange, cursor - 1]`, excluding the already-seen block.

### Bug 1 Fix: fromBlock Fallback

`getNextPageParam` in `useEpochRolloverHistory` falls back to `oldest?.stagedAt`, which is `undefined` when `entries` is empty. TanStack Query treats `undefined` as "no next page".

**Fix:**
- Add `fromBlock: bigint` to `EpochRolloverResult` so callers know where the last search started.
- Also set `reachedGenesis = true` when `fromBlock === 0n` (we have searched all the way to block 0, so there is nowhere left to look).
- In `getNextPageParam`, return `oldest?.stagedAt ?? lastPage.fromBlock` so that an empty page still advances the cursor backwards by one `maxBlockRange` window instead of stopping.

### Alternatives Considered

- **Skip empty pages inside `loadEpochRolloverHistory`** — loop internally until events are found or genesis is reached. Rejected because it may cause very long-running queries and removes per-page transparency for the user.
- **Store the `toBlock` instead of `fromBlock`** — using `toBlock` as the fallback cursor would repeat the same range. `fromBlock` is the correct next-window upper boundary.

---

## Tech Specs

### Modified files

- `explorer/src/lib/consensus.ts`
  - `EpochRolloverResult`: add `fromBlock: bigint`
  - `loadEpochRolloverHistory`: pass `cursor - 1n` to `getBlockRange`; set `reachedGenesis` also when `fromBlock === 0n`; return `fromBlock` in result
- `explorer/src/hooks/useEpochRolloverHistory.tsx`
  - `getNextPageParam`: return `oldest?.stagedAt ?? lastPage.fromBlock`
- `explorer/src/lib/consensus.test.ts`
  - Update "uses cursor as toBlock when provided" to expect `cursor - 1n`
  - Update "returns empty entries when no logs are found" to check `fromBlock`
  - Add tests for `fromBlock === 0n` → `reachedGenesis === true` and fallback cursor behaviour

---

## Implementation Phases

### Phase 1 (this PR) — Fix both bugs

Single PR since the changes are small, tightly coupled, and safe to review together.

Files touched:
- `explorer/src/lib/consensus.ts`
- `explorer/src/hooks/useEpochRolloverHistory.tsx`
- `explorer/src/lib/consensus.test.ts`

---

## Open Questions / Assumptions

- It is assumed that multiple `EpochStaged` events cannot share the same block number. If they can, using `cursor - 1n` is still safe because the previous page already fetches all events in that block (inclusive).
- `reachedGenesis` via `fromBlock === 0n` is conservative: it stops pagination as soon as the search window touches block 0, even if no genesis event was emitted there. This is preferable to an infinite loop on chains where the epoch 0 event was never emitted.
