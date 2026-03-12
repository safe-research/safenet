# Feature Proposal: Fix concurrent transaction resubmission race condition
Component: `validator`

---

## Overview

On validator startup, the `BlockWatcher` queues all recent blocks as `block_new` events at once. Each event triggers `protocol.checkPendingActions()` fire-and-forget (not awaited), causing multiple concurrent calls. These concurrent calls can pick up the same pending transaction and submit it multiple times simultaneously, resulting in duplicate `sendRawTransaction` calls, bumped-fee submissions, and `ALREADY_EXISTS` RPC errors.

The fix adds an in-flight nonce guard to `OnchainProtocol`: each nonce is tracked while `submitTransaction` is executing, and `checkPendingActions` skips any nonce that is already in-flight.

Single PR — the change is small and self-contained.

---

## Architecture Decision

A private `#inFlightNonces: Set<number>` field is added to `OnchainProtocol`. `submitTransaction` adds the nonce at the start and removes it in a `finally` block. `checkPendingActions` skips nonces present in the set when iterating pending transactions.

This is the minimal, targeted fix:
- No changes to the watcher, service, or state machine
- No additional locking or queuing machinery
- The guard window is limited to the duration of the `sendRawTransaction` RPC call (~ms), so legitimate gas-bump resubmissions (triggered after `blocksBeforeResubmit` blocks, ~seconds) are unaffected

### Alternatives Considered

**Guard on `checkPendingActions` itself** (skip the whole call if one is already running): Simpler, but coarser — would delay resubmitting *all* pending transactions while one long-running resubmit is in progress. Per-nonce tracking is more precise.

**Make `onTransition` async and await `checkPendingActions`**: Fixes the root trigger (fire-and-forget call), but requires changing the watcher callback signature and is a larger, riskier change.

---

## Tech Specs

### Changed files

- `validator/src/consensus/protocol/onchain.ts`
  - New field: `#inFlightNonces: Set<number> = new Set()`
  - `submitTransaction`: wrap body in `try/finally` to add/delete nonce
  - `checkPendingActions`: add `continue` guard for in-flight nonces

- `validator/src/consensus/protocol/onchain.test.ts`
  - New test: concurrent `checkPendingActions` calls result in exactly one `sendRawTransaction` per nonce

### Test cases

1. **Existing tests** — must all continue to pass
2. **New: concurrent resubmit guard** — simulate two `checkPendingActions` calls overlapping on the same pending TX; assert `sendRawTransaction` called exactly once

---

## Implementation Phases

### Phase 1 (this PR): Add in-flight nonce guard

Files touched:
- `validator/src/consensus/protocol/onchain.ts`
- `validator/src/consensus/protocol/onchain.test.ts`

---

## Open Questions / Assumptions

- `blocksBeforeResubmit` defaults to 1 block (~5 s on Gnosis Chain). The `sendRawTransaction` RPC latency is well under 1 block, so no legitimate resubmit will be blocked by this guard in practice.
- If `sendRawTransaction` itself hangs indefinitely, the nonce stays in-flight forever and will never be resubmitted. This pre-existing issue (no timeout on the RPC call) is out of scope for this fix.
