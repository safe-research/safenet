# F3-CORE-004 PoC (QA3-CORE)

The real `Driver` (real time, 1 s blocks, default `max_reorg_depth` 5) follows chain A, mocked at the transport boundary. Every block carries one watched `Ping` whose value tags its chain, so the persisted state shows which branch each applied event came from. Chain B replaces A from a fork block.

- Path B, `qa3_deep_reorg_while_stopped_is_accepted_on_restart`: run 1 follows A and is stopped; while it is down, B replaces 8 blocks (two below the oldest retained snapshot); run 2 restarts on the same database.
- Path A, `qa3_deep_reorg_exit_is_forgotten_on_restart`: B replaces 6 blocks live; run 1 exits on its own (`ExceededMaxReorgDepth`); run 2 restarts.

## Run

In a worktree at `361aaf3`:

```sh
git apply deep-reorg-restart-tests.patch
cargo test -p safenet-core --lib qa3_deep_reorg -- --nocapture --test-threads 2
```

Bite check: `deep-reorg-tests-plus-remediation1.full.patch` holds the tests, a no-reorg restart control (`qa3_restart_without_reorg_still_resumes`) and the remediation sketch (`remediation1-snapshot-hash.patch` alone): the hash of each live block is stored with its snapshot, and `Driver::new` refuses to resume when the node's block at the oldest snapshot has another hash.

## Files

- `deep-reorg-tests-at-361aaf3.txt`, `deep-reorg-tests-with-remediation1.txt`
