# CA3-FIX: fix verification log (run 3, Phase 4)

| Field | Value |
| --- | --- |
| Agent | CA3-FIX (confidence-assigner department), parallel with PC3 and the QA agents |
| Audit commit | `361aaf3` |
| Worktree | `<run folder>/wt/rv-xc` (clean at the end: `git status --short` empty); target dir `<run folder>/target-rv-xc`; Anvil only inside `integ` network namespaces |
| Time | 2026-10-08, about 18:50Z to 20:20Z |
| Rows | 11 of 11 done: 7 Fixed confirmed (3 with a condition or lead), 3 Moot confirmed, 1 Not fixed (F-SEN-015) |
| Output | 11 appended `## Fix verification (CA3-FIX, 2026-10-09)` sections, `state/run3/phase4/CA3-FIX.md`, `poc/<id>/ca3-fix/` for 11 folders, this log, report `<run folder>/reports/CA3-FIX.md` |

`<run folder>` is the local run folder. Every build and test ran after `source <run folder>/env/rv-xc.sh`, offline. Test modules were wired with `#[cfg(test)] #[path = <abs>] mod ..;` (`<run folder>/scratch/CA3-FIX/wire.py`) or pasted into `mod tests` (`paste_tests.py`), and mutations applied with `mutate.py` (copies in `poc/F-VAL-001/ca3-fix/` and `poc/F-SEN-015/ca3-fix/`); every edit was asserted and reverted with `git checkout -- <file>`.

## Rows

| ID | Result | What was run | Output |
| --- | --- | --- | --- |
| F-VAL-001 | Fixed confirmed | rv-val's 3 Rust PoCs and onchain harness re-run; 6 new Rust variants (`ca3fix_frost.rs`, `ca3fix_state.rs`), 3 onchain (`FVal001OnchainCa3Fix.t.sol`); Rust mutation (PoP removed), onchain mutations (PoP line, PoK line) | `poc/F-VAL-001/ca3-fix/`, `poc/F-VAL-001-onchain/ca3-fix/` |
| F-VAL-032 | Fixed confirmed | rv-val `foreign_sign.rs`; 4 new variants (`ca3fix_sign.rs`); drop-arm mutation | `poc/F-VAL-032/ca3-fix/` |
| F-VAL-005 | Fixed confirmed (mechanism), lead L2 | `secrets::store` tests; end-to-end reorg script built from the repo's reorg-nonce test with epoch-1 assertions (`integ`); all-online reorg script, 3 runs | `poc/F-VAL-005/ca3-fix/` |
| F-SEN-015 | Not fixed (restart variant) | rv-sen handler-route tests and original PoC; live-reorg variant with control; race emulation (two tests, 2,100 replays); end-to-end on the real sentinel binary: 2 + 12 + 8 + 6 restarts (`ca3fix_f_sen_015_e2e.py`, `integ`); store-bypass mutation | `poc/F-SEN-015/ca3-fix/` |
| F2-SEN-010 | Fixed confirmed | rv-sen `qa2_sen_010` and `f2_sen_010_run3`; 2 new page geometries; stopgap and warp-tick mutations | `poc/F2-SEN-010/ca3-fix/` |
| F2-XC-001 | Fixed confirmed, residual L3 | rv-xc `run.sh` re-run on rebuilt binaries; echo fuzz in both binaries (319,360 documents); `ca3fix_run.sh` (7 binary cases); `set_input` mutation | `poc/F2-XC-001/ca3-fix/` |
| F-VAL-030 | Moot confirmed | rv-val `lost_generate_nonces.rs`; handler-level replay variant (`ca3fix_effect.rs`); nonce-overwrite mutation | `poc/F-VAL-030/ca3-fix/` |
| F-VAL-039 | Moot confirmed | rv-val V4; W1 and W4 from `ca3fix_sign.rs`; `grep` for `Sign.sequence` reads | `poc/F-VAL-032/ca3-fix/` |
| F-CORE-069 | Moot confirmed | rv-core-b `tx_mod_tests_run3.rs`; 2 new variants (`ca3fix_tx.rs`); `mutate_recovery.py` | `poc/F-CORE-069/ca3-fix/` |
| F-SEN-003 | Fixed confirmed (mechanism) | rv-sen route b and `f_sen_003_run3`; 2 new variants (three cases); warp-tick mutation | `poc/F-SEN-003/ca3-fix/` |
| F2-VAL-003 | Fixed confirmed (mechanism), F3-VAL-062 must stay | rv-val `warp_boundary.rs`; x1 and x2 (`ca3fix_warp.rs`); warp-tick mutation | `poc/F2-VAL-003/ca3-fix/` |

## Commands (representative)

- `cargo test -p validator --bins --locked --offline -- <filters> --nocapture --test-threads=1` (jobs `r3-CA3FIX-val-existing`, `-val-new`, `-mut-val`).
- `CA3FIX_DB_DIR=<run folder>/scratch/CA3-FIX/db cargo test -p sentinel --bins --locked --offline -- <filters>` (jobs `r3-CA3FIX-sen`, `-batch2`, `-mut-sen`, `-sen003b`).
- `cargo test -p safenet-core --lib --locked --offline -- rv3_f_core_069 ca3fix_f_core_069 config::tests`.
- `FOUNDRY_OFFLINE=true FOUNDRY_TEST=<run folder>/scratch/CA3-FIX/ftest-fval001 forge test --offline --match-path '*FVal001Onchain*'` in `contracts/`, clean and with `mutate_line.py` on `FROSTCoordinator.sol:374` and `:375`.
- `TIMEOUT=120 integ bash <run folder>/scratch/CA3-FIX/ca3fix_f_val_005_e2e.sh`; `integ bash ca3fix_keygen_reorg_online.sh <wt> 3`.
- `integ python3 -I <run folder>/scratch/CA3-FIX/e2e/ca3fix_f_sen_015_e2e.py <wt> <out> <n> down|flip 4|3`.
- `holdtarget bash rust-audit/poc/F2-XC-001/run3/run.sh <target>/debug <scratch>` and `ca3-fix/ca3fix_run.sh`.

## Not done

- No restart variant was driven end to end for F-VAL-001 or F-VAL-032 (the binding is checked in pure code paths the restart replays identically).
- The F-SEN-015 window was measured at one commit latency (two blocks after the proposal) and `max_reorg_depth` 5; wider latencies widen it by arithmetic, not by a run.
- Lead L2 was not reproduced with every validator online (0 of 3); its re-inclusion ordering on a real Gnosis reorg is unmeasured.

## Leftovers

`<run folder>/scratch/CA3-FIX/` (scripts, end-to-end output folders `e2e/out-*`, `online/`, `db/`, about 2.3 MB) and the worktree's ignored `*_logs.txt` from the integration runs. Nothing else was created outside `rust-audit/`.
