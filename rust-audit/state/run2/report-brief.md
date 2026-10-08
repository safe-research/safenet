# Run 2 — brief for the final report (the last commit, REPORT.md alone)

Read `PROMPT.md` Sections 9 and 11, `report/RECONCILIATION.md` (the combined ledger — the single source for every number), `state/pr-review-threads.md`, `report/KNOWN-WORK.md`, `report/IN-FLIGHT.md`, `state/run2/coverage.md`, `state/run2/baseline.md`, and the finding files you cite.

## What this report is

A **rewrite** of `report/REPORT.md` for the combined result of two independent runs, written for engineers who will fix things: read the verdict, know what to do first, find the evidence in one click. It is committed **last and alone**, so a reviewer can diff it in isolation. Target 600–800 lines. **Compile; do not re-judge** — every severity, certainty and claim comes from the ledger and the finding files. Where the two runs disagree and the reconciliation could not settle it, show both positions; never average them.

## Structure

1. **Verdict** (half a page): scope and commit; that two independent runs (different models) were made and how far they agree — the agreement rate is itself a result; final counts; the Criticals in one table with one line each and what executed.
2. **What changed since the team last read this report**: engine removed from scope; A16/A17 adopted from the team's own review comments and what they moved; the pruning merge and what it fixed, partially fixed and introduced; findings the team accepted or assigned, so the list they act on is the list that remains.
3. **Fix these first** — ordered, with sequencing constraints and owners where the team assigned them.
4. **Do not ship these "fixes"** — the unsound remediations, both runs.
5. **All findings** — one table, canonical IDs, with a column showing which run(s) found it (`1`, `2`, `1+2`) and the team's disposition.
6. **Findings by severity** — Critical/High in a few lines each; Medium and below one line each. Every entry links to its file.
7. **Where the runs disagreed, and what the audit got wrong** — contradictions, run-1 findings run 2 missed, run-2 findings run 1 missed, refutations by execution, hallucinated claims caught.
8. **Open questions and still-blocked items** — team questions answered here, dependency questions remaining, `#914`/`#915`/batex status.
9. **Scope, method, safety, assumptions** — compact; A1–A17 with only the FALSE/changed ones explained; local-Anvil-only boundary and the live-RPC traps in the sample configs; pointers to `state/` and `poc/` rather than reproductions.

## Rules

No dates in prose or filenames (repo convention for this audit). No HTML comments (CI rejects them in PR text; keep the report clean too). Every relative link must resolve after the engine removal — check them. Prettier-clean (`proseWrap: never`). Keep exact numbers, selectors, addresses and `path:line` citations — those are the actionable parts. Write only `report/REPORT.md`; nothing else changes in this commit.
