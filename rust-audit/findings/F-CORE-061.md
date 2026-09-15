# F-CORE-061 `is_transaction_underpriced` only matches replacement rejections, so a first-submission fee rejection retries at an unchanged fee forever and blocks every later nonce

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | core, `tx/mod.rs`, `tx/storage.rs`, `tx/types.rs` |
| Location | `crates/core/src/tx/mod.rs:360-368` (related: `crates/core/src/tx/mod.rs:284-296`, `crates/core/src/tx/storage.rs:285-311`, `crates/core/src/tx/types.rs:61-86`, `crates/core/src/tx/fees.rs:39-42`) |
| Severity | Medium / Medium |
| Certainty | 58% |
| Assumptions involved | A4, A10 |
| Tags | dos, input-validation, fees |

## Claim

The queue decides whether to raise a transaction's fee by regex-matching the node's error string. Both patterns require the rejection to be about a _replacement_: one needs the words "replacement transaction" **and** "underpriced" together, the other is a single vendor-specific sentence. A node that rejects a **first** submission because its fee is below the node's own txpool floor produces neither. That rejection therefore falls into the generic branch, which deliberately does **not** record a fee floor. On the next block the row is rebuilt with `bump(fresh, None)`, which returns the fresh estimate unchanged — so the queue re-signs and re-broadcasts a transaction with the _same_ fee, is rejected identically, and repeats indefinitely.

Because the row already holds an allocated nonce and the queue never releases an allocated nonce, every transaction the service queues afterwards is assigned a strictly higher nonce and cannot be included until this one is. The service goes silent onchain, with nothing but a repeated `warn` line to show for it.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Both match arms require a _replacement_ rejection. A message such as geth's `transaction underpriced` satisfies the second regex but not the first, so the conjunction fails, and it is not the vendor string either. | E2 | `crates/core/src/tx/mod.rs:360-368` | <pre>/// Whether `err` is a node rejection indicating that the transaction's fees<br>/// are too low for the mempool.<br>fn is_transaction_underpriced(err: &TransportError) -> bool {<br> err.as_error_resp.is_some_and(\|payload\| {<br> (iregex!("replacement transaction").is_match(&payload.message)<br> && iregex!("underpriced").is_match(&payload.message))<br> \|\| iregex!("INTERNAL_ERROR: could not replace existing tx").is_match(&payload.message)<br> })<br>}</pre> |
| 2 | The generic branch logs and returns without calling `record_submission`, so neither `submitted_at` nor the fee floor is written. | E2 | `crates/core/src/tx/mod.rs:284-295` | <pre>// Other failures do not establish that the transaction reached the<br>// mempool or that its fees were insufficient. Leave the last<br>// accepted fee floor unchanged and retry without increasing it.<br>Err(err) => {<br> tracing::warn!(<br> nonce = submission.nonce,<br> ?err,<br> "submission failed, will retry without bumping fees"<br> );<br>}<br>}<br>Ok()</pre> |
| 3 | A row whose `submitted_at` is still `NULL` is returned as stale on every call. | E2 | `crates/core/src/tx/storage.rs:293-298` | <pre>sqlx::query_scalar::&lt;_, String&gt;(<br> "SELECT json_set(request, '$.nonce', nonce)<br> FROM transactions<br> WHERE nonce IS NOT NULL AND executed_at IS NULL<br> AND (submitted_at IS NULL OR submitted_at <= ?)<br> ORDER BY nonce ASC",<br>)</pre> |
| 4 | With no stored fees, `AllocatedTransaction::fees` yields `None` and `bump` returns the fresh estimate verbatim — the retry is fee-identical whenever the estimate has not moved. | E2 | `crates/core/src/tx/types.rs:79-86` | <pre>/// The fees the transaction was last submitted with, if it has been<br>/// submitted before.<br>fn fees(&self) -> Option&lt;Eip1559Estimation&gt; {<br> Some(Eip1559Estimation {<br> max_fee_per_gas: self.max_fee_per_gas?,<br> max_priority_fee_per_gas: self.max_priority_fee_per_gas?,<br> })<br>}</pre> |
| 5 | `bump` with no previous submission is the identity on the fresh estimate. | E2 | `crates/core/src/tx/fees.rs:39-42` | <pre>pub fn bump(fresh: Eip1559Estimation, previous: Option&lt;Eip1559Estimation&gt;) -> Eip1559Estimation {<br> let Some(previous) = previous else {<br> return fresh;<br> };</pre> |
| 6 | An allocated nonce is never released: `prune` deletes only executed rows and _queued_ (`nonce IS NULL`) expired rows. A row holding a nonce with `executed_at IS NULL` matches neither statement. | E2 | `crates/core/src/tx/storage.rs:249-265` | <pre>// Prune transactions executed at or below the reorg-safe block.<br>sqlx::query("DELETE FROM transactions WHERE executed_at IS NOT NULL AND executed_at <= ?")<br> .bind(safe)<br> .execute(&mut *tx)<br> .await?;<br><br>// Remove queued (not-yet-submitted) transactions that expired at or<br>// before the reorg-safe block. Never-expiring transactions have a<br>// `NULL` `expires_at` and are excluded explicitly rather than relying<br>// on the fact that SQL comparisons against `NULL` are never true.<br>sqlx::query(<br> "DELETE FROM transactions<br> WHERE nonce IS NULL AND expires_at IS NOT NULL AND expires_at <= ?",<br>)</pre> |
| 7 | Every later transaction is allocated a strictly higher nonce, so it queues behind the stuck one. | E2 | `crates/core/src/tx/storage.rs:144-156` | <pre>let Some(request) = sqlx::query_scalar::&lt;_, String&gt;(<br> "UPDATE transactions<br> SET nonce = MAX(?, COALESCE(<br> (SELECT MAX(nonce) + 1 FROM transactions),<br> 0<br> ))<br> WHERE id = (<br> SELECT id FROM transactions<br> WHERE nonce IS NULL AND (expires_at IS NULL OR expires_at > ?)<br> ORDER BY id ASC<br> LIMIT 1<br> )<br> RETURNING json_set(request, '$.nonce', nonce)",<br>)</pre> |
| 8 | Only the positive cases are tested; there is no test that a non-replacement fee rejection is (or is not) recognised. | E2 | `crates/core/src/tx/mod.rs:424-435` | <pre>#[test]<br>fn identifies_transaction_underpriced_error_messages {<br> for message in [<br> "replacement transaction is underpriced",<br> "rEpLaCeMeNt TrAnSaCtIoN uNdErPrIcEd",<br> "INTERNAL_ERROR: could not replace existing tx",<br> ] {<br> let err =<br> TransportError::err_resp(ErrorPayload::internal_error_message(message.into));<br> assert!(is_transaction_underpriced(&err));<br> }<br>}</pre> |
| 9 | An RPC error is classified as intermittent, so the driver logs it and continues rather than surfacing it — the stall is invisible above `warn`. | E2 | `crates/core/src/tx/mod.rs:47-54` | <pre>fn is_intermittent(&self) -> bool {<br> // Note that we only consider RPC errors as transient - everything else<br> // including SQLite errors (which only happen if you are in a pretty<br> // borked FS situation or there is a bug in the SQL logic) and signing<br> // errors (which indicate some issue with the signer configuration) are<br> // considered more serious.<br> matches!(self, Self::Rpc(_))<br>}</pre> |

## Trigger

The node's transaction-pool minimum price exceeds the fee that alloy's EIP-1559 estimator produces from `eth_feeHistory`, at the moment of a **first** submission for a nonce.

Concretely, an RPC endpoint configured with a price floor above the recent-reward percentile the estimator samples — geth's `--txpool.pricelimit`, Nethermind's `MinGasPrice`, or a hosted provider enforcing its own minimum — rejects the first `eth_sendRawTransaction` for that nonce with a message that says the transaction is underpriced but not that it is a _replacement_. Claim 1 shows the conjunction then fails. Claim 2 shows the fee floor is not recorded. Claims 3–5 show the next block rebuilds the identical transaction. The estimate is recomputed each block, so the loop breaks only if network conditions push the estimate above the node's floor on their own; while the floor stays above the estimate, the loop is unbounded. Claims 6–7 show that every action the service queues in the meantime is stuck behind it, and claim 9 shows the operator sees only repeated `warn` lines.

The same shape reaches the code through any rejection that is _permanent_ and _not_ a replacement-underpriced message. `already known` after a crash between broadcast and `record_submission` is one (harmless once the original mines, but it re-signs and re-broadcasts every block until then); `exceeds block gas limit` for a service-supplied `gas` that is too large is another, and that one never resolves.

I could not verify the exact strings emitted by any client or provider: no dependency sources are on disk and there is no network (A9 is FALSE for this run). The trigger is therefore class `I`; claims 1–9, the code path, are all `E2`.

## Considered and rejected

- **"The fee estimate changes each block, so the retry is not really identical."** The estimate is re-fetched per block (`crates/core/src/tx/mod.rs:322-345`), so it drifts with the base fee. But it drifts with the _network_, not with the node's static pool floor; nothing in the retry path applies pressure toward crossing that floor, which is the whole purpose of the bump the queue declines to apply here. When the floor is above where the estimator sits, the loop does not terminate on its own.
- **"The transaction expires and is dropped."** It does not. `expires_at` gates only allocation (`crates/core/src/tx/storage.rs:152`) and pruning of `nonce IS NULL` rows (claim 6). Once a nonce is allocated the deadline is void — see `F-CORE-064`.
- **"`mark_executed` will clear it."** Only if the account nonce moves past it (`crates/core/src/tx/storage.rs:224-235`), which for a transaction that never reaches any mempool requires some _other_ sender to consume that nonce. With the handbook's guidance that the key is not reused, nothing will.
- **"The in-flight cap protects the queue."** It does the opposite. `submit_pending` allocates nonces up to `max_in_flight_transactions` (`crates/core/src/tx/mod.rs:204-216`); the stuck row occupies one slot permanently and the rows behind it occupy the rest, so the cap converts a single-transaction stall into a queue-wide one.
- **"This is the same defect as `F-CORE-060`."** It is the mirror image. `F-CORE-060` is the case where the match returns `true` too often (the fee ratchets without bound); this is the case where it returns `false` when it should not (the fee never moves at all). They share the string-matching design but have opposite failure modes and different fixes, so they are filed separately.
- **"The generic branch's comment shows the behaviour is intended."** The comment (claim 2) reasons that other failures "do not establish that the transaction reached the mempool or that its fees were insufficient". That is sound for a transport error; it is wrong for a fee rejection the regex simply failed to recognise. The defect is the classification, not the branch.
- **Not a false positive because of the tests.** The only test of this function asserts three positive cases (claim 8). There is no negative case and no end-to-end test in which a first submission is rejected for being underpriced, so nothing in the suite would fail if the classification were wrong — which is consistent with `analysis-core.md` §10's list of untested paths.

## Remediation options

1. **Widen the match, deliberately.** Recognise a first-submission fee rejection as its own case: `underpriced` without `replacement`, plus the common phrasings for a pool floor (`fee too low`, `gas price too low`, `below minimum`, `intrinsic gas`-adjacent messages excluded). Treat it exactly as the replacement case — record the rejected fees as the floor so the next attempt bumps. Tradeoff: still string matching, still brittle; it enlarges the surface that `F-CORE-060`'s ratchet can be driven from, so it should land together with a ceiling.
2. **Prefer the JSON-RPC error code where one exists**, falling back to the string only when the code is generic. Codes are not standardised across clients either, but the pairing is strictly more information than the string alone.
3. **Invert the default.** Treat any `eth_sendRawTransaction` _error response_ (as opposed to a transport-level failure) as evidence that the attempted fees were not accepted, and bump; keep the "retry unchanged" path only for transport errors, where the request may never have been evaluated. This removes the need to enumerate strings for the common case and matches the comment's own reasoning about what a transport error does and does not establish. It requires the ceiling from `F-CORE-060` to be in place first.
4. **Bound the loop regardless of classification.** Count consecutive failed attempts per row; after N, log at `error` and expose a gauge for the oldest unexecuted nonce and its attempt count, so a stalled queue is visible even when the classification is wrong. This is the option that limits the blast radius of every future unrecognised message.

Tests to add: negative cases for `is_transaction_underpriced` (`transaction underpriced`, `already known`, `nonce too low`, `insufficient funds for gas * price + value`) asserting the intended classification of each; and a queue test that answers every submission with a fee rejection lacking the word "replacement", drives ten block statuses, and asserts either that the fee increased or that a terminal error was raised — today it would assert ten identical submissions.

## Trail

- Reviewer R3: drafted, self-estimate 60%. The code path is `E2` and fully traced; the estimate is held down because the trigger depends on client and provider message wording that cannot be checked in this run.

## Critic (C-CORE-B)

Read `is_transaction_underpriced` (`tx/mod.rs:360-368`) first. The predicate is `("replacement transaction" AND "underpriced") OR "INTERNAL_ERROR: could not replace existing tx"`. A rejection carrying only the word "underpriced" satisfies neither disjunct, so it takes the generic branch, which does **not** call `record_submission`; the row keeps `submitted_at IS NULL` and `max_fee_per_gas`/`max_priority_fee_per_gas` unset, so `AllocatedTransaction::fees` returns `None` and `bump(fresh, None)` is the identity (`fees.rs:39-42`). The next block rebuilds and rebroadcasts the same transaction at the same fee. That is precisely the Claim, derived independently.

### Per-claim verdicts

Rows 1-9 all **Supported**, verbatim. Row 8 is a good catch and checks out: `identifies_transaction_underpriced_error_messages` (`tx/mod.rs:424-435`) asserts three positive cases and no negative one, so nothing in the suite would fail if the classification were wrong.

### Where I hold the line

The finding is exactly as strong as its trigger, and the trigger is a **string** no evidence in this checkout can pin. The reviewer says so plainly ("The trigger is therefore class `I`"), which is the correct discipline under A6 and `state/baseline.md` §1 — no client or dependency source is on disk and there is no network. I decline to upgrade it on the strength of what I happen to know about geth's `ErrUnderpriced` wording; that would be exactly the unsourced-recall move the run mode forbids.

What _is_ `E2` and does not depend on any string: the classifier has a two-valued policy over an open-ended message space, the negative branch is a no-op that guarantees an identical retry, and there is no attempt counter or escalation anywhere. Any unrecognised permanent rejection — not only a fee one — produces the same unbounded identical-retry loop.

I also confirm the reviewer's sub-claims about the consequences: an allocated nonce is never released (no statement in `tx/storage.rs` ever writes `nonce = NULL`; `unmark_executed` at `:273-279` clears `executed_at` only), `prune` cannot remove such a row (`:250, 259-262`), and every later allocation is floored above it (`:145-149`), so one stuck row does stall the queue.

### Finding verdict

**Plausible — 58%.** Mechanism `E2` and complete; trigger `I` and unverifiable this run. The 40-69 band is the honest place for it. I set 58 rather than the reviewer's 60 for no material reason beyond the same evidence — this is not a challenge to their calibration.

**Severity: Medium (unchanged).** Correct. The outcome (a silently inert service) is severe, but it is reached through a node-configuration mismatch rather than anything an attacker controls, so the High band's "under attacker-controlled input or reorgs" does not apply.

Correctly filed **separately** from F-CORE-060 rather than merged: same classifier, opposite failure modes (`true` too often vs `false` when it should not be), and incompatible fixes — widening the match here enlarges the surface F-CORE-060's ratchet can be driven from, so remediation 1 must not land without F-CORE-060's ceiling.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 4 unconditionally; options 1 and 3 only after F-CORE-060's ceiling exists.**

The finding is unusually careful about sequencing and it is right to be. **Options 1 and 3 both widen the set of rejections that trigger a fee bump, and the bump is unbounded (F-CORE-060).** Widening the input to an unbounded ratchet before capping the ratchet makes the fund-drain finding worse, not better. Both options say so; the report must carry that condition prominently, because option 3 ("treat any `eth_sendRawTransaction` error response as evidence the fees were not accepted") is the most attractive-looking and the most dangerous in that order.

Option 3 is the soundest classification once the ceiling exists: it stops enumerating vendor strings for the common case and its reasoning about what a _transport_ error does and does not establish matches the comment already in the code. Option 1 (widen the regex set) is strictly more brittle and buys less.

Option 2 (prefer the JSON-RPC error code, falling back to the string) is sound and free — codes are not standardised either, but the pairing is strictly more information than the string alone. Take it with whichever of 1/3.

Option 4 (count consecutive failed attempts per row; after N log at `error` and expose the oldest unexecuted nonce and its attempt count) is the option I would take **first and independently**. It does not depend on the classification being right — which is the whole problem here — and it bounds the blast radius of every future unrecognised message. It is also the same counter **F-CORE-062 option 2** wants, and it makes **F-CORE-060 option 4** nearly free.

**Certainty note:** the trigger is a node's error _wording_, which no dependency source can settle. It needs a client (geth/Nethermind/Erigon) or a testnet, and is recorded as such in `rust-audit/../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md` §2a so it is not mistaken for a crate question. (That entry originally sat in the shared `UNRESOLVED-DEPENDENCY-QUESTIONS.md`; it was destroyed when that file was overwritten and has been restored to QA-CORE-SEN's own file pending the Manager's merge.)

## In-flight impact (FWD)

**Pertains to unmerged branches, not to `main`.** Assessed against the "Batched Execution" stack (`origin/feat/batex_0` … `origin/feat/batex_4`, PRs #899–#904). **Effect: unchanged.** `is_transaction_underpriced` and both of its regexes are absent from the cumulative diff, as is the generic rejection branch that declines to record a fee floor; `bump(fresh, None)` still returns the fresh estimate unchanged, so a first-submission fee rejection still retries at an identical fee forever. The nonce-retention half of the claim is also intact — `crates/core/src/tx/storage.rs` gains a nonce index, a span-aware allocation expression and two delegation helpers, but nothing that releases an allocated nonce. The only forward-looking change in character is scale: once Phase 7 wires batching in (not on any pushed branch), the row that wedges the queue this way holds a _batch_ of six to eight actions rather than one action, so the same defect blocks more work per occurrence while occurring less often. Severity and certainty unchanged. See `rust-audit/report/IN-FLIGHT.md`.
