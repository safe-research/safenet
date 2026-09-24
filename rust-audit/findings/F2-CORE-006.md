# F2-CORE-006 `revalidate_last_block` treats a `null` header as "uncled", so a lagging backend causes a spurious rollback and replay of a canonical block

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | safenet-core, index/blocks.rs |
| Location | crates/core/src/index/blocks.rs:504-507 (related: crates/core/src/index/blocks.rs:509-534, crates/core/src/index/mod.rs:106-130, crates/core/src/state/mod.rs:182-189) |
| Severity | Low / Low |
| Certainty | 70% (Critic C2-CORE-A; reviewer self-estimate 60%) |
| Assumptions involved | A4 |
| Tags | reorg |

Audited commit: `3ec8bc5`.

## Claim

Revalidation is entered when a logs query fails with `-32001`. It re-fetches the last emitted block by number and invalidates it when the node returns a different hash _or nothing at all_. A backend that has not yet imported block n — routine behind a load balancer — answers the by-hash logs query with "resource not found" and the by-number header query with `null`; the watcher then emits `Uncle { n }`, the state machine rolls back to snapshot n-1 (deleting snapshot n), `pending` is rewound to n, and the next poll re-fetches n — normally the identical block — which is re-emitted as `New { n }` with the same hash and its logs re-applied. Everything derived from block n runs twice: effects (permitted by contract, but real work), actions re-enqueued into the transaction queue (no deduplication — R3 seam), and `uncled_blocks_total` is incremented for a reorg that did not happen.

Elsewhere in the same file a `null` header for a block at or below the head means "not yet available" (retry with the configured delays) or "inconsistent node" (`MissingBlock`); only revalidation reads it as a reorg. The existing test `revalidate_invalidates_a_block_missing_from_rpc` pins the behaviour as intentional.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | `None` is treated the same as a different hash | E2 | crates/core/src/index/blocks.rs:504-507 | `let current = self.get_block(BlockId::number(last.number)).await?;` / `if current.map(\|block\| block.hash) == Some(last.hash) {` / `return Ok(None);` / `}` |
| 2 | Invalidation truncates `recent`, rewinds `pending`, replaces the queue | E2 | crates/core/src/index/blocks.rs:521-532 | `self.recent.truncate(last_index);` / `self.pending = PendingBlock {` / `number: invalidated.number,` / `timestamp_ms: timestamp * 1000,` / `};` ... `self.queue.clear();` / `self.queue.push_back(BlockUpdate::Uncle {` |
| 3 | The uncle is delivered to the state machine which rolls back to n-1 | E2 | crates/core/src/index/mod.rs:113-122; crates/core/src/state/mod.rs:182-188 | `Some(invalidated) => {` ... `self.events.on_block_invalidated(invalidated.hash)?;` / `Ok(None)` ... `let (_, state) = self.snapshots.reorg(number).await?;` / `let status = Status::BlockPending { pending: number };` |
| 4 | The same block is then re-fetched and re-emitted (parent still matches) | E2 | crates/core/src/index/blocks.rs:393-397, 421-423, 445-447 | `if let Some(block) = self.get_block(BlockId::number(self.pending.number)).await? {` / `break block;` ... `.pop_back_if(\|last\| last.hash != block.parent_hash)` ... `self.recent.push_back(block);` |
| 5 | The behaviour is pinned by a test | E1 | crates/core/src/index/blocks.rs:1120-1131 (run this session, passes) | `asserter.push_success::<Option<Block>>(&None);` / `assert_eq!(` / `blocks.revalidate_last_block().await.unwrap(),` / `Some(invalidated_block(&block(1000)))` / `);` |
| 6 | Contrast: `null` for the pending block means "retry" | E2 | crates/core/src/index/blocks.rs:399-403 | `// While we wait around the expected block time, the block is likely` / `// available now or shortly after, so retry with the decreasing` / `// \`block_retry_delays\`.` |
| 7 | Contrast: `null` during initialization means an inconsistent node | E2 | crates/core/src/index/blocks.rs:127-130, 240-242 | `/// A block at or below the chain head was missing, indicating an` / `/// inconsistent RPC node.` ... `self.get_block(id).await?.ok_or(Error::MissingBlock(id))` |

## Trigger

Load-balanced RPC URL. Backend A serves `eth_getBlockByNumber(n)` -> `New { n, h_n }`. Backend B (behind) serves `eth_getLogs { blockHash: h_n }` -> `-32001`, then `eth_getBlockByNumber(n)` -> `null`. Watcher emits `Uncle { n }`; state rolls back to n-1; next poll (any backend) returns h_n again; `New { n, h_n }` and its logs are re-applied. Mock: after init, `push_failure(-32001)`, `push_success(None)`, then `push_success(block(n))`; expect `Uncle { n }` followed by `New { n }` with the same hash.

## Considered and rejected

- _Correctness is at risk._ No: rollback plus replay is deterministic, so the state converges; the cost is duplicated side effects, gas, and misleading metrics — hence Low.
- _Reth requires the `null` branch._ The doc comment describes Reth as "briefly observing a block, exposing its hash, and then losing the ability to serve logs for it"; that node still answers the header query with a _different_ hash once the reorg is applied. A `null` header is a different condition and can be retried a bounded number of times first.

## Remediation options

1. In `revalidate_last_block`, return `Ok(None)` (retry) when the header is `null`; only a _different_ hash invalidates. Optionally invalidate after K consecutive `null`s to keep the Reth recovery.
2. Log the `null` case at warn with the block number so operators can see backend inconsistency.

Tests to add: the mock above asserting no `Uncle` on a `null` header followed by the same block.

## Trail

- Reviewer R1: drafted, self-estimate 60%. Confirms lead CORE-H9 as Low. Seams: duplicate action submission (R3, CORE-H5), effect re-execution contract (R2).

## Critic (C2-CORE-A)

Independent read of `crates/core/src/index/blocks.rs:483-535`: `current.map(|block| block.hash) == Some(last.hash)` is `false` for `None`, so a `null` header invalidates exactly like a different hash; `recent.truncate`, the `pending` rewind and the queue replacement follow; `next_logs` forwards the `Uncle` and the state machine calls `snapshots.reorg(n)` (`crates/core/src/state/mod.rs:182-188`), which succeeds because snapshot `n-1` exists (committed on the previous `Logs`, retained by `prune` since `n-1 >= safe`). The following `next()` re-fetches `n`, whose parent still matches `recent.back()` (`n-1`), and re-emits `New{n}` with the same hash. Matches the reviewer: a spurious, deterministic rollback-and-replay.

Per-claim verdicts: 1-7 **Supported** (claim 5's test `revalidate_invalidates_a_block_missing_from_rpc` re-run this session, passes; claim 7's doc comment at 127-130 and `require_block` at 240-242 verified).

Trigger realism: requires an RPC that answers the by-hash logs query with -32001 and the by-number header query with `null` - a backend that has not imported `n` behind a load balancer does both (Reth maps unknown blocks to -32001, the very code the recovery keys on). That is an A4 stale-RPC condition, not attacker input. Consequence is bounded: the state converges, but the `NewBlock(n)` transition and block `n`'s events run twice, re-emitting their actions into a transaction queue that does not deduplicate (R3 seam) and re-running effects (permitted by the `Command::Effect` contract, `state/mod.rs:60-62, 68-71`).

Verdict: **Confirmed**. Certainty **70** (E2; not executed end-to-end). Severity **Low / Low**. Remediation 1 is sound: a `null` header is not evidence of a reorg, and the Reth case the doc comment describes (a _different_ hash) is unaffected; keep a bounded-`null` invalidation so the fix does not create a new stall. A16/A17 not applicable.

## Reconciliation (run 2)

**Final: CONFIRMS `F-CORE-003` (canonical) — combined Medium, 70 (E2).** Identical mechanism and mock; run 1 rated Medium, this file Low. Medium is carried (routine A4 trigger; each spurious uncle re-emits actions into an undeduplicated queue, a cost `F-CORE-067` measured); this file's Low is recorded (`state/run2/reconciliation/core.md` §1).
