# F3-VAL-030: CA3 reruns (2026-10-09)

Worktree `wt/rv-val` at `361aaf3`, env `env/rv-val.sh`, Anvil port 31400. The validator binary was rebuilt before the runs (the target directory still held QA3-VAL's patched bite build).

## Primary QA PoC, unchanged

`bash reorg_oracle_poc.sh` (copy of `../reorg_oracle_poc.sh`): two real validators (2 of 2), `SimpleOracle`, `anvil_reorg 3`. Output `rerun1-simpleoracle.txt`: REPRODUCED (attestation id non-zero, 0 approved and 1 denied `OracleResult`, 1 `TransactionAttested`, guard path `MATCH`).

## SentinelOracle variant at the Aegis windows (PC3's settling check)

`OUT_DIR=<scratch dir> bash reorg_sentinel_oracle_poc.sh`. Real `SentinelOracle` (commit 6, reveal 6, fee 400000, bond 2000x, slash 1x, governance delay 51840 mined before the validators start, arbitration timeout 483840), validators at the Aegis default `max_reorg_depth` 5 and the code defaults `signing_timeout` 6 and `oracle_timeout` 24, 2 s blocks. Two sentinel accounts are scripted with `cast`: S1 commits approve at T+1 and reveals at T+7 on branch A (the only commit, so its reveal finalizes: `OracleResult(true)`); after both validators' `SignShared` are mined, interval mining is paused and `anvil_reorg` replaces T+6 onward with S2's deny commit in T+6 and both reveals in T+7.

| Run | Output | Reorg depth | Canonical result |
| --- | --- | --- | --- |
| 1 | `sentinel-oracle-run1.txt` | 4 (T+6..T+9) | `DisputeTriggered` at T+7, request `FROZEN`, 0 `OracleResult`, `TransactionAttested` at T+11, guard path `MATCH` |
| 2 | `sentinel-oracle-run2.txt` | 4 (T+6..T+9) | same |

Validator log excerpts: `val030-so-run2-*-key-lines.txt` (run 1) and `val030-so-run4-*-key-lines.txt` (run 2): "oracle approved transaction; committing nonces", three "reorg detected, uncling block" lines, one "received attestation on unexpected signing state" per validator, no deep-reorg exit.

Two earlier attempts aborted on harness bugs before any vote or reorg (a hex block number parsed with `jq tonumber`; a `Sign` event picked with `.[-1]` instead of by message). Both were fixed in the script; they are not counted as runs.

## Second reproduction, unchanged

QA3-2A's `../repro-2/qa2a_val030_no_finality_wait.rs` appended to `crates/validator/src/state/sign.rs`, test binary run with filter `qa2a_val030`: 1 passed (`qa2a-val030.txt`). Reverted with `git checkout`.
