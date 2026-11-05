# F-CORE-066 `tx::Config` accepts values that silently disable or destabilise the queue: `max_in_flight_transactions = 0`, `blocks_before_resubmit = 0`, and `priority_fee_cap_percentage = nan` or negative

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | core, `tx/mod.rs`, `tx/fees.rs` |
| Location | `crates/core/src/tx/mod.rs:69-83` (related: `crates/core/src/tx/fees.rs:12-31`, `crates/core/src/tx/mod.rs:204-216, 224-237, 322-345`) |
| Severity | Medium / Medium |
| Certainty | 78% |
| Assumptions involved | A1, A10 |
| Tags | config, dos, fees |

## Claim

`tx::Config` validates nothing beyond what serde's types enforce. Three accepted values each turn the transaction queue off or make it dangerous, with no error, no warning, and no metric:

- **`max_in_flight_transactions = 0`** makes the submission loop's range empty, so no transaction is ever allocated a nonce or broadcast. The service runs, indexes, updates state, queues actions into SQLite, and never acts onchain. This is total, permanent, silent loss of the service's onchain function from a single zero in a TOML file.
- **`blocks_before_resubmit = 0`** makes every in-flight transaction stale on every block, so each is rebuilt and rebroadcast with a compounding ≥10% fee bump every block. On Gnosis' ~5 s blocks (A10) that is ×3.1 per minute; combined with the absence of any ceiling (`F-CORE-060`) the fee reaches the balance limit within minutes.
- **`priority_fee_cap_percentage = nan`** — expressible in TOML — is clamped to 0 by `NaN.max(0.0)` and silently disables priority fees entirely, so every transaction is signed with `max_priority_fee_per_gas = 0`. A negative value does the same. On a chain where builders require a non-zero tip, nothing the service submits is ever included; the queue then fills with stuck transactions, and per `F-CORE-064` and `F-CORE-062` it wedges.

The sibling structs in this crate are validated with `NonZero` types where zero is meaningless (`events::Config` uses `NonZeroU64` for both its counters), so the omission here is inconsistent within the crate rather than a uniform house style.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The config declares plain `usize`/`u64`/`Option<f64>` with no `NonZero`, no range check and no validation hook; `deny_unknown_fields` only rejects unknown keys. | E2 | `crates/core/src/tx/mod.rs:69-83` | <pre>#[derive(Clone, Debug, Deserialize, PartialEq)]<br>#[serde(default, deny_unknown_fields)]<br>pub struct Config {<br> /// The maximum number of transactions that may be in flight (submitted<br> /// onchain but not yet executed) at any one time. The queue only submits new<br> /// transactions while it is below this limit.<br> pub max_in_flight_transactions: usize,<br> /// How many blocks a submitted transaction may go unexecuted before it is<br> /// resubmitted with a bumped fee.<br> pub blocks_before_resubmit: u64,<br> /// Caps the priority fee of estimated fees to at most this percentage of the<br> /// total max fee per gas, lowering the priority fee (and max fee) when an<br> /// estimate exceeds it. `None` applies no cap.<br> pub priority_fee_cap_percentage: Option&lt;f64&gt;,<br>}</pre> |
| 2 | With `max_in_flight_transactions = 0` the range `in_flight..0` is empty for every `in_flight`, so the body never runs and `next_transaction` is never called. | E2 | `crates/core/src/tx/mod.rs:204-216` | <pre>async fn submit*pending(&mut self, block: u64) -> Result&lt;, Error&gt; {<br> let in_flight = self.storage.count_in_flight.await?;<br> for * in in_flight..self.config.max_in_flight_transactions {<br> let nonce = self.nonce.await?;<br> let Some(transaction) = self<br> .storage<br> .next_transaction(Status { nonce, block })<br> .await?<br> else {<br> break;<br> };<br> self.submit_transaction(transaction, block).await?;<br> }</pre> |
| 3 | With `blocks_before_resubmit = 0`, `submitted_before == block`, so `submitted_at <= block` matches every row submitted at or before the current block — i.e. all of them, every block. | E2 | `crates/core/src/tx/mod.rs:224-229` | <pre>async fn resubmit_stale(&mut self, block: u64) -> Result&lt;, Error&gt; {<br> let submitted_before = block.checked_sub(self.config.blocks_before_resubmit);<br> let stale = self.storage.stale_submissions(submitted_before).await?;<br> if stale.is_empty {<br> return Ok();<br> }</pre> |
| 4 | Each such resubmission applies the compounding bump. | E2 | `crates/core/src/tx/fees.rs:52-56` | <pre>/// Returns `fresh`, raised to at least 10% above `previous`.<br>fn bump_fee(fresh: u128, previous: u128) -> u128 {<br> let bumped = previous.saturating_add(previous.div_ceil(10));<br> fresh.max(bumped)<br>}</pre> |
| 5 | `NaN.max(0.0)` is `0.0` in Rust (`f64::max` returns the non-NaN operand), so a `nan` cap scales to 0, `capped` is 0, and the priority fee is forced to 0. A negative cap takes the same path. | E2 | `crates/core/src/tx/fees.rs:12-25` | <pre>pub fn cap_priority_fee(fees: Eip1559Estimation, cap_percentage: f64) -> Eip1559Estimation {<br> // Scale the percentage into integer space for the fee math, allowing up to<br> // six digits of precision in `cap_percentage`.<br> const PRECISION: u128 = 1_000_000;<br> let scaled_percent = ((cap_percentage.max(0.0) / 100.0) \* PRECISION as f64).round as u128;<br> if scaled_percent >= PRECISION {<br> return fees;<br> }<br><br> let base_fee = fees<br> .max_fee_per_gas<br> .saturating_sub(fees.max_priority_fee_per_gas);<br> let capped = base_fee.saturating_mul(scaled_percent) / (PRECISION - scaled_percent);<br> let max_priority_fee_per_gas = fees.max_priority_fee_per_gas.min(capped);</pre> |
| 6 | The crate's own test fixes the zero/negative behaviour as intended at the `fees.rs` level, so nothing below the config layer will ever reject it. | E2 | `crates/core/src/tx/fees.rs:83-89` | <pre>#[test]<br>fn priority_fee_cap_saturates_at_its_bounds {<br> // A cap of 0% disables the priority fee, leaving just the base fee of 40.<br> assert_eq!(cap_priority_fee(fees(100, 60), 0.0), fees(40, 0));<br><br> // A negative cap is clamped to 0% and behaves identically.<br> assert_eq!(cap_priority_fee(fees(100, 60), -25.0), fees(40, 0));</pre> |
| 7 | The cap is applied silently; the only trace of it is a `debug` log, and only when it actually lowered the fee. | E2 | `crates/core/src/tx/mod.rs:327-340` | <pre>let fees = match self.config.priority_fee_cap_percentage {<br> Some(cap) => {<br> let capped = cap_priority_fee(fees, cap);<br> if capped.max_priority_fee_per_gas < fees.max_priority_fee_per_gas {<br> tracing::debug!(<br> original = fees.max_priority_fee_per_gas,<br> capped = capped.max_priority_fee_per_gas,<br> "priority fee capped"<br> );<br> }<br> capped<br> }<br> None => fees,<br>};</pre> |
| 8 | The neighbouring index config _does_ use `NonZero` for values where zero is meaningless, so this is an inconsistency within the crate. | E2 | `crates/core/src/index/events.rs:94-99` | <pre>impl Default for Config {<br> fn default -> Self {<br> Self {<br> block_page_size: NonZeroU64::new(100).expect("100 is nonzero"),<br> block_single_query_retry_count: NonZeroU64::new(3).expect("3 is nonzero"),<br> use_client_filtering: false,</pre> |
| 9 | The operator-facing sample presents the cap as a tunable percentage, with no mention that low, zero or negative values disable tips outright. | E2 | `crates/validator/validator.sample.toml:72-77` | <pre>[transactions]<br># Optional: caps the priority fee of estimated fees to at most this<br># percentage of the total max fee per gas (see the validator handbook's gas<br># cost tip). Lower values reduce what you can overpay on a bad fee estimate,<br># but risk slower inclusion if the cap ends up below what the network needs.<br># priority_fee_cap_percentage = 95</pre> |

## Trigger

Editing the `[transactions]` table of `validator.toml` or `sentinel.toml`. All three values parse, the process starts normally, `deny_unknown_fields` passes, and the config test suite (which only checks parseability, `crates/validator/src/config.rs:254, 279`) is unaffected.

- `max_in_flight_transactions = 0`: the most likely route is an operator throttling submissions ("start conservative, raise it later") and reaching for 0 rather than 1. From that moment the validator or sentinel indexes the chain, advances its state machine, writes rows into `transactions`, and never submits any of them — it looks healthy on every metric that exists (`safenet_core_block_number` keeps advancing, `/health` returns `OK`) while being entirely absent from the protocol. For a validator this is missed keygen and signing participation; the analysis' §3.4 notes the value is accepted but not what it costs.
- `blocks_before_resubmit = 0`: the likely route is an operator wanting faster inclusion. Claims 3 and 4 give a ≥10% compounding bump every block, and with no ceiling (`F-CORE-060`) the fee reaches the account's balance limit in minutes. The setting reads like "resubmit promptly" and behaves like "escalate without bound".
- `priority_fee_cap_percentage = nan`: TOML 1.0 accepts `nan` and `inf` as float literals, so `priority_fee_cap_percentage = nan` deserializes into `Some(f64::NAN)`. Claim 5 shows it becomes a 0% cap. Every subsequent transaction is signed with `max_priority_fee_per_gas = 0`; claim 7 shows the only trace is a `debug` line. The same outcome follows from any negative value or from a small positive one entered in the wrong unit (`0.95` intending 95%). Note the asymmetry: `inf` scales above `PRECISION` and is a harmless no-op (claim 5's early return), while `nan` is maximally harmful — the two malformed values behave oppositely.

I have not executed any of these (A9 FALSE). Claims 1–9 are `E2` from the cited code and sample; the operator sequences are `I`.

## Considered and rejected

- **"A1 makes operator error out of scope."** A1 assumes an honest operator, not an infallible one. These are silent failures of a service whose whole purpose is to act onchain, and the crate already guards the analogous cases in `index::Config` with `NonZero` (claim 8), which is the strongest argument that the omission is an oversight rather than a decision.
- **"`analysis-core.md` §3.4 already records this."** It records that the values are accepted (`blocks_before_resubmit = 0` resubmits every block; `max_in_flight_transactions = 0` never submits) as a one-line note under configuration, and lists `NaN` only as "clamped to 0 → tips disabled" in the panic census. It does not connect any of them to an outcome, does not note that TOML can express `nan`, and does not note the `nan`/`inf` asymmetry. It is a lead, not a finding, and there is no corresponding entry in `codebase-map.md` §4, so the `known` tag does not apply.
- **"`NaN` cannot reach the config."** It can. `priority_fee_cap_percentage: Option<f64>` (claim 1) with serde's default `f64` deserializer, and TOML 1.0 lists `nan` among its float literals. I could not run a parse to confirm end-to-end (no toolchain), so this specific step is `I`; the _consequence_ once a `NaN` arrives is `E2` from claim 5.
- **"The `debug` log is enough."** It fires only when the cap lowered the fee (claim 7) — which for a `nan`/0% cap is every time, but at `debug`, below the sample configs' `info` default, and it says "priority fee capped" rather than "priority fees disabled". No metric records the effective fee.
- **"A zero in-flight cap would be obvious immediately."** Nothing surfaces it. `submit_pending` returns `Ok()` (claim 2); `count_outstanding` still reports rows, so the reconciliation block still runs and still fetches the nonce each block, making the RPC traffic look normal. There is no queue-depth metric (Observation O6 in `rust-audit/../state/coverage-logs.md#r3`).
- **Not a duplicate of `F-CORE-060`.** That finding is that the _default_ configuration has no fee ceiling. This one is that the configuration layer accepts values which make that and two other failure modes far worse or immediate. Fixing either does not fix the other.

## Remediation options

1. **Use types that cannot express the bad values.** `max_in_flight_transactions: NonZeroUsize` and `blocks_before_resubmit: NonZeroU64`, matching `events::Config` (claim 8). Serde rejects zero at parse time with a clear message and no extra code. Tradeoff: `blocks_before_resubmit = 0` may be wanted in tests; a `NonZeroU64` of 1 is equivalent in practice (resubmit on the next block) and is the value an operator reaching for 0 actually means.
2. **Validate the cap after deserialization.** Reject `NaN`, reject negatives, and reject values below some floor (say 1.0) unless an explicit `disable_priority_fee: bool` is set, so that "no tip" is something an operator asks for rather than something they fall into. A custom `Deserialize` or a post-parse `Config::validate` called from both binaries' startup would do it.
3. **Log the effective configuration at `info` on startup**, including the resolved cap and what it means for the priority fee. Cheap, and it makes every misconfiguration in this finding visible in the first ten lines of the log.
4. **Add the missing metrics.** A queue-depth gauge and an effective-fee gauge would make all three of these observable in production rather than only in a code review; this is the same recommendation as `F-CORE-060` option 2 and `F-CORE-062` option 2.
5. **Document the bounds in the sample configs** for all three fields, not just the cap (claim 9), including what zero does.

Tests to add: config tests asserting that `max_in_flight_transactions = 0`, `blocks_before_resubmit = 0`, `priority_fee_cap_percentage = nan` and a negative cap are all rejected; a `fees.rs` test pinning `cap_priority_fee(_, f64::NAN)` to whatever the intended behaviour becomes (there is currently no `NaN` case — `analysis-core.md` §10 lists it as untested).

## Trail

- Reviewer R3: drafted, self-estimate 75%. Claims 1–9 are `E2` and the behaviours follow directly from the cited code; the `NaN`-reaches-config step and the operator sequences are `I`. Severity Medium rather than Low because `max_in_flight_transactions = 0` is a complete and permanent silent loss of the service's onchain function, which sits above the Low band's "limited impact", while remaining an operator-reachable configuration issue under A1.

## Critic (C-CORE-B)

I evaluated the three values against the code before reading the Claim, and all three behave as stated.

- **`max_in_flight_transactions = 0`**: `for _ in in_flight..0` is an empty range for any `in_flight >= 0` (`tx/mod.rs:205-206`), so `next_transaction` is never called, no nonce is ever allocated, and nothing is ever broadcast. The service indexes, transitions and enqueues rows forever. **Confirmed.**
- **`blocks_before_resubmit = 0`**: `submitted_before = block.checked_sub(0) = Some(block)`, and `stale_submissions` matches `submitted_at <= block`, which includes the row submitted in this very block (`tx/mod.rs:225`, `tx/storage.rs:297`). Every in-flight row is stale every block and is rebuilt with a ≥10% bump. **Confirmed.**
- **`priority_fee_cap_percentage = nan`**: `f64::max` returns the non-NaN operand, so `NaN.max(0.0)` is `0.0`; `(0.0 / 100.0) * 1_000_000.0` rounds to `0.0` and casts to `0u128`; `0 >= PRECISION` is false, so the early return is skipped and `capped = base_fee * 0 / 1_000_000 = 0`, giving `max_priority_fee_per_gas = 0` (`tx/fees.rs:16-25`). The asymmetry the reviewer points out is real and I verified it: `inf.max(0.0)` is `inf`, `inf as u128` saturates to `u128::MAX >= PRECISION`, so `inf` hits the early return and is a harmless no-op while `nan` silently zeroes the tip. **Confirmed.**

### Per-claim verdicts

All basis rows **Supported** against the cited ranges. The reviewer's supporting point that the sibling `events::Config` uses `NonZeroU64` for both its counters is correct (`index/events.rs:94-104`), so the omission is inconsistent within the crate rather than a house style — that is a fair use of in-tree evidence rather than an appeal to taste.

TOML 1.0 does accept `nan` and `inf` as float literals, so `Some(f64::NAN)` really is expressible in `validator.toml`. That is a specification fact, not a dependency-internals claim, so it is legitimately `E2` here.

### Finding verdict

**Confirmed — 78%.** Every mechanism is verified in code, and the trigger — a value in the `[transactions]` table of a config file — is fully in-tree: `tx::Config` has `#[serde(default, deny_unknown_fields)]` (`tx/mod.rs:69-71`) and no validation beyond serde's types, so all three values parse and start the process. The reviewer marks the _operator sequences_ `I`, which is right about motive but not about reachability; the value itself is reachable with certainty, which is why this sits in the Confirmed band rather than with its `I`-triggered neighbours.

**Severity: Medium (unchanged).** Not High: A1 makes the operator trusted-but-fallible and no attacker supplies the config. Not Low: "configuration weaknesses with **limited** impact" does not describe a single `0` that silently removes the service's entire onchain function while every exported signal keeps reporting health, nor a `nan` that zeroes the tip on every transaction the service will ever send.

The smallest complete fix is the one the crate already uses next door: `NonZeroUsize`/`NonZeroU64` for the two counters, and a range check plus a `NaN` rejection on `priority_fee_cap_percentage` at deserialisation. That is a validation change with no behavioural risk and should be sequenced ahead of the F-CORE-060 ceiling, which it partly pre-empts.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 1 for the integers, option 2 for the float. Neither is optional.**

Option 1 (`max_in_flight_transactions: NonZeroUsize`, `blocks_before_resubmit: NonZeroU64`) is sound and is the right mechanism: serde rejects zero at parse time with a clear message and no extra code, and it matches what `events::Config` already does, so it is consistency rather than novelty. The stated tradeoff — that `blocks_before_resubmit = 0` may be wanted in tests — is correctly answered: `NonZeroU64` of 1 is what an operator reaching for 0 actually means.

Option 2 (validate the cap: reject `NaN`, reject negatives, reject values below a floor unless an explicit `disable_priority_fee: bool` is set) is sound and the `disable_priority_fee` half is the good part — "no tip" should be something an operator asks for, not something they fall into by typing `0`. Note that `cap_priority_fee` currently handles a negative cap by clamping to 0% via `cap_percentage.max(0.0)` (`tx/fees.rs:15`), which is _defined_ but is silently the most destructive setting available; validation is the fix, not the clamp.

Option 3 (log the effective configuration at `info` on startup, including the resolved cap and what it means) is cheap and makes every misconfiguration in this finding visible in the first ten lines of the log. Take it regardless.

Option 4 (queue-depth and effective-fee gauges) is the same metric set **F-CORE-060 option 2** and **F-CORE-062 option 2** ask for. Consolidate.

**One interaction worth recording:** `priority_fee_cap_percentage` is currently the _only_ fee bound in the crate, and **F-CORE-060** shows it does not survive a bump. Tightening its validation here makes the config option look more trustworthy than it is. Option 2 should land together with F-CORE-060's ceiling, or an operator who carefully sets a valid cap still has no bound on spend.

## In-flight impact (FWD)

**Pertains to unmerged branches, not to `main`.** Assessed against PR #902 ("[Phase 2] Adjust config", `origin/feat/batex_2`, carried through `origin/feat/batex_4`). **Effect: unchanged in kind, surface extended.** All three accepted-but-dangerous values in the claim survive verbatim: `max_in_flight_transactions` and `blocks_before_resubmit` are still plain `usize`/`u64` with no `NonZero`, and `priority_fee_cap_percentage` is still an unvalidated `Option<f64>`. Phase 2 adds two more fields to the same struct and validates neither. `max_batch_gas: u64` defaults to `2_000_000` and accepts `0`; a zero (or any value below the epic's ~26 000 base term) makes every transaction fail the "fits in an empty batch" test, so batching silently degrades to one transaction per action with a `warn` per action — the same class of silent, config-driven functional loss this finding is about. `executor: Option<Address>` accepts **any** address, including the zero address, which the sample TOMLs ship as their literal example value; under EIP-7702 that is the delegation-_clearing_ target, and any codeless target re-opens the epic's own "`execute` succeeds as a no-op and every batched action is silently dropped" mode. The epic plans an `eth_getCode(executor)` startup check for Phase 5 — **not on any pushed branch**, so as the stack stands the field is accepted with no check at all. `#[serde(default, deny_unknown_fields)]` is preserved, and the two services' existing `parses_sample_config` tests still pass. Severity and certainty unchanged. See `rust-audit/report/IN-FLIGHT.md`.

## Reconciliation (run 2)

**Final: Confirmed, Medium, 90 (E1), canonical.** Rediscovered by run 2 as `F2-CORE-065` (CONFIRMS, executed for all three values including TOML `nan` through the validator's real loader: 0 submissions in 3 blocks, a bump on every block, tip 0). Run 2 rated Low under A1; Medium is carried because `max_in_flight_transactions = 0` silently removes the service's entire on-chain function while every exported signal reports health — not "limited impact" (`state/run2/reconciliation/core.md` §1). 78 → 90. Still valid at `fe9e84c`. Counterpart: `F2-CORE-065`; the NaN case is `F-CORE-061`/`F2-CORE-062`'s entry point.

## Run 3 re-validation (rv-core-b, 2026-10-08)

| Field | Value |
| --- | --- |
| Audit commit | `361aaf3` |
| Verdict | Still valid |
| Changed by | `9ed4b80` #991 moved `tx::Config` to `crates/core/src/tx/config.rs`; `e9101e2` #992 added `SubmissionMode` and a `TryFrom<RawConfig>` that validates only the mode combination and rejects a zero `executor`. None of the three values in this row is validated. `cd46431` #1023 added a startup check on `max_batch_gas` against the block gas limit, which is a different field. |
| Severity, certainty | Unchanged: Medium, 90 |
| Evidence | E1 `poc/F2-CORE-065/run3/` (run-2 PoC adapted and re-run, including TOML `nan` through the validator's real loader) |

Re-based anchors (old location at `fe9e84c`, new location at `361aaf3`, verbatim quote).

- `tx/mod.rs:69-83` (no validation beyond serde types) to `crates/core/src/tx/config.rs:13-25` and `:78-86` (`RawConfig`, `#[serde(default, deny_unknown_fields)]`, `max_in_flight_transactions: Option<usize>`, `blocks_before_resubmit: u64`, `priority_fee_cap_percentage: Option<f64>`), and the conversion at `crates/core/src/tx/config.rs:104-134`:

```rust
            (max_in_flight_transactions, None, None) => SubmissionMode::Direct {
                max_in_flight_transactions: max_in_flight_transactions
                    .unwrap_or(DEFAULT_MAX_IN_FLIGHT_TRANSACTIONS),
            },
            (_, None, Some(_)) => return Err("`max_batch_gas` requires `executor`"),
            (Some(_), Some(_), _) => {
                return Err("`max_in_flight_transactions` cannot be combined with `executor`");
            }
```

and, after the zero-executor arm:

```rust
        Ok(Self {
            mode,
            blocks_before_resubmit: raw.blocks_before_resubmit,
            priority_fee_cap_percentage: raw.priority_fee_cap_percentage,
        })
```

- `tx/mod.rs:204-216` (zero in-flight limit never allocates) to `crates/core/src/tx/mod.rs:235-237`: `while in_flight < self.config.mode.max_in_flight_transactions()`, which is `while 0 < 0` for `Direct { max_in_flight_transactions: 0 }`.
- `tx/mod.rs:224-229` (zero resubmit interval) to `crates/core/src/tx/mod.rs:291-293` (unchanged text).
- `tx/fees.rs:52-56` (each resubmission bumps) unchanged at `crates/core/src/tx/fees.rs:52-56`.
- `tx/fees.rs:12-25` (`NaN.max(0.0)` is 0, tip forced to 0) unchanged at `crates/core/src/tx/fees.rs:12-25`.
- `tx/fees.rs:83-89` (test pins the zero and negative behaviour) to `crates/core/src/tx/fees.rs:84-89`.
- `tx/mod.rs:327-340` (the cap's only trace is a `debug` log) to `crates/core/src/tx/mod.rs:396-409`.
- `index/events.rs:94-99` (`NonZeroU64` used next door) to `crates/core/src/index/events.rs:87-90, 108-109`.
- `validator.sample.toml:72-77` to `crates/validator/validator.sample.toml:77-104`; the sample now documents `max_in_flight_transactions`, `executor` and `max_batch_gas` but still says nothing about zero values or a non-finite cap.

What was run (worktree at `361aaf3`, PoC snippets appended to the `mod tests` of `crates/core/src/tx/mod.rs` and `crates/validator/src/config.rs`, reverted, `git status --short` empty):

- `cargo test -p safenet-core --lib rv3_ -- --nocapture --test-threads=1`: `rv3_f2_core_065_degenerate_config_values_are_accepted_and_misbehave_silently` ok. JSON `{"max_in_flight_transactions":0,"blocks_before_resubmit":0}` parses to `Config { mode: Direct { max_in_flight_transactions: 0 }, blocks_before_resubmit: 0, .. }`; `max_in_flight_transactions = 0` gives 3 blocks, 3 account requests and 0 submissions; `blocks_before_resubmit = 0` bumps on every block, 210/10 to 281/15 by block 13; a NaN cap gives `{40, 0}`. Identical to run 2.
- `cargo test -p validator --bins rv3_ -- --nocapture`: `rv3_f_core_066_toml_accepts_zero_and_nan_transaction_values` ok; the validator's TOML loader returns `Config { mode: Direct { max_in_flight_transactions: 0 }, blocks_before_resubmit: 0, priority_fee_cap_percentage: Some(NaN) }`.

Notes:

- The new `TryFrom<RawConfig>` is the natural place for the missing checks and shows the pattern is now in use in the same struct (it already rejects `executor = 0x0` with an explicit message), which strengthens remediation option 1.
- `max_batch_gas = 0` is a documented setting ("`0` sends every transaction through the executor without batching", `crates/core/src/tx/config.rs:55-58`) and is not part of this row. `blocks_before_resubmit = 0` and a NaN or non-positive cap apply in batched mode too, through the same resubmission and fee path.
- Counterpart `F2-CORE-065`; the NaN case is `F-CORE-061`'s zero-tip entry point.

## Run 3 counterpart

- Rediscovered blind by run 3 as [`F3-CORE-061`](F3-CORE-061.md) (CONFIRMS). This ID stays canonical.
- Ledger values: Informational, certainty 90 (`F3-CORE-061`'s CA3 value: operator values under A1).
