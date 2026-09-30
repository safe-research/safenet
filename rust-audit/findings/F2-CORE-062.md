# F2-CORE-062 First-submission "underpriced" rejections are not recognised and a zero priority fee can never be bumped; `priority_fee_cap_percentage <= 0` stalls all submissions on geth-family nodes

| Field | Value |
| --- | --- |
| Status | QA'd |
| Crate and module | safenet-core, tx/mod.rs, tx/fees.rs |
| Location | crates/core/src/tx/mod.rs:362-368 (related: tx/mod.rs:284-293; tx/fees.rs:11, 16-19, 52-56) |
| Severity | Medium (reviewer) / Medium (Critic) |
| Certainty | 65% (QA2-CORE; Critic C2-CORE-B set 55%) |
| Assumptions involved | A1, A4 |
| Tags | config, dos, input-validation |

## Claim

`is_transaction_underpriced` only matches the wording nodes use when rejecting a _replacement_ ("replacement transaction ... underpriced" from geth, "could not replace existing tx" from Erigon). The wording for a _first_ submission whose tip is below the node's mempool floor -- geth and Erigon `transaction underpriced` / `underpriced`, Nethermind `FeeTooLow` (Nethermind wording not verified on disk) -- lands in the generic branch, which by design records no fee floor and does not bump. The row keeps its nonce with `submitted_at = NULL`, is retried on every block with the unchanged fresh estimate, and stays rejected for as long as the estimate is below the floor; every later nonce is allocated behind it (up to `max_in_flight_transactions`) and is rejected for the same reason.

The estimate cannot self-heal in the case that matters: `cap_priority_fee` documents that a cap of 0% or negative "disables priority fees", producing a tip of exactly 0, and `bump_fee(0, 0) = 0 + ceil(0 / 10) = 0`, so no replacement can ever raise a zero tip either. geth's default `--txpool.pricelimit` is 1 wei, so with `priority_fee_cap_percentage = 0` -- a value the code comment presents as valid -- every submission through a geth or Erigon RPC is rejected forever, at warn level, and the service never transacts. (alloy's estimator floors the tip at 1 wei, so a zero tip is reachable only through the cap; NaN also yields 0, see F2-CORE-065.)

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Only replacement wording is recognised. | E2 | crates/core/src/tx/mod.rs:362-368 | `fn is_transaction_underpriced(err: &TransportError) -> bool {` / `err.as_error_resp().is_some_and(\|payload\| {` / `(iregex!("replacement transaction").is_match(&payload.message)` / `&& iregex!("underpriced").is_match(&payload.message))` / `\|\| iregex!("INTERNAL_ERROR: could not replace existing tx").is_match(&payload.message)` / `})` / `}` |
| 2 | Unrecognised rejections record nothing and retry unchanged. | E2 | crates/core/src/tx/mod.rs:284-293 | `// Other failures do not establish that the transaction reached the` / `// mempool or that its fees were insufficient. Leave the last` / `// accepted fee floor unchanged and retry without increasing it.` / `Err(err) => {` / `tracing::warn!(` / `nonce = submission.nonce,` / `?err,` / `"submission failed, will retry without bumping fees"` |
| 3 | Rows never recorded as submitted are retried on every block. | E2 | crates/core/src/tx/storage.rs:296-297 | `WHERE nonce IS NOT NULL AND executed_at IS NULL` / `AND (submitted_at IS NULL OR submitted_at <= ?)` |
| 4 | A cap of 0 or negative is documented as valid and produces a zero tip. | E2 | crates/core/src/tx/fees.rs:11, 16, 24-25 | `/// priority fee to it (never raising it). A cap of 0% or negative disables` / `/// priority fees, and a cap of 100% or more is a no-op.` ... `let scaled_percent = ((cap_percentage.max(0.0) / 100.0) * PRECISION as f64).round() as u128;` ... `let capped = base_fee.saturating_mul(scaled_percent) / (PRECISION - scaled_percent);` / `let max_priority_fee_per_gas = fees.max_priority_fee_per_gas.min(capped);` |
| 5 | The existing test pins the zero-tip outcome. | E1 | crates/core/src/tx/fees.rs:85-86 (run this session, passes) | `// A cap of 0% disables the priority fee, leaving just the base fee of 40.` / `assert_eq!(cap_priority_fee(fees(100, 60), 0.0), fees(40, 0));` |
| 6 | A zero component is never raised by a bump. | E2 | crates/core/src/tx/fees.rs:53-55 | `fn bump_fee(fresh: u128, previous: u128) -> u128 {` / `let bumped = previous.saturating_add(previous.div_ceil(10));` / `fresh.max(bumped)` |
| 7 | The pinned alloy estimator floors the tip at 1 wei, so 0 arises only via the cap. | E2 | ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/alloy-provider-2.0.5/src/utils.rs:26, 107 | `pub const EIP1559_MIN_PRIORITY_FEE: u128 = 1;` ... `std::cmp::max(median, EIP1559_MIN_PRIORITY_FEE)` |
| 8 | geth rejects a tip below `txpool.pricelimit` (default 1 wei) with `transaction underpriced`; Erigon with `underpriced`; neither contains "replacement". | I | go-ethereum `core/txpool/errors.go` (`ErrUnderpriced`), Erigon `txpool` discard reasons -- protocol knowledge, not on disk | -- |
| 9 | The existing test corpus contains no first-submission wording, so the gap is untested. | E2 | crates/core/src/tx/mod.rs:426-430 | `for message in [` / `"replacement transaction is underpriced",` / `"rEpLaCeMeNt TrAnSaCtIoN uNdErPrIcEd",` / `"INTERNAL_ERROR: could not replace existing tx",` / `] {` |

## Trigger

Set `priority_fee_cap_percentage = 0` (or any negative, or `nan`) in `[transactions]` and point `rpc` at a geth or Erigon node with default pool settings. Every queued action is allocated a nonce, signed with `max_priority_fee_per_gas = 0`, rejected with `transaction underpriced`, and retried identically every block; the warn log is the only symptom. Without the cap, the same stall occurs whenever a node's configured price floor exceeds the 20th-percentile tip estimate (an operator-hardened `--txpool.pricelimit`, a quiet chain with many zero-tip blocks), and lasts until the market estimate crosses the floor.

## Considered and rejected

- _A recognised first-submission rejection would fix it._ Only when the tip is non-zero: with a zero floor, `bump` returns 0 forever (Basis 6), so both the classifier and the arithmetic need fixing.
- _Nethermind accepts a zero-tip transaction._ Possibly into the pool, but block producers on Gnosis are Nethermind with a default `MinGasPrice` of 1 wei; whether a zero-tip transaction is ever included there is not verified here and is recorded as an observation.
- _No operator would set 0._ The code comment and the handbooks present the cap as a fee-saving knob; "disables priority fees" reads like a feature.

## Remediation options

1. Recognise first-submission underpriced wording (`underpriced` without `replacement`, `FeeTooLow`, `fee too low`) as a floor-establishing rejection, and make `bump_fee` raise a zero component to at least 1 wei (or a configurable minimum tip).
2. Reject `priority_fee_cap_percentage <= 0` (and non-finite values) at configuration load, or clamp the resulting tip to a minimum of 1 wei; document that tips cannot be disabled.
3. Add a resubmission counter per row and log at error / expose a metric when a row has been rejected N times without a floor change, so the stall is visible.

Tests to add: `is_transaction_underpriced("transaction underpriced")`; a queue test with cap 0 and `push_failure_msg("transaction underpriced")` on every block asserting the tip is raised on retry.

## Trail

- Reviewer R3: drafted at commit 3ec8bc5, self-estimate 55%
- Critic C2-CORE-B: Plausible, 55%, severity Medium (reviewer Medium).
- QA2-CORE: Reproduced for the in-crate mechanism (classifier gap, zero tip never bumped, identical retries); node wording not attempted (mocked); certainty 55 → 65; PoC `poc/F2-CORE-062/`.

## Critic (C2-CORE-B)

Method: read title and Location only, traced `is_transaction_underpriced`, the three arms of `submit_transaction` and the cap/bump arithmetic myself, then compared.

| # | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | mod.rs:362-368: both regexes must match ("replacement transaction" AND "underpriced"), or the Nethermind/Erigon "could not replace existing tx" string. |
| 2 | Supported | mod.rs:284-293. |
| 3 | Supported | storage.rs:296-297. |
| 4 | Supported | fees.rs:11, 16, 24-25. |
| 5 | Supported (E1) | fees.rs:85-86; passes in my run (`tx::` tests, 41 passed with `state::`/`effects::`). |
| 6 | Supported | fees.rs:53-55. |
| 7 | Supported | `alloy-provider-2.0.5/src/utils.rs:26` `EIP1559_MIN_PRIORITY_FEE: u128 = 1`, 97 (empty rewards → 1) and 107 `max(median, EIP1559_MIN_PRIORITY_FEE)` — re-opened on disk. |
| 8 | Unverifiable here, stays `I` | No go-ethereum or Erigon source is on disk. The only local node implementation is Anvil, whose binary carries `replacement transaction underpriced` but no first-submission "underpriced" string (`strings ~/.foundry/bin/anvil`), so the repo's own integration environment cannot exercise this branch in either direction. Not `H`: the claim is about a dependency outside the checkout, not a misquoted location. |
| 9 | Supported | mod.rs:426-430. |

Own arithmetic: with `cap <= 0`, `scaled_percent = 0`, `capped = 0`, so `p = min(p, 0) = 0` and `mf = base` (fees.rs:16-30); on resubmission `bump_fee(0, 0) = 0 + ceil(0/10) = 0` and `max(fresh = 0, 0) = 0` — a zero tip is never raised, as claimed. `NaN` gives `f64::max(NaN, 0.0) = 0.0`, the same. The classifier gap is independent of the config: any node floor above the fresh estimate puts the row on the generic path, where nothing ever bumps.

Finding verdict: **Plausible** — the mechanism is verified (E1/E2), the decisive trigger step (the node's wording and default floor for a first submission) is unverified offline. Certainty **55%**. Severity **Medium / Medium**: a total, silent stall of the service's on-chain output from a value the code documents as valid ("A cap of 0% or negative disables priority fees", fees.rs:10-11); A1 tempers but does not remove it, and the classifier half needs no misconfiguration. Interplay: an unrecognised first-submission rejection is exactly the "row N blocked" feeder that makes F2-CORE-060 compound on every row behind it.

QA note: `is_transaction_underpriced("transaction underpriced")` returning `false` is a one-line E1 for row 1; the node-wording question needs a geth/Erigon/Nethermind node, which QA may not have.

## QA (QA2-CORE)

**Outcome: Reproduced** for the in-crate mechanism (classifier gap, zero tip never bumped, identical retry on every block). The node-side half (row 8: first-submission wording and default price floor) was **not attempted**: Anvil carries no first-submission "underpriced" string and no geth/Erigon/Nethermind node is available offline, so the wording is mocked.

Command: paste `poc/F2-CORE-062/tx_mod_tests.rs` into the `mod tests` of `crates/core/src/tx/mod.rs`; `cargo test -p safenet-core --lib qa_f2_core_062 -- --nocapture --test-threads=1`; file reverted.

Decisive output (`poc/F2-CORE-062/output.txt`): `"transaction underpriced" -> is_transaction_underpriced = false` (likewise `underpriced`, `FeeTooLow`, `fee too low`, `max priority fee per gas below minimum`); `cap 0%  : {210, 10} -> {200, 0}` and `cap NaN : .. -> {200, 0}`; `replacement 1..5` all keep `max_priority_fee_per_gas: 0`; at queue level with `priority_fee_cap_percentage = 0` and the node answering `transaction underpriced`, after 11 blocks both rows hold nonces 0 and 1 with `submitted_at None` and no fee fields in `request` — re-sent unchanged on every block (22 sends, 22 rejections).

Unexecuted half, for the record: against a geth node with the default `--txpool.pricelimit 1` and `priority_fee_cap_percentage = 0`, the expected `eth_sendRawTransaction` error is `transaction underpriced` on every block and the expected queue behaviour is the mocked one above.

Certainty: 55 → **65**. Plausible band: mechanism `E1`, decisive node wording still `I`.

Remediation check: option 2 (reject a `<= 0` or non-finite cap at load) is the smallest sound fix for the configuration half; option 1 is needed for the classifier half and must raise a zero component explicitly (`max(bumped, previous + 1)`), since `0 + ceil(0 / 10) = 0`. Option 3 sound (visibility).

## Reconciliation (run 2)

**Final: EXTENDS `F-CORE-061` (canonical) — combined Medium, 65 (E1 in-crate).** Same classifier gap; this file adds the zero-tip arithmetic and the `priority_fee_cap_percentage <= 0` / NaN stall. Node wording stays `I` in both runs (`state/run2/reconciliation/core.md` §1). `F2-CORE-065` / `F-CORE-066` is the NaN entry point.
