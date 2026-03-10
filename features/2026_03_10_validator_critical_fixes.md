# Feature Proposal: Validator Critical Fixes
Component: `validator`

---

## Overview

Address three concrete issues in the validator that were identified during codebase review: a typo in an exported function name, a missing epoch verification step in the rollover handler, and seven unestimated gas values for on-chain transaction submissions.

**Phases:**

1. **Phase 1** — Rename `calcTreshold` → `calcThreshold` and remove the epoch verification TODO (independent PR)
2. **Phase 2** — Benchmark and update gas estimates for all on-chain calls (independent PR)

---

## Architecture Decision

No architectural changes. All fixes are drop-in corrections to existing code. Phase 1 is a pure rename + small implementation; Phase 2 requires running the Foundry gas reporter against the deployed contracts on a test chain and updating constants.

### Alternatives Considered

- **Dynamic gas estimation at runtime** (e.g. `eth_estimateGas` before each submission): adds an extra RPC round-trip per transaction and can fail if the call would revert. Hardcoded conservative estimates with a known safety margin are simpler and sufficient.

---

## Tech Specs

### Phase 1a — Rename `calcTreshold` → `calcThreshold`

The exported function `calcTreshold` in `validator/src/machine/keygen/group.ts:45` has a typo. It is used in two call sites.

**Files touched:**
- `validator/src/machine/keygen/group.ts` — rename function, update internal call on line 68
- `validator/src/machine/keygen/trigger.ts` — update import and call on lines 7 and 29

The rename is a breaking change to the exported API but `calcTreshold` is only consumed within the `validator` workspace, so no other workspace is affected.

### Phase 1b — Implement Epoch Verification in Rollover Handler

`EpochRolloverHandler.hashAndVerify` in `validator/src/consensus/verify/rollover/handler.ts:9` has a `// TODO: verify epoch` comment with no implementation. The method parses and hashes the packet but does not verify that the epoch numbers in the packet are consistent with the validator's current consensus state.

The verification should check that:
1. The rollover packet's `activeEpoch` matches the validator's tracked active epoch.
2. The rollover packet's `proposedEpoch` equals `activeEpoch + 1`.
3. The rollover packet's `rolloverBlock` is in the future (greater than the current block).

The consensus state required for checks 1 and 3 is available from the existing `ConsensusState` type. The handler should receive the current state as a constructor argument (consistent with how other handlers in `validator/src/consensus/verify/` are structured).

**Files touched:**
- `validator/src/consensus/verify/rollover/handler.ts` — implement epoch verification
- `validator/src/consensus/verify/engine.ts` — pass consensus state to `EpochRolloverHandler` if not already done

### Phase 2 — Gas Estimation

Seven gas values in `validator/src/consensus/protocol/onchain.ts` are annotated as unestimated:

| Line | Call | Current placeholder |
|------|------|---------------------|
| 303 | `keyGenCommit` (per-share component) | `250_000n + shares.length * 25_000n` |
| 329 | `keyGenSecretShare` | `300_000n` |
| 339 | `keyGenComplain` | `300_000n` |
| 362 | `keyGenComplaintResponse` | `400_000n` |
| 481 | `signRevealNonces` | `400_000n` |
| 491 | `signShared` | `400_000n` |
| 501 | `signShareWithCallback` | `400_000n` |

**Approach:** Run `forge test --gas-report` against the full test suite on Sepolia and Gnosis Chain testnets. Use the 95th-percentile observed gas as the basis and add a 20% safety margin. Remove the TODO comments once values are set.

**Files touched:**
- `validator/src/consensus/protocol/onchain.ts` — update gas constants at lines 303, 329, 339, 362, 481, 491, 501

---

## Implementation Phases

### Phase 1 — Rename + Epoch Verification (single PR)

**Files touched:**
- `validator/src/machine/keygen/group.ts`
- `validator/src/machine/keygen/trigger.ts`
- `validator/src/consensus/verify/rollover/handler.ts`
- `validator/src/consensus/verify/engine.ts` (if needed)

**Test cases:**
- Existing `group.ts` tests (if any) continue to pass after rename.
- New tests for `EpochRolloverHandler.hashAndVerify`:
  - Valid packet with correct epoch numbers → returns hash, no error.
  - Packet with wrong `activeEpoch` → throws.
  - Packet with non-sequential `proposedEpoch` → throws.
  - Packet with `rolloverBlock` in the past → throws.

Run `npm run check` and `npm run test` in the `validator` workspace before merging.

---

### Phase 2 — Gas Estimates (single PR)

**Files touched:**
- `validator/src/consensus/protocol/onchain.ts`

Run `npm run check` in the `validator` workspace before merging.

---

## Open Questions / Assumptions

1. **Epoch verification state source**: The rollover handler needs access to the current consensus state. Confirm whether it is passed via constructor injection (matching other handlers) or retrieved from a shared store.
2. **Gas estimation environment**: Gnosis Chain has significantly lower gas costs than Ethereum mainnet. Confirm which chain(s) the gas estimates should be optimised for, and whether a single set of values is used across all chains or chain-specific values are maintained.
3. **Rollover block validation**: Check whether the `rolloverBlock` can legitimately be in the past when a validator processes a delayed packet (e.g. after a restart). If so, the past-block check may need to be relaxed or omitted.
