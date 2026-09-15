# F-CORE-063 Execution is inferred from the account nonce alone and invalidated only by a block-number regression, so a transaction can be marked executed, pruned and silently lost

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | core, `tx/storage.rs`, `tx/mod.rs` |
| Location | `crates/core/src/tx/storage.rs:222-235` (related: `crates/core/src/tx/storage.rs:245-269, 271-279`, `crates/core/src/tx/mod.rs:145-200`) |
| Severity | Medium / Medium |
| Certainty | 55% |
| Assumptions involved | A4, A5 |
| Tags | reorg, crash-consistency, input-validation |

## Claim

The queue never looks at a transaction receipt. It concludes that its transaction executed purely from the account's transaction count having moved past that transaction's nonce, and it then deletes the row once the marking block is reorg-safe. The row is the only record that the action was ever requested, so after deletion the action is gone: it is not retried, not reported, and not recoverable.

Two independent things can move the count past a nonce without the queue's transaction having run: another sender using the same key (which the code explicitly documents as supported), and an RPC view in which the count is higher than canonical. In both cases an action the state machine decided to perform is dropped with no signal.

The invalidation side is asymmetric in a way that makes this stick. Execution is decided by a **nonce** comparison but un-decided only by a **block-number** regression: `unmark_executed` fires when `status.latest` moves backwards. A view in which the count is wrong without the height regressing — an inconsistent backend, or a reorg the block watcher resolves before the queue is next asked — leaves the marker in place until `prune` makes it permanent. The window is `latest - safe` blocks, i.e. `max_reorg_depth` (default 5, about 25 s on Gnosis).

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Execution is decided by a nonce comparison alone. No receipt, block hash or inclusion proof is consulted. | E2 | `crates/core/src/tx/storage.rs:222-235` | <pre>/// Marks every in-flight transaction the account has moved past (nonce below<br>/// `execution.nonce`) as executed at `execution.block`.<br>pub async fn mark_executed(&self, status: Status) -> Result&lt;, Error&gt; {<br> sqlx::query(<br> "UPDATE transactions<br> SET executed_at = ?<br> WHERE nonce IS NOT NULL AND nonce < ? AND executed_at IS NULL",<br> )<br> .bind(i64::try_from(status.block)?)<br> .bind(i64::try_from(status.nonce)?)<br> .execute(&self.pool)<br> .await?;<br> Ok()<br>}</pre> |
| 2 | A marked row is deleted permanently once the marking block is at or below `safe`. | E2 | `crates/core/src/tx/storage.rs:249-253` | <pre>// Prune transactions executed at or below the reorg-safe block.<br>sqlx::query("DELETE FROM transactions WHERE executed_at IS NOT NULL AND executed_at <= ?")<br> .bind(safe)<br> .execute(&mut *tx)<br> .await?;</pre> |
| 3 | The only un-marking trigger is a regression of the block height, not of the nonce. | E2 | `crates/core/src/tx/mod.rs:166-179` | <pre>// Invalidate execution markers for transactions as necessary.<br>if let Some(block) = match previous {<br> // On startup, conservatively invalidate all transactions executed<br> // past the `safe` block, as there may have been reorgs.<br> None => status.safe.checked_add(1),<br> // In case of a reorg (where the status has a latest block before<br> // the last status we've seen) indicates a reorg to `latest`, so<br> // invalidate markers accordingly.<br> Some(previous) if previous.latest > status.latest => status.latest.checked_add(1),<br> // In all other cases, there are no markers to invalidate.<br> _ => None,<br>} {<br> self.storage.unmark_executed(block).await?;<br>}</pre> |
| 4 | Un-marking is also keyed on the block number, so a marker written at a block that was never uncled is untouchable. | E2 | `crates/core/src/tx/storage.rs:271-279` | <pre>/// Clears the executed marker from transactions executed at or after `block`,<br>/// used when `block` is uncled by a reorg.<br>pub async fn unmark_executed(&self, block: u64) -> Result&lt;, Error&gt; {<br> sqlx::query("UPDATE transactions SET executed_at = NULL WHERE executed_at >= ?")<br> .bind(i64::try_from(block)?)<br> .execute(&self.pool)<br> .await?;<br> Ok()<br>}</pre> |
| 5 | Marking uses the nonce fetched from the RPC for the current block, with no validation. | E2 | `crates/core/src/tx/mod.rs:185-197` | <pre>if previous.is_none_or(\|previous\| previous.latest < status.latest)<br> && self.storage.count_outstanding(status.latest).await? > 0<br>{<br> let nonce = self.nonce.await?;<br> self.storage<br> .mark_executed(Status {<br> block: status.latest,<br> nonce,<br> })<br> .await?;<br> self.resubmit_stale(status.latest).await?;<br> self.submit_pending(status.latest).await?;<br>}</pre> |
| 6 | Consuming a nonce outside the queue is a documented, supported mode of operation — so "another sender moved the count" is by design, not misuse. | E2 | `crates/core/src/tx/storage.rs:126-130` | <pre>/// The nonce is the first free nonce at or above `status.nonce` (the<br>/// account's current onchain transaction count, passed in so nonces<br>/// consumed by transactions submitted outside the queue are respected),<br>/// accounting for the nonces of other in-flight transactions. Selecting the<br>/// nonce and reserving it for the transaction happen atomically.</pre> |
| 7 | Nothing distinguishes "our transaction executed" from "the row vanished": the deletion is unconditional and unlogged, and there is no execution callback to the state machine. | E2 | `crates/core/src/tx/mod.rs:160-164` | <pre>// Prune the transaction storage if necessary.<br>if previous.is_none_or(\|previous\| previous.safe != status.safe) {<br> self.storage.prune(status.safe).await?;<br>}</pre> |

## Trigger

Any observation in which the account's transaction count exceeds the number of the queue's own transactions that actually executed, sustained across `max_reorg_depth` blocks.

1. **Shared key (supported by claim 6).** The queue holds nonce 5 in flight. An operator or tool sends a transaction from the same key; because a client fills the nonce from the account count, it also takes 5 and, being broadcast later at a higher fee or simply reaching a different mempool, wins. The count moves to 6. `mark_executed` marks the queue's row for nonce 5 as executed at the current `latest` (claim 1). Five blocks later `safe` passes that block, `prune` deletes it (claim 2), and the action — a `keyGenSecretShare`, a sentinel commit, whatever the state machine decided — was never sent and will never be sent. The state machine has already advanced past it, so nothing re-queues it.
2. **Inconsistent RPC view (A4).** A load-balanced provider routes `eth_getTransactionCount` to a backend on a different fork at the same height, in which the signer sent a transaction the canonical chain does not have. The count reads one higher. The block watcher, served by another backend, sees no reorg, so `status.latest` never regresses and claim 3's un-marking never fires. The marker persists, `prune` makes it permanent, and the action is lost.
3. **Reorg the queue does not observe as a height regression.** `unmark_executed` compares only against the immediately previous status (claim 3). A reorg that replaces blocks without lowering the height below the previously observed `latest` — or one whose intermediate statuses the queue never sees, because `update_block_status` returned early on an equal status (`crates/core/src/tx/mod.rs:146-149`) or was skipped by a swallowed intermittent error (`crates/core/src/driver.rs:247-253`) — leaves markers written against the orphaned view intact.

The consequence in all three cases is the same and is the reason this is not merely cosmetic: the queue's contract to the driver is that a queued action is eventually executed or retried, and here it is neither. `analysis-core.md` §5 records this as invariant I6 ("A transaction that holds a nonce is eventually mined or replaced (never dropped)"); claims 1–2 show the invariant is enforced against the _nonce_ being consumed, not against _this transaction_ being included, which is a strictly weaker property than the driver relies on.

I cannot demonstrate instances 2 or 3 offline. Instance 1 needs only an operator action the code documents as supported, but I have not executed it (A9 FALSE), so the trigger is `E2`-adjacent at best and I record it as `I`. The code path — nonce-only inference, block-only invalidation, unconditional deletion — is `E2`.

## Considered and rejected

- **"The design intends this and it is documented."** Partly. `crates/core/src/tx/storage.rs:126-130` documents that nonces consumed outside the queue are _respected during allocation_; it does not say that a queued action may be silently discarded as a result. The two are different promises, and `mark_executed`'s own doc comment ("Marks every in-flight transaction the account has moved past") describes the mechanism without naming the consequence. The `known` tag does not apply: this is not in `codebase-map.md` §4.
- **"A receipt check would be equivalent."** It would not be equivalent, it would be strictly stronger — `eth_getTransactionReceipt` on the hash the queue already computes (`crates/core/src/tx/signer.rs:69-71`, logged at `crates/core/src/tx/mod.rs:262`) distinguishes "our transaction was included" from "the nonce was consumed". The queue computes the hash and throws it away.
- **"The reorg handling covers it."** It covers the case where the _height_ regresses (claim 3). It cannot cover a nonce that is wrong at a stable height, because `unmark_executed` selects on `executed_at`, a block number (claim 4). This asymmetry — decide on nonce, undo on height — is the specific defect.
- **"`prune` at `safe` is conservative enough."** `safe` bounds the _reorg_ window (A5), and within that model pruning at `safe` is correct for reorgs. It does nothing for a marker that was wrong for a reason other than a reorg, which is instances 1 and 2.
- **"The driver would re-queue the action on replay."** Replay after a restart re-emits actions from the last `max_reorg_depth` blocks (`analysis-core.md` §6.3, outside my scope), which would only help if the loss coincided with a restart, and would then produce a _duplicate_ rather than a repair. In steady-state operation nothing replays.
- **"The tests cover this."** They do not. `initial_status_reconciles_executions_in_the_reorg_window` (`crates/core/src/tx/mod.rs:457-487`) and `submits_queued_transactions_with_reorg_awareness` (`crates/core/src/tx/mod.rs:489-538`) drive the mocked count forward and back in lockstep with the block height; no test moves the count without the height, which is exactly the case that fails. `analysis-core.md` §10 lists "`mark_executed` with nonces consumed externally" and "`prune` of executed rows" as untested.

## Remediation options

1. **Confirm inclusion by receipt.** Persist the submitted transaction hash (the queue already computes it) and, before marking a row executed, fetch its receipt. Mark executed only on a receipt whose block is at or below `latest`; if the nonce moved past the row but no receipt exists for any of its submitted hashes, mark the row _displaced_ rather than executed. Tradeoff: one extra RPC call per newly executed transaction, on a queue that currently makes at most three calls per block. This is the option that makes the distinction the system actually needs.
2. **Report displacement instead of swallowing it.** Whatever the detection mechanism, a row that is removed without having been included must be surfaced: an `error`-level log naming the action, a counter, and ideally a callback so the service can decide whether to re-queue. Silence is the harmful part; a validator that skipped a keygen share should know.
3. **Do not prune on a marker that could be wrong.** Require both `executed_at <= safe` _and_ a confirmed receipt before deleting. Rows that fail the second test are retained and re-evaluated, bounding storage growth by requiring an eventual decision rather than an eventual timeout.
4. **Re-check markers when the nonce regresses, not only when the height does.** Track the last observed count alongside `block_status`; if a later observation at the same or a greater height returns a _lower_ count, treat it as an invalidation and `unmark_executed` from the affected block. Cheap, no extra RPC, and it closes instances 2 and 3 without addressing instance 1.
5. **Detect the shared key.** If a receipt check is in place, a nonce consumed by a transaction the queue did not send is directly observable; log it at `error` and expose a counter, since the handbook already tells operators never to reuse the key and this would tell them when they have.

Tests to add: a queue test that advances the mocked transaction count by one _without_ advancing the block height beyond the previous status and asserts the intended behaviour; a test that drives the count past an in-flight nonce, prunes, and asserts that the action is reported rather than silently gone; and a storage test for `mark_executed` followed by `prune` asserting that a displaced row is distinguishable from an executed one.

## Trail

- Reviewer R3: drafted, self-estimate 60%. The mechanism (nonce-only inference, height-only invalidation, unconditional deletion, no reporting) is `E2` and directly cited. The estimate reflects that the most reachable trigger — a second sender on the same key — is a mode the code documents as supported while the handbook advises against it, so a Critic may reasonably argue it is accepted risk; the invalidation asymmetry in claims 3 and 4 stands independently of that argument. This is the "silently dropped action" half of `analysis-core.md` H11; the "permanent gap" half is `F-CORE-062`.

## Critic (C-CORE-B)

Read `mark_executed` (`tx/storage.rs:222-235`), `prune` (`:245-269`), `unmark_executed` (`:271-279`) and `update_block_status` (`tx/mod.rs:145-200`) before the Claim. `mark_executed` sets `executed_at = status.block` for every row with `nonce < status.nonce` — a pure nonce inference, never a receipt — and `prune` then deletes those rows once `executed_at <= safe`. Invalidation, by contrast, is triggered only by `previous.latest > status.latest`, a **block-number** regression. The asymmetry is exactly as claimed and the row is the only record the action was ever requested.

### Per-claim verdicts

All basis rows **Supported** against the cited ranges; the code quoted is present verbatim and behaves as described. Nothing is `H`. The reviewer's negatives are also right: `unmark_executed` selects on `executed_at`, a block number, so it structurally cannot undo a nonce that was wrong at a stable height; and the two reconciliation tests (`tx/mod.rs:457-487`, `:489-538`) drive the mocked count and the height in lockstep, so no test covers the case that fails.

### One trigger instance I refute

**Trigger instance 3 does not hold**, and R3's own coverage log contradicts it. The sub-clauses are:

- _"`update_block_status` returned early on an equal status"_ — an equal status carries an equal `latest`, so no regression is being hidden; the comparison at `tx/mod.rs:174` is against the **last seen** status, not against every intermediate one.
- _"or was skipped by a swallowed intermittent error (`driver.rs:247-253`)"_ — this is wrong on the ordering. `self.block_status = Some(status)` is assigned at `tx/mod.rs:153`, `prune` and `unmark_executed` run at `:162-179`, and the first RPC call (`nonce`) is at `:188`. The only swallowable error is the RPC one, which happens **after** `unmark_executed`. A lifted error therefore cannot skip the un-marking.
- _"a reorg that replaces blocks without lowering the height below the previously observed `latest`"_ — `BlockWatcher::status` is `pending.number - 1` (`index/blocks.rs:376-381`) and the uncle path rewinds `pending` to the uncled block before returning `BlockUpdate::Uncle` (`:421-433`), while `Driver::update` calls `update_block_status(self.watcher.block_status)` on **every** update including that one (`driver.rs:240-247`). So the height always regresses and the un-marking always fires. R3's own rejected-hypothesis 26 says the same thing and refutes their own instance 3.

Instances 1 (shared key) and 2 (fork-inconsistent nonce view) are unaffected and carry the finding.

### Finding verdict

**Plausible — 55%.** Mechanism `E2` and complete; instance 3 refuted; instances 1 and 2 are `I`, as the reviewer concedes ("I record it as `I`"). Instance 1 needs an operator action the code documents as supported but the handbook discourages; instance 2 needs the same fork-inconsistent RPC that F-CORE-062 rests on, which stretches A4's letter.

**Severity: Medium (unchanged).** Correct band: an action the state machine decided to perform is dropped with no signal and no retry, which is "incorrect behaviour under unusual but reachable conditions". Not High — no attacker steers it. Not Low — the queue's contract to the driver is broken silently and the loss is permanent after `prune`.

The remediation worth carrying into the report is the cheap one the reviewer identifies: the queue already computes the transaction hash (`tx/signer.rs:69-71`) and logs it (`tx/mod.rs:262`) before throwing it away. Keeping it and checking a receipt distinguishes "our transaction was included" from "the nonce was consumed", which is the whole defect.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 4 as the cheap fix, option 1 as the correct one. Option 3 has an ordering dependency.**

Option 4 (track the last observed transaction count alongside `block_status`; treat a _lower_ count at the same or greater height as an invalidation) is cheap, needs no extra RPC, and closes two of the three instances. It is the right first change and the finding rates it correctly.

Option 1 (persist the submitted transaction hash and confirm inclusion by receipt, marking a row _displaced_ rather than executed when the nonce moved past it with no receipt) is the fix that makes the distinction the system actually needs. Its cost — one extra RPC per newly executed transaction, on a queue that makes at most three calls per block — is proportionate. Sound.

Option 2 (surface a displaced row: `error` log, counter, ideally a callback so the service can re-queue) is the part that matters most and is independent of the detection mechanism. The finding is right that "silence is the harmful part": a validator that skipped a keygen share must know.

Option 3 (require both `executed_at <= safe` **and** a confirmed receipt before pruning) is sound but depends on option 1 — there is no receipt to require until hashes are persisted. Sequence it after, and note the storage consequence the option acknowledges: rows that fail the receipt test are retained indefinitely unless the "eventual decision" is actually implemented.

Option 5 (detect a shared key) is a free by-product of option 1 and is worth calling out separately in the report, because the handbook already tells operators never to reuse the signer key and currently has no way to tell them when they have.

**Cross-finding, important:** this finding is a prerequisite for **F-CORE-067 option 1**. An idempotency key only suppresses a duplicate while the row exists; rows are pruned on `executed_at <= safe`, and this finding shows `executed_at` can be wrong. A key freed by a wrongly inferred execution re-opens the duplicate the key exists to prevent. **F-CORE-063 and F-CORE-067 option 1 must be fixed together**, or the key must be retained beyond the row's lifetime.

## In-flight impact (FWD)

**Pertains to unmerged branches, not to `main`.** Assessed against the "Batched Execution" stack (`origin/feat/batex_0` … `origin/feat/batex_4`, PRs #899–#904) and its epic. **Effect: worsen (blast radius).** `mark_executed` is untouched by the whole stack — still `WHERE nonce IS NOT NULL AND nonce < ? AND executed_at IS NULL`, still no receipt read — and so is `prune` and the asymmetric `unmark_executed`. Two things make the same inference cost more. First, once Phase 7 lands (not on any pushed branch) a single row is a _batch_ of six to eight protocol actions on the epic's default `max_batch_gas = 2_000_000`, so one wrong "executed" marking drops a batch rather than an action. Second, Phase 1's new per-call gas guard in `Safenet7702Executor.execute` — `require(gasleft() * 63 / 64 >= call.gasLimit, InsufficientGas(i))` — introduces a **whole-batch revert** that did not previously exist, and a reverted transaction advances the nonce exactly like a successful one, so it is invisible to this inference. The epic's claim that the guard "converts a silent action-dropping failure into a loud one" is true onchain and false offchain. The delegation transaction the epic adds is itself declared executed purely from a nonce advance. Filed separately as **`F-CORE-068`**. Severity and certainty unchanged. See `rust-audit/report/IN-FLIGHT.md`.
