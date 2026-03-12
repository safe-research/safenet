# Feature Proposal: Use Validator Address as FROST Identifier
Component: `all`

---

## Overview

Replace the separate sequential `participantId` / `FROST.Identifier` with the validator's Ethereum address cast to a `uint256` scalar (`uint256(uint160(address))`). This eliminates the auxiliary identifier mapping between address and ID, reduces ceremony complexity, and makes the on-chain and off-chain participant identification consistent and self-describing.

The change affects three layers: the Solidity contracts (identifier derivation and Merkle leaf format), the TypeScript validator service (type system, storage backends, and cryptographic clients), and all associated tests. It is a **breaking protocol change** — all existing deployed FROST groups are invalidated and must be rotated.

Three phases can be reviewed as separate PRs:
1. **Contract changes** — remove explicit identifier parameter, derive from sender.
2. **Validator TypeScript changes** — remove `ParticipantId`, update storage and crypto clients.
3. **Test and integration updates** — update test utilities and integration test fixtures.

---

## Architecture Decision

### Why address-as-identifier is cryptographically valid

The FROST protocol (RFC-9591) requires participant identifiers to be non-zero elements of the scalar field, i.e. values in the range `[1, n-1]` where `n` is the secp256k1 curve order (`n ≈ 1.16 × 10^77 ≈ 2^256`). Ethereum addresses are 20-byte (`uint160`) values.

- **Non-zero**: Valid validator addresses are never the zero address. ✓
- **Below curve order**: `uint160 max ≈ 1.46 × 10^48 << n`. All addresses satisfy `address < n`. ✓
- **Unique**: Ethereum addresses are globally unique by design, preventing identifier collisions. ✓
- **FROST scalar arithmetic**: Lagrange coefficient computation uses identifiers as field elements in modular arithmetic (`mulmod`, `submod`, `divmod` mod `n`). Any unique non-zero value below `n` is valid — sequential integers were never required. ✓

The proposed scalar conversion is: `identifier = uint256(uint160(validatorAddress))`.

### Changes to identifier handling

Currently `FROSTCoordinator.keyGenCommit` accepts an explicit `identifier` parameter supplied by the participant. Under the proposed design the identifier is **derived deterministically from `msg.sender`**, eliminating the ability for participants to choose or forge their identifier.

The `FROSTParticipantMap` currently stores a bidirectional mapping between address and identifier. After this change, the identifier is always computable from the address, so only the `address → key` direction is needed; the reverse mapping is a no-op.

### Merkle leaf format change

Current leaf (both on-chain and off-chain):
```
keccak256(bytes32(identifier), bytes32(uint256(uint160(address))))
```
Where `identifier` is a sequential integer (1, 2, 3, …).

Proposed leaf (since identifier is now derived from address):
```
keccak256(bytes32(uint256(uint160(address))))
```
The leaf encodes only the address as a padded `bytes32` scalar. The redundant second field is dropped.

The off-chain Typescript equivalent changes from:
```typescript
keccak256(encodePacked(["uint256", "uint256"], [p.id, BigInt(p.address)]))
```
to:
```typescript
keccak256(encodePacked(["uint256"], [BigInt(p.address)]))
```

### Commitment ordering

`FROST._encodeCommitments` requires commitments to be ordered by identifier value (`require(identifier > previousIdentifier)`). With sequential IDs the natural submission order was 1, 2, 3, … With address-derived identifiers, ordering is by address value. Validators must sort commitments by ascending `uint160(address)` before submitting nonce reveals and signature shares. This is a deterministic sort that all participants can compute independently.

### Alternatives Considered

- **Hash-derived identifier**: Use `uint256(keccak256(address)) % n` for a uniformly distributed 256-bit scalar. Rejected: adds computation overhead, less transparent, and offers no practical security advantage since the simple cast already satisfies all FROST requirements.
- **Keep sequential IDs but derive from on-chain staking registry**: Assign IDs based on validator registration order in `Staking.sol`. Rejected: introduces ordering ambiguity across chains, adds coordination complexity, and is no simpler than addresses.
- **Keep the current design**: No action. Rejected: the separate ID requires extra mapping maintenance, increases ceremony parameter surface area, and makes debug output less human-readable.

---

## Tech Specs

### Solidity contract changes

#### `contracts/src/libraries/FROST.sol`
- No changes to `Identifier` type (`type Identifier is uint256`) or hash functions.
- `requireValidIdentifier` already accepts any non-zero value — no change required.

#### `contracts/src/libraries/FROSTParticipantMap.sol`
- `register(identifier, participant, poap)` → `register(participant, poap)`:
  - Remove `identifier` parameter.
  - Derive internally: `FROST.Identifier identifier = FROST.Identifier.wrap(uint256(uint160(participant)));`
  - Update Merkle leaf computation to hash only the address:
    ```solidity
    bytes32 leaf = keccak256(abi.encodePacked(bytes32(uint256(uint160(participant)))));
    ```
- `struct T`: remove `mapping(address => FROST.Identifier) identifiers` (address → id lookup is replaced by deterministic derivation); keep `mapping(FROST.Identifier => Secp256k1.Point) keys` and `mapping(FROST.Identifier => ParticipantState) states`.
- `identifierOf(address participant)` → becomes a pure computation: `return FROST.Identifier.wrap(uint256(uint160(participant)));`. Still needed as a helper.
- `isParticipating(FROST.Identifier identifier)` remains unchanged.
- `set(participant, y)` remains unchanged (calls `identifierOf` internally).
- All complaint/confirmation functions operate on identifiers and remain unchanged.

#### `contracts/src/FROSTCoordinator.sol`
- `keyGenCommit(gid, identifier, poap, commitment)` → `keyGenCommit(gid, poap, commitment)`:
  - Remove `identifier` parameter.
  - Derive identifier before calling `group.participants.register`:
    ```solidity
    FROST.Identifier identifier = FROST.Identifier.wrap(uint256(uint160(msg.sender)));
    group.participants.register(msg.sender, poap);
    ```
  - All downstream event emissions retain `identifier` (now derived).
- `keyGenComplain` and `keyGenComplaintRespond`: currently use `group.participants.identifierOf(msg.sender)` to derive the caller's identifier — **no change needed**.
- `keyGenConfirm`: same — **no change needed**.
- `signRevealNonces` and `signShare`: use `identifierOf(msg.sender)` — **no change needed**.
- Event signatures do **not** change (still emit `FROST.Identifier`).

#### `contracts/test/util/ParticipantMerkleTree.sol`
- Remove sequential identifier assignment (`FROST.newIdentifier(i + 1)`).
- Derive identifier from address: `FROST.Identifier identifier = FROST.Identifier.wrap(uint256(uint160(participant)));`
- Update leaf hash to match new format.
- Remove the `$identifiers` mapping; keep `$addresses` (identifier → address) as `identifier` is now directly computable from the address but is useful for test lookups.
- Update `proof(uint256 identifier)` to locate the leaf index by address value rather than sequential position.
- Remove assertion `participant > last` (address ordering no longer implied by identifier order; replace with assertion that identifier is unique).

#### `contracts/test/FROSTCoordinator.t.sol`
- Remove `identifier` argument from all `keyGenCommit` calls.
- Regenerate Merkle proofs using updated `ParticipantMerkleTree`.

### TypeScript validator changes

#### `validator/src/frost/types.ts`
- Remove `export type ParticipantId = bigint;`

#### `validator/src/consensus/storage/types.ts`
- `Participant` type: remove `id: ParticipantId` field:
  ```typescript
  export type Participant = {
    address: Address;
  };
  ```
- `GroupInfoStorage`:
  - `registerGroup(...): ParticipantId` → `registerGroup(...): Address` (returns the node's own address for downstream use).
  - `participantId(groupId: GroupId): ParticipantId` → remove or replace with `participantAddress(groupId: GroupId): Address`.
- `KeyGenInfoStorage`: Replace all `ParticipantId` keys with `Address`:
  - `registerCommitments(groupId, participantId, ...)` → `(groupId, participantAddress, ...)`
  - `registerSecretShare(groupId, participantId, share)` → `(groupId, participantAddress, share)`
  - `missingCommitments(groupId): ParticipantId[]` → `Address[]`
  - `missingSecretShares(groupId): ParticipantId[]` → `Address[]`
  - `encryptionPublicKey(groupId, participantId)` → `(groupId, participantAddress)`
  - `commitments(groupId, participantId)` → `(groupId, participantAddress)`
  - `commitmentsMap(groupId): Map<ParticipantId, ...>` → `Map<Address, ...>`
  - `secretSharesMap(groupId): Map<ParticipantId, ...>` → `Map<Address, ...>`
- `SignatureRequestStorage`: Replace `ParticipantId` with `Address`:
  - `registerSignatureRequest(..., signers: readonly ParticipantId[], ...)` → `Address[]`
  - `registerNonceCommitments(..., signerId: ParticipantId, ...)` → `Address`
  - `missingNonces(signatureId): ParticipantId[]` → `Address[]`
  - `signers(signatureId): ParticipantId[]` → `Address[]`
  - `nonceCommitmentsMap(signatureId): Map<ParticipantId, ...>` → `Map<Address, ...>`

#### `validator/src/types/schemas.ts`
- Update `participantsSchema` to remove sequential ID assignment:
  ```typescript
  const participantsSchema = z
    .preprocess(...)
    .array(checkedAddressSchema)
    .transform((participants) => participants.map((address) => ({ address })));
  ```

#### `validator/src/consensus/merkle.ts`
- `hashParticipant`:
  ```typescript
  export const hashParticipant = (p: Participant): Hex =>
    keccak256(encodePacked(["uint256"], [BigInt(p.address)]));
  ```
- `generateParticipantProof(participants, participantAddress: Address)`:
  - Change signature from `participantId: ParticipantId` to `participantAddress: Address`.
  - Lookup by address: `participants.findIndex((p) => p.address === participantAddress)`.

#### `validator/src/consensus/signing/group.ts`
- `lagrangeCoefficient` takes `bigint[]` and works unchanged — callers change what they pass in.

#### `validator/src/consensus/keyGen/client.ts`
- Replace all `participant.id` usages with `BigInt(participant.address)`:
  - `evalPoly(coefficients, BigInt(participant.address))` for secret share generation.
  - Skip self: `if (participant.address === this.#account) continue;`
  - `generateParticipantProof(participants, this.#account)` (pass address, not ID).
- `this.#storage.participantId(groupId)` → `this.#storage.participantAddress(groupId)`.

#### `validator/src/consensus/signing/client.ts`
- All `ParticipantId` references replaced with `Address`.
- `signers`, `missingNonces`, `nonceCommitmentsMap` now use `Address` as keys/elements.
- Lagrange coefficient calls: `lagrangeCoefficient(signers.map(BigInt), BigInt(ownAddress))`.

#### `validator/src/machine/types.ts` and `validator/src/machine/transitions/types.ts`
- Replace all `ParticipantId` type references with `Address`.
- `KeyGenCommittedEvent.identifier: ParticipantId` → `Address`.
- Complaint transitions: `plaintiff: ParticipantId` → `Address`, `accused: ParticipantId` → `Address`.

#### `validator/src/machine/keygen/secretShares.ts`
- Update participant iteration to use `BigInt(participant.address)` as polynomial evaluation point.

#### `validator/src/machine/signing/timeouts.ts`
- Replace `signingClient.participantId(groupId)` with `signingClient.participantAddress(groupId)` for comparison logic.

#### `validator/src/machine/storage/schemas.ts`
- Update Zod schemas that parse `id` integer fields: replace with address parsing.

#### `validator/src/consensus/storage/inmemory.ts`
- `GroupInfo.participantId: bigint` → `participantAddress: Address`.
- All `Map<ParticipantId, ...>` → `Map<Address, ...>`.
- `checkInformationComplete` iterates `Address[]` instead of `ParticipantId[]`.
- `registerGroup` returns `Address` instead of `ParticipantId`.

#### `validator/src/consensus/storage/sqlite.ts`
- Schema migration: `group_participants.id INTEGER` → `group_participants.address TEXT PRIMARY KEY(group_id, address)` (the `id` column is removed; `address` is already present and becomes the sole identifier).
- `group_secret_shares.from_participant INTEGER` → `from_participant TEXT` (address).
- `signature_commitments.signer INTEGER` → `signer TEXT` (address).
- Remove `dbIntegerSchema` usage for participant IDs; use `checkedAddressSchema` instead.
- `dbParticipantSchema`: remove `id` field; `address` remains.
- `dbCommitmentsSchema`: `id: dbIntegerSchema` → `address: checkedAddressSchema`.
- `dbSecretShareSchema`: `id: dbIntegerSchema` → `address: checkedAddressSchema`.
- `dbSignatureCommitmentSchema`: `signer: dbIntegerSchema` → `signer: checkedAddressSchema`.
- `registerGroup`: remove participant `id` from INSERT; `insertParticipant` becomes `INSERT INTO group_participants (group_id, address) VALUES (?, ?)`.
- All queries that JOIN or filter by `id` are updated to use `address`.

#### `validator/src/consensus/protocol/onchain.ts`
- Remove `identifier` argument from the `keyGenAndCommit` ABI call (matches the contract change).
- For complaint actions, convert `accusedAddress: Address` → `BigInt(accusedAddress)` when calling the contract.
- Update action type references from `participantId` to `participantAddress`.

#### `validator/src/consensus/protocol/sqlite.ts`
- Update Zod schemas for serialized protocol actions: `participantId` integer fields become address strings.
- `startKeyGenSchema.participantId` → `participantAddress: checkedAddressSchema`.
- `keyGenComplainSchema.accused` → `accusedAddress: checkedAddressSchema`.
- `keyGenComplaintResponseSchema.plaintiff` → `plaintiffAddress: checkedAddressSchema`.
- **Migration risk**: Any pending serialized actions in the queue at upgrade time will fail to deserialize. The queue must be drained or the database cleared as part of the upgrade procedure.

#### `validator/src/types/abis.ts`
- Remove the `identifier` input from the `keyGenAndCommit` / `keyGenCommit` ABI entry.

### No changes needed

- `certora/` — Certora specs only cover `Staking.sol` and do not reference FROST identifiers.
- `explorer/` — The explorer does not interact with participant IDs directly; it reads pre-computed state from the validator via HTTP.
- `contracts/src/libraries/FROSTSignatureShares.sol`, `FROSTNonceCommitmentSet.sol`, `FROSTGroupId.sol` — use FROST identifiers only through downstream methods that already derive from address.

---

## Implementation Phases

### Phase 1 — Contract changes (PR 1)

**What this covers:** Update `FROSTParticipantMap` and `FROSTCoordinator` to derive the FROST identifier from `msg.sender`. Update Merkle leaf format. Update test utilities.

**Files touched:**
- `contracts/src/libraries/FROSTParticipantMap.sol` — remove `identifier` param from `register`, update leaf hash, update `struct T`
- `contracts/src/FROSTCoordinator.sol` — remove `identifier` param from `keyGenCommit`
- `contracts/test/util/ParticipantMerkleTree.sol` — derive identifier from address, update leaf hash
- `contracts/test/FROSTCoordinator.t.sol` — remove identifier args, regenerate Merkle proofs

**Verification:** `forge test` passes; `forge lint` passes.

---

### Phase 2 — TypeScript validator changes (PR 2, depends on Phase 1)

**What this covers:** Remove `ParticipantId` type, update `Participant` to address-only, update all storage interfaces (InMemory + SQLite), update crypto client code, update Merkle helpers.

**Files touched:**
- `validator/src/frost/types.ts` — remove `ParticipantId`
- `validator/src/consensus/storage/types.ts` — update `Participant`, all storage interfaces
- `validator/src/types/schemas.ts` — remove sequential ID assignment in `participantsSchema`
- `validator/src/consensus/merkle.ts` — update `hashParticipant`, `generateParticipantProof`
- `validator/src/consensus/keyGen/client.ts` — replace `participant.id` with `BigInt(participant.address)`
- `validator/src/consensus/signing/client.ts` — replace `ParticipantId` with `Address`
- `validator/src/consensus/signing/group.ts` — callers change inputs; function body unchanged
- `validator/src/consensus/storage/inmemory.ts` — update all Maps and return types
- `validator/src/consensus/storage/sqlite.ts` — update schema and all queries
- `validator/src/consensus/protocol/onchain.ts` — remove `identifier` arg from ABI call, update action types
- `validator/src/consensus/protocol/sqlite.ts` — update serialized action schemas; drain queue on upgrade
- `validator/src/types/abis.ts` — remove `identifier` from `keyGenCommit` ABI entry
- `validator/src/machine/types.ts` — replace `ParticipantId` with `Address`
- `validator/src/machine/transitions/types.ts` — replace `ParticipantId` with `Address`
- `validator/src/machine/keygen/secretShares.ts` — use `BigInt(participant.address)` for polynomial eval
- `validator/src/machine/keygen/complaintSubmitted.ts` — filter by address instead of id
- `validator/src/machine/signing/timeouts.ts` — use `participantAddress` instead of `participantId`
- `validator/src/machine/storage/schemas.ts` — update Zod schemas

**Verification:** `npm test -w validator` passes; `npm run check` passes.

---

### Phase 3 — Test and integration updates (PR 3, depends on Phase 2)

**What this covers:** Update unit test fixtures and integration test scripts to use address-based participants. Validate end-to-end devnet flow.

**Files touched:**
- `validator/src/__tests__/data/machine.ts` — update `makeGroupSetup(participantId)` signature
- `validator/src/__tests__/data/protocol.ts` — replace `participantId: 1n` with addresses
- All `*.test.ts` files referencing `participantId` — update to use addresses
- `scripts/run_integration_test.sh` — verify participant config format is unchanged (addresses only)

**Verification:** `npm test` (all workspaces) passes; `npm run test:integration` passes.

---

## Potential Pitfalls

### P1 — Breaking protocol change (hard migration)
All existing deployed FROST groups computed with sequential identifiers are invalid after this change. The DKG outputs (group public keys, signing shares, verification shares) and nonce preprocessing data are all identifier-dependent. There is no in-place migration path — all active groups must be dissolved and new key generation ceremonies initiated after deployment.

### P2 — Commitment ordering by address value
`FROST._encodeCommitments` enforces strictly ascending identifier order. With sequential IDs the order was 1, 2, 3 — predictable from config list position. With address-derived identifiers, the sort order is by `uint160(address)` value. The `ParticipantMerkleTree` test helper currently asserts `participant > last` (ascending address order in the constructor). The validator service must sort commitments by ascending address before submitting nonce reveals and signature shares, and the test tree must be constructed with sorted participant arrays.

### P3 — SQLite INTEGER overflow for address-as-bigint
SQLite `INTEGER` is a signed 64-bit value (max ≈ 9.2 × 10^18). Ethereum addresses are 160-bit unsigned values (max ≈ 1.46 × 10^48), which overflow SQLite `INTEGER`. The `dbIntegerSchema` used for participant IDs today silently truncates large values. Changing the column type to `TEXT` and parsing with `checkedAddressSchema` avoids this. The schema migration drops and recreates affected tables (acceptable given the breaking protocol change above).

### P4 — Lagrange coefficient computation with 160-bit inputs
The current implementation uses 256-bit field arithmetic. Address-derived scalars occupy only 160 bits. The math is correct but developers should be aware that group operations on smaller scalars may differ from published test vectors that use 256-bit random scalars. There is no security reduction, but test vectors derived from sequential IDs will no longer match.

### P5 — Zero-address validator
`requireValidIdentifier` rejects `identifier == 0`, which corresponds to the zero address. The zero address is not a valid Ethereum account (no private key), so this case should never arise in practice. No mitigation is needed, but validators should never be registered with the zero address in `Staking.sol`.

### P6 — ABI and event signature changes
Removing the `identifier` parameter from `keyGenCommit` changes the function selector and ABI. Any external callers or off-chain listeners relying on the old ABI (e.g. explorer event indexers or monitoring tools) must be updated. The `KeyGenCommitted` event retains the `identifier` field (now derived on-chain) so its ABI is unchanged, but event parsing libraries that previously decoded the submitted identifier from calldata will need to read it from the event log instead.

### P7 — `from_participant` column in secret shares table
The `group_secret_shares` table stores `from_participant INTEGER` to identify which participant sent a secret share. This must be migrated to `TEXT` (address) for the same overflow reason as `group_participants.id`. All queries joining on this column require corresponding updates.

### P8 — Nonce preprocessing
Nonce commitments in `FROSTNonceCommitmentSet` are indexed by `FROST.Identifier`. The contract derives the identifier from `msg.sender` already (via `identifierOf`), so preprocessing calls are unaffected at the contract level. The TypeScript nonce storage and lookup, however, must be verified to use addresses consistently after the storage interface changes.

### P9 — `AlreadyRegistered` check loses its mechanism
`FROSTParticipantMap.register` currently detects double-registration by checking `self.identifiers[participant] == 0`. Once the `identifiers` mapping is removed this check no longer has a backing store. The keys mapping (`self.keys[identifierOf(participant)]`) is only populated in Round 2, so it cannot serve as a Round 1 sentinel. A minimal fix is to add a `mapping(address => bool) registered` boolean field to `struct T`. This adds one extra `SSTORE` per registration but preserves the invariant cleanly. This design choice must be resolved before Phase 1 implementation begins.

### P10 — `identifierOf` participation guard is lost
`FROSTParticipantMap.identifierOf` currently reverts with `NotParticipating` if the participant's address is absent from the `identifiers` mapping. After the mapping is removed, `identifierOf` becomes a pure derivation function that always succeeds. All callers that relied on the revert as a participation guard — `keyGenConfirm`, `preprocess`, `signRevealNonces`, `signShare` — must call a separate `requireParticipating(self, address)` helper (e.g. checking that the `keys` mapping for the derived identifier is non-zero) before proceeding. Missing this guard would allow unregistered addresses to trigger state transitions.

### P11 — `SqliteActionQueue` pending action deserialization
The protocol action queue (`consensus/protocol/sqlite.ts`) serializes pending actions (including `startKeyGen`, `complain`, `complaintResponse`) to a SQLite table using integer-typed `participantId`, `accused`, and `plaintiff` fields. After the upgrade, these fields become address strings. Any actions serialized in the old format that remain in the queue at upgrade time will fail to parse and cause the validator to crash on startup. The upgrade procedure must drain or wipe the action queue, or a forward-migration is provided for the old integer format.

---

## Open Questions / Assumptions

1. **Leaf encoding format**: The proposed leaf is `keccak256(abi.encodePacked(bytes32(uint256(uint160(address)))))` — a single 32-byte field. An alternative is to reuse `Hashes.efficientKeccak256` with the address padded into both slots (redundant). The single-field approach is cleaner and should be confirmed before implementation.

2. **`identifierOf` helper retention**: After the change, `identifierOf` is a pure function (`return Identifier.wrap(uint256(uint160(participant)))`). It may be kept as a helper or inlined at call sites. This is a readability decision for implementation.

3. **Explorer `ValidatorInfo` type**: `explorer/src/lib/validators/info.ts` exposes `ValidatorInfo.identifier: bigint` (the participant ID) alongside the address. After this change the identifier is redundant — it equals `BigInt(address)`. This field should either be removed or kept as a derived convenience. The display logic showing a numeric identifier to users should be updated to show the address instead. This is a UX follow-up and does not block the protocol change.

4. **Devnet and integration test setup**: The integration test script passes participants as a comma-separated `PARTICIPANTS` env var (addresses only). This format is unchanged. Confirm that no scripts inject sequential IDs separately.

5. **Assumption — single EOA per node**: The current design assumes each validator node controls exactly one address. The existing SQLite comment (`// TODO: not possible to correctly support multiple participant IDs managed by the same EOA`) is resolved by this change since the address is now the canonical identifier. If multi-address nodes are needed in the future, a separate design is required.

6. **`AlreadyRegistered` replacement mechanism**: With the `identifiers` mapping removed, the recommended approach is adding `mapping(address => bool) registered` to `FROSTParticipantMap.struct T`. An alternative is reusing the `keys` mapping as a sentinel, but keys are only set in Round 2. The boolean flag adds one `SSTORE` per registration. This must be decided before Phase 1.

7. **`requireParticipating` helper**: Since `identifierOf` becomes a pure function, a new `requireParticipating(T storage self, address participant)` function is needed (checking that the derived identifier has a registered key or the `registered` flag). All callers that previously relied on `identifierOf` reverting must be updated to call this helper explicitly.
