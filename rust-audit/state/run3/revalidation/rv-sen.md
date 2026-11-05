# Run 3 re-validation: rv-sen (sentinel)

Audit commit `361aaf3`, worktree `wt/rv-sen`, 2026-10-08. 19 canonical rows. Each row's evidence and anchors are in the `## Run 3 re-validation (rv-sen, 2026-10-08)` section appended to its finding file.

| ID | Ledger severity | Verdict | Proposed severity | Certainty | Changed by | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| F-SEN-001 | High | Still valid | High (unchanged) | 99 | nothing in scope (#979 narrows the restart race only, by its own description) | E1 `poc/F-SEN-001/run3/output.txt`, `rv-sen-variants.txt` (variants a, b, c) |
| F-SEN-002 | High | Changed | High (unchanged) | 98 | #964 recovers the mirror case when the uncounted committer reveals later; #1014/#1015 remove the deadline-block undercount; primary path unchanged | E1 `poc/F-SEN-002/run3/output.txt`, `poc/F2-SEN-002/run3/output.txt` |
| F-SEN-003 | Low | Fixed | n/a | n/a | #1017 (`24eaf8d`), with #1015 | E1 `poc/F2-SEN-003/run3/output.txt` (route b passes), variants in `poc/F2-SEN-010/run3/`, `poc/F2-SEN-004/run3/` |
| F-SEN-004 | Medium | Changed | Medium (unchanged) | 78 | trigger moved by Aegis parameters: starvation needs more than 16 proposals per block; bond exposure (800 USDC.e each) is the operative branch | E1 `poc/F2-SEN-005/run3/output.txt`; E2 bond arithmetic |
| F-SEN-005 | Medium | Changed | Medium (unchanged) | 95 | #974 (`fd0d2a7`) fixes the dispute half; `WaitingForOutcome` still never expires | E1 `poc/F2-SEN-004/run3/output.txt` |
| F-SEN-006 | Low | Still valid | Low (unchanged) | 90 | nothing removed it; #1015 adds the optimistic-transition replay site | E1 `poc/F2-SEN-007/run3/output.txt` |
| F-SEN-007 | Low | Still valid | Low (unchanged) | 82 | nothing in scope | E1 `poc/F-SEN-010/run3/startup.txt`, `poc/F2-SEN-006/run3/output.txt` |
| F-SEN-008 | Low | Changed | High (proposed) | 90 | code unchanged; #1062 makes USDC.e (FiatToken proxy) the fee token: a zero-to-non-zero `approve` needs 55,437 gas against the 55,000 limit in the default `Direct` mode | E1 `poc/F-SEN-008/run3/output.txt` |
| F-SEN-009 | Low | Still valid | Low (unchanged) | 82 | nothing in scope (#1062 sample value and doc only) | E1 `poc/F2-SEN-006/run3/output.txt`, `poc/F2-SEN-008/run3/output.txt`, `poc/F-SEN-010/run3/startup.txt` |
| F-SEN-010 | Informational | Still valid | Informational (unchanged) | 85 | nothing in scope; basis now E1 | E1 `poc/F-SEN-010/run3/startup.txt` |
| F-SEN-011 | Low | Still valid | Low (unchanged) | 82 | nothing in scope; known (#973) | E1 `poc/F2-CORE-030/run3/rv-sen-f-sen-011.txt` |
| F-SEN-012 | Low | Still valid | Low (unchanged) | 90 | nothing in scope | E1 `poc/F2-SEN-008/run3/output.txt` |
| F-SEN-013 | Informational (refuted as filed) | Still valid (as a refutation; non-live) | Informational (unchanged) | 98 (refutation) | `alloy-sol-types` 1.6.0 unchanged | E2 `Cargo.lock` at both commits |
| F-SEN-014 | Informational | Changed | Informational (unchanged) | 88 | #964: with every committer revealed, all K `Finalize` calls revert | E1 `poc/F-SEN-014/run3/output.txt`, `forge-964-unanimous.txt` |
| F-SEN-015 | High | Fixed | n/a | n/a | #978 (`d5b24dc`), #979 (`be86ee3`) | E1 `poc/F-SEN-015/run3/handler-route.txt` (6 variants) |
| F-SEN-016 | n/a (unmerged bundle) | Superseded | n/a | n/a | D1 by `F2-SEN-010` (now Fixed), D3 by `F-SEN-006`; D2, D4 moot in the merged design (#1014, #1015, #1017) | E1 via those rows; E2 for D2, D4 |
| F2-SEN-003 | Medium | Changed | Medium (unchanged) | 90 | #1017 closes trigger (b) and routes `F-SEN-003`'s downtime scenario into this drop; trigger (a) unchanged | E1 `poc/F2-SEN-003/run3/output.txt` |
| F2-SEN-010 | High | Fixed | n/a | n/a | #1014 (`fef8cfd`), with #1015, #1017 | E1 `poc/F2-SEN-010/run3/output.txt`, `rv-sen-variants.txt` (5 variants) |
| F2-SEN-011 | Informational | Changed | Informational (unchanged) | 80 | #1017 removes the warp trigger; remaining route needs lost `Revealed` logs (A4); #974 adds a second metric site | E1 `poc/F2-SEN-011/run3/output.txt`; E2 doc and metric |

Counts: Still valid 8 (one of them the refuted F-SEN-013), Changed 7, Fixed 3, Superseded 1, Moot 0, Settled 0.
