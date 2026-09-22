# F2-CORE-063 Actions replayed after a restart or rollback are enqueued and broadcast again with a fresh nonce; the queue has no idempotency key although its retention window matches the replay window exactly

| Field | Value |
| --- | --- |
| Status | QA'd |
| Crate and module | safenet-core, tx/storage.rs, tx/mod.rs |
| Location | crates/core/src/tx/storage.rs:89-104 (related: tx/mod.rs:128-141; driver.rs:243-289; state/mod.rs:55-70, 181-188) |
| Severity | Low (reviewer) / Low (Critic) |
| Certainty | 90% (QA2-CORE; Critic C2-CORE-B set 75%) |
| Assumptions involved | A1 |
| Tags | crash-consistency, reorg |

## Claim

The state machine's documented contract is at-least-once: effects "may be performed more than once for the same chain message, for example after a crash or reorg replay", and every restart replays the last `max_reorg_depth` blocks from the `safe` snapshot after a synthetic uncle, re-emitting the actions those blocks produced. The transaction queue is the component that turns actions into onchain side effects, and it applies no deduplication at all: `enqueue` is an unconditional `INSERT`, every row gets its own nonce, and every row is broadcast. So each restart (and each real reorg within the window) resubmits every action emitted in the replayed blocks whose `expires_at` has not passed -- and, for the timeless actions (`expires_at: None`: the validator's nonce registration, three sentinel actions), always. The contract's cost is pushed entirely onto the contracts (the duplicate must revert or be a no-op) and the signer balance (gas for the duplicate, plus an in-flight slot).

What makes this worth a finding rather than a design note is that the queue already holds exactly the information needed to deduplicate: executed rows are retained until `safe` and replay never reaches below `safe`, so a replayed action's original row is always still present when the duplicate arrives. A uniqueness check on `request` among retained rows would be exact.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The contract: effects/actions may be replayed. | E2 | crates/core/src/state/mod.rs:55-58 | `/// Effects may be performed more than once for the same chain message,` / `/// for example after a crash or reorg replay. Transitions that emit effects must be` / `/// prepared for the replayed effect to resume with a different result.` |
| 2 | A rollback re-applies the events after the uncle, re-running the transitions that emit actions. | E2 | crates/core/src/state/mod.rs:181-188 | `Update::Block(BlockUpdate::Uncle { number })` / `if matches!(status, Status::BlockPending { pending } if number < pending)` / `\|\| matches!(status, Status::BlockEvents { latest } if number <= latest) =>` / `{` / `let (_, state) = self.snapshots.reorg(number).await?;` |
| 3 | Every restart with a persisted snapshot emits a synthetic uncle at `safe + 1`. | E2 | crates/core/src/index/blocks.rs:254-266 | `// The earliest retained snapshot is the rollback anchor. Replay` / `// everything after it, but only emit an uncle when there are newer` / `// snapshots to discard.` ... `let uncle = indexed.safe.checked_add(1);` / `if let Some(uncle) = uncle` / `&& uncle <= indexed.latest` / `{` / `self.queue.push_back(BlockUpdate::Uncle { number: uncle });` |
| 4 | The driver pushes every emitted action straight into the queue. | E2 | crates/core/src/driver.rs:275-283 | `state::Command::Action(action) => {` / `transactions.push(self.actions.encode_action(action));` / `}` ... `if !transactions.is_empty() {` / `let result = self.transactions.queue(transactions).await;` |
| 5 | Enqueue is an unconditional insert; no uniqueness on `request`, no idempotency key. | E2 | crates/core/src/tx/storage.rs:94-100 | `for (transaction, expires_at) in transactions {` / `let request = serde_json::to_string(&transaction)?;` / `sqlx::query("INSERT INTO transactions (request, expires_at) VALUES (?, ?)")` / `.bind(request)` / `.bind(expires_at.map(i64::try_from).transpose()?)` / `.execute(&mut *tx)` |
| 6 | Each row is allocated its own nonce and broadcast. | E2 | crates/core/src/tx/mod.rs:204-216 | `let in_flight = self.storage.count_in_flight().await?;` / `for _ in in_flight..self.config.max_in_flight_transactions {` / `let nonce = self.nonce().await?;` / `let Some(transaction) = self` / `.storage` / `.next_transaction(Status { nonce, block })` ... `self.submit_transaction(transaction, block).await?;` |
| 7 | Executed rows are retained until `safe`, i.e. exactly the replay window. | E2 | crates/core/src/tx/storage.rs:237-239, 250 | `/// Prunes transactions that can no longer be affected by a reorg: those` / `/// executed at or below the reorg-safe block \`safe\`, and queued transactions`/`/// that expired at or before it.`...`sqlx::query("DELETE FROM transactions WHERE executed_at IS NOT NULL AND executed_at <= ?")` |
| 8 | Timeless actions exist in both services, so some duplicates are never filtered by expiry. | E2 | crates/validator/src/service/action.rs:252; crates/sentinel/src/service.rs:525, 572, 642 | `// Nonce registration doesn't carry an expiry - we cannot` ... `expires_at: None,` |

## Trigger

Queue an action at block B (it is submitted and, say, mined at B+1). Stop the service before `safe` passes B+1 (within five blocks at defaults, or any time it is restarted routinely). On start, the watcher emits `Uncle { safe + 1 }`, the state rolls back, the events of B are re-applied, the same action is encoded, `enqueue` inserts a second row with identical `request`, `next_transaction` gives it nonce `MAX(nonce)+1`, and it is broadcast. Onchain the second call reverts or is a no-op; gas is paid either way. A real reorg of depth 1 that keeps the triggering event produces the same duplicate without a restart.

## Considered and rejected

- _Expiry filters duplicates._ Only for actions whose `expires_at` has passed by the time of replay; within a five-block window most have not, and `None` never does.
- _The queue's `processes_each_block_status_once` test covers replay._ It covers repeated _statuses_, not repeated _actions_ (mod.rs:437-455).
- _Dedup is unsafe because identical calldata can be a legitimately distinct intent._ Within the retention window and with calldata that includes request ids / epochs (as the encoders here do), an identical `request` from a replayed message is the same intent by construction; a dedup keyed on `request` plus a "not yet executed or executed above `safe`" condition is conservative.

## Remediation options

1. In `enqueue`, skip an insert when an identical `request` exists among retained rows (`executed_at IS NULL OR executed_at > safe`, which is all retained rows); optionally log at debug. Cheap and exact given the retention rule.
2. Have the state machine hand the queue a deterministic action id (block, log index, action kind) and store it in a `UNIQUE` column; `INSERT OR IGNORE`. More invasive but explicit.
3. Document the at-least-once behaviour of the queue in `tx/mod.rs` and in the handbooks so operators expect duplicate reverts after restarts.

Tests to add: enqueue the same `Transaction` twice around a simulated restart (`block_status = None` as in `initial_status_reconciles_executions_in_the_reorg_window`) and assert one broadcast.

## Trail

- Reviewer R3: drafted at commit 3ec8bc5, self-estimate 70%
- Critic C2-CORE-B: Confirmed, 75%, severity Low (reviewer Low). Canonical of the pair with F2-CORE-032.
- QA2-CORE: Reproduced (replayed identical action inserted, allocated nonce 1 and broadcast while the original is retained); certainty 75 → 90; PoC `poc/F2-CORE-063/`.

## Critic (C2-CORE-B)

Method: read title and Location only, traced `enqueue`/`queue`/`next_transaction`, the driver dispatch and the restart replay myself, then compared.

| # | Verdict | Note |
| --- | --- | --- |
| 1 | Supported, line correction | The quoted doc lines are state/mod.rs:60-62 (the `Command` doc), inside the section the row cites as 55-58. |
| 2 | Supported | state/mod.rs:182-189. |
| 3 | Supported | index/blocks.rs:261-266. |
| 4 | Supported | driver.rs:275-283. |
| 5 | Supported | tx/storage.rs:94-100: unconditional `INSERT`, no `UNIQUE`. |
| 6 | Supported | tx/mod.rs:204-216. |
| 7 | Supported | tx/storage.rs:237-239, 250. |
| 8 | Supported | validator service/action.rs:252-254 (`None,`), sentinel service.rs:525, 572, 642 (`expires_at: None`). |

Finding verdict: **Confirmed**. Certainty **75%** (E2; the duplicate path is fully traced; the on-chain revert for one action is traced in F2-CORE-032 row 8). Severity **Low / Low**: bounded gas and in-flight pressure.

Same defect as **F2-CORE-032** (R2, driver side). **F2-CORE-063 is canonical**: the missing idempotency lives in `TransactionStorage::enqueue`, and the observation that the queue's retention (rows kept until `executed_at <= safe`) equals the replay window is what makes a fix exact. F2-CORE-032 contributes two points that should travel with the merge: the queue is never rolled back on `Uncle` (nothing calls it there, driver.rs:249/283 are the only queue calls), and the traced contract revert. F2-SEN-007 (R7) is the sentinel's own statement of the same replay duplicates.

Correction to the rejected item "an identical `request` from a replayed message is the same intent by construction": not for `ApproveToken` — it encodes `approve(oracle, bond)` with no request id (sentinel service.rs:226-233), and a `commit` consumes the allowance, so two requests with equal `bond_target` inside the window need two approvals. Remediation 1 keyed on `request` alone would drop the second and make the second `Commit` fail; remediation 2 (a deterministic action id from block and log index, `UNIQUE` column, `INSERT OR IGNORE`) is the safe one. `INSERT OR IGNORE` on a `UNIQUE` column is plain SQLite and needs nothing from sqlx beyond `query(..).execute`.

Interaction: the replayed duplicate `Commit` is also what pins `self_committed = false` in F2-CORE-030's replay variant, so fixing this finding alone does not fix that one.

## QA (QA2-CORE)

**Outcome: Reproduced.**

Command: paste `poc/F2-CORE-063/tx_mod_tests.rs` into the `mod tests` of `crates/core/src/tx/mod.rs`; `cargo test -p safenet-core --lib qa_f2_core_063 -- --nocapture --test-threads=1`; file reverted.

Decisive output (`poc/F2-CORE-063/output.txt`): after the action executed at block 11 and a simulated restart (`block_status = None` → `{latest: 11, safe: 10}`), the replayed identical action yields `rows after the replay: [(1, .. "data":"0x5afe" .., nonce Some(0), submitted_at Some(10), executed_at Some(11)), (2, .. "data":"0x5afe" .., nonce Some(1), submitted_at Some(11), executed_at None)]` — a second row with identical calldata, allocated nonce 1 and broadcast (the mock's transaction-hash response was consumed) while the original is still retained.

Certainty: 75 → **90**. Confirmed plus `E1`; the on-chain revert of the duplicate stays traced (F2-CORE-032 row 8).

Remediation check: agree with the Critic that option 1 keyed on `request` is unsound for `ApproveToken` and option 2 is the safe one, with one addition: the deterministic id must not be `(block number, log index)` alone, since after a real reorg a different event can occupy the same position — include the block hash (or a hash of the encoded request) in the key. Option 3 is documentation only.

## Anchors at fe9e84c (Manager)

Anchor in `crates/sentinel/src/service.rs` moved with the `origin/main` merge (`fe9e84c`): 525 → 543. Content unchanged (`state/run2/baseline-delta.md` §3).

## Reconciliation (run 2)

**Final: CONFIRMS `F-CORE-067` (canonical) — combined Medium, 98 (E1); run-2 canonical of the 032/063 pair.** Same unconditional `INSERT`; run 1 measured the duplicates on the real binary. This file's contributions carried into the canonical: a `request`-keyed dedup is unsound for `ApproveToken`, the key must include the block hash; run 1's contribution: the key's lifecycle depends on `F-CORE-063`. This file's Low is recorded; Medium is carried because `F-VAL-065`'s replayed `Sign` is not a revert and the duplicate `Commit` sits on `F2-CORE-030`'s slash path (`state/run2/reconciliation/core.md` §1.1, §4.2).
