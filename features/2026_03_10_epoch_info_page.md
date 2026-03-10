# Feature Proposal: Epoch Info Page
Component: `explorer`

---

## Overview

A dedicated `/epoch` page that surfaces the current epoch state and the historic epoch rollover list. An optional second phase adds a "Pending Key Generation" section for in-progress ceremonies that have not yet been proposed to the Consensus contract.

**Phases:**

1. **Phase 1** *(implemented)* — Current epoch cards + epoch rollover history list with lazy keygen details.
2. **Phase 2** — Pending key generation section, shown at the top of the page when a live ceremony exists.

---

## Architecture Decision

The page uses the Consensus contract's canonical epoch lifecycle events as its primary data source:

```
EpochStaged(uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 rolloverBlock, Secp256k1.Point groupKey, FROST.Signature attestation)
```

Each `EpochStaged` event represents one completed epoch rollover. The keygen for epoch N is bounded to the block range `[EpochStaged(N-1).blockNumber, EpochStaged(N).blockNumber]`, enabling precise per-epoch queries without a global scan.

Phase 2 queries `KeyGen` events from the FROSTCoordinator starting from the most recent `EpochStaged` block (or `currentBlock - maxBlockRange` as fallback). Any keygen whose result has not yet been proposed to Consensus is shown as pending.

### Alternatives Considered

- **Query coordinator directly for all KeyGen events within `maxBlockRange`**: simpler but can only surface groups currently in contract state, has no pagination path, and requires indirect epoch correlation.

---

## User Flow

### Phase 1 (implemented)

1. User clicks the GroupId in the Explorer header → navigates to `/epoch`.
2. Page shows two cards: Current Epoch and (if staged) Staged Epoch.
3. Below the cards, an "Epoch Rollover History" list shows past `EpochStaged` entries with epoch numbers and block numbers.
4. Each entry has a "Show details" toggle that lazily loads per-validator keygen participation (committed / shared / confirmed) for that epoch.
5. A "Load more" button fetches older entries one `maxBlockRange` window at a time.

### Phase 2

6. If any `KeyGen` events are found in the block window after the most recent `EpochStaged`, a "Pending Key Generation" section appears above the history list.
7. Pending keygen entries are always expanded (live state is of immediate interest) and refetch on the standard `refetchInterval` until terminal.

### Page Layout (Phase 2 addition)

```
┌─────────────────────────────────────────────────────┐
│  ← Back                        Epoch Info           │
├─────────────────────────────────────────────────────┤
│  [Current Epoch card]  [Staged Epoch card (opt.)]   │
│                                                      │
│  Pending Key Generation      ← Phase 2              │
│  ┌───────────────────────────────────────────────┐  │
│  │  [PENDING] KeyGen 0xabcd…                     │  │
│  │  Threshold: 3 of 5                            │  │
│  │  Committed: V1 ✅  V2 ✅  V3 ⏳  V4 ⏳  V5 ⏳  │  │
│  └───────────────────────────────────────────────┘  │
│                                                      │
│  Epoch Rollover History      ← Phase 1 (done)       │
│  [entries …]                                         │
│  [ Load more ]                                       │
└─────────────────────────────────────────────────────┘
```

---

## Tech Specs

### Phase 1 — Implemented

The following are already in place:

| File | Purpose |
|------|---------|
| `explorer/src/routes/epoch.tsx` | `/epoch` route |
| `explorer/src/components/epoch/EpochCard.tsx` | Current / staged epoch card |
| `explorer/src/components/epoch/EpochRolloverItem.tsx` | History row with lazy keygen detail expansion |
| `explorer/src/components/epoch/KeyGenStatusItem.tsx` | Per-keygen participation display reusing `ValidatorList` |
| `explorer/src/hooks/useEpochsState.tsx` | TanStack Query: current epoch state |
| `explorer/src/hooks/useEpochRolloverHistory.tsx` | TanStack Query: `EpochStaged` events with load-more cursor |
| `explorer/src/hooks/useKeyGenDetails.tsx` | TanStack Query: lazy per-epoch keygen participation |
| `explorer/src/lib/coordinator/keygen.ts` | `loadKeyGenDetails()` |

The `blocksPerEpoch` optional setting is used to snap the keygen query start block to the epoch boundary; the previous entry's `stagedAt` is used as the fallback start block.

### Phase 2 — Pending Key Generation

#### New hook: `useKeyGenPending`

```typescript
// explorer/src/hooks/useKeyGenPending.tsx
useQuery({
  queryKey: ["keyGenPending", consensus, lastStagedAt, currentBlock, settings],
  queryFn: () => loadPendingKeyGens({ provider, consensus, lastStagedAt, currentBlock, settings }),
  refetchInterval: (query) => {
    const allTerminal = query.state.data?.every(k => k.finalized || k.compromised);
    return allTerminal ? false : settings.refetchInterval;
  },
});
```

#### New loader: `loadPendingKeyGens`

Add to `explorer/src/lib/coordinator/keygen.ts`. Reuses `loadCoordinator()` (already exported from `signing.ts`) and the existing `KeyGen` event parsing logic from `loadKeyGenDetails`. The start block is derived with the same `blocksPerEpoch` / `prevStagedAt` / `maxBlockRange` fallback chain already used in `loadKeyGenDetails`.

#### Component reuse

`KeyGenStatusItem` is already parameterised by `KeyGenStatus` and renders pending vs. terminal states. No changes needed — Phase 2 only passes `enabled={true}` and removes the toggle wrapper, since pending entries are always expanded.

#### Files touched

| File | Change |
|------|--------|
| `explorer/src/lib/coordinator/keygen.ts` | Add `loadPendingKeyGens()` |
| `explorer/src/hooks/useKeyGenPending.tsx` | New hook |
| `explorer/src/routes/epoch.tsx` | Add "Pending Key Generation" section using `useKeyGenPending` |

### Settings

`blocksPerEpoch` (optional `number`) is already in `settings.ts`. No new settings required.

### Test Cases

| Scenario | Expected |
|----------|----------|
| No `KeyGen` events since last `EpochStaged` | Pending section not rendered |
| One active keygen (not yet finalized) | Pending section shown with participation rows |
| Keygen reaches terminal state | Refetching stops; section stays visible showing final state |
| `loadPendingKeyGens` — `blocksPerEpoch` set | Start block snaps to epoch boundary |
| `loadPendingKeyGens` — no `blocksPerEpoch`, `lastStagedAt` known | Start block = `lastStagedAt` |
| `loadPendingKeyGens` — neither available | Start block = `currentBlock - maxBlockRange` |

---

## Implementation Phases

### Phase 1 — Rollover History (implemented)

Done. See files listed in Tech Specs above.

### Phase 2 — Pending Key Generation (single PR)

**Files touched:**
- `explorer/src/lib/coordinator/keygen.ts` — add `loadPendingKeyGens()`
- `explorer/src/hooks/useKeyGenPending.tsx` — new
- `explorer/src/routes/epoch.tsx` — add pending section

Run `npm run check` and `npm test` in the `explorer` workspace before merging.

---

## Open Questions / Assumptions

1. A keygen that starts before the most recent `EpochStaged` block but is still in progress (edge case during chain reorg or slow participants) will not be surfaced. This is acceptable for the initial implementation.
2. The `participants` field in the `KeyGen` event is a Merkle root, not a participant list. The validator identifier set for the `ValidatorList` "all" prop is derived from `KeyGenCommitted` identifiers seen so far; before any commits the validator info map is used as fallback (existing behaviour).
3. Complaints (`KeyGenComplained` / `KeyGenComplaintResponded`) are shown as annotations on the keygen item; detailed complaint breakdown is out of scope.
