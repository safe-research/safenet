# Run 2 — brief for the Reconciliation agent (after Phase 4 of run 2)

Repository root: `/home/shebin.guest/safe/safenet`. Audited commit: `3ec8bc5`. You are the **first agent permitted to read both runs**. Read `PROMPT.md` Sections 2, 8 and 11 first.

## Inputs

- **Run 2:** `findings/F2-*.md` (with their Critic and QA sections), `state/run2/coverage.md`, `state/run2/STATE.md`.
- **Run 1:** `findings/F-*.md` (the surviving run-1 set — `crates/sentinel-engine` findings were removed with the engine's scope), `report/KNOWN-WORK.md`, `report/IN-FLIGHT.md`, `state/pr-review-threads.md` (the team's dispositions: accepted, assigned, fix claimed, questions).
- Run 1 audited `2893917`; run 2 audits `3ec8bc5`, which additionally carries the sentinel verdict-aggregation/meta-tx changes and the validator scheduled-secret-pruning series. A run-1 finding may therefore be **fixed, changed or newly exposed** by code that run 2 saw and run 1 did not; say so explicitly rather than treating every difference as disagreement.

## Task — produce `report/RECONCILIATION.md`

1. **Map every run-2 finding** to run 1, one row each: `F2-id → F-id | CONFIRMS / CONTRADICTS / EXTENDS / NEW`. _Confirms_: same defect, compatible severity and mechanism (note any certainty or severity gap and which run's number the final report should carry, with the reason — the run with executed evidence usually wins). _Contradicts_: the two runs disagree on mechanism, trigger or existence — record both positions with citations and, where a five-minute check settles it, settle it. _Extends_: same root cause, materially wider or narrower. _New_: no run-1 counterpart (double-check by grepping run-1 titles and claims for the same file:line before calling it new).
2. **List run-1 findings run 2 did not rediscover.** For each: still valid on `3ec8bc5` (a miss by an independent reviewer — say so plainly), fixed by the merged code (cite the fix), out of scope under A16/A17 (say which), or superseded. A miss is information about run 2's coverage, not a reason to drop the finding.
3. **Apply A16 and A17 to run 1's surviving findings** and record the resulting status changes (e.g. genesis-only liveness → Informational/`known`; operator-restore triggers → out of scope), each with a one-line justification. Do not edit run-1 finding files' existing text; append a `## Reconciliation (run 2)` section to each affected finding, run-1 or run-2, stating its final combined status.
4. **Fold in the team's dispositions** from `state/pr-review-threads.md`: accepted, assigned, fix-claimed. Where run 2 has evidence bearing on an open team question (`F-CORE-033`'s deadline/fan-out question; `F-CORE-067`'s reverting duplicate; `F-VAL-005` "fixed by the prune refactor"), answer it in the reconciliation with citations.
5. **Produce the combined ledger** the report is built from: every finding in either run, its canonical ID (prefer the run-1 ID where both exist, so the team's existing links keep working), final severity, final certainty, basis class, status, and a one-line trail ("run 1 E1 96%; run 2 confirms 90%; team: assigned"). Give the final counts by severity and by status.

## Rules

Change no verdict or number inside an existing section — append only. Cite `path:line` at `3ec8bc5` for anything you decide. Write only under `rust-audit/`; never modify tracked files outside it; never commit, branch, stash or push. Return at most ten lines: counts per category, the list of run-1 findings run 2 missed, and any contradiction you could not settle.
