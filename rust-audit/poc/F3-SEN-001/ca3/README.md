# F3-SEN-001: CA3 reruns (2026-10-09)

Worktree `wt/rv-sen` at `361aaf3`, env `env/rv-sen.sh`, Anvil port 31500. The sentinel binary was rebuilt first (the target directory held a build from after QA3-SEN's bite edit). The USDC.e code and slots were re-extracted from `~/.foundry/cache/rpc/xdai/48635154`: code hashes `0xad30d8…c435` (proxy) and `0xaada1f…deb4` (implementation), identical bytes, identical slot set.

| Run | Command | Result |
| --- | --- | --- |
| Direct mode 1 | `bash e2e.sh buggy1` | `run-buggy1.txt`: approve status 0, gas 54,629 of 55,000; commit status 0 (100,202); 0 `Committed`; allowance 0; vote NONE |
| Direct mode 2 | `bash e2e.sh buggy2` | `run-buggy2.txt`: identical |
| Batched mode control | `bash e2e-batched.sh batched1` (same harness plus a `Safenet7702Executor` and `executor` in the sentinel config) | `run-batched1.txt`: 1 `Committed`, vote recorded, the sentinel revealed and claimed (balance +400,000, oracle balance 0) |
| Gas threshold, second method | C3-SEN's forge project (the local run folder's `scratch/C3-SEN/usdc-gas`), `forge test -vv --offline --isolate --match-test "test_c\|test_d\|test_e"` | `forge-isolate.txt`: forwarded 34,000 fails, minimum transaction gas limit 55,819, a 55,000 callee budget succeeds |

The `e2e.sh` cleanup trap leaves the sentinel and engine stub running; they were killed by PID after each run and the post-teardown log spam was trimmed with `trim_logs.py`.
