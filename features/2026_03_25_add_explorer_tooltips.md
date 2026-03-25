# Feature Proposal: Add Explorer Tooltips
Component: `explorer`

---

## Overview

The explorer's proposal detail view and validator participation lists expose useful identifiers (Signature ID, Group ID, validator addresses) only in the fetched data, but not in the UI. Users who need to cross-reference on-chain logs or verify validator identities currently have no way to access these values from the explorer.

This feature adds two targeted tooltip enhancements in a single PR:

1. **Proposal details** — Signature ID (`sid`) and Group ID (`groupId`) are shown in the attestation status section with copy buttons.
2. **Validator labels** — hovering over any validator name in a participation list shows a native browser tooltip containing the full checksummed address.

---

## Architecture Decision

### Proposal Signature ID / Group ID

`AttestationStatus.sid` and `AttestationStatus.groupId` are already fetched in `SafeTxAttestationStatus` but unused in the render output. The simplest change is to add two labeled rows (following the same `md:flex md:justify-between` layout used throughout the component) using the existing `InlineHash` component for display and the existing `CopyButton` component for copy-to-clipboard.

No new components are needed.

### Validator Label Address Tooltip

`ValidatorList` currently builds an array of display strings and joins them into a single text node. To support per-item tooltips, the render is refactored to emit a `<Fragment>` containing `<span title={address}>` elements separated by ", " text. The `mapInfo` function API and all call sites are unchanged; addresses are sourced directly from the `active` and `all` arrays that `ValidatorList` already receives.

The `title` attribute provides a native browser tooltip without requiring a third-party tooltip library, consistent with the existing pattern in `Badge` and `NetworkBadge`.

### Alternatives Considered

- **Custom floating tooltip component**: Would give more control over styling and interactivity (e.g. click-to-copy for addresses). Rejected as over-engineering for this scope — native `title` is sufficient for showing an address on hover.
- **Displaying full addresses inline**: Would clutter the validator list. Rejected in favour of the tooltip approach.
- **Showing sid/groupId as an overlay tooltip on "Proposal #N"**: The data is only available after `useAttestationStatus` resolves inside `SafeTxAttestationStatus`. Surfacing it as labeled rows within that component avoids lifting state and keeps the loading skeleton behaviour intact.

---

## User Flow

### Proposal Detail — Signature ID / Group ID

User navigates to a Safe transaction detail page. For each proposal card:
- The attestation status section now shows "Signature ID" and "Group ID" rows
- Each row displays a truncated hex value (`InlineHash`) and a copy icon (`CopyButton`)
- Clicking the copy icon copies the full hex string to clipboard

### Validator Labels — Address Tooltip

User hovers over any validator name (e.g. "Alice ✅") in the Committed or Attested lists:
- A native browser tooltip appears showing the full checksummed address (e.g. `0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045`)

### Page Layout

```
┌──────────────────────────────────────────────────┐
│ Proposal #1                                       │
│ Status:          ATTESTED                         │
│ Proposed:        Block 100 at …  [Explorer Tx]    │
│ Attested:        Block 110 at …  [Explorer Tx]    │
│ Signature ID:    0x1234abcd…5678efab  [📋]        │  ← new
│ Group ID:        0xdeadbeef…cafebabe  [📋]        │  ← new
│ Validators:                                       │
│   Committed:     Alice ✅, Bob ✅                 │
│   Attested:      Alice ✅, Bob ✅                 │
│                  ^ hover shows full address       │  ← new
└──────────────────────────────────────────────────┘
```

---

## Tech Specs

### Components Modified

| File | Change |
|---|---|
| `explorer/src/components/transaction/SafeTxAttestationStatus.tsx` | Add Signature ID and Group ID rows using `InlineHash` + `CopyButton` |
| `explorer/src/components/common/ValidatorList.tsx` | Refactor render to emit `<span title={address}>` JSX elements |

### Components Reused (no changes)

| Component | Path |
|---|---|
| `InlineHash` | `explorer/src/components/common/InlineHash.tsx` |
| `CopyButton` | `explorer/src/components/common/CopyButton.tsx` |

### New Test Files

| File | Coverage |
|---|---|
| `explorer/src/components/common/ValidatorList.test.tsx` | Validator label render, address title attributes, emoji suffixes, sort order |

### No New Routes or Environment Variables

---

## Implementation Phases

### Phase 1 (this PR) — All changes together

The two changes are small and tightly related (both concern surfacing existing data in the explorer UI). Shipping them together keeps the PR reviewable.

**Files touched:**
- `explorer/src/components/transaction/SafeTxAttestationStatus.tsx`
- `explorer/src/components/common/ValidatorList.tsx`
- `explorer/src/components/common/ValidatorList.test.tsx` (new)
- `features/2026_03_25_add_explorer_tooltips.md` (this file)

---

## Open Questions / Assumptions

- **Assumption**: Native `title` tooltips are acceptable for validator addresses. If a richer tooltip UX is desired (e.g. click-to-copy address), a follow-up feature can introduce a shared `Tooltip` component.
- **Assumption**: Showing Signature ID and Group ID as always-visible labeled rows (rather than a hover overlay) is appropriate, since these values are relevant to debugging and verification workflows.
