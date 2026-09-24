# PoC — F-CORE-002

**Never compiled, never run.** No Rust toolchain on the audit host (`rust-audit/state/baseline.md` §1). Identifiers checked by hand against commit `2893917`; expect mechanical fixes on first build.

## What it shows

This is the most consensus-relevant claim in `safenet-core`: **a block's logs are committed as complete when they are not.**

`use_client_filtering = true` is what the validator handbook (lines 31-40) tells operators to set when their RPC provider serves logs unreliably. Its integrity check is a bloom equality against the block header (`crates/core/src/index/events.rs:450`). That check runs only while `retries < block_single_query_retry_count`; every failure increments `retries`, **including the `IncompleteLogs` error the check itself raises**. Once the budget is spent the watcher switches to a node-filtered query with no completeness check at all, and whatever the node returns — `[]` included — is accepted. The state machine commits a snapshot at that block, so the block is permanently recorded as processed and never re-fetched.

For a validator that means silently dropped `Coordinator` events: a missed `Sign`, `KeyGenCommitted`, `KeyGenSecretShared` or `Preprocess`. For a sentinel, a missed `NewRequest` / `Committed` / `Revealed`.

## Where the code goes

`Fetch`, `Step` and `EventWatcher::block` are private to `events.rs`, and `Provider::mocked` sits behind `#[cfg(any(test, feature = "test-util"))]` which `cargo test -p safenet-core` does not enable for an integration test target (`crates/core/Cargo.toml` does not add a self dev-dependency with that feature). Paste the three tests into the existing `#[cfg(test)] mod tests` block at the bottom of `crates/core/src/index/events.rs`, immediately before its closing `}`. They reuse that module's `watcher`, `log`, the `Erc20` `sol!` block and the `WATCHED` constant.

```
# from the repo root
$EDITOR crates/core/src/index/events.rs    # paste poc_events.rs before the final }
cargo test -p safenet-core --lib index::events::tests::poc_f_core_002
git checkout -- crates/core/src/index/events.rs
```

## Fixtures — spelled out

Under A4 a stale, rate-limited or incomplete RPC response is in scope (a _lying_ RPC is not, and this PoC needs no lying node).

| Fixture | Literal value |
| --- | --- |
| Config (test 1) | `use_client_filtering: true`, `block_single_query_retry_count: 1`, rest default |
| Config (test 2) | `use_client_filtering: true`, `block_single_query_retry_count: 3` (**the shipped default**, `events.rs:98`) |
| Watched address | `WATCHED` = `0x1111111111111111111111111111111111111111` |
| Watched topics | `Erc20::Transfer` and `Erc20::Approval` (the test module's event set — two topics, hence two per-topic queries in the fallback) |
| Block | `number = 1337`, `hash = 0x1313…13` |
| `logs_bloom` | `bloom::compute_logs_bloom([Transfer{amount:1} from WATCHED])` — a **non-zero** bloom, i.e. the header asserts a watched log IS in this block |
| Node responses (test 1) | `[]` · `[]` · `[]` |
| Node responses (test 2) | `429 Too Many Requests` ×3, then `[]` · `[]` |
| Warp (test 3) | `BlockUpdate::Warp { from: 1, to: 50 }`, `block_page_size: 100`, one `[]` response |

## Reading the result

**Test 1 — `poc_f_core_002_incomplete_logs_exhaust_the_budget_that_detects_them`**

- The first `assert_matches!` (`Err(IncompleteLogs { block_hash: 0x1313…13 })`) is expected to **hold** on this checkout; a failure there is a harness problem.
- **Fails at the second `assert_matches!`, with the message reporting `Ok(Some(EventUpdate { blocks: 1337..=1337, logs: [] }))`** → **the finding reproduces.** The node's second, entirely unverified answer was accepted as authoritative for a block whose own header bloom says a watched log is present.
- **Passes** → the completeness check survived the retry budget. Fixed.

**Test 2 — `..._transient_failures_strip_the_integrity_check_default_budget`**

Same reading, at the **shipped default** budget of 3 and with no incomplete response at all. This is the cheaper and more damning trigger: three HTTP 429s from a rate-limited provider strip the integrity check off the very next attempt, and the answer then accepted comes from a node that would have passed the check.

**Test 3 — `poc_f_core_002_warp_range_is_never_bloom_verified`**

Documentary, not a bug assertion. It is expected to **pass** on this checkout, recording that a 50-block warp — the path every restart takes — is served by one unverified node-filtered query even with `use_client_filtering` enabled. Keep it: it pins the current behaviour so any future change to warp verification breaks visibly, and it is the evidence for remediation option 4 (document the limit).

## A caution when running these

`Asserter` is a strict FIFO of canned responses. If a fix changes the _number_ of RPC calls (for example by keeping `Fetch::ClientFiltered` on every attempt, which is one call rather than two per-topic calls), these tests will fail on a queue-length mismatch rather than on the assertion of interest. Read the failure message before concluding anything: `assert!(asserter.read_q.is_empty)` at the end of each test is there to make that distinguishable.

## Remediation check

See the `## QA (QA-CORE-SEN)` section of `rust-audit/findings/F-CORE-002.md`.
