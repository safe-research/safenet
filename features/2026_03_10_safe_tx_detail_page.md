# Feature Proposal: Safe TX Detail Page — E100
Component: `explorer`

---

## Overview

Enrich the existing `/safeTx` detail page so users can see all relevant information about a Safe
transaction and its proposals in one place. The page already exists but presents information in a
compact prose form. This feature restructures it into three clearly-labelled sections — **Safe TX**,
**Transaction Summary**, and **Proposals / Attestations** — and adds copy buttons, Safe Wallet UI
deep-links, timed-out proposal status, and cleaner validator attestation labels.

The changes are scoped entirely to the `explorer` workspace and are split into three sequentially
deliverable PRs:

1. **Phase 1** — Safe TX header section (prominent SafeTxHash, Safe Wallet links, copy buttons)
2. **Phase 2** — Transaction Summary section (structured key-value layout, calldata copy)
3. **Phase 3** — Proposals UX improvements (numbered proposals, timed-out status, Attested/Missing labels)

---

## Architecture Decision

### Phase 1 — Safe TX Header

The current page renders `<SafeTxOverview>` inside the first `Box`. `SafeTxOverview` is a prose
component also used in the transaction list; it must remain unchanged to avoid breaking the list
view. A new `SafeTxHeader` component is created specifically for the detail page header, replacing
the `SafeTxOverview` usage in `safeTx.tsx`.

`useSafeTransactionDetails` currently returns `SafeTransaction | null`. To drive the conditional
Safe Wallet UI link on the SafeTxHash (only shown when the transaction is known to the Safe API),
the hook's return type is extended to include `fromSafeApi: boolean`. The internal `findAny`
helper is updated to tag each source before racing with `Promise.any`.

Safe Wallet UI deep-links use the chain's `shortName` already present in `SAFE_SERVICE_CHAINS`:
- Transaction URL: `https://app.safe.global/transactions/tx?safe=${shortName}:${safe}&id=${safeTxHash}`
- Safe URL: `https://app.safe.global/balances?safe=${shortName}:${safe}`

These helpers are extracted to a new `lib/safe/wallet.ts` utility module.

A small `CopyButton` component encapsulates copy-to-clipboard using `navigator.clipboard.writeText`,
showing a brief ✓ confirmation after copying.

### Phase 2 — Transaction Summary Section

`SafeTxOverview` currently renders operation, `to`, value, and data in a single prose sentence. On
the detail page we want these as labelled rows. A new `SafeTxSummary` component renders the
transaction-body fields (Operation, To, Value, Calldata length, Raw calldata + copy, Decode link),
consuming the existing `opString`, `valueString`, `dataString` utilities from
`lib/safe/formatting.ts` and the `decoder` URL from `useSettings`. `SafeTxDataDetails` is extended
with a `CopyButton` for the calldata hex (reusing the component from Phase 1). `SafeTxOverview` is
untouched.

### Phase 3 — Proposals UX Improvements

**Timed-out detection**: timeout is a validator configuration parameter, not an epoch boundary.
A `signingTimeout` field (type `number`, default `12`) is added to the settings schema in
`lib/settings.ts`. A proposal with `attestedAt === null` is considered `TIMED OUT` when
`currentBlock - proposal.proposedAtBlock > signingTimeout`. `currentBlock` is obtained from the
existing `useProvider` hook. `signingTimeout` is read from `useSettings` and passed as a prop to
`SafeTxProposals`.

**Validator labels**: `SafeTxAttestationStatus` currently shows "Committed" (nonce pre-commitment)
and "Attested" rows. This two-stage representation is more accurate than the wireframe's simplified
view and is kept as-is. Only visual styling is updated (e.g. colour tokens, badge shapes) to align
with the rest of the redesigned page; no label text or logic changes.

**Proposal numbering**: the `.map()` in `SafeTxProposals` is updated to render a "Proposal #N"
heading for each item.

**No-proposals CTA**: the existing button in `NoSafeTxProposals` is kept; the message is updated
to include the chain name (e.g. "No proposals found for this SafeTxHash on Base.").

### Alternatives Considered

- **Modify `SafeTxOverview` instead of creating `SafeTxHeader`** — rejected because `SafeTxOverview`
  is also used in the transaction list, and further parameterising it would couple list and detail
  concerns. A dedicated detail-page component is cleaner.
- **Epoch-based timeout detection** — using `proposal.epoch < currentEpoch` was considered but
  rejected because the signing timeout is a validator configuration parameter, not an epoch
  boundary. A configurable `signingTimeout` (blocks) is more accurate.
- **Single large PR** — rejected for reviewability; the three phases touch different components and
  can be reviewed independently.

---

## User Flow

### Page Layout

#### Safe TX Section

```
SAFE TX
+-----------------------------------------------------------------------------------+
| SafeTxHash: 0x9f12…7aBc   [Open in Safe Wallet ↗]   [Copy]                       |
| Network: Base (chainId 8453)                                                      |
| Safe: 0xA1b2…9CDe   [Open in Safe Wallet ↗]   [Copy]                             |
+-----------------------------------------------------------------------------------+
```

- "Open in Safe Wallet" link for SafeTxHash is shown only when `fromSafeApi === true` AND
  the chain is in `SAFE_SERVICE_CHAINS`.
- "Open in Safe Wallet" link for Safe address is shown whenever the chain is in
  `SAFE_SERVICE_CHAINS` (no API check needed).

#### Transaction Summary Section

```
TRANSACTION SUMMARY
+-----------------------------------------------------------------------------------+
| Operation:    CALL                                                                |
| To:           0xBEEF…c0de   [Copy]                                                |
| Value:        0.120000 ETH                                                        |
| Calldata:     196 bytes                                                           |
| Raw calldata:                                                                     |
|   0xa9059cbb000000000000000000000000…                                             |
|   [Copy calldata]   [Decode ↗]                                                    |
+-----------------------------------------------------------------------------------+
```

#### Proposals / Attestations Section

```
PROPOSALS / ATTESTATIONS
+-----------------------------------------------------------------------------------+
| Proposal #1                                                                       |
| Status: ATTESTED                                                                  |
| Proposed: Block 19,234,120 at 2026-02-09 12:39:10   [Explorer tx ↗]              |
| Attested: Block 19,234,130 at 2026-02-09 12:39:55   [Explorer tx ↗]              |
| Validators:                                                                       |
|   Attested: [val-01 ✅] [val-02 ✅] [val-03 ✅]                                    |
|   Missing:  [val-04 ❌] [val-05 ❌]                                                |
|-----------------------------------------------------------------------------------|
| Proposal #2                                                                       |
| Status: PROPOSED                                                                  |
| Proposed: Block 19,234,200 at 12:41:02   [Explorer tx ↗]                          |
| Attested: —                                                                       |
| Validators:                                                                       |
|   Attested: [val-02 ✅]                                                            |
|   Missing:  [val-01 ⏳] [val-03 ⏳] [val-04 ⏳]                                    |
+-----------------------------------------------------------------------------------+

IF NO PROPOSAL (relayer configured)
+-----------------------------------------------------------------------------------+
| No proposals found for this SafeTxHash on Base.                                   |
| [Submit Proposal]  (sponsored, no wallet required)                                |
+-----------------------------------------------------------------------------------+
```

**Status rules:**
- `attestedAt !== null` → **ATTESTED**
- `attestedAt === null && currentBlock - proposal.proposedAtBlock > signingTimeout` → **TIMED OUT**
- `attestedAt === null && currentBlock - proposal.proposedAtBlock <= signingTimeout` → **PROPOSED**

**Auto-refresh**: `useProposalsForTransaction` already polls at `settings.refetchInterval`. No
change required; proposals auto-refresh until they reach a terminal state.

---

## Tech Specs

### New Files

| File | Purpose |
|---|---|
| `explorer/src/lib/safe/wallet.ts` | `safeWalletTxUrl(shortName, safe, safeTxHash)` and `safeWalletSafeUrl(shortName, safe)` |
| `explorer/src/components/common/CopyButton.tsx` | Copy-to-clipboard button with brief ✓ confirmation |
| `explorer/src/components/transaction/SafeTxHeader.tsx` | Detail-page header: SafeTxHash, network, Safe address |
| `explorer/src/components/transaction/SafeTxSummary.tsx` | Labelled transaction body fields (op, to, value, calldata) |

### Modified Files

| File | Change |
|---|---|
| `explorer/src/lib/settings.ts` | Add `signingTimeout: number` field (default `12`) to settings schema |
| `explorer/src/hooks/useSafeTransactionDetails.tsx` | Extend return to `{ data: SafeTransaction \| null; fromSafeApi: boolean; isFetching: boolean }` |
| `explorer/src/routes/safeTx.tsx` | Use `SafeTxHeader` + `SafeTxSummary`; pass `fromSafeApi`, `currentBlock`, and `signingTimeout` |
| `explorer/src/components/transaction/SafeTxDataDetails.tsx` | Add `CopyButton` for raw calldata |
| `explorer/src/components/transaction/SafeTxProposals.tsx` | Numbered proposals, block-based timed-out status, updated CTA text; accept `currentBlock` and `signingTimeout` props |
| `explorer/src/components/transaction/SafeTxAttestationStatus.tsx` | Visual styling updates only (colour tokens, badge shapes); no label or logic changes |

### Reused Components & Utilities

| Existing item | File | Reused in |
|---|---|---|
| `InlineAddress` | `components/common/InlineAddress.tsx` | `SafeTxHeader` (Safe, To addresses) |
| `InlineBlockInfo`, `InlineExplorerTxLink` | `components/common/Info.tsx` | `SafeTxProposals` (unchanged) |
| `ValidatorList` | `components/common/ValidatorList.tsx` | `SafeTxAttestationStatus` (label text only changes) |
| `Box`, `BoxTitle` | `components/Groups.tsx` | All new section components |
| `opString`, `valueString`, `dataString` | `lib/safe/formatting.ts` | `SafeTxSummary` |
| `SAFE_SERVICE_CHAINS` | `lib/chains.ts` | `SafeTxHeader` for Safe Wallet URL and chain name |
| `useConsensusState` | `hooks/useConsensusState.tsx` | `safeTx.tsx` to get `currentEpoch` |
| `useSettings` | `hooks/useSettings.tsx` | `SafeTxSummary` for decoder URL |
| `useProposalsForTransaction` | `hooks/useProposalsForTransaction.tsx` | `SafeTxProposals` (unchanged) |

### Test Cases

| Test | File |
|---|---|
| `safeWalletTxUrl` — correct URL format | `lib/safe/wallet.test.ts` |
| `safeWalletSafeUrl` — correct URL format | `lib/safe/wallet.test.ts` |
| `CopyButton` — calls `navigator.clipboard.writeText` with correct value | `components/common/CopyButton.test.tsx` |
| `CopyButton` — shows ✓ confirmation after copy | `components/common/CopyButton.test.tsx` |
| `SafeTxHeader` — shows Safe Wallet tx link when `fromSafeApi=true` and chain supported | `components/transaction/SafeTxHeader.test.tsx` |
| `SafeTxHeader` — hides Safe Wallet tx link when `fromSafeApi=false` | `components/transaction/SafeTxHeader.test.tsx` |
| `SafeTxHeader` — hides Safe Wallet links for unsupported chainId | `components/transaction/SafeTxHeader.test.tsx` |
| `SafeTxSummary` — renders all labelled fields correctly | `components/transaction/SafeTxSummary.test.tsx` |
| `SafeTxProposals` — labels attested proposal "ATTESTED" | `components/transaction/SafeTxProposals.test.tsx` (update existing) |
| `SafeTxProposals` — labels proposal as "TIMED OUT" when `currentBlock - proposedAtBlock > signingTimeout` | `components/transaction/SafeTxProposals.test.tsx` |
| `SafeTxProposals` — labels proposal as "PROPOSED" when within `signingTimeout` blocks | `components/transaction/SafeTxProposals.test.tsx` |
| `SafeTxProposals` — numbers proposals starting at 1 | `components/transaction/SafeTxProposals.test.tsx` |
| `loadSettings` — `signingTimeout` defaults to 12 | `lib/settings.test.ts` |

---

## Implementation Phases

### Phase 1 — Safe TX Header Section (PR 1)

**What this covers:**
- Prominent SafeTxHash display with copy button and conditional Safe Wallet UI deep-link
- Network row: chain name + chainId
- Safe address with copy button and conditional Safe Wallet UI link

**Files touched:**
- `explorer/src/lib/safe/wallet.ts` — new
- `explorer/src/components/common/CopyButton.tsx` — new
- `explorer/src/components/transaction/SafeTxHeader.tsx` — new
- `explorer/src/hooks/useSafeTransactionDetails.tsx` — extend return type with `fromSafeApi`
- `explorer/src/routes/safeTx.tsx` — use `SafeTxHeader` in top `Box`
- Test files: `lib/safe/wallet.test.ts`, `components/common/CopyButton.test.tsx`, `components/transaction/SafeTxHeader.test.tsx`

**Components implemented:** `CopyButton`, `SafeTxHeader`

---

### Phase 2 — Transaction Summary Section (PR 2)

**What this covers:**
- Structured key-value rows for Operation, To, Value replacing prose `SafeTxOverview` output
- Calldata length, truncated raw calldata with copy button, decode link

**Files touched:**
- `explorer/src/components/transaction/SafeTxSummary.tsx` — new
- `explorer/src/components/transaction/SafeTxDataDetails.tsx` — add `CopyButton`
- `explorer/src/routes/safeTx.tsx` — replace second `Box` with `SafeTxSummary`
- Test files: `components/transaction/SafeTxSummary.test.tsx`

**Components implemented:** `SafeTxSummary`

---

### Phase 3 — Proposals UX Improvements (PR 3)

**What this covers:**
- Numbered "Proposal #N" headings
- TIMED OUT status using block-based `signingTimeout` setting (default 12 blocks)
- Visual styling improvements to `SafeTxAttestationStatus` (no label or logic changes)
- Updated "no proposals" message including chain name

**Files touched:**
- `explorer/src/lib/settings.ts` — add `signingTimeout` field
- `explorer/src/components/transaction/SafeTxProposals.tsx` — numbered proposals, block-based timed-out, updated CTA text, new `currentBlock`/`signingTimeout` props
- `explorer/src/components/transaction/SafeTxAttestationStatus.tsx` — styling only
- `explorer/src/routes/safeTx.tsx` — pass `currentBlock` and `signingTimeout` into `SafeTxProposals`
- Test files: `components/transaction/SafeTxProposals.test.tsx` (update existing)

---

## Open Questions / Assumptions

1. **Safe Wallet tx URL format**: The deep-link format
   `https://app.safe.global/transactions/tx?safe=${shortName}:${safe}&id=${safeTxHash}`
   is assumed to be stable. Only `lib/safe/wallet.ts` needs updating if it changes.

2. **`fromSafeApi` tracking**: The `useSafeTransactionDetails` hook resolves whichever of the two
   sources (Safe API or on-chain log) responds first via `Promise.any`. To track the source, each
   branch is wrapped with a tagged result `{ data, fromSafeApi: true/false }` before racing.

3. **`SafeTxOverview` in list**: `SafeTxOverview` remains in use for the transaction list items
   (via `RecentTransactionProposals`). It is not touched in any phase; the separate feature
   covering the list row redesign addresses its replacement there.
