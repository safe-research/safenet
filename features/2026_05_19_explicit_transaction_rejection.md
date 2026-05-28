# Feature Proposal: Explicit Transaction Rejection Response
Component: `all`

---

## Overview

Currently, when a validator determines a transaction is invalid, it silently drops it. The FROST signing ceremony times out with no on-chain record. From the user's and operator's perspective, "rejected" and "error/timeout" are indistinguishable.

This feature adds an explicit **decline** to the signing ceremony: a validator that determines a transaction is invalid calls `signDeclineWithCallback` on `FROSTCoordinator`. Once enough validators decline (threshold: `count - threshold + 1`), the ceremony is definitively marked rejected on-chain — the ceremony becomes mathematically uncompletable, a callback triggers `onSignRejected` on `Consensus`, which emits a `TransactionRejected` event, and signature queries revert with `NotSigned`. The explorer surfaces this as a distinct "Rejected" status.

**Phases (separate PRs):**
1. **FROSTCoordinator** — Add `signDecline`, threshold stopping logic, and `SignDeclined`/`SignRejected` events. No callback yet.
2. **Consensus callback** — Add `signDeclineWithCallback` to `FROSTCoordinator`, add `onSignRejected` to `IFROSTCoordinatorCallback`, implement on `Consensus`, emit `TransactionRejected`/`OracleTransactionRejected`.
3. **Validator** — Add `"waiting_to_decline"` state and `sign_decline_with_callback` action. Can be developed and merged in parallel with Phase 2; both depend only on Phase 1.
4. **Explorer: rejection status** — Add `REJECTED` to `ProposalStatus`, derive from `TransactionRejected` events on Consensus. Requires Phase 2.
5. **Explorer: declined breakdown** — Add `declined` to `AttestationStatus`, query `SignDeclined` events on coordinator, add "Declined" row to UI. Can be developed in parallel with Phases 2–4, can start after Phase 1.

---

## Architecture Decision

### Decline lives in `FROSTCoordinator`, not `Consensus`

A rejection is semantically "I am not participating in this signing ceremony." The signing ceremony is managed by `FROSTCoordinator` — it already tracks per-participant actions (`SignRevealedNonces`, `SignShared`). An explicit decline is the natural counterpart to those actions, and belongs in the same contract.

`Consensus` only stores a pointer (the `FROSTSignatureId`) to the completed signature, which lives in the coordinator. The rejection state lives in the coordinator too; `Consensus` learns about it exclusively via the coordinator callback.

### Decline is a flag — no reason code

Only a per-participant boolean is stored: "did this participant decline this ceremony?" A reason code would improve user-facing messages but requires maintaining an enum in sync across the contract and validator, and adds a contract upgrade path when new checks are introduced. The flag alone is sufficient for the goals of fast feedback and observability.

### Decline tracking lives in `FROSTNonceCommitmentSet`, not `Signature`

Originally, per-participant ceremony state was split across two boolean mappings: `declined` (in the `Signature` struct in `FROSTCoordinator`) and `anyShared` (in `FROSTSignatureShares.T`). These were mutually exclusive by invariant but not by type, creating hidden coupling between two different structs with no compile-time enforcement.

The refactored design uses a single `used` boolean stored in `FROSTNonceCommitmentSet.Commitments` per `(participant, sequence)`. Since nonces are already keyed by sequence — the same key space as ceremonies — it is the natural home for this state. The first of `signRevealNonces` or `signDecline` to run for a given `(participant, sequence)` sets `used = true` (atomic check-and-set, reverts `NoncesAlreadyUsed` if the slot is already consumed). No further distinction between "revealed" and "burned" is needed — nothing in the contract needs to query which action was taken, only that the slot has been consumed at most once.

The on-chain invariant is now: per signing sequence, a participant either times out, reveals their nonces exactly once (and subsequently shares), or declines exactly once.

### Decline stops signing at threshold — but never blocks attestation

Two distinct behaviors:

**At signing threshold**: When `decline_count >= count - threshold + 1`, enough validators have declined that the ceremony can no longer reach threshold regardless of who is left. At this point, the ceremony is definitively rejected. `signatureVerify`/`signatureValue` revert with `NotSigned` (a rejected ceremony has `signed == bytes32(0)`). The coordinator fires `onSignRejected` on Consensus, which emits `TransactionRejected`. `signShare` is **not** blocked after rejection — this is symmetric with `signDecline` being callable past the threshold for observability — but the ceremony can never complete because the remaining non-declined participants are fewer than `threshold`.

**Attestation is never blocked before threshold**: If threshold participants sign before enough declines accumulate, the ceremony completes and the transaction is attested. `ATTESTED` always takes precedence over `REJECTED` in the explorer.

**Why**: The allowed-modules, allowed-guards, and allowed-fallback-handler lists are hardcoded per validator binary version (see `checks/config/modules.ts`, `guards.ts`, `fallback.ts`). During a rolling upgrade, a validator on an older version could decline a transaction that the majority considers valid. Making even one decline block signing would give a single outdated validator the power to deny any transaction in that epoch with no recovery path. The threshold (`count - threshold + 1`) is the exact point at which the ceremony is mathematically uncompletable — blocking signing is safe only at this point.

### Rejection callback mirrors the existing signing callback pattern

`signShareWithCallback` calls `onSignCompleted` on `Consensus` when signing completes. Rejection uses the same mechanism: `signDeclineWithCallback` calls `onSignRejected` on `Consensus` when the rejection threshold is first crossed. The callback context uses the same function-selector-prefixed ABI encoding as attestation, dispatching to `rejectTransaction` or `rejectOracleTransaction`.

This avoids cross-contract event correlation in the explorer: `TransactionRejected` lives on `Consensus` alongside `TransactionProposed` and `TransactionAttested`, which simplifies queries to a single contract.

### Validator uses a `"waiting_to_decline"` signing state

When a transaction is deemed invalid in `handleTransactionProposed`, the validator creates a `"waiting_to_decline"` signing state (instead of `"waiting_for_request"` for valid transactions). When the `Sign` event from the coordinator arrives for that message, `handleSign` detects this state and emits a `sign_decline_with_callback` action rather than proceeding with nonce commitments. The signing state is then cleared.

This slots into the existing state machine with minimal changes: `handleSign` already dispatches on the signing state type.

### Alternatives Considered

**Off-chain validator database**: Validators write rejections to a shared DB. Rejected because it breaks transparency and requires additional infrastructure.

**Single decline blocks attestation**: Making any one decline prevent signing. Creates a single-validator DoS risk. The threshold-based approach only stops signing when it is mathematically impossible anyway.

**Reason codes**: Storing a reason alongside the flag would improve user-facing messages. Dropped to avoid keeping TS and Solidity enums in sync and to avoid contract upgrades when new checks are added.

**Explorer cross-contract correlation**: Query `SignDeclined` events on the coordinator and correlate by SID to determine rejection status. Dropped in favour of `TransactionRejected` on Consensus (same source as `TransactionProposed` / `TransactionAttested`), which is simpler and avoids combining two data sources at the component level.

---

## User Flow

### Rejected transaction in explorer

```
Transaction page
─────────────────────────────────────────────────────
Transaction Proposals

  Proposal #1
  ┌─────────────────────────────────────────────────┐
  │ Status:       [ REJECTED ]                      │
  │ Proposed:     Block 12345  (Explorer Tx)        │
  │ Committed:    0x1a2b…  0x3c4d…                  │
  │ Signed:       —                                 │
  │ Declined:     0x5e6f…  0x7a8b…                  │
  └─────────────────────────────────────────────────┘
```

"Declined" is a new row in the existing attestation status breakdown (alongside "Committed" and "Signed"), showing which validators explicitly opted out of the ceremony. Status is `REJECTED` when a `TransactionRejected` event exists and no `TransactionAttested` event exists. If attestation completes before enough declines accumulate, status is `ATTESTED`.

---

## Tech Specs

### Phase 1 — FROSTCoordinator (`FROSTCoordinator.sol`)

No changes to `IFROSTCoordinatorCallback.sol` or `Consensus.sol` in this phase. `signDeclineWithCallback` is added in Phase 2 once `IFROSTCoordinatorCallback.onSignRejected` exists.

#### Updated `Signature` struct

Add `declineCount` to track how many participants have declined. Whether a ceremony is rejected is computable as `declineCount > group.count - group.threshold`, so no separate `rejected` flag is stored:

```solidity
struct Signature {
    bytes32 message;
    bytes32 signed;
    uint16 declineCount;
    FROSTSignatureShares.T shares;
}
```

Per-participant decline and reveal state is tracked in `FROSTNonceCommitmentSet` via a single `used` boolean per sequence (see Architecture Decision above). No per-participant mappings are added to `Signature` or `FROSTSignatureShares.T`.

#### New events

```solidity
event SignDeclined(FROSTSignatureId.T indexed sid, address indexed participant);
event SignRejected(FROSTSignatureId.T indexed sid);
```

#### New errors

No new errors are added to `FROSTCoordinator` in Phase 1. `FROSTNonceCommitmentSet` adds:

```solidity
error NoncesAlreadyUsed();
```

Thrown by `burn()` or `verify()` when the nonce slot for `(participant, sequence)` is already consumed. This enforces the mutual-exclusion invariant: a participant cannot both reveal and decline for the same sequence.

The following errors considered during design are intentionally **not** added:

- `AlreadyDeclined` and `NoncesNotRevealed` in `signShare` — the `selectionRoot` Merkle commitment already prevents a participant who has not revealed nonces (or who has burned them) from producing a valid signing selection. `FROST.verifyShare` is the authoritative gate.
- `SigningComplete` in `signDecline` — late declines after signing completes are harmless: the ceremony has already succeeded and `declineCount` can never reach the rejection threshold after enough participants have shared.
- `SignatureRejected` in `signatureVerify`/`signatureValue` — a rejected ceremony has `signed == bytes32(0)`, so `NotSigned` already covers this case without additional error differentiation.
- `CeremonyRejected` in `signShare` — after the rejection threshold is crossed, the remaining non-declined participants are fewer than `threshold`, so the ceremony can never complete regardless.

#### New function `signDecline`

```solidity
function signDecline(FROSTSignatureId.T sid) public returns (bool rejected);
```

Implementation:
1. `require($signatures[sid].message != bytes32(0), NotSigning())` — validates ceremony exists.
2. `Group storage group = $groups[sid.group()]`.
3. `group.participants.getKey(msg.sender)` — reverts with `InvalidParticipant` for non-members, acting as access control.
4. `Signature storage signature = $signatures[sid]`.
5. `group.nonces.burn(msg.sender, sid.sequence())` — atomically enforces mutual exclusion: reverts `NoncesAlreadyUsed` if the participant already called `signRevealNonces` or already called `signDecline`. Sets the `used` flag to `true` on success.
6. `signature.declineCount++`.
7. Emit `SignDeclined(sid, msg.sender)`.
8. `GroupState memory state = group.state`.
9. If `signature.declineCount == state.count - state.threshold + 1` (equality, fires exactly once):
    - Emit `SignRejected(sid)`.
    - Return `true`.
10. Return `false`.

Note: the equality check in step 9 ensures `SignRejected` fires exactly once without needing to store a `rejected` flag. Additional declines past the threshold still emit `SignDeclined` (for observability) but not `SignRejected`. Late declines after signing completes are also accepted.

#### View functions — not added in Phase 1

`isSignDeclined`, `isSignShared`, `isSignRejected`, and `signatureMessage` are intentionally **not** added in Phase 1. They are only needed by `Consensus.rejectTransaction` (Phase 2), which calls `isSignRejected` to validate the SID and `signatureMessage` to bind the SID to the correct ceremony. Adding them before their caller exists would introduce dead external API surface. They are added in Phase 2 alongside `signDeclineWithCallback` and `IFROSTCoordinatorCallback.onSignRejected`.

#### Updated `FROSTSignatureShares`

`FROSTSignatureShares.T` is unchanged from its pre-Phase-1 definition:

```solidity
struct T {
    mapping(bytes32 root => Aggregate) aggregates;
}
```

Mutual exclusion between sharing and declining is enforced by `FROSTNonceCommitmentSet` (see Architecture Decision above), not by a field in this library.

#### Updated `signShare`

No new guards are added. The `selectionRoot` Merkle commitment is the authoritative gate: only participants who have revealed their nonces can appear in a valid signing selection, so no separate nonce checks are needed. `FROST.verifyShare` handles invalid share submissions. The function is otherwise unchanged.

`CeremonyRejected` is **not** added. After the rejection threshold is crossed, the ceremony can no longer complete mathematically — the remaining non-declined participants are fewer than `threshold`. Omitting the guard is symmetric with `signDecline` being callable past the threshold for observability.

#### Updated `signatureVerify` and `signatureValue`

No rejection check is added. A rejected ceremony has `signed == bytes32(0)` because signing never completed, so the existing `NotSigned` check covers both the "still in progress" and "rejected" cases. Error differentiation is not useful for these non-user-facing functions.

#### Test cases (Phase 1)

Phase 1 ships with one focused test (`test_SignDecline_ThresholdReached_EmitsSignRejected`) covering the happy path: threshold number of participants declining emits `SignRejected` and the final call returns `true`. Full coverage is added in four stacked PRs targeting this branch (all unblocked after Phase 1 merges):

- **Branch A (threshold boundary)**: declines below threshold (no `SignRejected`), exactly at threshold (`SignRejected` emitted once, returns `true`), above threshold (returns `false`, no re-emit).
- **Branch B (mutual exclusion)**: `signDecline` after `signRevealNonces` reverts `NoncesAlreadyUsed`; double decline reverts `NoncesAlreadyUsed`; they are mutually exclusive per participant per sequence.
- **Branch C (error paths)**: non-participant reverts `InvalidParticipant`; double decline reverts `NoncesAlreadyUsed`; decline of non-existent ceremony reverts `NotSigning`; `signatureVerify`/`signatureValue` for incomplete or rejected SID reverts `NotSigned`.
- **Branch D (post-rejection / post-completion)**: additional declines after threshold are recorded (`SignDeclined` emitted) but `SignRejected` not re-emitted; ceremony completing before rejection threshold succeeds normally.

---

### Phase 2 — Consensus Callback (`FROSTCoordinator.sol`, `IFROSTCoordinatorCallback.sol`, `Consensus.sol`)

#### New view functions (`FROSTCoordinator.sol`)

Added here alongside `signDeclineWithCallback` because `rejectTransaction` (below) needs them to validate the SID:

```solidity
function isSignRejected(FROSTSignatureId.T sid) external view returns (bool);
function signatureMessage(FROSTSignatureId.T sid) external view returns (bytes32);
```

`isSignRejected` computes rejection from `declineCount` and the group state: `$signatures[sid].declineCount > group.state.count - group.state.threshold`. `signatureMessage` returns `$signatures[sid].message` — used by `rejectTransaction` to verify the SID corresponds to the ceremony for the given message (preventing a caller from submitting a legitimately-rejected SID from a different ceremony to emit a spurious `TransactionRejected` event).

#### New function `signDeclineWithCallback` (`FROSTCoordinator.sol`)

Added in this phase alongside `onSignRejected` — the two must be introduced together since the coordinator calls into the updated interface:

```solidity
function signDeclineWithCallback(FROSTSignatureId.T sid, Callback calldata callback)
    external
    returns (bool rejected)
{
    rejected = signDecline(sid);
    if (rejected) {
        callback.target.onSignRejected(sid, callback.context);
    }
}
```

#### Updated `IFROSTCoordinatorCallback`

Add the new callback method:

```solidity
interface IFROSTCoordinatorCallback {
    function onKeyGenCompleted(FROSTGroupId.T gid, bytes calldata context) external;
    function onSignCompleted(FROSTSignatureId.T sid, bytes calldata context) external;
    function onSignRejected(FROSTSignatureId.T sid, bytes calldata context) external;  // new
}
```

#### New storage (`Consensus.sol`)

```solidity
mapping(bytes32 message => FROSTSignatureId.T) private $rejections;
```

Mirrors `$attestations` for rejection state.

#### New events (`IConsensus.sol`)

```solidity
event TransactionRejected(
    bytes32 indexed safeTxHash,
    uint256 chainId,
    address indexed safe,
    uint64 epoch,
    FROSTSignatureId.T signatureId
);

event OracleTransactionRejected(
    bytes32 indexed safeTxHash,
    uint256 chainId,
    address indexed safe,
    uint64 epoch,
    address oracle,
    FROSTSignatureId.T signatureId
);
```

#### New errors (`Consensus.sol`)

```solidity
error AlreadyRejected();
error NotRejected();
```

`NotRejected` — thrown when `rejectTransaction` is called with a signatureId that the coordinator has not marked as rejected. Guards against direct calls bypassing the callback mechanism.

#### New public functions (`Consensus.sol`)

```solidity
function rejectTransaction(
    uint64 epoch,
    uint256 chainId,
    address safe,
    bytes32 safeTxStructHash,
    FROSTSignatureId.T signatureId
) public;

function rejectOracleTransaction(
    uint64 epoch,
    address oracle,
    uint256 chainId,
    address safe,
    bytes32 safeTxStructHash,
    FROSTSignatureId.T signatureId
) public;
```

`rejectTransaction` implementation:
1. Compute `safeTxHash = SafeTransaction.partialHash(chainId, safe, safeTxStructHash)`.
2. Compute `message = domainSeparator().transactionProposal(epoch, safeTxHash)`.
3. `require($rejections[message].isZero(), AlreadyRejected())`.
4. `require(_COORDINATOR.isSignRejected(signatureId), NotRejected())` — validates the coordinator has marked this SID as rejected.
5. `require(_COORDINATOR.signatureMessage(signatureId) == message, WrongSignature())` — validates the SID belongs to this specific ceremony. Without this check, a caller could pass a legitimately rejected SID from an unrelated ceremony to emit a false `TransactionRejected` event for any proposed transaction. `WrongSignature` is an existing coordinator error; add an equivalent to `Consensus.sol` or reuse a matching one.
6. `$rejections[message] = signatureId`.
7. Emit `TransactionRejected(safeTxHash, chainId, safe, epoch, signatureId)`.

`rejectOracleTransaction` follows the same pattern with `oracleTransactionProposal` and `OracleTransactionRejected`.

#### Updated `onSignRejected` (`Consensus.sol`)

Dispatches on the function selector in `context`, matching the pattern of `onSignCompleted`:

```solidity
function onSignRejected(FROSTSignatureId.T signatureId, bytes calldata context)
    external
    onlyCoordinator
{
    bytes4 selector = bytes4(context);
    if (selector == this.rejectTransaction.selector) {
        (uint64 epoch, uint256 chainId, address safe, bytes32 safeTxStructHash) =
            abi.decode(context[4:], (uint64, uint256, address, bytes32));
        rejectTransaction(epoch, chainId, safe, safeTxStructHash, signatureId);
    } else if (selector == this.rejectOracleTransaction.selector) {
        (uint64 epoch, address oracle, uint256 chainId, address safe, bytes32 safeTxStructHash) =
            abi.decode(context[4:], (uint64, address, uint256, address, bytes32));
        rejectOracleTransaction(epoch, oracle, chainId, safe, safeTxStructHash, signatureId);
    } else {
        revert UnknownSignatureSelector();
    }
}
```

#### Updated `supportsInterface` (`Consensus.sol`)

`IFROSTCoordinatorCallback` now has an additional method; no code change required here as the interface ID is derived from all methods.

#### Test cases (Phase 2)

- `signDeclineWithCallback`: callback fires when rejection threshold is first crossed, not on subsequent declines.
- `signDeclineWithCallback`: no callback when threshold is not yet reached.
- `onSignRejected` called by non-coordinator: reverts with `NotCoordinator`.
- `onSignRejected` dispatches to `rejectTransaction` for safe tx context.
- `onSignRejected` dispatches to `rejectOracleTransaction` for oracle tx context.
- `onSignRejected` with unknown selector: reverts with `UnknownSignatureSelector`.
- `rejectTransaction` emits `TransactionRejected`, stores signatureId.
- `rejectTransaction` with unrejected signatureId (coordinator does not have it marked): reverts with `NotRejected`.
- `rejectTransaction` with a rejected signatureId from a different ceremony (wrong message): reverts with `WrongSignature`.
- `rejectTransaction` called twice for same message: reverts with `AlreadyRejected`.
- `rejectOracleTransaction` mirrors the above for oracle transactions.

---

### Phase 3 — Validator

#### New signing state (`types.ts`)

```typescript
| {
      id: "waiting_to_decline";
      packet: SafeTransactionPacket | OracleTransactionPacket;
      deadline: bigint;
  }
```

`packet` is required to conform with `BaseSigningState` and to allow the SQLite storage layer to persist and restore this state across validator restarts. Both `SafeTransactionPacket` and `OracleTransactionPacket` are available at creation time in their respective handlers.

Add to the `SigningState` union alongside the existing states.

#### Updated `handleTransactionProposed` (`transactionProposed.ts`)

When `result.status === "invalid"`, instead of `return {}`:

```typescript
return {
    signing: [
        message,
        {
            id: "waiting_to_decline",
            packet,
            deadline: event.block + machineConfig.signingTimeout,
        },
    ],
};
```

#### Updated `handleOracleTransactionProposed` (`oracleTransactionProposed.ts`)

Same change when `result.status === "invalid"`, using `OracleTransactionPacket`.

#### Updated `handleSign` (`sign.ts`)

Add a branch for `"waiting_to_decline"` before the existing `"waiting_for_request"` check. Nonce replenishment (`checkAvailableNonces`) is skipped — a declining validator will not participate in the ceremony:

```typescript
if (status?.id === "waiting_to_decline") {
    const callbackContext = buildDeclineCallbackContext(status.packet);
    return {
        signing: [event.message], // clears the state
        actions: [{ id: "sign_decline_with_callback", signatureId: event.sid, callbackContext }],
    };
}
```

Note the absence of `...diff` (nonce replenishment diff) — a declining validator should not trigger nonce registration for a ceremony it is opting out of.

#### New callback context builder (`signing/declines.ts`)

Analogous to `buildCallbackContext` in `nonces.ts`, encoding the function selector and arguments for the Consensus callback. Place in a new `validator/src/machine/signing/declines.ts`:

```typescript
export const buildDeclineCallbackContext = (
    packet: SafeTransactionPacket | OracleTransactionPacket,
): Hex => {
    if (packet.type === "safe_transaction_packet") {
        const { chainId, safe, ...txData } = packet.proposal.transaction;
        return encodeFunctionData({
            abi: CONSENSUS_FUNCTIONS,
            functionName: "rejectTransaction",
            args: [packet.proposal.epoch, chainId, safe, safeTxStructHash(txData), zeroHash],
        });
    }
    const { chainId, safe, ...txData } = packet.proposal.transaction;
    return encodeFunctionData({
        abi: CONSENSUS_FUNCTIONS,
        functionName: "rejectOracleTransaction",
        args: [packet.proposal.epoch, packet.proposal.oracle, chainId, safe, safeTxStructHash(txData), zeroHash],
    });
};
```

`CONSENSUS_FUNCTIONS` must include `rejectTransaction` and `rejectOracleTransaction` ABI entries.

The `zeroHash` passed as the `signatureId` argument in both encodings is a placeholder — it is never decoded from the context. `onSignRejected` receives the actual `signatureId` as its first parameter and passes it directly to `rejectTransaction`. This is the same convention used in `buildTransactionAttestationCallback` in `nonces.ts`.

#### New protocol action type (`consensus/protocol/types.ts`)

```typescript
export type DeclineSignature = {
    id: "sign_decline_with_callback";
    signatureId: SignatureId;
    callbackContext: Hex;
};
```

Add `DeclineSignature` to the `SigningAction` union.

#### Updated `BaseProtocol` (`consensus/protocol/base.ts`)

- Add `case "sign_decline_with_callback"` to `performAction`.
- Add abstract method `declineSignature(args: DeclineSignature): Promise<SubmittedAction>`.

#### Updated `OnchainProtocol` (`consensus/protocol/onchain.ts`)

Implement `declineSignature` by calling `FROSTCoordinator.signDeclineWithCallback(sid, { target: consensus, context: callbackContext })`:

```typescript
protected declineSignature({ signatureId, callbackContext }: DeclineSignature): Promise<SubmittedAction> {
    return this.submitAction({
        address: this.#coordinator,
        abi: COORDINATOR_FUNCTIONS,
        functionName: "signDeclineWithCallback",
        args: [
            signatureId,
            { target: this.#consensus, context: callbackContext },
        ],
        gas: 200_000n,
    });
}
```

`COORDINATOR_FUNCTIONS` must include `signDeclineWithCallback` ABI entry.

#### `NoncesAlreadyUsed` handling

This is a terminal, non-retryable outcome — another validator instance submitted the same decline. Catch this revert in `onchain.ts`, resolve as a completed action, and log at `info` rather than `warn`.

#### Updated `timeouts.ts`

Add a timeout case for `"waiting_to_decline"`: simply drop the state, no retry. If the `Sign` event is never observed (e.g., it was missed), the signing state times out and the validator gives up. No decline is submitted.

#### Test cases (Phase 3)

- Invalid safe tx produces a `"waiting_to_decline"` signing state with the correct `SafeTransactionPacket`.
- Invalid oracle tx produces a `"waiting_to_decline"` signing state with the correct `OracleTransactionPacket`.
- `Sign` event for a `"waiting_to_decline"` message emits `sign_decline_with_callback` action with correct callback context, clears state, and does NOT include nonce replenishment actions.
- `Sign` event for a `"waiting_for_request"` message (valid tx) is unaffected.
- `NoncesAlreadyUsed` revert resolves without retry, logged at info.
- Validator restart while in `"waiting_to_decline"`: state is restored from SQLite and decline is submitted after the `Sign` event is re-observed.
- Timeout in `"waiting_to_decline"`: state is cleared, no action emitted.

---

### Phase 4 — Explorer: Rejection Status

#### Updated `ProposalStatus` (`consensus/transactions.ts`)

```typescript
export type ProposalStatus = "ATTESTED" | "PROPOSED" | "TIMED_OUT" | "REJECTED";
```

Status precedence: `ATTESTED` > `REJECTED` > `TIMED_OUT` > `PROPOSED`.

#### Updated status derivation (`consensus/transactions.ts`)

`loadTransactionProposals` already queries `TransactionProposed` and `TransactionAttested` from Consensus. Add `TransactionRejected` to the same query. Status is `REJECTED` when `TransactionRejected` exists and no `TransactionAttested` exists for the same message. No cross-contract correlation needed.

#### Updated ABI

Add `TransactionRejected` and `OracleTransactionRejected` events to the Consensus ABI used by the explorer.

#### Test cases (Phase 4)

- `loadTransactionProposals` with `TransactionRejected` event: `status: "REJECTED"`.
- `loadTransactionProposals` with `TransactionAttested` and `TransactionRejected` both present (should not occur in practice, but guard for precedence): `status: "ATTESTED"`.
- Proposal past timeout with no declines: `status: "TIMED_OUT"`.

---

### Phase 5 — Explorer: Declined Breakdown

#### Updated `AttestationStatus` type (`coordinator/signing.ts`)

```typescript
type AttestationStatus = {
    status: "pending" | "completed" | "error";
    sid: Hex;
    groupId: Hex;
    sequence: bigint;
    committed: AttestationParticipation[];
    signed: AttestationParticipation[];
    declined: AttestationParticipation[];   // new
    signature?: Hex;
};
```

#### Updated signing progress query (`coordinator/signing.ts`)

Add `SignDeclined` event to the existing log queries for a SID. Populate `declined` from those events, keyed by participant address. The SID is already known from the existing `Sign` event correlation.

Add `SignDeclined` event to the coordinator ABI used by the explorer.

**SID scope note**: `signDecline` fires for all coordinator ceremonies (epoch rollover, oracle transactions). Since `useAttestationStatus` is already scoped to a specific proposal's SID (derived from the `Sign` event for that proposal's message), `SignDeclined` events are naturally filtered to the correct ceremony.

#### Updated `SafeTxAttestationStatus.tsx`

Add a "Declined" row displaying validator addresses from `status.declined`, following the same pattern as the existing "Committed" and "Signed" rows.

#### Test cases (Phase 5)

- Signing progress with `SignDeclined` events populates `declined` correctly.
- "Declined" row renders validator addresses from `status.declined`.

---

## Implementation Phases

### Phase 1 — FROSTCoordinator (PR 1)
**Can start immediately.**

Completes the coordinator side of the feature in isolation: decline tracking and threshold-based stopping. `signDeclineWithCallback` is intentionally excluded — it requires `IFROSTCoordinatorCallback.onSignRejected` which does not exist until Phase 2.

Files:
- `contracts/src/FROSTCoordinator.sol` — `Signature` struct update (`declineCount`, `shares` field), `SignDeclined`/`SignRejected` events, `signDecline` (using `FROSTNonceCommitmentSet.burn`, equality check for `SignRejected`, no `rejected` flag write).
- `contracts/src/libraries/FROSTNonceCommitmentSet.sol` — `used` boolean in `Commitments`, `NoncesAlreadyUsed` error, `verify()` made non-view (sets `used`), `burn()`.
- `contracts/test/FROSTCoordinator.t.sol` — `test_SignDecline_ThresholdReached_EmitsSignRejected` added. Full coverage in stacked PRs.

### Phase 2 — Consensus Callback (PR 2)
**Can start after Phase 1 is merged.**

Completes the on-chain rejection story: `Consensus` learns about rejected ceremonies via the coordinator callback and emits `TransactionRejected`/`OracleTransactionRejected`. At this point the full on-chain flow is end-to-end testable.

Files:
- `contracts/src/FROSTCoordinator.sol` — Add `signDeclineWithCallback`.
- `contracts/src/interfaces/IFROSTCoordinatorCallback.sol` — Add `onSignRejected`.
- `contracts/src/interfaces/IConsensus.sol` — Add `TransactionRejected`, `OracleTransactionRejected` events.
- `contracts/src/Consensus.sol` — `$rejections` storage, `AlreadyRejected`/`NotRejected` errors, `rejectTransaction`/`rejectOracleTransaction`, `onSignRejected` implementation.
- `contracts/test/` — Tests for the full rejection callback chain (`signDeclineWithCallback` → `onSignRejected` → `rejectTransaction` → event emitted).

### Phase 3 — Validator (PR 3)
**Can start and merge after Phase 1 is merged. Independent of Phase 2.**

Develop using local ABI drafts for `signDeclineWithCallback` (coordinator, Phase 2) and `rejectTransaction`/`rejectOracleTransaction` (Consensus, Phase 2); update to the real ABI before merging.

Files:
- `validator/src/machine/types.ts` — Add `"waiting_to_decline"` to `SigningState`.
- `validator/src/machine/consensus/transactionProposed.ts` — Return `"waiting_to_decline"` state for invalid safe tx.
- `validator/src/machine/consensus/oracleTransactionProposed.ts` — Same for invalid oracle tx.
- `validator/src/machine/signing/sign.ts` — Handle `"waiting_to_decline"` state.
- `validator/src/machine/signing/declines.ts` — `buildDeclineCallbackContext`.
- `validator/src/machine/signing/timeouts.ts` — Add timeout case for `"waiting_to_decline"`.
- `validator/src/consensus/protocol/types.ts` — `DeclineSignature` type, updated `SigningAction`.
- `validator/src/consensus/protocol/base.ts` — New case in `performAction`, abstract method.
- `validator/src/consensus/protocol/onchain.ts` — Implement `declineSignature` with terminal error handling.
- `validator/src/machine/signing/sign.test.ts` — Tests.
- `validator/src/machine/consensus/transactionProposed.test.ts` — Tests.
- `validator/src/machine/consensus/oracleTransactionProposed.test.ts` — Tests.
- `validator/src/machine/storage/sqlite.test.ts` — Test persistence and restore of `"waiting_to_decline"` state.

### Phase 4 — Explorer: Rejection Status (PR 4)
**Can start after Phase 2 is merged.**

Adds the `REJECTED` proposal status. Entirely Consensus-side: queries `TransactionRejected` alongside existing events, no coordinator interaction.

Files:
- `explorer/src/lib/consensus/transactions.ts` — Add `"REJECTED"` to `ProposalStatus`, query `TransactionRejected` events.
- `explorer/src/lib/consensus/abi.ts` (or equivalent) — Add `TransactionRejected`/`OracleTransactionRejected` events.
- Tests for the above.

### Phase 5 — Explorer: Declined Breakdown (PR 5)
**Can start after Phase 1 is merged. Can be developed in parallel with Phases 2, 3, and 4; should merge after Phase 4 for UX coherence (showing who declined is most useful once the REJECTED status is visible).**

Adds the per-validator "Declined" row. Entirely coordinator-side: queries `SignDeclined` events for a known SID, no Consensus interaction.

Files:
- `explorer/src/lib/coordinator/signing.ts` — Add `declined` to `AttestationStatus`, query `SignDeclined` events.
- `explorer/src/lib/coordinator/abi.ts` (or equivalent) — Add `SignDeclined` event.
- `explorer/src/components/transaction/SafeTxAttestationStatus.tsx` — Add "Declined" row.
- Tests for the above.

---

## PR Workflow

### Dependency graph

- **PR 1** — no dependencies
- **PR 2** — depends on PR 1
- **PR 3** — depends on PR 1 (independent of PR 2)
- **PR 4** — depends on PR 2
- **PR 5** — depends on PR 1; merge after PR 4 for UX coherence

### Merge sequence

| PR | Merge after | Can develop after |
|----|-------------|-------------------|
| PR 1 | — | immediately |
| PR 2 | PR 1 | PR 1 |
| PR 3 | PR 1 | PR 1 (use local ABI drafts for PR 2's `signDeclineWithCallback` and Consensus additions) |
| PR 4 | PR 2 | PR 2 |
| PR 5 | PR 4 | PR 1 (use local ABI drafts; merge after PR 4 for UX) |

**Parallel tracks**: PRs 2 and 3 are fully independent of each other — both branch from PR 1. PRs 4 and 5 can be developed in parallel with each other and with PR 3.

**Note on local ABI drafts**: PR 3 needs `signDeclineWithCallback` (added to coordinator in PR 2) and `rejectTransaction`/`rejectOracleTransaction` (Consensus, PR 2) for the callback context encoding. PR 5 needs `SignDeclined` (coordinator, PR 1). When developing ahead of the upstream PR, stub the relevant ABI entries locally and update to the real ABI before merging.

---

## PR Descriptions

### PR 1 — FROSTCoordinator: Decline Tracking and Threshold Stopping

> Adds per-participant decline recording and threshold-based ceremony stopping to `FROSTCoordinator`. Once enough validators decline, the ceremony is definitively marked rejected and further signing reverts.

#### Context and Motivation

- Currently, validators silently drop transactions they consider invalid. The signing ceremony then times out with no on-chain record, making rejection indistinguishable from a timeout or error.
- This PR lays the on-chain foundation: validators can explicitly opt out of a ceremony via `signDecline`, and when enough have done so (`count - threshold + 1`), the ceremony is blocked from completing.
- `signDeclineWithCallback` (the callback to Consensus) is intentionally excluded — it requires `IFROSTCoordinatorCallback.onSignRejected` which does not exist until PR 2. The two must be introduced together.

#### Decisions and Tradeoffs

- **Decline is a flag, no reason code**: avoids maintaining a Solidity/TypeScript enum in sync and avoids a contract upgrade path every time a new validation check is added.
- **`declineCount` lives in the `Signature` struct**: keeps ceremony-level aggregate state together. `rejected` is not stored — rejection is computable as `declineCount > group.count - group.threshold`.
- **Per-participant state lives in `FROSTNonceCommitmentSet`**: a single `used` boolean per `(participant, sequence)` consolidates the former `declined` mapping and `anyShared` flag into a single source of truth. The nonce set is the natural home because it is already keyed by sequence, the same key space as ceremonies.
- **Threshold formula `count - threshold + 1`**: this is the exact minimum number of declines that makes the ceremony mathematically uncompletable regardless of who is left. Blocking signing only at this point ensures the guard is never a false positive.
- **Advisory before threshold**: the ceremony can still complete if threshold participants sign before enough declines accumulate. This preserves liveness during rolling validator upgrades where older-version validators may decline transactions the majority considers valid.
- **Additional declines after threshold are accepted**: subsequent `signDecline` calls still record `SignDeclined` events (useful for the explorer's per-validator breakdown) but `SignRejected` is not re-emitted — the equality check `declineCount == rejectThreshold` ensures exactly one emission.
- **`signDecline` returns `bool rejected`**: mirrors `signShare` returning `bool signed`, enabling the callback wrapper in PR 2 to follow the identical pattern.

#### Testing

- PR 1 ships one focused test: `test_SignDecline_ThresholdReached_EmitsSignRejected` — `count - threshold + 1` participants declining emits `SignRejected` and the final call returns `true`.
- Full coverage follows in four stacked PRs (threshold boundary, mutual exclusion, error paths, post-rejection/post-completion behaviour). See "Test cases (Phase 1)" above.

---

### PR 2 — Consensus Callback: `onSignRejected` and `TransactionRejected`

> Wires the coordinator's rejection threshold to Consensus via a callback, emitting `TransactionRejected`/`OracleTransactionRejected` on the same contract as `TransactionProposed` and `TransactionAttested`.

#### Context and Motivation

- PR 1 added the coordinator mechanics. This PR completes the on-chain story: when enough validators decline, `Consensus` emits `TransactionRejected`, making rejection a first-class event alongside proposal and attestation.
- By placing `TransactionRejected` on `Consensus` (not the coordinator), the explorer can derive all proposal status from a single contract — no cross-contract SID correlation needed.

#### Decisions and Tradeoffs

- **`signDeclineWithCallback` added here, not in PR 1**: it calls `callback.target.onSignRejected`, which did not exist on `IFROSTCoordinatorCallback` until this PR. The coordinator function and the interface method must be introduced together.
- **Mirrors `signShareWithCallback` → `onSignCompleted` exactly**: same `Callback` struct, same dispatch-on-selector pattern in `onSignCompleted`/`onSignRejected`, same context encoding (`abi.encode(selector, args)`). Reviewers familiar with the existing flow will find nothing surprising.
- **`rejectTransaction`/`rejectOracleTransaction` are public**: matches `attestTransaction`/`attestOracleTransaction`. They validate via `_COORDINATOR.isSignRejected(signatureId)` to guard against direct calls that bypass the callback. The `onlyCoordinator` guard on `onSignRejected` ensures the callback path is always safe.
- **`$rejections` storage mirrors `$attestations`**: prevents double-rejection recording and provides a consistent storage pattern.

#### Testing

- Full callback chain: `signDeclineWithCallback` → `onSignRejected` → `rejectTransaction` → `TransactionRejected` event emitted.
- `onSignRejected` selector dispatch for both safe tx and oracle tx contexts.
- `rejectTransaction` with unrejected SID reverts `NotRejected`.
- `rejectTransaction` called twice reverts `AlreadyRejected`.

---

### PR 3 — Validator: `"waiting_to_decline"` State and Decline Action

> Validators now explicitly decline signing ceremonies for transactions they consider invalid, instead of silently ignoring them. Adds a `"waiting_to_decline"` signing state and a `sign_decline_with_callback` protocol action.

#### Context and Motivation

- PRs 1 and 2 set up the on-chain machinery. This PR makes validators actually use it.
- `handleTransactionProposed` currently returns `{}` for invalid transactions (a silent drop). This PR changes that to return a `"waiting_to_decline"` state, so when the `Sign` event arrives, the validator calls `signDeclineWithCallback` on the coordinator.

#### Decisions and Tradeoffs

- **`"waiting_to_decline"` slots into the existing `handleSign` dispatch**: `handleSign` already branches on `status.id`. The new branch fires before `"waiting_for_request"` and skips nonce replenishment entirely — a declining validator will never need nonces for a ceremony it is opting out of.
- **`packet` is required in the state**: (1) `BaseSigningState` requires it, (2) the SQLite persistence layer needs it to restore state across restarts, (3) it provides `epoch`/`safe`/`chainId` to build the callback context for `onSignRejected`.
- **Callback context built in `declines.ts`**: mirrors `buildTransactionAttestationCallback`/`buildOracleTransactionAttestationCallback` from `nonces.ts`, using `rejectTransaction`/`rejectOracleTransaction` selectors.
- **Timeout drops the state with no retry**: if the `Sign` event is never observed, the validator gives up silently. The ceremony will time out on-chain anyway; submitting a late decline after timeout provides no value.
- **`NoncesAlreadyUsed` is terminal, logged at `info` not `warn`**: this is a benign race — another validator instance submitted the same decline. It requires no intervention.

#### Testing

- Invalid safe tx and oracle tx produce correct `"waiting_to_decline"` state with the right packet.
- `Sign` event for `"waiting_to_decline"` emits `sign_decline_with_callback` with correct callback context, clears state, no nonce replenishment.
- `Sign` event for `"waiting_for_request"` (valid tx) is unaffected.
- `NoncesAlreadyUsed` (double-decline) revert resolves without retry, logged at `info`.
- Validator restart: `"waiting_to_decline"` state is restored from SQLite and decline is submitted after `Sign` is re-observed.
- Timeout: state is cleared, no action emitted.

---

### PR 4 — Explorer: Transaction Rejection Status

> Adds `REJECTED` as a new `ProposalStatus`, derived from `TransactionRejected` events on Consensus. No coordinator interaction required.

#### Context and Motivation

- PR 2 emits `TransactionRejected` on `Consensus`. This PR surfaces it in the explorer so users see `REJECTED` instead of `TIMED_OUT` for transactions that were explicitly declined by the validator set.
- The change is entirely within `consensus/transactions.ts` — `TransactionRejected` is queried alongside the existing `TransactionProposed` and `TransactionAttested` events from the same contract.

#### Decisions and Tradeoffs

- **Single-source status derivation**: an earlier design would have required cross-referencing `SignDeclined` events from the coordinator with the proposal's SID. By placing `TransactionRejected` on `Consensus`, the explorer needs only one contract for all proposal status logic.
- **`ATTESTED` takes precedence over `REJECTED`**: if a ceremony somehow completes after partial declines, `ATTESTED` is shown. In practice this cannot happen — after the rejection threshold is crossed, the remaining non-declined participants are fewer than `threshold`, so the ceremony can never accumulate enough shares to complete.

#### Testing

- `loadTransactionProposals` with `TransactionRejected`: `status: "REJECTED"`.
- `loadTransactionProposals` with both `TransactionRejected` and `TransactionAttested`: `status: "ATTESTED"`.
- Proposal past timeout with no rejection event: `status: "TIMED_OUT"`.

---

### PR 5 — Explorer: Declined Validators Breakdown

> Adds a "Declined" row to the attestation status UI showing which validators explicitly opted out of a signing ceremony, sourced from `SignDeclined` events on the coordinator.

#### Context and Motivation

- PR 4 shows that a transaction was rejected. This PR shows who declined it — useful for debugging and operator visibility.
- The change is entirely within the coordinator signing hook (`coordinator/signing.ts`) and the attestation status component — independent of `TransactionRejected` on Consensus.
- Merged after PR 4 so that users see the `REJECTED` status before the breakdown is added; showing declined validators without the status label would be confusing.

#### Decisions and Tradeoffs

- **`SignDeclined` events are naturally scoped**: `useAttestationStatus` is already scoped to a specific proposal's SID (derived from the `Sign` event for that proposal's message). `SignDeclined` events are therefore automatically filtered to the correct ceremony — no risk of surfacing declines from epoch rollover or oracle tx ceremonies on a safe tx proposal page.
- **Parallel development with PR 4**: PR 5 only needs `SignDeclined` from the coordinator (available since PR 1). It is developed in parallel with PRs 2–4 and merged last among the explorer PRs purely for UX coherence.

#### Testing

- Signing progress with `SignDeclined` events populates `declined` correctly.
- "Declined" row renders the correct validator addresses.

---

## Open Questions / Assumptions

- **`Signature` struct storage layout**: Only `uint16 declineCount` is added to `Signature`. `bool rejected` is not stored — rejection is computed from `declineCount` and group parameters. Verify packing does not break any assembly or low-level access patterns in `FROSTSignatureShares`.
- **Oracle handler name**: The spec refers to `handleOracleTransactionProposed` in `oracleTransactionProposed.ts` — confirmed from the codebase.
- **Devnet deployment**: After all phases are merged, a devnet redeployment is required to pick up the new coordinator and consensus interfaces.
- **Decline before `Sign` event**: Validators cannot decline before the `Sign` event because they need the SID. The `"waiting_to_decline"` state ensures the decline is submitted as soon as the `Sign` event is observed (emitted in the same transaction as `proposeTransaction`). This is by design and requires no special handling.
- **`signRevealNonces` after rejection**: No `CeremonyRejected` guard is added to `signRevealNonces`. A validator that reveals nonces before the rejection threshold is crossed can do nothing with those nonces — `signShare` will succeed but not advance the ceremony past the mathematical impossibility (not enough non-declined participants remain). This is harmless. Note: attempting to decline after revealing nonces reverts with `NoncesAlreadyUsed` (thrown by `FROSTNonceCommitmentSet.burn()`), not with an error from `signShare`.
- **Validator that already called `signRevealNonces` cannot then call `signDecline`**: `burn()` reverts with `NoncesAlreadyUsed` if the slot is already consumed. This is intentional — nonce revelation signals intent to participate; the validator is committed to the ceremony for that sequence once they reveal. If they want to not participate, they should decline *before* revealing nonces.
- **Gas estimate for `declineSignature`**: `200_000n` gas is set in the spec. The call chain `signDeclineWithCallback` → `onSignRejected` → `rejectTransaction` involves two cross-contract calls plus an `isSignRejected` view call and storage writes on both contracts. Verify this estimate against the actual implementation before finalising; increase if necessary (compare with `signShare`'s `400_000n` as an upper bound).
