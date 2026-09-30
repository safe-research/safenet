# Run 2 — shared brief for QA

Repository root: `/home/shebin.guest/safe/safenet`. Audited commit: see `state/run2/STATE.md`. Read `PROMPT.md` Sections 1, 2, 6 (the **QA** role), 8 and **11**, then `state/run2/critic-brief.md`, then each assigned finding **including its `## Critic` section**, which sets the current verdict and certainty.

## 1. This run can execute — so do

Unlike a read-only run, your job here is the real one: **turn `E2` into `E1`** by writing a failing test or PoC, running it, and saving source and output under `poc/F2-<id>/`. Start every bash call with `export PATH="$HOME/.foundry/bin:$HOME/.cargo/bin:$PATH" CARGO_TARGET_DIR=/home/shebin.guest/.cache/safenet-run2/target CARGO_BUILD_JOBS=2`. Foreground commands, long timeout; never build under `/tmp`.

- `sentinel` and `validator` are **binary-only** crates and `safenet-core`'s `tx::storage` is private, so a PoC usually cannot live in `tests/`: paste it into an existing `#[cfg(test)] mod tests` block, run `cargo test -p <crate> --bins <filter>` (`--lib` for core), then **revert**.
- **Temporary edits to tracked files are allowed for exactly this** (PROMPT.md Section 1) under strict discipline: before reverting, `git diff <file>` must show only your PoC hunk; then `git checkout -- <file>`. At the end, `git status --short` from the repo root must show nothing outside `rust-audit/` — verify and report it. Never commit, branch, stash, push or fetch.
- Dependency sources are on disk under `~/.cargo/registry`: a question about `frost-core`, `alloy` or `sqlx` behaviour is settled by reading or running against the source, not by listing it as open.
- Live scenarios run on **local Anvil only**, on the port range in your launch message. All three `*.sample.toml` ship `rpc = "https://rpc.gnosischain.com"` — **live Gnosis mainnet** — so never start a service from an unmodified sample: copy it, rewrite `rpc` to loopback, and print the effective value into your log. Kill only PIDs you started. Note that `anvil_reorg` mines **empty** replacement blocks, so reorged transactions are dropped and logs are not replayed — a trigger that needs replay may be untestable that way; say so rather than working around it dishonestly.

## 2. What to record

Append a `## QA (<your name>)` section with exactly one outcome per finding:

- **Reproduced** — the PoC ran and showed the claimed behaviour. Give the command, the verbatim decisive output and the path under `poc/`. This is what lets a Confirmed finding enter the 90–100 band; say what certainty you set and why.
- **Not reproduced** — you traced or ran the path and it does not behave as claimed. A real result: report it, cite the counter-evidence, and lower the certainty or recommend Refuted.
- **Not attempted** — with the precise reason (needs a corpus, needs a restart no harness performs, needs live-chain data). Then write the PoC anyway, honestly labelled unexecuted, with the exact command and expected outcome.

**A PoC that fails to compile, or passes where the finding says it should fail, is evidence against the finding.** Repairing signatures and imports is legitimate; changing what a test asserts is not. Distinguish "needed mechanical repair, then reproduced" from "did not reproduce".

## 3. Remediation check

For every assigned finding, read `## Remediation options` and say whether at least one is **sound**: does it close the mechanism the Critic confirmed; does it break the documented runtime contract in `crates/core/src/state/mod.rs` (transitions pure and non-failing; effects may run more than once; resume ordering undefined); does it contradict the Solidity under A7; does it introduce a new failure mode. A fix that makes things worse is more urgent than a finding — call it out plainly and give a better option. Compile a fix in a scratch copy when cheap.

## 4. Scope, assumptions, boundaries

`crates/sentinel-engine` is out of scope. Apply **A16** and **A17** when judging severity and triggers. Do not read run-1 findings, reports or state outside `state/run2/`. Write only under `rust-audit/`. Return at most ten lines: PoC directories written, outcomes per finding, certainties moved and in which direction, remediations judged unsound, and the clean-tree confirmation.
