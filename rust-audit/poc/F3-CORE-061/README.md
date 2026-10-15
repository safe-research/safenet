# F3-CORE-061 PoC (QA3-CORE)

R3's probe already holds E1 for the parsing, the zero tip that bumps never lift, and the queue that never submits with an in-flight limit of 0 (`state/run3/logs/R3-fee-config-probe.txt`). This folder holds the bite check: a test that `[transactions]` rejects `max_in_flight_transactions = 0`, `priority_fee_cap_percentage = nan` and `blocks_before_resubmit = 0`, run before and after remediation 1.

## Run

In a worktree at `361aaf3`:

```sh
git apply rejection-test.patch
cargo test -p safenet-core --lib qa3_rejects_values_that_silently_stop_the_queue -- --nocapture
git restore crates/core && git apply rejection-test-plus-remediation1.patch
cargo test -p safenet-core --lib qa3_rejects_values_that_silently_stop_the_queue -- --nocapture
cargo test -p safenet-core --lib && cargo test -p validator -p sentinel --bins config
```

## Files

- `rejection-test-at-361aaf3.txt`: all three values accepted.
- `rejection-test-with-remediation1.txt`: all three rejected; core, validator and sentinel config suites pass.
