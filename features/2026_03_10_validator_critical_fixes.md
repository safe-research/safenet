# Feature Proposal: Validator Critical Fixes
Component: `validator`

---

## Overview

Two targeted fixes for the validator workspace:

1. **Typo fix**: Rename exported `calcTreshold` → `calcThreshold` in `group.ts` and update call sites.
2. **Epoch verification**: Add validation in `EpochRolloverHandler` to reject rollover packets where `proposedEpoch <= activeEpoch`. Accepts an optional `EpochCheck` callback for future state-dependent checks.

Single PR covers both fixes.

---

## Architecture Decision

- The epoch check uses `proposedEpoch > activeEpoch` (not strict `+1`) because epochs can be skipped when keygen fails/aborts. This is the normal recovery path: after a failed keygen, the next successful keygen targets a later epoch, so the gap between `activeEpoch` and `proposedEpoch` can be greater than 1.
- The optional `EpochCheck` callback follows the same constructor-injection pattern used by `SafeTransactionHandler`, keeping the handler testable and extensible.

### Alternatives Considered

- **Strict sequential check (`proposedEpoch === activeEpoch + 1`)**: Rejected because it breaks the keygen abort recovery flow where epochs are legitimately skipped.

---

## User Flow

N/A — internal validator logic, no user interaction.

---

## Tech Specs

### Files Modified

- `validator/src/machine/keygen/group.ts` — rename `calcTreshold` → `calcThreshold`
- `validator/src/machine/keygen/trigger.ts` — update import and call site
- `validator/src/consensus/verify/rollover/handler.ts` — add epoch verification and `EpochCheck` callback
- `validator/src/consensus/verify/rollover/handler.test.ts` — tests for epoch verification

### Test Cases

- Valid packet with `proposedEpoch = activeEpoch + 1` passes
- Valid packet with `proposedEpoch > activeEpoch + 1` (skipped epochs) passes
- Packet with `proposedEpoch = activeEpoch` is rejected
- Packet with `proposedEpoch < activeEpoch` is rejected
- `EpochCheck` callback is invoked when provided
- `EpochCheck` error propagates correctly

---

## Implementation Phases

Single PR — both fixes are small and self-contained.

---

## Open Questions / Assumptions

- The `EpochCheck` callback is not wired in `service.ts` yet — it's a hook for future state-dependent validation.
