# QA3-SEN coverage log (run 3, discovery track)

Primary QA for the sentinel findings. Worktree `wt/rv-sen`, Anvil ports 31500 to 31509. Audit commit `361aaf3`. All source edits were in the worktree only and reverted; `git status --short` is clean of tracked changes.

## Findings handled

| Finding | Result | Method | Bite | Evidence |
| --- | --- | --- | --- | --- |
| F3-SEN-001 (High) | Reproduced | end-to-end: real `sentinel` binary, Direct mode, real USDC.e etched on local Anvil, Aegis oracle params | bites; remediation (raise gas) sound | `poc/F3-SEN-001/` |
| F3-SEN-002 (Medium) | Reproduced | FSM, real `SentinelTransition::apply_transition` | bites; remediation option 1 sound | `poc/F3-SEN-002/` |
| F3-SEN-003 (Medium) | Reproduced | FSM, real transition | bites; remediation option 1 sound | `poc/F3-SEN-003/` |
| F3-SEN-004 (Medium) | Reproduced | real core `StateMachine` warp replay over in-memory sqlite | bites (drop line); sound fix is option 1/2/3, not the raw not-drop | `poc/F3-SEN-004/` |
| F3-XC-006 (Info) | Reproduced | existing passing tests (contract + sentinel flow) plus end-to-end sighting; no new test (no duplication) | n/a | `poc/F3-XC-006/` |

F3-XC-001 and F3-XC-002 are duplicates of F3-SEN-002 and F3-SEN-003; not separately reproduced (the canonical findings cover them).

## Commands

- Extract USDC.e: `python3 -I poc/F3-SEN-001/extract_usdc.py` (cache `~/.foundry/cache/rpc/xdai/48635154`).
- Token probe: `bash poc/F3-SEN-001/probe_etch.sh` (approve 55000 fails, 80000 succeeds gasUsed 55437).
- End-to-end: `bash poc/F3-SEN-001/e2e.sh buggy` and `... fixed` (after editing the encoder gas and `cargo build -p sentinel`).
- FSM 002/003: paste `fsm_poc_module.rs` into `crates/sentinel/src/service.rs` `mod tests`; `cargo test -p sentinel --bins qa3_sen_poc -- --nocapture`.
- StateMachine 004: paste `statemachine_poc_module.rs`; `cargo test -p sentinel --bins qa3_sen_004 -- --nocapture`.
- F3-XC-006: `cd contracts && forge test --match-test test_UnanimousApprove_FeeDistributedAndBondsReturned --match-contract SentinelOracleTest`; `cargo test -p sentinel --bins flow_unanimous_approve_finalizes_via_early_reveal_and_claims`.

## Notes and hygiene

- Anvil: only port 31500 (chain 100), killed at the end; `pgrep anvil` on my ports is empty.
- The `e2e.sh` cleanup trap did not reliably kill the `sentinel`/`engine_stub` children under the tool sandbox: three orphan processes survived the two runs and regrew `out-*/sentinel.log` to about 5.5 MB each with connection-refused spam. I killed the three PIDs I started and re-trimmed the logs (`trim_logs.py`); each `out-*` folder is now about 24 to 28 KB. A later agent reusing `e2e.sh` should `pgrep -af "target/debug/sentinel"` after the run and kill leftovers.
- `run-bg` tripped the sandbox (exit 144) when the detached job sourced the env and ran the harness; I ran the harness in the foreground instead.
- No memory writes (parallel instances rule); generic lessons are in the report.

## Not attempted

- A live two-sentinel integration run with controlled engine latency for F3-SEN-002/003 (the honest trigger): left to the second reproducer QA3-2B. The FSM reproduction executes the mechanism deterministically; the adversarial variant needs no latency gap.
- F3-SEN-004 end-to-end with a file-backed database restart: the real-`StateMachine` warp test exercises the same replay ordering and rollback through production code, which is the crux; a full binary restart is the residual gap.

## Time

About 3.5 hours of wall time (build warm from the cache; the end-to-end harness dominated).
