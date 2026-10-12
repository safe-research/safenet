# F3-CORE-002 PoC (QA3-CORE)

Two levels.

1. Unit (the Critic's recipe), `unit-test.patch`: the real `EventWatcher` with `use_client_filtering = true` and an `Asserter` that answers `[]` five times. Three `IncompleteLogs`, then the per-event fallback returns `Ok(Some(EventUpdate { blocks: 1337..=1337, logs: [] }))` for a block whose header bloom holds a watched `Transfer`.
2. Driver, real time, `driver-too-soon-test.patch`: the real `Driver` with `use_client_filtering = true` follows a chain mocked at the transport boundary. Block 1003 holds one watched `Ping`; the node answers every `eth_getLogs` for a block with `[]` until a "too soon" window after it first served that block's header (the handbooks' behaviour), then with the real logs. The test prints each logs query for block 1003 with its age, for windows of 200, 500, 1,000 and 2,000 ms.

## Run

In a worktree at `361aaf3`:

```sh
git apply unit-test.patch
cargo test -p safenet-core --lib qa3_client_filtering_accepts_empty_logs_after_three_mismatches -- --nocapture
git restore crates/core && git apply driver-too-soon-test.patch
cargo test -p safenet-core --lib qa3_client_filtering_loses_events_inside_a_short_too_soon_window -- --nocapture
```

Bite checks on top of either test: `git apply remediation2.patch` (an empty per-event answer for a block whose header bloom may hold a watched log is an error, unbounded). The backoff variant reuses `../F3-CORE-001/remediation1-backoff.patch`.

## Files

- `unit-test-at-361aaf3.txt`, `unit-test-with-remediation2.txt`
- `driver-test-at-361aaf3.txt`, `driver-test-with-remediation2.txt`, `driver-test-with-remediation1-backoff.txt`
- `unit-test-plus-remediation2.patch`: both hunks together, as run.
