# F-CORE-060 Underpriced-rejection fee ratchet is unbounded, runs every block, and bypasses `priority_fee_cap_percentage`

| Field | Value |
| --- | --- |
| Status | Verified |
| Crate and module | core, `tx/fees.rs`, `tx/mod.rs`, `tx/storage.rs`, `tx/types.rs` |
| Location | `crates/core/src/tx/fees.rs:52-56` (related: `crates/core/src/tx/mod.rs:224-237, 265-296, 322-345`, `crates/core/src/tx/storage.rs:285-311`, `crates/core/src/tx/types.rs:61-77`) |
| Severity | High / High |
| Certainty | 98% (RW-CORE-SEN, Phase 8 real-world) |
| Assumptions involved | A1, A4, A10 |
| Tags | dos, config, fees |

## Claim

A transaction that the node keeps rejecting as an underpriced _replacement_ has its `max_fee_per_gas` and `max_priority_fee_per_gas` multiplied by 1.1 **on every block**, compounding, with no ceiling of any kind. The configured `priority_fee_cap_percentage` — whose documented purpose is precisely to bound overpayment — is applied only to the fresh estimate and is silently overridden by the bump, so it provides no protection once the ratchet has started. The only brake in the whole system is the signer's own balance: the ratchet stops when the node begins rejecting for insufficient funds, at which point the recorded fee floor is pinned just under `balance / gas_limit`. If that transaction is subsequently included, it pays a priority fee bounded only by the account balance, i.e. a single transaction can consume the validator's or sentinel's entire gas budget.

The 1.1× per **block** rate (rather than per `blocks_before_resubmit` blocks) is the part that makes this materially worse than the design intends: an underpriced rejection writes `submitted_at = NULL`, and the resubmission query treats a `NULL` `submitted_at` as unconditionally stale. On Gnosis' ~5 s blocks (A10) that is ×3.1 per minute and ×10^29 per hour.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The bump raises each fee component to at least 110% of the previous one, compounding, with `saturating_add` as the only bound (so it pins at `u128::MAX` rather than erroring). | E2 | `crates/core/src/tx/fees.rs:52-56` | <pre>/// Returns `fresh`, raised to at least 10% above `previous`.<br>fn bump_fee(fresh: u128, previous: u128) -> u128 {<br> let bumped = previous.saturating_add(previous.div_ceil(10));<br> fresh.max(bumped)<br>}</pre> |
| 2 | The module documents that bumping defeats the priority-fee cap. | E2 | `crates/core/src/tx/fees.rs:38-39` | <pre>/// Note that fee bumps can cause priority fee caps to not be observed.<br>pub fn bump(fresh: Eip1559Estimation, previous: Option&lt;Eip1559Estimation&gt;) -> Eip1559Estimation {</pre> |
| 3 | The cap is applied only to the freshly estimated fees, and the result is cached per block. Nothing re-applies it after the bump. | E2 | `crates/core/src/tx/mod.rs:326-341` | <pre>let fees = self.provider.estimate_eip1559_fees.await?;<br>let fees = match self.config.priority_fee_cap_percentage {<br> Some(cap) => {<br> let capped = cap_priority_fee(fees, cap);<br> if capped.max_priority_fee_per_gas < fees.max_priority_fee_per_gas {<br> tracing::debug!(<br> original = fees.max_priority_fee_per_gas,<br> capped = capped.max_priority_fee_per_gas,<br> "priority fee capped"<br> );<br> }<br> capped<br> }<br> None => fees,<br>};<br>self.fee_cache = Some(fees);</pre> |
| 4 | The bump is applied last, over the already-capped estimate, using the fees persisted from the previous attempt. | E2 | `crates/core/src/tx/types.rs:61-71` | <pre>impl AllocatedTransaction {<br> /// Builds a concrete EIP-1559 transaction for signing, bumping `estimate`<br> /// above any fees from a previous submission so that it replaces it.<br> pub fn build(self, chain_id: u64, estimate: Eip1559Estimation) -> TxEip1559 {<br> let fees = fees::bump(estimate, self.fees);<br> TxEip1559 {<br> chain_id,<br> nonce: self.nonce,<br> gas_limit: self.transaction.gas,<br> max_fee_per_gas: fees.max_fee_per_gas,<br> max_priority_fee_per_gas: fees.max_priority_fee_per_gas,</pre> |
| 5 | An underpriced rejection records the _rejected_ fees as the new floor and deliberately clears the submission block. | E2 | `crates/core/src/tx/mod.rs:271-283` | <pre>Err(err) if is_transaction_underpriced(&err) => {<br> tracing::warn!(<br> nonce = submission.nonce,<br> ?err,<br> "transaction underpriced, will bump fees and retry next block"<br> );<br> self.storage<br> .record_submission(Submission {<br> block: None,<br> ..submission<br> })<br> .await?;<br>}</pre> |
| 6 | `record_submission` binds a `None` block as SQL `NULL` and writes the rejected fees back into the row. | E2 | `crates/core/src/tx/storage.rs:176-186` | <pre>let updated = sqlx::query(<br> "UPDATE transactions<br> SET submitted_at = ?,<br> request = json_set(<br> request,<br> '$.maxFeePerGas', ?,<br>             '$.maxPriorityFeePerGas', ?<br> )<br> WHERE nonce = ?",<br>)<br>.bind(block.map(i64::try_from).transpose?)</pre> |
| 7 | A row with `submitted_at IS NULL` is stale on **every** call, regardless of `blocks_before_resubmit`, so the bump runs once per advancing block. | E2 | `crates/core/src/tx/storage.rs:293-298` | <pre>sqlx::query_scalar::&lt;_, String&gt;(<br> "SELECT json_set(request, '$.nonce', nonce)<br> FROM transactions<br> WHERE nonce IS NOT NULL AND executed_at IS NULL<br> AND (submitted_at IS NULL OR submitted_at <= ?)<br> ORDER BY nonce ASC",<br>)</pre> |
| 8 | `resubmit_stale` runs once per advancing block and rebuilds every stale row. | E2 | `crates/core/src/tx/mod.rs:224-234` | <pre>async fn resubmit_stale(&mut self, block: u64) -> Result&lt;, Error&gt; {<br> let submitted_before = block.checked_sub(self.config.blocks_before_resubmit);<br> let stale = self.storage.stale_submissions(submitted_before).await?;<br> if stale.is_empty {<br> return Ok();<br> }<br><br> for transaction in stale {<br> tracing::debug!(nonce = transaction.nonce, "resubmitting stale transaction");<br> self.submit_transaction(transaction, block).await?;<br> }</pre> |
| 9 | The crate's own test demonstrates the per-block rate: after an underpriced rejection at block 12 leaves the floor at 231/11, block 13 (the very next block) submits 255/13. | E2 | `crates/core/src/tx/mod.rs:703-717` | <pre>queue.update_block_status(block_status(12)).await.unwrap;<br>assert!(asserter.read_q.is_empty);<br>let transaction = in_flight(&queue).await;<br>assert_eq!(transaction.max_fee_per_gas, Some(231));<br>assert_eq!(transaction.max_priority_fee_per_gas, Some(11));<br><br>// The next retry bumps above the rejected fee floor.<br>asserter.push_success(&U64::from(0));<br>asserter.push_success(&fee_history);<br>asserter.push_success(&B256::ZERO);<br>queue.update_block_status(block_status(13)).await.unwrap;<br>assert!(asserter.read_q.is_empty);<br>let transaction = in_flight(&queue).await;<br>assert_eq!(transaction.max_fee_per_gas, Some(255));<br>assert_eq!(transaction.max_priority_fee_per_gas, Some(13));</pre> |
| 10 | One matched rejection string is provider-specific and is not a fee comparison at all, so a provider that emits it for an unrelated reason drives the ratchet indefinitely. | E2 for the match, I for provider behaviour | `crates/core/src/tx/mod.rs:362-368` | <pre>fn is_transaction_underpriced(err: &TransportError) -> bool {<br> err.as_error_resp.is_some_and(\|payload\| {<br> (iregex!("replacement transaction").is_match(&payload.message)<br> && iregex!("underpriced").is_match(&payload.message))<br> \|\| iregex!("INTERNAL_ERROR: could not replace existing tx").is_match(&payload.message)<br> })<br>}</pre> |
| 11 | The operator-facing documentation presents `priority_fee_cap_percentage` as the control that bounds overpayment. | E2 | `crates/validator/validator.sample.toml:72-77` | <pre>[transactions]<br># Optional: caps the priority fee of estimated fees to at most this<br># percentage of the total max fee per gas (see the validator handbook's gas<br># cost tip). Lower values reduce what you can overpay on a bad fee estimate,<br># but risk slower inclusion if the cap ends up below what the network needs.<br># priority_fee_cap_percentage = 95</pre> |
| 12 | Nothing in the submission path consults the signer's balance, and no metric records the queue's fee level. Every failure is a `warn`. | E2 | `crates/core/src/tx/mod.rs:287-293` | <pre>Err(err) => {<br> tracing::warn!(<br> nonce = submission.nonce,<br> ?err,<br> "submission failed, will retry without bumping fees"<br> );<br>}</pre> |

## Trigger

Any condition under which `is_transaction_underpriced` keeps returning `true` for successive replacement attempts. Two concrete instances:

1. **Provider-specific replacement error (A4: rate-limited or inconsistent provider).** A hosted endpoint that answers a replacement attempt with `INTERNAL_ERROR: could not replace existing tx` for a reason other than fee level — a private/bundled mempool, a load-balanced backend that does not hold the original, a rate-limit path that reuses the internal-error code — matches claim 10's third regex on every attempt. Because the fee is never actually the reason, raising it never helps, so the ratchet runs for as long as the transaction is outstanding: block N submits `f`, N+1 submits `1.1f`, N+2 `1.21f`, …. On Gnosis (A10, ~5 s blocks) the fee crosses 10^6× its starting value in about 145 blocks, roughly 12 minutes.
2. **A replacement floor the queue cannot reach in one step.** After a crash between `eth_sendRawTransaction` and `record_submission` the stored floor is `NULL`, so the resubmission uses the bare fresh estimate (`bump(fresh, None) == fresh`, `crates/core/src/tx/fees.rs:39-42`). If the copy already in the mempool carries a much higher fee, every attempt is rejected as an underpriced replacement and the recorded floor climbs 10% per block until it overtakes it. This instance converges, but the _rate_ is still one bump per block, and combined with instance 1 or with a wedged nonce (`F-CORE-062`) it does not.

The damage is realised at the moment of inclusion. Suppose a validator with a 10 xDAI balance and the validator's own `gas: 250_000` (`crates/validator/src/service/action.rs:145`). The node admits a replacement only while `gas_limit * max_fee_per_gas + value <= balance`, so the ratchet stops at `max_fee_per_gas ≈ 4 × 10^13` wei/gas (40,000 gwei) and `max_priority_fee_per_gas` has climbed in lockstep. If the transaction then lands, the tip alone is `min(max_fee - base_fee, max_priority_fee) * gas_used` — up to the full 10 xDAI for one protocol message that should have cost a fraction of a cent. Reaching that ceiling from a 1 gwei estimate takes `log(4×10^4)/log(1.1) ≈ 111` blocks, under ten minutes.

## Considered and rejected

- **"The bump is bounded by `blocks_before_resubmit = 2`, so it is 10% per two blocks."** This is what `analysis-core.md` H6 assumes, and it is wrong for the underpriced path. `record_submission` is called with `block: None` (`crates/core/src/tx/mod.rs:279`), which `storage.rs:186` binds as SQL `NULL`, and `stale_submissions` selects `submitted_at IS NULL OR submitted_at <= ?` (`crates/core/src/tx/storage.rs:297`). The `IS NULL` disjunct ignores `submitted_before` entirely. The crate's own test (claim 9) shows 231 → 255 across a single block.
- **"`bump_fee` will overflow and panic, which at least fails loudly."** It will not: `saturating_add` (`crates/core/src/tx/fees.rs:54`) pins at `u128::MAX`. There is no arithmetic ceiling and no error.
- **"`cap_priority_fee` re-clamps the result."** It does not. `cap_priority_fee` is called exactly once, inside `fees` on the fresh estimate (`crates/core/src/tx/mod.rs:329`), and `bump` is applied afterwards inside `build` (`crates/core/src/tx/types.rs:65`). Grep confirms `cap_priority_fee` has no other call site in the crate.
- **"`bump` could produce `max_priority_fee_per_gas > max_fee_per_gas`, so the transaction would be rejected as malformed and the ratchet would stop."** It cannot. `bump_fee` is monotone non-decreasing in `previous` and takes a per-component `max` (`crates/core/src/tx/fees.rs:43-56`); given `max_fee >= max_priority` in both the fresh estimate and the stored pair, the property is preserved. `cap_priority_fee` also preserves it, returning `max_fee_per_gas: base_fee + max_priority_fee_per_gas` (`crates/core/src/tx/fees.rs:27-30`). So there is no self-limiting malformed-transaction path.
- **"The account balance is checked before signing."** Nothing in `submit_transaction` (`crates/core/src/tx/mod.rs:241-296`) queries a balance; the only RPC calls in the queue are `eth_getTransactionCount`, `eth_feeHistory` (via `estimate_eip1559_fees`) and `eth_sendRawTransaction`. The balance acts as a brake only indirectly, through node rejections that then take the _generic_ branch and freeze the floor at its highest accepted value — which is exactly the worst place to freeze it.
- **"An operator would notice."** `crates/core/src/metrics.rs` exports RPC request counts, block numbers and uncled-block counts. There is no fee, queue-depth or in-flight metric, and both failure branches log at `warn` (`crates/core/src/tx/mod.rs:272, 288`). See Observation O6 in `rust-audit/../state/coverage-logs.md#r3`.
- **False-positive check on claim 9.** The test is `underpriced_replacements_advance_the_fee_floor`; it asserts the values as _intended_ behaviour, so it is evidence about the mechanism, not about a bug the team already knows is a bug. The defect claimed here is the absence of a ceiling, not the presence of a bump.

## Remediation options

1. **Absolute ceiling, configured.** Add `max_fee_per_gas_cap` (and optionally `max_priority_fee_per_gas_cap`) to `tx::Config` and clamp the output of `bump` in `AllocatedTransaction::build`. When the clamp binds, log at `error` and stop resubmitting rather than broadcasting a capped-but-identical transaction (which would be rejected as a non-replacement anyway). Tradeoff: an operator who sets it too low gets a stalled queue instead of an expensive one — which is the safer failure for this system, but it must be visible.
2. **Re-apply the cap after the bump.** Change `AllocatedTransaction::build` to `cap_priority_fee(fees::bump(estimate, self.fees), cap)`, making `priority_fee_cap_percentage` mean what its documentation says. This bounds the _tip_ as a fraction of the total fee but not the total fee itself, so it should be combined with option 1. Note that capping after bumping can lower `max_priority_fee_per_gas` below the previous submission's, which some clients reject as a non-replacement; the queue would then need to treat that rejection as terminal rather than as another bump.
3. **Relative ceiling.** Bound the bump to a multiple of the current fresh estimate, e.g. `bumped.min(fresh.saturating_mul(BUMP_CEILING_MULTIPLE))`. Cheap and self-adjusting to network conditions, and it makes a genuinely-underpriced transaction still recover while a falsely-underpriced one stops climbing after a bounded overshoot.
4. **Bound the number of consecutive underpriced rejections.** Track an attempt counter on the row; after N consecutive underpriced rejections stop bumping, log at `error` and expose a gauge. This is the only option that also addresses instance 1 of the trigger, where the rejection is not about fees at all.
5. **Decouple the rate from the branch.** Give the underpriced branch a `submitted_at` of the current block (so `blocks_before_resubmit` applies) and add a separate `retry_immediately` flag if the immediate retry is genuinely wanted. As it stands the `NULL` sentinel silently conflates "never submitted" with "rejected as underpriced", giving the second the retry cadence of the first.

Tests to add: (a) a queue test that accepts the first submission, then answers every subsequent `eth_sendRawTransaction` with `replacement transaction underpriced`, drives 60 block statuses and asserts a bound on the resulting `max_priority_fee_per_gas`; (b) a `fees.rs` unit test asserting that a configured cap still binds after a bump; (c) a test asserting the _rate_ — that an underpriced rejection does not make a row eligible for resubmission more often than `blocks_before_resubmit` — which would have caught the conflation in the first place.

## Trail

- Reviewer R3: drafted, self-estimate 75%. Mechanism is `E2` throughout and corroborated by the crate's own test; the trigger's first instance depends on provider behaviour that cannot be verified offline (no dependency sources, no network), which is what holds the estimate below 85%.

## Critic (C-CORE-B)

I traced the fee path before reading the Claim and arrived at the same, non-obvious, load-bearing detail: the underpriced branch calls `record_submission` with `block: None` (`tx/mod.rs:277-282`), `record_submission` binds that as SQL `NULL` into `submitted_at` (`tx/storage.rs:186`) while writing the _rejected_ fees into the request JSON, and `stale_submissions`' predicate is `(submitted_at IS NULL OR submitted_at <= ?)` (`tx/storage.rs:297`). The `IS NULL` disjunct ignores `submitted_before` entirely, so the row is stale on **every** advancing block, not every `blocks_before_resubmit` blocks. `AllocatedTransaction::build` then applies `fees::bump` _after_ `fees` has applied the cap, and `bump_fee` is `max(fresh, previous + previous/10)` with `saturating_add` (`tx/fees.rs:53-56`). Nothing re-applies the cap and nothing bounds the result.

### Per-claim verdicts

Rows 1-9, 11 and 12 all **Supported**, verbatim at the cited ranges. Row 9's test citation is exact: `crates/core/src/tx/mod.rs:703-717` shows 231/11 at block 12 and 255/13 at block 13 — a single block apart, which settles the rate empirically from inside the crate. Row 10 is correctly split (`E2` for the match, `I` for provider behaviour); the third regex `INTERNAL_ERROR: could not replace existing tx` is verified present at `tx/mod.rs:366`. Nothing is `H`.

I independently confirmed the two negatives the reviewer used to close off escape hatches:

- `cap_priority_fee` has exactly one call site, inside `fees` at `tx/mod.rs:329`, and `bump` is applied later inside `build` (`types.rs:65`). The cap genuinely cannot re-clamp the bump.
- `bump` cannot produce `max_priority > max_fee`: `bump_fee` is monotone per component and `cap_priority_fee` returns `max_fee = base_fee + max_priority` (`fees.rs:27-30`), so no self-limiting malformed-transaction path exists. `fees.rs:38-39`'s own doc comment concedes the cap bypass.

### One correction the reviewer should absorb

The ratchet advances **only when `record_submission` runs**, i.e. on an accepted submission or an underpriced rejection. The generic branch (`tx/mod.rs:287-293`) records nothing, so once the node starts refusing for insufficient funds the stored floor freezes at the last accepted value rather than continuing to climb. That is exactly what the reviewer says in Considered-and-rejected ("freeze the floor at its highest accepted value"), and it is the right description — but it means the _ratchet_ is bounded by `balance / gas_limit` rather than by `u128::MAX`. Row 1's "pins at `u128::MAX`" is true of the arithmetic and not of the reachable state. This does not soften the finding: freezing just under the balance limit is the worst place to freeze, because the tip actually paid on inclusion is `min(max_priority, max_fee - base_fee) * gas_used`, so a single included transaction can consume the account's whole gas budget.

### A4 check

Trigger instance 1 needs a provider that answers a replacement attempt with a non-fee internal error — an erroring or inconsistent provider, in scope. Instance 2 needs no provider fault at all (a crash between broadcast and `record_submission`). Neither requires a malicious RPC. On the right side of the line.

### Finding verdict

**Confirmed — 82%.** Mechanism `E2` end to end and corroborated by the crate's own test; the per-block rate is proved rather than argued. Held below 85 only because the _starting_ trigger of the unbounded instance (a provider emitting the vendor string for a non-fee reason) is `I`, and because instance 2 converges on its own — the genuinely unbounded case needs a transaction that stays unincludable, which in practice means composing with F-CORE-062's wedge.

**Severity: High (unchanged).** This is the one finding in either reviewer's set that reaches the "unbounded fund drain through gas" line in PROMPT.md §8. It is not merely an overpayment: a stuck lower nonce makes every in-flight row ratchet in lockstep, and the loss is realised in full at inclusion. Confirmed High.

## QA (QA-CORE-SEN)

**Outcome: Reproduced by inspection.** Not executed — no Rust toolchain (`state/baseline.md` §1). The fee values quoted below were derived by evaluating `bump_fee`'s recurrence by hand, not by running anything, and are labelled as such in the PoC.

**PoC written:** `rust-audit/poc/F-CORE-060/` — `poc_tx_queue.rs` (three tests) plus a README. Tests 1 and 2 go into `crates/core/src/tx/mod.rs`'s `mod tests`; **test 3 goes into `crates/core/src/tx/storage.rs`'s `mod tests`** (`TransactionStorage`, `Submission` and `Status` are in the private `tx::storage` module and are not nameable from `tx/mod.rs`'s test module).

### Reproduced by inspection — all three claims

**Unbounded.** `bump_fee` is `previous.saturating_add(previous.div_ceil(10))` then `fresh.max(bumped)` (`tx/fees.rs:52-56`). There is no ceiling argument, no config lookup and no caller-side clamp: `AllocatedTransaction::build` is `let fees = fees::bump(estimate, self.fees);` and uses the result directly (`tx/types.rs:61-77`). The only `saturating_` is against `u128` overflow.

**Bypasses the cap.** `TransactionQueue::fees` applies `cap_priority_fee` to the **fresh** estimate and caches it (`tx/mod.rs:322-345`); `build` then bumps against the **previous submission's** stored floor. The cap is never re-applied. `fees.rs:37` says so itself in a doc comment — "Note that fee bumps can cause priority fee caps to not be observed" — which is the crate acknowledging the defect in prose rather than in code.

**Per block, not per `blocks_before_resubmit`.** This is the multiplier and it is exactly as claimed. The underpriced arm writes `Submission { block: None, ..submission }` (`tx/mod.rs:274-282`), and `stale_submissions`' predicate is `submitted_at IS NULL OR submitted_at <= ?` (`tx/storage.rs:293-296`) — the `NULL` short-circuits the block comparison entirely. I verified the call cadence against `update_block_status` (`tx/mod.rs:144-199`): one `nonce` per new latest, then `resubmit_stale(latest)` → `submit_transaction` → `fees` + `send_raw_transaction`. So a row in the underpriced state is rebuilt and rebroadcast on **every** block.

Hand-evaluating the recurrence from the crate's own fixture estimate (max_fee 210, priority 10) over the 59 bumps the PoC drives gives **priority 4,037 / max_fee 59,550** — 404x and 284x the honest estimate, in 60 blocks. The honest estimate never moves.

**Certainty: unchanged at 82%.** No new evidence; 89% is the ceiling. Severity High is right: it is an unbounded fund drain whose only brake is the signer's balance, and A4 supplies the trigger without needing a malicious node.

**One thing the PoC exposed that the finding does not state.** A **ratio** cap cannot bound this even if it were re-applied, because a uniform 1.1x bump of both components preserves the ratio. The cap is only _observably_ violated when `cap_priority_fee` binds hard enough that `bump_fee`'s `div_ceil(10)` lifts a small priority fee by more than 10% while the large max fee rises by exactly 10% (with `priority_fee_cap_percentage = 1.0` the realised ratio reaches ~3.17% after 29 bumps). This matters for remediation, below.

### Remediation check

**Option 2 (re-apply the cap after the bump) is NOT a fix for the main claim and must not be presented as one.** `priority_fee_cap_percentage` is a _ratio_ of the priority fee to the total max fee. Re-applying it after the bump constrains the split between tip and base-fee component; it places **no bound whatsoever on the absolute fee**, which is what drains the balance. The finding already says option 2 "should be combined with option 1"; I would go further and say option 2 alone leaves the finding fully open. Its other stated hazard is also real and sharper than written: capping after bumping can lower `max_priority_fee_per_gas` **below** the previous submission's, which clients reject as a non-replacement — so the queue would then classify its own capped transaction as underpriced and bump it again, i.e. option 2 implemented naively **creates a new ratchet loop**.

**Option 1 (absolute configured ceiling) is sound and is the one that closes the claim.** Its "stalled queue instead of an expensive one" tradeoff is the right direction. Two conditions: the clamp has to be applied in `AllocatedTransaction::build` (the single place `bump` is called), and when it binds the queue must **stop resubmitting** rather than rebroadcast a capped-but-identical transaction — the option says this and it is important, because an identical rebroadcast is rejected as a non-replacement, which `is_transaction_underpriced` may or may not match, which is F-CORE-061.

**Option 3 (relative ceiling — bound the bump to a multiple of the fresh estimate) is sound and is my preferred cheap fix.** It is self-adjusting to network conditions, needs no new config, and preserves the recovery behaviour the bump exists for: a genuinely underpriced transaction still climbs to a working fee, while a _falsely_ underpriced one stops after a bounded overshoot. One line in `build`.

**Option 4 (bound consecutive underpriced rejections) is the only option that addresses trigger instance 1**, where the rejection has nothing to do with fees and raising them can never help. It is complementary to 1 and 3, not an alternative, and it needs a new column. Take it.

**Option 5 (decouple the rate from the branch) is sound and has a cross-finding payoff the text does not mention.** Giving the underpriced branch a real `submitted_at` plus an explicit `retry_immediately` flag removes the `NULL` overload — and that same overload is what makes **F-CORE-067 option 3 unsound** (rolling back "never submitted" queue rows on an `Uncle` would delete rows that are actually in a mempool) and what forces **F-CORE-062 option 3** to guard on a condition the schema cannot express. One column fixes the rate here and unblocks two other remediations. **Recommend option 5 be sequenced first.**

**Recommendation:** option 5, then option 3 (or 1), plus option 4. Option 2 only alongside 1 or 3, and never alone. None of these touches `apply_transition` or the effect system, so the `core::state` contract is unaffected.

## Verification (V-CORE-SEN, Phase 5)

**Executed. Reproduced (all three parts), with every predicted number matching exactly. `E1`.**

`rust-audit/poc/F-CORE-060/poc_tx_queue.rs` was split as its README directs — parts 1 and 2 into the existing `#[cfg(test)] mod tests` of `crates/core/src/tx/mod.rs`, part 3 into that of `crates/core/src/tx/storage.rs` — and run with

```
cargo test -p safenet-core --lib poc_f_core_060
```

**No mechanical repair was needed**; the PoC compiled unmodified and no assertion was altered. Both files were reverted with `git checkout --`. Full output in `rust-audit/poc/F-CORE-060/RESULT-V-CORE-SEN.out`.

### Verbatim result

```
running 3 tests
test tx::tests::poc_f_core_060_underpriced_ratchet_is_unbounded_and_per_block ... FAILED
test tx::tests::poc_f_core_060_priority_fee_cap_does_not_survive_the_bump ... FAILED
test tx::storage::tests::poc_f_core_060_underpriced_rejection_ignores_blocks_before_resubmit ... FAILED

---- poc_f_core_060_underpriced_ratchet_is_unbounded_and_per_block stdout ----
thread '...' panicked at crates/core/src/tx/mod.rs:829:5:
after 60 blocks of underpriced rejections the priority fee ratcheted to 4037 wei/gas (max_fee 59550)
from an unchanged honest estimate of 10 — compounding once per BLOCK, with no ceiling of any kind.
Expected value on this checkout: 4_037 / 59_550.

---- poc_f_core_060_priority_fee_cap_does_not_survive_the_bump stdout ----
thread '...' panicked at crates/core/src/tx/mod.rs:919:5:
priority_fee_cap_percentage = 1.0 was configured, but after 29 bumped resubmissions
max_priority_fee_per_gas = 104 against max_fee_per_gas = 3277 — the cap is applied only to the fresh
estimate (tx/mod.rs:322-345) and is silently overridden by AllocatedTransaction::build's bump
(tx/types.rs:61-77). Expected value on this checkout: 104 / 3_277, i.e. 3.17% against a 1% cap.

---- poc_f_core_060_underpriced_rejection_ignores_blocks_before_resubmit stdout ----
thread '...' panicked at crates/core/src/tx/storage.rs:553:5:
an underpriced rejection at block 10 made the row eligible again at block 11, one block later,
despite blocks_before_resubmit = 2 — the NULL submitted_at sentinel conflates 'never submitted' with
'rejected as underpriced' (tx/storage.rs:285-311), giving the ratchet a per-block cadence:
[AllocatedTransaction { nonce: 0, transaction: Transaction { to: 0x5ff1…2789, value: 0, data: 0x01,
gas: 21000 }, max_fee_per_gas: Some(210), max_priority_fee_per_gas: Some(10) }]

test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 97 filtered out
```

### What is now established by execution rather than by reading

1. **Unbounded, and per block.** Sixty blocks of `"replacement transaction underpriced"` responses take the priority fee from **10 to 4,037** wei/gas and the max fee from **210 to 59,550**, while the honest estimate never moves off 10/210. Both numbers match the hand-derived recurrence `previous + previous.div_ceil(10)` (`tx/fees.rs:52-56`) **exactly**, which is worth stating: the finding's arithmetic was correct, not merely plausible. There is no ceiling in the code path.
2. **The cap does not survive the bump.** With `priority_fee_cap_percentage = 1.0` configured, the realised ratio after 29 bumps is **104 / 3,277 = 3.17 %** — more than triple the configured cap. `cap_priority_fee` is applied to the fresh estimate only (`tx/mod.rs:322-345`) and is then overridden by `AllocatedTransaction::build`'s bump (`tx/types.rs:61-77`).
3. **The `submitted_at IS NULL` conflation, executed.** This is the part with consequences beyond this finding. An underpriced rejection writes `submitted_at = NULL`, and `stale_submissions`' predicate is `submitted_at IS NULL OR submitted_at <= ?` (`tx/storage.rs:293-296`), so the `NULL` **short-circuits the block comparison entirely**: a row rejected at block 10 is eligible again at block 11 despite `blocks_before_resubmit = 2`. `NULL` therefore means both "never submitted" and "rejected as underpriced", and no query in the module can tell them apart. Any remediation that assumes it can distinguish the two states — including three proposed elsewhere in this audit — is unsound as written until a distinct rejection marker (a `rejected_at` column, or a nullable `attempts` counter) is added. **Treat this as the load-bearing result of the three.**

### Bonus: question 11 of `rust-audit/poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md`, settled

The README noted the starting fee level came from the tests' mock rather than from the real estimator. That is now answered — see `rust-audit/poc/Q11-fee-estimator/`. `estimate_eip1559_fees` issues exactly one `eth_feeHistory(0x0a, "latest", [20.0])` (`alloy-provider-2.0.5/src/provider/trait.rs:276-306`, `EIP1559_FEE_ESTIMATION_PAST_BLOCKS = 10`, `EIP1559_FEE_ESTIMATION_REWARD_PERCENTILE = 20.0`) and, when `reward` is empty (`Some([])`, `None`, or all-zero rewards, all three executed), returns

```
Eip1559Estimation { max_fee_per_gas: 201, max_priority_fee_per_gas: 1 }
```

for a base fee of 100: `estimate_priority_fee` floors at `EIP1559_MIN_PRIORITY_FEE = 1` (`alloy-provider-2.0.5/src/utils.rs:26`, `:93-107`) and the max fee is `base_fee * EIP1559_BASE_FEE_MULTIPLIER(2) + priority`. So the real starting level on an empty-reward chain is **conservative — 1 wei of priority fee**, not high. That makes the _absolute_ wei figures in part 1 an over-estimate by roughly an order of magnitude, and it does **not** soften the finding: the ratchet compounds off the _previous submission_, never off the estimate, so a 1-wei honest floor makes the divergence between the honest estimate and the ratcheted fee larger, not smaller. If the report quotes absolute wei, quote them from part 1's mock and say it is a mock.

### Residual uncertainty

The five-minutes-of-Gnosis framing rests on A10's block time, not on execution, and no anvil is available to confirm a real node's rejection wording beyond `is_transaction_underpriced`'s own matcher (`tx/mod.rs:359-365`, covered by the crate's existing test).

**Basis class:** `E1`. **Certainty: 82% → 97%. Status: Critiqued → Verified.** Severity unchanged (High / High).

## Real-world validation (Phase 8, RW-CORE-SEN)

**Verdict: Reproduced end-to-end** for the ratchet, its per-block rate and the `priority_fee_cap_percentage` bypass, measured against a real Anvil fee market. **Not testable locally** for the "signer's balance is the only brake" sub-claim — see below.

### Scenario

Local Anvil only (`http://127.0.0.1:8645`, chain 31337, 1 s blocks). The fee market, `eth_feeHistory`, base fee, blocks and gas are all real Anvil; the sentinel is the real `target/debug/sentinel` binary driving a real `SentinelOracle`. Its effective `rpc` was `http://127.0.0.1:8648`, a local proxy in front of that Anvil, printed and asserted local before start. No sample config, no live endpoint.

The proxy forwards everything untouched **except** `eth_sendRawTransaction`, which it answers with Anvil's own verbatim rejection — `-32003 "replacement transaction underpriced"`, the exact string and code Anvil itself produced in a separate direct test — emulating a node whose pool already holds a higher-fee transaction at that nonce. Config: `priority_fee_cap_percentage = 1`.

### Verbatim outcome — run `s6` (signer funded 1 ETH, ~80 blocks)

```
submit#1   nonce=0 tip=1     maxfee=66905347
submit#3   nonce=0 tip=2     maxfee=73595882
submit#19  nonce=0 tip=10    maxfee=157759315
submit#59  nonce=0 tip=94    maxfee=1061325807
submit#159 nonce=0 tip=11527 maxfee=124589942081
```

160 submissions across nonces 0 and 1, **exactly one bump per nonce per block**, over ~80 blocks. Sentinel log: `160 × "transaction underpriced, will bump fees and retry next block"`, `0 ×` any other submission failure. Anvil's real base fee at the end of the run: **772 wei/gas**.

- Priority fee: **1 → 11,527 wei** (×11,527).
- Max fee per gas: **66,905,347 → 124,589,942,081 wei ≈ 124.6 gwei**, against a real base fee of 772 wei — a factor of ~1.6 × 10^8 over the market rate.
- `priority_fee_cap_percentage = 1` admits a priority fee of at most `772 × 1/99 ≈ 7 wei`. The observed 11,527 is **~1,600× the configured cap**. The cap was silently bypassed exactly as claimed.

### Verbatim outcome — run `s6b` (signer funded 0.001 ETH, ~110 blocks)

```
submit#219 nonce=0 tip=201207 maxfee=4238607810118
```

220 underpriced warnings, 0 other failures, still climbing when the run ended: priority fee **201,207 wei** (~28,700× the 1% cap) and max fee **≈ 4,239 gwei**.

### On the Phase 5 mock's absolute-wei overstatement

Phase 5 measured 10 → 4,037 over 60 blocks and noted the mock overstated absolute wei by ~10×. The correction runs the other way here: against Anvil's real (very low) base fee the _starting_ estimate is much smaller but the _ceiling_ reached is far larger — 124.6 gwei and then 4,239 gwei — because the ratchet is multiplicative and the market rate is irrelevant to it. The **ratio** is what the mock got right: 1.1× per block per nonce, confirmed exactly.

### What could not be shown locally, and why

The claim that the signer's balance is the _only_ brake was **not observable in this rig**. The rejection has to come from the node for the ratchet to run, and the proxy answers before Anvil can apply its balance check; letting real submissions through means Anvil simply mines them and the ratchet stops. Over 110 blocks with the signer funded at 0.001 ETH the ratchet showed **no other brake of any kind** — no cap, no ceiling, no back-off — which is the whole of the claim that is locally checkable. Demonstrating the balance floor itself would need a node that both persistently rejects a valid replacement and applies its own funds check, which no local Anvil will do.

### One mitigating fact, measured directly

Against a real Anvil, a replacement that raises **both** `maxFeePerGas` and `maxPriorityFeePerGas` by exactly 10% — which is precisely what `fees::bump` produces (`previous + previous.div_ceil(10)`) — is **accepted**:

```
initial      tip=100 maxfee=1000000000  -> accepted
replacement  tip=110 maxfee=1000000000  -> -32003 replacement transaction underpriced
replacement  tip=110 maxfee=1100000000  -> accepted
```

So the ratchet does **not** self-start on a healthy node: the sentinel's own bumps are always large enough. It needs the recorded fee floor to be _behind_ what the pool actually holds — a database restored from backup (which the sample config explicitly instructs operators to do), a foreign transaction occupying the nonce, or a stricter node implementation. Once any one of those produces a single underpriced rejection, `submitted_at = NULL` makes the row unconditionally stale and the per-block ratchet above runs unopposed.

**Certainty: 97% → 98%.** Severity unchanged (High / High) — the ratchet, its rate and the cap bypass are all confirmed against a real fee market; the trigger condition is narrower than the claim implies and is recorded above.

## In-flight impact (FWD)

**Pertains to unmerged branches, not to `main`.** Assessed against the "Batched Execution" stack (`origin/feat/batex_0` … `origin/feat/batex_4`, PRs #899–#904) and its epic `epics/2026_09_09_safenet_7702_executor_tx_batching.md`. **Effect: unchanged, with a new trigger.** `crates/core/src/tx/fees.rs` does not appear in the cumulative `origin/main`…`origin/feat/batex_4` diff at all, so `fees::bump` and its unbounded 1.1× ratchet are byte-identical; the `priority_fee_cap_percentage` bypass is likewise untouched. The one change in the fee path is cosmetic: `submit_transaction` now computes `AllocatedTransaction::bumped_fees(estimate)` _before_ calling `build`, and records the `Submission` from that value instead of reading `max_fee_per_gas` / `max_priority_fee_per_gas` back off the built `TxEip1559` (`tx/mod.rs`, `tx/types.rs`). Both produce the same numbers, so `record_submission` writes the same fee floor and the ratchet behaves identically. What the stack _adds_ is a new way to keep transactions in the state the ratchet feeds on: the Phase 4 two-nonce delegation reservation can write a permanent nonce gap (see the new `F-CORE-069`), above which transactions stay in flight, stay stale-eligible, and are resubmitted with compounding bumps indefinitely while being permanently unincludable. The dangerous moment is the _repair_: an operator who closes the gap by hand releases the whole stranded backlog at once at whatever fee the ratchet has reached, bounded only by the signer balance — i.e. this finding's worst case, reached through the recovery action rather than through the failure. Severity and certainty unchanged. See `rust-audit/report/IN-FLIGHT.md`.
