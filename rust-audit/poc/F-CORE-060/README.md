# PoC — F-CORE-060

**Never compiled, never run.** No Rust toolchain on the audit host (`rust-audit/state/baseline.md` §1). Identifiers checked by hand against commit `2893917`; expect mechanical fixes on first build. The expected fee values below were derived by evaluating `bump_fee`'s recurrence (`previous + previous.div_ceil(10)`, `crates/core/src/tx/fees.rs:52-56`) by hand, not by running the code.

## What it shows

Three separate claims, one per test:

1. **Unbounded.** A transaction the node keeps rejecting as an underpriced replacement has its fees multiplied by ~1.1 **every block**, compounding, with no ceiling. Over the 60 blocks this test drives — five minutes on Gnosis' ~5 s blocks (A10) — the priority fee goes from **10 to 4,037** wei/gas and the max fee from **210 to 59,550**, while the honest estimate never moves.
2. **The cap does not survive the bump.** With `priority_fee_cap_percentage = 1.0` configured, the realised ratio reaches **3.17%** after 29 bumps — more than triple the configured cap.
3. **The rate is per block, not per `blocks_before_resubmit`.** An underpriced rejection writes `submitted_at = NULL`, and `stale_submissions`' predicate is `submitted_at IS NULL OR submitted_at <= ?` (`crates/core/src/tx/storage.rs:293-296`), so the `NULL` short-circuits the block comparison entirely. This is the conflation that turns a per-two-block ratchet into a per-block one.

## Where the code goes

**Tests 1 and 2** → the existing `#[cfg(test)] mod tests` block at the bottom of `crates/core/src/tx/mod.rs`, before its closing `}`. They reuse that module's `queue`, `tx`, `block_status`, `fee_history`, `in_flight` helpers and its `CHAIN_ID` constant.

**Test 3** → the existing `mod tests` at the bottom of `crates/core/src/tx/storage.rs` (NOT `tx/mod.rs`): `TransactionStorage`, `Submission` and `Status` live in the private `tx::storage` module and are not nameable from `tx/mod.rs`'s test module. It reuses that module's `storage`, `tx` and `fees` helpers.

```
# from the repo root
$EDITOR crates/core/src/tx/mod.rs          # paste tests 1 and 2
$EDITOR crates/core/src/tx/storage.rs      # paste test 3
cargo test -p safenet-core --lib poc_f_core_060
git checkout -- crates/core/src/tx/mod.rs crates/core/src/tx/storage.rs
```

## Fixtures — spelled out

| Fixture | Literal value |
| --- | --- |
| `chain_id` | `1` |
| Transaction | `to = 0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789`, `data = 0x01`, `gas = 21_000`, `expires_at = None` |
| Signer | `SigningKey::from_slice(keccak256("test signer"))` (the module's own helper) |
| Signer nonce (every block) | `0` — the transaction never executes |
| Fee estimate (every block) | `FeeHistory { base_fee_per_gas: [100, 100], reward: [[10]] }` → `max_fee = 210`, `max_priority = 10`. **It never moves**, so every wei above 10 is pure ratchet. |
| Block 10 response | success, hash `0x00…00` — establishes the fee floor |
| Blocks 12+ response | JSON-RPC error `"replacement transaction underpriced"`, which matches `is_transaction_underpriced` (`crates/core/src/tx/mod.rs:359-365`) |
| Blocks driven | test 1: `10 ..= 70`; test 2: `10 ..= 40` |
| Config (test 2) | `priority_fee_cap_percentage: Some(1.0)`, everything else default (`max_in_flight_transactions: 16`, `blocks_before_resubmit: 2`) |

**Why the rejection is modelled as persistent.** This is trigger instance 1 from the finding: a provider that answers a replacement attempt with a replacement/internal error for a reason that is _not_ the fee level — a private or bundled mempool, a load-balanced backend that does not hold the original, or a rate-limit path reusing the internal-error code. Because the fee is never actually the reason, raising it never helps, and the ratchet runs for as long as the transaction is outstanding. That is squarely in scope under A4 (a stale, rate-limited or inconsistent provider response), which is distinct from a _lying_ RPC, which is out of scope.

**Why the cap in test 2 is 1% and not 5%.** A 5% cap would not show the violation: the honest estimate's own ratio is `10/210 = 4.76%`, already under 5%, and a uniform 1.1× bump of both components preserves the ratio. The violation comes from `bump_fee`'s `previous.div_ceil(10)` raising a _small_ priority fee by more than 10% while the much larger max fee rises by exactly 10%, so the ratio climbs on every bump. A 1% cap makes `cap_priority_fee` bind on the fresh estimate (`p ≤ 2` for `base_fee = 200`), which is the only configuration in which the cap can be observed to be violated at all. This is exactly the case `crates/core/src/tx/fees.rs:37` warns about in its own doc comment — "Note that fee bumps can cause priority fee caps to not be observed" — and which nothing ever re-checks.

## Reading the result

**Test 1 — `poc_f_core_060_underpriced_ratchet_is_unbounded_and_per_block`**

- **Fails at `assert!(asserter.read_q.is_empty)`** → the resubmission cadence is not what this PoC assumes (perhaps a fix changed it). Re-derive the response counts before reading anything into the fee.
- **Fails at `assert!(max_priority <= 100)` reporting `4037` / `59550`** → **the finding reproduces.** 100 is ten times the honest estimate — a deliberately generous stand-in for "any bound at all". The realised value is 404× the honest estimate after five minutes.
- **Passes** → some bound now exists (an absolute cap, a multiple of the fresh estimate, or a bounded consecutive-rejection count). Fixed.

**Test 2 — `poc_f_core_060_priority_fee_cap_does_not_survive_the_bump`**

- **Fails reporting `104` against `3277`** → **the finding reproduces**: 3.17% realised against a configured 1% cap.
- **Passes** → the cap is re-applied after the bump (remediation option 2), or the bump is bounded (options 1/3).

**Test 3 — `poc_f_core_060_underpriced_rejection_ignores_blocks_before_resubmit`**

- **Fails with a non-empty `stale`** → **the finding reproduces.** With `blocks_before_resubmit = 2`, a row rejected as underpriced at block 10 is eligible again at block 11. This is the multiplier that makes the ratchet ×1.1 per block rather than ×1.1 per two blocks — ×3.1/minute rather than ×1.8/minute on Gnosis.
- **Passes** → the `NULL` sentinel no longer conflates "never submitted" with "rejected as underpriced" (remediation option 5).

## Remediation check

See the `## QA (QA-CORE-SEN)` section of `rust-audit/findings/F-CORE-060.md`.
