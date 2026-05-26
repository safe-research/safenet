# Feature Proposal: Competitive Transaction Checker Oracle V1
Component: `contracts`, `validator`

---

## Overview

This feature introduces a competitive transaction checker oracle as a new `IOracle` implementation. Rather than a single designated approver (as in `SimpleOracle`), a permissioned set of *checker nodes* races to post bonded votes ("Approve" = safe, "Deny" = poisoned) against a transaction within a time-boxed window. The contract escrows the user's fee, collects bonds, and resolves the outcome once the window closes or bond thresholds are met. Checker nodes running alongside the validator service monitor the `NewRequest` event and submit their assessments on-chain.

**Phases (separate PRs):**
1. **Phase 1 — Core contract** (`SentinelOracle.sol`): request lifecycle, bonding, unanimous resolution, fee distribution, and timeout-default-reject logic.
2. **Phase 2 — Arbitration** (`SentinelOracle.sol` extension): conflict detection, `triggerArbitration`, `resolveDispute`, slashing, and user fee refund.
3. **Phase 2.5 — Fee payment fix** (`SentinelOracle.sol` + `Consensus.sol`): correct the fee pull flow so `Consensus` collects the fee from the user and approves the oracle, and add proposer tracking with a user-facing refund claim method on `Consensus`.
4. **Phase 3 — Sentinel node service** (`validator/`): off-chain sentinel daemon that listens for `NewRequest` events and posts bonds with Approve/Deny votes.
5. **Phase 4 — Deployment script**: Forge deployment script for `SentinelOracle`.

---

## Architecture Decision

### New Contract: `CheckerOracle`

`CheckerOracle` implements `IOracle` and replaces `SimpleOracle` for deployments that require competitive bonding. The `Consensus` contract calls `postRequest` on the oracle, passing the original transaction proposer and the reward offer (token and amount) so the oracle can pull funds and issue refunds directly without `Consensus` acting as a token intermediary.

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

The user submits a Safe transaction proposal through `Consensus.proposeOracleTransaction(rewardToken, rewardAmount, ...)`. Before calling, the user must approve `SentinelOracle` to pull `rewardAmount` of `rewardToken` (obtained by calling `oracle.getFee()` off-chain or on-chain). `Consensus` calls `oracle.postRequest(requestId, msg.sender, rewardToken, rewardAmount)`, passing the caller as the proposer. The oracle validates that `rewardToken` and `rewardAmount` match its accepted values, pulls the fee directly from the proposer via `transferFrom`, locks it in escrow, and emits `NewRequest`. Fee refunds (on timeout or arbitration) are sent by the oracle directly back to the proposer — `Consensus` is not involved in token flows at all.

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
- Monitors `OracleResult` events (emitted after unanimous resolution or after `resolveDispute`) to trigger `claim`. `ArbitrationTriggered` does **not** signal that a dispute is resolved; `claim` must not be triggered until `resolveDispute` has been called and the request is in a resolved state.

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

### Phase 2.5 — Fee Payment Fix (PR 2.5, depends on Phase 1)

**Goal:** Correct the fee payment flow so the oracle pulls the fee directly from the original transaction proposer, and handles all refund logic back to that proposer without `Consensus` acting as a token intermediary.

**Context:** In Phase 1, `SentinelOracle.postRequest` calls `FEE_TOKEN.transferFrom(msg.sender, address(this), fee)` where `msg.sender` is the `Consensus` contract. `Consensus` holds no ERC-20 balance and sets no allowance, so this call always reverts. This phase fixes that by extending `IOracle.postRequest` to accept the proposer address and the reward offer, so the oracle can pull funds from and refund to the end user directly. This is an initial implementation — further refinements to fee mechanics are deferred to later phases.

**Design decisions:**

*`IOracle` interface:*
- `postRequest` signature is extended to `postRequest(bytes32 requestId, address proposer, address rewardToken, uint256 rewardAmount)`. `proposer` is the address that offered the reward and to whom any refund is owed. `rewardToken` and `rewardAmount` describe the offered fee.
- A new view function `getFee() returns (address token, uint256 amount)` is added to allow off-chain callers (frontends, scripts) to discover the currently accepted token and required amount before constructing a call. All oracle implementations must implement both additions.

*`SentinelOracle`:*
- `postRequest` stores `proposer` from the parameter (not `msg.sender`) in `Request.proposer`. This replaces the previous use of `msg.sender` (the `Consensus` address) as the proposer.
- On receiving a `postRequest` call, the oracle validates that `rewardToken == FEE_TOKEN` and `rewardAmount == fee` (exact match), reverting otherwise. It then pulls the fee directly from `proposer` via `IERC20(FEE_TOKEN).transferFrom(proposer, address(this), fee)`.
- On resolution, fee refunds (timeout, arbitration win) are transferred directly to `Request.proposer`. No `pendingRefund` field or `claimRefund` function is needed — the oracle pushes the refund in the resolution transaction.

*`Consensus`:*
- `proposeOracleTransaction` is updated to accept `rewardToken` and `rewardAmount` as parameters from the caller and pass them through to the oracle along with `msg.sender` as the proposer: `oracle.postRequest(requestId, msg.sender, rewardToken, rewardAmount)`.
- No token handling in `Consensus` — no `transferFrom`, no `approve`, no balance tracking.
- No `requestProposers` mapping and no `claimOracleRefund` function — refund routing is entirely the oracle's responsibility.

**Updated user flow:**
1. User calls `oracle.getFee()` (or reads it off-chain) to learn the required fee token and amount.
2. User approves `SentinelOracle` directly for that fee amount in the returned token.
3. User calls `Consensus.proposeOracleTransaction(rewardToken, rewardAmount, ...)`.
4. Consensus calls `oracle.postRequest(requestId, msg.sender, rewardToken, rewardAmount)`. The oracle validates the reward, pulls the fee from the user, and emits `NewRequest`.
5. If the request results in a fee refund (timeout or arbitration win): the oracle transfers the refund directly to the proposer (the user) as part of the resolution transaction — no further user action required.

Files touched:
- `contracts/src/interfaces/IOracle.sol` — extend `postRequest` to `postRequest(bytes32, address proposer, address rewardToken, uint256 rewardAmount)`; add `getFee() external view returns (address token, uint256 amount)`
- `contracts/src/SentinelOracle.sol` — update `postRequest` to accept and store proposer, validate reward, pull from proposer; push refunds to proposer on resolution; implement `getFee()`
- `contracts/src/SimpleOracle.sol` — update `postRequest` signature; add stub `getFee()` returning `(address(0), 0)`
- `contracts/src/AlwaysApproveOracle.sol` — update `postRequest` signature; add stub `getFee()` returning `(address(0), 0)`
- `contracts/src/Consensus.sol` — update `proposeOracleTransaction` to accept `rewardToken` and `rewardAmount` and pass them with `msg.sender` to `postRequest`
- `contracts/test/SentinelOracle.t.sol` — update all `postRequest` call sites; test reward validation, direct proposer pull, and direct refund push
- `contracts/test/Consensus.t.sol` — end-to-end test: user approves oracle, calls Consensus, fee lands in oracle, refund returns to user

Test cases:
- `getFee()` returns the expected fee token address and amount.
- Oracle pulls the fee directly from the proposer (not from Consensus).
- `postRequest` reverts if `rewardToken` does not match `FEE_TOKEN`.
- `postRequest` reverts if `rewardAmount` does not match the expected fee.
- Timeout refund is pushed directly to the proposer by the oracle.
- Arbitration refund is pushed directly to the proposer by the oracle.
- `Consensus` holds no token balance before or after a successful `proposeOracleTransaction` call.

### Phase 3 — Sentinel Node Service (PR 3, can be parallelized with Phase 2)

**Goal:** Off-chain sentinel daemon integrated into the validator service.

Files touched:
- `validator/src/sentinel/` — new directory: `sentinel.ts`, `detector.ts`, `wallet.ts`
- `validator/src/index.ts` — wire up sentinel sub-service
- `validator/test/sentinel/` — unit tests for detector logic

Flows covered: event subscription, address-poisoning detection, bond submission, claim trigger (only after dispute resolution, not on `ArbitrationTriggered`).

### Phase 4 — Final cleanup (PR 4, depends on previous phases)

**Goal:** Final adjustments and wiring — Forge deployment script for `SentinelOracle`.

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
- `IOracle.postRequest` gains `proposer`, `rewardToken`, and `rewardAmount` parameters in Phase 2.5; `getFee()` is added as a view for off-chain discovery. The full fee mechanism (dynamic pricing, variable tokens) is deferred to a later phase.
- The chain has low gas fees and no deep reorgs (consistent with existing Safenet deployment assumptions).

---

## Additional Notes

### Alternatives Considered for Fee Amount Resolution (Phase 2.5)

The core challenge is that `Consensus` must know the fee amount before it can pull tokens from the user, but the oracle owns the fee definition. Four approaches were evaluated.

**Option 1 — Oracle exposes `getFee()` view; Consensus queries first**

`Consensus` calls `oracle.getFee()` before pulling from the user, then approves exactly that amount. The oracle retains full ownership of its pricing. The only IOracle change is a single view function. The minor TOCTOU risk (fee changing between query and `postRequest`) is negligible given governance delays on parameter changes. `getFee()` is retained in the chosen design as a convenience view for off-chain callers, but is not called on-chain by `Consensus`.

**Option 2 — Fee hardcoded or configured in Consensus**

`Consensus` stores a `feeAmount` and pulls that from the user. Simple to implement but duplicates fee state across two contracts: any oracle fee change requires a coordinated governance update to both. Not viable beyond a quick prototype.

**Option 3 — User passes a max fee; oracle charges actual amount, excess refunded**

`proposeOracleTransaction(uint256 maxFee, ...)` — Consensus pulls `maxFee` from user and approves the oracle for it. The oracle charges the actual fee and stores the excess as a per-request refundable balance. Gives the user an explicit on-chain ceiling but adds oracle state for excess tracking, almost always produces a two-step UX (propose + claim-excess), and doesn't eliminate the need for the user to know the oracle's pricing anyway (deferred to off-chain).

**Option 3a — Proposed fee as a caller-supplied reward; oracle validates and returns amount taken** *(chosen)*

A variant of Option 3 where the fee is reframed as a *reward* offered by the caller rather than a ceiling on what the oracle charges. `postRequest(requestId, address proposer, address rewardToken, uint256 rewardAmount)` — `Consensus` passes the proposer (the end user, `msg.sender`) along with the token and amount they are willing to pay. The oracle checks that `rewardToken` and `rewardAmount` exactly match its accepted values and reverts if they differ, so there is no ambiguity about what was agreed. The oracle pulls the fee directly from `proposer` and handles all refund logic back to `proposer`, keeping `Consensus` entirely out of the token flow. The trade-off versus Option 1 is that `IOracle.postRequest` gains three parameters and all call sites must be updated; the benefit is that `Consensus` never touches ERC-20 tokens, the reward commitment is explicit and auditable in the call, and refund routing requires no additional Consensus state.

**Option 4 — Consensus passes `msg.sender` as `feePayer`; oracle pulls directly from user**

`postRequest(requestId, feePayer)` — oracle calls `transferFrom(feePayer, ...)` directly; user pre-approves the oracle, not Consensus. Cleanest token flow (Consensus never holds ERC-20) and refunds go directly to `feePayer` without Consensus involvement. However, it changes `IOracle.postRequest`, requires the user to approve a contract they don't directly interact with (the oracle address depends on Consensus configuration), and makes `feePayer` a load-bearing trust parameter — the oracle must trust Consensus to pass the real caller, turning the existing `CONSENSUS` access gate into a security invariant rather than just access control.
