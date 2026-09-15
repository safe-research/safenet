# F-CORE-064 `expires_at` is silently void once a nonce is allocated, contradicting the queue's documented contract

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | core, `tx/storage.rs`, `tx/mod.rs` |
| Location | `crates/core/src/tx/storage.rs:285-311` (related: `crates/core/src/tx/storage.rs:118-168, 245-269`, `crates/core/src/tx/mod.rs:128-141, 221-237`) |
| Severity | Medium / Medium |
| Certainty | 72% |
| Assumptions involved | A2, A10 |
| Tags | input-validation, reorg, fees |

## Claim

`TransactionQueue::queue` documents `expires_at` as the block by which the transaction is dropped if it has not been submitted. In practice the deadline gates exactly one thing — whether a _queued_ row may be allocated a nonce — and stops applying the instant a nonce is allocated. The resubmission query carries no expiry predicate, and pruning deletes expired rows only while `nonce IS NULL`. So a transaction submitted one block before its deadline is rebuilt, re-signed and rebroadcast on the queue's normal cadence for as long as it goes unexecuted: hours, days, or the lifetime of the deployment, each time with a fee at least 10% above the last (and, per `F-CORE-060`, every block rather than every `blocks_before_resubmit` blocks once a rejection is classified as underpriced).

The services use `expires_at` for real protocol deadlines — the sentinel's commit and reveal deadlines, the validator's keygen and signing deadlines. A transaction carrying such a deadline can therefore be broadcast, and land, arbitrarily long after the deadline it was supposed to respect, at a fee unrelated to the value of the action. The service's own state machine has meanwhile moved on and has no way to learn that the abandoned action was still in flight.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The public contract says an unsubmitted transaction is dropped at `expires_at`. | E2 | `crates/core/src/tx/mod.rs:128-135` | <pre>/// Queues `transaction` for execution, to be dropped if it has not been<br>/// submitted by block `expires_at`, or never dropped if `expires_at` is<br>/// `None`, then attempts to submit it (and any other queued transactions)<br>/// onchain.<br>pub async fn queue(<br> &mut self,<br> transactions: impl IntoIterator&lt;Item = (Transaction, Option&lt;u64&gt;)&gt;,<br>) -> Result&lt;, Error&gt; {</pre> |
| 2 | The deadline is consulted only when selecting a _queued_ (`nonce IS NULL`) row for allocation. | E2 | `crates/core/src/tx/storage.rs:150-156` | <pre>WHERE id = (<br> SELECT id FROM transactions<br> WHERE nonce IS NULL AND (expires_at IS NULL OR expires_at > ?)<br> ORDER BY id ASC<br> LIMIT 1<br> )<br> RETURNING json_set(request, '$.nonce', nonce)",<br>)</pre> |
| 3 | The resubmission query has no expiry predicate at all; an allocated row is selected on submission age alone. | E2 | `crates/core/src/tx/storage.rs:293-298` | <pre>sqlx::query_scalar::&lt;_, String&gt;(<br> "SELECT json_set(request, '$.nonce', nonce)<br> FROM transactions<br> WHERE nonce IS NOT NULL AND executed_at IS NULL<br> AND (submitted_at IS NULL OR submitted_at <= ?)<br> ORDER BY nonce ASC",<br>)</pre> |
| 4 | Pruning an expired row requires `nonce IS NULL`, so an allocated row is never dropped for expiry. | E2 | `crates/core/src/tx/storage.rs:259-265` | <pre>sqlx::query(<br> "DELETE FROM transactions<br> WHERE nonce IS NULL AND expires_at IS NOT NULL AND expires_at <= ?",<br>)<br>.bind(safe)<br>.execute(&mut *tx)<br>.await?;</pre> |
| 5 | The crate's own test asserts the behaviour as intended, and states the rationale. | E2 | `crates/core/src/tx/mod.rs:573-584` | <pre>// At block 12, another transaction gets mined, but the outstanding<br>// transaction has already expired and is not executed. However, we<br>// do get resubmissions of the remaining original inflight transactions<br>// because of the resubmit deadline, despite being past the expiry. This<br>// is because once a transaction is in the mempool, it has to execute.<br>asserter.push_success(&U64::from(2)); // signer transaction count<br>asserter.push_success(&fee_history); // fee estimate<br>for _ in 2..queue.config.max_in_flight_transactions {<br> asserter.push_success(&B256::ZERO); // transaction hash from submission<br>}<br>queue.update_block_status(block_status(12)).await.unwrap;<br>assert!(asserter.read_q.is_empty);</pre> |
| 6 | Resubmission is unconditional over the stale set and applies the fee bump every time. | E2 | `crates/core/src/tx/mod.rs:224-236` | <pre>async fn resubmit_stale(&mut self, block: u64) -> Result&lt;, Error&gt; {<br> let submitted_before = block.checked_sub(self.config.blocks_before_resubmit);<br> let stale = self.storage.stale_submissions(submitted_before).await?;<br> if stale.is_empty {<br> return Ok();<br> }<br><br> for transaction in stale {<br> tracing::debug!(nonce = transaction.nonce, "resubmitting stale transaction");<br> self.submit_transaction(transaction, block).await?;<br> }<br><br> Ok()<br>}</pre> |
| 7 | `expires_at` carries real protocol deadlines in the sentinel. | E2 | `crates/sentinel/src/service.rs:426-437` | <pre>actions.push(<br> SentinelAction {<br> kind: SentinelActionKind::Reveal {<br> id: *id,<br> approve,<br> salt,<br> reason,<br> },<br> expires_at: Some(reveal_deadline),<br> }<br> .into,<br>);</pre> |
| 8 | And in the validator, where the state machine supplies a per-action deadline. | E2 | `crates/validator/src/service/action.rs:149-168` | <pre>Action::KeyGenSecretShare {<br> group_id,<br> share,<br> expires_at,<br>} => {<br> let gas = 250_000 + 25_000 * share.f.len as u64;<br> (<br> Transaction {<br> to: self.coordinator,<br> value: U256::ZERO,<br> data: Coordinator::keyGenSecretShareCall {<br> gid: group_id,<br> share,<br> }<br> .abi_encode<br> .into,<br> gas,<br> },<br> expires_at,<br> )<br>}</pre> |

## Trigger

Any transaction that is allocated a nonce at or before its `expires_at` block and then fails to be included promptly.

The narrowest concrete case: the sentinel queues a `Reveal` with `expires_at: reveal_deadline` (claim 7). One block before the deadline the queue allocates it a nonce and broadcasts it — legitimately, since claim 2's predicate still passes. The transaction is not included (the mempool is congested, the fee estimate was low, or, per `F-CORE-062`, a lower nonce is missing). From the next block onward claims 3, 4 and 6 apply without reference to the deadline: the queue rebuilds it, bumps the fee at least 10%, and rebroadcasts, every `blocks_before_resubmit` blocks — or every block once a rejection matches `is_transaction_underpriced` (`crates/core/src/tx/mod.rs:271-283`). The reveal deadline passes. The sentinel's state machine has moved on. The queue keeps escalating the fee to get a transaction onchain that the service no longer wants, and will keep doing so across restarts, since the row is in SQLite.

Two outcomes follow, and which one applies depends on the contract, not on the Rust:

- The onchain call reverts because it is past its deadline. Gas is burned at an escalating price, and the nonce is finally consumed, so the queue unblocks. Under A2 the deadline is attacker-influenced (a proposer choosing when to act), so an adversary who can keep the sentinel's transactions out of blocks around a deadline converts each one into a paid-for revert.
- The onchain call has no deadline check and succeeds late — the action the service abandoned takes effect. Whether that matters is a per-action question for R5/R6, not for core; but core is where the guarantee was promised and not kept.

Claims 1–8 are all `E2`. I have not executed anything (A9 FALSE), and I have not established which onchain calls check their own deadlines, so the _impact_ half is `I`.

## Considered and rejected

- **"This is intended, and the test says so."** The test comment (claim 5) argues "once a transaction is in the mempool, it has to execute", which is correct and is exactly why the row must not be dropped — dropping it would leave an allocated nonce unfilled and wedge the queue (`F-CORE-062`). The finding is not that the row should be deleted. It is that (a) the documented contract in claim 1 says "dropped if it has not been submitted by block `expires_at`" without qualifying that submission is a one-way door, and (b) _continuing to escalate the fee_ for an action whose deadline has passed is a different decision from _not dropping the row_, and the code makes only the first decision while inheriting the second by omission. A transaction past its deadline could be kept, resubmitted at an unchanged or minimal fee, and reported — none of which happens.
- **"The service could simply not set `expires_at`."** Both services set it deliberately for deadline-bearing actions and leave it `None` otherwise (`crates/sentinel/src/service.rs:231, 239, 434` versus `524, 571, 638, 667`). They are using the API as documented; the documentation is what is wrong.
- **"`prune` will remove it eventually."** Only via `executed_at` (`crates/core/src/tx/storage.rs:250`), i.e. only once the account nonce moves past it. Expiry-based pruning is gated on `nonce IS NULL` (claim 4).
- **"The in-flight cap bounds the damage."** It bounds the _count_ to `max_in_flight_transactions` (default 16), not the duration or the fee. Sixteen simultaneously-expired transactions each ratcheting their fee is the worse case, not the better one, and it also means no new action can be queued (`crates/core/src/tx/mod.rs:204-216`).
- **"An operator would see it."** No metric distinguishes an expired in-flight transaction from a healthy one; resubmission logs at `debug` (claim 6) and failures at `warn`. See Observation O6 in `rust-audit/../state/coverage-logs.md#r3`.
- **Not a duplicate of `F-CORE-060`.** That finding is about the absence of a fee ceiling. This one is about the deadline having no effect after allocation; it would remain true with a fee ceiling in place, because the transaction would still be broadcast indefinitely past its deadline.

## Remediation options

1. **Fix the contract text first.** `crates/core/src/tx/mod.rs:128-131` should say that `expires_at` prevents _allocation_ and has no effect afterwards, and that a transaction which has been allocated a nonce is resubmitted until it is included regardless of its deadline. This is a one-line change that stops services relying on a guarantee that does not exist, and it should land whatever else is decided.
2. **Stop escalating past the deadline.** Keep resubmitting an expired in-flight transaction — the nonce must still be consumed — but stop bumping once `safe > expires_at`, resubmitting at the last accepted fee. The nonce still clears when the network catches up, without an unbounded spend on an action nobody wants. Tradeoff: a genuinely underpriced expired transaction then takes longer to clear, extending the head-of-line block.
3. **Replace rather than persist.** On expiry, replace the transaction at the same nonce with a minimal self-transfer of zero value. This consumes the nonce cheaply and deterministically, unblocks the queue, and guarantees the abandoned call never lands. Tradeoff: the replacement must itself outbid the original (so it inherits the bump), and there is a race in which the original is included first — acceptable, since that is the status quo outcome.
4. **Report it.** Log at `warn` and expose a counter the first time an in-flight transaction passes its `expires_at`, naming the nonce. Cheap, and it is the only way an operator learns that a deadline-bearing action is still being pushed.
5. **Give the service a say.** Extend `ActionEncoder` so the service can declare what should happen at expiry (drop, freeze the fee, replace), since only the service knows whether a late-landing call is harmless or harmful.

Tests to add: a queue test that allocates a transaction with `expires_at = N`, advances past `N` without executing it, and asserts the intended post-deadline policy (today it would assert continued escalating resubmission); a storage test asserting that `stale_submissions` distinguishes expired from unexpired in-flight rows once such a distinction exists.

## Trail

- Reviewer R3: drafted, self-estimate 70%. Claims 1–8 are `E2` and the divergence between the documented contract and the behaviour is not in doubt. The estimate is held below 85% because a Critic may reasonably rule that the divergence is documentation-only (option 1) rather than a behavioural defect; the escalating-fee-past-deadline half (option 2) is what I consider the substantive part, and its impact depends on onchain deadline checks I have not verified.

## Critic (C-CORE-B)

Derived independently before reading the Claim. `expires_at` appears in exactly three places in `tx/storage.rs`: it is written by `enqueue` (`:96-100`), it gates the _selection_ predicate in `next_transaction` (`:152`), and it gates one of `prune`'s two `DELETE`s — the one restricted to `nonce IS NULL` (`:259-262`). `count_outstanding` also consults it (`:214`) but only for counting. `stale_submissions` (`:293-298`) carries **no** expiry predicate at all. So the deadline gates allocation and nothing else: once a nonce is on the row, the transaction is rebuilt, re-signed and rebroadcast indefinitely, with `fees::bump` raising the fee at least 10% each time.

### Per-claim verdicts

All basis rows **Supported** against the cited ranges. The `expires_at: Some(commit_deadline)` / `Some(reveal_deadline)` sentinel citations are present at `crates/sentinel/src/service.rs:231, 239, 434`, so the "real protocol deadlines" half is verified rather than asserted.

### One qualification to the "contradicts the documented contract" framing

The documented wording is narrower than the title implies. `TransactionQueue::queue` says the transaction is "dropped if it has not been **submitted** by block `expires_at`" (`tx/mod.rs:128-131`), and `ActionEncoder::encode_action` says "the block number after which it should be dropped **if it has not yet been submitted**" (`driver.rs:66-69`). Since `next_transaction` allocates and `submit_transaction` immediately attempts a broadcast, allocation and submission normally coincide, so for the happy path the code does what the doc says. The real gap is the two cases the doc does not cover:

1. a row that is **allocated but never actually enters a mempool** (the broadcast fails and takes the generic branch, `tx/mod.rs:287-293`) — it is "not submitted" by any reasonable reading, yet its deadline can never fire again; and
2. a row that **did** reach the mempool and is then resubmitted forever with no reference to the deadline it was carrying.

Case 2 is the one with teeth, and it is not strictly a contradiction of the doc so much as a silence in it. That is a real defect either way — a deadline-bearing action can land arbitrarily late at an arbitrarily escalated fee — but the finding overstates it slightly by calling it a contradiction. Recommend re-titling to "`expires_at` stops applying once a nonce is allocated, so a deadline-bearing action can be rebroadcast and land arbitrarily late".

### Finding verdict

**Confirmed — 72%.** Unlike its neighbours, this finding needs no provider anomaly and no operator error: the trigger ("allocated at or before `expires_at`, then not included promptly") is reachable from ordinary mempool congestion and is verified entirely in code. The reviewer is right to mark the _impact_ half `I` — whether a late `Reveal` reverts or succeeds is a Solidity question they correctly declined to answer (A7 puts the contracts out of scope for findings). 72 reflects a fully verified mechanism and trigger with an unresolved consequence.

**Severity: Medium (unchanged).** Correct. Not High: the deadline overrun costs gas and a wasted action, not the node's liveness, and A2's "attacker chooses when to act" framing is one inference step beyond what is shown. Not Low: this composes directly with F-CORE-060 (each rebroadcast ratchets the fee) and F-CORE-062 (the row holds an in-flight slot forever), and it defeats a control the services rely on for protocol deadlines.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 1 unconditionally; option 2 is the best behavioural fix; option 3 is riskier than stated.**

Option 1 (fix the contract text at `tx/mod.rs:128-131` to say that `expires_at` prevents _allocation_ and has no effect afterwards) is a one-line change, is unconditionally correct, and should land whatever else is decided. As shipped, a service reading that doc comment reasonably believes a deadline-bearing action will not be broadcast after its deadline — and the sentinel's `commit` and `reveal` are exactly such actions (`crates/sentinel/src/service.rs:230-241`, `:424-437`).

Option 2 (keep resubmitting an expired in-flight transaction — the nonce must still be consumed — but stop _bumping_ once `safe > expires_at`) is the best behavioural fix. It preserves the invariant that makes the queue work (an allocated nonce must clear) while removing the unbounded spend, and it is a one-condition change in `resubmit_stale`. Its tradeoff (a genuinely underpriced expired transaction takes longer to clear, extending the head-of-line block) is real but bounded, and it directly limits **F-CORE-060**'s ratchet for the class of actions that carry deadlines.

Option 3 (replace with a zero-value self-transfer at the same nonce) is sound in outline but riskier than the text allows. The replacement must outbid the original, so it inherits the bump — meaning it is subject to F-CORE-060's ratchet itself — and the race in which the original lands first is not merely "the status quo outcome" for a **sentinel `reveal`**, where a late-landing reveal is _desirable_, not harmless. If option 3 is taken it must be per-action-kind, which is option 5.

Option 5 (let `ActionEncoder` declare the expiry policy: drop, freeze the fee, replace) is the architecturally right answer and is also the hook **F-CORE-067** needs for the reorg case, where a queued-but-unsubmitted action should sometimes be replaced rather than de-duplicated. Two findings, one trait extension.

Option 4 (log at `warn` and count the first time an in-flight transaction passes `expires_at`) is cheap and is the only way an operator currently learns anything about this at all.

## In-flight impact (FWD)

**Pertains to unmerged branches, not to `main`.** Assessed against the "Batched Execution" stack (`origin/feat/batex_0` … `origin/feat/batex_4`, PRs #899–#904) and its epic. **Effect: reshape.** The mechanism is unchanged — the resubmission query still carries no expiry predicate and `prune` still deletes expired rows only while `nonce IS NULL` — but the epic makes the never-expiring row a _designed_ component rather than an accident: the EIP-7702 delegation transaction is specified with `expires_at: None` and the epic's safety argument depends on that ("it is never dropped or pruned while unexecuted, and it is resubmitted with bumped fees until it lands"). On the batching side the epic chose the conservative rule — a new batch starts on _any_ `expires_at` change, explicitly rejecting minimum-expiry grouping because "silently losing actions is worse than a smaller batch" — so no batch inherits a deadline that does not belong to it, and this finding's failure mode is not amplified by mixed expiries. What does change is granularity: a batch is one nonce, so a batch submitted just before its deadline and rebroadcast for hours afterwards lands **all six to eight** of its stale actions at once, at a fee unrelated to any of their values, and the state machine has moved on from all of them. Severity and certainty unchanged. See `rust-audit/report/IN-FLIGHT.md`.
