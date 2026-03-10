# Feature Proposal: Epoch Info Page

**Component:** `explorer/` (Safenet Explorer)
**Branch:** `claude/add-epoch-info-modal-hgBkV`
**Status:** Proposed

---

## Overview

When a user clicks the **GroupId** in the Explorer header, they are navigated to a new dedicated `/epoch` route. The page is structured in two distinct phases, each backed by different on-chain data:

1. **Current epoch state** — which epoch is active, which (if any) is staged for rollover
2. **Phase 1 — Historic epoch rollover list** — driven by `EpochProposed` / `EpochStaged` events on the Consensus contract; each entry represents one completed or pending-attestation epoch change, with keygen details loaded lazily on demand
3. **Phase 2 — Pending key generation** — any `KeyGen` events on the FROSTCoordinator emitted after the most recent `EpochProposed`, representing an in-progress ceremony for the next epoch that has not yet been proposed to Consensus

---

## Architecture Decision

This feature uses the Consensus contract's canonical epoch lifecycle events as its primary data source:

```
EpochProposed(activeEpoch, proposedEpoch, rolloverBlock, groupKey)
EpochStaged(activeEpoch, proposedEpoch, rolloverBlock, groupKey, attestation)
EpochRolledOver(newActiveEpoch)
```

- **Precise block ranges**: the keygen for `proposedEpoch = N` definitively completed between `EpochStaged(proposedEpoch=N-1).block` and `EpochProposed(proposedEpoch=N).block`. Querying the coordinator within that narrow window is far more accurate than a global scan.
- **Natural pagination**: `EpochProposed` events are lightweight; the history list can load instantly and extend backward with "Load more" without pre-fetching all keygen details.
- **Lazy keygen details**: a "Show details" toggle per epoch fetches participation events only on demand.
- **Clean phase separation**: Phase 1 (completed rollover history) and Phase 2 (live pending ceremony) are independently queryable.

### Alternative considered

An alternative approach would query the FROSTCoordinator directly for `KeyGen` events within `maxBlockRange` and filter them by currently-known group IDs obtained from `getEpochsState()` (previous/active/staged). This is simpler to implement but has meaningful drawbacks: it can only surface groups currently held in contract state, the `maxBlockRange` bluntly cuts off history with no pagination path, and the correlation between a keygen and its epoch is indirect (derived by matching `gid` against state rather than from lifecycle events). The event-driven approach is preferred.

---

## User Flow

1. User opens the Safenet Explorer. The header shows:
   ```
   Block: 12345 | Epoch: 3 | GroupId: 0xdeadbeef...
   ```
2. User clicks the `GroupId` text (rendered as a navigation link).
3. User is taken to `/epoch`.
4. Page loads current epoch state + the epoch rollover history list.
5. If a pending keygen exists it is shown at the top.
6. User can click "Show details" on any history entry to load per-validator keygen participation.
7. User can click "Load more" to fetch older epoch entries beyond the initial `maxBlockRange`.

---

## Route

| Path | File |
|------|------|
| `/epoch` | `explorer/src/routes/epoch.tsx` |

---

## Page Layout

```
┌─────────────────────────────────────────────────────┐
│  ← Back                   Epoch Info                │
├─────────────────────────────────────────────────────┤
│  ┌───────────────────────┐  ┌──────────────────────┐│
│  │  Current Epoch        │  │  Staged Epoch (opt.) ││
│  │  Epoch:    3          │  │  Epoch:    4         ││
│  │  Group ID: 0xdead...  │  │  Group ID: 0xabcd... ││
│  └───────────────────────┘  │  Rollover: 99000     ││
│                              └──────────────────────┘│
│                                                      │
│  Pending Key Generation          (Phase 2, if any)   │
│  ┌───────────────────────────────────────────────┐   │
│  │  [PENDING] KeyGen 0xabcdef...                 │   │
│  │  Threshold: 3 of 5                            │   │
│  │  Committed: V1 ✅  V2 ✅  V3 ⏳  V4 ⏳  V5 ⏳  │   │
│  │  Shared:    V1 ✅  V2 ⏳  V3 ⏳  V4 ⏳  V5 ⏳  │   │
│  └───────────────────────────────────────────────┘   │
│                                                      │
│  Epoch Rollover History          (Phase 1)           │
│  ┌───────────────────────────────────────────────┐   │
│  │  Epoch 2 → 3     Proposed: block 88000        │   │
│  │                  Staged:   block 88042         │   │
│  │  [Show details ▼]                             │   │
│  │  ┌─────────────────────────────────────────┐  │   │
│  │  │  [FINALIZED] KeyGen 0xdeadbeef...       │  │   │
│  │  │  Threshold: 3 of 5                      │  │   │
│  │  │  Confirmed: V1 ✅  V2 ✅  V3 ✅  V4 ❌  V5 ✅│  │   │
│  │  └─────────────────────────────────────────┘  │   │
│  └───────────────────────────────────────────────┘   │
│  ┌───────────────────────────────────────────────┐   │
│  │  Epoch 1 → 2     Proposed: block 50000        │   │
│  │                  Staged:   block 50089         │   │
│  │  [Show details ▼]                             │   │
│  └───────────────────────────────────────────────┘   │
│                                                      │
│               [ Load more ]                          │
└─────────────────────────────────────────────────────┘
```

---

## Data Sources

### Current Epoch State

A single `getEpochsState()` call on the Consensus contract returns `{previous, active, staged, rolloverBlock}`. `getEpochGroupId(active)` and (if staged > 0) `getEpochGroupId(staged)` provide the group IDs for the two cards at the top.

```solidity
function getEpochsState() external view returns (uint64 previous, uint64 active, uint64 staged, uint64 rolloverBlock);
function getEpochGroupId(uint64 epoch) external view returns (bytes32 group);
```

### Settings

One new optional setting is added alongside the existing ones in `explorer/src/lib/settings.ts`:

| Key | Type | Default | Purpose |
|-----|------|---------|---------|
| `blocksPerEpoch` | `number \| undefined` | `undefined` | Expected epoch duration in blocks. When set, the keygen query start block is computed as `proposedBlock - (proposedBlock % blocksPerEpoch)`, snapping to the epoch boundary. When absent, the previous entry's `EpochStaged.blockNumber` is used as the start block instead. |

The field is exposed in the Consensus Settings form alongside `maxBlockRange`.

---

### Phase 1 — Historic Epoch Rollover List

#### Primary event (Consensus contract)

```
EpochStaged(uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 rolloverBlock, Secp256k1.Point groupKey, FROST.Signature attestation)
```

Each `EpochStaged` event represents one completed epoch rollover and contains all required information: `activeEpoch`, `proposedEpoch`, `rolloverBlock`, and the `groupKey`/`attestation`. The `blockNumber` of the event provides the `stagedAt` timestamp.

**Initial load:** fetch `EpochStaged` events from `[currentBlock - maxBlockRange, latest]`.

**"Load more":** each click fetches the next `maxBlockRange` window backward (i.e., `[prevFromBlock - maxBlockRange, prevFromBlock]`) and appends newly found entries.

#### Per-epoch keygen details (FROSTCoordinator contract, lazy)

For each history entry, clicking "Show details" triggers a query for that epoch's keygen participation:

- **gid**: `getEpochGroupId(proposedEpoch)` — one contract read per epoch, performed at expansion time
- **Block range for the keygen query** — the keygen for epoch N completed before `EpochStaged(N).blockNumber`. The end block is always `EpochStaged(N).blockNumber`. The start block is derived as follows:
  - **If `blocksPerEpoch` is configured in settings**: `EpochStaged(N).blockNumber - (EpochStaged(N).blockNumber % blocksPerEpoch)` — snaps back to the epoch boundary block
  - **Fallback 1** (no `blocksPerEpoch`): `EpochStaged(N-1).blockNumber`, i.e. the block at which the previous epoch was staged — available from the already-loaded history without any extra query
  - **Fallback 2** (no previous entry): `EpochStaged(N).blockNumber - maxBlockRange`
- **Events fetched**: `KeyGen` (to get `count` / `threshold`) + `KeyGenCommitted / KeyGenSecretShared / KeyGenConfirmed / KeyGenComplained` for that `gid` within the range

> **Note**: `EpochStaged` carries `groupKey` (a `Secp256k1.Point {x, y}`), not the `gid` (bytes32). The `gid` must be resolved via `getEpochGroupId(proposedEpoch)` — one read per epoch, done lazily at expansion time.

#### Epoch entry display (collapsed)

| Field | Source |
|-------|--------|
| `activeEpoch → proposedEpoch` | event args |
| Staged at block | `EpochStaged.blockNumber` |

---

### Phase 2 — Pending Key Generation

Query `KeyGen` events from the coordinator from `[pendingKeyGenStartBlock, latest]`, where `pendingKeyGenStartBlock` is derived using the same logic as Phase 1's keygen block range:

- **If `blocksPerEpoch` is configured in settings**: `currentBlock - (currentBlock % blocksPerEpoch)` — snaps back to the current epoch boundary.
- **Fallback** (no `blocksPerEpoch`): the most recent `EpochStaged.blockNumber` available from Phase 1 data. `EpochStaged` marks the block at which the previous keygen's result was officially accepted by the Consensus contract; any keygen started after that block is a candidate for the pending ceremony.

Any `KeyGen` events in this window represent ceremonies that have been started but whose resulting group has not yet been proposed to the Consensus contract.

Each pending keygen entry is **always expanded** (no "show details" toggle) since it is live and the validator participation is of immediate interest. Refetching follows the standard `refetchInterval` setting and stops once the keygen reaches a terminal state.

Per-validator participation events (`KeyGenCommitted / KeyGenSecretShared / KeyGenConfirmed / KeyGenComplained`) are fetched for the gid(s) found.

---

## Reuse from Attestation Status Flow

All reuse decisions from the previous spec revision remain valid. Summarised here for completeness.

### `ValidatorList` (move to shared location)

Move from `SafeTxAttestationStatus.tsx` → `components/common/ValidatorList.tsx`. No logic changes. Both `SafeTxAttestationStatus` and `KeyGenStatusItem` import from the shared location.

### `useValidatorInfoMap()`

Imported directly into `KeyGenStatusItem`, unchanged.

### `mapInfo` pattern

`(suffix: string) => (identifier: bigint) => string` — duplicated verbatim in `KeyGenStatusItem`, co-located with the component.

### `loadCoordinator()` in `signing.ts`

Exported so `keygen.ts` imports it directly, sharing the module-level coordinator address cache.

### `useQuery` hook structure

`useKeyGenDetails` (lazy, per-epoch) and `useKeyGenPending` (Phase 2) both follow the identical shape as `useAttestationStatus`: `useQuery` keyed on consensus + relevant block range identifiers, `refetchInterval` driven by `settings.refetchInterval`, stopping once terminal.

### Loading skeleton

Same `<Skeleton className="w-full h-10 bg-primary/10" />` pattern.

### Conditional phase rendering

`KeyGenStatusItem` follows the same conditional row-visibility rules as `SafeTxAttestationStatus`:

| Condition | Committed row | Shared row | Confirmed row |
|-----------|--------------|------------|---------------|
| In progress (COMMITTING) | shown | hidden | hidden |
| In progress (SHARING / CONFIRMING) | shown | shown | hidden |
| Terminal (FINALIZED / COMPROMISED) | hidden | hidden | shown |

`completed=true` passed to `ValidatorList` when terminal → ❌ for non-participants.

---

## Types

```typescript
// explorer/src/lib/consensus.ts (additions)

export type EpochsState = {
  previous: bigint;
  active: bigint;
  staged: bigint;           // 0n if no epoch is staged
  rolloverBlock: bigint;
  activeGroupId: Hex;
  stagedGroupId: Hex | null;
};

export type EpochRolloverEntry = {
  activeEpoch: bigint;
  proposedEpoch: bigint;
  rolloverBlock: bigint;
  stagedAt: bigint;    // blockNumber of EpochStaged event
};

// explorer/src/lib/coordinator/keygen.ts

export type KeyGenParticipation = {
  identifier: bigint;
  block: bigint;
};

export type KeyGenStatus = {
  gid: Hex;
  count: number;
  threshold: number;
  startBlock: bigint;
  committed: KeyGenParticipation[];
  shared: KeyGenParticipation[];
  confirmed: KeyGenParticipation[];
  finalized: boolean;
  compromised: boolean;
};
```

---

## New Files

| File | Purpose |
|------|---------|
| `explorer/src/routes/epoch.tsx` | `/epoch` route component |
| `explorer/src/components/epoch/EpochCard.tsx` | Current / staged epoch card |
| `explorer/src/components/epoch/EpochRolloverItem.tsx` | One history row: collapsed summary + lazy "Show details" expansion |
| `explorer/src/components/epoch/KeyGenStatusItem.tsx` | Per-keygen participation display; reuses `ValidatorList` + `useValidatorInfoMap` |
| `explorer/src/lib/consensus.ts` *(extended)* | `EpochRolloverEntry` type + `loadEpochRolloverHistory()` |
| `explorer/src/lib/coordinator/keygen.ts` | `loadKeyGenDetails()` (for one epoch, given gid + block range); `loadPendingKeyGens()` (Phase 2) |
| `explorer/src/hooks/useEpochsState.tsx` | TanStack Query: `getEpochsState` + group IDs |
| `explorer/src/hooks/useEpochRolloverHistory.tsx` | TanStack Query: `EpochProposed`/`EpochStaged` events with load-more cursor |
| `explorer/src/hooks/useKeyGenDetails.tsx` | TanStack Query: lazy per-epoch keygen participation |
| `explorer/src/hooks/useKeyGenPending.tsx` | TanStack Query: Phase 2 pending ceremonies |

---

## Modified Files

| File | Change |
|------|--------|
| `explorer/src/components/Header.tsx` | Wrap `GroupId` in `<Link to="/epoch">` |
| `explorer/src/lib/consensus.ts` | Add `getEpochsState`, `getEpochGroupId`, `EpochProposed`/`EpochStaged` events to ABI; export loaders |
| `explorer/src/lib/coordinator/signing.ts` | Export `loadCoordinator` |
| `explorer/src/lib/settings.ts` | Add optional `blocksPerEpoch` field |
| `explorer/src/components/settings/ConsensusSettingsForm.tsx` | Add `blocksPerEpoch` input |
| `explorer/src/components/transaction/SafeTxAttestationStatus.tsx` | Update `ValidatorList` import |
| `explorer/src/components/common/ValidatorList.tsx` | New shared file — `ValidatorList` moved here |

---

## Implementation Steps

### Phase 1

1. Move `ValidatorList` to `components/common/ValidatorList.tsx`; update import in `SafeTxAttestationStatus.tsx`
2. Export `loadCoordinator` from `coordinator/signing.ts`
3. Extend `consensus.ts` ABI with `getEpochsState`, `getEpochGroupId`, `EpochProposed`, `EpochStaged` events; add `EpochsState` + `EpochRolloverEntry` types; export `loadEpochsState()` + `loadEpochRolloverHistory()`
4. Create `coordinator/keygen.ts` with `loadKeyGenDetails()` and `loadPendingKeyGens()`
5. Create `useEpochsState`, `useEpochRolloverHistory`, `useKeyGenDetails`, `useKeyGenPending` hooks
6. Create `EpochCard`, `KeyGenStatusItem`, `EpochRolloverItem` components
7. Create `/epoch` route
8. Update `Header.tsx`
9. `npm run check` + fix; commit + push

### Phase 2

Implement `useKeyGenPending` + the "Pending Key Generation" section of the page, using the `lastEpochProposedBlock` cursor already available from Phase 1 data.

---

## Open Questions / Assumptions

- A proposed epoch without a matching `EpochStaged` event (un-staged proposal) is not shown in the history list. Such entries indicate the attestation signing is in progress; surfacing them explicitly is out of scope for this feature.
- When `blocksPerEpoch` is not configured and the previous `EpochStaged` entry is not available in the loaded window (e.g. for the oldest loaded entry), the keygen query range falls back to `[EpochProposed.block - maxBlockRange, EpochProposed.block]`.
- Complaints (`KeyGenComplained` / `KeyGenComplaintResponded`) are surfaced as annotations on the keygen item; a detailed complaint breakdown is out of scope.
- The `participants` field in the `KeyGen` event is a Merkle root of participant identifiers (not a packed list). The `all` set for `ValidatorList` is derived from the `KeyGenCommitted` identifiers seen (populated once round 1 starts); before any commits appear, the validator info map is used as the fallback.
- The page auto-refreshes at `settings.refetchInterval`; Phase 2 stops refetching once all pending keygens reach a terminal state.
