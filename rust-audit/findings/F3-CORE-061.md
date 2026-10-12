# F3-CORE-061 Queue settings that silently stop submission or inclusion are accepted

| Field | Value |
| --- | --- |
| Status | Final: Accepted |
| Crate and module | safenet-core, `tx/config.rs`, `tx/fees.rs`, `tx/mod.rs` |
| Location | `crates/core/src/tx/config.rs:77-135` (related: `crates/core/src/tx/fees.rs:12-31`, `crates/core/src/tx/fees.rs:53-56`, `crates/core/src/tx/mod.rs:236-237`, `crates/core/src/tx/mod.rs:291-293`, `crates/validator/validator.sample.toml:100-104`, `crates/sentinel/sentinel.sample.toml:85-89`) |
| Severity | Informational / Informational |
| Certainty | 90% (CA3: evidence checked) |
| Assumptions involved | A1 |
| Tags | config |

## Claim

`tx::Config` parses `max_in_flight_transactions`, `blocks_before_resubmit` and `priority_fee_cap_percentage` without range checks (`RawConfig` to `Config` validates only the submission-mode combination). Three accepted values defeat the queue with no error at startup and no specific log line afterwards:

1. `max_in_flight_transactions = 0`. `submit_pending` loops `while in_flight < 0`, which never runs, so no queued action is ever allocated a nonce or sent. The service keeps indexing and queuing; nothing reaches the chain.
2. `priority_fee_cap_percentage` of `0`, any negative value, or `nan` (valid TOML). `cap_priority_fee` returns a zero tip: `cap_percentage.max(0.0)` maps NaN and negatives to 0. A zero tip then never grows, because `bump_fee` adds `previous.div_ceil(10)`, which is 0 for a previous tip of 0; only `maxFeePerGas` climbs, which cannot raise the effective tip. If the block producers serving the chain require a nonzero tip, none of this node's transactions is ever included. The sample configs say a low cap risks "slower inclusion"; for any cap above 0 the bumps do eventually lift the tip, but at 0 it is never, and NaN is not documented anywhere.
3. `blocks_before_resubmit = 0`. Every unexecuted transaction is rebuilt with bumped fees and rebroadcast on every new block, including one submitted on the block before, which doubles the compounding rate described in F3-CORE-060.

The operator is trusted (A1), so this is hardening: a typo or a misunderstanding (0 read as "no limit" or "no cap") silently takes a validator or sentinel offline.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The three values pass parsing unchanged | E1 | `crates/core/src/tx/config.rs:101-135`; `rust-audit/state/run3/logs/R3-fee-config-probe.txt` (section D) | `max_in_flight_transactions = 0       -> Ok(Config { mode: Direct { max_in_flight_transactions: 0 }, blocks_before_resubmit: 2, priority_fee_cap_percentage: None })` |
| 2 | With an in-flight limit of 0 the submission loop never runs | E1 | `crates/core/src/tx/mod.rs:236-237`; probe section E3 | `while in_flight < self.config.mode.max_in_flight_transactions() {` |
| 3 | NaN, 0 and negative caps give a zero tip | E1 | `crates/core/src/tx/fees.rs:16`; probe section A | `let scaled_percent = ((cap_percentage.max(0.0) / 100.0) * PRECISION as f64).round() as u128;` |
| 4 | A zero tip stays zero across resubmissions | E1 | `crates/core/src/tx/fees.rs:53-56`; probe sections C and E2 | `let bumped = previous.saturating_add(previous.div_ceil(10));` |
| 5 | A resubmit interval of 0 makes every unexecuted row due on every block | E2 | `crates/core/src/tx/mod.rs:292`, `crates/core/src/tx/storage.rs:401` | `let submitted_before = block.checked_sub(self.config.blocks_before_resubmit);` |
| 6 | The samples describe the low-cap risk as slower inclusion | E2 | `crates/validator/validator.sample.toml:100-104` | `# but risk slower inclusion if the cap ends up below what the network needs.` |

Executed output (`R3-fee-config-probe.txt`):

```text
cap    NaN: max_fee     84000000 tip            0
cap      0: max_fee     84000000 tip            0
cap    -25: max_fee     84000000 tip            0
round 100: max_fee          1157571473420 tip 0
priority_fee_cap_percentage = nan    -> Ok(Config { mode: Direct { max_in_flight_transactions: 16 }, blocks_before_resubmit: 2, priority_fee_cap_percentage: Some(NaN) })
blocks_before_resubmit = 0           -> Ok(Config { mode: Direct { max_in_flight_transactions: 16 }, blocks_before_resubmit: 0, priority_fee_cap_percentage: None })
round 20: max_fee        565110009 tip 0
block 12: unused mocked responses 2 (fee history and send never requested); allocated nonces 0; queued actions 1
```

The E2 run (NaN cap) and the E3 run (in-flight limit 0) drive the real `TransactionQueue::update_block_status` with a mocked RPC and an in-memory pool; the E3 run shows the fee estimate and the send are never requested and the action stays unallocated.

## Trigger

An operator config with `max_in_flight_transactions = 0`, `priority_fee_cap_percentage = 0` (or a negative value, or `nan`), or `blocks_before_resubmit = 0` in `[transactions]`. Whether a zero tip is ever included depends on the block producers' minimum tip, which I could not check offline.

## Considered and rejected

- The zero-tip behaviour for `0` and negative caps is documented in code (`crates/core/src/tx/fees.rs:10-11`: "A cap of 0% or negative disables priority fees"). What is not documented is that bumps can never lift it, and that NaN behaves the same.
- `max_in_flight_transactions` cannot be combined with `executor`, and Batched mode always uses 1 (`crates/core/src/tx/config.rs:65-74`), so case 1 needs Direct mode.
- Caps of 100 or more, and infinity, are no-ops (probe section A), which matches the documentation.

## Remediation options

1. Reject `max_in_flight_transactions = 0`, a non-finite `priority_fee_cap_percentage`, and `blocks_before_resubmit = 0` in `TryFrom<RawConfig>`, the same way invalid mode combinations are rejected today.
2. If a zero tip is meant to be allowed, warn at startup, or let `bump_fee` raise a zero tip to the fresh uncapped estimate (or at least 1 wei) so resubmissions can still make progress.

Tests to add: `rejects_invalid_submission_mode` cases for the three values.

Pre-existing at run 2's commit `fe9e84c`: yes. The fields existed in `tx/mod.rs` with no range checks, the fee arithmetic is unchanged, and the old submission loop `for _ in in_flight..self.config.max_in_flight_transactions` also never runs for 0 (`git show fe9e84c:crates/core/src/tx/mod.rs`, lines 203-206). The settings moved to `tx/config.rs` in #991 and #992 without gaining checks.

## Trail

- 2026-10-08 Reviewer R3 (security): drafted. E1 for parsing, the zero tip and the never-submitting queue (probe compiled against the audit worktree, no tracked file changed). Self-estimate 90% for the mechanism; impact depends on operator error (A1).

## Critic (C3-CORE, 2026-10-08)

| Row | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | `crates/core/src/tx/config.rs:101-135` validates only the mode combination; probe section D. |
| 2 | Supported | `crates/core/src/tx/mod.rs:236-237`; probe section E3. |
| 3 | Supported | `crates/core/src/tx/fees.rs:16` (`f64::max` returns the non-NaN operand); probe section A. |
| 4 | Supported | `fees.rs:53-56`; probe sections C and E2. |
| 5 | Supported | `tx/mod.rs:292`; `crates/core/src/tx/storage.rs:401`. Precision: `blocks_before_resubmit = 1` also resubmits on every block (a row submitted at `b` is due at `b + 1`, where `submitted_before = b`), so 0 behaves like 1; item 3 is an undocumented value, not a distinct failure. |
| 6 | Supported | `crates/validator/validator.sample.toml:100-104`. |

Finding verdict: Confirmed. Severity: Informational to Informational. Certainty: 85, band 70 to 89 (items 1 and 3 and the zero-tip arithmetic are E1; whether block producers on Gnosis ever include a zero-tip transaction is not verified, so the "stops inclusion" half of item 2 stays inferred). Duplicate: canonical.

Confirmation. Each accepted value does what the file says, the operator is trusted (A1), and nothing logs the condition: `max_in_flight_transactions = 0` leaves every action queued forever while the account is still read every block (`tx/mod.rs:207-216`). The documented part of item 2 (0 or negative disables priority fees, `fees.rs:10-11`) is intended; the undocumented parts are NaN and the fact that no bump can lift a zero tip.

QA recipe. E1 exists for parsing and arithmetic. Remaining: zero-tip inclusion cannot be shown offline (Anvil includes zero-tip transactions), so QA records that half as not attempted. Bite check for remediation 1: add `tx/config.rs` test cases rejecting `max_in_flight_transactions = 0`, `priority_fee_cap_percentage = nan` and `blocks_before_resubmit = 0`, and confirm each fails before the fix.

## QA (QA3-CORE, 2026-10-08)

Result: Reproduced (bite check on the existing E1).

Route: a `tx/config.rs` test that parses `[transactions]` tables through the real `Config` deserializer: `max_in_flight_transactions = 0`, `priority_fee_cap_percentage = nan` and `blocks_before_resubmit = 0` are all accepted at `361aaf3` (the test, which expects rejection, fails and lists all three). R3's probe already holds E1 for the queue that never submits and the zero tip that no bump lifts.

Command and output: `poc/F3-CORE-061/` (`rejection-test-at-361aaf3.txt`).

Bite check: remediation 1 (reject the three values in `TryFrom<RawConfig>`): each is rejected with a message naming the field; the core suite (152 tests) and the validator and sentinel config tests pass. Sound. Rejecting `blocks_before_resubmit = 0` is cosmetic, since 1 behaves the same (Critic row 5). Zero-tip inclusion on Gnosis not attempted: Anvil includes zero-tip transactions, and no Gnosis client policy can be checked offline.

Certainty: 90 (E1, Critic Confirmed; the inclusion half of item 2 stays inferred).

## Practicality (PC3, 2026-10-09)

| Field | Value |
| --- | --- |
| Actor | The operator (A1), writing `max_in_flight_transactions = 0`, a non-positive or `nan` `priority_fee_cap_percentage`, or `blocks_before_resubmit = 0` |
| Cost | n/a |
| Condition | One of those values in the operator's own `[transactions]` table; none is in a sample or in `docs/configuration.md` |
| Gate | Fails gate 3 (trusted role only, no amplifier) and gate 4 in the security sense (self-inflicted; the operator's own node stops sending or overpays) |
| Practicality | theoretical (operator-only, self-inflicted) |
| Recommended severity | Informational (reviewer, Critic and QA agree) |
| Intended route and parameters | QA3-CORE parsed the values through the real `Config` deserializer; R3's probe holds the queue and fee arithmetic |
| Cheapest settling change | None needed |

Reason. Hardening: reject the three values at startup so a typo fails loudly instead of silently stopping a node.

## Confidence (CA3, 2026-10-09)

| Field | Value |
| --- | --- |
| Result | Evidence holds. Final: Accepted, Informational, certainty 90 |
| Evidence check | `poc/F3-CORE-061/rejection-test-at-361aaf3.txt`: `max_in_flight_transactions = 0`, `priority_fee_cap_percentage = nan` and `blocks_before_resubmit = 0` all accepted |
| Final severity | Informational, agree with PC3 (operator values) |

## Reconciliation (run 3)

| Field | Value |
| --- | --- |
| Relation | CONFIRMS `F-CORE-066` (the same three values; run 2's `F2-CORE-065` is folded there) |
| Canonical | `F-CORE-066` |
| Ledger values | Informational, certainty 90 (this file's CA3 value: operator values under A1, PC3 theoretical). Run 1's Medium and run 2's Low are recorded as dissent in the ledger |
| Source | Ledger: [`report/RECONCILIATION.md`](../report/RECONCILIATION.md) sections 2 and 3 |
