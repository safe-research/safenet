# F3-CORE-004 A reorg deeper than `max_reorg_depth` is fatal while running but silently accepted across a restart, because snapshots carry no block hash

| Field | Value |
| --- | --- |
| Status | Final: Accepted |
| Crate and module | core, index/blocks.rs (with state/storage.rs) |
| Location | crates/core/src/index/blocks.rs:309-320, 383-394 (related: blocks.rs:185-190, 490-494; crates/core/src/state/storage.rs:50-55; crates/core/src/driver.rs:213-217) |
| Severity | Low / Low |
| Certainty | 92% (CA3: evidence checked) |
| Assumptions involved | A4, A5 |
| Tags | reorg, crash-consistency |

## Claim

While running, the block watcher keeps the hash of its reorg-safe anchor and stops the service with `ExceededMaxReorgDepth` when a new block no longer builds on it (A5: deeper reorgs cause a deliberate exit; the error text says the watcher "cannot recover on its own"). The persisted state has no such anchor: the `snapshots` table stores only `block_number` and `state`. On startup the watcher takes whatever block the node now has at its safe height as the new anchor, queues an uncle at `indexed.safe + 1`, and replays from there. It never checks that the persisted snapshot at `indexed.safe` was built on the chain the node now serves.

So the deliberate exit lasts only until the next start. After an `ExceededMaxReorgDepth` exit, a restart continues on the new chain with the state of the abandoned branch at the anchor (and below it). The same happens without any exit when the deep reorg occurs while the service is stopped. In both cases the restored state keeps events that are no longer canonical and never applies the events of the replacement blocks at or below the anchor, and nothing is logged.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Snapshots carry no block hash | E2 | crates/core/src/state/storage.rs:51-54 | `"CREATE TABLE IF NOT EXISTS snapshots (`<br>`block_number INTEGER PRIMARY KEY,`<br>`state        TEXT    NOT NULL`<br>`)",` |
| 2 | On resume the watcher trusts the snapshot at `indexed.safe` and replays only from `indexed.safe + 1` | E2 | crates/core/src/index/blocks.rs:315-320 | `let uncle = indexed.safe.checked_add(1);`<br>`if let Some(uncle) = uncle`<br>`&& uncle <= indexed.latest`<br>`{`<br>`self.queue.push_back(BlockUpdate::Uncle { number: uncle });` |
| 3 | The new anchor is simply the node's current block at `latest - max_reorg_depth`, unrelated to the persisted state | E2 | crates/core/src/index/blocks.rs:387-394 | `let safe = self`<br>`.recent`<br>`.pop_front()`<br>`.expect("the range scan always includes the safe block");`<br>`self.safe = SafeBlock {`<br>`number: safe.number,`<br>`hash: safe.hash,` |
| 4 | Live, a mismatch with the anchor is a fatal error | E2 | crates/core/src/index/blocks.rs:490-494; crates/core/src/driver.rs:213-217 | `if self.recent.is_empty() && self.safe.hash != block.parent_hash {` … `return Err(Error::ExceededMaxReorgDepth(self.config.max_reorg_depth));`; `err @ index::Error::Blocks(index::blocks::Error::ExceededMaxReorgDepth(_)),`<br>`) => {`<br>`return Err(err);` |
| 5 | The error is documented as unrecoverable by the watcher itself | E2 | crates/core/src/index/blocks.rs:185-188 | `/// A reorg unwound every block being tracked for reorg detection without`<br>`/// finding a common ancestor, meaning it went deeper than the configured`<br>``/// `max_reorg_depth` and a block already assumed final was replaced. The``<br>`/// watcher can no longer verify the chain and cannot recover on its own.` |
| 6 | Each uncle on the way to the fatal error deletes the snapshots above it, so after the exit the store holds the anchor snapshot from the abandoned branch as its only resume point | E2 | crates/core/src/state/storage.rs:130-139 | `sqlx::query("DELETE FROM snapshots WHERE block_number >= ?")` … `.bind(i64::try_from(parent)?)` |

## Trigger

Gnosis, default `max_reorg_depth` 5.

Path A (restart after the deliberate exit):

1. The service has processed up to block `S + 5`; its anchor is `S`, `recent` holds `S + 1 ..= S + 5`, snapshots `S ..= S + 5`.
2. A reorg replaces blocks from `S` upward (6 deep). The watcher emits `Uncle { S + 5 }` down to `Uncle { S + 1 }`, each restoring the snapshot below it, so only the snapshot at `S` remains (Basis 6). The replacement block `S + 1'` does not build on `S`: `ExceededMaxReorgDepth`, the service exits (Basis 4).
3. The operator or a process manager restarts it. `indexed = { safe: S, latest: S }`, so no uncle is queued; the watcher anchors on the node's current block at its safe height and replays from `S + 1` on the new chain (Basis 2, 3). The state still contains the events of the abandoned block `S` (and of any replaced block below it), and the events of the replacement block `S'` are never applied. No error is raised.

Path B (deep reorg while stopped): the service stops with snapshots `n - 5 ..= n`; while it is down a reorg replaces blocks from `n - 7`. On restart it restores the snapshot at `n - 5` (abandoned branch) and replays `n - 4` onwards from the new chain. Blocks `n - 7 ..= n - 5` of the new chain are never applied.

A node that served a minority fork for more than 5 blocks and then resynced produces the same view (A4: eventually correct, not malicious).

## Considered and rejected

- The design accepts that deep reorgs break ceremonies (`docs/overview.md:83`) and the secret store prevents nonce reuse across replays (`docs/overview.md:65-68`), so the impact is a diverged node state, not key exposure. That bounds the severity; it does not make the silent continuation consistent with A5.
- The known item about pruned key-gen secrets after a restart (`settled.md` section 2) concerns reorgs within `max_reorg_depth`; this is about reorgs beyond it.
- Pre-existing at run 2's commit `fe9e84c`: the snapshot schema (`crates/core/src/state/storage.rs` is unchanged since then) and the resume logic (`git show fe9e84c:crates/core/src/index/blocks.rs`, lines 255-278) are the same.

## Remediation options

1. Store the block hash with each snapshot (or at least with the oldest retained one). On resume, fetch the node's block at `indexed.safe` and fail with the same `ExceededMaxReorgDepth` (or a dedicated error) when the hash differs. Aegis nodes start fresh, so the schema change needs no migration.
2. At minimum, document that a node stopped by `ExceededMaxReorgDepth` must not simply be restarted, and what the operator should do instead.

Tests to add: resume with a persisted hash at `indexed.safe` that differs from the mocked node's block at that height, asserting a startup error.

## Trail

- 2026-10-08 Reviewer R1: drafted, self-estimate 70% (code path certain; depends on a reorg deeper than 5 blocks on Gnosis, which is rare).

## Critic (C3-CORE, 2026-10-08)

| Row | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | `crates/core/src/state/storage.rs:51-54`. |
| 2 | Supported | `crates/core/src/index/blocks.rs:315-320`. |
| 3 | Supported | `blocks.rs:387-394`. |
| 4 | Supported | `blocks.rs:490-494`; `crates/core/src/driver.rs:213-217`. |
| 5 | Supported | `blocks.rs:185-190`. |
| 6 | Supported | `storage.rs:130-139`. Traced: each `Uncle { n }` while the machine is `BlockPending` or `BlockApplied` restores `n - 1` (`crates/core/src/state/mod.rs:187-195`), so after `Uncle { S + 1 }` only snapshot `S` remains, and the restart sees `indexed = { safe: S, latest: S }`. |

Finding verdict: Confirmed. Severity: Low to Low. Certainty: 72, band 70 to 89 (E2: mechanism and trigger traced end to end; a reorg deeper than 5 blocks on Gnosis is rare, which the severity carries). Duplicate: canonical. Lens: invariant (INV-C5 is enforced at the live site `blocks.rs:490-494` and missing at the restart site `blocks.rs:309-343`) and asymmetry (the same condition is fatal in one path and silent in its twin).

Confirmation. Path A is the natural sequence after the deliberate exit. Nothing in `docs/` tells an operator what to do after `ExceededMaxReorgDepth` (`grep -rn -i reorg docs/` finds only the design notes at `docs/overview.md:63-83` and the secret retention text at `docs/validator-handbook.md:95`), and wiping the database is costly because the validator's `KeyShare` lives in the snapshot JSON (`state/run3/map.md` section 1.4). So a plain restart is what an operator or a process manager does, and it continues silently on the abandoned branch's state. Severity stays Low: `docs/overview.md:83` already says Safenet needs a chain without deep reorgs, and the secret store keeps nonces single-use across the replay (`docs/overview.md:65-68`), so the cost is a diverged node, not key exposure. Related, not a duplicate: R3 observation O9 (on the first status after a restart the queue prunes at the new safe block before it unmarks, `crates/core/src/tx/mod.rs:183-200`) is the queue-side twin.

QA recipe. Extend the R2 harness (the local run folder's `scratch/R2/poc`). Path B: run 1 indexes chain A (hashes `keccak256("A{n}")`) for blocks 1000 to 1006 through the real `StateMachine`, leaving snapshots 1001 to 1006; run 2 restarts the real `Driver` against a mocked RPC that serves chain B (hashes `keccak256("B{n}")`, parents chained within B) for every block from 1000 up, head 1012. Expected at `361aaf3`: no error, `Driver::run` keeps running, snapshot 1001 (chain A) is the base for chain B from 1002 on. Path A: run the real `Driver` live over chain A to 1006, then serve chain B from block 1001 up; expected `Uncle` 1006 down to 1002, then `ExceededMaxReorgDepth` and the run ends; restart against chain B: it continues without error. Bite check: storing the anchor's hash with the snapshot (remediation 1) makes both restarts fail at startup.

## QA (QA3-CORE, 2026-10-08)

Result: Reproduced.

Route: the real `Driver` (real time, 1 s blocks, default `max_reorg_depth` 5) follows chain A over an RPC mocked at the transport boundary; every block carries one watched `Ping` tagged with its chain, so the persisted state shows which branch each applied event came from. Path B: run 1 stops with snapshots 1002 to 1007; while it is down, chain B replaces every block from 1000 (8 deep) and runs 3 blocks ahead; run 2 starts without error and keeps running, and its state holds A's events for blocks 1000 to 1002 while B's events for those blocks are never applied (B's start at 1003). Path A: B replaces 6 blocks live; run 1 ends on its own (`ExceededMaxReorgDepth`) with only snapshot 1003 left; run 2 starts without error and continues on B, A's event for 1003 kept and B's never applied. Neither restart logs anything about the reorg.

Command and output: `poc/F3-CORE-004/` (`deep-reorg-tests-at-361aaf3.txt`).

Bite check: remediation 1 as a sketch (a `block_hash` column filled for live blocks, and `Driver::new` comparing the node's block at the oldest snapshot against it): both restarts fail at startup with `ExceededMaxReorgDepth(5)`; a restart on the unchanged chain still resumes (control); the other 151 core tests pass. Sound. A complete version also needs a hash for snapshots that close a warp page (the block watcher already holds the warp end's hash as its safe anchor at startup); the sketch skips the check when the oldest snapshot came from a warp page.

Certainty: 92 (E1 for both paths, Critic Confirmed).

## Practicality (PC3, 2026-10-09)

| Field | Value |
| --- | --- |
| Actor | Nature or the operator's own node: a Gnosis reorg deeper than `max_reorg_depth` (5), or a node that followed a minority fork for more than 5 blocks and then resynced (A4: eventually correct). No outside party can produce either |
| Cost | n/a |
| Condition | Path A: the service exits with `ExceededMaxReorgDepth` and is restarted (by hand under the documented `docker run`, which has no restart policy). Path B: the deep reorg happens while the service is down |
| Gate | Gate 1 passes (real `Driver`, both paths, QA3-CORE). Gate 3: no unprivileged trigger. Gate 4 is bounded: one node continues on a diverged state (events of abandoned blocks kept, replacement blocks at or below the anchor never applied), so that validator can disagree with its peers on DKG or signing state; the secret store keeps nonces single-use, so no key exposure |
| Practicality | conditional (a reorg deeper than 5 blocks, of which none is verified on Gnosis in 294.8 days of explorer data and which needs a finality failure past about 32 slots; or a node that followed a minority fork for more than 5 blocks, which client-bug incidents can produce) |
| Recommended severity | Low (reviewer, Critic and QA agree) |
| Intended route and parameters | Real `Driver` with 1 s blocks and default `max_reorg_depth` 5 over a transport-level RPC mock that serves two chains; the deep reorg is modelled, which is unavoidable for a natural trigger |
| Cheapest settling change | None needed for the band |

Reason. Low stands. The measured Gnosis record (researcher consult, about 5.09 million slots across 2025-07 to 2025-10 and 2026-03 to 2026-10) has no verified reorg of 3 or more blocks, let alone 6, and finality after about 32 slots bounds any reorg while it holds; no finality failure is reported since the merge. The more plausible route is a node that served a minority fork and then resynced, as in client-bug incidents such as Nethermind's invalid-block bursts of 2023-10-19 (its postmortem reports no long reorgs to full nodes, https://hackmd.io/@dapplion/gnosis_oct19_postmorten). When it happens the cost is one diverged node, not key exposure. Run 1's `F-CORE-001` (ledger High) and run 2's `F2-CORE-001` are the same defect and should take this practicality; on today's Gnosis data a High for it is not supported.

Sources (researcher consult, read 2026-10-09): https://dora.gnosischain.com/slots/filtered?f&f.orphaned=2&f.missing=0&c=100, https://beaconchain.gnosischain.com/charts/blocks, finality time https://github.com/gnosischain/documentation/blob/3d857fa8cdc4239b15d72aec4d35a1bd9a5e12c0/docs/about/networks/mainnet.md#L61.

## Confidence (CA3, 2026-10-09)

| Field | Value |
| --- | --- |
| Result | Evidence holds. Final: Accepted, Low, certainty 92 |
| Evidence check | `poc/F3-CORE-004/deep-reorg-tests-at-361aaf3.txt`: both tests pass (`qa3_deep_reorg_while_stopped_is_accepted_on_restart`, `qa3_deep_reorg_exit_is_forgotten_on_restart`), `Driver::new` returns ok on each restart |
| Final severity | Low, agree with PC3: no reorg deeper than 5 blocks verified on Gnosis; see the cross-run note in the CA3 report for `F-CORE-001` |

## Reconciliation (run 3)

| Field | Value |
| --- | --- |
| Relation | CONFIRMS `F-CORE-001` (run 2's `F2-CORE-001` is folded there) |
| Canonical | `F-CORE-001` |
| Ledger values | Low (CA3's one band for the three runs' twins: a reorg deeper than `max_reorg_depth`, none verified on Gnosis; the row was High), certainty 99 (rv-core-a reproduced it on the real validator binary, `poc/F-CORE-001/run3/`) |
| Source | Ledger: [`report/RECONCILIATION.md`](../report/RECONCILIATION.md) sections 2 and 3 |
