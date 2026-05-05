# Feature Proposal: Competitive Transaction Checker Oracle V1
Component: `contracts`, `validator`

---

## Overview

This feature introduces a competitive transaction checker oracle as a new `IOracle` implementation. Rather than a single designated approver (as in `SimpleOracle`), a permissioned set of *checker nodes* races to post bonded votes ("Yes" = safe, "No" = poisoned) against a transaction within a time-boxed window. The contract escrows the user's fee, collects bonds, and resolves the outcome once the window closes or bond thresholds are met. Checker nodes running alongside the validator service monitor the `NewRequest` event and submit their assessments on-chain.

**Phases (separate PRs):**
1. **Phase 1 — Core contract** (`CheckerOracle.sol`): request lifecycle, bonding, unanimous resolution, fee distribution, and timeout-default-reject logic.
2. **Phase 2 — Arbitration** (`CheckerOracle.sol` extension): conflict detection, `triggerArbitration`, `resolveDispute`, slashing, and user fee refund.
3. **Phase 3 — Checker node service** (`validator/`): off-chain checker daemon that listens for `NewRequest` events and posts bonds with Yes/No votes.

---

## Architecture Decision

### New Contract: `CheckerOracle`

`CheckerOracle` implements `IOracle` and replaces `SimpleOracle` for deployments that require competitive bonding. The `Consensus` contract calls `postRequest(requestId)` as it does today — no changes to `Consensus` or `FROSTCoordinator`.

The contract owns an internal escrow: the user's fee is locked at request time and disbursed (or refunded) at finalization. Checker bonds are locked at vote time and returned with rewards (or slashed) at resolution.

```
Consensus ──postRequest()──▶ CheckerOracle
                                  │
                    ┌─────────────┴──────────────┐
                    │                            │
              Checker Nodes                 Foundation
           (commitYes / commitNo)      (triggerArbitration /
                                         resolveDispute)
```

### Key Design Decisions

#### Asymmetric Bond Thresholds

- `yesBondTarget` — dynamic, set per request by the user as a multiple of the fee (e.g. `2x fee`). Represents the total aggregate "Yes" bond required.
- `noBondCeiling` — global, fixed low cap (e.g. `$50` denominated in the fee token). A single "No" commitment at or below this ceiling counts as a valid alarm.

This asymmetry ensures that a whistleblower can never be priced out of flagging a poisoned transaction, at the cost of cheap griefing potential (mitigated off-chain in V1 — see §Grief-and-Sweep below).

#### Explicit Finalization ("Pull over Push")

Bond thresholds being met does **not** automatically distribute funds. An explicit `finalize(requestId)` (callable by any address) or `claim(requestId)` (per checker) must be invoked after the voting window closes. This bounds gas for voting transactions, prevents reentrancy, and allows checkers to batch claims.

#### Timeout Default: Reject

If the voting window expires and neither the "Yes" aggregate threshold nor at least one valid "No" bond has been posted, the request resolves as **rejected** (defensive fail-safe). The user's fee is refunded. This sacrifices liveness in exchange for a strict defensive posture.

#### Grief-and-Sweep Attack

The contract is intentionally left mathematically vulnerable: a malicious node can post the cheap "No" bond to stall a transaction, lose arbitration, and immediately vote "Yes" on the retry to sweep the fee. In V1 this is mitigated entirely off-chain: the foundation monitors this pattern and invokes a ban + master-deposit slash on the offending checker. A dedicated contract mechanism is out of scope for V1.

#### Arbitration (Phase 2)

Conflict (at least one "Yes" and one "No" commitment) freezes the request. The foundation calls `triggerArbitration(requestId)` and, after off-chain review, `resolveDispute(requestId, winner, loser)`. The loser's bond is slashed in a two-step waterfall:
1. Full user fee refund from slashed bond.
2. Remainder to Foundation Treasury.

Because the "No" bond ceiling is set to cover arbitration costs, this keeps manual review financially self-sustaining. In V1, if the slashed bond is insufficient to cover the full fee refund, the remainder is absorbed by the treasury (deficit accepted).

### Alternatives Considered

- **Symmetric bonds**: Rejected — would price out honest whistleblowers when facing high-fee transactions.
- **Automatic push on threshold**: Rejected — reentrancy risk and unbounded gas cost for voting transactions.
- **On-chain conflict resolution without human arbitration**: Deferred to a future version. V1 assumes a permissioned checker set where conflict is rare and human oversight is practical.
- **Fee token = native ETH vs. ERC-20**: Open question (see §Open Questions). The spec is written token-agnostic; the implementation will parameterize the fee token.

---

## User Flow

### Request Submission (via Consensus)

The user submits a Safe transaction proposal through `Consensus.proposeTransaction()`. If a `CheckerOracle` is configured, `Consensus` calls `checkOracle.postRequest(requestId)` which emits `NewRequest` and locks the user's fee in escrow (fee must be approved/transferred before calling, or pulled from a user deposit — see §Open Questions).

### Checker Node Vote

Each permissioned checker node:
1. Observes `NewRequest(requestId, ...)` on-chain.
2. Evaluates the transaction payload (e.g. detects address poisoning).
3. Calls `commitYes(requestId)` with `yesBondPerChecker` ETH/tokens attached, or `commitNo(requestId)` with up to `noBondCeiling` attached.

### Finalization

After the voting window closes (or once `yesBondTarget` is fully met):
- **Unanimous Yes**: `OracleResult` emitted with `approved=true`. Fee distributed proportionally (capital-weighted speed score). Bonds returned.
- **Unanimous No**: `OracleResult` emitted with `approved=false`. Fee refunded to user. Bonds returned. Checkers earn no fee reward on No outcomes to prevent collusion (see §Open Questions #10).
- **Conflict**: State frozen. Foundation triggers arbitration (Phase 2).
- **Timeout / undercapitalized**: `OracleResult` emitted with `approved=false`. Fee refunded to user. Bonds returned.

---

## Tech Specs

### Contract: `CheckerOracle.sol`

#### Storage

```solidity
struct Request {
    address proposer;          // Consensus contract address
    uint256 fee;               // locked user fee
    uint256 yesBondTarget;     // aggregate Yes bond required
    uint256 deadline;          // block.timestamp + VOTING_WINDOW
    State   state;             // PENDING | FROZEN | RESOLVED
    uint256 totalYesBond;
    uint256 totalNoBond;
    uint256 checkerCount;      // total eligible commitments
    uint256 totalScore;        // cached at finalize() to avoid recomputation per claim
    bool    arbitrated;
}

struct Commitment {
    bool    isYes;
    uint256 bondAmount;        // effective (capped) bond
    uint256 position;          // arrival order (1-indexed)
    bool    claimed;
}

mapping(bytes32 requestId => Request) requests;
mapping(bytes32 requestId => mapping(address checker => Commitment)) commitments;
mapping(bytes32 requestId => address[]) checkerOrder; // ordered arrival list
```

#### Constants / Immutables

| Name | Description |
|---|---|
| `VOTING_WINDOW` | Duration in seconds for the voting window (e.g. 5 minutes) |
| `NO_BOND_CEILING` | Maximum "No" bond (e.g. 50 USDC equivalent) |
| `REGISTRY` | Address of the permissioned checker registry |
| `FEE_TOKEN` | ERC-20 token (or `address(0)` for native ETH) for bonds and fees |
| `FOUNDATION_TREASURY` | Recipient of slashed remainder |

#### Events

```solidity
event OracleResult(bytes32 indexed requestId, address indexed proposer, bytes result, bool approved); // IOracle compliance
event NewRequest(bytes32 indexed requestId, address indexed proposer, uint256 fee, uint256 yesBondTarget, uint256 deadline);
event Committed(bytes32 indexed requestId, address indexed checker, bool isYes, uint256 bondAmount, uint256 position);
event Resolved(bytes32 indexed requestId, bool approved, ResolveReason reason);
event ArbitrationTriggered(bytes32 indexed requestId);
event DisputeResolved(bytes32 indexed requestId, address winner, address loser, uint256 slashed);
event Claimed(bytes32 indexed requestId, address indexed checker, uint256 bondReturn, uint256 feeReward);
```

```solidity
enum ResolveReason { UNANIMOUS_YES, UNANIMOUS_NO, TIMEOUT, ARBITRATION }
```

#### Key Functions

| Function | Access | Description |
|---|---|---|
| `postRequest(requestId)` | `Consensus` | Opens request, locks fee, emits `NewRequest` |
| `commitYes(requestId)` | Permissioned checker | Posts Yes bond |
| `commitNo(requestId)` | Permissioned checker | Posts No bond (capped at `NO_BOND_CEILING`) |
| `finalize(requestId)` | Anyone | Resolves request after deadline or on full Yes threshold |
| `claim(requestId)` | Checker | Returns bond + proportional fee reward |
| `triggerArbitration(requestId)` | Foundation | Freezes conflicted request |
| `resolveDispute(requestId, winner, loser)` | Foundation | Slashes loser, waterfall distribution |

#### Fee Distribution Math

Only commitments recorded before `yesBondTarget` is fully met are eligible. If a commitment overshoots the remaining gap, only the gap-filling portion is counted; the excess is immediately returnable.

```
Score_i    = effectiveBond_i × positionMultiplier_i
             where positionMultiplier_i = (checkerCount + 1 - position_i)
             and   checkerCount = total eligible commitments recorded before yesBondTarget was met

TotalScore = Σ Score_i  (cached in Request.totalScore during finalize())

Payout_i   = totalFee × (Score_i / TotalScore)
```

### Checker Registry

A lightweight `CheckerRegistry.sol` (or extension to `Staking.sol`) that tracks the permissioned checker set and their master deposits. The foundation can `ban(checker)` and slash the master deposit.

> **Open question**: Should checkers be registered in a separate `CheckerRegistry` or reuse/extend the existing `Staking.sol`? (see §Open Questions)

### Validator Service Changes (Phase 3)

A new `checker` sub-service in `validator/src/`:
- Subscribes to `NewRequest` events from `CheckerOracle`.
- Runs address-poisoning detection logic against the transaction payload.
- Signs and submits `commitYes` / `commitNo` via an injected wallet.
- Monitors `Resolved` and `ArbitrationTriggered` events to trigger `claim`.

### Test Cases

**Unit tests (Solidity / Forge):**
- `testUnanymousYes_fullBond` — all checkers vote Yes, full threshold met; fee distributed correctly.
- `testUnanimousYes_timeout` — window expires with partial Yes bond above threshold; resolves Yes.
- `testUnanimousNo` — all checkers vote No; fee refunded, bonds returned, `approved=false`.
- `testTimeout_undercapitalized` — window expires, Yes threshold not met, no No votes; defaults to reject.
- `testConflict_arbitration` — mixed votes; `triggerArbitration` freezes; `resolveDispute` slashes loser.
- `testFeeDistribution_positionMultiplier` — three checkers with different bonds and positions; verify payout math.
- `testExcessBond_capped` — commitment that would overshoot threshold; verify only gap-filling portion counted.
- `testNoBond_ceilingEnforced` — `commitNo` with amount > `NO_BOND_CEILING` reverts or is capped.
- `testClaimIdempotency` — double-claim reverts.
- `testUnpermissionedChecker_reverts` — non-registry address cannot commit.

**Integration tests:**
- End-to-end with Anvil: deploy `CheckerOracle`, run two checker nodes, verify unanimous resolution flow.

---

## Implementation Phases

### Phase 1 — Core Contract (PR 1)

**Goal:** Request lifecycle through unanimous resolution and timeout.

Files touched:
- `contracts/src/CheckerOracle.sol` — new contract
- `contracts/src/CheckerRegistry.sol` — new minimal registry (or extend `Staking.sol`)
- `contracts/src/interfaces/ICheckerOracle.sol` — new interface
- `contracts/test/CheckerOracle.t.sol` — unit tests for Phase 1 flows
- `contracts/script/DeployCheckerOracle.s.sol` — deployment script

Flows covered: `postRequest`, `commitYes`, `commitNo`, `finalize`, `claim`, timeout default.

### Phase 2 — Arbitration (PR 2, depends on Phase 1)

**Goal:** Conflict detection, arbitration trigger, dispute resolution, slashing waterfall.

Files touched:
- `contracts/src/CheckerOracle.sol` — `triggerArbitration`, `resolveDispute`, slashing logic
- `contracts/test/CheckerOracle.t.sol` — arbitration unit tests

Flows covered: conflict freeze, foundation arbitration, loser slash, user fee refund, treasury transfer.

### Phase 3 — Checker Node Service (PR 3, can be parallelized with Phase 2)

**Goal:** Off-chain checker daemon integrated into the validator service.

Files touched:
- `validator/src/checker/` — new directory: `checker.ts`, `detector.ts`, `wallet.ts`
- `validator/src/index.ts` — wire up checker sub-service
- `validator/test/checker/` — unit tests for detector logic

Flows covered: event subscription, address-poisoning detection, bond submission, claim trigger.

---

## Open Questions / Assumptions

1. **Fee token**: Should the fee and bonds be denominated in native ETH or an ERC-20 (e.g. USDC)? ETH is simpler operationally; a stablecoin makes the $50 `NO_BOND_CEILING` economically stable. Decision needed before Phase 1 implementation.

2. **Fee escrow mechanism**: How does the user's fee reach the contract? Options: (a) user pre-approves an ERC-20 transfer called by `Consensus` when posting the request; (b) user maintains a deposit balance in `CheckerOracle`. Option (a) is simpler but requires Consensus changes; option (b) adds UX complexity.

3. **`yesBondTarget` parameterization**: Who sets the Yes bond target per request — the user, a governance parameter, or a formula (e.g. `2x fee`)? A fixed formula reduces griefing surface but may not suit all transaction sizes.

4. **Registry vs. Staking extension**: Should the permissioned checker set and master deposits live in a new `CheckerRegistry.sol` or extend the existing `Staking.sol`? Reusing `Staking.sol` reduces contract count but may conflate validator and checker economics.

5. **`VOTING_WINDOW` value**: What is the acceptable latency for a user waiting for a transaction check? 5 minutes is used as a placeholder. This has direct UX impact and should be confirmed with product.

6. **Slashing deficit**: If the loser's bond (`NO_BOND_CEILING` = $50) is less than the user's fee, the user cannot be fully refunded from the slashed bond alone. The spec assumes the treasury absorbs the deficit in V1. Is this acceptable, or should the foundation top up the refund?

7. **Partial No threshold**: Currently, a single valid "No" commitment triggers a conflict. Should there be a minimum "No" bond aggregate before conflict is declared, to reduce cheap-griefing surface even within V1?

8. **Checker banning on-chain vs. off-chain**: The grief-and-sweep mitigation relies on the foundation manually banning checkers. Should `CheckerRegistry` support an on-chain `ban(address)` callable by the foundation multisig, or is off-chain tracking sufficient for V1?

9. **Interaction with existing `AlwaysApproveOracle`**: Is there a migration or dual-oracle path needed, or will `CheckerOracle` be a clean replacement for `SimpleOracle` in new deployments only?

10. **No-vote incentive**: With Unanimous No now refunding the fee to the user (checkers earn no reward), what motivates permissioned checkers to vote "No"? In V1 this relies on honest checker operators fulfilling their role by protocol agreement, not financial reward. Is this acceptable, or should a separate "alarm reward" mechanism (funded by the foundation) be added to compensate checkers who correctly flag poisoned transactions?

**Assumptions:**
- The permissioned checker set is small (≤ 20 nodes) and operated by vetted foundation partners in V1.
- Conflicts are rare; arbitration is expected to be an exceptional path.
- The `Consensus` contract interface (`postRequest`) remains unchanged.
- The chain has low gas fees and no deep reorgs (consistent with existing Safenet deployment assumptions).
