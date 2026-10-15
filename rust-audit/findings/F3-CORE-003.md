# F3-CORE-003 Restarting against an RPC node whose head is below the oldest retained snapshot makes the first block update a `BadUpdate`, and the service exits

| Field | Value |
| --- | --- |
| Status | Final: duplicate of F3-CORE-030 (follows its rating) |
| Crate and module | core, index/blocks.rs (with state/mod.rs) |
| Location | crates/core/src/index/blocks.rs:298-320, 399-420 (related: crates/core/src/state/mod.rs:140-148, 187-199, 272; crates/core/src/driver.rs:195-198) |
| Severity | Low / follows F3-CORE-030 |
| Certainty | n/a (duplicate; see F3-CORE-030) |
| Assumptions involved | A4 |
| Tags | liveness, crash-consistency |

## Claim

On resume, `BlockWatcher::initialize` takes the RPC node's current head as `latest` and queues a synthetic `Uncle { indexed.safe + 1 }`, then `New` updates only for scanned blocks at or above `indexed.safe + 1`. It never checks that the node has reached the persisted snapshots. When the node's head is below the oldest retained snapshot (`latest < indexed.safe`), no `New` update is queued, the block watcher continues from `latest + 1`, and the state machine, which the uncle left at `BlockPending { pending: indexed.safe + 1 }`, rejects `New { latest + 1 }` as a `BadUpdate`. The machine is poisoned and the driver exits. Every restart fails the same way until the node passes `indexed.safe`.

A node that is behind is within A4 ("may be stale"). During live operation the same node lag is harmless: the block watcher just waits for the missing block. Only the restart path turns it into a fatal error, and the log says only "bad update".

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The watcher's view starts at the node's current head | E2 | crates/core/src/index/blocks.rs:299-300, 307 | `let latest = self.require_block(BlockId::latest()).await?;`<br>`let safe = latest.number.saturating_sub(self.config.max_reorg_depth);`; `self.update_next_pending_block(latest.number, latest.timestamp);` |
| 2 | On resume an uncle at `indexed.safe + 1` is queued whenever newer snapshots exist; nothing compares `indexed` with `latest` | E2 | crates/core/src/index/blocks.rs:315-320 | `let uncle = indexed.safe.checked_add(1);`<br>`if let Some(uncle) = uncle`<br>`&& uncle <= indexed.latest`<br>`{`<br>`self.queue.push_back(BlockUpdate::Uncle { number: uncle });` |
| 3 | `New` updates are queued only for scanned blocks at or above `indexed.safe + 1`, so none when the node's head is lower | E2 | crates/core/src/index/blocks.rs:399-411 | `for block in self.recent.iter().filter(\|block\| {` … `\|indexed\| {`<br>`indexed`<br>`.safe`<br>`.checked_add(1)`<br>`.is_some_and(\|from\| block.number >= from)` |
| 4 | The next live block is `latest + 1` | E2 | crates/core/src/index/blocks.rs:595-599 | `self.pending = PendingBlock {`<br>`number: number + 1,` |
| 5 | `indexed` is the range of persisted snapshots | E2 | crates/core/src/state/storage.rs:86-90 | `pub async fn status(&self) -> Result<Option<BlockStatus>, Error> {`<br>`let (safe, latest) = sqlx::query_as::<_, (Option<i64>, Option<i64>)>(`<br>`"SELECT MIN(block_number), MAX(block_number) FROM snapshots",` |
| 6 | The resumed machine expects `latest snapshot + 1`; the uncle moves it to `BlockPending { pending: number }` | E2 | crates/core/src/state/mod.rs:143-145, 187-193 | `let pending = latest.checked_add(1).ok_or(Error::EndOfChain)?;`<br>`Ok((state, Status::BlockPending { pending }))`; `Update::Block(BlockUpdate::Uncle { number })` … `let status = Status::BlockPending { pending: number };` |
| 7 | A `New` block is accepted only when its number equals `pending`; anything else is `BadUpdate` | E2 | crates/core/src/state/mod.rs:196-199, 272 | `Update::Block(BlockUpdate::New { number, .. })`<br>`if matches!(status, Status::Initialized)`<br>`\|\| matches!(status, Status::BlockPending { pending } if pending == number)` … `_ => return Err(Error::BadUpdate),` |
| 8 | An error leaves the machine poisoned (the state was taken out and is not put back) and the driver exits | E2 | crates/core/src/state/mod.rs:176, 274; crates/core/src/driver.rs:195-198 | `let (state, status) = mem::take(&mut *lock).ok_or(Error::Poisoned)?;`; `*lock = Some((state, status));`; `if let Err(err) = result {`<br>`tracing::error!(?err, "unrecoverable driver error; exiting");`<br>`break;` |

## Trigger

With the default `max_reorg_depth` 5 on Gnosis (5 s blocks):

1. The service processes blocks live up to block `n`. Each new block moves the watcher's safe block to `n - 5` and the driver prunes snapshots below it, so the store holds snapshots `n - 5 ..= n` (`crates/core/src/driver.rs:242`, `:263`; `crates/core/src/state/storage.rs:151-161`).
2. The operator restarts it against a node whose head is block `n - 10`, 50 s behind: for example a backup endpoint after a failover, or the same node restarted from a snapshot and still catching up.
3. `initialize` sees `latest = n - 10`, queues `Uncle { n - 4 }` and no `Warp` (`n - 4 > n - 15`), scans `n - 15 ..= n - 10`, queues no `New` (all below `n - 4`), and sets the next block to `n - 9`.
4. The state machine applies the uncle, restoring the snapshot at `n - 5` and expecting block `n - 4`.
5. The block watcher returns `New { n - 9 }`. `handle_update` returns `BadUpdate`, the driver logs "unrecoverable driver error; exiting" and stops. Because of the known exit status 0 (`settled.md` section 2), an on-failure restart policy does not bring it back; a manual restart fails again until the node reaches `n - 5`.

The condition is the node's head being at least 6 blocks (30 s) behind the last block the service processed.

## Considered and rejected

- A node behind by 5 blocks or fewer is fine: the first `New` is then `indexed.safe + 1` (from the scan or from the live poll) and matches `pending`.
- A long downtime is fine: the node is then ahead and the warp covers the gap.
- Before exiting, the driver already passed the stale chain view to the transaction queue (`crates/core/src/driver.rs:242-249`); what the queue does with it belongs to the queue review.
- Pre-existing at run 2's commit `fe9e84c`: `initialize` is unchanged (`git show fe9e84c:crates/core/src/index/blocks.rs`, lines 244-289) and the `New` arm required `pending == number` (`git show fe9e84c:crates/core/src/state/mod.rs`, lines 190-192).

## Remediation options

1. In `initialize`, when `indexed.latest` (or at least `indexed.safe`) is above the node's head, wait and re-read the head until the node catches up, logging why. Small and local.
2. Alternatively fail fast with a specific error ("RPC node is behind the persisted state at block X") instead of a poisoned state machine, so operators see the cause.

Tests to add: `BlockWatcher::new` with `indexed = { safe: 1000, latest: 1005 }` against a mocked head of 990, asserting that the watcher waits (or errors clearly) instead of emitting `New { 991 }`.

## Trail

- 2026-10-08 Reviewer R1: drafted, self-estimate 75% (code path traced end to end; not executed).

## Critic (C3-CORE, 2026-10-08)

| Row | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | `crates/core/src/index/blocks.rs:299-300, 307`. |
| 2 | Supported | `blocks.rs:315-320`. |
| 3 | Supported | `blocks.rs:399-411`. |
| 4 | Supported | `blocks.rs:595-599`. |
| 5 | Supported | `crates/core/src/state/storage.rs:86-90`. |
| 6 | Supported | `crates/core/src/state/mod.rs:143-145, 187-193`. |
| 7 | Supported | `state/mod.rs:196-199, 272`. |
| 8 | Supported | `state/mod.rs:176` takes the state out and only `:274` puts it back, so every error return leaves the machine poisoned; `crates/core/src/driver.rs:195-198`. |

Finding verdict: Confirmed. Severity: Low to Medium, as the canonical. Certainty: 90, through the canonical's executed run, which this Critic re-ran (see F3-CORE-030). Duplicate: duplicate of F3-CORE-030 (same defect, same trigger; F3-CORE-030 has the executed run against the real `Driver`). Only this file adds the worked example with a node 10 blocks behind and the remark that the stale chain view reaches the transaction queue before the exit (`driver.rs:242-249`). I traced that path: it is harmless, because pruning at a lower safe block removes less and executed markers above it are only cleared and re-marked later (`crates/core/src/tx/mod.rs:183-216`).

QA recipe: see F3-CORE-030.

## Practicality (PC3, 2026-10-09)

Duplicate of `F3-CORE-030` (C3-CORE); it follows that row's practicality and recommended severity. See the PC3 practicality section in `F3-CORE-030.md`.

## Confidence (CA3, 2026-10-09)

Duplicate of `F3-CORE-030`. No separate score: this row follows `F3-CORE-030`'s final status, severity and certainty (see the CA3 confidence section there).

## Reconciliation (run 3)

| Field | Value |
| --- | --- |
| Relation | Duplicate of `F3-CORE-030` (run 3), which CONFIRMS `F2-CORE-007` |
| Canonical | `F2-CORE-007`. This file stays this run as independent evidence and follows the canonical row; listed in the ledger's fold index (section 3.1) |
| Source | Ledger: [`report/RECONCILIATION.md`](../report/RECONCILIATION.md) sections 2 and 3 |
