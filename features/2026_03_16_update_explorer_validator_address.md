# Feature Proposal: Update Explorer Validator Address Events
Component: `explorer`

---

## Overview

The contracts workspace removed the `FROST.Identifier` (`uint256`) type from all coordinator events in favour of emitting the validator `address` directly (PRs #279 and #280). This change updates the explorer workspace to match the new event signatures and removes all identifier-based types throughout the data layer and UI components.

Single PR: the change is self-contained within the explorer workspace and can be shipped in one review.

---

## Architecture Decision

All coordinator events that previously carried a `uint256 identifier` parameter now carry an `address participant` (or `address plaintiff` / `address accused` for complaint events). The explorer previously maintained a `Map<bigint, ValidatorInfo>` keyed by identifier to resolve display labels; this map is now keyed by `Address`.

The `ValidatorInfo` type loses its `identifier` field since the address is now the sole stable identifier. Components and hooks that previously passed `bigint[]` for validator lists are updated to pass `Address[]`.

### Alternatives Considered

- **Keep identifier in ValidatorInfo and derive it from the address**: Rejected — the identifier is now an internal contract detail, not a useful display concept. Removing it simplifies the type and the JSON configuration format.
- **Map by both address and identifier**: Rejected — adds unnecessary complexity and the identifier is no longer surfaced in events.

---

## User Flow

No user-visible behaviour change. Validator labels continue to be resolved from the `validatorInfo` JSON configuration. Validators without a label entry now display a shortened address (`0x1234…abcd`) instead of `Validator N`.

---

## Tech Specs

### Updated event signatures

| Event | Old | New |
|---|---|---|
| `KeyGenCommitted` | `(gid, uint256 identifier, address participant, commitment, committed)` | `(gid, address participant, commitment, committed)` |
| `KeyGenSecretShared` | `(gid, uint256 identifier, share, shared)` | `(gid, address participant, share, shared)` |
| `KeyGenConfirmed` | `(gid, uint256 identifier, confirmed)` | `(gid, address participant, confirmed)` |
| `KeyGenComplained` | `(gid, uint256 plaintiff, uint256 accused, compromised)` | `(gid, address plaintiff, address accused, compromised)` |
| `KeyGenComplaintResponded` | `(gid, uint256 plaintiff, uint256 accused, secretShare)` | `(gid, address plaintiff, address accused, secretShare)` |
| `SignRevealedNonces` | `(sid, uint256 identifier, nonces)` | `(sid, address participant, nonces)` |
| `SignShared` | `(sid, selectionRoot, uint256 identifier, z)` | `(sid, selectionRoot, address participant, z)` |
| `Preprocess` | `(gid, uint256 identifier, chunk, commitment)` | `(gid, address participant, chunk, commitment)` |

### Modified files

- `explorer/src/lib/coordinator/abi.ts`
- `explorer/src/lib/validators/info.ts`
- `explorer/src/lib/coordinator/keygen.ts`
- `explorer/src/lib/coordinator/signing.ts`
- `explorer/src/components/common/ValidatorList.tsx`
- `explorer/src/components/epoch/KeyGenStatusItem.tsx`
- `explorer/src/components/transaction/SafeTxAttestationStatus.tsx`
- `explorer/src/hooks/useValidatorInfo.tsx`
- `explorer/src/lib/coordinator/keygen.test.ts`

---

## Implementation Phases

### Phase 1 (single PR)

All changes are within the explorer workspace and are tightly coupled (type changes cascade from ABI → participation types → components). Ship as one PR.

Files touched and changes:
1. `abi.ts` — update 8 event signatures
2. `info.ts` — remove `identifier` field, rekey map by address
3. `keygen.ts` — `KeyGenParticipation.address`, update `toParticipation`
4. `signing.ts` — `AttestationParticipation.address`, update event aggregation
5. `ValidatorList.tsx` — switch from `bigint` to `Address`, update fallback label
6. `KeyGenStatusItem.tsx` — use `p.address` instead of `p.identifier`
7. `SafeTxAttestationStatus.tsx` — use `s.address` instead of `s.identifier`
8. `useValidatorInfo.tsx` — update generic type
9. `keygen.test.ts` — update mock encoders and assertions to use addresses

---

## Open Questions / Assumptions

- Assumes contract deployments have already been updated to emit the new event signatures. The explorer will not decode events from old deployments correctly after this change.
- The `validatorInfo` JSON configuration files used in deployments should have the `identifier` field removed (or it will simply be ignored by the updated schema).
