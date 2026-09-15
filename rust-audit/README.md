# Rust Audit Kit

Everything for the AI-driven review of the Rust services lives in this folder. Nothing here is wired into CI or the build; it is documentation plus the working files the audit run produces.

| File or folder | Purpose |
| --- | --- |
| `PROMPT.md` | The multi-agent prompt. Section 3 is the assumptions checklist the team completes before running; Section 0 is the message that starts the run. |
| `codebase-map.md` | Read-only analysis of the four crates made while writing the prompt: architecture, trust boundaries, invariants, hotspots, seeded leads, reviewer assignments. |
| `analysis/` | The four full per-crate analysis reports the map condenses; agent-generated, citation-spot-checked, unexecuted. Reviewers read the one for their crate. |
| `state/` | Produced by the run: `STATE.md` (single source of truth), `baseline.md`, `coverage.md`, per-agent logs, command logs. |
| `findings/` | Produced by the run: one file per finding with the reviewer, Critic and QA sections appended in order. |
| `poc/` | Produced by the run: proof-of-concept sources and their outputs, one folder per finding. |
| `report/REPORT.md` | Produced by the run: the final report. |

## Which mode to run

Run the prompt in a normal interactive Claude Code session on Claude Fable 5.1 at `/effort max`. This is enough; the "ultracode" setting (xhigh effort plus automatic workflow planning) and the Workflow tool are not required.

Reasons:

- The run is gated by human usage checks between phases. A foreground Manager with background subagents stops cleanly at a gate; a background workflow keeps spawning agents until it is killed.
- Resilience comes from files, not from the harness: every agent writes its findings and logs to disk as it goes, and `state/STATE.md` is re-read after any interruption or compaction. A stopped workflow can only be resumed inside the same session, so it adds little here.
- About ten to fifteen agents across five phases is well within what the Agent tool handles directly (the default concurrency limit is 20 subagents).

If the team prefers workflows anyway, say "use a workflow" in the start message; Section 10 of the prompt tells the Manager how to map phases onto workflows. Effort levels in Claude Code are `low`, `medium`, `high` (default), `xhigh`, and `max`, set with `/effort`, the `--effort` flag, or `effortLevel` in settings. Subagents inherit the session model; if you define custom agents under `.claude/agents/`, pin `effort: max` in their frontmatter. Fable 5.1 has a 1M-token context window with no extra setting.

## Machine requirements

The machine used to write this kit had no Rust toolchain, no Foundry, and under 4 GB of RAM, so the analysis in `codebase-map.md` is read-only. The audit run itself needs:

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
5. After `pause`, wait for the window to reset, reopen the session with `claude --continue` (or the VS Code session picker), and type `resume`. The Manager reads `state/STATE.md` and continues from its "Next action".
6. The run ends at Gate 4 with `report/REPORT.md`. Confirm `git status --short` shows only `rust-audit/`; `target/` is ignored by git.

Interrupting mid-phase (Escape) kills running subagents. Their finished finding files and logs stay on disk; on `resume` the Manager re-launches only the agents STATE.md marks as running or failed.

## Compaction and usage, in short

- Claude Code compacts automatically as the context fills; the prompt cannot prevent it, so it makes compaction harmless: agents return at most ten lines, all substance is in files, and STATE.md overrides memory. If the Manager ever seems lost after a compaction, say: "Read rust-audit/state/STATE.md and continue from Next action."
- Claude cannot read plan usage from inside a session; `/usage` is the operator's job. The 90% threshold and the pause protocol are in `PROMPT.md` Section 5.
- Plan for several 5-hour windows. Reviewers each read three to eight thousand lines plus tests; Critics and QA re-read the cited code.

## After the run

Findings are proposals. The team triages `report/REPORT.md`, verifies the Evidence / Inference / Hallucination trail of anything it intends to act on, and applies fixes through normal pull requests. Nothing in this folder needs to be kept in the repository once the team has what it needs; if it is kept, `just check` formats Markdown with Prettier, so run `just fix` before committing.
