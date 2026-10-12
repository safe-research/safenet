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

## Reconciliation (run 2)

**Final: Unmerged, forward-looking — unchanged.** PR #904's span-aware allocation is not in the tree at `fe9e84c` (`crates/core/src/tx/storage.rs` allocation is still `MAX(?, COALESCE((SELECT MAX(nonce) + 1 …), 0))`, `:145-149`), so run 2 did not assess it; not a miss. Its `main`-side prerequisite, the permanent gap, is `F-CORE-062` (now with executed evidence from `F2-CORE-061`). Re-validate when the stack merges. No run-2 counterpart (`state/run2/reconciliation/core.md` §2).

## Run 3 re-validation (rv-core-b, 2026-10-08)

| Field | Value |
| --- | --- |
| Audit commit | `361aaf3` |
| Verdict | Moot |
| Changed by | The code this row is about never merged: PR #904's JSON-inferred two-nonce span, `pending_delegation` and the standalone startup `SetCode` transaction were explicitly rejected by the merged epic (`git show a9f7eef^:epics/2026_09_23_safenet_7702_executor_tx_batching.md`, lines 130 to 131). The replacement merged as `622cf89` #993 (one `allocated_nonces` row per nonce, an authorization gets its own row), `ce01715` #995 (bundles and authorizations allocated together) and `4847664` #996 (recover an unused authorization nonce). The replacement was checked and does not carry the defect. |
| Severity, certainty | Not applicable (no live defect). Was High / Medium, forward-looking, 80. |
| Evidence | E1 `poc/F-CORE-069/run3/` (two new tests pass; with the recovery disabled both fail, which shows they detect the gap; the merged design's own tests pass) |

Re-based anchors (old `feat/batex_4` locations, merged locations at `361aaf3`, verbatim quote).

- Span-aware allocation via `json_extract` (`storage.rs:194-206` on `feat/batex_4`): no counterpart. Allocation is `crates/core/src/tx/storage.rs:190-195`, `SELECT MAX(?, COALESCE((SELECT MAX(nonce) + 1 FROM allocated_nonces), 0))`, and the authorization's nonce is a row of its own, inserted in the same SQLite transaction (`crates/core/src/tx/storage.rs:200-211`).
- `pending_delegation` and the one-shot `error!` self-check (`storage.rs:126-137`, `mod.rs:198-222` on `feat/batex_4`): no counterpart (`grep -rn pending_delegation crates/` finds nothing). The gap is closed instead, `crates/core/src/tx/storage.rs:294-312`:

```rust
    /// Recovers from the account's onchain `nonce` having moved past a
    /// transaction but not past the authorization it carries, which would
    /// leave every later nonce stuck behind a gap. The authorization at
    /// `nonce`, if there is one, becomes a cancellation transaction
    /// ([`Transaction::default()`]) that is immediately due for submission,
    /// and the transaction before it keeps its authorization. An authorization
    /// is recovered at most once. Returns whether one was recovered.
    pub async fn recover_authorization_gap(&self, nonce: u64) -> Result<bool, Error> {
        let updated = sqlx::query(
            "UPDATE allocated_nonces
             SET request = ?
             WHERE nonce = ? AND delegate IS NOT NULL AND request IS NULL",
        )
```

- It runs on every new block right after `mark_executed` and before `resubmit_stale`, so the cancellation is broadcast in the same pass whatever the in-flight budget (`crates/core/src/tx/mod.rs:207-225`). An unused authorization row keeps `count_outstanding` above zero, so the account is read on every block until it is resolved (`crates/core/src/tx/storage.rs:266-277`).

What was run (worktree at `361aaf3`, `poc/F-CORE-069/run3/tx_mod_tests_run3.rs` appended to `crates/core/src/tx/mod.rs`'s `mod tests`, reverted, `git status --short` empty):

- `cargo test -p safenet-core --lib rv3_ -- --nocapture --test-threads=1`.
- `rv3_f_core_069_unused_authorization_nonce_is_cancelled_not_a_permanent_gap` (the row's own failure path, batched mode): ok. A batch at nonce 0 carries the authorization, which reserves nonce 1; the account then reads nonce 1 with no code. In the same block the reserved row becomes a cancellation (`Transaction::default()`) and is broadcast; once it lands (nonce 2) the waiting transaction is allocated at nonce 2 with a fresh authorization at 3, and after that lands nothing is outstanding. No gap, no stranded backlog.
- `rv3_f_core_069_one_inconsistent_reading_leaves_no_gap` (the row's A4 route): ok. One inconsistent reading of nonce 1 creates the cancellation; the next canonical reading (nonce 2, delegated) marks it executed and the next batch follows at nonce 2 without an authorization.
- Mutation check (`poc/F-CORE-069/run3/mutate_recovery.py`, `output-recovery-disabled.txt`): with `recover_authorization_gap` forced to return `Ok(false)`, both tests FAIL; the waiting transaction is allocated at nonce 2 behind the unused nonce 1, which is exactly the gap this row described. Mutation reverted.
- The merged design's own tests (`output-existing-tests.txt`): `recovers_an_unused_authorization_nonce_with_a_full_budget`, `recovers_an_unused_authorization_nonce_once`, `only_recovers_the_authorization_at_the_account_nonce`, `marks_a_cancellation_executed_once_the_account_is_past_it` (reorg variant), `authorization_uses_up_the_next_nonce`, `delegates_an_undelegated_account_to_the_executor`: 6 passed.

Notes:

- The secondary defect (`pending_delegation` with no `ORDER BY` and no `nonce IS NOT NULL`) has no merged counterpart.
- The "alarm fires once" concern no longer applies: the merged code logs a `warn` once per recovery ("authorization nonce unused onchain, sending a cancellation in its place"), and there is no persistent condition left to alarm about.
- What the replacement does not fix belongs to other rows: if a false nonce reading marks the authorizing transaction executed and the canonical chain never reaches that nonce, the cancellation sits behind the gap and holds the batched mode's only in-flight slot. That is `F-CORE-062` and `F-CORE-063` (both still valid; batched variant in `poc/F2-CORE-061/run3/`), not a gap created by the reservation.
- The `F-CORE-060` interaction recorded in that row's "In-flight impact (FWD)" (repairing a permanent gap releases a ratcheted backlog) is moot with this row.

## Fix verification (CA3-FIX, 2026-10-09)

| Field | Value |
| --- | --- |
| Audit commit | `361aaf3` |
| Result | Moot confirmed |
| Replacement | `622cf89` (#993), `ce01715` (#995), `4847664` (#996): one `allocated_nonces` row per nonce, an authorization in its own row; `recover_authorization_gap` (`crates/core/src/tx/storage.rs:294-312`) runs on every new block right after `mark_executed` (`crates/core/src/tx/mod.rs:210-222`) |
| Re-validation re-run | Matches rv-core-b: both `rv3_f_core_069_*` tests pass |
| New variants | 2 (a restart between the execution mark and the recovery; Direct mode's un-delegation authorization left unused); both recover; all four tests fail with the recovery disabled |
| Confidence | 92 |
| Evidence | E1: `poc/F-CORE-069/ca3-fix/RESULT.txt` (source `ca3fix_tx.rs`) |

What was run: `../run3/tx_mod_tests_run3.rs` and `ca3fix_tx.rs` pasted into `mod tests` of `crates/core/src/tx/mod.rs` (reverted), `cargo test -p safenet-core --lib --locked --offline`, test filters `rv3_f_core_069` and `ca3fix_f_core_069`, with `--nocapture` and `--test-threads=1`.

Variants the re-validation did not list:

1. Restart between `mark_executed` and `recover_authorization_gap`, which are separate statements: the state at the crash point (batch at nonce 0 marked executed, authorization at nonce 1 still unused) written through the storage API, then a new queue over the same database while the account still reads nonce 1. The startup unmark (`tx/mod.rs:188-200`) and the next block re-run the sequence: nonce 1 becomes the cancellation, and once it lands the waiting transaction follows at nonce 2.
2. Direct mode after a Batched-to-Direct switch: the account is still delegated, so Direct mode sends its first transaction with an authorization to the zero address (`tx/mod.rs:248-256`), stored as `delegate = 0x0…0`, not `NULL` (`storage.rs:204-210`). When the un-delegation does not apply (nonce 1, still delegated) the recovery cancels it, and the next transaction follows at nonce 2 with a fresh un-delegation. The gap this row described has no Direct-mode counterpart either.

Mutation: `../run3/mutate_recovery.py apply` (recovery forced to return `Ok(false)`) fails all four tests.
