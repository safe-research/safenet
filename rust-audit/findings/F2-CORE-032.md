# F2-CORE-032 Restart and rollback replay re-queues and re-submits already-submitted actions; the queue is neither deduplicated nor rolled back

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | safenet-core, driver.rs / state/mod.rs (tx queue interface) |
| Location | crates/core/src/driver.rs:272-290 (related: crates/core/src/state/mod.rs:182-199, 213-223; crates/core/src/index/blocks.rs:256-266; crates/core/src/tx/mod.rs:132-141; crates/core/src/tx/storage.rs:93-103; crates/sentinel/src/service.rs:226-242) |
| Severity | Low / Low |
| Certainty | 75% (Critic C2-CORE-B; reviewer self-estimate in Trail) |
| Assumptions involved | A17 |
| Tags | reorg, crash-consistency |

Audited commit: `3ec8bc5`.

## Claim

Core has no notion of "this action was already queued". After a restart the state machine is rolled back to the oldest retained snapshot and the blocks above it (up to `max_reorg_depth`, default 5) are re-applied from that state; after a reorg the canonical replacement blocks are applied from the common ancestor. In both cases the transitions re-emit the `Command::Action`s they emitted the first time. The driver encodes every action and calls `TransactionQueue::queue`, which unconditionally inserts a new row and immediately tries to submit it; there is no comparison with existing rows and `expires_at` cannot suppress a duplicate that is allocated a nonce in the same call. The transaction queue is also never rolled back on `Uncle`, so actions emitted by orphaned blocks stay queued and are submitted against the canonical chain as well.

The result is one duplicate on-chain transaction per action emitted in the replayed window, on every restart and every reorg. On-chain the duplicate reverts (e.g. a second `commit` fails `checkNotCommitted`, `contracts/src/libraries/SentinelOracleCommitments.sol:95-96`), so the direct cost is the gas of a reverted call plus the transaction's slot in `max_in_flight_transactions` (16) until it is mined - a small, bounded fee loss and a short head-of-line delay for genuine actions queued behind the duplicates. Whether any replayed action is _not_ rejected on-chain (and therefore executes twice) is a per-contract question for R4/R5/R7; core offers no protection either way.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Every `Command::Action` from a transition is encoded and queued; nothing checks whether it was queued before. | E2 | crates/core/src/driver.rs:272-290 | `for command in commands { match command { state::Command::Action(action) => { transactions.push(self.actions.encode_action(action)); }` ... `if !transactions.is_empty() { let result = self.transactions.queue(transactions).await;` |
| 2 | `queue` inserts and immediately submits, so `expires_at` never gets a chance to drop a duplicate. | E2 | crates/core/src/tx/mod.rs:132-141 | `pub async fn queue(&mut self, transactions: impl IntoIterator<Item = (Transaction, Option<u64>)>) -> Result<(), Error> {` `self.storage.enqueue(transactions).await?;` `if let Some(status) = self.block_status { self.submit_pending(status.latest).await?; }` |
| 3 | Enqueue is an unconditional insert. | E2 | crates/core/src/tx/storage.rs:93-103 | `sqlx::query("INSERT INTO transactions (request, expires_at) VALUES (?, ?)")` `.bind(request)` `.bind(expires_at.map(i64::try_from).transpose()?)` |
| 4 | Rollback restores the parent snapshot; replayed blocks re-run the transitions and re-emit their commands. | E2 | crates/core/src/state/mod.rs:182-189, 213-223 | `let (_, state) = self.snapshots.reorg(number).await?;` ... `for log in logs { let (new_state, new_commands) = self.transition.apply_transition(state, Message::Event(log)); state = new_state; commands.extend(new_commands); }` |
| 5 | Every restart with more than one retained snapshot rolls back to the oldest one and replays what follows. | E2 | crates/core/src/index/blocks.rs:256-266 | `// The earliest retained snapshot is the rollback anchor. Replay` `// everything after it, but only emit an uncle when there are newer` `// snapshots to discard.` ... `self.queue.push_back(BlockUpdate::Uncle { number: uncle });` |
| 6 | Nothing in the state machine or driver removes queued transactions on `Uncle` (the only queue calls are `update_block_status` and `queue`). | E2 | crates/core/src/driver.rs:249, 283 | `let result = self.transactions.update_block_status(block_status).await;` ... `let result = self.transactions.queue(transactions).await;` |
| 7 | Example of a replayed action pair (sentinel): approve + commit, both with the commit deadline as expiry. | E2 | crates/sentinel/src/service.rs:226-242 | `SentinelAction { kind: SentinelActionKind::ApproveToken { bond: U256::from(bond_target), }, expires_at: Some(commit_deadline), }.into(), SentinelAction { kind: SentinelActionKind::Commit { id: request_id, hash, }, expires_at: Some(commit_deadline), }.into(),` |
| 8 | The duplicate commit reverts on-chain (so the cost is gas, not a second bond). | E2 | contracts/src/libraries/SentinelOracleCommitments.sol:95-96 | `function add(T storage self, bytes32 requestId, address sentinel, bytes32 commitHash, uint96 bondAmount) internal {` `checkNotCommitted(self, requestId, sentinel);` |
| 9 | Some validator actions carry no expiry at all and can therefore never be dropped once queued. | E2 | crates/validator/src/service/action.rs:252-255, 377 | `// Nonce registration doesn't carry an expiry - we cannot` `// reliably know for how long it is valuable.` `None,` ... `None,` |

## Trigger

Sentinel with defaults, requests flowing. Restart while `latest == L`: on start `Uncle{L-4}` restores snapshot `L-5`; blocks `L-4..L` are re-applied (as `New` if the node has not advanced, otherwise via `Warp` for the reorg-safe part). Every `TransactionProposed`/`NewRequest` in that window re-creates its entry, re-spawns the engine check, and on resume queues `ApproveToken` + `Commit` again although the first `Commit` is already mined; the duplicate `commit` reverts. Observable as two `transactions` rows with the same `request` JSON and different nonces, and a reverted transaction from the sentinel's account per replayed request. A one-block reorg produces the same pattern for the actions of the uncled block (queued from the orphaned view, then queued again from the canonical replay).

## Considered and rejected

- "`expires_at` bounds the duplicates" - it only prevents _allocation_ of rows that have not yet been allocated when a later block arrives; `queue` allocates and submits in the same call (row 2), so a replayed action is on the wire before any expiry check unless `max_in_flight_transactions` is exhausted.
- "The design accepts this" - the replay itself is documented (`blocks.rs:256-260`) but neither the queue nor the driver documents or mitigates the duplicate submissions; there is no `known` item for it (`codebase-map.md` section 4).
- "Same-nonce retries make this safe" - those cover the crash-between-broadcast-and-record case inside the queue (R3), not two distinct rows created by two runs of the transition.
- "Higher severity because a duplicate could execute twice" - not shown for any traced action (row 8); left to the per-contract reviewers.

## Remediation options

1. Deduplicate on enqueue: give `Transaction` a deterministic key (hash of `to`, `data`, `value` plus the emitting block/log index from the state machine) and skip inserting when a non-executed row with the same key exists. Tradeoff: legitimately repeated identical actions (rare; e.g. `ApproveToken` for the same bond) would need a distinguishing nonce in the key.
2. Persist, with each snapshot, the set of action keys emitted at that block, and have the driver drop replayed actions whose key is in a snapshot at or above the restored block. Precise, but more state per block.
3. Roll the queue back on `Uncle`: delete queued-but-unallocated rows created by the orphaned blocks (allocated rows must stay because their nonce may already be in the mempool). Only addresses the reorg half.

Tests to add: a `tx` integration test that enqueues the same `(Transaction, expires_at)` twice across a simulated restart and asserts a single allocation; a driver-level test (once `driver.rs` has a mock harness) that replays a block after `Uncle` and asserts no second submission for the same action.

## Trail

- Reviewer R2: drafted, self-estimate 65% (E2 for the mechanism; the on-chain outcome traced for one action only). Severity Low: bounded gas loss and transient in-flight pressure; would rise to Medium if a reviewer finds a replayed action that the contract accepts twice, or if the in-flight cap is routinely exhausted by duplicates.
- Critic C2-CORE-B: Confirmed, 75%, severity Low (reviewer Low). Same defect as F2-CORE-063, which is canonical.

## Critic (C2-CORE-B)

Method: read title and Location only, traced the driver's dispatch, `TransactionQueue::queue`/`enqueue` and the rollback/replay path myself, then compared.

| # | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | driver.rs:272-290. |
| 2 | Supported | tx/mod.rs:132-141. |
| 3 | Supported | tx/storage.rs:93-103. |
| 4 | Supported | state/mod.rs:182-189, 213-223. |
| 5 | Supported | index/blocks.rs:261-266. |
| 6 | Supported | driver.rs:249 and 283 are the only queue calls; nothing in `state/` touches the queue. |
| 7 | Supported | sentinel service.rs:226-242. |
| 8 | Supported | SentinelOracleCommitments.sol:91-96 re-opened: `add` calls `checkNotCommitted`, which requires `commitHash == 0` else `AlreadyCommitted()`. |
| 9 | Supported | validator service/action.rs:252-254 and 377 (`None,`). |

Finding verdict: **Confirmed**. Certainty **75%** (E2; on-chain outcome traced for `commit` only, as the reviewer says). Severity **Low / Low**.

**One defect seen from two files.** F2-CORE-063 (R3) describes the same missing idempotency from the queue side. **F2-CORE-063 is canonical**: the fix belongs in `TransactionStorage::enqueue`, and R3's observation that the queue's retention window (`executed_at <= safe` pruned) equals the replay window is what makes a dedup exact. Two points from this file should travel with the merge: (i) the queue is never rolled back on `Uncle`, so actions from orphaned blocks stay queued alongside their canonical replays; (ii) the traced contract revert (row 8). F2-SEN-007 (R7) is the sentinel's own statement of the same duplicates.

Correction shared with F2-CORE-063: the reviewer's caveat on remediation 1 is right and stronger than stated — `ApproveToken` encodes `approve(oracle, bond)` with no request id (service.rs:226-233) and a `commit` consumes the allowance, so a `request`-keyed dedup would drop a legitimately repeated approval and make the following `Commit` fail; key on the emitting (block, log index) instead (remediation 2 of either finding).

Interaction: in F2-CORE-030's replay variant the replayed duplicate `Commit` is what leaves `self_committed = false`; the two findings are distinct but compound.

## Reconciliation (run 2)

**Final: CONFIRMS `F-CORE-067` (canonical) — combined Medium, 98 (E1); run-2 duplicate of `F2-CORE-063`.** Two points from this file travel with the merge: the queue is never rolled back on `Uncle`, and the traced `AlreadyCommitted` revert (now `SentinelOracleCommitments.sol:94`, `vote == NONE`). Run 1 measured the duplicates live (nonces 2 and 3, `0xbfec5558`) (`state/run2/reconciliation/core.md` §1, §4.2).
