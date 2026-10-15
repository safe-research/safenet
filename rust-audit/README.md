# Rust Audit Kit

Everything for the AI-driven review of the Rust services lives in this folder. Nothing here is wired into CI or the build; it is documentation plus the working files the audit run produces.

| File or folder | Purpose |
| --- | --- |
| `PROMPT.md` | The multi-agent prompt. Section 3 is the assumptions checklist the team completes before running; Section 0 is the message that starts the run; Sections 11 and 12 are the run-2 and run-3 addenda. |
| `findings/` | One file per live finding, with the reviewer, Critic, QA and later sections appended in order: `F-*` from run 1, `F2-*` from run 2, `F3-*` from run 3. Resolved rows leave the folder (`report/REPORT.md` Section 7). |
| `poc/` | Proof-of-concept sources and their outputs, one folder per finding. Run-3 re-runs are in `poc/<id>/run3/`. |
| `state/run3/` | Run 3's state, the only state folder left (see "Run 3" below). |
| `report/REPORT.md` | The final report for the team. |
| `report/RECONCILIATION.md` | The ledger: one canonical row per defect across the three runs, and the single source for every number in `REPORT.md`. |
| `report/KNOWN-WORK.md` | The live findings mapped onto the team's issues, TODOs and known items. |

Run 3 removed the earlier runs' working files: the run-1 codebase map and per-crate analyses, the top-level `state/` files and logs, `state/run2/`, the in-flight assessment and the PR thread record (`state/run3/removal-manifest.md` lists every path). Finding files still cite some of them in their historical sections. Those paths resolve at `<run-3 review commit>`.

## Which mode to run

Run the prompt in a normal interactive Claude Code session on Claude Fable 5.1 at `/effort max`. This is enough; the "ultracode" setting (xhigh effort plus automatic workflow planning) and the Workflow tool are not required.

Reasons:

- The run is gated by human usage checks between phases. A foreground Manager with background subagents stops cleanly at a gate; a background workflow keeps spawning agents until it is killed.
- Resilience comes from files, not from the harness: every agent writes its findings and logs to disk as it goes, and the run's `STATE.md` is re-read after any interruption or compaction. A stopped workflow can only be resumed inside the same session, so it adds little here.
- About ten to fifteen agents across five phases is well within what the Agent tool handles directly (the default concurrency limit is 20 subagents).

If the team prefers workflows anyway, say "use a workflow" in the start message; Section 10 of the prompt tells the Manager how to map phases onto workflows. Effort levels in Claude Code are `low`, `medium`, `high` (default), `xhigh`, and `max`, set with `/effort`, the `--effort` flag, or `effortLevel` in settings. Subagents inherit the session model; if you define custom agents under `.claude/agents/`, pin `effort: max` in their frontmatter. Fable 5.1 has a 1M-token context window with no extra setting.

## Machine requirements

The machine used to write this kit had no Rust toolchain, no Foundry, and under 4 GB of RAM, so its first codebase analysis was read-only. The audit run itself needs:

- Rust stable with `cargo`, plus `cargo-audit` (installed by the Recon agent if missing).
- Foundry 1.5.1 (`just foundryup`) for `anvil`, `forge`, `cast`; `just`; `jq`.
- At least 8 GB RAM and 15 GB free disk for `target/` (the workspace builds `alloy`).
- Network access to the Cargo registry only. No live RPC endpoints are used.

Optional: a checkout of `sentinel-test-vectors` next to the repo, if assumption A8 allows QA to run the engine corpus.

## Running

1. Complete the checklist in `PROMPT.md` Section 3. Tick every box or write `FALSE:` with a note. The Manager will not launch reviewers while a box is unticked.
2. Start Claude Code in the repository root. Select Claude Fable 5.1 and run `/effort max`.
3. Paste the start message from `PROMPT.md` Section 0.
4. At every gate the Manager prints a summary and asks for one word. Run `/usage` first. Reply `continue` when the 5-hour and weekly windows are both under 90%, `pause` when either is at or above 90%, or `compact` if the context has grown large (the Manager prints the `/compact` text to use).
5. After `pause`, wait for the window to reset, reopen the session with `claude --continue` (or the VS Code session picker), and type `resume`. The Manager reads the run's `STATE.md` and continues from its "Next action".
6. The run ends at Gate 4 with `report/REPORT.md`. Confirm `git status --short` shows only `rust-audit/`; `target/` is ignored by git.

Interrupting mid-phase (Escape) kills running subagents. Their finished finding files and logs stay on disk; on `resume` the Manager re-launches only the agents STATE.md marks as running or failed.

## Compaction and usage, in short

- Claude Code compacts automatically as the context fills; the prompt cannot prevent it, so it makes compaction harmless: agents return at most ten lines, all substance is in files, and STATE.md overrides memory. If the Manager ever seems lost after a compaction, say: "Read the run's STATE.md and continue from Next action." (For run 3 that file is `state/run3/STATE.md`.)
- Claude cannot read plan usage from inside a session; `/usage` is the operator's job. The 90% threshold and the pause protocol are in `PROMPT.md` Section 5.
- Plan for several 5-hour windows. Reviewers each read three to eight thousand lines plus tests; Critics and QA re-read the cited code.

## After the run

Findings are proposals. The team triages `report/REPORT.md`, verifies the Evidence / Inference / Hallucination trail of anything it intends to act on, and applies fixes through normal pull requests. Nothing in this folder needs to be kept in the repository once the team has what it needs; if it is kept, `just check` formats Markdown with Prettier, so run `just fix` before committing.

## Run 2 (independent second opinion)

PROMPT.md Section 11 defines a second, independent run over the same crates with `crates/sentinel-engine` out of scope and two extra assumptions (A16 genesis need not be recoverable, A17 only the services touch their databases). Run-2 agents did not read run-1 findings, report or state; its reconciliation phase was the first place both runs met.

Its findings (`findings/F2-*.md`) and proofs of concept (`poc/F2-*/`) stay where they carry a live row. Run 3 removed the run-2 files that were folded into a canonical row and the whole `state/run2/` folder (briefs, baselines, coverage, agent logs, per-crate reconciliation parts). Those paths resolve at `<run-3 review commit>`.

## Run 3 (blind discovery plus re-validation)

PROMPT.md Section 12 defines a third run over the same scope at `361aaf3` (the merge of `origin/main` `c355af1`), with Claude Opus 5.5 at max effort and assumptions A1 to A17 plus one Aegis rule: every node starts fresh. It ran two tracks. Blind discovery filed `findings/F3-*.md` without reading any earlier material. An informed re-validation re-checked every earlier row at the audit commit, appended a `## Run 3 re-validation` section to the row's file and re-ran its PoC under `poc/<id>/run3/`. Every Medium or higher run-3 row got two independent reproductions, a practicality check against the Aegis production parameters and a confidence score. The tracks met in a reconciliation that rewrote `report/RECONCILIATION.md`. Resolved rows and the earlier working files then left the folder.

| Path | Contents |
| --- | --- |
| `state/run3/STATE.md` | Run-3 state: agent roster with outcomes, decisions, the safety-boundary note, next action. |
| `state/run3/baseline.md` | Toolchain, build, tests, clippy, `cargo audit` and the refreshed assumption values at the audit commit. |
| `state/run3/map.md` | The codebase map at the audit commit, the reviewer split, open issues and TODOs. |
| `state/run3/settled.md` | What blind agents were told not to file, the known items and the real parameters. |
| `state/run3/coverage.md` | Per-file coverage and the disposition of every observation. |
| `state/run3/agents/` | Every agent's log: reviewers, Critics, QA, re-validators, practicality, confidence, reconciliation, documentation. |
| `state/run3/revalidation/`, `state/run3/phase4/` | The re-validators' summaries; the practicality, confidence and fix-verification ledgers. |
| `state/run3/logs/` | Command and test logs. |
| `state/run3/removal-manifest.md` | What left the folder in run 3 and why, with the link check. |
| `poc/F3-*/`, `poc/<id>/ca3/`, `poc/<id>/ca3-fix/` | Run-3 proofs of concept and the confidence and fix-verification evidence. |

Builds, tests and PoCs ran in detached worktrees at the audit commit, outside the repository. Integration scripts ran in a private network namespace. The run folder and the worktrees stay on the audit host. What a finding needs was copied into `poc/` and `state/run3/`.
