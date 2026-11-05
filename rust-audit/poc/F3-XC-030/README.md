# F3-XC-030 PoC (QA3-CORE)

The real `Driver` (real time, 1 s blocks, default `max_reorg_depth` 5) on an in-memory SQLite database, with the RPC mocked at the transport boundary. The test service returns an action for every watched event, like a validator's last `KeyGenCommitted` returning its `KeyGenSecretShare`. The queue keeps rows in its `transactions` table (`max_in_flight_transactions = 0`, so nothing is sent), so queued actions can be counted.

The crash is a test-only hook in `Driver::update`, right before `self.transactions.queue(...)` (`driver.rs:283`): the driver task panics, so nothing more reaches SQLite, as with kill -9, an OOM kill or a host failure. Run 0 follows the chain live; after 15 s of downtime run 1 restarts and warps over a page holding the event; run 2 restarts on the same database.

Scenarios: a warp without a crash (control), a crash on a live block (control), and a crash after a warp page's commit and prune (the finding).

## Run

In a worktree at `361aaf3`:

```sh
git apply warp-crash-test.patch
cargo test -p safenet-core --lib qa3_crash_after_a_warp_page_prune_loses_its_actions -- --nocapture --test-threads 1
```

Bite check: `warp-crash-test-plus-remediation1.full.patch` adds remediation 1 (prune after the queue insert).

## Files

- `warp-crash-test-at-361aaf3.txt`, `warp-crash-test-with-remediation1.txt`
