# Feature Proposal: Competitive Transaction Checker Oracle V1
Component: `contracts`, `validator`

---

## Overview

This feature introduces a competitive transaction checker oracle as a new `IOracle` implementation. Rather than a single designated approver (as in `SimpleOracle`), a permissioned set of *checker nodes* races to post bonded votes ("Approve" = safe, "Deny" = poisoned) against a transaction within a time-boxed window. The contract escrows the user's fee, collects bonds, and resolves the outcome once the window closes or bond thresholds are met. Checker nodes running alongside the validator service monitor the `NewRequest` event and submit their assessments on-chain.

**Phases (separate PRs):**
1. **Phase 1 — Core contract** (`CheckerOracle.sol`): request lifecycle, bonding, unanimous resolution, fee distribution, and timeout-default-reject logic.
2. **Phase 2 — Arbitration** (`CheckerOracle.sol` extension): conflict detection, `triggerArbitration`, `resolveDispute`, slashing, and user fee refund.
3. **Phase 3 — Checker node service** (`validator/`): off-chain checker daemon that listens for `NewRequest` events and posts bonds with Approve/Deny votes.

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
           (commitApprove / commitDeny)  (triggerArbitration /
                                           resolveDispute)
```

### Key Design Decisions

#### Asymmetric Bond Thresholds

- `approveBondTarget` — computed at request time as `fee × bondMultiplier`. `bondMultiplier` is a governance parameter (e.g. `50`) updateable by the `ARBITRATOR` with a time delay. Represents the total aggregate "Approve" bond required.
- `denyBond` — computed per request using the same multiplier, capped so that raising the fee never prices out a whistleblower:
  ```
  denyBond = min(fee × bondMultiplier, max(DENY_BOND_CEILING, fee))
  ```
  When `fee ≤ DENY_BOND_CEILING`, the cap is `DENY_BOND_CEILING` (cheap alarm preserved). When `fee > DENY_BOND_CEILING`, the cap is `fee` itself (ensures the slashed bond always covers a full fee refund to the user).

This asymmetry ensures that a whistleblower can never be priced out of flagging a poisoned transaction, at the cost of cheap griefing potential (mitigated off-chain in V1 — see §Grief-and-Sweep below).

#### Explicit Finalization ("Pull over Push")

Bond thresholds being met does **not** automatically distribute funds. An explicit `finalize(requestId)` (callable by any address) or `claim(requestId)` (per checker) must be invoked after the voting window closes. This bounds gas for voting transactions, prevents reentrancy, and allows checkers to batch claims.

#### Timeout Default: Reject

If the voting window expires and neither the "Approve" aggregate threshold nor at least one valid "Deny" bond has been posted, the request resolves as **rejected** (defensive fail-safe). The user's fee is refunded. This sacrifices liveness in exchange for a strict defensive posture.

#### Grief-and-Sweep Attack

The contract is intentionally left mathematically vulnerable: a malicious node can post the cheap "Deny" bond to stall a transaction, lose arbitration, and immediately vote "Approve" on the retry to sweep the fee. In V1 this is mitigated entirely off-chain: the foundation monitors this pattern and invokes a ban + master-deposit slash on the offending checker. A dedicated contract mechanism is out of scope for V1.

#### Arbitration (Phase 2)

Conflict (at least one "Approve" and one "Deny" commitment) freezes the request. The foundation calls `triggerArbitration(requestId)` and, after off-chain review, `resolveDispute(requestId, winner, loser)`. The loser's bond is slashed in a two-step waterfall:
1. Full user fee refund from slashed bond.
2. Remainder to `ARBITRATOR`.

Because the "Deny" bond ceiling is set to cover arbitration costs, this keeps manual review financially self-sustaining. In V1, if the slashed bond is insufficient to cover the full fee refund, the remainder is absorbed by the treasury (deficit accepted).

### Alternatives Considered

- **Symmetric bonds**: Rejected — would price out honest whistleblowers when facing high-fee transactions.
- **Automatic push on threshold**: Rejected — reentrancy risk and unbounded gas cost for voting transactions.
- **On-chain conflict resolution without human arbitration**: Deferred to a future version. V1 assumes a permissioned checker set where conflict is rare and human oversight is practical.
- **Fee token = native ETH vs. ERC-20**: Open question (see §Open Questions). The spec is written token-agnostic; the implementation will parameterize the fee token.

---

## User Flow

### Request Submission (via Consensus)

The user submits a Safe transaction proposal through `Consensus.proposeTransaction()`. Before calling, the user must approve `CheckerOracle` to pull the fee amount in the ERC-20 fee token. `Consensus` then calls `checkOracle.postRequest(requestId)`, which pulls the fee via `transferFrom`, locks it in escrow, and emits `NewRequest`.

### Checker Node Vote

Each permissioned checker node:
1. Observes `NewRequest(requestId, ...)` on-chain.
2. Evaluates the transaction payload (e.g. detects address poisoning).
3. Approves `CheckerOracle` to pull the bond amount from the checker's ERC-20 balance, then calls `commitApprove(requestId)` or `commitDeny(requestId)`. The contract pulls the bond via `transferFrom` at call time.

### Finalization

After the voting window closes (or once `approveBondTarget` is fully met):
- **Unanimous Approve**: `OracleResult` emitted with `approved=true`. Fee distributed proportionally (capital-weighted speed score). Bonds returned.
- **Unanimous Deny**: `OracleResult` emitted with `approved=false`. Fee transferred to `ARBITRATOR`. Bonds returned to checkers. The checking service was correctly performed; the user's fee is the cost of that service.
- **Conflict**: State frozen. Foundation triggers arbitration (Phase 2). User fee refunded from the losing checker's slashed bond.
- **Timeout / undercapitalized**: `OracleResult` emitted with `approved=false`. Fee refunded to user. Bonds returned.

---

## Tech Specs

### Contract: `CheckerOracle.sol`

#### Storage

```solidity
struct Request {
    address proposer;             // Consensus contract address
    uint256 fee;                  // locked user fee
    uint256 approveBondTarget;    // fee * bondMultiplier, locked at postRequest time
    uint256 deadline;             // block.number + VOTING_WINDOW
    State   state;                // PENDING | FROZEN | RESOLVED
    uint256 totalApproveBond;     // running sum of Approve bonds; compared against approveBondTarget
    uint256 totalDenyBond;        // running sum of Deny bonds; used to detect conflict (>0 with Approve votes present) and to refund bonds on Unanimous Deny
    uint256 checkerCount;         // number of Approve voters eligible for fee distribution (committed before approveBondTarget was met)
    uint256 totalScore;           // cached at finalize() to avoid recomputation per claim
    bool    arbitrated;
}

struct Commitment {
    bool    approved;          // true = Approve vote, false = Deny vote
    uint256 bondAmount;        // effective (capped) bond
    uint256 position;          // arrival order (1-indexed)
    bool    claimed;
}

// Governance parameters (time-delayed updates via GOVERNANCE_DELAY)
uint256 bondMultiplier;                // current multiplier; approveBondTarget = fee * bondMultiplier
uint256 pendingBondMultiplier;         // staged new multiplier
uint256 bondMultiplierActiveAt;        // block at which pendingBondMultiplier becomes active (0 = none pending)

mapping(address => uint256) checkerActiveAt;  // 0 = not a checker; >0 = active once block.number >= value
mapping(bytes32 requestId => Request) requests;
mapping(bytes32 requestId => mapping(address checker => Commitment)) commitments;
mapping(bytes32 requestId => address[]) checkerOrder; // ordered arrival list
```

#### Constants / Immutables

| Name | Description |
|---|---|
| `VOTING_WINDOW` | Duration in blocks for the voting window (12 blocks ≈ 1 minute on Gnosis Chain) |
| `DENY_BOND_CEILING` | Baseline cap on the Deny bond for low-fee transactions (e.g. 50 USDC equivalent); for high-fee transactions the cap rises to `fee` — see §Asymmetric Bond Thresholds |
| `GOVERNANCE_DELAY` | Time delay in blocks applied to all governance changes: adding checkers and updating `bondMultiplier` |
| `FEE_TOKEN` | ERC-20 token for bonds and fees |
| `ARBITRATOR` | Foundation address authorised to manage checkers, update `bondMultiplier`, call `triggerArbitration` / `resolveDispute`, and recipient of slashed remainder |

#### Events

```solidity
event OracleResult(bytes32 indexed requestId, address indexed proposer, bytes result, bool approved); // IOracle compliance
event NewRequest(bytes32 indexed requestId, address indexed proposer, uint256 fee, uint256 approveBondTarget, uint256 deadline);
event CheckerScheduled(address indexed checker, uint256 activeAtBlock);
event CheckerRemoved(address indexed checker);
event BondMultiplierScheduled(uint256 newMultiplier, uint256 activeAtBlock);
event BondMultiplierApplied(uint256 newMultiplier);
event Committed(bytes32 indexed requestId, address indexed checker, bool approved, uint256 bondAmount, uint256 position);
event Resolved(bytes32 indexed requestId, bool approved, ResolveReason reason);
event ArbitrationTriggered(bytes32 indexed requestId);
event DisputeResolved(bytes32 indexed requestId, address winner, address loser, uint256 slashed);
event Claimed(bytes32 indexed requestId, address indexed checker, uint256 bondReturn, uint256 feeReward);
```

```solidity
enum ResolveReason { UNANIMOUS_APPROVE, UNANIMOUS_DENY, TIMEOUT, ARBITRATION }
```

#### Key Functions

| Function | Access | Description |
|---|---|---|
| `postRequest(requestId)` | `Consensus` | Opens request, locks fee, emits `NewRequest` |
| `commitApprove(requestId)` | Active checker | Posts Approve bond |
| `commitDeny(requestId)` | Active checker | Posts Deny bond (amount = `min(fee × bondMultiplier, max(DENY_BOND_CEILING, fee))`) |
| `finalize(requestId)` | Anyone | Resolves request after deadline or on full Approve threshold |
| `claim(requestId)` | Checker | Returns bond + proportional fee reward |
| `triggerArbitration(requestId)` | `ARBITRATOR` | Freezes conflicted request |
| `resolveDispute(requestId, winner, loser)` | `ARBITRATOR` | Slashes loser, waterfall distribution |
| `addChecker(checker)` | `ARBITRATOR` | Schedules a checker to become active after `GOVERNANCE_DELAY` blocks |
| `removeChecker(checker)` | `ARBITRATOR` | Immediately removes a checker from the active set |
| `scheduleBondMultiplier(newValue)` | `ARBITRATOR` | Stages a new bond multiplier, active after `GOVERNANCE_DELAY` blocks |
| `applyBondMultiplier()` | Anyone | Commits the staged multiplier once its activation block is reached |

#### Fee Distribution Math

Only commitments recorded before `approveBondTarget` is fully met are eligible. If a commitment overshoots the remaining gap, only the gap-filling portion is counted; the excess is immediately returnable.

```
Score_i    = effectiveBond_i × positionMultiplier_i
             where positionMultiplier_i = (checkerCount + 1 - position_i)
             and   checkerCount = total eligible Approve voters recorded before approveBondTarget was met

TotalScore = Σ Score_i  (cached in Request.totalScore during finalize())

Payout_i   = totalFee × (Score_i / TotalScore)
```

### Checker Management

The permissioned checker set is tracked directly inside `CheckerOracle` — no separate registry contract is needed. The `ARBITRATOR` manages the set via `addChecker` / `removeChecker`.

- **Adding** a checker is time-delayed by `GOVERNANCE_DELAY` blocks to prevent an arbitrator from front-running pending requests with a newly enrolled checker.
- **Removing** a checker is immediate, allowing the arbitrator to react instantly to misbehaving nodes.
- There is no on-chain master deposit. The recommended minimum balance a checker should hold is off-chain guidance only.

### Validator Service Changes (Phase 3)

A new `checker` sub-service in `validator/src/`:
- Subscribes to `NewRequest` events from `CheckerOracle`.
- Runs address-poisoning detection logic against the transaction payload.
- Signs and submits `commitApprove` / `commitDeny` via an injected wallet.
- Monitors `Resolved` and `ArbitrationTriggered` events to trigger `claim`.

### Test Cases

**Unit tests (Solidity / Forge):**
- `testUnanimousApprove_fullBond` — all checkers vote Approve, full threshold met; fee distributed correctly.
- `testUnanimousApprove_timeout` — window expires with Approve bond above threshold; resolves Approve.
- `testUnanimousDeny` — all checkers vote Deny; fee refunded, bonds returned, `approved=false`.
- `testTimeout_undercapitalized` — window expires, Approve threshold not met, no Deny votes; defaults to reject.
- `testConflict_arbitration` — mixed votes; `triggerArbitration` freezes; `resolveDispute` slashes loser.
- `testFeeDistribution_positionMultiplier` — three checkers with different bonds and positions; verify payout math.
- `testExcessBond_capped` — commitment that would overshoot threshold; verify only gap-filling portion counted.
- `testDenyBond_ceilingEnforced` — `commitDeny` with amount > `DENY_BOND_CEILING` reverts or is capped.
- `testClaimIdempotency` — double-claim reverts.
- `testUnpermissionedChecker_reverts` — non-registry address cannot commit.

**Integration tests:**
- End-to-end with Anvil: deploy `CheckerOracle`, run two checker nodes, verify unanimous resolution flow.

---

## Implementation Phases

### Phase 1 — Core Contract (PR 1)

**Goal:** Request lifecycle through unanimous resolution and timeout.

Files touched:
- `contracts/src/CheckerOracle.sol` — new contract (includes checker set management)
- `contracts/src/interfaces/ICheckerOracle.sol` — new interface
- `contracts/test/CheckerOracle.t.sol` — unit tests for Phase 1 flows
- `contracts/script/DeployCheckerOracle.s.sol` — deployment script

Flows covered: `postRequest`, `commitApprove`, `commitDeny`, `finalize`, `claim`, `addChecker`, `removeChecker`, timeout default.

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

1. ~~**Fee token**~~ **Decided**: ERC-20 token. Both user fees and checker bonds are denominated in an ERC-20 token, using the standard approve + `transferFrom` pull pattern.

2. ~~**Fee escrow mechanism**~~ **Decided**: Option (a) — user pre-approves `CheckerOracle` for the fee amount; `Consensus` calls `postRequest` which pulls the fee via `transferFrom` at request time. Same pull pattern applies to checker bonds on `commitApprove` / `commitDeny`.

3. ~~**`approveBondTarget` parameterization**~~ **Decided**: `approveBondTarget = fee × bondMultiplier`. `bondMultiplier` defaults to `50` and is updateable by `ARBITRATOR` with a `GOVERNANCE_DELAY` block time delay via `scheduleBondMultiplier` / `applyBondMultiplier`.

4. ~~**Registry vs. Staking extension**~~ **Decided**: Checker set is managed directly inside `CheckerOracle` with no separate registry contract and no on-chain master deposit. `ARBITRATOR` manages additions (time-delayed by `GOVERNANCE_DELAY`) and removals (immediate).

5. ~~**`VOTING_WINDOW` value**~~ **Decided**: 12 blocks (≈ 1 minute on Gnosis Chain).

6. ~~**Slashing deficit**~~ **Resolved**: The `denyBond` formula (`min(fee × bondMultiplier, max(DENY_BOND_CEILING, fee))`) ensures that when `fee > DENY_BOND_CEILING` the Deny bond equals `fee`, so the slashed bond always fully covers the user fee refund. For `fee ≤ DENY_BOND_CEILING` the bond is at most `DENY_BOND_CEILING`, and the treasury absorbs any remaining deficit (accepted for V1).

7. **Partial Deny threshold**: Currently, a single valid "Deny" commitment triggers a conflict. Should there be a minimum "Deny" bond aggregate before conflict is declared, to reduce cheap-griefing surface even within V1?

8. ~~**Checker banning on-chain vs. off-chain**~~ **Decided**: `ARBITRATOR` can call `removeChecker(address)` immediately on-chain. The grief-and-sweep mitigation relies on the arbitrator monitoring for the pattern and invoking this function.

9. ~~**Interaction with existing oracles**~~ **Resolved**: Oracle selection is managed in the validator code. Multiple oracles can be active in parallel as long as validators mark them as valid. No migration from `SimpleOracle` / `AlwaysApproveOracle` is required; `CheckerOracle` is an additive deployment.

10. **Deny-vote incentive**: On Unanimous Deny the fee goes to `ARBITRATOR`, not to the checkers who flagged the transaction. Checkers therefore earn no direct financial reward for a correct Deny vote. In V1 this relies on honest operators fulfilling their role by protocol agreement. Is this acceptable, or should a portion of the fee be routed to Deny voters as a correct-alarm reward?

**Assumptions:**
- The permissioned checker set is small (≤ 20 nodes) and operated by vetted foundation partners in V1.
- Conflicts are rare; arbitration is expected to be an exceptional path.
- The `Consensus` contract interface (`postRequest`) remains unchanged.
- The chain has low gas fees and no deep reorgs (consistent with existing Safenet deployment assumptions).
