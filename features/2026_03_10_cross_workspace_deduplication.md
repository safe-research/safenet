# Feature Proposal: Cross-Workspace Code Deduplication
Component: `all`

---

## Overview

The explorer and validator workspaces contain duplicated logic for SafeTx hashing, schema validation, and type definitions. This feature identifies these duplications and proposes a shared package to eliminate them, reducing the risk of the two implementations diverging.

**Phases:**

1. **Phase 1** — Audit and document all cross-workspace duplications
2. **Phase 2** — Extract shared SafeTx types and hashing into a common package
3. **Phase 3** — Migrate both workspaces to use the shared package

---

## Architecture Decision

Introduce a new workspace (`shared/` or `common/`) within the existing npm monorepo that contains shared types, schemas, and hashing utilities. Both `explorer` and `validator` would depend on this package.

### Duplicated Code Identified

#### SafeTx Hashing

The SafeTx EIP-712 hash computation is implemented independently in three places:

1. **Solidity** (`contracts/src/libraries/SafeTransaction.sol`): The canonical on-chain implementation using inline assembly for gas efficiency. Computes `EIP712Domain(uint256 chainId, address verifyingContract)` + `SafeTx(...)` struct hash.

2. **Validator** (`validator/src/consensus/verify/safeTx/hashing.ts`): TypeScript implementation using viem's `hashTypedData`. Defines `safeTxHash()` with the same EIP-712 types. This is the implementation validators use to verify transaction attestation requests.

3. **Explorer** (`explorer/src/lib/safe/hashing.ts`): TypeScript implementation using viem's `hashTypedData`. Defines `calculateSafeTxHash()` with the same EIP-712 types. This is used for display purposes.

The validator and explorer implementations are functionally identical but maintained separately. If one is updated without the other, transaction hash mismatches would cause silent failures.

#### SafeTransaction Type Definition

- **Validator**: `validator/src/consensus/verify/safeTx/schemas.ts` defines `SafeTransaction` via a Zod schema
- **Explorer**: `explorer/src/lib/consensus.ts` defines `SafeTransaction` via a Zod schema (`safeTransactionSchema`)
- Both define the same 12-field structure (`chainId`, `safe`, `to`, `value`, `data`, `operation`, `safeTxGas`, `baseGas`, `gasPrice`, `gasToken`, `refundReceiver`, `nonce`)

#### BigInt / Hex Schemas

Both workspaces define similar Zod schemas for parsing `bigint` and `Hex` values from JSON/RPC responses.

### Alternatives Considered

| Alternative | Reason rejected |
|---|---|
| Copy-paste with a comment "keep in sync" | Error-prone; the whole point of this feature is to eliminate this pattern |
| NPM package published to a registry | Overkill for an internal monorepo; workspace dependencies are simpler |
| Symbolic links between workspaces | Fragile, doesn't work well with TypeScript module resolution |
| Only deduplicate at the type level | Insufficient — the hashing logic is the highest-risk duplication |

---

## Tech Specs

### Proposed Shared Package

**Location:** `shared/` workspace (added to root `package.json` workspaces array)

**Contents:**

| File | Exports |
|---|---|
| `shared/src/safe/types.ts` | `SafeTransaction` type, `SafeTransactionSchema` Zod schema |
| `shared/src/safe/hashing.ts` | `safeTxHash(transaction: SafeTransaction): Hex` |
| `shared/src/schemas.ts` | `bigIntSchema`, `hexDataSchema`, `checkedAddressSchema` |

**Dependencies:** `viem`, `zod` (already used by both workspaces)

### Cross-Workspace Test

A critical test should verify that the TypeScript `safeTxHash` produces the same output as the Solidity `SafeTransaction.hash()` for a set of reference vectors. This test already exists partially in `explorer/src/lib/safe/hashing.test.ts` and `contracts/test/libraries/SafeTransaction.t.sol`, but there is no cross-verification.

---

## Implementation Phases

### Phase 1 — Audit and Document (independent PR)

**Scope:** Add a comment to each duplicated location referencing the other implementations, and create a tracking issue. No code changes.

**Files touched:**
- `validator/src/consensus/verify/safeTx/hashing.ts` — add reference comment
- `explorer/src/lib/safe/hashing.ts` — add reference comment

---

### Phase 2 — Create Shared Package (independent PR)

**Scope:** Create the `shared/` workspace with types, schemas, and hashing.

**Files touched:**
- `shared/package.json` — new
- `shared/tsconfig.json` — new
- `shared/src/safe/types.ts` — new (extracted from validator + explorer)
- `shared/src/safe/hashing.ts` — new (extracted from validator + explorer)
- `shared/src/safe/hashing.test.ts` — new (consolidated tests with reference vectors from Solidity tests)
- `shared/src/schemas.ts` — new (extracted common Zod schemas)
- `shared/src/schemas.test.ts` — new
- `package.json` — add `shared` to workspaces

---

### Phase 3 — Migrate Workspaces (depends on Phase 2)

**Scope:** Update explorer and validator to import from the shared package instead of their local copies.

**Files touched:**
- `explorer/package.json` — add `shared` dependency
- `explorer/src/lib/safe/hashing.ts` — re-export from shared
- `explorer/src/lib/consensus.ts` — import `SafeTransaction` from shared
- `explorer/src/lib/schemas.ts` — re-export from shared
- `validator/package.json` — add `shared` dependency
- `validator/src/consensus/verify/safeTx/hashing.ts` — re-export from shared
- `validator/src/consensus/verify/safeTx/schemas.ts` — import from shared

---

## Open Questions / Assumptions

1. **Package name**: Should the shared package be named `@safenet/shared`, `@safenet/common`, or just `shared`? The naming should follow npm workspace conventions used in the project.
2. **Build tooling**: Does the shared package need its own build step, or can workspaces consume TypeScript sources directly via `tsconfig` paths? The latter is simpler for a monorepo.
3. **Solidity reference vectors**: The cross-verification test needs reference hash values from the Solidity tests. These should be extracted from `contracts/test/libraries/SafeTransaction.t.sol` as test fixtures.
4. **Scope of shared schemas**: Should only the schemas that are actually duplicated be shared, or should this be an opportunity to consolidate all Zod schemas? Starting with only the duplicated ones is safer.
