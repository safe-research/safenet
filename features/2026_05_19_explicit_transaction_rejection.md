# Feature Proposal: Explicit Transaction Rejection Response
Component: `all`

---

## Overview

Currently, when a validator determines a transaction is invalid, it silently drops it. The FROST signing ceremony times out with no on-chain record. From the user's and operator's perspective, "rejected" and "error/timeout" are indistinguishable.

This feature adds an explicit on-chain rejection mechanism: a validator that determines a transaction is invalid submits a `rejectTransaction` (or `rejectOracleTransaction`) call to `Consensus.sol`. Each validator's rejection is recorded individually. The explorer surfaces this as a distinct "Rejected" status.

**Phases (separate PRs):**
1. **Contracts** — Add `rejectTransaction`, `rejectOracleTransaction`, `getTransactionRejection`, `getOracleTransactionRejection`, `TransactionRejected`, and `OracleTransactionRejected` events.
2. **Validator** — Emit `consensus_reject_transaction` / `consensus_reject_oracle_transaction` action on invalid transactions instead of silently returning.
3. **Explorer** — Query `TransactionRejected` / `OracleTransactionRejected` events and display rejection status.

Phases 2 and 3 can be developed in parallel once Phase 1 is merged.

---

## Architecture Decision

### On-chain rejection over off-chain signaling

Rejection is recorded on-chain via new `rejectTransaction` / `rejectOracleTransaction` functions rather than via off-chain storage or P2P messages. This is consistent with the existing architecture where all protocol communication happens through contract events, and it ensures transparency and verifiability.

### Rejection is advisory — attestation is never blocked

A rejection event provides fast feedback and observability, but **does not prevent a subsequent attestation**. `attestTransaction` is not modified. If an attestation is recorded for a previously rejected proposal, the explorer shows `ATTESTED` (attestation takes precedence).

**Why**: The allowed-modules, allowed-guards, and allowed-fallback-handler lists are hardcoded per validator binary version (see `checks/config/modules.ts`, `guards.ts`, `fallback.ts`). During a rolling upgrade where some validators run a newer version with an expanded allowlist, a validator on the old version could reject a transaction that the majority considers valid. Making rejection block attestation would give a single outdated validator the power to permanently deny any transaction in that epoch, with no override mechanism. Keeping rejection advisory eliminates this DoS vector entirely.

### Per-validator tracking

Each validator's rejection is stored as an individual flag. This means:
- Multiple `TransactionRejected` events can be emitted for the same proposal, one per rejecting validator.
- Rejection and attestation coexist naturally — no "first wins" special-casing needed.
- The explorer can show which validators rejected, providing richer observability.

### Rejection stored on `Consensus`, not `FROSTCoordinator`

Rejection is a consensus-layer concern (is this Safe transaction valid per the network's rules?) and belongs alongside attestation in `Consensus.sol`. The `FROSTCoordinator` manages low-level FROST signing mechanics and has no knowledge of Safe transactions or validator validity rules. Placing rejection there would introduce a semantic mismatch and couple the signing infrastructure to application-layer policy. Keeping both attestation and rejection in `Consensus` also maintains a consistent and complete picture of a proposal's lifecycle in one contract.

### Rejection is a flag — no reason code

Only a boolean flag per (message, validator) is stored. A reason code would improve user-facing messages but requires maintaining a `RejectionReason` enum in sync across the contract and validator, and adds a contract upgrade path when new checks are introduced. The flag alone is sufficient for the primary goals of fast feedback and observability.

### Access control: FROST group participants only

`rejectTransaction` calls `_COORDINATOR.participantKey($groups[epoch], msg.sender)`, which reverts with `InvalidParticipant` (from `FROSTParticipantMap.getKey`) if the caller is not a member of the epoch's signing group. This revert propagates and serves as the access control guard. No return-value check is needed.

### Oracle transaction parity

`proposeOracleTransaction` has a parallel lifecycle with a different message derivation (includes oracle address). This feature adds `rejectOracleTransaction` to provide the same rejection visibility for oracle-checked proposals.

### Alternatives Considered

**Off-chain validator database**: Validators could write rejections to a shared or local DB for the explorer to query. Rejected because it breaks the transparency guarantee and requires additional infrastructure.

**Threshold-based rejection**: Require M-of-N validators to reject before marking as rejected. Would handle version-mismatch cases more gracefully, but adds significant complexity (per-validator storage, threshold parameter). Advisory rejection achieves the same safety with less complexity.

**Mutual exclusivity (rejection blocks attestation)**: Initially considered for a cleaner state machine. Rejected because the hardcoded allowlists mean validators on different versions can disagree. A single old-version validator could permanently block a valid transaction in an epoch with no recovery path. See "Rejection is advisory" above.

**Reason codes**: Storing a `RejectionReason` enum alongside the flag would improve user-facing messages. Dropped in favour of the simpler flag to avoid keeping TS and Solidity enums in sync and to avoid contract upgrades when new checks are added.

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
  │            Block 12348  (Explorer Tx)           │
  └─────────────────────────────────────────────────┘
```

Each rejection event is shown as a separate row. If a proposal has both rejections and an attestation, `ATTESTED` is displayed (attestation takes precedence). This state indicates a validator version mismatch during the rejection and is an operational signal, not an error.

---

## Tech Specs

### Contracts

#### New storage in `Consensus.sol`

```solidity
mapping(bytes32 message => mapping(address validator => bool)) private $rejections;
```

#### New events in `IConsensus.sol`

```solidity
event TransactionRejected(
    bytes32 indexed safeTxHash,
    uint256 indexed chainId,
    address indexed safe,
    uint64 epoch,
    address validator
);

event OracleTransactionRejected(
    bytes32 indexed safeTxHash,
    uint256 indexed chainId,
    address indexed safe,
    uint64 epoch,
    address oracle,
    address validator
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
    bytes32 safeTxStructHash
) external;
```

Implementation:
1. Reconstruct `safeTxHash` via `SafeTransaction.partialHash(chainId, safe, safeTxStructHash)`.
2. Reconstruct `message` via `domainSeparator().transactionProposal(epoch, safeTxHash)`.
3. Call `_COORDINATOR.participantKey($groups[epoch], msg.sender)` — reverts with `InvalidParticipant` for non-members, acting as access control.
4. `require($attestations[message].isZero(), AlreadyAttested())` — don't reject what's already attested.
5. `require(!$rejections[message][msg.sender], AlreadyRejected())` — prevent double rejection by the same validator.
6. Set `$rejections[message][msg.sender] = true`.
7. Emit `TransactionRejected(safeTxHash, chainId, safe, epoch, msg.sender)`.

#### New function `rejectOracleTransaction` in `IConsensus.sol` / `Consensus.sol`

```solidity
function rejectOracleTransaction(
    uint64 epoch,
    address oracle,
    uint256 chainId,
    address safe,
    bytes32 safeTxStructHash
) external;
```

Implementation mirrors `rejectTransaction` but uses `domainSeparator().oracleTransactionProposal(epoch, oracle, safeTxHash)` as the message key.

#### New view functions in `IConsensus.sol` / `Consensus.sol`

```solidity
function getTransactionRejection(uint64 epoch, bytes32 safeTxHash, address validator)
    external view returns (bool rejected);

function getOracleTransactionRejection(uint64 epoch, address oracle, bytes32 safeTxHash, address validator)
    external view returns (bool rejected);
```

#### `attestTransaction` — no change

`attestTransaction` is **not** modified. Rejection is advisory and does not block attestation.

#### Test cases

- Valid tx: can be attested; attempting to reject it after attestation reverts with `AlreadyAttested`.
- Invalid tx: can be rejected by multiple validators independently; attestation can still proceed after rejection (advisory).
- Double rejection by the same validator: second call reverts with `AlreadyRejected`.
- Two different validators rejecting the same proposal: both succeed, two events emitted.
- Non-participant rejection: reverts with `InvalidParticipant` (from coordinator).
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
};

export type RejectOracleTransaction = {
    id: "consensus_reject_oracle_transaction";
    epoch: bigint;
    oracle: Address;
    chainId: bigint;
    safe: Address;
    safeTxStructHash: Hex;
};
```

Add both to the `ConsensusAction` union.

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

When `rejectTransaction` reverts with `AlreadyRejected`, the same validator somehow submitted the rejection twice — treat it as a terminal, non-retryable outcome. When it reverts with `AlreadyAttested`, the tx was attested before rejection was submitted — also terminal. Both are caught in `onchain.ts`, resolved as completed actions, and logged at `info` rather than `warn`.

#### Test cases

- Invalid tx produces a `consensus_reject_transaction` action.
- `AlreadyRejected` revert resolves without retry and logs at info level.
- `AlreadyAttested` revert from `rejectTransaction` resolves without retry.
- Oracle transaction variants of the above.

---

### Explorer

#### Updated `ProposalStatus` (`transactions.ts`)

```typescript
export type ProposalStatus = "ATTESTED" | "PROPOSED" | "TIMED_OUT" | "REJECTED";
```

Status precedence (highest wins): `ATTESTED` > `REJECTED` > `TIMED_OUT` > `PROPOSED`.

#### Updated `TransactionProposal` type

```typescript
export type RejectionEvent = {
    validator: Address;
    block: bigint;
    tx: Hex;
};

export type TransactionProposal = {
    // ...existing fields...
    rejections: RejectionEvent[];  // one entry per rejecting validator
};
```

#### Updated `loadTransactionProposals` (`transactions.ts`)

Include `TransactionRejected` (and `OracleTransactionRejected` for oracle proposals) in the `eth_getLogs` topic filter alongside `TransactionProposed` and `TransactionAttested`. Collect all rejection events per `safeTxHash:epoch` into the `rejections` array. Status is `REJECTED` if `rejections` is non-empty and no attestation exists.

#### Updated ABI (`abi.ts`)

Add `TransactionRejected` and `OracleTransactionRejected` events to `consensusAbi` and their selectors to `transactionEventSelectors`.

#### Updated `SafeTxProposals.tsx`

- Show each entry in `rejections` as a row with block number and tx link (similar to `attestedAt`).
- `StatusBadge` gets a red/error colour for `"REJECTED"`.

#### Test cases

- Proposal with one `TransactionRejected` event: `status: "REJECTED"`, one entry in `rejections`.
- Proposal with multiple `TransactionRejected` events: `status: "REJECTED"`, multiple entries in `rejections`.
- Proposal with both rejections and attestation: `status: "ATTESTED"` (attestation wins), `rejections` still populated.
- Proposal past timeout with no rejection: `status: "TIMED_OUT"`.
- Oracle proposal variants of the above.

---

## Implementation Phases

### Phase 1 — Contracts (PR 1)
**Can start immediately.**

Files:
- `contracts/src/interfaces/IConsensus.sol` — `TransactionRejected` and `OracleTransactionRejected` events, `AlreadyRejected` error, `rejectTransaction`, `rejectOracleTransaction`, `getTransactionRejection`, `getOracleTransactionRejection` signatures.
- `contracts/src/Consensus.sol` — `$rejections` storage, all four function implementations. No change to `attestTransaction` or `attestOracleTransaction`.
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
- `explorer/src/lib/consensus/transactions.ts` — `RejectionEvent` type, updated `TransactionProposal`, updated `loadTransactionProposals` with precedence logic.
- `explorer/src/components/transaction/SafeTxProposals.tsx` — Show rejection rows.
- `explorer/src/components/common/StatusBadge.tsx` — Add `REJECTED` colour.
- `explorer/src/lib/consensus/transactions.test.ts` — Tests.

---

## Open Questions / Assumptions

- **`safeTxStructHash` availability**: The validator must compute the EIP-712 struct hash from `event.transaction` when constructing the reject action. This follows the same pattern used for `attestTransaction`.
- **`participantKey` access control**: `FROSTParticipantMap.getKey` reverts with `InvalidParticipant` for non-members (confirmed by reading the implementation). The key is only set after DKG completes, so only participants in a fully-formed group can call `rejectTransaction`. This is the desired behaviour.
- **Rejection + attestation coexistence**: Because rejection is advisory, it is possible for both `TransactionRejected` and `TransactionAttested` to exist for the same proposal. This indicates a validator version mismatch at the time of rejection. The explorer shows `ATTESTED` in this case; the rejection events remain visible for auditability.
- **Devnet deployment**: After all three phases are merged, a devnet redeployment is required to pick up the new contract interface.
