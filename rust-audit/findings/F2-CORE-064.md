# F2-CORE-064 `expires_at` stops applying the moment a nonce is allocated: rows never accepted by a node are still broadcast after their deadline, and a row that can never be accepted blocks every later nonce with no cancellation path

| Field | Value |
| --- | --- |
| Status | QA'd |
| Crate and module | safenet-core, tx/storage.rs, tx/mod.rs |
| Location | crates/core/src/tx/storage.rs:294-298 (related: tx/storage.rs:150-155, 259-265; tx/mod.rs:224-237, 284-293) |
| Severity | Low (reviewer) / Low (Critic) |
| Certainty | 90% (QA2-CORE; Critic C2-CORE-B set 70%) |
| Assumptions involved | A1, A4 |
| Tags | dos, crash-consistency |

## Claim

Expiry is consulted in two places only: when choosing the next row to allocate (`nonce IS NULL AND expires_at > latest`) and when pruning unallocated rows. Once a nonce is assigned the row is resubmitted until executed regardless of `expires_at`. The tests document this as intentional for transactions that reached the mempool ("once a transaction is in the mempool, it has to execute"), which is right: an accepted transaction cannot be un-sent. The same rule is applied, however, to rows that were allocated but _never accepted_ (`submitted_at IS NULL`: transport failure, node down, generic rejection). Those are not in any mempool, yet after the outage they are signed and broadcast past their deadline -- for the sentinel, a commit or reveal after `commit_deadline` / `reveal_deadline`, plus the `ApproveToken` that precedes it -- and mine only to revert. Gas is spent, an in-flight slot is consumed, and the revert is invisible to the queue (F2-CORE-061).

The second half of the same design is that an allocated row can never be removed or neutralised: there is no cancellation (no-op replacement at the same nonce) and no drop-if-highest-nonce path. Any row the node permanently refuses -- a frozen floor above the node's fee cap (F2-CORE-060), an account that cannot cover its `gas` for that action, a zero tip on a geth node (F2-CORE-062) -- holds its nonce forever, and the sequential-nonce rule means every later action queues behind it until an operator intervenes with a manual transaction from the signer key.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Expiry gates allocation only. | E2 | crates/core/src/tx/storage.rs:150-155 | `WHERE id = (` / `SELECT id FROM transactions` / `WHERE nonce IS NULL AND (expires_at IS NULL OR expires_at > ?)` / `ORDER BY id ASC` / `LIMIT 1` / `)` |
| 2 | Pruning by expiry is restricted to unallocated rows. | E2 | crates/core/src/tx/storage.rs:259-265 | `sqlx::query(` / `"DELETE FROM transactions` / `WHERE nonce IS NULL AND expires_at IS NOT NULL AND expires_at <= ?",` / `)` / `.bind(safe)` |
| 3 | Never-accepted rows are resubmitted every block without any expiry condition. | E2 | crates/core/src/tx/storage.rs:281-284, 294-298 | `/// Returns the in-flight transactions due for (re)submission, ordered by` / `/// nonce: those last submitted at or before \`submitted_before\`, as well as`/`/// any that were assigned a nonce but never recorded as submitted (so they`/`/// are not stranded holding a reserved nonce).`...`WHERE nonce IS NOT NULL AND executed_at IS NULL`/`AND (submitted_at IS NULL OR submitted_at <= ?)` |
| 4 | Generic rejections keep the row and the nonce with no attempt counter or terminal state. | E2 | crates/core/src/tx/mod.rs:287-295 | `Err(err) => {` / `tracing::warn!(` / `nonce = submission.nonce,` / `?err,` / `"submission failed, will retry without bumping fees"` / `);` / `}` / `}` / `Ok(())` |
| 5 | The design rationale in the tests addresses accepted transactions specifically. | E2 | crates/core/src/tx/mod.rs:573-577 | `// At block 12, another transaction gets mined, but the outstanding` / `// transaction has already expired and is not executed. However, we` / `// do get resubmissions of the remaining original inflight transactions` / `// because of the resubmit deadline, despite being past the expiry. This` / `// is because once a transaction is in the mempool, it has to execute.` |
| 6 | Services attach deadlines that the contracts enforce; the sentinel's commit and its token approval both carry `commit_deadline`. | E2 | crates/sentinel/src/service.rs:231, 239 | `expires_at: Some(commit_deadline),` ... `expires_at: Some(commit_deadline),` |
| 7 | There is no code path that deletes or rewrites an allocated row other than marking it executed. | E2 | crates/core/src/tx/storage.rs:58-312 (whole impl: `enqueue`, `count_in_flight`, `next_transaction`, `record_submission`, `count_outstanding`, `mark_executed`, `prune`, `unmark_executed`, `stale_submissions`) | `pub async fn prune(&self, safe: u64) -> Result<(), Error> {` ... `pub async fn unmark_executed(&self, block: u64) -> Result<(), Error> {` (no other `DELETE`/`UPDATE` of allocated rows) |

## Trigger

1. Sentinel queues `ApproveToken` and `Commit` with `expires_at = commit_deadline`; both are allocated at `latest = commit_deadline - 1`; the RPC returns a transport error (test `retries_failed_submissions_without_bumping_fees` shows the state: nonce held, no floor). The RPC is back three blocks later; both rows are broadcast, mine, and the commit reverts past its deadline while the approval succeeds pointlessly.
2. Any permanently rejected row (see the cross-referenced findings) blocks the queue with no automatic way out.

## Considered and rejected

- _Dropping an allocated row would open a nonce gap._ True for a row below the highest allocated nonce; not for the highest one (it can simply be released), and a no-op self-transfer at the same nonce is always safe. Neither exists today.
- _Expired-in-flight is rare because allocation happens right before broadcast._ The allocation-to-acceptance window is the RPC's availability; outages spanning a deadline are the exact case deadlines exist for.
- _The reverting transaction is cheap._ Reverts are charged up to the revert point; approvals succeed and cost full gas; and each occupies one of 16 in-flight slots until mined.

## Remediation options

1. Apply expiry to rows with `submitted_at IS NULL`: if `expires_at <= latest` and the row holds the highest allocated nonce, clear the nonce (or delete the row); otherwise replace the payload with a zero-value self-transfer so the nonce sequence stays intact. Tradeoff: a never-recorded broadcast (crash between send and record) could already be in a mempool; the no-op replacement handles that case correctly since it is a valid replacement.
2. Add a terminal state after N consecutive generic rejections (with the same no-op replacement) and a metric/alert.
3. At minimum, log at error (not warn) when a row is resubmitted past its `expires_at`.

Tests to add: allocate with `push_failure_msg`, advance past `expires_at`, assert no broadcast (or a no-op replacement) on the next block.

## Trail

- Reviewer R3: drafted at commit 3ec8bc5, self-estimate 60%
- Critic C2-CORE-B: Confirmed, 70%, severity Low (reviewer Low).
- QA2-CORE: Reproduced (never-accepted row broadcast at block 14 with `expires_at = 12`); certainty 70 → 90; PoC `poc/F2-CORE-064/`.

## Critic (C2-CORE-B)

Method: read title and Location only, traced every read and write of `expires_at` in `tx/storage.rs` and the three arms of `submit_transaction` myself, then compared.

| # | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | storage.rs:150-155. |
| 2 | Supported | storage.rs:259-265. |
| 3 | Supported | storage.rs:281-284, 294-298: no expiry term in the predicate. |
| 4 | Supported | mod.rs:287-295. |
| 5 | Supported | mod.rs:573-577. |
| 6 | Supported | sentinel service.rs:231, 239. |
| 7 | Supported | The whole `impl TransactionStorage` (storage.rs:58-312) contains no `DELETE`/`UPDATE` that touches a row with `nonce IS NOT NULL` other than `record_submission`, `mark_executed` and `unmark_executed`. |

Own trace of trigger 1: allocation at `latest = D - 1` with `expires_at = D` passes `expires_at > ?` (storage.rs:153); a transport failure lands in the generic arm and leaves `submitted_at NULL` (mod.rs:287-293; test 613-640 shows the resulting row); after the outage `stale_submissions` returns the row with no expiry test (storage.rs:296-297) and it is broadcast past `D`. That a late `commit` reverts is a contract property consistent with the sentinel's own deadline handling (service.rs:410-417); not re-opened in Solidity.

Finding verdict: **Confirmed**. Certainty **70%** (E2; trigger 1 concrete, not executed). Severity **Low / Low**. Trigger 2 (a _permanently_ rejected row) depends on F2-CORE-060/062 to supply the rejection; on its own this finding establishes the missing cancellation path.

Remediation check: the reviewer's caveat is right and important — a row with `submitted_at NULL` may still be in a mempool after a crash between `send_raw_transaction` and `record_submission` (mod.rs:265-266), so "clear the nonce" is unsafe except for a same-nonce no-op replacement; option 1 as written handles that.

## QA (QA2-CORE)

**Outcome: Reproduced** (trigger 1).

Command: paste `poc/F2-CORE-064/tx_mod_tests.rs` into the `mod tests` of `crates/core/src/tx/mod.rs`; `cargo test -p safenet-core --lib qa_f2_core_064 -- --nocapture --test-threads=1`; file reverted.

Decisive output (`poc/F2-CORE-064/output.txt`): a row with `expires_at = 12` allocated at block 11 during an RPC outage (`block 11: [(1, .., Some(12), Some(0), None, None)]`), still failing at 12 and 13, then `block 14: [(1, .., Some(12), Some(0), Some(14), None)]` — signed and broadcast at block 14, two blocks after its expiry, with fees now stamped on the row.

Certainty: 70 → **90**. Confirmed plus `E1` for trigger 1; trigger 2 depends on F2-CORE-060/062.

Remediation check: option 1 is sound only in its no-op-replacement form. Its "clear the nonce / delete the row when it holds the highest allocated nonce" branch is unsound: a crash between `send_raw_transaction` and `record_submission` (`mod.rs:265-266`) leaves a pooled transaction with `submitted_at NULL`, and releasing that nonce would let the queue reuse it for a different payload. Always replace at the same nonce. Options 2 and 3 sound.

## Reconciliation (run 2)

**Final: EXTENDS `F-CORE-064` (canonical) and the no-cancellation half of `F-CORE-062` — combined Medium, 90 (E1).** This file executes the never-accepted-row case and shows the "clear the nonce" fix branch is unsafe; run 1 carries the fee-escalation-past-deadline composition. This file's Low is recorded; Medium is carried (`state/run2/reconciliation/core.md` §1).
