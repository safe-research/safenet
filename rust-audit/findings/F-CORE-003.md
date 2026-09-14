# F-CORE-003 A lagging RPC backend that answers `null` for a block it has not imported is treated as a reorg, producing a spurious uncle, a state rollback and a full replay

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | core, `index/blocks.rs` (entered from `index/mod.rs`) |
| Location | `crates/core/src/index/blocks.rs:483-535` (specifically `504-507`; related: `crates/core/src/index/mod.rs:106-130`) |
| Severity | Medium / Medium |
| Assumptions involved | A4, A5 |
| Certainty | 70% |
| Tags | reorg, input-validation, crash-consistency |

## Claim

`BlockWatcher::revalidate_last_block` conflates two different node answers. It asks the node for the last emitted block _by number_ and treats **both** "the node returns a different hash" (a real reorg) and "the node returns `null`" (the node does not have that block right now) as proof that the block was uncled. The `null` case is folded in by `current.map(|block| block.hash) == Some(last.hash)`: when `current` is `None` the comparison is `None == Some(h)`, which is false, so control falls through into the invalidation path.

Under assumption A4 the RPC is not malicious but _may be stale_, and a `null` answer for a block the watcher itself observed a moment earlier is precisely what a lagging or load-balanced endpoint returns. The result is a fabricated reorg: `Uncle { n }` is emitted, the state machine deletes the snapshot at `n` and rolls the service back to `n-1` (`crates/core/src/state/mod.rs:182-189`), the block is re-fetched, and — since the block was never actually uncled — the _same_ block `n` is re-emitted and every one of its events is applied a second time. `safenet_core_uncled_blocks_total` is incremented for an event that did not occur, so the one metric an operator would use to detect reorg pressure is polluted by RPC flakiness.

Replay is documented as tolerable (`effects.rs:21-24`), but it is not free: replayed events re-queue their actions, and the transaction queue inserts unconditionally with a fresh nonce (`crates/core/src/tx/storage.rs:96-100`), so each spurious uncle costs one duplicate on-chain transaction per action produced in that block, plus one re-execution of every effect the block triggered (for the sentinel, a duplicate engine check per `TransactionProposed`).

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | A `None` (JSON `null`) response takes the invalidation path exactly like a hash mismatch. | E2 | `crates/core/src/index/blocks.rs:504-507` | <pre>let current = self.get_block(BlockId::number(last.number)).await?;<br>if current.map(&#124;block&#124; block.hash) == Some(last.hash) {<br> return Ok(None);<br>}</pre> |
| 2 | The invalidation path unconditionally rewinds, clears the queue, counts an uncle and queues `Uncle`. | E2 | `crates/core/src/index/blocks.rs:520-532` | <pre>metrics::uncled_blocks_total.increment(1);<br>let timestamp = last.timestamp;<br>self.recent.truncate(last_index);<br>self.pending = PendingBlock {<br> number: invalidated.number,<br> timestamp_ms: timestamp * 1000,<br>};<br><br>// Clear the queue and insert the uncle update.<br>self.queue.clear;<br>self.queue.push_back(BlockUpdate::Uncle {<br> number: invalidated.number,<br>});</pre> |
| 3 | The behaviour is deliberate and encoded in a test, so it is a design decision rather than an oversight — which is what makes it worth flagging rather than silently fixing. | E2 | `crates/core/src/index/blocks.rs:1120-1131` | <pre>async fn revalidate_invalidates_a_block_missing_from_rpc {<br> let asserter = Asserter::new;<br> let mut blocks = initialized_watcher_skip_ready(&asserter, config).await;<br><br> asserter.push_success::<Option<Block>>(&None);<br><br> assert_eq!(<br> blocks.revalidate_last_block.await.unwrap,<br> Some(invalidated_block(&block(1000)))<br> );<br>}</pre> |
| 4 | Revalidation is entered only from a JSON-RPC "resource not found" on the logs query — the same symptom a not-yet-imported block produces on a different request. | E2 | `crates/core/src/index/mod.rs:109-121` | <pre>Err(err) if is_resource_not_found(&err) => {<br> // Some RPC nodes will see an uncled block but not support querying<br> // logs for it (for example, Reth). Ask the `BlockWatcher` to revalidate<br> // the last block it produced and make sure it is still canonical.<br> match self.blocks.revalidate_last_block.await? {<br> // It was uncled; tell the event watcher to move on.<br> Some(invalidated) => {</pre> |
| 5 | The uncle drives a real state rollback: the snapshot at `n` and everything above it is deleted and the state is replaced by the one at `n-1`. | E2 | `crates/core/src/state/mod.rs:182-189` | <pre>Update::Block(BlockUpdate::Uncle { number })<br> if matches!(status, Status::BlockPending { pending } if number < pending)<br> |  | matches!(status, Status::BlockEvents { latest } if number <= latest) =><br>{<br> let (_, state) = self.snapshots.reorg(number).await?;<br> let status = Status::BlockPending { pending: number };<br> (state, status, vec![])<br>}</pre> |
| 6 | The replayed block's actions are inserted again with no de-duplication, so each spurious uncle costs duplicate transactions. | E2 | `crates/core/src/tx/storage.rs:96-100` | <pre>sqlx::query("INSERT INTO transactions (request, expires_at) VALUES (?, ?)")<br> .bind(request)<br> .bind(expires_at.map(i64::try_from).transpose?)<br> .execute(&mut *tx)</pre> |

## Trigger

A load-balanced RPC endpoint (one hostname, several backends with independent import progress — the normal shape of a commercial provider, and explicitly in scope under A4):

1. `BlockWatcher::next` issues `eth_getBlockByNumber(n, false)`. Backend A has imported block `n` and returns its header. `New { number: n, hash: h }` is emitted and the state machine applies `Message::NewBlock(n)`.
2. `EventWatcher::next` issues `eth_getLogs { blockHash: h, ... }`. The request is routed to backend B, which has not imported block `n` yet and answers JSON-RPC error `-32001 resource not found`.
3. `Watcher::next_logs` recognises the code and calls `revalidate_last_block` (`index/mod.rs:109-113`).
4. Revalidation issues `eth_getBlockByNumber(n, false)`. It is routed to backend B again, which still does not have block `n` and answers `null`.
5. `current` is `None`, so `current.map(..) == Some(last.hash)` is false and the watcher declares block `n` uncled: `recent` is truncated, `pending` is rewound to `n`, the queue is cleared, an `Uncle { n }` is queued and `uncled_blocks_total` is incremented.
6. `StateMachine` deletes the snapshot at `n` and restores `n-1`. On the next iteration the watcher re-fetches block `n`, gets the same hash `h` back (backend A, or backend B once it has caught up), emits `New { n, h }` again, and every event in block `n` is applied and re-actioned a second time.

The window needed is small: a backend that is one block behind for the ~500 ms between the header fetch and the logs fetch (`block_propagation_delay` defaults to 500) is enough, and the same transient produces both the `-32001` in step 2 and the `null` in step 4 because they are the same condition on the same backend.

## Considered and rejected

- **"The hash comparison protects it."** It only protects the case where the node _has_ a block at that height. The `Option` is flattened before comparison, so absence and disagreement are the same outcome (basis 1).
- **"The doc comment says this is intended."** It does — `blocks.rs:472-476` says _"or cannot find the block at all"_ — which is why this finding is about the consequence, not a misreading. The intent (recovering from Reth briefly exposing and then dropping an uncled block) is served by the hash-mismatch branch alone; the `null` branch adds a false-positive path that the intent does not require.
- **"A retry would be unsafe here."** It would not. `Ok(None)` from `revalidate_last_block` already means "still canonical, the logs just are not available yet" (`index/mod.rs:124-125`) and the caller re-raises the original error, which the driver retries. Treating `None` the same way is a strictly smaller change than treating it as an uncle, and cannot lose a genuine reorg: a genuine reorg makes the node return a _different hash_, not `null`, once the node has re-imported the height.
- **"`ExceededMaxReorgDepth` would eventually fire and make it loud."** It cannot on this path. `revalidate_last_block` never touches `self.safe` (documented at `blocks.rs:479-482`) and returns `Ok(None)` once `recent` is exhausted (`blocks.rs:498-501`), so repeated spurious invalidations degrade into the silent retry loop of F-CORE-004 rather than into the loud failure.
- **"The state machine would reject the uncle."** It accepts it: the guard at `state/mod.rs:182-184` only requires `number <= latest` / `number < pending`, which a just-emitted block satisfies (basis 5).
- **"Replay is harmless — the runtime documents effects as replayable."** Replay of _effects_ is documented as safe (`effects.rs:21-24`), but replay of _actions_ is not de-duplicated anywhere (basis 6), so the cost is real on-chain gas and one in-flight slot per duplicate.
- **Not a false positive because** every step of the trigger is a documented, in-scope RPC behaviour under A4, and the two RPC answers involved (`-32001` on `eth_getLogs`, `null` on `eth_getBlockByNumber`) are the same node state observed twice.

## Remediation options

1. **Distinguish absence from disagreement.** Match on the `Option` explicitly: `Some(block) if block.hash == last.hash => Ok(None)`; `Some(_) => invalidate`; `None => Ok(None)` (i.e. "not available yet", letting the caller re-raise and the driver retry). Tradeoff: if a node genuinely serves `null` forever for an uncled height, the indexer stalls instead of recovering — mitigate by counting consecutive `None` answers and invalidating only after a small threshold, which also keeps the Reth recovery working.
2. **Confirm against the head before invalidating.** Only treat the block as uncled when the node's `latest` is at or above `last.number` (a node that answers `null` for `n` while reporting a head below `n` is simply behind). One extra `eth_getBlockByNumber("latest")` per revalidation, which only happens on the `-32001` path.
3. **Make the false positive observable.** Emit a distinct metric/label for "invalidated because the node had no block" versus "invalidated because the hash changed", so `safenet_core_uncled_blocks_total` stops being polluted and operators can see RPC flakiness for what it is. This is worth doing regardless of option 1 or 2.

Tests to add:

- `blocks.rs`: rename/extend `revalidate_invalidates_a_block_missing_from_rpc` into two tests, one asserting the hash-mismatch invalidation and one asserting that a single `null` answer yields `Ok(None)` (or invalidates only after the configured threshold).
- `index/mod.rs`: a composition test where `-32001` is followed by `null` and then by the same block hash, asserting the watcher does not emit `Uncle` and does not re-emit `New` for the same block.

## Trail

- Reviewer R1: drafted from lead CORE-H9 (analysis confidence 45%). Raised on re-reading because the trigger is a _single_ backend condition producing both RPC answers rather than two independent failures, and because the existing test (basis 3) pins the behaviour, so the codebase will not drift away from it on its own. Self-estimate 75% for the mechanism, 60% that a real deployment hits it often enough to matter. No `E1`: read-only run.

## Critic (C-CORE-A)

Method note: I derived `blocks.rs:483-535` before reading the Claim and independently flagged line 505 — `if current.map(|block| block.hash) == Some(last.hash)` — as conflating "absent" with "different", because `None == Some(h)` is `false` and falls through into the invalidation path.

### Per-claim verdicts

All basis rows I re-opened are **Supported**. Specifically:

- `blocks.rs:504-507` reads exactly as quoted; the `Option` is flattened by `.map` before the comparison, so `Ok(None)` from `get_block` and a hash mismatch are the same branch.
- `blocks.rs:509-534` performs the mutations in the order claimed: `recent.truncate(last_index)`, `pending` rewound to the invalidated number, `queue.clear`, `queue.push_back(Uncle)`, and `metrics::uncled_blocks_total.increment(1)` at `:520`.
- `index/mod.rs:106-130` routes only `-32001` here (`is_resource_not_found`, `:135-142`) and re-raises the original error on `Ok(None)` (`:124-125`).
- `state/mod.rs:182-189` accepts the uncle on `number < pending` / `number <= latest` and calls `snapshots.reorg(number)`, which deletes `>= number` and restores `number - 1` (`storage.rs:124-143`).
- `tx/storage.rs:96-100` re-read: `INSERT INTO transactions (request, expires_at) VALUES (?, ?)` with no `ON CONFLICT` and no idempotency key. The reviewer's "actions are not de-duplicated" is exact — and it is the right distinction to draw against `effects.rs:21-24`, which documents replay-safety for _effects_ only ("The same effect may be performed more than once for the same chain message").

### The intent objection, re-weighed

The doc comment does say "or cannot find the block at all" (`blocks.rs:473-475`), so this is a deliberate design decision and not an oversight. I still land on the reviewer's side, for a reason worth stating precisely: the recovery exists because the _log_ query failed with `-32001`, and the `null` answer to the _block_ query is then read as corroboration. But the two requests can be answered by the same lagging backend, in which case `null` corroborates nothing — it restates the same fact that triggered the recovery. Treating `None` as `Ok(None)` (the reviewer's remediation) is strictly weaker and cannot lose a real reorg: a genuine reorg eventually yields a _different hash_ at that height, not an absence, and until it does the caller simply retries. I could construct no scenario in which a real uncle is only ever observable as `null`.

### Finding verdict

**Confirmed** — mechanism verified exactly; the trigger is a concrete, in-scope A4 behaviour (one lagging backend behind a shared hostname produces both the `-32001` and the `null`). **Certainty 70%.** I set this at the bottom of the Confirmed band rather than the reviewer's 75/60: the code path is certain, but the trigger rests on a deployment shape (multi-backend endpoint with independent import progress) that is assumed rather than observed, and a single-node endpoint that answered `-32001` for logs would usually also serve the header. **Severity Medium, unchanged.** The state converges — the same block is re-emitted and re-applied — so this is "incorrect behaviour under unusual but reachable conditions" with recoverable impact. The non-recoverable part is the cost: one duplicate on-chain transaction per action in the replayed block (basis 6, verified above) and a permanently inflated `safenet_core_uncled_blocks_total`, which is the one signal an operator would use to judge reorg pressure. Not High: no liveness is lost and the trigger is not attacker-controlled. Not Low: real gas is spent and a diagnostic metric is corrupted.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 1 with its own threshold caveat, plus option 3.**

Option 1 is right in shape — `Some(block) if hash matches`, `Some(_) => invalidate`, `None => Ok(None)` — and its own tradeoff is the real risk: a node that serves `null` forever for a genuinely uncled height would stall instead of recovering, and the finding's mitigation (count consecutive `None`s, invalidate after a small threshold) is required, not optional. Note that the threshold turns this into another unbounded-retry path unless it feeds F-CORE-004 option 3's `consecutive_failures` metric; the two should share one counter rather than adding a second private one.

Option 2 (confirm against `latest` before invalidating) is sound and cheap, and is the more robust of the two because it distinguishes "behind" from "disagrees" using evidence rather than a heuristic count. Its extra RPC only fires on the `-32001` path.

Option 3 (separate metric label for "no block" vs "hash changed") is unconditionally correct and should land whichever of 1 or 2 is chosen — without it, `safenet_core_uncled_blocks_total` cannot be used as an alerting signal by anyone.

No option touches `apply_transition` or the effect system; the `core::state` contract is unaffected. **Interaction:** a spurious uncle triggers the full replay path, so every occurrence of this defect is also an occurrence of **F-CORE-067** (duplicate queued actions). Fixing F-CORE-003 reduces F-CORE-067's frequency without touching its mechanism, and the report should present it that way rather than as a mitigation.

## In-flight impact (AS-SEN)

**Pertains to unmerged branches, not to `main`.** Assessed against `origin/fix/sentinel_deadlines` (PR #914) and `origin/feat/optimistic_block_transition` (PR #915, tip `b2aad06`), built from a `git archive` extraction. Both apply cleanly onto `main` — `main`'s crates are byte-identical to their base `199629e`, so a verdict on the tip is also the verdict for the tip merged into `main`. **Effect: unchanged.** `crates/core/src/index/blocks.rs:505` is unchanged.
