# F-CORE-069 (UNMERGED) The two-nonce delegation reservation writes a permanent nonce gap into durable state on the epic's own acknowledged failure path, and the only alarm fires exactly once

| Field | Value |
| --- | --- |
| Status | Filed (forward-looking, unmerged code) |
| Crate and module | core, `tx/storage.rs`, `tx/mod.rs` |
| Location | **UNMERGED — applies to branch `origin/feat/batex_4` (PR #904, "[Phase 4] Adjust nonce handling"); not present on `main` (`49d7e39`).** `crates/core/src/tx/storage.rs:190-215` (span-aware allocation), `:126-137` (`pending_delegation`), `crates/core/src/tx/mod.rs:198-222` (the self-check and `mark_executed` call). Specified in `epics/2026_09_09_safenet_7702_executor_tx_batching.md` §"A self-sponsored `SetCode` transaction reserves two nonces". |
| Severity | High / Medium (forward-looking) |
| Certainty | 80% (static; the delegation enqueue that arms this is Phase 5 and is not yet on any pushed branch) |
| Assumptions involved | A1, A4, A10 |
| Tags | unmerged, eip-7702, nonce, liveness, dos |

## Claim

Phase 4 makes nonce allocation span-aware: a row whose `request` JSON carries an `authorization` reserves two nonces instead of one (`storage.rs:195-205`). The second of those two nonces is consumed not by the queue but by the chain, when it applies the self-signed authorization. If the authorization does **not** apply, the account stops one nonce short of the reservation and the queue has written a gap into SQLite that nothing can close:

- Allocation is floored at `MAX(status.nonce, max_row.nonce + span)` — it never allocates _into_ a gap below the high-water mark, which is exactly `F-CORE-062`'s defect.
- Every subsequent transaction is allocated above the gap and can never be included, because the chain executes nonces in order.
- The rows holding those nonces are never dropped: `prune` deletes only executed rows and _unallocated_ expired ones (`storage.rs:301-330`), so `count_in_flight` keeps counting them and, after `max_in_flight_transactions` (default 16) of them accumulate, the queue stops allocating at all.
- Restarting does not help, because the state is durable.

The epic is aware of this and argues it is acceptable because it is a "loud failure (nothing progresses, resubmissions repeat, block metrics flatten) rather than a silent one", backed by an `error!` log in `update_block_status`. **That alarm fires exactly once.** The check is

```rust
if let Some(delegation_nonce) = self.storage.pending_delegation().await?
    && nonce == delegation_nonce + 1
{ tracing::error!(...) }
```

and the very next statement is `self.storage.mark_executed(Status { nonce, .. })`, which sets `executed_at` on the delegation row (its nonce `N` is `< N + 1`). On the following block `pending_delegation()` — which filters `executed_at IS NULL` (`storage.rs:128-133`) — returns `None`, so the branch is dead forever after. A single `error` line, at a level an operator may well not be paging on, is the entire signal for a permanently wedged service. There is no metric, no health-check degradation, and no `Error` returned.

The second-order consequence is worse than the stall itself. The transactions stranded above the gap remain in flight and remain eligible for stale resubmission, so `F-CORE-060`'s fee ratchet runs against them without bound: `record_submission` writes a new fee floor each time and `fees::bump` raises it ≥10% per resubmission, compounding for as long as the service runs. Those transactions can never be included while the gap stands — but if an operator _repairs_ the gap by hand (sending a self-transfer at the missing nonce, the obvious remedy), the entire stranded backlog becomes includable at once at whatever fee the ratchet has reached, bounded only by the signer's balance. The recovery action is therefore the trigger for `F-CORE-060`'s worst case.

## Basis

- Span-aware allocation, `storage.rs:194-206`: `SET nonce = MAX(?, COALESCE((SELECT nonce + IIF(json_extract(request, '$.authorization') IS NULL, 1, 2) FROM transactions WHERE nonce IS NOT NULL ORDER BY nonce DESC LIMIT 1), 0))`.
- The test `allocation_skips_the_two_nonces_reserved_by_a_delegation` (`storage.rs:~600`) asserts the reservation directly: delegation at 5, next transaction at **7**.
- `pending_delegation` filters on `executed_at IS NULL`, and `update_block_status` calls `mark_executed` immediately after the check (`mod.rs:204-222`), so the condition is self-clearing.
- `prune` never deletes a row that holds a nonce and is not executed (`storage.rs:316-329`).
- The epic names the preconditions itself: "reachable only through a construction bug or a contract account at the signer address" — and A4 (an RPC inconsistent between calls) supplies a third, since `nonce` here is `eth_getTransactionCount` and a view that reports `N + 1` for one block is enough to log the error, clear the flag, and leave the queue's belief permanently wrong.

## Secondary defect in the same code: `pending_delegation` is order-undefined

```sql
SELECT nonce FROM transactions
WHERE json_extract(request, '$.authorization') IS NOT NULL AND executed_at IS NULL
LIMIT 1
```

There is no `ORDER BY` and no `WHERE nonce IS NOT NULL`, yet the doc comment promises "the nonce of the outstanding delegation transaction, if one is in flight (queued with a nonce assigned)". With two unexecuted delegation rows — reachable when a reorg's `unmark_executed` restores an already-executed delegation alongside a freshly enqueued one, and reachable generally because `enqueue` has no idempotency key (`F-CORE-067`) and Phase 5's `has_delegation` guard is a read-then-write with no unique constraint — SQLite may return the row whose `nonce` is `NULL`. `.flatten()` turns that into `None` and the authorization self-check above is silently disabled. The three tests that cover this function each create exactly one delegation row, so none of them can catch it.

## Remediation options

- Assert the invariant instead of logging it: if the account nonce lands inside a reservation, return an `Error` (or mark the queue unhealthy and stop allocating) rather than logging once and carrying on. A wedged queue that reports itself wedged is recoverable; one that does not is not.
- Make the check stateless with respect to `mark_executed` — record the reservation's second nonce in its own column and keep alarming while it is unfilled — and back it with a gauge so the condition is visible to monitoring rather than to a log grep.
- Prefer the epic's rejected "barrier" alternative: refusing to allocate while a delegation is in flight cannot create a gap at all, at the cost of one delegation-length pipeline stall per start.
- Fix `pending_delegation` to `WHERE ... AND nonce IS NOT NULL ORDER BY nonce ASC LIMIT 1`, and add a test with two unexecuted delegation rows.
- Independently, give the queue the gap-detection `F-CORE-062` already asks for: assert `status.nonce <= MAX(nonce) + span` and surface a violation.

## Trail

Filed by FWD from `origin/feat/batex_4` + the epic at `epics/2026_09_09_safenet_7702_executor_tx_batching.md`. No PoC: nothing sets `authorization` on any pushed branch (Phase 5 enqueues the delegation), so Phase 4 is behaviourally inert today and this is static-only. Re-validate when Phase 5 lands.
