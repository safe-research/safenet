# F3-XC-160 second reproduction (QA3-2B)

Watcher-level test driving the real combined `Watcher` (block + event watchers wired through `index/mod.rs` `next_logs` -> `revalidate_last_block`) with a mocked Asserter. The two tests in `xc160_watcher_tests.rs.txt` were added to the tests module of `crates/core/src/index/mod.rs` as a temporary hunk, run, then reverted.

## Command

```
source .../env/qa-2b.sh
cargo test --package safenet-core --lib index::tests::qa3_2b_xc160
```

## Result: reproduced (E1, both tests pass at 361aaf3)

```
test index::tests::qa3_2b_xc160_null_revalidation_reports_false_uncle ... ok
test index::tests::qa3_2b_xc160_control_canonical_revalidation_does_not_uncle ... ok
```

A -32001 on a block's logs followed by a `null` revalidation (`eth_getBlockByNumber` from a lagging backend) makes `watcher.next()` emit `Uncle{1000}` for a block that is still canonical. The control (same hash) does not uncle; the logs error is surfaced for a retry. So a `null` is wrongly treated like a real reorg (different hash) rather than like the canonical "not available yet" case.

The downstream consequence (the rollback drops a committed sentinel request, no reveal) is the known #973/#961 hole; the same ignore-own-commit-after-rollback mechanism is reproduced end to end in `rust-audit/poc/F3-SEN-004/repro-2`.

## Bite check: sound

Remediation option 1 in `revalidate_last_block` (treat a `None` current block as "not available yet", return `Ok(None)`; uncle only on a different hash). After the fix:

```
test ...qa3_2b_xc160_null_revalidation_reports_false_uncle ... FAILED
  panicked ... called `Result::unwrap()` on an `Err` value: Events(Rpc(ErrorResp(-32001 ...)))
test ...qa3_2b_xc160_control_canonical_revalidation_does_not_uncle ... ok
```

The null case now returns the -32001 error for retry instead of a false uncle; the control (and a genuine different-hash reorg) still uncles. Remediation sound.
