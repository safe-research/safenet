# F2-CORE-060 Replacement fee escalation is geometric with no absolute ceiling; the signer balance is the only brake and the persisted floor never comes down

| Field | Value |
| --- | --- |
| Status | QA'd |
| Crate and module | safenet-core, tx/fees.rs, tx/mod.rs, tx/storage.rs, tx/types.rs |
| Location | crates/core/src/tx/fees.rs:52-56 (related: tx/mod.rs:224-237, 241-296; tx/storage.rs:174-202, 285-311; tx/types.rs:64-77) |
| Severity | Medium (reviewer) / Medium (Critic) |
| Certainty | 92% (QA2-CORE; Critic C2-CORE-B set 72%) |
| Assumptions involved | A1, A4 |
| Tags | dos, config |

## Claim

Every `blocks_before_resubmit` blocks (default 2) an in-flight transaction that has not executed is re-signed with both fee components raised to at least 110% of the last _accepted_ values, and the accepted values are persisted as the new floor. Nothing in configuration or code bounds this: there is no absolute `max_fee_per_gas` ceiling, the priority-fee cap is a ratio that the bump preserves rather than a limit, and the floor is only ever raised (mempool acceptance or an "underpriced replacement" rejection), never lowered. A transaction that the RPC node keeps accepting but that cannot be mined for a while -- a nonce gap in front of it, an RPC backend that acknowledges but does not propagate, a partition of the node from block producers -- therefore compounds at 1.1 per two blocks: on Gnosis (5 s blocks) x1.77 per minute, x304 after ten minutes, x9.3e4 after twenty. Every other in-flight row behind it compounds in lockstep, since staleness is per row. When propagation resumes the transactions mine at the escalated tip and the loss is bounded only by the signer's balance (geth-family RPCs additionally enforce `--rpc.txfeecap`, 1 native token per transaction by default; Nethermind, which dominates Gnosis, has no equivalent).

A second-order effect makes the episode sticky: the floor recorded in the row's `request` JSON is never reduced, and every future attempt is `max(fresh, 1.1 x floor)`. If the pooled transaction is later dropped by the node (pool eviction, node restart -- remote transactions are not journaled by geth) while its recorded floor already exceeds what the node will accept ("insufficient funds", "exceeds the configured cap"), every resubmission is rejected on the generic path, the floor is frozen at the unacceptable value, the row keeps its nonce and every later nonce is blocked. There is no code path that lowers a floor or replaces a row with a no-op, so the stall is permanent without out-of-band intervention.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Each bump raises both components to at least 110% of the previous accepted value with no upper bound other than `u128` saturation. | E2 | crates/core/src/tx/fees.rs:52-56 | `/// Returns \`fresh\`, raised to at least 10% above \`previous\`.`/`fn bump_fee(fresh: u128, previous: u128) -> u128 {`/`let bumped = previous.saturating_add(previous.div_ceil(10));`/`fresh.max(bumped)` |
| 2 | The bump is applied on every build from the persisted previous fees; the crate itself notes the cap is not observed across bumps. | E2 | crates/core/src/tx/types.rs:64-66 and tx/fees.rs:38 | `pub fn build(self, chain_id: u64, estimate: Eip1559Estimation) -> TxEip1559 {` / `let fees = fees::bump(estimate, self.fees());` ... `/// Note that fee bumps can cause priority fee caps to not be observed.` |
| 3 | A row is stale, and therefore rebuilt and rebroadcast, once `submitted_at <= latest - blocks_before_resubmit`; all such rows are resubmitted, not just the head of the nonce sequence. | E2 | crates/core/src/tx/mod.rs:224-234 | `let submitted_before = block.checked_sub(self.config.blocks_before_resubmit);` / `let stale = self.storage.stale_submissions(submitted_before).await?;` ... `for transaction in stale {` / `tracing::debug!(nonce = transaction.nonce, "resubmitting stale transaction");` / `self.submit_transaction(transaction, block).await?;` |
| 4 | Stale selection is per row, ordered by nonce, with no dependency on whether the lower nonce has executed. | E2 | crates/core/src/tx/storage.rs:294-298 | `"SELECT json_set(request, '$.nonce', nonce)` / `FROM transactions` / `WHERE nonce IS NOT NULL AND executed_at IS NULL` / `AND (submitted_at IS NULL OR submitted_at <= ?)` / `ORDER BY nonce ASC",` |
| 5 | Mempool acceptance persists the bumped fees as the new floor, so the next bump compounds on them. | E2 | crates/core/src/tx/mod.rs:249-256, 265-266 | `let submission = Submission {` / `block: Some(block),` / `nonce: transaction.nonce,` / `fees: Eip1559Estimation {` / `max_fee_per_gas: transaction.max_fee_per_gas,` ... `match self.provider.send_raw_transaction(signed.as_raw()).await {` / `Ok(_) => self.storage.record_submission(submission).await?,` |
| 6 | The floor is written into the row and read back on every later build; there is no code path that lowers it. | E2 | crates/core/src/tx/storage.rs:176-184 | `"UPDATE transactions` / `SET submitted_at = ?,` / `request = json_set(` / `request,` / `'$.maxFeePerGas', ?,` / `'$.maxPriorityFeePerGas', ?` / `)` / `WHERE nonce = ?",` |
| 7 | Generic rejections (insufficient funds, fee cap exceeded, and so on) leave the floor and the nonce reservation in place and retry next block at `max(fresh, 1.1 x floor)` again. | E2 | crates/core/src/tx/mod.rs:284-293 | `// Other failures do not establish that the transaction reached the` / `// mempool or that its fees were insufficient. Leave the last` / `// accepted fee floor unchanged and retry without increasing it.` / `Err(err) => {` / `tracing::warn!(` / `nonce = submission.nonce,` / `?err,` / `"submission failed, will retry without bumping fees"` |
| 8 | Existing tests demonstrate the compounding step: 210/10 -> 231/11 -> 255/13 across two accepted or underpriced-rejected resubmissions. | E1 | crates/core/src/tx/mod.rs:705-717 (run this session: `cargo test -p safenet-core --lib tx::` 20 passed) | `assert_eq!(transaction.max_fee_per_gas, Some(231));` / `assert_eq!(transaction.max_priority_fee_per_gas, Some(11));` ... `assert_eq!(transaction.max_fee_per_gas, Some(255));` / `assert_eq!(transaction.max_priority_fee_per_gas, Some(13));` |
| 9 | The only fee configuration is the ratio cap; there is no absolute ceiling field. | E2 | crates/core/src/tx/mod.rs:71-83 | `pub struct Config {` / `pub max_in_flight_transactions: usize,` / `pub blocks_before_resubmit: u64,` / `/// Caps the priority fee of estimated fees to at most this percentage of the` / `/// total max fee per gas, ...` / `pub priority_fee_cap_percentage: Option<f64>,` |
| 10 | The operator documentation presents the ratio cap as protection against runaway fees; the compounding path is not documented anywhere. | I | docs/validator-handbook.md:65, docs/sentinel-handbook.md:57 | `setting \`priority_fee_cap_percentage = 95\` ensures the tip never exceeds 95% of \`maxFeePerGas\`, protecting against runaway estimates while still allowing normal inclusion.` |

## Trigger

1. Any accepted-but-unminable episode. Concretely: row N is rejected by the node for a reason the queue treats as generic (for example the account balance covers a 100k-gas action but not a 400k-gas one, see the `gas` constants in `crates/validator/src/service/action.rs:145-375`), while row N+1 (cheaper) is accepted into the node's queued pool as a future-nonce transaction. Row N retries every block with the fresh estimate (no floor, Basis 7) and stays rejected; row N+1 is stale every two blocks and is accepted each time with fees x1.1 (Basis 1, 3, 5). After k resubmissions its floor is 1.1^k times the estimate. Once the operator tops up, N mines at a normal fee and N+1 mines at its escalated tip; the tip paid is bounded by the balance check at acceptance time, not by any configuration.
2. Same escalation for an RPC endpoint that acknowledges `eth_sendRawTransaction` but does not propagate for some minutes (public load-balanced endpoints, of which the shipped samples use one: `rpc = "https://rpc.gnosischain.com"`), or a node briefly partitioned from producers. On reconnect all in-flight rows mine at their escalated fees.
3. Sticky stall: after such an episode the pooled transaction is dropped (node restart); the next attempt at 1.1 x floor is rejected (insufficient funds / fee cap), the floor freezes there (Basis 7), the row never clears and blocks all later nonces.

## Considered and rejected

- _The priority-fee cap bounds the bump._ It does not. `cap_priority_fee` (fees.rs:12-31) enforces `p / (base + p) <= c` on the fresh estimate only; `bump` (fees.rs:39-50) then raises each component independently. Because `max(fresh_p, 1.1 prev_p) / max(fresh_mf, 1.1 prev_mf) <= c` whenever both inputs satisfy the ratio, the ratio is preserved through bumps (up to wei-scale rounding of `div_ceil`), so the lead's sub-claim "the cap does not apply to bumps" is wrong in ratio terms -- but a ratio bounds nothing in absolute terms, which is the point of this finding.
- _The fresh estimate keeps the bump honest._ `max(fresh, bumped)` only ever raises; a falling market never lowers the floor.
- _Arithmetic overflow._ `saturating_add` (fees.rs:54) prevents a panic; saturation at `u128::MAX` is not a practical brake.
- _`failed_replacements_do_not_advance_the_fee_floor` (mod.rs:643-681, PR #686) fixes this._ It only stops compounding on _failed_ RPC calls; accepted resubmissions still compound, as that same test's final assertion shows (231/11 after one accepted bump).
- _A stuck transaction is impossible because a higher fee always mines._ Not while a lower nonce is missing from producers' pools, and not while the RPC is not propagating; both are ordinary operational states.

## Remediation options

1. Add an absolute ceiling to `tx::Config` (for example `max_fee_per_gas` and `max_priority_fee_per_gas` in wei); clamp after `bump` and stop resubmitting (log at warn, expose a metric) once the ceiling is reached. Tradeoff: a ceiling too low stalls under real congestion; make it mandatory in the sample configs with a generous default.
2. Bound the number of bumps per row (or per row per hour) and only bump the lowest in-flight nonce; rows behind a gap are re-broadcast unchanged. Tradeoff: slightly slower catch-up when the whole batch is genuinely underpriced.
3. Allow the floor to decay or be reset when the node reports a non-underpriced rejection for a row whose `submitted_at` is older than some window, and add a cancellation path (replace the payload with a zero-value self-transfer at the same nonce) so a row can always be cleared. Tradeoff: more code paths in a component that is currently simple.

Tests to add: mock that accepts every submission and never advances the nonce; drive 60 block statuses and assert the recorded fees stay below a configured ceiling; a two-row test where the lower nonce is generically rejected and the upper accepted, asserting the upper row's fee is not bumped while the lower is unaccepted.

## Trail

- Reviewer R3: drafted at commit 3ec8bc5, self-estimate 65%
- Critic C2-CORE-B: Confirmed, 72%, severity Medium (reviewer Medium).
- QA2-CORE: Reproduced (60 accepted bumps in 120 blocks: x311.9 max fee, x444.1 tip; blocked-row lockstep trigger); certainty 72 → 92; PoC `poc/F2-CORE-060/`.

## Critic (C2-CORE-B)

Method: read title and Location only, then did the arithmetic against `tx/fees.rs`, `tx/types.rs` and `tx/mod.rs` before reading the reviewer's Claim. Every citation re-opened at `3ec8bc5`; I re-ran `cargo test -p safenet-core --lib -- tx:: state:: effects::` (41 passed), which covers the tests cited in row 8.

| # | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | fees.rs:53-56: `previous.saturating_add(previous.div_ceil(10))`, then `fresh.max(bumped)`. |
| 2 | Supported | types.rs:64-66 and the doc note at fees.rs:38. |
| 3 | Supported | mod.rs:224-234. |
| 4 | Supported | storage.rs:294-298. |
| 5 | Supported | mod.rs:249-256, 265-266. |
| 6 | Supported | storage.rs:176-184; read back through `AllocatedTransaction::fees()` (types.rs:81-86). |
| 7 | Supported | mod.rs:284-293. |
| 8 | Supported (E1) | mod.rs:705-717; passes in my run. |
| 9 | Supported | mod.rs:71-83. |
| 10 | Supported as `I` | docs/validator-handbook.md:65 and docs/sentinel-handbook.md:57 re-opened; the quoted sentence is there. Stays `I` (documentation). |

Arithmetic (my own). `bump_fee(f, p) = max(f, p + ceil(p/10)) >= 1.1 p`. With the default `blocks_before_resubmit = 2` (mod.rs:89) a row submitted at block `s` is stale at `s + 2` (`submitted_at <= block - 2`, mod.rs:225-226 with storage.rs:297), so one accepted resubmission every two blocks, every 10 s on Gnosis (A10). After k accepted bumps the floor is at least `1.1^k` times the first accepted fee: k = 6 (1 min) 1.77x, k = 30 (5 min) 17.4x, k = 60 (10 min) 304x, k = 120 (20 min) 9.3e4x, k = 180 (30 min) 2.8e7x. The reviewer's figures are right. The compounding base is the _recorded_ floor, not the market: `record_submission` writes the bumped values into `request` (storage.rs:176-184) and the next `build` bumps from them (types.rs:65).

Cap sub-claim (the Manager asked for this to be settled). `cap_priority_fee` is applied exactly once, to the fresh estimate inside `fees()` (mod.rs:326-340: `let capped = cap_priority_fee(fees, cap); ... self.fee_cache = Some(fees)`), never after a bump; `build` (types.rs:64-66) then calls `fees::bump(estimate, self.fees())` and `bump` (fees.rs:43-49) raises `max_fee_per_gas` and `max_priority_fee_per_gas` independently. So the cap is **not re-applied to replacement bumps as a step** — yet the _ratio_ is preserved: if `p_f <= c*mf_f` and `p_prev <= c*mf_prev`, then `max(p_f, 1.1 p_prev) <= max(c*mf_f, c*1.1 mf_prev) = c*max(mf_f, 1.1 mf_prev)`, up to the sub-wei slack of `div_ceil` on the tip. R3's rejected item R7 is therefore arithmetically correct, the doc note at fees.rs:38 overstates the problem, and the ratio can only be violated if `priority_fee_cap_percentage` is lowered between runs (previous fees recorded under the old ratio). R3's actual claim — a ratio bounds nothing in absolute terms — stands.

Trigger. Verified reachable in this crate: a row that was never accepted (`submitted_at NULL`) is retried every block with `bump(fresh, None) = fresh` and no floor (types.rs:81-86; storage.rs:296-297), while an accepted higher-nonce row gets a floor and compounds every two blocks (mod.rs:265-266). Node-side facts — acceptance of future-nonce replacements, per-transaction balance checks, geth's `--rpc.txfeecap` — are `I` (no node source on disk) and are not needed for the mechanism. Concrete feeders for "row N blocked" exist in this crate: F2-CORE-062 (first-submission underpriced never bumped), F2-CORE-064 (a permanently rejected row holds its nonce), F2-CORE-061 (a false executed mark opens a gap). The "sticky stall" second-order claim is plausible but rests on node behaviour; treat it as `I`.

Finding verdict: **Confirmed** (mechanism E1/E2; trigger verified by trace with concrete in-crate feeders). Certainty **72%**. Severity **Medium / Medium**: a real path to spending the signer's balance, silent below `warn` (F2-CORE-067), but bounded by the balance and requiring an independent blocking condition that no attacker controls; not the High "unbounded drain" case.

QA note: the reviewer's proposed test (mock that accepts every submission and never advances the nonce; drive 60 statuses) is cheap with the existing `Asserter` harness in `tx/mod.rs` tests and would turn row 1 into an end-to-end E1.

## QA (QA2-CORE)

**Outcome: Reproduced** — unbounded compounding and the lockstep-behind-a-blocked-row trigger, with the in-crate `Asserter` mock.

Command: paste `poc/F2-CORE-060/tx_mod_tests.rs` into the `mod tests` of `crates/core/src/tx/mod.rs`; `cargo test -p safenet-core --lib qa_f2_core_060 -- --nocapture --test-threads=1`; file reverted.

Decisive output (`poc/F2-CORE-060/output.txt`), default `Config` (`blocks_before_resubmit = 2`), node accepting every submission, nonce never advancing, market estimate flat at 210/10 over 120 blocks (10 min on Gnosis): `bump 60 at block 130: maxFeePerGas=65505 maxPriorityFeePerGas=4441  (x311.9 / x444.1 of the estimate)`, against `1.1^60 = 304.5` — the tip compounds faster than 1.1^k because `div_ceil` rounds every bump up. Trigger 1 (row 0 rejected generically with `insufficient funds ...` on every block, row 1 accepted every second block): after 20 blocks `nonce 0 data 0x01 maxFeePerGas=None maxPriorityFeePerGas=None` (no floor, never bumped) and `nonce 1 data 0x02 maxFeePerGas=Some(553) maxPriorityFeePerGas=Some(33)` (ten bumps behind it).

Certainty: 72 → **92**. Confirmed plus `E1` for rows 1-8 and trigger 1; node-side behaviour stays `I`.

Remediation check: option 2 (bound the bumps; bump only the lowest in-flight nonce) is the soundest and directly matches trigger 1. Option 1 (absolute ceiling, stop resubmitting) is sound only together with a cancellation path (F2-CORE-064): stopping at the ceiling otherwise leaves the row in flight and every later nonce blocked. Option 3 sound but larger.

## Reconciliation (run 2)

**Final: EXTENDS `F-CORE-060` (canonical) — combined High, 98 (E1); cap sub-claim settled.** This file's accepted-resubmission arm (bump every 2 blocks, ×311.9 after 60 bumps), the lockstep behind a blocked row and the sticky floor are carried alongside run 1's underpriced-rejection arm (bump every block via `block: None` at `tx/mod.rs:277-282`, measured live). On the cap: this file's Critic is right that `cap_priority_fee` runs once on the fresh estimate and the ratio tip / `maxFeePerGas` is preserved (re-checked: 201,207 / 4.24e12 = 4.7e-8); run 1 is right that the tip is never re-bounded against the chain's base fee (7 wei admitted at 772 wei and 1 %; 201,207 observed = 28,744×) — both statements go into the report (`state/run2/reconciliation/core.md` §1.2; executed anchor `state/run2/logs/rec-core-fees-test.txt`). Severity: this file's Medium applies to its own arm; the combined finding is High because the underpriced arm is unbounded except by balance after a single rejection.
