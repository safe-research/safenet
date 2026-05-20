# Feature Proposal: Competitive Transaction Checker Oracle V1
Component: `contracts`, `validator`

---

## Overview

This feature introduces a competitive transaction checker oracle as a new `IOracle` implementation. Rather than a single designated approver (as in `SimpleOracle`), a permissioned set of *checker nodes* races to post bonded votes ("Approve" = safe, "Deny" = poisoned) against a transaction within a time-boxed window. The contract escrows the user's fee, collects bonds, and resolves the outcome once the window closes or bond thresholds are met. Checker nodes running alongside the validator service monitor the `NewRequest` event and submit their assessments on-chain.

**Phases (separate PRs):**
1. **Phase 1 — Core contract** (`SentinelOracle.sol`): request lifecycle, bonding, unanimous resolution, fee distribution, and timeout-default-reject logic.
2. **Phase 2 — Arbitration** (`SentinelOracle.sol` extension): conflict detection, `triggerArbitration`, `resolveDispute`, slashing, and user fee refund.
3. **Phase 3 — Sentinel node service** (`validator/`): off-chain sentinel daemon that listens for `NewRequest` events and posts bonds with Approve/Deny votes.
4. **Phase 4 — Deployment script**: Forge deployment script for `SentinelOracle`.

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

#### Symmetric Bond Thresholds

Both Approve and Deny sides use identical bond mechanics:

- **Bond target**: `fee × bondMultiplier` aggregate, applied equally to both sides.
- A bond threshold is **reached** when the aggregate for that side equals `fee × bondMultiplier`.
- Over-contributions beyond the threshold are excess and returned to the contributor immediately.

Per-checker Deny caps and asymmetric ceilings are deferred to a later implementation.

#### Explicit Finalization ("Pull over Push")

Bond thresholds being met does **not** automatically distribute funds. An explicit `finalize(requestId)` (callable by any address) or `claim(requestId)` (per checker) must be invoked after the voting window closes. This bounds gas for voting transactions, prevents reentrancy, and allows checkers to batch claims.

#### Timeout Default: Reject

If the voting window expires and neither the Approve nor the Deny aggregate threshold has been fully met, the request resolves as **rejected** (defensive fail-safe). The user's fee is refunded. This sacrifices liveness in exchange for a strict defensive posture.

#### Grief-and-Sweep Attack

With symmetric bonds the attack is no longer cheap: reaching the Deny threshold requires the same total capital (`fee × bondMultiplier`) as the Approve side. A losing Deny side has that bond slashed, and sweeping the fee on the Approve retry only recovers at most `fee` — a net loss of `fee × (bondMultiplier - 1)`. The foundation monitoring and on-chain `removeChecker` remain as an additional deterrent.

#### Arbitration (Phase 2)

Conflict (at least one "Approve" and one "Deny" commitment) freezes the request. The foundation calls `triggerArbitration(requestId)` and, after off-chain review, `resolveDispute(requestId, winner, loser)`. The loser's bond is slashed in a two-step waterfall:
1. Full user fee refund from slashed bond.
2. Remainder to `ARBITRATOR`.

Because both sides post the same total bond (`fee × bondMultiplier`), the slashed bond always equals the full fee, ensuring the user refund is covered without a deficit.

### Alternatives Considered

- **Asymmetric bonds**: Deferred due to complexity and because the initial version assumes low bond values.
- **Automatic push on threshold**: Rejected — reentrancy risk and unbounded gas cost for voting transactions.
- **On-chain conflict resolution without human arbitration**: Deferred to a future version. V1 assumes a permissioned checker set where conflict is rare and human oversight is practical.
- **Native ETH as fee token**: Rejected — implementation is ERC-20 only. Native ETH can be supported by wrapping it (e.g. WETH) before interacting with the contract.

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

After the voting window closes:
- **Unanimous Approve** (Approve threshold reached, Deny threshold not reached): `OracleResult` emitted with `approved=true`. Fee distributed proportionally to Approve checkers (capital-weighted speed score). Bonds returned. Any sub-threshold Deny contributions are returned without effect.
- **Unanimous Deny** (Deny threshold reached, Approve threshold not reached): `OracleResult` emitted with `approved=false`. Fee distributed proportionally to Deny checkers (same capital-weighted speed score formula). Bonds returned. Any sub-threshold Approve contributions are returned without effect.
- **Conflict** (both Approve and Deny thresholds fully reached): State frozen. `ARBITRATOR` triggers arbitration (Phase 2). User fee refunded from the losing side's slashed bonds.
- **Timeout / undercapitalized** (neither threshold reached by deadline): `OracleResult` emitted with `approved=false`. Fee refunded to user. Bonds returned.

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
    uint256 totalApproveBond;     // running sum of Approve bonds; threshold reached when == approveBondTarget
    uint256 totalDenyBond;        // running sum of Deny bonds; threshold reached when == approveBondTarget; conflict when both sides reach threshold
    uint256 checkerCount;         // number of winning-side voters eligible for fee distribution (committed before bondTarget was met)
    uint256 totalScore;           // running sum of checker scores, updated on each commit; avoids recomputation at claim time
    bool    arbitrated;
}

struct Commitment {
    bool    approved;          // true = Approve vote, false = Deny vote
    uint256 bondAmount;        // bond amount committed
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
| `commitDeny(requestId)` | Active checker | Posts Deny bond; aggregate target = `fee × bondMultiplier` (same as Approve) |
| `finalize(requestId)` | Anyone | Resolves request after deadline or on full Approve threshold |
| `claim(requestId)` | Checker | Returns bond + proportional fee reward |
| `triggerArbitration(requestId)` | `ARBITRATOR` | Freezes conflicted request |
| `resolveDispute(requestId, winner, loser)` | `ARBITRATOR` | Slashes loser, waterfall distribution |
| `addChecker(checker)` | `ARBITRATOR` | Schedules a checker to become active after `GOVERNANCE_DELAY` blocks |
| `removeChecker(checker)` | `ARBITRATOR` | Immediately removes a checker from the active set |
| `scheduleBondMultiplier(newValue)` | `ARBITRATOR` | Stages a new bond multiplier, active after `GOVERNANCE_DELAY` blocks |
| `applyBondMultiplier()` | Anyone | Commits the staged multiplier once its activation block is reached |

#### Fee Distribution Math

Only commitments recorded before the bond target is fully met are eligible. If a commitment overshoots the remaining gap, only the gap-filling portion counts toward the score; the excess is returned immediately.

Each checker's score rewards both early arrival and capital risked:

```
Score_i    = bond_i / position_i
             (earlier arrivals receive a larger score for the same bond amount)

TotalScore = Σ Score_i
             (updated incrementally on every eligible commit; stored in Request.totalScore)

Payout_i   = fee × (Score_i / TotalScore)
```

The same formula is applied to whichever side wins (Approve or Deny).

> **Solidity decimal note**: integer division truncates, so `bond / position` loses precision for small bonds or large position numbers. The implementation must scale the numerator before dividing (e.g. compute `Score_i = bond_i * PRECISION / position_i` with a constant such as `PRECISION = 1e18`) and account for the same scale factor when computing payouts.

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

Focus on behavioral end-to-end scenarios that exercise complete flows rather than internal logic. Fewer well-chosen tests are preferable to exhaustive coverage of implementation details.

**Key scenarios (Solidity / Forge + Anvil):**
- **Unanimous Approve flow** — checkers collectively reach the Approve threshold; fee is distributed proportionally and bonds are returned; `OracleResult` is emitted with `approved=true`.
- **Unanimous Deny flow** — checkers collectively reach the Deny threshold; fee is distributed to Deny checkers; `OracleResult` is emitted with `approved=false`.
- **Timeout / undercapitalized** — voting window expires with neither threshold met; fee is refunded to user; request resolves rejected.
- **Conflict and arbitration** — both thresholds are reached; request is frozen; `ARBITRATOR` resolves the dispute; loser is slashed and user is refunded.
- **Checker management** — `ARBITRATOR` adds a checker (with delay) and removes one; only active checkers can commit bonds.
- **Bond multiplier update** — `ARBITRATOR` schedules a new `bondMultiplier`; it is not applied until after `GOVERNANCE_DELAY` blocks.

---

## Implementation Phases

### Phase 1 — Core Contract (PR 1)

**Goal:** Request lifecycle through unanimous resolution and timeout.

Files touched:
- `contracts/src/SentinelOracle.sol` — new contract (includes sentinel set management)
- `contracts/src/interfaces/ISentinelOracle.sol` — new interface
- `contracts/test/SentinelOracle.t.sol` — unit tests for Phase 1 flows

Flows covered: `postRequest`, `commitApprove`, `commitDeny`, `finalize`, `claim`, `addSentinel`, `removeSentinel`, timeout default.

### Phase 2 — Arbitration (PR 2, depends on Phase 1)

**Goal:** Conflict detection, arbitration trigger, dispute resolution, slashing waterfall.

Files touched:
- `contracts/src/SentinelOracle.sol` — `triggerArbitration`, `resolveDispute`, slashing logic
- `contracts/test/SentinelOracle.t.sol` — arbitration unit tests

Flows covered: conflict freeze, foundation arbitration, loser slash, user fee refund, treasury transfer.

### Phase 3 — Sentinel Node Service (PR 3, can be parallelized with Phase 2)

**Goal:** Off-chain sentinel daemon integrated into the validator service.

Files touched:
- `validator/src/sentinel/` — new directory: `sentinel.ts`, `detector.ts`, `wallet.ts`
- `validator/src/index.ts` — wire up sentinel sub-service
- `validator/test/sentinel/` — unit tests for detector logic

Flows covered: event subscription, address-poisoning detection, bond submission, claim trigger.

### Phase 4 — Final cleanup (PR 4, depends on previous phases)

**Goal:** Final adjustments and wiring
- Forge deployment script for `SentinelOracle`.
- Change refund fee to pull instead of push model (currently gets pushed to Consensus contract).

Files touched:
- `contracts/script/DeploySentinelOracle.s.sol` — deployment script reading constructor params from env vars

---

## Open Questions / Assumptions

1. ~~**Fee token**~~ **Decided**: ERC-20 token. Both user fees and checker bonds are denominated in an ERC-20 token, using the standard approve + `transferFrom` pull pattern.

2. ~~**Fee escrow mechanism**~~ **Decided**: Option (a) — user pre-approves `CheckerOracle` for the fee amount; `Consensus` calls `postRequest` which pulls the fee via `transferFrom` at request time. Same pull pattern applies to checker bonds on `commitApprove` / `commitDeny`.

3. ~~**`approveBondTarget` parameterization**~~ **Decided**: `approveBondTarget = fee × bondMultiplier`. `bondMultiplier` defaults to `50` and is updateable by `ARBITRATOR` with a `GOVERNANCE_DELAY` block time delay via `scheduleBondMultiplier` / `applyBondMultiplier`.

4. ~~**Registry vs. Staking extension**~~ **Decided**: Checker set is managed directly inside `CheckerOracle` with no separate registry contract and no on-chain master deposit. `ARBITRATOR` manages additions (time-delayed by `GOVERNANCE_DELAY`) and removals (immediate).

5. ~~**`VOTING_WINDOW` value**~~ **Decided**: 12 blocks (≈ 1 minute on Gnosis Chain).

6. ~~**Slashing deficit**~~ **Resolved**: With symmetric bonds both sides post `fee × bondMultiplier` in aggregate. The slashed bond always covers the full user fee refund with no deficit.

7. ~~**Partial Deny threshold**~~ **Decided**: Conflict requires the full Deny bond target (`fee × bondMultiplier`) to be reached, identical to the Approve side. Sub-threshold Deny votes have no effect and bonds are returned. Over-achievement on either side is not rewarded (excess returned).

8. ~~**Checker banning on-chain vs. off-chain**~~ **Decided**: `ARBITRATOR` can call `removeChecker(address)` immediately on-chain. The grief-and-sweep mitigation relies on the arbitrator monitoring for the pattern and invoking this function.

9. ~~**Interaction with existing oracles**~~ **Resolved**: Oracle selection is managed in the validator code. Multiple oracles can be active in parallel as long as validators mark them as valid. No migration from `SimpleOracle` / `AlwaysApproveOracle` is required; `CheckerOracle` is an additive deployment.

10. ~~**Deny-vote incentive**~~ **Resolved**: On Unanimous Deny the fee is distributed proportionally to Deny checkers (same formula as Approve). This provides a financial incentive to correctly flag poisoned transactions. The earlier collusion concern (cheap Deny bonds + fee reward) no longer applies because the Deny aggregate threshold is now `fee × bondMultiplier` — a colluding group must post the same total capital as the Approve side, making griefing economically unattractive.

**Assumptions:**
- The permissioned checker set is small (≤ 20 nodes) and operated by vetted foundation partners in V1.
- Conflicts are rare; arbitration is expected to be an exceptional path.
- The `Consensus` contract interface (`postRequest`) remains unchanged.
- The chain has low gas fees and no deep reorgs (consistent with existing Safenet deployment assumptions).
