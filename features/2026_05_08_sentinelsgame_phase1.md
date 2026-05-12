# Feature Proposal: SentinelsGame — Competitive Transaction Checker Oracle (Phase 1)
Component: `contracts`

---

## Overview

SentinelsGame introduces a competitive transaction checker oracle as a new `IOracle` implementation (`CheckerOracle`). Rather than a single designated approver (as in `SimpleOracle`), a permissioned set of *checker nodes* races to post bonded votes ("Approve" or "Deny") against a transaction within a time-boxed window. The contract escrows the user's fee, collects bonds, and resolves the outcome once the window closes.

**Phase 1** implements the core contract covering the request lifecycle, bonding, unanimous resolution, fee distribution, and timeout-default-reject logic.

Phases:
1. **Phase 1 (this PR)** — Core contract: `CheckerOracle.sol`, request lifecycle, bonding, unanimous resolution, fee distribution, timeout.
2. **Phase 2 (follow-up PR)** — Arbitration: conflict detection, `triggerArbitration`, `resolveDispute`, slashing.
3. **Phase 3 (can be parallelised with Phase 2)** — Checker node service: off-chain daemon in `validator/`.

---

## Architecture Decision

`CheckerOracle` implements `IOracle`. `Consensus` calls `postRequest(requestId)` unchanged — no modifications to `Consensus` or `FROSTCoordinator`.

The contract owns an internal escrow: the proposer's fee is pulled via `transferFrom` at request time and disbursed at finalisation. Checker bonds are pulled at vote time and returned (with rewards or without) at claim time.

Bond thresholds are symmetric: both sides target `fee × bondMultiplier`. Over-contributions are uncollected (only the gap-filling amount is pulled). Finalisation is pull-based (`finalize` then `claim`) to bound gas and prevent reentrancy.

**Timeout default**: if neither threshold is met by the deadline the request resolves rejected and the fee is refunded. This is a defensive fail-safe.

---

## User Flow

1. Proposer pre-approves `CheckerOracle` for `REQUEST_FEE` in the fee token.
2. `Consensus.proposeOracleTransaction` calls `CheckerOracle.postRequest(requestId)` — fee is pulled and escrowed.
3. Active checkers observe `NewRequest`, pre-approve `CheckerOracle` for their bond amount, then call `commitApprove` or `commitDeny`.
4. After the voting window (`VOTING_WINDOW` blocks) closes, anyone calls `finalize(requestId)`.
5. Each winning checker calls `claim(requestId)` to retrieve their bond and proportional fee reward.

---

## Tech Specs

### Files

| File | Change |
|---|---|
| `contracts/src/interfaces/ICheckerOracle.sol` | New interface |
| `contracts/src/CheckerOracle.sol` | New contract |
| `contracts/test/CheckerOracle.t.sol` | Unit tests |
| `contracts/script/DeployCheckerOracle.s.sol` | Deployment script |

### Key Constants (immutables set at deployment)

| Name | Default | Description |
|---|---|---|
| `VOTING_WINDOW` | 12 blocks | Voting window duration |
| `GOVERNANCE_DELAY` | 100 blocks | Delay for checker additions and multiplier updates |
| `REQUEST_FEE` | configured | Fixed fee per request pulled from proposer |
| `FEE_TOKEN` | configured | ERC-20 token for fees and bonds |
| `ARBITRATOR` | configured | Foundation address for governance |

### Fee Distribution

Each winning checker's score is proportional to their bond and inversely proportional to their arrival position (earlier is better):

```
Score_i     = bond_i / position_i
TotalScore  = Σ Score_i          (accumulated on-chain per side during commits)
Reward_i    = fee × Score_i / TotalScore
```

The score is computed with integer arithmetic (Solidity fixed-point). The division `bond / position` truncates at most `position - 1` wei per checker, leaving negligible dust in the contract.

`approveTotalScore` and `denyTotalScore` are incremented on each `commitApprove` / `commitDeny` call, so no iteration over all commitments is required at finalisation.

### Test Scenarios (Phase 1)

- Unanimous Approve flow (multi-checker fee split)
- Unanimous Deny flow
- Timeout / undercapitalised (fee refunded, bonds returned)
- Conflict detection (FROZEN state — arbitration is Phase 2)
- Losing-side bond slashing
- Checker management (add with governance delay, remove immediately, re-add)
- Bond multiplier governance (schedule + apply with delay)

---

## Implementation Phases

### Phase 1 (this PR)

Flows: `postRequest`, `commitApprove`, `commitDeny`, `finalize`, `claim`, `addChecker`, `removeChecker`, `scheduleBondMultiplier`, `applyBondMultiplier`, timeout default.

### Phase 2

Flows: conflict freeze, `triggerArbitration`, `resolveDispute`, loser slash, user fee refund, treasury transfer.

### Phase 3

Flows: off-chain checker daemon, event subscription, address-poisoning detection, bond submission, claim trigger.

---
