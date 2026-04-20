# Feature Proposal: Oracle Interface and Collaborative Transaction Checks
Component: `all`

---

## Overview

This feature introduces an oracle framework to Safenet, enabling external contracts to participate in transaction security checks. Rather than relying solely on deterministic validator-side logic, oracles allow market participants and off-chain data sources to express security judgements that validators then attest to using FROST threshold signatures.

The implementation is structured in four independently releasable PRs:

1. **PR 1 — Oracle contract interface + Consensus extension**: Define `IOracle.sol`, add `OracleTransactionProposal` as a new FROST-signed package type in `Consensus.sol`.
2. **PR 2 — Validator new package type** (parallel to PR 1): New `oracle_transaction_packet` handler, EIP-712 hashing, and machine states for `OracleTransactionProposed` / `OracleTransactionAttested` events.
3. **PR 3 — Validator oracle event integration** (depends on PR 2): Oracle event listener, `wait_for_oracle` signing state with configurable timeout, and `OracleResult` machine handler.
4. **PR 4 — PoC oracle implementations + game rules** (parallel to PR 2/3, depends on PR 1): Reference oracle contracts, oracle game rules documentation, and end-to-end integration test.

---

## Architecture Decision

### Oracle Message Design

Each oracle-checked transaction is identified by an EIP-712 message:

```
OracleTransactionProposal(uint64 epoch, address oracle, bytes32 safeTxHash)
```

The `oracle` address is embedded in the signed message rather than stored as contract configuration. This means:
- Different oracle contracts produce distinct message hashes (explicit oracle selection, no ambiguity).
- `Consensus.sol` requires no owner or configuration setter — it remains assertion-style.
- Validators maintain their own allowlist of trusted oracle addresses in `MachineConfig`; they simply refuse to sign for oracles not on the list.

### Oracle Request/Result Linkage

The EIP-712 hash of the `OracleTransactionProposal` message serves as the `requestId` that links an oracle result back to the signing request:

```
requestId = keccak256("\x19\x01" || domainSeparator || keccak256(OracleTransactionProposal(...)))
```

When the oracle emits `OracleResult(requestId, result, approved)`, validators correlate it to the pending signing state via this `requestId`.

### New Signing State: `wait_for_oracle`

The existing signing state machine for transactions is extended with an intermediate state:

```
waiting_for_request
  → [OracleTransactionProposed event]
wait_for_oracle              ← new state (timeout: ORACLE_TIMEOUT_BLOCKS env var)
  → [OracleResult(approved=true) event]
collect_nonce_commitments
  → ... (existing FROST signing flow)
waiting_for_attestation
  → [OracleTransactionAttested event]
(done)
```

If the oracle result is `approved = false`, or if the oracle address is not in the validator's allowlist, or if the `ORACLE_TIMEOUT_BLOCKS` deadline is reached without a result, the signing request is dropped and state is cleaned up silently.

### No Owner on Consensus

`Consensus.sol` already contains no owner or config setters for epoch/transaction logic; this design is preserved. Oracle configuration (which oracles to trust) lives entirely in the validator's off-chain config, and the oracle address chosen by the transaction proposer is encoded in the on-chain message.

### Security Considerations

`proposeOracleTransaction` calls an arbitrary user-supplied `oracle` address. This is safe for the following reasons:

- **No value transferred**: The call is `oracle.postRequest(requestId)` — no ETH is sent.
- **Fixed calldata**: The selector and argument type are fixed by `IOracle`; the only variable is the `requestId` (a `bytes32`), so the oracle cannot be manipulated into executing arbitrary logic on the Consensus contract's behalf.
- **Call-last ordering with gas cap**: The oracle call is made as the final statement in `proposeOracleTransaction`, after all state changes and event emissions, using a low-level call with a fixed gas stipend (e.g. `oracle.call{gas: 50_000}(abi.encodeCall(IOracle.postRequest, (requestId)))`). This eliminates reentrancy impact and ensures a malicious or broken oracle (non-contract, reverting, or gas-exhausting) cannot prevent the transaction from being proposed — the call failure is silently ignored.
- **Validator allowlist**: Validators independently verify that the oracle address is trusted before signing. A malicious oracle that emits a false approval cannot produce a FROST signature without a validator quorum.

### Alternatives Considered

- **Storing approved oracles in Consensus.sol via governance**: Rejected — adds an owner/admin pattern and requires governance to add new oracle types. Validator-side allowlisting is more flexible and consistent with the existing no-owner design.
- **Passing full transaction data to the oracle**: Rejected — the oracle can independently fetch transaction data from the `OracleTransactionProposed` event; only the `requestId` (message hash) needs to be passed to correlate the result.
- **Reusing the existing `TransactionProposed` event with an oracle field**: Rejected — mixing oracle and non-oracle transactions in the same event makes filtering and attestation lookup ambiguous. A distinct package type cleanly separates the flows.
- **Synchronous oracle checks in `OracleTransactionHandler.hashAndVerify()`**: Rejected — oracle results arrive asynchronously via on-chain events. The handler verifies packet structure only; oracle result arrival drives signing participation via a new machine transition.

---

## User Flow

Oracle-checked transactions are proposed on-chain similarly to regular transactions. The proposer additionally selects an oracle contract and posts the request to it. Validators then watch for the oracle result before participating in signing.

```
Proposer                Consensus.sol           Oracle.sol          Validators
   |                        |                       |                   |
   |-- proposeOracleTransaction(oracle, tx) ------->|                   |
   |<-- (safeTxHash, requestId) --------------------|                   |
   |                        |-- OracleTransactionProposed(requestId) -->|
   |-- postRequest(requestId) ------------------- >|                   |
   |                        |                       |-- OracleResult -->|
   |                        |                       |   (approved=true) |
   |                        |<-- sign(groupId, oracleMsg) -------------|
   |                        |       (FROST signing round)              |
   |                        |-- OracleTransactionAttested event ------>|
   |<-- getOracleTransactionAttestation() ----------|                  |
```

---

## Tech Specs

### Smart Contract Changes

#### New: `contracts/src/interfaces/IOracle.sol`

```solidity
interface IOracle {
    /// @notice Emitted when an oracle produces a result for a request.
    /// @param requestId The EIP-712 hash of the OracleTransactionProposal message.
    /// @param result Arbitrary result data (oracle-specific encoding).
    /// @param approved Whether the oracle approves the transaction.
    event OracleResult(
        bytes32 indexed requestId,
        bytes result,
        bool approved
    );

    /// @notice Post a signing request to the oracle for evaluation.
    /// @param requestId The EIP-712 hash of the OracleTransactionProposal message.
    /// @dev Transaction data is not passed here; the oracle is expected to fetch it
    ///      independently from the OracleTransactionProposed event.
    function postRequest(bytes32 requestId) external;
}
```

#### Modified: `contracts/src/libraries/ConsensusMessages.sol`

Add:
```solidity
/// @custom:precomputed keccak256("OracleTransactionProposal(uint64 epoch,address oracle,bytes32 safeTxHash)")
bytes32 internal constant ORACLE_TRANSACTION_PROPOSAL_TYPEHASH = ...;

function oracleTransactionProposal(
    bytes32 domainSeparator,
    uint64 epoch,
    address oracle,
    bytes32 safeTxHash
) internal pure returns (bytes32 result);
```

#### Modified: `contracts/src/interfaces/IConsensus.sol` and `contracts/src/Consensus.sol`

New events:
```solidity
event OracleTransactionProposed(
    bytes32 indexed safeTxHash,
    uint256 indexed chainId,
    address indexed safe,
    bytes32 requestId,   // EIP-712 hash — enables direct correlation with OracleResult
    uint64 epoch,
    address oracle,
    SafeTransaction.T transaction
);

event OracleTransactionAttested(
    bytes32 indexed safeTxHash,
    uint256 indexed chainId,
    address indexed safe,
    uint64 epoch,
    address oracle,
    FROSTSignatureId.T signatureId,
    FROST.Signature attestation
);
```

New functions:
```solidity
/// @notice Proposes a transaction for oracle-checked validator approval.
/// @param oracle Address of the oracle contract to use for evaluation.
/// @param transaction The Safe transaction to propose.
/// @return safeTxHash The Safe transaction hash.
/// @return requestId The EIP-712 hash of the OracleTransactionProposal message,
///         needed to call oracle.postRequest() and to correlate OracleResult events.
function proposeOracleTransaction(
    address oracle,
    SafeTransaction.T memory transaction
) external returns (bytes32 safeTxHash, bytes32 requestId);

/// @notice Attests to an oracle-checked transaction.
/// Called internally via onSignCompleted callback.
function attestOracleTransaction(
    uint64 epoch,
    address oracle,
    uint256 chainId,
    address safe,
    bytes32 safeTxStructHash,
    FROSTSignatureId.T signatureId
) external;

/// @notice Retrieves an oracle transaction attestation.
function getOracleTransactionAttestationByHash(
    uint64 epoch,
    address oracle,
    bytes32 safeTxHash
) external view returns (FROST.Signature memory signature);
```

Context encoding in `onSignCompleted` for the new package type:
```solidity
abi.encode(
    bytes4(this.attestOracleTransaction.selector),
    uint64 epoch,
    address oracle,
    uint256 chainId,
    address safe,
    bytes32 safeTxStructHash
)
```

#### New: `contracts/src/SimpleOracle.sol` (PoC)

A minimal oracle for testing where a designated approver address can call `approve(requestId)` or `reject(requestId)`. On approval, emits `OracleResult(requestId, "", true)`. On rejection, emits `OracleResult(requestId, "", false)`. `postRequest` records the `requestId` as pending but does not emit a result — the approver must act explicitly.

#### New: `contracts/src/AlwaysApproveOracle.sol` (PoC)

Immediately emits `OracleResult(requestId, "", true)` inside `postRequest`. Useful for integration testing and demonstrating the end-to-end flow without manual interaction.

### Validator Changes

#### New: `validator/src/consensus/verify/oracleTx/schemas.ts`

```typescript
export const oracleTransactionPacketSchema = z.object({
    type: z.literal("oracle_transaction_packet"),
    domain: consensusDomainSchema,
    proposal: z.object({
        epoch: z.bigint().nonnegative(),
        oracle: checkedAddressSchema,
        transaction: safeTransactionSchema,
    }),
});
export type OracleTransactionPacket = z.infer<typeof oracleTransactionPacketSchema>;
```

#### New: `validator/src/consensus/verify/oracleTx/hashing.ts`

EIP-712 hash for `OracleTransactionProposal` — mirrors `safeTxProposalHash` in `safeTx/hashing.ts` but includes the `oracle` field.

#### New: `validator/src/consensus/verify/oracleTx/handler.ts`

`OracleTransactionHandler implements PacketHandler<OracleTransactionPacket>`: validates packet structure, checks oracle address against the validator's allowlist (passed via constructor), and returns the EIP-712 message hash. Does not wait for oracle result — that is handled by the machine state.

#### New: `validator/src/machine/consensus/oracleTransactionProposed.ts`

Handles `OracleTransactionProposed` event. Returns `StateDiff` with a new `wait_for_oracle` signing entry, recording the oracle address, packet, and a deadline of `event.block + oracleTimeout` (read from `MachineConfig.oracleTimeout`, sourced from env var `ORACLE_TIMEOUT_BLOCKS`).

#### New: `validator/src/machine/consensus/oracleResult.ts`

Handles `OracleResult` event from oracle contracts. If `approved = true` and state is `wait_for_oracle`, transitions to the signing flow (`collect_nonce_commitments`). If `approved = false`, drops state. If the current block exceeds the deadline (checked on each new-block transition), state is also dropped.

#### New: `validator/src/machine/consensus/oracleTransactionAttested.ts`

Handles `OracleTransactionAttested` event. Mirrors `handleTransactionAttested` — cleans up signing state for the completed oracle-transaction.

#### Modified: `validator/src/machine/transitions/types.ts`

Add new transition types:
- `event_oracle_transaction_proposed`
- `event_oracle_result`
- `event_oracle_transaction_attested`

#### Modified: `validator/src/machine/types.ts`

Extend `SigningStateData` union with `wait_for_oracle` state variant, storing the oracle address, packet, and block deadline for later reference.

#### Modified: `validator/src/types/abis.ts`

Add `IOracle.OracleResult` ABI event definition for use by the watcher.

#### Modified: `validator/src/service/service.ts`

- Register `"oracle_transaction_packet"` handler in `verificationHandlers`.
- Add oracle contract address(es) from config to the watcher's event subscription.
- Add `allowedOracles: Address[]` field to `MachineConfig`.
- Add `oracleTimeout: bigint` field to `MachineConfig`, sourced from env var `ORACLE_TIMEOUT_BLOCKS` (blocks to wait for an oracle result before dropping the signing request).

### Oracle Game Rules (new doc: `docs/oracle-game-rules.md`)

| Rule | Description |
|------|-------------|
| **Request identity** | `requestId` = EIP-712 hash of `OracleTransactionProposal(epoch, oracle, safeTxHash)` |
| **Finality** | The first `OracleResult` emitted for a given `requestId` is final; subsequent emissions are ignored by validators |
| **Validator allowlist** | Validators only respond to oracles on their configured allowlist; unlisted oracles are ignored silently |
| **Signing threshold** | Standard FROST threshold (same as regular transaction attestation) applies; oracle approval alone does not attest — a quorum of validators must sign |
| **No double-attest** | `attestOracleTransaction` reverts if a signature already exists for the message (mirroring `attestTransaction`) |
| **Oracle-agnostic Consensus** | `Consensus.sol` does not validate oracle correctness; that responsibility belongs to validators via their allowlist |

### Test Cases

| Test | Location |
|------|----------|
| `ORACLE_TRANSACTION_PROPOSAL_TYPEHASH` matches expected keccak256 | `contracts/test/libraries/ConsensusMessages.t.sol` |
| `proposeOracleTransaction` emits `OracleTransactionProposed` with correct fields | `contracts/test/Consensus.t.sol` |
| `attestOracleTransaction` stores attestation and emits `OracleTransactionAttested` | `contracts/test/Consensus.t.sol` |
| Double attest reverts | `contracts/test/Consensus.t.sol` |
| `getOracleTransactionAttestationByHash` returns correct signature | `contracts/test/Consensus.t.sol` |
| `SimpleOracle.approve` emits `OracleResult(approved=true)` | `contracts/test/SimpleOracle.t.sol` |
| `SimpleOracle.reject` emits `OracleResult(approved=false)` | `contracts/test/SimpleOracle.t.sol` |
| `AlwaysApproveOracle.postRequest` emits `OracleResult` immediately | `contracts/test/AlwaysApproveOracle.t.sol` |
| `oracleTransactionPacketSchema` rejects invalid oracle address | `validator/src/consensus/verify/oracleTx/handler.test.ts` |
| `OracleTransactionHandler.hashAndVerify` rejects unlisted oracle | `validator/src/consensus/verify/oracleTx/handler.test.ts` |
| `OracleTransactionHandler.hashAndVerify` returns correct EIP-712 hash | `validator/src/consensus/verify/oracleTx/handler.test.ts` |
| `handleOracleTransactionProposed` stores `wait_for_oracle` state | `validator/src/machine/consensus/oracleTransactionProposed.test.ts` |
| `handleOracleResult` (approved) transitions to signing | `validator/src/machine/consensus/oracleResult.test.ts` |
| `handleOracleResult` (rejected) cleans up state | `validator/src/machine/consensus/oracleResult.test.ts` |
| `handleOracleTransactionAttested` cleans up signing state | `validator/src/machine/consensus/oracleTransactionAttested.test.ts` |

---

## Implementation Phases

### Phase 1 — Oracle Interface + Consensus Contract Extension (PR 1)

**What this covers:**
- `IOracle.sol` interface with `postRequest` and `OracleResult`
- `OracleTransactionProposal` EIP-712 message type in `ConsensusMessages.sol`
- `proposeOracleTransaction` and `attestOracleTransaction` in `Consensus.sol` / `IConsensus.sol`
- Reuse existing `$attestations[bytes32 message]` mapping — oracle proposal hashes are distinct from transaction proposal hashes due to differing TypeHash, so no new mapping is needed
- `onSignCompleted` branch for the new selector
- `SimpleOracle.sol` and `AlwaysApproveOracle.sol` PoC contracts
- Full test coverage for all new contract code

**Files touched:**
- `contracts/src/interfaces/IOracle.sol` — new
- `contracts/src/libraries/ConsensusMessages.sol` — add typehash + function
- `contracts/src/interfaces/IConsensus.sol` — add events + function signatures
- `contracts/src/Consensus.sol` — implement new functions + callback branch
- `contracts/src/SimpleOracle.sol` — new PoC oracle
- `contracts/src/AlwaysApproveOracle.sol` — new PoC oracle
- `contracts/test/Consensus.t.sol` — extend with oracle tests
- `contracts/test/SimpleOracle.t.sol` — new
- `contracts/test/AlwaysApproveOracle.t.sol` — new
- `contracts/test/libraries/ConsensusMessages.t.sol` — add typehash test

---

### Phase 2 — Validator New Package Type (PR 2, parallel to PR 1)

**What this covers:**
- New `oracle_transaction_packet` type: schema, EIP-712 hashing, and `OracleTransactionHandler`
- Machine handlers for `OracleTransactionProposed` and `OracleTransactionAttested` events
- New transition types wired into the existing state machine
- Oracle allowlist in `MachineConfig`; validator ignores unlisted oracle addresses

**Files touched:**
- `validator/src/consensus/verify/oracleTx/schemas.ts` — new
- `validator/src/consensus/verify/oracleTx/hashing.ts` — new
- `validator/src/consensus/verify/oracleTx/handler.ts` — new
- `validator/src/consensus/verify/oracleTx/handler.test.ts` — new
- `validator/src/machine/consensus/oracleTransactionProposed.ts` — new
- `validator/src/machine/consensus/oracleTransactionProposed.test.ts` — new
- `validator/src/machine/consensus/oracleTransactionAttested.ts` — new
- `validator/src/machine/consensus/oracleTransactionAttested.test.ts` — new
- `validator/src/machine/transitions/types.ts` — add `event_oracle_transaction_proposed`, `event_oracle_transaction_attested`
- `validator/src/machine/transitions/onchain.ts` — map new events to transitions
- `validator/src/machine/types.ts` — add `wait_for_oracle` to `SigningStateData` union
- `validator/src/service/service.ts` — register `oracle_transaction_packet` handler; add `allowedOracles` to `MachineConfig`

---

### Phase 3 — Validator Oracle Event Integration (PR 3, depends on PR 2)

**What this covers:**
- `IOracle` ABI added to the event watcher so `OracleResult` events are indexed
- `handleOracleResult` machine handler: approved → transition to signing, rejected or timed-out → drop state
- `oracleTimeout` (`ORACLE_TIMEOUT_BLOCKS`) env var wired into `MachineConfig`; timeout checked on each `block_new` transition

**Files touched:**
- `validator/src/types/abis.ts` — add `IOracle` ABI with `OracleResult` event
- `validator/src/machine/consensus/oracleResult.ts` — new
- `validator/src/machine/consensus/oracleResult.test.ts` — new
- `validator/src/machine/transitions/types.ts` — add `event_oracle_result` transition type
- `validator/src/machine/transitions/onchain.ts` — map `OracleResult` log to transition
- `validator/src/service/service.ts` — add oracle contract(s) to watcher subscription; add `oracleTimeout` to `MachineConfig`

---

### Phase 4 — Oracle Game Rules + PoC Documentation (PR 4, parallel to PR 2/3, depends on PR 1)

**What this covers:**
- Formal documentation of oracle game rules (request identity, finality, timeout, validator allowlist)
- Survey of oracle types: deterministic (allowlist/blocklist), collaborative (multi-sig veto), time-locked (challenge period), market-based (future)
- End-to-end integration test demonstrating the full oracle flow using `AlwaysApproveOracle`

**Files touched:**
- `docs/oracle-game-rules.md` — new: game rules, oracle taxonomy, design rationale
- `contracts/script/OracleDemo.s.sol` — Forge script demonstrating oracle flow end-to-end
- `validator/src/consensus/integration.test.ts` — extend with oracle integration test scenario

---

## Open Questions / Assumptions

1. **Oracle allowlist management**: Validators configure `allowedOracles` statically in their config file. If a new oracle contract is deployed, validators must update config and restart. A more dynamic approach (on-chain registry) is left for a future iteration.
2. **Multiple oracle results for the same request**: The spec treats the first `OracleResult` as final. If an oracle emits a second (conflicting) event, validators ignore it. This assumes well-behaved oracle contracts; malicious oracles that emit multiple results would be removed from validator allowlists.
3. **Oracle result timing**: Validators wait at most `ORACLE_TIMEOUT_BLOCKS` blocks for an oracle result. After the deadline, the signing request is silently dropped. The proposer may re-propose if needed.
4. **Proposer responsibility**: The spec assumes the transaction proposer also calls `oracle.postRequest()`. If the proposer fails to do so, validators will never see an `OracleResult` and the signing state will stall until timeout. The oracle could optionally allow anyone to post a request for a given `requestId`.
5. **Oracle result storage**: Oracle contracts are not required to store results on-chain. Validators only need the event. If a validator comes online late and misses the event, it may need to query historical logs. The watcher's backfill mechanism handles this.
6. **Certora specs**: Formal verification of the new `Consensus.sol` oracle paths is deferred to a follow-up. The existing `StakingRules.spec` pattern can guide future oracle specs.
7. **Collaborative oracle design**: The most powerful oracle form — where market participants stake capital and compete on security assessments — is described in `docs/oracle-game-rules.md` but its full implementation is out of scope for this PoC. The PoC focuses on externally-controlled simple oracles to validate the interface and end-to-end flow.
