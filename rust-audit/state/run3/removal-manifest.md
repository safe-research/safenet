# Run 3 removal manifest

| Field | Value |
| --- | --- |
| Written by | REC3 (auditor department), Phase 5 of Rust audit run 3, 2026-10-09 |
| Audit commit | `361aaf3` |
| Rules | the run's plan (a resolved finding loses its file and its PoC folder; `REPORT.md` keeps a one-line index), the run's cleanup set, the REC3 brief: a PoC folder stays if a surviving finding file, `REPORT.md` or the new ledger cites it; `state/run3/` stays; run-3 duplicate files stay this run |
| Ledger | [`report/RECONCILIATION.md`](../../report/RECONCILIATION.md): section 4 is the resolved index, section 3 the fold index |
| Who deletes | nobody here; the head turns this list into commands remedcu runs. Run them after the run-3 review commit, so the evidence stays in that commit |
| Total | 111 paths: 75 finding files, 11 run-1 and run-2 working paths, the stale map and `analysis/`, 21 PoC entries, 2 files no longer needed |

## 1. Paths to delete

Paths are relative to the repository root. A trailing `/` marks a folder.

### Fixed (8 finding files)

```text
rust-audit/findings/F-VAL-001.md
rust-audit/findings/F-VAL-005.md
rust-audit/findings/F-VAL-032.md
rust-audit/findings/F-VAL-067.md
rust-audit/findings/F2-VAL-003.md
rust-audit/findings/F-SEN-003.md
rust-audit/findings/F2-SEN-010.md
rust-audit/findings/F2-XC-001.md
```

### Moot (7 finding files)

```text
rust-audit/findings/F-VAL-030.md
rust-audit/findings/F-VAL-031.md
rust-audit/findings/F-VAL-036.md
rust-audit/findings/F-VAL-038.md
rust-audit/findings/F-VAL-039.md
rust-audit/findings/F2-VAL-031.md
rust-audit/findings/F-CORE-069.md
```

### Settled (3 finding files)

```text
rust-audit/findings/F-CORE-037.md
rust-audit/findings/F-VAL-033.md
rust-audit/findings/F2-VAL-034.md
```

### Superseded (3 finding files)

```text
rust-audit/findings/F-VAL-068.md
rust-audit/findings/F-SEN-016.md
rust-audit/findings/F-CORE-068.md
```

### Refuted (1 finding file)

```text
rust-audit/findings/F-SEN-013.md
```

### Union row (1 finding file)

```text
rust-audit/findings/F-XC-002.md
```

### Folded run-2 files (52 finding files)

The 51 files the run-2 reconciliation folded into a canonical row, plus `F2-VAL-004`, which run 3 folds into `F3-VAL-062` (ledger section 3.3).

```text
rust-audit/findings/F2-CORE-001.md
rust-audit/findings/F2-CORE-002.md
rust-audit/findings/F2-CORE-004.md
rust-audit/findings/F2-CORE-005.md
rust-audit/findings/F2-CORE-006.md
rust-audit/findings/F2-CORE-008.md
rust-audit/findings/F2-CORE-009.md
rust-audit/findings/F2-CORE-010.md
rust-audit/findings/F2-CORE-030.md
rust-audit/findings/F2-CORE-031.md
rust-audit/findings/F2-CORE-032.md
rust-audit/findings/F2-CORE-033.md
rust-audit/findings/F2-CORE-034.md
rust-audit/findings/F2-CORE-035.md
rust-audit/findings/F2-CORE-036.md
rust-audit/findings/F2-CORE-060.md
rust-audit/findings/F2-CORE-061.md
rust-audit/findings/F2-CORE-062.md
rust-audit/findings/F2-CORE-063.md
rust-audit/findings/F2-CORE-064.md
rust-audit/findings/F2-CORE-065.md
rust-audit/findings/F2-CORE-066.md
rust-audit/findings/F2-VAL-001.md
rust-audit/findings/F2-VAL-002.md
rust-audit/findings/F2-VAL-005.md
rust-audit/findings/F2-VAL-006.md
rust-audit/findings/F2-VAL-030.md
rust-audit/findings/F2-VAL-032.md
rust-audit/findings/F2-VAL-033.md
rust-audit/findings/F2-VAL-035.md
rust-audit/findings/F2-VAL-060.md
rust-audit/findings/F2-VAL-061.md
rust-audit/findings/F2-VAL-062.md
rust-audit/findings/F2-VAL-063.md
rust-audit/findings/F2-VAL-065.md
rust-audit/findings/F2-VAL-066.md
rust-audit/findings/F2-VAL-067.md
rust-audit/findings/F2-VAL-068.md
rust-audit/findings/F2-SEN-001.md
rust-audit/findings/F2-SEN-002.md
rust-audit/findings/F2-SEN-004.md
rust-audit/findings/F2-SEN-005.md
rust-audit/findings/F2-SEN-006.md
rust-audit/findings/F2-SEN-007.md
rust-audit/findings/F2-SEN-008.md
rust-audit/findings/F2-SEN-009.md
rust-audit/findings/F2-XC-003.md
rust-audit/findings/F2-XC-004.md
rust-audit/findings/F2-XC-006.md
rust-audit/findings/F2-XC-008.md
rust-audit/findings/F2-XC-009.md
rust-audit/findings/F2-VAL-004.md
```

### Run-1 and run-2 working files (11 paths)

Role briefs, STATE narratives, coverage logs, the paren-repair log, the raw logs (3.2 MB) and the whole run-2 state folder (936 KB), including its per-crate reconciliation parts, which this ledger replaces.

```text
rust-audit/state/STATE.md
rust-audit/state/baseline.md
rust-audit/state/coverage-logs.md
rust-audit/state/coverage.md
rust-audit/state/critic-brief.md
rust-audit/state/logs/
rust-audit/state/paren-repair-log.md
rust-audit/state/phase8-brief.md
rust-audit/state/qa-brief.md
rust-audit/state/reviewer-brief.md
rust-audit/state/run2/
```

### Stale map and analysis (2 paths)

Written at `82b3e0d`; run 3 uses `state/run3/map.md`.

```text
rust-audit/codebase-map.md
rust-audit/analysis/
```

### PoC entries with no surviving citer (21 paths)

No surviving finding file and not the new ledger cites them. `state/run3/` files that cite them keep the citation as a historical record (section 3).

```text
rust-audit/poc/ANSWERS-V-CORE-SEN.md
rust-audit/poc/F-CORE-069/
rust-audit/poc/F-SEN-003/
rust-audit/poc/F-VAL-001-onchain/
rust-audit/poc/F-VAL-032/
rust-audit/poc/F-VAL-033/
rust-audit/poc/F-VAL-067/
rust-audit/poc/F2-SEN-010/
rust-audit/poc/F2-VAL-001/
rust-audit/poc/F2-VAL-030/
rust-audit/poc/F2-VAL-031/
rust-audit/poc/F2-VAL-032/
rust-audit/poc/F2-VAL-033/
rust-audit/poc/F2-VAL-034/
rust-audit/poc/F2-VAL-061/
rust-audit/poc/F2-VAL-062/
rust-audit/poc/F2-VAL-065/
rust-audit/poc/F2-VAL-067/
rust-audit/poc/Q10-axum-rejections/
rust-audit/poc/Q2-utf8/
rust-audit/poc/V-XC-phase5/
```

### Files no longer needed (2 paths)

- `report/IN-FLIGHT.md`: no open PR touches the crates at `361aaf3` (Map3, `state/run3/map.md` section 5.3). One line for the report: the in-flight stacks the earlier runs assessed all closed; the batex stack was replaced by the merged 7702 executor stack (#987 to #1006), and #914 and #915's deadline work merged as #1014, #1015 and #1017, which resolved `F2-SEN-010` and turned `F-SEN-016` into a superseded row.
- `state/pr-review-threads.md`: its 13 threads and their status now are in ledger section 6.

```text
rust-audit/report/IN-FLIGHT.md
rust-audit/state/pr-review-threads.md
```

## 2. What stays, and why

- **PoC folders of removed findings that surviving files cite** (the citer rule wins over the run plan's default): `F-VAL-001` (cited by `F-VAL-002`), `F-VAL-005` (by `F3-VAL-100`), `F-VAL-005-066` (by `F-VAL-035`, `F-VAL-066`), `F-VAL-030` (by `F-VAL-061`), `F-VAL-030-032-061` (by `F-VAL-034`, `F-VAL-040`, `F-VAL-061`), `F-VAL-030-032-061-phase8` (by `F-VAL-061`), `F-XC-002` (by `F-CORE-036`, `F-VAL-062`), `F-CORE-068` (by `F-CORE-063`), `F2-VAL-003` (by `F3-VAL-100`), `F2-XC-001` (by `F-XC-003`, `F-XC-009`, `F2-XC-007`, `F3-XC-040`), `F2-VAL-004` (by `F3-VAL-062`), and 25 folded run-2 PoC folders that canonical rows cite (`F2-CORE-001`, `002`, `004`, `005`, `030`, `060` to `065`; `F2-SEN-001`, `002`, `004` to `008`; `F2-VAL-002`, `006`, `060`, `063`; `F2-XC-004`, `006`, `009`).
- **All 36 run-3 finding files**, the 7 duplicates included (independent evidence, ledger section 3.1).
- **`state/run3/`** (this run's evidence), `PROMPT.md`, `README.md` (DOC3 updates it), `report/KNOWN-WORK.md` (DOC3 refreshes it), `report/REPORT.md` (DOC3 rewrites it) and `report/RECONCILIATION.md` (rewritten by REC3).
- **The root PoC notes still cited**: `UNRESOLVED-DEPENDENCY-QUESTIONS.md`, `V-VAL-dependency-questions/`, `Q11-fee-estimator/`.

## 3. Link check

Scope: every Markdown link and every backticked `findings/`, `poc/`, `state/` or `report/` path in the Markdown files that stay (215 files: the 115 finding files, Markdown under the surviving PoC folders, `state/run3/` with the REC3 log, `PROMPT.md`, `README.md` and the new ledger). Not scanned: `REPORT.md` and `KNOWN-WORK.md`, which DOC3 rewrites and must not link removed files, and this manifest, which names removed paths on purpose. Bare finding IDs are not paths and do not break.

Result: 246 references would break or are already broken: 243 point at removed paths, 3 were already broken. None is in the new ledger, and none points at a removed finding file.

| Referencing files | Target | Count | Fix |
| --- | --- | --- | --- |
| PROMPT.md | state/STATE.md | 3 | no edit: the prompt describes where a run writes |
| PROMPT.md | state/coverage.md | 3 | no edit: the prompt describes where a run writes |
| PROMPT.md | state/run2 | 2 | no edit: the prompt describes where a run writes |
| PROMPT.md | codebase-map.md | 1 | the Markdown link at `PROMPT.md:8` breaks: drop the link and point at `state/run3/map.md` (the head edits `PROMPT.md`) |
| PROMPT.md | state/baseline.md | 1 | no edit: the prompt describes where a run writes |
| PROMPT.md | state/logs | 1 | no edit: the prompt describes where a run writes |
| PoC notes | state/baseline.md | 12 | none: historical PoC notes; covered by the same `README.md` line |
| PoC notes | state/STATE.md | 1 | none: historical PoC notes; covered by the same `README.md` line |
| PoC notes | state/coverage.md | 1 | none: historical PoC notes; covered by the same `README.md` line |
| PoC notes | state/logs | 1 | none: historical PoC notes; covered by the same `README.md` line |
| PoC notes | poc/F-VAL-033 | 1 | none: historical PoC notes; covered by the same `README.md` line |
| README.md | state/run2 | 5 | DOC3 rewrites the layout table and the run-2 section (its brief) |
| README.md | state/STATE.md | 2 | DOC3 rewrites the layout table and the run-2 section (its brief) |
| finding files | state/run2 | 80 | none in the files: historical trail sections; DOC3 adds one line to `README.md` saying these paths resolve at `<run-3 review commit>` |
| finding files | state/baseline.md | 64 | none in the files: historical trail sections; DOC3 adds one line to `README.md` saying these paths resolve at `<run-3 review commit>` |
| finding files | state/coverage-logs.md | 13 | none in the files: historical trail sections; DOC3 adds one line to `README.md` saying these paths resolve at `<run-3 review commit>` |
| finding files | report/IN-FLIGHT.md | 13 | none in the files: historical trail sections; DOC3 adds one line to `README.md` saying these paths resolve at `<run-3 review commit>` |
| finding files | analysis | 9 | none in the files: historical trail sections; DOC3 adds one line to `README.md` saying these paths resolve at `<run-3 review commit>` |
| finding files | state/logs | 7 | none in the files: historical trail sections; DOC3 adds one line to `README.md` saying these paths resolve at `<run-3 review commit>` |
| finding files | already broken | 2 | already broken before run 3; see the list below |
| finding files | state/reviewer-brief.md | 1 | none in the files: historical trail sections; DOC3 adds one line to `README.md` saying these paths resolve at `<run-3 review commit>` |
| finding files | codebase-map.md | 1 | none in the files: historical trail sections; DOC3 adds one line to `README.md` saying these paths resolve at `<run-3 review commit>` |
| state/run3 | poc/F2-SEN-010 | 5 | none: run-3 evidence records; they resolve at `<run-3 review commit>` |
| state/run3 | poc/F-VAL-032 | 4 | none: run-3 evidence records; they resolve at `<run-3 review commit>` |
| state/run3 | poc/F-VAL-001-onchain | 3 | none: run-3 evidence records; they resolve at `<run-3 review commit>` |
| state/run3 | poc/F-CORE-069 | 2 | none: run-3 evidence records; they resolve at `<run-3 review commit>` |
| state/run3 | poc/F2-VAL-001 | 2 | none: run-3 evidence records; they resolve at `<run-3 review commit>` |
| state/run3 | poc/F-VAL-067 | 2 | none: run-3 evidence records; they resolve at `<run-3 review commit>` |
| state/run3 | poc/F-SEN-003 | 1 | none: run-3 evidence records; they resolve at `<run-3 review commit>` |
| state/run3 | codebase-map.md | 1 | none: run-3 evidence records; they resolve at `<run-3 review commit>` |
| state/run3 | analysis | 1 | none: run-3 evidence records; they resolve at `<run-3 review commit>` |
| state/run3 | already broken | 1 | already broken before run 3; see the list below |

### 3.1 Every reference, grouped by target

Format: referencing file and line numbers. Targets are written without backticks so this manifest does not count itself.

- **analysis** (10): findings/F-CORE-001.md:54; findings/F-CORE-007.md:70; findings/F-VAL-003.md:53; findings/F-VAL-004.md:21; findings/F-XC-001.md:31; findings/F-XC-006.md:35; findings/F-XC-008.md:35; findings/F-XC-009.md:37; findings/F2-CORE-067.md:58; state/run3/agents/R7.md:14
- **codebase-map.md** (3): PROMPT.md:8; findings/F-VAL-004.md:36; state/run3/agents/R7.md:14
- **poc/F-CORE-069** (2): state/run3/agents/CA3-FIX.md:26; state/run3/revalidation/rv-core-b.md:16
- **poc/F-SEN-003** (1): state/run3/agents/CA3-FIX.md:27
- **poc/F-VAL-001-onchain** (3): state/run3/agents/CA3-FIX.md:18; state/run3/agents/rv-val.md:18; state/run3/revalidation/rv-val.md:7
- **poc/F-VAL-032** (4): state/run3/agents/CA3-FIX.md:19,25; state/run3/agents/rv-val.md:25; state/run3/revalidation/rv-val.md:14
- **poc/F-VAL-033** (1): poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md:609
- **poc/F-VAL-067** (2): state/run3/agents/rv-val.md:41; state/run3/revalidation/rv-val.md:30
- **poc/F2-SEN-010** (5): state/run3/agents/CA3-FIX.md:22; state/run3/agents/rv-sen.md:28,34; state/run3/revalidation/rv-sen.md:9,24
- **poc/F2-VAL-001** (2): state/run3/agents/rv-val.md:18; state/run3/revalidation/rv-val.md:7
- **report/IN-FLIGHT.md** (13): findings/F-CORE-060.md:254; findings/F-CORE-061.md:110; findings/F-CORE-062.md:127; findings/F-CORE-063.md:116; findings/F-CORE-064.md:111; findings/F-CORE-065.md:107; findings/F-CORE-066.md:112; findings/F-CORE-067.md:259; findings/F-SEN-006.md:141; findings/F-SEN-007.md:146; findings/F-VAL-063.md:132; findings/F-VAL-065.md:135; findings/F-XC-009.md:175
- **state/STATE.md** (6): PROMPT.md:59,82,113; README.md:22,44; poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md:5
- **state/baseline.md** (77): PROMPT.md:74; findings/F-CORE-001.md:106; findings/F-CORE-002.md:123; findings/F-CORE-003.md:94; findings/F-CORE-004.md:119; findings/F-CORE-005.md:88; findings/F-CORE-006.md:104; findings/F-CORE-007.md:86; findings/F-CORE-008.md:95; findings/F-CORE-009.md:96; findings/F-CORE-010.md:74; findings/F-CORE-011.md:34,68; findings/F-CORE-012.md:60,78; findings/F-CORE-030.md:87,97; findings/F-CORE-031.md:111; findings/F-CORE-032.md:82; findings/F-CORE-033.md:80; findings/F-CORE-034.md:94; findings/F-CORE-035.md:87; findings/F-CORE-036.md:39,73,82,90; findings/F-CORE-038.md:44,86,90; findings/F-CORE-039.md:29,91; findings/F-CORE-040.md:70,76; findings/F-CORE-060.md:98; findings/F-CORE-061.md:76,92; findings/F-CORE-062.md:107; findings/F-CORE-063.md:96; findings/F-CORE-064.md:93; findings/F-CORE-065.md:89; findings/F-CORE-066.md:94; findings/F-CORE-067.md:82,90; findings/F-SEN-001.md:117; findings/F-SEN-002.md:116; findings/F-SEN-004.md:106; findings/F-SEN-005.md:93; findings/F-SEN-006.md:96; findings/F-SEN-007.md:107; findings/F-SEN-008.md:87; findings/F-SEN-009.md:84; findings/F-SEN-010.md:79; findings/F-SEN-011.md:94; findings/F-SEN-012.md:83; findings/F-SEN-014.md:78; findings/F-SEN-015.md:86; findings/F-VAL-035.md:261; findings/F-VAL-040.md:56; findings/F-VAL-061.md:113; findings/F-VAL-062.md:89; findings/F-VAL-064.md:82; findings/F-XC-007.md:42; findings/F-XC-050.md:62; findings/F-XC-051.md:23,38; poc/F-CORE-001/README.md:3; poc/F-CORE-002/README.md:3; poc/F-CORE-060/README.md:3; poc/F-CORE-067/README.md:3; poc/F-SEN-001/README.md:3; poc/F-SEN-002/README.md:3; poc/F-SEN-015/README.md:3; poc/F-VAL-001/README.md:5; poc/F-VAL-004/README.md:5; poc/F-XC-002/README.md:3; poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md:15,306
- **state/coverage-logs.md** (13): findings/F-CORE-006.md:52; findings/F-CORE-040.md:64; findings/F-CORE-067.md:76; findings/F-SEN-015.md:82; findings/F-VAL-003.md:53; findings/F-VAL-040.md:60; findings/F-VAL-060.md:102,130; findings/F-VAL-063.md:90; findings/F-XC-050.md:66; findings/F-XC-051.md:38; findings/F2-CORE-011.md:74
- **state/coverage.md** (4): PROMPT.md:76,105,109; poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md:13
- **state/logs** (9): PROMPT.md:99; findings/F-XC-007.md:170,201; findings/F-XC-011.md:17,154,167,196; poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md:68
- **state/reviewer-brief.md** (1): findings/F-CORE-012.md:40
- **state/run2** (87): PROMPT.md:240,241; README.md:65,67,68,69; findings/F-CORE-001.md:295; findings/F-CORE-002.md:283; findings/F-CORE-003.md:114; findings/F-CORE-008.md:113; findings/F-CORE-009.md:114; findings/F-CORE-010.md:92; findings/F-CORE-011.md:86; findings/F-CORE-012.md:94; findings/F-CORE-030.md:113; findings/F-CORE-031.md:198; findings/F-CORE-032.md:102; findings/F-CORE-033.md:100; findings/F-CORE-035.md:105; findings/F-CORE-036.md:124; findings/F-CORE-038.md:106; findings/F-CORE-040.md:94; findings/F-CORE-060.md:258; findings/F-CORE-064.md:115; findings/F-CORE-065.md:111; findings/F-CORE-066.md:116; findings/F-CORE-067.md:271; findings/F-SEN-001.md:321; findings/F-SEN-002.md:322; findings/F-SEN-004.md:152; findings/F-SEN-005.md:185; findings/F-SEN-006.md:149; findings/F-SEN-007.md:150; findings/F-SEN-008.md:120; findings/F-SEN-009.md:125; findings/F-SEN-010.md:109; findings/F-SEN-011.md:131; findings/F-SEN-012.md:117; findings/F-SEN-015.md:294; findings/F-VAL-002.md:191; findings/F-VAL-003.md:160; findings/F-VAL-004.md:251; findings/F-VAL-034.md:332; findings/F-VAL-035.md:354; findings/F-VAL-037.md:256; findings/F-VAL-040.md:97; findings/F-VAL-060.md:188; findings/F-VAL-061.md:233; findings/F-VAL-062.md:237; findings/F-VAL-063.md:136; findings/F-VAL-064.md:144; findings/F-VAL-065.md:139; findings/F-VAL-066.md:200; findings/F-XC-001.md:176; findings/F-XC-003.md:180; findings/F-XC-004.md:129; findings/F-XC-006.md:134; findings/F-XC-007.md:219; findings/F-XC-008.md:197; findings/F-XC-009.md:183; findings/F-XC-011.md:212; findings/F-XC-050.md:149; findings/F-XC-051.md:166; findings/F2-CORE-003.md:67; findings/F2-CORE-007.md:63; findings/F2-CORE-011.md:13,38,80,92,108; findings/F2-CORE-067.md:65; findings/F2-SEN-003.md:163; findings/F2-SEN-011.md:71; findings/F2-VAL-007.md:78; findings/F2-VAL-064.md:52,76; findings/F2-XC-002.md:70,72,94; findings/F2-XC-005.md:108; findings/F2-XC-007.md:77; findings/F2-XC-050.md:83,125
- **already broken (not caused by the removals)** (3): findings/F2-CORE-011.md:74; findings/F2-VAL-064.md:66; state/run3/agents/rv-xc.md:22

Already broken, detail: findings/F2-CORE-011.md:74 writes `F-CORE-009/010/033/037` as a shorthand for four files (not a path, no fix); findings/F2-VAL-064.md:66 names a sentinel loopback TOML that the PoC folder never held (no fix needed, the run-3 output beside it carries the result); state/run3/agents/rv-xc.md:22 names a scratch stub outside the repository (no fix).
