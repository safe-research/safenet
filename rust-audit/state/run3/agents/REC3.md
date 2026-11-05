# REC3 coverage log (run 3, Phase 5 reconciliation)

| Field | Value |
| --- | --- |
| Agent | REC3, auditor department |
| Brief | the local run folder's `briefs/rec3.md`, plus the head's Phase 4 additions (PC3, CA3, CA3-FIX, LT3, OFF3 boundary note) |
| Audit commit | `361aaf3` |
| Date | 2026-10-09 |
| Outputs | `report/RECONCILIATION.md` (full rewrite), `state/run3/removal-manifest.md`, a `## Reconciliation (run 3)` section in each of the 36 `findings/F3-*.md`, a `## Run 3 counterpart` note in 21 earlier finding files, this log, the department report |

## Inputs read

| Input | Use |
| --- | --- |
| Previous `report/RECONCILIATION.md` (all 405 lines) | the 103 earlier rows, fold index, A16 and A17 changes, dispositions, unsettled items |
| The PR #905 thread record, `report/KNOWN-WORK.md` (sections 1 and 2), the in-flight assessment (head; both records leave with the manifest) | the 13 threads, issue mapping, in-flight status |
| `state/run3/revalidation/*.md` (5 files) | the verdict, proposed band and certainty of every earlier row |
| `state/run3/phase4/PC3.md`, `CA3.md`, `CA3-FIX.md` and the PC3, CA3 department reports | final run-3 values, cross-run bands, Not-fixed and conditional deletions, the researcher facts |
| Every `findings/F3-*.md`: header, Claim, Considered and rejected, Critic verdict, Lead triage, Practicality, Confidence | the mapping of each run-3 file |
| Earlier files read for the mapping: `F-CORE-003`, `F-CORE-033`, `F-CORE-034`, `F-CORE-060`, `F-CORE-066`, `F-CORE-068`, `F-SEN-008`, `F-SEN-014`, `F-VAL-063`, `F-XC-050`, `F-XC-051`, `F2-SEN-003`, `F2-VAL-003`, `F2-VAL-004`, `F2-XC-050` (claim and run-3 sections), headers of `F-SEN-002`, `F-SEN-015`, `F-VAL-060` | canonical choice and band settlement |
| `state/run3/settled.md`, `STATE.md`, `coverage.md` (sections 3 to 7) | known items, roster, observation dispositions |
| Reviewer logs `state/run3/agents/R1.md` to `R8.md`, `L-crypto.md`, `L-rust.md` (checklists, rejected hypotheses, observations), `LT3.md`, `C3-VAL.md` and `C3-XC.md` (relevant rows) | whether a run-3 reviewer examined an earlier defect (ledger section 5.1, every line cited) |
| `PROMPT.md` sections 3, 8, 11, 12; `README.md`; the maintainers' project notes; the team's decisions | scale, rules, decisions in force |

## Checks run

| Check | Command or method | Result |
| --- | --- | --- |
| Pre-existence of the NEW rows at `fe9e84c` | the `F3` files' own `git show fe9e84c` citations; `git show fe9e84c:crates/validator/src/state/keygen.rs` for `F3-VAL-100` (drop arm at line 263, epoch-1 start on the final confirmation at line 499); `git show fe9e84c:Cargo.lock` and `361aaf3:Cargo.lock` for `alloy-rpc-client` (2.0.5 at both) | 7 of 9 NEW rows were present at `fe9e84c` (ledger section 5.2) |
| Stale warp comment | `git show 361aaf3:crates/sentinel/src/service.rs`, lines 476 to 508; `git show 361aaf3:crates/core/src/state/mod.rs`, lines 226 to 245 | the comment at `:481-483` predates #1017 (`24eaf8d`), which applies `NewBlock` for every warped block |
| `F3-XC-004` Basis 4 class | read its Basis table | E1 (R8's run): userinfo not logged, path and query tokens logged |
| Ledger arithmetic | the ledger is generated from one row list; counts asserted in the generator (90 rows; High 4, Medium 26, Low 36, Informational 24; Confirmed 77; E1 73) | consistent; the misses list was checked to equal the 80 live earlier rows minus the 20 with a counterpart |
| PoC citers | a scan of the 115 surviving finding files for each `poc/<entry>` path | 21 entries uncited (all removed rows or old root notes); `poc/F2-VAL-004/` kept through the `F3-VAL-062` section |
| Link check | a scan of every Markdown link and backticked `findings/`, `poc/`, `state/`, `report/` path in the 215 Markdown files that stay | 246 references: 243 to removed paths (historical trail citations), 3 already broken; the ledger's 282 links and the manifest's link all resolve |
| Prettier 3.9.6 (`explorer/node_modules/.bin/prettier --check`) | the ledger, the manifest, this log; the 57 touched finding files compared with and without my section | ledger, manifest and log clean; no new failure in the 57 files; 10 `F3` files fail for older sections (formatting pass) |
| Style | the style check on the ledger, the manifest, this log and the appended sections extracted into one file | 0 errors, 0 warnings; no em dash |

## Decisions taken here

- Canonical: earlier IDs stay for 18 run-3 counterparts; `F3-VAL-062` becomes canonical and `F2-VAL-004` folds into it (#1017 made the trigger a deterministic restart path that only the run-3 file executed); `F3-XC-040` is canonical because `F2-XC-001` is Fixed; `F-SEN-008` stays canonical over `F3-SEN-001` (its claim named the proxied fee token; code unchanged).
- Bands outside CA3's explicit list: `F-CORE-066` to Informational and `F-VAL-063` to Low (the counterpart's CA3 band covers the leg that carried the band); `F-CORE-060` kept High and listed as unsettled (the counterpart covers only the accepted arm); `F-XC-050` kept Medium and listed as unsettled (precondition false on Aegis, not re-weighed by PC3).
- Re-validator notes: `F-CORE-068` folds into `F-CORE-063`; `F2-XC-050` stays Low 92 with the #1017 widening; `F-XC-051` stays Informational hardening; the stale comment is a note.
- `known` added in the ledger for `F-CORE-040` (cancel safety of `next_input`, `settled.md` section 2), `F-CORE-031`, `F-SEN-001`, `F-SEN-011`, `F2-SEN-003` (issues #961, #973 and the #1017 follow-up), as the run-3 files tag them.

## Incidents

- One scratch command included an `rm -rf` of my own scratch folder; the approval hook blocked it and I did not retry it. Nothing was deleted. A fresh scratch folder was used instead.
- No network access, no RPC, no GitHub reads; thread status is from the local record.

## Not covered

- I did not re-run any PoC; every number comes from the agents' executed evidence and their sections.
- `REPORT.md` and `KNOWN-WORK.md` are DOC3's; they were not link-checked.
- The pending offline evidence for `F3-SEN-060` (OFF3) was not read; the ledger marks it pending.
