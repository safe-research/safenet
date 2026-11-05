# rv-core-b: run 3 re-validation log (core transaction queue rows)

| Field | Value |
| --- | --- |
| Agent | rv-core-b (auditor department) |
| Assignment | 10 canonical rows, `scratch/rv-core-b.md` (F-CORE-060 to F-CORE-069) |
| Audit commit | `361aaf3` |
| Worktree | the local run folder's `wt/rv-core-b` (clean at the end, `git status --short` empty) |
| Time | about 16:50 to 17:45 UTC on 2026-10-08 (start approximate; the first build in the worktree ran shortly before 17:07 UTC) |
| Rows not finished | none |

## Rows

| ID | Status | Verdict |
| --- | --- | --- |
| F-CORE-060 | done | Still valid (High, 98) |
| F-CORE-061 | done | Still valid (Medium, 65) |
| F-CORE-062 | done | Still valid (Medium, 60) |
| F-CORE-063 | done | Still valid (Medium, 55) |
| F-CORE-064 | done | Still valid (Medium, 90) |
| F-CORE-065 | done | Still valid (Low, `known`; certainty 70 to 90 proposed) |
| F-CORE-066 | done | Still valid (Medium, 90) |
| F-CORE-067 | done | Still valid (Medium, 98) |
| F-CORE-068 | done | Changed (proposed Low, 90; fold into F-CORE-063) |
| F-CORE-069 | done | Moot |

## Commands run

All in the worktree after `source env/rv-core-b.sh`:

1. `cargo test -p safenet-core --lib tx::` (baseline, unmodified): 56 passed.
2. Background job `r3-rv-core-b-forge-executor`: `cd contracts && forge test --match-path test/Safenet7702Executor.t.sol -vv`: 10 passed. Forge first tried to auto-install the uninitialised submodules by `git clone` from GitHub; the clones failed because the directories exist and nothing was fetched. Recorded here because the run allows no network beyond the Cargo registry; use a worktree with initialised submodules (or skip forge) next time.
3. PoC snippets appended to the `mod tests` of `crates/core/src/tx/mod.rs`, `crates/core/src/tx/storage.rs` and `crates/validator/src/config.rs` with an asserting insertion script (`poc/F-CORE-060/run3/insert_tests.py`); applied hunks verified equal to the published sources.
4. `cargo test -p safenet-core --lib rv3_ --no-run`: compiles, no warnings.
5. `cargo test -p safenet-core --lib rv3_ -- --nocapture --test-threads=1`: 27 tests, 18 passed, 9 failed; the 9 failures are exactly the healthy-behaviour assertions of the run-1 PoCs and of the F-CORE-060 and F-CORE-067 variants (defect reproduced), every run-2 PoC passed (defect reproduced), the F-CORE-065 and F-CORE-068 tests passed, and the F-CORE-069 tests passed (no gap).
6. `cargo test -p validator --bins rv3_ -- --nocapture`: 1 passed (TOML `nan` and zeros accepted).
7. The merged design's own recovery tests, `cargo test -p safenet-core --lib` filtered to `recovers_an_unused_authorization`, `only_recovers_the_authorization`, `marks_a_cancellation_executed`, `authorization_uses_up_the_next_nonce` and `delegates_an_undelegated_account`: 6 passed.
8. Mutation check: `mutate_recovery.py apply` (forces `recover_authorization_gap` to return `Ok(false)`), `cargo test -p safenet-core --lib rv3_f_core_069 -- --test-threads=1`: both F-CORE-069 tests FAIL (next batch at nonce 2 behind the unused nonce 1); `mutate_recovery.py revert`, verified no mutation left.
9. Reverted the three edited files (`crates/core/src/tx/mod.rs`, `crates/core/src/tx/storage.rs`, `crates/validator/src/config.rs`) with the two-hyphen `git checkout` form; `git status --short` empty.
10. GitHub REST API, generic User-Agent, 3 requests: PR #996, PR #1023, review comment 4165406017 (PR #1004).
11. the style check on every appended section and on this file and the revalidation table: 0 errors. Prettier (repo config, `--stdin-filepath` under `rust-audit/findings/`): every appended section is unchanged by Prettier. Seven of the ten finding files already had Prettier differences in their earlier text before this run's append (local Prettier 3.7.4); not touched.

## Outputs

- Appended sections: `rust-audit/findings/F-CORE-060.md` to `F-CORE-069.md`.
- PoC folders: `rust-audit/poc/{F-CORE-060,F2-CORE-060,F2-CORE-061,F2-CORE-062,F2-CORE-063,F2-CORE-064,F2-CORE-065,F-CORE-065,F-CORE-067,F-CORE-068,F-CORE-069}/run3/` (sources, trimmed outputs, README).
- Table: `rust-audit/state/run3/revalidation/rv-core-b.md`.
