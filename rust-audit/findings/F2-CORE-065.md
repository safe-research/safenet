# F2-CORE-065 `tx::Config` accepts degenerate values silently: zero in-flight limit never submits, zero resubmit interval bumps fees every block, NaN or negative cap disables tips

| Field | Value |
| --- | --- |
| Status | QA'd |
| Crate and module | safenet-core, tx/mod.rs, tx/fees.rs |
| Location | crates/core/src/tx/mod.rs:68-93 (related: tx/mod.rs:204-206, 225; tx/fees.rs:16) |
| Severity | Low (reviewer) / Low (Critic) |
| Certainty | 90% (QA2-CORE; Critic C2-CORE-B set 80%) |
| Assumptions involved | A1 |
| Tags | config, input-validation |

## Claim

The only validation on the `[transactions]` table is serde's type checking plus `deny_unknown_fields`. Three values that are almost certainly mistakes are accepted and produce silent misbehaviour:

- `max_in_flight_transactions = 0`: `submit_pending` iterates `in_flight..0`, so nothing is ever allocated; actions accumulate in the table forever (no cap on queued rows) while the queue still spends one `eth_getTransactionCount` per block because `count_outstanding > 0`. No log line distinguishes this from a healthy idle queue.
- `blocks_before_resubmit = 0`: `submitted_before = latest`, so every in-flight row is stale on the very next block and is re-broadcast with a 10% bump each block (x1.1 per block, x2 every ~7 blocks) even when the network would have included it a block later.
- `priority_fee_cap_percentage = nan` (TOML allows `nan`, `inf`, `-inf`): `f64::max(NaN, 0.0)` is `0.0`, so NaN is treated as 0% and tips are disabled with the consequences in F2-CORE-062; negative values do the same by documented design. `inf` saturates to a no-op.

None of these is rejected at load, and none is surfaced at startup.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Config has serde defaults and unknown-field denial only; no validation hook. | E2 | crates/core/src/tx/mod.rs:69-83 | `#[derive(Clone, Debug, Deserialize, PartialEq)]` / `#[serde(default, deny_unknown_fields)]` / `pub struct Config {` / `pub max_in_flight_transactions: usize,` / `pub blocks_before_resubmit: u64,` / `pub priority_fee_cap_percentage: Option<f64>,` / `}` |
| 2 | Zero in-flight limit yields an empty loop. | E2 | crates/core/src/tx/mod.rs:205-206 | `let in_flight = self.storage.count_in_flight().await?;` / `for _ in in_flight..self.config.max_in_flight_transactions {` |
| 3 | The nonce RPC is still issued every block while rows are outstanding. | E2 | crates/core/src/tx/mod.rs:185-188 | `if previous.is_none_or(\|previous\| previous.latest < status.latest)` / `&& self.storage.count_outstanding(status.latest).await? > 0` / `{` / `let nonce = self.nonce().await?;` |
| 4 | Zero resubmit interval makes every row stale on the next block. | E2 | crates/core/src/tx/mod.rs:225-226 and tx/storage.rs:297 | `let submitted_before = block.checked_sub(self.config.blocks_before_resubmit);` / `let stale = self.storage.stale_submissions(submitted_before).await?;` ... `AND (submitted_at IS NULL OR submitted_at <= ?)` |
| 5 | NaN is clamped to zero by `f64::max`. | E2 | crates/core/src/tx/fees.rs:16 | `let scaled_percent = ((cap_percentage.max(0.0) / 100.0) * PRECISION as f64).round() as u128;` |
| 6 | Rust `f64::max` returns the non-NaN operand (documented std behaviour, not on disk). | I | std docs for `f64::max`: "If one of the arguments is NaN, then the other argument is returned." | -- |
| 7 | The consuming services deserialize the table verbatim with no post-validation of these fields (validator config test shows plain passthrough). | E2 | crates/validator/src/config.rs:254, 279 | `max_in_flight_transactions = 4` ... `assert_eq!(config.driver.transactions.max_in_flight_transactions, 4);` |

## Trigger

Write any of the three values into `[transactions]` of a sample config and start the service; it starts cleanly and behaves as described. No panic, no error, no startup log.

## Considered and rejected

- _`NonZero` types elsewhere show the pattern is known._ `index::Config` uses `NonZeroU64` for page sizes (events.rs:76); `tx::Config` does not use it for `max_in_flight_transactions`, so the protection is inconsistent rather than deliberate.
- _Operators are trusted (A1)._ Trusted, not infallible; the point of validation is to turn a typo into a startup error rather than a silent stall or a fee leak.

## Remediation options

1. Validate at deserialisation (`TryFrom<RawConfig>` or a `validate()` called from `Driver::new`): `max_in_flight_transactions >= 1` (`NonZeroUsize`), `blocks_before_resubmit >= 1`, `priority_fee_cap_percentage` finite and within `(0, 100]`.
2. Log the effective `tx::Config` at info on startup so degenerate values are at least visible.

Tests to add: config parsing tests asserting rejection of `0`, `0`, and `nan` respectively.

## Trail

- Reviewer R3: drafted at commit 3ec8bc5, self-estimate 75%
- Critic C2-CORE-B: Confirmed, 80%, severity Low (reviewer Low).
- QA2-CORE: Reproduced (zero values and TOML `nan` accepted; never-submit, bump-every-block and zero-tip behaviours shown); certainty 80 → 90; PoC `poc/F2-CORE-065/`.

## Critic (C2-CORE-B)

Method: read title and Location only, traced each of the three values through `submit_pending`, `resubmit_stale` and `cap_priority_fee` myself, then compared.

| # | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | mod.rs:69-83; no `validate`/`TryFrom` anywhere in `tx/`. |
| 2 | Supported | mod.rs:205-206: `for _ in in_flight..0` is empty. |
| 3 | Supported | mod.rs:185-188: `count_outstanding` counts queued non-expired rows (storage.rs:212-215), so the nonce RPC is still issued. |
| 4 | Supported | mod.rs:225-226 with storage.rs:297; `resubmit_stale` runs before `submit_pending` (mod.rs:195-196), so a row submitted at `b` is first stale at `b + 1` — one bump per block, as claimed. |
| 5 | Supported | fees.rs:16. |
| 6 | `I`, correct | std `f64::max` returns the non-NaN operand; TOML accepts `nan`/`inf`. Stays `I` (standard-library behaviour), not load-bearing beyond the NaN case. |
| 7 | Supported | validator config.rs:254 and 279 re-opened: plain passthrough of `max_in_flight_transactions = 4`. |

Finding verdict: **Confirmed**. Certainty **80%** (E2; each behaviour follows directly from the cited lines). Severity **Low / Low** (A1: operator-supplied values; the value of the finding is turning a typo into a startup error).

Note: the `nan` case is the entry point to F2-CORE-062's zero-tip stall; the two findings should cite each other in the report.

## QA (QA2-CORE)

**Outcome: Reproduced** — all three values, including TOML `nan` through the validator's real loader.

Command: paste `poc/F2-CORE-065/tx_mod_tests.rs` into the `mod tests` of `crates/core/src/tx/mod.rs` and run `cargo test -p safenet-core --lib qa_f2_core_065 -- --nocapture`; paste `poc/F2-CORE-065/validator_config_tests.rs` into the `mod tests` of `crates/validator/src/config.rs` and run `cargo test -p validator --bins qa_f2_core_065 -- --nocapture`; both reverted.

Decisive output (`poc/F2-CORE-065/output.txt`, `output-validator-toml.txt`): from TOML, `parsed [transactions]: Config { max_in_flight_transactions: 0, blocks_before_resubmit: 0, priority_fee_cap_percentage: Some(NaN) }`; `cap NaN: Eip1559Estimation { max_fee_per_gas: 40, max_priority_fee_per_gas: 0 }`; `max_in_flight_transactions = 0: 3 blocks, 3 nonce RPCs, 0 submissions`; `blocks_before_resubmit = 0`: blocks 10..=13 give `210/10 → 231/11 → 255/13 → 281/15`, a bump on every block.

Certainty: 80 → **90**. Confirmed plus `E1`; row 6 is no longer `I`.

Remediation check: option 1 sound (`NonZeroUsize`, `blocks_before_resubmit >= 1`, cap finite and within `(0, 100]`); option 2 useful for visibility but not a fix.

## Reconciliation (run 2)

**Final: CONFIRMS `F-CORE-066` (canonical) — combined Medium, 90 (E1).** Identical three values, executed here including TOML `nan` through the validator loader. This file's Low is recorded; Medium is carried because a single `0` silently removes all on-chain function while every signal reports health (`state/run2/reconciliation/core.md` §1).
