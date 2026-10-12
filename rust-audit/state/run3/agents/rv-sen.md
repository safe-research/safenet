# rv-sen agent log (run 3 re-validation, sentinel)

- Date: 2026-10-08, about 21:45 to 23:30 IST.
- Audit commit `361aaf3`; the local run folder's `wt/rv-sen` worktree; env `env/rv-sen.sh` (target `target-rv-sen`, Anvil ports 31500 and 31501 used).
- Rows: 19 of 19 done, none unfinished. Verdict table: `state/run3/revalidation/rv-sen.md`. Department report: `reports/rv-sen.md` in the run folder.

## Rows

| ID | Verdict | Run 3 artefacts |
| --- | --- | --- |
| F-SEN-001 | Still valid | `poc/F-SEN-001/run3/` |
| F-SEN-002 | Changed | `poc/F-SEN-002/run3/`, `poc/F2-SEN-002/run3/` |
| F-SEN-003 | Fixed | `poc/F2-SEN-003/run3/` |
| F-SEN-004 | Changed | `poc/F2-SEN-005/run3/` |
| F-SEN-005 | Changed | `poc/F2-SEN-004/run3/` |
| F-SEN-006 | Still valid | `poc/F2-SEN-007/run3/` |
| F-SEN-007 | Still valid | `poc/F-SEN-010/run3/`, `poc/F2-SEN-006/run3/` |
| F-SEN-008 | Changed (proposed High) | `poc/F-SEN-008/run3/` |
| F-SEN-009 | Still valid | `poc/F2-SEN-006/run3/`, `poc/F2-SEN-008/run3/` |
| F-SEN-010 | Still valid | `poc/F-SEN-010/run3/` |
| F-SEN-011 | Still valid | `poc/F2-CORE-030/run3/rv-sen-f-sen-011.txt` |
| F-SEN-012 | Still valid | `poc/F2-SEN-008/run3/` |
| F-SEN-013 | Still valid (refutation) | none (Cargo.lock comparison) |
| F-SEN-014 | Changed | `poc/F-SEN-014/run3/` |
| F-SEN-015 | Fixed | `poc/F-SEN-015/run3/` |
| F-SEN-016 | Superseded | via `F2-SEN-010`, `F-SEN-006` |
| F2-SEN-003 | Changed | `poc/F2-SEN-003/run3/` |
| F2-SEN-010 | Fixed | `poc/F2-SEN-010/run3/` |
| F2-SEN-011 | Changed | `poc/F2-SEN-011/run3/` |

## Commands run (all in the worktree after `source env/rv-sen.sh`)

- Baseline: `cargo test -p sentinel --bins` (50 passed).
- PoC groups, each after pasting the run 3 sources into `mod tests` of `crates/sentinel/src/service.rs` and reverting with `git checkout --` afterwards: `cargo test -p sentinel --bins <filter> -- --nocapture --test-threads=1` for `poc_f_sen_001`, `poc_f_sen_002`, `poc_f_sen_015`, `qa2_sen::`, `qa2_sen_010`, `rv_sen_run3`. Sources: `poc/F-SEN-001/run3/poc_service.run3.rs`, `poc/F-SEN-002/run3/poc_service.run3.rs`, `poc/F-SEN-015/run3/poc_service.run3.rs`, `poc/F2-SEN-001/run3/qa2_sen_all.run3.rs` (diff against the `fe9e84c` copy alongside), `poc/F2-SEN-010/run3/poc.run3.rs`, `poc/F2-SEN-001/run3/rv_sen_run3.rs` (new). Full outputs in `poc/F2-SEN-001/run3/`.
- `cargo build -p sentinel`, then the binary with the shipped sample (rpc and database pointed at loopback and scratch) against a local Anvil on port 31500 (`poc/F-SEN-010/run3/f010_run.sh`).
- `forge test --offline --match-path test/SentinelOracle.t.sol --match-test test_UnanimousApprove_FeeDistributedAndBondsReturned` in `contracts/` with out and cache dirs in the scratchpad. forge's dependency auto-install tried `git submodule update` for the three `contracts/lib` submodules and failed immediately because the paths exist; nothing was fetched and the test compiled and passed.
- USDC.e `approve` gas on a local Anvil (port 31501, chain id 100) loaded from the Foundry fork cache `~/.foundry/cache/rpc/xdai/48635154` written by the 2026-10-07 run (`poc/F-SEN-008/run3/f008_run.sh`, `f008_usdce.py`), on the latest hardfork, `prague` and `cancun`. No live RPC was contacted.
- GitHub API: none used; PR descriptions read from the squash-merge commit messages (`git log -1 <commit>`).
- Every Anvil this agent started was stopped by its script; `pgrep -a anvil` shows none. The worktree ends clean (`git status --short` empty).

## Notes for the head

- `F-SEN-008` needs routing before Aegis sentinels go live: in the default `Direct` mode every `approve` of the production fee token runs out of gas.
- Leads (not filed) are in the department report: the arbitration-timeout drop after #974 when `timeoutArbitration` reverts, the verdict-prune race after a restart warp, and the stale warp comment at `service.rs:481-483`.
