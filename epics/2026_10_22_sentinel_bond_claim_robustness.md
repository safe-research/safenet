# Plan: Sentinel bond claim robustness

Component: the `crates/sentinel` crate (`service.rs`, `state.rs`, `config.rs`), plus `docs/sentinel-handbook.md` and `docs/configuration.md`. No contract changes.

---

## Overview

On 2026-10-21 a production sentinel on Gnosis lost track of a request it had put an 800 USDC.e bond on, never sent `claim()`, and stopped voting until the bond was recovered by hand. A series of local state-machine mistakes caused this:

1. Another sentinel's `Committed` arrived while our request was still `WaitingForEngineCheck { request: Some(_) }`. `handle_committed` only counts commits in `CollectingCommitments`, so it threw the event away.
2. `commit_vote` started `CollectingCommitments` at `committed_count: 0`, so the local count stayed one short of the onchain count from then on.
3. In `CollectingVotes`, the other sentinel's reveal made `revealed_count == committed_count`, so `handle_revealed` ended voting early before our own reveal had been seen.
4. `finalize()` saw `!self_revealed && !timed_out` and **dropped the request**. When our reveal, the other sentinel's `finalize()` and the `OracleResult` arrived, they were all ignored as untracked, so nothing queued a `Claim`.

While reproducing this we found a second way to strand a bond. If our own reveal never lands, `finalize()` drops the request the same way. Yet the contract only slashes `terms.slashAmount` from a sentinel that didn't reveal (`SentinelOracleRequests.slashAmountFor`), so `bondAmount - slashAmount` can still be claimed and nothing claims it. If no other sentinel calls `finalize()` either, the request stays `PENDING` and the whole bond stays locked.

Both cases come from the same weakness: the FSM could drop a request while we still had a bond locked in it onchain, and nothing made a lost bond visible. This epic enforces one rule:

> **Once our own commit is seen (`self_committed`), a request is only removed from state when our own `Claimed` arrives, or, if the claim deadline passes first, with an `error` log.**

`Claim` is queued with no expiry, and the transaction queue resubmits it until it is mined, so normally our `Claimed` arrives well before the deadline. If it doesn't (the `Claim` reverted, or the queue is stuck), the sentinel doesn't retry. It logs an `error` so an operator can investigate and recover the bond by hand, then removes the request so state doesn't grow without bound.

Steps:

- **Phase 1:** count commitments that arrive during the engine check.
- **Phase 2:** finalize early exactly when the counts match; warn if our own reveal hasn't been seen at that point.
- **Phase 3:** always finalize, and never drop the request, so the unslashed remainder of our bond is claimed even when our own reveal is missing.
- **Phase 4:** add a `Claiming` state that waits for our own `Claimed` until a configurable claim deadline, then logs an `error` and removes the request (no retries).
- **Phase 5:** update the sentinel handbook.
- **Phase 6:** remove this specification.

---

## Architecture Decision

### Fix the FSM paths that drop a bonded request

The bond lifecycle stays in the per-request FSM. The only addition is one new state, `Claiming`. The fix makes sure every state that holds a bond moves on to `Claiming`, which ends either with our own `Claimed` or with an `error` log:

| State | Bond held | Exit |
| --- | --- | --- |
| `WaitingForEngineCheck`, `WaitingForRequest` | no | may be dropped (deadline / engine failure) |
| `CollectingCommitments { self_committed: false }` | no | dropped at commit deadline |
| `CollectingCommitments { self_committed: true }` | yes | `CollectingVotes` at commit deadline |
| `CollectingVotes` | yes | early finalize (`revealed_count == committed_count`) or reveal deadline → `Finalize` queued, `WaitingForOutcome` (**never dropped**, phase 3) |
| `WaitingForOutcome` | yes | terminal event → `Claim` queued, `Claiming` (phase 4) |
| `WaitingForDisputeResolution` | yes | resolution / arbitration timeout → `Claim` queued, `Claiming` (phase 4) |
| `Claiming` (new) | yes, until our `Claimed` | our own `Claimed` → removed; claim deadline passed → `error` log, removed, **no retry** |

Cases that should be impossible under this rule, such as our own `Committed` arriving for an untracked request, or the counts matching before our own reveal is seen, are logged at `warn`. No recovery code is written for them.

### Snapshot compatibility

Snapshots are stored as JSON. The new fields on existing variants (`committed_count` in phase 1, `self_revealed` in phase 3) get `#[serde(default)]` so a running sentinel can load its saved state after upgrading, with no database reset. The new `Claiming` variant (phase 4) doesn't affect existing snapshots. `State` keeps its current shape.

### Alternatives Considered

- **Only fix the counting (phases 1–2).** This fixes the incident, but leaves the "missed own reveal" case stranding `bondAmount - slashAmount`. Rejected.
- **Drop the request as soon as `Claim` is queued (today's behaviour).** The transaction queue resubmits `Claim` until it is mined, so it normally lands. But a `Claim` that reverts, or a stuck queue, would go unnoticed, and the incident shows how expensive an unnoticed stranded bond is. Rejected in favour of waiting for our own `Claimed`.
- **Retry `Claim` (and `Finalize`) from the FSM until our own `Claimed` arrives.** Rejected: the transaction queue already resubmits until mining, and a `Claim` that is mined but reverts points to a bug or an onchain state we don't expect. Resending it would only spend gas. An `error` log and manual investigation fit better.
- **A separate, event-built bond ledger next to the request map.** Rejected: it creates a second source of truth that can disagree with the FSM, and it would require reshaping `State`, with compatibility code for old snapshots.
- **An out-of-band RPC reconciler** (scanning `Committed` logs, calling `getCommitment`/`getRequest`, as the incident report suggests). It isn't needed: after a database reset the indexer replays from its start block, which rebuilds request entries from events. For bonds the indexer can't see, the handbook documents the manual procedure (phase 4).

---

## Tech Specs

### Phase 1: commitment counting during the engine check

- `WaitingForEngineCheck` gains `committed_count: u64` (`#[serde(default)]`).
- `handle_committed` increments it when the entry is `WaitingForEngineCheck { request: Some(_), .. }`. A `Committed` event while `request: None` can't happen onchain (`NewRequest` is always first), so it stays a warning.
- `handle_engine_check_result` passes the count to `commit_vote`, which seeds `CollectingCommitments.committed_count` with it. When the engine check finishes before `NewRequest`, the path through `WaitingForRequest` keeps starting at `0`, which is correct.
- A commit can't be ours here: we only commit after the engine check, so `self_committed` stays `false`.
- `handle_committed`: our own `Committed` for an untracked request is logged at `warn` (today it's logged at `debug` together with everyone else's commits). Nothing else happens.

### Phase 2: exact early-finalize condition with a consistency warning

- `handle_revealed` ends voting early exactly when `revealed_count == committed_count` (today it's `>=`, written as an early return on `<`). If the counts somehow overshoot, the request just waits for the reveal deadline.
- In `CollectingVotes` we have always committed, so matching counts mean our own reveal must already have been seen. If `self_revealed` is still `false` at that point, the local counts are wrong: log a `warn` with the counts, and don't handle it any further.

### Phase 3: finalize never drops a bonded request

- Remove the `!self_revealed && !timed_out` early return in `finalize()` (the actual fix). Its comment ("this sentinel did not participate") is wrong: `CollectingVotes` is only reached with `self_committed`, so our bond is always at stake there, and the unslashed remainder `bondAmount - slashAmount` is only reclaimable once the request is finalized. `finalize()` now always queues `Finalize` and moves to `WaitingForOutcome`; the `timed_out`/`revealed_count` bindings go too. If another sentinel finalizes first, ours reverts with `RequestNotPending`, which is harmless. Either way the terminal event (`OracleResult`, `DisputeTriggered`, or `RequestTimedOut`) goes through the existing handlers, which queue `Claim`.
- `finalize()` returns `RequestState` instead of `Option<RequestState>`, since `None` only came from the dropped case. Its two callers (`handle_revealed` and the reveal-deadline branch of `handle_block_advance`) replace their `match update { None => remove, Some => insert }` with a plain assignment.
- `WaitingForOutcome` gains `self_revealed: bool`, carried over from `CollectingVotes`, and `RequestState::self_revealed()` returns it instead of always `true`. This is only for the metric: `handle_oracle_result` records `ResolvedOutcome::RevealMissed` instead of `Unanimous` when our bond comes back slashed. For old snapshots, `#[serde(default = ...)]` returns `true`, matching today's behaviour: every `WaitingForOutcome` saved before the upgrade either came from a path where we revealed, or from a total timeout, which ends in `RequestTimedOut` and never reads the field.
- If `slashAmount >= bondAmount`, the eventual `claim()` returns nothing. This isn't special-cased: at worst it costs one empty `claim()`.
- Update the doc comments on `finalize`, `handle_oracle_result` and `SentinelRequestState::self_revealed` to match.

### Phase 4: wait for our own `Claimed`, with a deadline

- New config field `SentinelConfig.claim_window: u64`: the number of blocks to wait for our own `Claimed` after queuing `Claim`. It is passed into `SentinelTransition` like `voting_window`.
- New variant `SentinelRequestState::Claiming { deadline: u64 }`, where `deadline = block + claim_window` and `block` is the block in which `Claim` was queued.
- Every place that queues `Claim` and then removes the request moves it to `Claiming` instead: `handle_oracle_result`, `handle_request_timed_out`, `handle_resolved`, `handle_arbitration_timeout`, and the arbitration-deadline branch of `handle_block_advance`. The event handlers don't take the log's block number today; it is passed in from `apply_transition` (`log.block.number`).
- `handle_claimed`: when `event.sentinel == self.signer.address()` and the entry is `Claiming`, remove it. Our own `Claimed` for any other state, or for an untracked request, is logged at `warn`. The fee-reward metric stays as it is.
- `handle_block_advance`: `Claiming` with `block > deadline` → log an `error` (request ID, deadline) and remove the request. No action is queued, so there is no retry.
- Terminal events that arrive while the entry is already `Claiming` (for example re-delivered logs) are ignored with a `debug` log, so they don't queue a second `Claim`.
- `approve_and_slash_amount()`/`self_revealed()` get `Claiming` match arms. Terminal handlers never act on `Claiming`, so these values aren't used there.
- `Claim` keeps `expires_at: None`. Removing the request at the deadline only stops local tracking; it doesn't cancel a `Claim` that is still in the transaction queue.
- Update `docs/configuration.md`, `crates/sentinel/sentinel.sample.toml`, and the sentinel configs written by `scripts/run_devnet.sh` and `scripts/run_sentinel_integration_test.sh` (unless the field gets a default, see Open Questions).

### Testing

Following the sentinel FSM test policy, tests run real event sequences through `apply_transition` instead of editing state directly:

- Phase 1: the incident flow. Proposal, `NewRequest`, the other sentinel commits during the engine check, the engine approves, we commit, then reveals arrive. Assert that `committed_count == 2` and that the request is not dropped after the other sentinel's reveal.
- Phase 2: the same flow with the other sentinel revealing first. Assert no `Finalize` until our reveal (the counts only match once ours is in); after our reveal, `Finalize` and then `Claim` on `OracleResult`.
- Phase 3: our reveal never lands and the other sentinel reveals. At the reveal deadline, assert that `Finalize` is sent and the state becomes `WaitingForOutcome { self_revealed: false, .. }`. On `OracleResult`, assert `Claim`.
- Phase 4: existing terminal flows end in `Claiming` and are removed only when our own `Claimed` arrives; another sentinel's `Claimed` leaves our entry alone. A re-delivered terminal event while `Claiming` queues no second `Claim`. Past the claim deadline without our `Claimed`, the request is removed and no action is queued. Existing flow tests are updated to append our own `Claimed`.
- `just test-integration-sentinel` stays green for every phase.

---

## Implementation Phases

Phases 1, 2, 3 and 4 are independent and can be reviewed in parallel. They touch nearby code in `service.rs`, so later merges need trivial rebases. Phase 5 follows phases 1–4.

### Phase 1: count commitments received during the engine check

- Files: `crates/sentinel/src/state.rs`, `crates/sentinel/src/service.rs` (+ incident regression flow test).
- About 80 lines.

### Phase 2: exact early-finalize condition with a consistency warning

- Files: `crates/sentinel/src/service.rs` (+ flow test).
- About 40 lines.

### Phase 3: always finalize instead of dropping when our reveal is missing

- Files: `crates/sentinel/src/state.rs`, `crates/sentinel/src/service.rs` (+ missed-reveal flow test; adjust existing `WaitingForOutcome` assertions to the new field).
- About 150 lines.

### Phase 4: `Claiming` state with a claim deadline

- Files: `crates/sentinel/src/config.rs`, `crates/sentinel/src/main.rs`, `crates/sentinel/src/state.rs`, `crates/sentinel/src/service.rs`, `crates/sentinel/sentinel.sample.toml`, `docs/configuration.md`, and the two scripts (+ updated flow tests).
- About 250 lines, mostly tests that now append our own `Claimed`.

### Phase 5: documentation

- Files: `docs/sentinel-handbook.md`. Describe the bond lifecycle (the table above), the "claim deadline passed" `error` log and what it means, and the manual recovery procedure for a stranded bond (compare our own `Committed`/`Claimed` events, then call `claim()` with the sentinel key).

### Phase 6: remove this specification

- Delete `epics/2026_10_22_sentinel_bond_claim_robustness.md` once phases 1–5 are merged and the open questions below are resolved.

---

## Open Questions and Assumptions

- **Assumption:** onchain, `NewRequest` always comes before any `Committed` for the same request, so commits can only appear in `WaitingForEngineCheck` once `request` is `Some`.
- **Assumption:** `Finalize` and `Claim` are queued with `expires_at: None`, and the transaction queue resubmits them until they are mined. A `Finalize` that reverts with `RequestNotPending` (someone else finalized first) is expected and harmless. A reverting `Claim` means something else is wrong; phase 4 makes it visible through the `error` log at the claim deadline instead of retrying.
- **Open:** should `claim_window` be required, like `voting_window`, or have a default? A default (for example a few hundred blocks) avoids breaking existing sentinel configs on upgrade. Making it required matches the current `voting_window` policy (see the TODO in `config.rs`), but every deployment config then has to be updated together with the release.
- **Open:** should the claim-deadline case also bump a metric (for example `requests_resolved_total` with a `ClaimMissed` outcome), so it can be alerted on and not just found in logs?
- **Operational, outside this epic:** running sentinels with capital for only one bond makes any stranded bond block all further voting. The handbook should recommend keeping several bonds' worth of fee token.
