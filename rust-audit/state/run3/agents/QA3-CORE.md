# QA3-CORE coverage log (run 3, Phase 3, primary QA for core)

Agent: QA3-CORE (tester department). Worktree `wt/rv-core-a` at `361aaf3`, env `env/rv-core-a.sh`, Anvil ports 31200 to 31208 only. Work ran from 18:53 to 20:15 UTC on 2026-10-08. Every PoC hunk was applied in the worktree only and reverted; the worktree ended clean (`git status --short` empty), the binaries in `target-rv-core-a` were rebuilt at `361aaf3`, and no Anvil, proxy or service of mine is left running. Scratch scripts and raw logs: the local run folder's `scratch/QA3-CORE/`.

## Findings handled

| Finding | Result | Route | PoC folder | Bite check | Certainty |
| --- | --- | --- | --- | --- | --- |
| F3-CORE-001 | Reproduced | real `Driver`, transport-level rate limiter, 16 events, real time | `poc/F3-CORE-001/` | remediation 2 (single query after a failed per-event attempt) removes it on the block path, not on warp pages; remediation 1 alone does not | 80 |
| F3-CORE-002 | Reproduced | unit on `EventWatcher`; real `Driver` with a "too soon" node, real time | `poc/F3-CORE-002/` | remediation 2 delivers every window (needs its time bound); driver backoff covers about 0.8 s | 75 |
| F3-CORE-004 | Reproduced | real `Driver`, chain A then B, paths A and B | `poc/F3-CORE-004/` | snapshot hash checked in `Driver::new` refuses both restarts; a no-reorg restart still resumes | 92 |
| F3-CORE-005 | Reproduced | real sentinel binary, Anvil headers 10 s ahead; paused-time unit | `poc/F3-CORE-005/` | skip-wait brings the 10.15 s lag to 0.47 s, about 2 extra polls per second while behind | 92 |
| F3-CORE-030 | Reproduced | real sentinel binary, proxy serving the same chain behind | `poc/F3-CORE-030/` | wait in `initialize` turns the exit into a wait | 95 |
| F3-CORE-032 | Reproduced | R2 harness re-run; real `StateMachine` with real `SentinelTransition` | `poc/F3-CORE-032/` | replayed verdict restores the reveal only when it resumes before the replayed own commit | 93 |
| F3-CORE-060 | Reproduced | real `Driver` and queue over HTTP, proxy swallowing sends, Anvil landing | `poc/F3-CORE-060/` | ceiling at 3x the fresh estimate keeps fees flat | 75 |
| F3-CORE-061 | Reproduced (bite check on E1) | config parsing test | `poc/F3-CORE-061/` | remediation 1 rejects all three values; suites pass | 90 |
| F3-XC-130 | Reproduced | sentinel binary, literal and `${NAME}` RPC URL | `poc/F3-XC-130/` | scheme check prints only the scheme | 95 |
| F3-XC-030 | Reproduced | real `Driver`, crash hook before the queue insert | `poc/F3-XC-030/` | prune after the queue insert restores the action | 85 |
| F3-XC-160 | Reproduced (modelled backend) | watcher unit; two real sentinels, engines and `SentinelOracle` on Anvil, proxy injecting -32001 and `null` | `poc/F3-XC-160/` | missing block treated as not yet available: A reveals and claims; one existing unit test pins the old behaviour | 75 |

## Commands (all after `source <run folder>/env/rv-core-a.sh`)

- Unit and driver PoCs: `git apply <patch>` from the PoC folder, then `cargo test -p safenet-core --lib <test name> -- --nocapture` (sentinel: `cargo test -p sentinel --bins <test name>`), then `git checkout -- <files>`.
- F3-CORE-001 also with `QA3_OUTAGE_MS=3000`.
- Binary runs: `f030.sh`, `f030-boundary.sh`, `f005.sh`, `f060.sh` (with the `qa3_fee` example copied into `crates/core/examples/` and moved out afterwards), `f160.sh`, each saved in its PoC folder with `procs.sh` and `rpcproxy.py`.
- F3-XC-130: the sentinel binary with three configs; R2 re-run from the local run folder: `scratch/R2/target/debug/deps/r2_poc-0770c0a9a0e420d8 lost-effect 1001` and `1002`.
- Every saved patch was checked with `git apply --check` against `361aaf3`.

## Not attempted, and why

- F3-CORE-001: no real Gnosis provider limit (no live RPC); `/health` during the stall not probed; remediation 3 not tried.
- F3-CORE-002: no real node's "too soon" window measured (no live RPC).
- F3-CORE-032: remediation 1 (core) not tried; the core API cannot match a resume to its effect.
- F3-CORE-060: no production route to an accepted but never included transaction (the proxy models trigger 2).
- F3-CORE-061: zero-tip inclusion; Anvil includes zero-tip transactions and Gnosis client policy cannot be checked offline.
- F3-XC-160: Gnosis client error codes for logs of an unknown block hash and load-balancer routing (no live RPC). Anvil 1.5.1 answers `[]` there, not -32001.

## Notes for the head

- Prettier flags `F3-CORE-030.md` and `F3-CORE-032.md` for the Basis table separator rows (`| - | ----- | ...`) written before QA; my sections pass. Left untouched, since earlier text is not to be edited.
- Test-clock artefact worth knowing for future driver-level PoCs: in `cfg(test)` builds every `BlockWatcher` restarts its clock at `TEST_SYSTEM_TIME_EPOCH_SECONDS`, so a restarted watcher sees header timestamps ahead of its clock (the F3-CORE-005 effect) unless the mock shifts timestamps per run.
