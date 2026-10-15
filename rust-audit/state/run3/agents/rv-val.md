# rv-val: run 3 re-validation log (validator rows)

| Field             | Value                                                    |
| ----------------- | -------------------------------------------------------- |
| Agent             | rv-val (auditor department)                              |
| Assignment        | 31 canonical rows, `scratch/rv-val.md`                   |
| Audit commit      | `361aaf3`                                                |
| Worktree          | the local run folder's `wt/rv-val` (clean at the end)    |
| Started, finished | 2026-10-08 16:56 UTC, about 18:05 UTC (about 70 minutes) |
| Rows not finished | none                                                     |

## Rows

Every row has a `## Run 3 re-validation (rv-val, 2026-10-08)` section appended to its canonical file and a line in `state/run3/revalidation/rv-val.md`.

| ID | Verdict | Run-3 evidence |
| --- | --- | --- |
| F-VAL-001 | Fixed (#1027) | `poc/F-VAL-001/run3/`, `poc/F2-VAL-001/run3/`, `poc/F-VAL-001-onchain/run3/` |
| F-VAL-002 | Changed, Low `known` | `poc/F-VAL-002/run3/`, `poc/F2-VAL-002/run3/` |
| F-VAL-003 | Still valid, Medium | `poc/F-VAL-003/run3/` |
| F-VAL-004 | Still valid (rollover); genesis settled | `poc/F2-VAL-063/run3/` |
| F-VAL-005 | Fixed (before run 2) | `poc/F-VAL-005/run3/` |
| F-VAL-030 | Moot (#1038, #1040) | `poc/F-VAL-030/run3/` |
| F-VAL-031 | Moot (#1040) | E2 |
| F-VAL-032 | Fixed (#1036) | `poc/F-VAL-032/run3/` |
| F-VAL-033 | Settled (A17) | E2 |
| F-VAL-034 | Still valid, Low | `poc/F-VAL-034/run3/` |
| F-VAL-035 | Changed, Informational | E2 |
| F-VAL-036 | Moot (#1038) | E2 |
| F-VAL-037 | Still valid, Informational | `poc/F-VAL-037/run3/` |
| F-VAL-038 | Moot (#1040) | E2 |
| F-VAL-039 | Moot (#1038) | E2, F-VAL-032 V4 |
| F-VAL-040 | Changed, Low | `poc/F-VAL-040/run3/` |
| F-VAL-060 | Changed, High conditional | `poc/F-VAL-060/run3/` |
| F-VAL-061 | Changed, Medium | `poc/F-VAL-061/run3/` |
| F-VAL-062 | Changed, Informational | `poc/F-VAL-061/run3/` |
| F-VAL-063 | Still valid, Medium | `poc/F-VAL-063/run3/` |
| F-VAL-064 | Still valid, Low | E2 |
| F-VAL-065 | Changed, Low | E2 |
| F-VAL-066 | Changed, Low | `poc/F-VAL-066/run3/` |
| F-VAL-067 | Fixed (#1019) | `poc/F-VAL-067/run3/` |
| F-VAL-068 | Superseded | E2 |
| F2-VAL-003 | Fixed (#1017) | `poc/F2-VAL-003/run3/` |
| F2-VAL-004 | Still valid, Low | `poc/F2-VAL-004/run3/` |
| F2-VAL-007 | Still valid, Low | `poc/F2-VAL-007/run3/` |
| F2-VAL-031 | Moot (#1039, #1040) | E2 |
| F2-VAL-034 | Settled (fresh start) | E2 |
| F2-VAL-064 | Still valid, Informational | `poc/F2-VAL-064/run3/` |

## Commands run

All builds and tests in the worktree after `source env/rv-val.sh`. PoC modules were wired with `#[cfg(test)] #[path = "<abs path under rust-audit/poc/>"] mod ...;` appended to the module named in each PoC header, and every tracked-file hunk was reverted with `git checkout -- <file>`; `git status --short` is empty at the end. No Anvil was started (`pgrep -a anvil` empty); Foundry tests ran in-process with `FOUNDRY_TEST=<scratchpad dir>`.

- `cargo test -p validator --bins --no-run` (warm cache, 22 s).
- F-VAL-001: `cargo test -p validator --bins frost::poc_f_val_001` (original adapted, 0 of 3), `frost::poc_f_val_001_run3` (8 of 8), mutation with the PoP check removed (original 3 of 3 pass again, v1 to v6 fail), `frost::poc_f2_val_001_run3` (0 of 1); `FOUNDRY_TEST=<scratch> forge test --match-path '*FVal001OnchainRun3*'` (7 of 7), mutation without the onchain PoP line (6 fail).
- F-VAL-002: `cargo test -p validator --bins poc_f_val_002_run3 poc_f2_val_002_run3` (3 of 3).
- Keygen state PoCs (F-VAL-003, F-VAL-060, F-VAL-067, F2-VAL-003, F2-VAL-004, F2-VAL-007, F2-VAL-063): `cargo test -p validator --bins state::keygen::` (13 of 13); F2-VAL-003 mutation without the warped `NewBlock` in core (3 fail).
- Sign and effect PoCs (F-VAL-030, F-VAL-032, F-VAL-034, F-VAL-040, F-VAL-061, F-VAL-062): `cargo test -p validator --bins -- poc_f_val_032_run3 poc_f_val_034_run3 poc_f_val_040_run3 poc_f_val_030_run3 poc_f_val_061_run3` (12 of 12); F-VAL-032 mutation restoring the drop arm (3 fail).
- F-VAL-067 contract half: `forge test --match-test test_KeyGenComplain_CompromisedByRespondedComplaints` (pass), mutation re-adding `accusations--` (fail).
- F-VAL-005: `cargo test -p validator --bins secrets::store` (12 of 12).
- F2-VAL-064: `cargo build -p validator --bin validator`, then the binary with `--config-file=<path>` (rejected, exit 1) and `--config-file <path>` (parsed).
- F-VAL-037: `cargo test -p validator --bins poc_f_val_037_run3` (1 of 1).
- F-VAL-063: `cargo test -p validator --bins poc_f_val_063_run3` (run-2 `F2-VAL-067` PoC, 1 of 1).
- F-VAL-066: `RV_VAL_DB_DIR=<scratch> cargo test -p validator --bins poc_f_val_066_run3` (2 of 2: same-input race lost 20 of 20; short-outage loss deterministic).
- F-VAL-030: the run-1 harness `poc/F-VAL-030-032-061/nonce_state.rs` no longer compiles (11 errors on removed types).
- Consult from the head (F-CORE-031, discarded nonce resumes): `cargo test -p validator --bins poc_consult` (2 of 2).
- Lookups: `git log --oneline fe9e84c..361aaf3 -- crates/...`, `git log -S '<symbol>'`, `git log --diff-filter=D`, `git show <commit>` for #1008, #1017, #1019, #1027, #1036, #1039, #1041. No GitHub API requests were needed (commit messages carry the PR descriptions).

## Process notes

- `poc/F-VAL-001/run3/poc_original_adapted.rs` had the run-1 text's em dashes replaced by commas (comments and panic messages only) and was re-run; its result file is from that run.
- Prettier insisted on a blank line between list items in F-VAL-040's appended section; `prettier --write` changed only the appended block (the diff against `HEAD` is a pure addition).
