# Feature Proposal: Explicit Transaction Rejection Response
Component: `all`

---

## Overview

Currently, when a validator determines a transaction is invalid, it silently drops it. The FROST signing ceremony times out with no on-chain record. From the user's and operator's perspective, "rejected" and "error/timeout" are indistinguishable.

This feature adds an explicit on-chain rejection mechanism: a validator that determines a transaction is invalid submits a `rejectTransaction` (or `rejectOracleTransaction`) call to `Consensus.sol`, storing the rejection reason on-chain. The explorer surfaces this as a distinct "Rejected" status with the specific reason code.

**Phases (separate PRs):**
1. **Contracts** — Add `rejectTransaction`, `rejectOracleTransaction`, `getTransactionRejection`, `getOracleTransactionRejection`, `TransactionRejected`, `OracleTransactionRejected` events, and `RejectionReason` enum.
2. **Validator** — Emit `consensus_reject_transaction` / `consensus_reject_oracle_transaction` action on invalid transactions instead of silently returning.
3. **Explorer** — Query `TransactionRejected` / `OracleTransactionRejected` events and display rejection status with reason.

Phases 2 and 3 can be developed in parallel once Phase 1 is merged.

---

## Architecture Decision

### On-chain rejection over off-chain signaling

Rejection is recorded on-chain via new `rejectTransaction` / `rejectOracleTransaction` functions rather than via off-chain storage or P2P messages. This is consistent with the existing architecture where all protocol communication happens through contract events, and it ensures transparency and verifiability.

### Rejection is advisory — attestation is never blocked

A rejection event provides fast feedback and observability, but **does not prevent a subsequent attestation**. `attestTransaction` is not modified. If an attestation is recorded for a previously rejected proposal, the explorer shows `ATTESTED` (attestation takes precedence).

**Why**: The allowed-modules, allowed-guards, and allowed-fallback-handler lists are hardcoded per validator binary version (see `checks/config/modules.ts`, `guards.ts`, `fallback.ts`). During a rolling upgrade where some validators run a newer version with an expanded allowlist, a validator on the old version could reject a transaction that the majority considers valid. Making rejection block attestation would give a single outdated validator the power to permanently deny any transaction in that epoch, with no override mechanism. Keeping rejection advisory eliminates this DoS vector entirely.

### First validator wins (race-to-reject)

The first validator to call `rejectTransaction` establishes the on-chain record; subsequent calls revert with `AlreadyRejected`. This keeps storage and gas costs to a single slot and a single tx per proposal.

`rejectTransaction` still guards against rejecting an already-attested proposal (`require($attestations[message].isZero(), AlreadyAttested())`) to avoid emitting misleading rejection events after a successful attestation.

### Access control: FROST group participants only

`rejectTransaction` calls `_COORDINATOR.participantKey($groups[epoch], msg.sender)`, which reverts with `InvalidParticipant` (from `FROSTParticipantMap.getKey`) if the caller is not a member of the epoch's signing group. This revert propagates and serves as the access control guard. No return-value check is needed.

### Error codes as a `uint8` enum

Rejection reasons are stored on-chain as a `uint8` enum (`RejectionReason`), matching the current set of `TransactionCheckErrorCode` values in the validator. Adding new codes requires a contract upgrade.

### Oracle transaction parity

`proposeOracleTransaction` has a parallel lifecycle with a different message derivation (includes oracle address). This feature adds `rejectOracleTransaction` to provide the same rejection visibility for oracle-checked proposals.

### Alternatives Considered

**Off-chain validator database**: Validators could write rejections to a shared or local DB for the explorer to query. Rejected because it breaks the transparency guarantee and requires additional infrastructure.

**Threshold-based rejection**: Require M-of-N validators to reject before marking as rejected. Would handle version-mismatch cases more gracefully, but adds significant complexity (per-validator storage, threshold parameter). Advisory rejection achieves the same safety with less complexity.

**Mutual exclusivity (rejection blocks attestation)**: Initially considered for a cleaner state machine. Rejected because the hardcoded allowlists mean validators on different versions can disagree. A single old-version validator could permanently block a valid transaction in an epoch with no recovery path. See "Rejection is advisory" above.

**bytes32 string instead of enum**: Stores the human-readable reason code (e.g. `"no_delegatecall"`) directly. More flexible for adding new codes without an upgrade but costs slightly more gas and has weaker type safety.

---

## User Flow

### Rejected transaction in explorer

```
Transaction page
─────────────────────────────────────────────────────
Transaction Proposals

  Proposal #1
  ┌─────────────────────────────────────────────────┐
  │ Status:    [ REJECTED ]                         │
  │ Proposed:  Block 12345  (Explorer Tx)           │
  │ Rejected:  Block 12347  (Explorer Tx)           │
  │ Reason:    No delegatecall                      │
  └─────────────────────────────────────────────────┘
```

The "Reason" field displays a human-readable label for the `RejectionReason` enum value. The rejection block and tx link provide auditability.

If a proposal has both a rejection and an attestation, `ATTESTED` is displayed (attestation takes precedence). This state indicates a validator version mismatch during the rejection and is an operational signal, not an error.

---

## Tech Specs

### Contracts

#### New: `RejectionReason` enum (in `IConsensus.sol`)

```solidity
enum RejectionReason {
    NotRejected,                // 0 — default storage value; returned by view functions when no rejection exists
    Unknown,                    // 1
    NoDelegatecall,             // 2
    UnsupportedModule,          // 3
    UnsupportedModuleGuard,     // 4
    UnsupportedGuard,           // 5
    UnsupportedFallbackHandler, // 6
    InvalidSelfCall,            // 7
    InvalidMultisend,           // 8
    InvalidMigration,           // 9
    InvalidSignMessage,         // 10
    InvalidCreateCall           // 11
}
```

#### New storage in `Consensus.sol`

```solidity
struct RejectionInfo {
    address validator;       // 20 bytes — non-zero acts as "is rejected" sentinel
    RejectionReason reason;  // 1 byte
}
mapping(bytes32 message => RejectionInfo) private $rejections;
```

#### New events in `IConsensus.sol`

```solidity
event TransactionRejected(
    bytes32 indexed safeTxHash,
    uint256 indexed chainId,
    address indexed safe,
    uint64 epoch,
    address validator,
    RejectionReason reason
);

event OracleTransactionRejected(
    bytes32 indexed safeTxHash,
    uint256 indexed chainId,
    address indexed safe,
    uint64 epoch,
    address oracle,
    address validator,
    RejectionReason reason
);
```

#### New errors in `Consensus.sol`

```solidity
error AlreadyRejected();
```

#### New function `rejectTransaction` in `IConsensus.sol` / `Consensus.sol`

```solidity
function rejectTransaction(
    uint64 epoch,
    uint256 chainId,
    address safe,
    bytes32 safeTxStructHash,
    RejectionReason reason
) external;
```

Implementation:
1. Reconstruct `safeTxHash` via `SafeTransaction.partialHash(chainId, safe, safeTxStructHash)`.
2. Reconstruct `message` via `domainSeparator().transactionProposal(epoch, safeTxHash)`.
3. Call `_COORDINATOR.participantKey($groups[epoch], msg.sender)` — reverts with `InvalidParticipant` for non-members, acting as access control.
4. `require($attestations[message].isZero(), AlreadyAttested())` — don't reject what's already attested.
5. `require($rejections[message].validator == address(0), AlreadyRejected())` — first validator wins.
6. Store `$rejections[message] = RejectionInfo(msg.sender, reason)`.
7. Emit `TransactionRejected(safeTxHash, chainId, safe, epoch, msg.sender, reason)`.

#### New function `rejectOracleTransaction` in `IConsensus.sol` / `Consensus.sol`

```solidity
function rejectOracleTransaction(
    uint64 epoch,
    address oracle,
    uint256 chainId,
    address safe,
    bytes32 safeTxStructHash,
    RejectionReason reason
) external;
```

Implementation mirrors `rejectTransaction` but uses `domainSeparator().oracleTransactionProposal(epoch, oracle, safeTxHash)` as the message key and checks `$attestations` via the oracle message key.

#### New view functions in `IConsensus.sol` / `Consensus.sol`

```solidity
function getTransactionRejection(uint64 epoch, bytes32 safeTxHash)
    external view returns (address validator, RejectionReason reason);

function getOracleTransactionRejection(uint64 epoch, address oracle, bytes32 safeTxHash)
    external view returns (address validator, RejectionReason reason);
```

Both return `(address(0), RejectionReason.NotRejected)` when not rejected.

#### `attestTransaction` — no change

`attestTransaction` is **not** modified. Rejection is advisory and does not block attestation.

#### Test cases

- Valid tx: can be attested; attempting to reject it after attestation reverts with `AlreadyAttested`.
- Invalid tx: can be rejected; attestation can still proceed after rejection (advisory).
- Double rejection: second call reverts with `AlreadyRejected`.
- Non-participant rejection: reverts with `InvalidParticipant` (from coordinator).
- All `RejectionReason` values round-trip through event and view.
- Oracle variants: same cases for `rejectOracleTransaction`.

---

### Validator

#### New protocol action types (`types.ts`)

```typescript
export type RejectTransaction = {
    id: "consensus_reject_transaction";
    epoch: bigint;
    chainId: bigint;
    safe: Address;
    safeTxStructHash: Hex;
    reason: TransactionCheckErrorCode;
};

export type RejectOracleTransaction = {
    id: "consensus_reject_oracle_transaction";
    epoch: bigint;
    oracle: Address;
    chainId: bigint;
    safe: Address;
    safeTxStructHash: Hex;
    reason: TransactionCheckErrorCode;
};
```

Add both to the `ConsensusAction` union.

#### Mapping `TransactionCheckErrorCode` → `RejectionReason` (new helper)

```typescript
const REJECTION_REASON: Record<TransactionCheckErrorCode, number> = {
    unknown: 1,
    no_delegatecall: 2,
    unsupported_module: 3,
    unsupported_module_guard: 4,
    unsupported_guard: 5,
    unsupported_fallback_handler: 6,
    invalid_self_call: 7,
    invalid_multisend: 8,
    invalid_migration: 9,
    invalid_sign_message: 10,
    invalid_create_call: 11,
};
```

#### Updated `handleTransactionProposed` (`transactionProposed.ts`)

When `result.status === "invalid"`, instead of `return {}`:

```typescript
return {
    actions: [{
        id: "consensus_reject_transaction",
        epoch: event.epoch,
        chainId: protocol.chainId(),
        safe: event.transaction.safe,
        safeTxStructHash: event.transaction.structHash,
        reason: result.error.code,
    }],
};
```

The oracle transaction handler (`handleOracleTransactionProposed`) is updated analogously, using `"consensus_reject_oracle_transaction"` and including the `oracle` address.

#### Updated `BaseProtocol` (`base.ts`)

- Add `case "consensus_reject_transaction"` and `case "consensus_reject_oracle_transaction"` to `performAction`.
- Add abstract methods `rejectTransaction` and `rejectOracleTransaction`.

#### Updated `OnchainProtocol` (`onchain.ts`)

Implement both methods by calling the corresponding contract functions.

#### `AlreadyRejected` handling

When `rejectTransaction` reverts with `AlreadyRejected`, it means another validator submitted the rejection first — the goal is already achieved. The validator must:
- **Not retry**: detect the `AlreadyRejected` error in `onchain.ts` and return a successful `SubmittedAction` (or throw a dedicated terminal error that `BaseProtocol` recognises as non-retryable).
- **Log at `info` level**, not `warn` — this is an expected race outcome, not a failure.

The same handling applies to `AlreadyAttested` reverts from `rejectTransaction` (the tx was attested before rejection was submitted — also a terminal, non-error outcome).

#### Test cases

- Invalid tx produces a `consensus_reject_transaction` action with correct reason code.
- `AlreadyRejected` revert is treated as success (no retry, info log).
- `AlreadyAttested` revert from `rejectTransaction` is treated as success (no retry, info log).
- All error codes map to the correct `RejectionReason` uint8.
- Oracle variants of the above.

---

### Explorer

#### Updated `ProposalStatus` (`transactions.ts`)

```typescript
export type ProposalStatus = "ATTESTED" | "PROPOSED" | "TIMED_OUT" | "REJECTED";
```

Status precedence (highest wins): `ATTESTED` > `REJECTED` > `TIMED_OUT` > `PROPOSED`.

#### Updated `TransactionProposal` type

```typescript
export type RejectionInfo = {
    validator: Address;
    reason: string;  // human-readable label
    block: bigint;
    tx: Hex;
};

export type TransactionProposal = {
    // ...existing fields...
    rejectedAt: RejectionInfo | null;
};
```

#### Updated `loadTransactionProposals` (`transactions.ts`)

Include `TransactionRejected` (and `OracleTransactionRejected` for oracle proposals) in the `eth_getLogs` topic filter alongside `TransactionProposed` and `TransactionAttested`. Build a `rejections` map keyed by `safeTxHash:epoch` and apply precedence: if both `rejectedAt` and `attestedAt` are present, status is `ATTESTED`.

#### Updated ABI (`abi.ts`)

Add `TransactionRejected` and `OracleTransactionRejected` events to `consensusAbi` and their selectors to `transactionEventSelectors`.

#### Updated `SafeTxProposals.tsx`

- Show `rejectedAt` block + tx link (similar to `attestedAt`).
- Show "Reason: \<human-readable label\>" when rejected.
- `StatusBadge` gets a red/error color for `"REJECTED"`.

#### Human-readable reason labels

```typescript
const REJECTION_REASON_LABELS: Record<number, string> = {
    1: "Unknown",
    2: "No delegatecall",
    3: "Unsupported module",
    4: "Unsupported module guard",
    5: "Unsupported guard",
    6: "Unsupported fallback handler",
    7: "Invalid self call",
    8: "Invalid multisend",
    9: "Invalid migration",
    10: "Invalid sign message",
    11: "Invalid create call",
};
```

#### Test cases

- Proposal with `TransactionRejected` event: `status: "REJECTED"` with correct `rejectedAt`.
- Proposal with both `TransactionRejected` and `TransactionAttested`: `status: "ATTESTED"` (attestation wins).
- Proposal with attestation only: `status: "ATTESTED"`.
- Proposal past timeout with no rejection: `status: "TIMED_OUT"`.
- Oracle proposal variants of the above.

---

## Implementation Phases

### Phase 1 — Contracts (PR 1)
**Can start immediately.**

Files:
- `contracts/src/interfaces/IConsensus.sol` — `RejectionReason` enum, `TransactionRejected` and `OracleTransactionRejected` events, `AlreadyRejected` error, `rejectTransaction`, `rejectOracleTransaction`, `getTransactionRejection`, `getOracleTransactionRejection` signatures.
- `contracts/src/Consensus.sol` — `RejectionInfo` struct, `$rejections` storage, all four function implementations. No change to `attestTransaction` or `attestOracleTransaction`.
- `contracts/test/` — Unit tests for rejection flow (regular and oracle variants).

### Phase 2a — Validator (PR 2)
**Can start after Phase 1 is merged (or in parallel using a local ABI draft).**

Files:
- `validator/src/consensus/protocol/types.ts` — `RejectTransaction`, `RejectOracleTransaction` types, updated `ConsensusAction`.
- `validator/src/machine/consensus/transactionProposed.ts` — Return `actions` diff for invalid tx.
- `validator/src/machine/consensus/oracleTransactionProposed.ts` (or equivalent) — Same for oracle proposals.
- `validator/src/consensus/protocol/base.ts` — New cases in `performAction`, abstract methods.
- `validator/src/consensus/protocol/onchain.ts` — Implement on-chain calls with `AlreadyRejected` / `AlreadyAttested` terminal handling.
- `validator/src/machine/consensus/transactionProposed.test.ts` — Tests.

### Phase 2b — Explorer (PR 3)
**Can start after Phase 1 is merged, in parallel with Phase 2a.**

Files:
- `explorer/src/lib/consensus/abi.ts` — Add `TransactionRejected`, `OracleTransactionRejected` events + selectors.
- `explorer/src/lib/consensus/transactions.ts` — `RejectionInfo` type, updated `TransactionProposal`, updated `loadTransactionProposals` with precedence logic.
- `explorer/src/components/transaction/SafeTxProposals.tsx` — Show rejection info.
- `explorer/src/components/common/StatusBadge.tsx` — Add `REJECTED` color.
- `explorer/src/lib/consensus/transactions.test.ts` — Tests.

---

## Open Questions / Assumptions

- **`safeTxStructHash` availability**: The validator must compute the EIP-712 struct hash from `event.transaction` when constructing the reject action. This follows the same pattern used for `attestTransaction`.
- **`participantKey` access control**: `FROSTParticipantMap.getKey` reverts with `InvalidParticipant` for non-members (confirmed by reading the implementation). The key is only set after DKG completes, so only participants in a fully-formed group can call `rejectTransaction`. This is the desired behaviour.
- **Rejection + attestation coexistence**: Because rejection is advisory, it is theoretically possible for both `TransactionRejected` and `TransactionAttested` to exist for the same proposal. This indicates a validator version mismatch at the time of rejection. The explorer shows `ATTESTED` in this case and the co-existence of both events is an operational signal for the team.
- **Devnet deployment**: After all three phases are merged, a devnet redeployment is required to pick up the new contract interface.
