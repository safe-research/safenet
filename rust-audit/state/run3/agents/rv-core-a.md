# rv-core-a: run 3 re-validation log (core indexing and runtime rows)

| Field        | Value                                     |
| ------------ | ----------------------------------------- |
| Agent        | rv-core-a (auditor department)            |
| Assignment   | 27 canonical rows, `scratch/rv-core-a.md` |
| Audit commit | `361aaf3`                                 |
| Worktree     | the local run folder's `wt/rv-core-a`     |
| Started      | 2026-10-08 16:56 UTC                      |
| Finished     | 2026-10-08 17:45 UTC (about 50 minutes)   |

## Rows

| ID          | Status                                         | Verdict     |
| ----------- | ---------------------------------------------- | ----------- |
| F-CORE-001  | done, PoC re-run and real-binary restart       | Still valid |
| F-CORE-002  | done, PoC re-run plus variant                  | Still valid |
| F-CORE-003  | done, new PoC                                  | Still valid |
| F-CORE-004  | done, PoC re-run plus variant                  | Still valid |
| F-CORE-005  | done, new PoC                                  | Still valid |
| F-CORE-006  | done, trace plus decode PoC re-run             | Still valid |
| F-CORE-007  | done, trace                                    | Still valid |
| F-CORE-008  | done, trace                                    | Still valid |
| F-CORE-009  | done, new PoC (cases 1, 3)                     | Still valid |
| F-CORE-010  | done, trace                                    | Still valid |
| F-CORE-011  | done, trace                                    | Still valid |
| F-CORE-012  | done, new PoC                                  | Still valid |
| F-CORE-030  | done, run-2 sketch executed on the real binary | Still valid |
| F-CORE-031  | done, two PoCs re-run                          | Still valid |
| F-CORE-032  | done, existing test run                        | Still valid |
| F-CORE-033  | done, trace                                    | Still valid |
| F-CORE-034  | done, trace                                    | Changed     |
| F-CORE-035  | done, trace                                    | Changed     |
| F-CORE-036  | done, redaction PoC executed plus variants     | Still valid |
| F-CORE-037  | done                                           | Settled     |
| F-CORE-038  | done, tests and Python check                   | Still valid |
| F-CORE-039  | done, trace                                    | Still valid |
| F-CORE-040  | done, trace                                    | Still valid |
| F2-CORE-003 | done, new PoC                                  | Still valid |
| F2-CORE-007 | done, new PoC                                  | Still valid |
| F2-CORE-011 | done, PoC re-run                               | Still valid |
| F2-CORE-067 | done, trace                                    | Changed     |

## Commands run

All in the worktree with `env/rv-core-a.sh` sourced; PoC sources injected into test modules with a small script and reverted with `git checkout` after each batch; `git status --short` clean at the end.

- Batch A (events): `cargo test -p safenet-core --lib -- --nocapture --test-threads=1 index::events::tests::poc_f_core_002 index::events::tests::qa_f2_core_002 index::events::tests::rv3_` (10 tests: 8 pass, 2 fail by design).
- Batch B and C (index, blocks, state, storage, effects, kdf): `cargo test -p safenet-core --lib -- --nocapture --test-threads=1 poc_f_core_001 qa_f2_core_001 qa_f2_core_011 qa_f2_core_030 qa_f2_core_005 rv3_ revalidate_invalidates_a_block_missing_from_rpc supports_no_reorg_protection safe_block_without_reorg_protection_is_the_last_indexed_block skips_panicked_tasks_without_losing_successful_resumes kdf::tests` (25 tests: 22 pass, 3 fail by design).
- Batch D: `cargo test -p sentinel --bins -- --nocapture --test-threads=1 poc_f_sen_001` (1 pass, 1 fails by design) and `cargo test -p validator --bins -- --nocapture --test-threads=1 qa_xc_002 qa2_cov_7_6` (5 pass; first attempt failed to compile on the run-1 PoC's nonexistent `round1::SecretPackage::secret_share()`, fixed in the adapted copy).
- Integration (private network namespace): `integ bash rust-audit/poc/F2-VAL-066/run3/rv-core-a-deep_reorg_exit_status.sh OUTDIR` and `integ bash rust-audit/poc/F-CORE-001/run3/rv-core-a-deep_reorg_restart.sh OUTDIR` (both via `run-bg`, jobs `r3-rv-core-a-deep-reorg-exit` and `r3-rv-core-a-deep-reorg-restart`).
- `python3 -I rust-audit/poc/F-CORE-038/run3/rv-core-a-hkdf_variants.py` and `python3 -I rust-audit/poc/F-CORE-038/hkdf_reference_vector.py`.
- Reads: `git log` and `git diff fe9e84c 361aaf3` for every cited file; `git log -S "exponential backoff"` for the handbook sentence (#1062). No GitHub API requests were needed.
- Formatting: prettier 3.9.6 (the CI version, from the npx cache) on the 27 finding files after appending; the style check on every file written.

## Rows not finished

None.
