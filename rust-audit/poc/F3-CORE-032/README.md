# F3-CORE-032 PoC (QA3-CORE)

1. Core mechanism: R2's harness binary in the local run folder re-run unchanged (`scratch/R2/target/debug/deps/r2_poc-0770c0a9a0e420d8 lost-effect 1001` and `1002`), output in `r2-harness-rerun.txt`.
2. Sentinel consequence, `sentinel-rollback-test.patch`: the real core `StateMachine` (SQLite snapshots, real `Uncle` rollback) driven with the real `SentinelTransition`. `TransactionProposed` and `NewRequest` land in block S (Aegis amounts in base units: fee 400000, bond 800000000, slash 400000, commit deadline S + 6, reveal deadline S + 12); the engine verdict resumes after S's snapshot, as `Driver::update` spawns effects only after `handle_update`; the sentinel's own `Committed` lands in S + 2. Without a rollback the run reaches a `Reveal` at S + 7; with `Uncle { S + 1 }` (a one-block reorg, or a restart whose restore point is S) it does not.

## Run

In a worktree at `361aaf3`:

```sh
git apply sentinel-rollback-test.patch
cargo test -p sentinel --bins qa3_rollback_to_the_proposal_block -- --nocapture
```

Bite check: `remediation2-replay-verdict-plus-tests.patch` adds a minimal form of remediation 2 (a `ReplayVerdict` effect emitted on every block while a request waits for its check, answered from the `VerdictStore`) and `qa3_rollback_with_replayed_verdict`, which emulates the driver performing it, once with the replayed verdict resuming before the own commit is replayed and once after.

```sh
cargo test -p sentinel --bins qa3_ -- --nocapture --test-threads 1
```

## Files

- `sentinel-test-at-361aaf3.txt`, `sentinel-test-with-remediation2.txt`, `r2-harness-rerun.txt`
