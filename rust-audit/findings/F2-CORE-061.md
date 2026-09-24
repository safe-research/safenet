# F2-CORE-061 Execution is inferred from a single `eth_getTransactionCount` reading and the mark is irreversible without an observed reorg; an inconsistent RPC view can strand an unmined transaction as "executed" and open a permanent nonce gap

| Field | Value |
| --- | --- |
| Status | QA'd |
| Crate and module | safenet-core, tx/mod.rs, tx/storage.rs |
| Location | crates/core/src/tx/mod.rs:185-197 (related: tx/mod.rs:166-179, 300-317; tx/storage.rs:224-235, 245-253) |
| Severity | Medium (reviewer) / Medium (Critic) |
| Certainty | 55% (QA2-CORE; Critic C2-CORE-B set 50%) |
| Assumptions involved | A4 |
| Tags | reorg, dos |

## Claim

The queue decides that a transaction executed solely because the account's transaction count, read once per block at `BlockId::Number(latest)`, is greater than the row's nonce. Transaction hashes are never persisted and receipts are never consulted. The mark (`executed_at = latest`) is only ever cleared by `unmark_executed` when the _block watcher_ observes a regression of `latest`; a later, lower nonce reading does not revoke it, and once `safe` passes `executed_at` the row is deleted. The block watcher and the nonce query use the same provider URL but not necessarily the same backend: with a load-balanced endpoint (the shipped samples point at one) or any node on a short-lived fork, the nonce query can answer from a branch that includes transaction N while the watcher follows the branch that does not. Row N is then marked executed at `latest`, the watcher never emits an uncle for a branch it never saw, so the mark sticks and the row is pruned five blocks later. If transaction N does eventually mine on the canonical branch nothing is lost; if it is dropped instead (pool eviction, node restart, base fee moving above its now-frozen `max_fee_per_gas` while nothing bumps it), the account nonce stays at N forever while the queue continues to allocate N+1, N+2, ... from `MAX(nonce)+1`. Those rows are accepted as future-nonce transactions, can never mine, and are fee-bumped every two blocks (F2-CORE-060). The service's onchain output is dead until an operator sends a nonce-N transaction from the signer key by hand; a restart heals it only if it happens within `max_reorg_depth` blocks (the startup `unmark_executed(safe + 1)` re-checks rows not yet pruned).

A related consequence of nonce-only inference: a transaction that mines but reverts is also "executed"; the queue has no way to tell and no telemetry for it.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Execution marking is driven by one nonce reading per block; no receipt or hash is involved. | E2 | crates/core/src/tx/mod.rs:185-197 | `if previous.is_none_or(\|previous\| previous.latest < status.latest)` / `&& self.storage.count_outstanding(status.latest).await? > 0` / `{` / `let nonce = self.nonce().await?;` / `self.storage` / `.mark_executed(Status {` / `block: status.latest,` / `nonce,` / `})` / `.await?;` / `self.resubmit_stale(status.latest).await?;` / `self.submit_pending(status.latest).await?;` |
| 2 | Every allocated row with a nonce below the reading is marked, unconditionally. | E2 | crates/core/src/tx/storage.rs:224-231 | `pub async fn mark_executed(&self, status: Status) -> Result<(), Error> {` / `sqlx::query(` / `"UPDATE transactions` / `SET executed_at = ?` / `WHERE nonce IS NOT NULL AND nonce < ? AND executed_at IS NULL",` / `)` / `.bind(i64::try_from(status.block)?)` / `.bind(i64::try_from(status.nonce)?)` |
| 3 | The reading is by block _number_, so any backend holding a different block at that height answers for its own branch. | E2 | crates/core/src/tx/mod.rs:304-312 | `let block_id = self` / `.block_status` / `.map(\|block_status\| BlockId::from(block_status.latest))` / `.unwrap_or_else(BlockId::latest);` / `let nonce = self` / `.provider` / `.get_transaction_count(self.signer.address())` / `.block_id(block_id)` / `.await?;` |
| 4 | Marks are only revoked when the watcher's own `latest` regresses (or on the first status after start); a lower nonce reading has no effect. | E2 | crates/core/src/tx/mod.rs:166-179 | `if let Some(block) = match previous {` / `None => status.safe.checked_add(1),` / `Some(previous) if previous.latest > status.latest => status.latest.checked_add(1),` / `_ => None,` / `} {` / `self.storage.unmark_executed(block).await?;` / `}` |
| 5 | The watcher only regresses when _its_ fetched block does not build on _its_ recent chain; a branch it never fetched produces no uncle. | E2 | crates/core/src/index/blocks.rs:421-433 | `if let Some(last) = self` / `.recent` / `.pop_back_if(\|last\| last.hash != block.parent_hash)` / `{` / `self.pending = PendingBlock {` / `number: last.number,` ... `return Ok(BlockUpdate::Uncle {` / `number: last.number,` / `});` |
| 6 | A falsely marked row is deleted once `safe` passes its observation block, after which nothing can restore it. | E2 | crates/core/src/tx/storage.rs:249-253 | `// Prune transactions executed at or below the reorg-safe block.` / `sqlx::query("DELETE FROM transactions WHERE executed_at IS NOT NULL AND executed_at <= ?")` / `.bind(safe)` / `.execute(&mut *tx)` / `.await?;` |
| 7 | Later allocations start above the stranded nonce, so the gap is never filled by the queue itself. | E2 | crates/core/src/tx/storage.rs:145-149 | `"UPDATE transactions` / `SET nonce = MAX(?, COALESCE(` / `(SELECT MAX(nonce) + 1 FROM transactions),` / `0` / `))` |
| 8 | Neither the hash nor the receipt is stored: the schema has no hash column. | E2 | crates/core/src/tx/storage.rs:70-77 | `"CREATE TABLE IF NOT EXISTS transactions (` / `id           INTEGER PRIMARY KEY,` / `request      TEXT    NOT NULL,` / `expires_at   INTEGER DEFAULT NULL,` / `nonce        INTEGER DEFAULT NULL,` / `submitted_at INTEGER DEFAULT NULL,` / `executed_at  INTEGER DEFAULT NULL` / `)",` |
| 9 | The shipped configurations use a public load-balanced endpoint, where the watcher and the nonce query routinely hit different backends. | E2 | crates/validator/validator.sample.toml:10, crates/sentinel/sentinel.sample.toml:10 | `rpc = "https://rpc.gnosischain.com"` |

## Trigger

Mockable sequence in the style of `submits_queued_transactions_with_reorg_awareness` (mod.rs:489-538): queue one transaction; status 10 with nonce 0 -> allocated nonce 0 and broadcast; status 11 with nonce reading **1** (backend on a 1-block fork that included it) -> row marked executed at 11; statuses 12..20 with nonce reading **0** (canonical branch never included it) and no watcher regression -> the row remains executed, is pruned once `safe >= 11`, and a second queued transaction is allocated nonce 1 and broadcast, never to mine. The real-world path needs, in addition, the original transaction to be dropped from producers' pools rather than re-injected after the fork loses; that step is I (ordinary but not guaranteed).

## Considered and rejected

- _A low reading (lagging or forked backend that has not seen our executed transactions) causes the mirror image: a too-low nonce allocated, then marked executed by the next correct reading, silently losing the action._ Mostly guarded: allocation takes `MAX(reading, MAX(nonce)+1)` (Basis 7) and executed rows are retained until `safe`, so during the reorg window the queue's own records dominate a low reading. Losing that protection needs all rows pruned and a fork deeper than `max_reorg_depth`, which is out of scope. Recorded as a rejected hypothesis in the coverage log.
- _`executed_at = latest` is an observation block, not the inclusion block, so marks are conservative._ True in the honest case (inclusion <= observation) and it makes `unmark_executed` on a real uncle correct; it does nothing for a mark that was never true.
- _The watcher and the queue share a provider so they see one chain._ They share a URL; per-request routing is the provider's business, and `Provider` has no affinity mechanism (`crates/core/src/provider/mod.rs`, read for context only).
- _The false mark is harmless because the transaction mines anyway._ Usually. The finding is about the absence of any recovery when it does not, and the queue is the only component that could notice.

## Remediation options

1. Persist the transaction hash on submission and confirm execution with `eth_getTransactionReceipt` (or `eth_getTransactionByHash` block number) before marking, falling back to the nonce heuristic only for rows without a hash. Tradeoff: one extra RPC per in-flight row per block while in flight.
2. Make the mark revocable: keep executed rows until `safe` (already the case) and, on every block, re-check `nonce < reading` for rows executed above `safe`; clear the mark when a lower reading appears without an uncle, logging a warning about inconsistent RPC views. Cheap, no new RPC.
3. Sanity-check readings against the queue's own records (`reading <= MAX(nonce)+1` unless the row was submitted; a reading that jumps past rows never accepted into a mempool is suspicious) and skip marking that block rather than trusting it.

Tests to add: the mock sequence in Trigger; a test that a reading which later decreases without an uncle un-marks the row.

## Trail

- Reviewer R3: drafted at commit 3ec8bc5, self-estimate 45%
- Critic C2-CORE-B: Plausible, 50%, severity Medium (reviewer Medium).
- QA2-CORE: Reproduced for the irrevocable mark and nonce gap (mocked reading); counter-case shows the gap closes unless an action is queued inside the retention window; certainty 50 → 55; PoC `poc/F2-CORE-061/`.

## Critic (C2-CORE-B)

Method: read title and Location only, traced `update_block_status`, `nonce()`, `mark_executed`/`unmark_executed`/`prune` and the watcher's reorg detection myself, then compared.

| # | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | mod.rs:185-197. |
| 2 | Supported | storage.rs:224-231. |
| 3 | Supported | mod.rs:304-312: `BlockId::from(block_status.latest)` — a number, not a hash. |
| 4 | Supported | mod.rs:166-179. |
| 5 | Supported | blocks.rs:421-433. |
| 6 | Supported | storage.rs:249-253. |
| 7 | Supported | storage.rs:145-149. |
| 8 | Supported | storage.rs:70-77. |
| 9 | Supported | `rpc = "https://rpc.gnosischain.com"` at validator.sample.toml:10 and sentinel.sample.toml:10. Whether that endpoint routes per request across backends is `I`. |

Own view of the mechanism: agreed — one `eth_getTransactionCount` at `latest` per block is the sole execution oracle, the mark is monotone, and only a watcher-observed regression (`previous.latest > status.latest`) or the startup path clears it. `Provider` has a single `ClientBuilder`/`RootProvider` and no affinity (provider/mod.rs:129-137), confirmed.

Counter-evidence on the trigger's likelihood (not on the mechanism): the watcher validates the parent hash of every block _it_ fetches (blocks.rs:421-433), so a fork the watcher also touches yields `Uncle` → `unmark_executed(latest + 1)` (mod.rs:174-178) and heals; the harmful sequence needs the fork to be visible to the nonce query only, _and_ the transaction to be dropped from producers' pools rather than re-mined on the canonical branch — two independent unusual events on an endpoint that A4 allows to be stale but not malicious. A restart within `max_reorg_depth` heals via the startup `unmark_executed(safe + 1)` (mod.rs:170), as the reviewer notes.

Finding verdict: **Plausible** (mechanism verified; trigger reachable but unproven). Certainty **50%**. Severity **Medium / Medium**: the outcome is a permanent, silent stall of the service's on-chain output (F2-CORE-067) with no in-service recovery, but the trigger is environmental and not attacker-controlled.

QA note: the mock sequence in Trigger demonstrates the _irrevocability_ of the mark (a valid E1 for rows 1-4 and 6-7), not the RPC inconsistency itself; it should be labelled that way. Related out-of-scope note: any other holder of the signer key (an A1 operator mistake) drives the same irrevocable mark by advancing the nonce.

## QA (QA2-CORE)

**Outcome: Reproduced** — the irrevocability of the mark and the resulting nonce gap (rows 1-4, 6-7), labelled as the Critic asked; the RPC inconsistency itself is mocked (one reading of 1, then readings of 0), not observed.

Command: paste `poc/F2-CORE-061/tx_mod_tests.rs` into the `mod tests` of `crates/core/src/tx/mod.rs`; `cargo test -p safenet-core --lib qa_f2_core_061 -- --nocapture --test-threads=1`; file reverted.

Decisive output (`poc/F2-CORE-061/output.txt`): after a single reading of 1 at block 11 the row is `executed_at = 11`; `blocks 12..=15: no RPC issued, mark untouched` (nothing is outstanding, so `eth_getTransactionCount` is not even asked); `second action allocated nonce 1 while the chain nonce reads 0`; after block 30 (safe 25): `[(2, .., nonce Some(1), submitted_at Some(29), executed_at None)]` — row 0 pruned, row 1 stuck and bumped seven times (`maxFeePerGas=Some(414)`).

Counter-case (`qa_f2_core_061_gap_closes_when_nothing_is_queued_inside_the_retention_window`): if no action is queued while the false-marked row is retained, the row is pruned once `safe` passes the mark and the next allocation is `next action allocated nonce 0` — the gap closes. The permanent stall therefore needs a third condition the Claim does not state: an action queued within `max_reorg_depth` blocks of the false mark (routine for a sentinel with requests flowing, not guaranteed).

Certainty: 50 → **55**. Stays in the Plausible band: the mechanism is now `E1`, but the trigger is a conjunction of an RPC view split, the transaction being dropped, and an action queued inside the retention window.

Remediation check: option 1 sound. Option 2 (re-check `nonce < reading` for rows executed above `safe`) is sound only if such rows also count as outstanding: `count_outstanding` excludes executed rows today, so no reading is taken while only a false-marked row exists (shown above) and the re-check would never run. Option 3 fine as a heuristic.

## Reconciliation (run 2)

**Final: EXTENDS `F-CORE-063` (canonical, Medium 55) and `F-CORE-062` (Medium 60) — E1 for the irrevocable mark and the gap.** Run 1 filed the nonce-only inference (063) and the permanent wedge (062) separately at E2; this file executes both halves and narrows the trigger (an action must be queued inside the retention window). Run 1 alone carries the shared-key instance and the height-only invalidation asymmetry (`state/run2/reconciliation/core.md` §1).
