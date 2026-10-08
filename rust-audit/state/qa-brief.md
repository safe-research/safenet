# Shared brief — Phase 3 QA

Repository root: `/home/shebin.guest/safe/safenet`. Commit `2893917`.

## 1. Read first

`rust-audit/PROMPT.md` Sections 1, 2, 6 (the **QA** role), 8 · `rust-audit/state/critic-brief.md` · `rust-audit/state/baseline.md` (what the environment can and cannot do) · the finding files assigned to you, **including their `## Critic` sections**, which set the current certainty and verdict.

## 2. The constraint that reshapes this role

**There is no Rust toolchain on this machine** — no `cargo`, `rustc`, `forge`, `anvil`, `just`; no dependency sources on disk; no `sentinel-test-vectors` corpus (A8 and A9 both FALSE). You therefore **cannot execute anything**, and the normal QA deliverable — turning `E2` into `E1` — is unreachable this run.

Do not attempt to install a toolchain, do not run `curl`/`rustup`/`apt`, do not use the network. Do not write a `## QA` section claiming a test passed or failed.

What you do instead, and it is still worth doing well:

### 2.1 Write the PoC the team will run

For each assigned finding whose Critic verdict is **Confirmed** or **Plausible**, write a **complete, ready-to-run** Rust test or reproduction under `rust-audit/poc/<finding-id>/`, plus a `README.md` in that directory giving:

- the exact command the team runs once they have a toolchain (`cargo test -p <crate> --test <name>`, or the `just` recipe, or the Anvil script);
- what a **passing** run means and what a **failing** run means — state the expected observable outcome precisely (a panic, a wrong verdict value, a stalled block number, a nonce reused across two messages), so the result is unambiguous to someone who did not read the finding;
- every fixture the test needs: concrete calldata, addresses, block numbers, event sequences, config values. Under A2 the Safe transaction contents and chain messages are attacker-controlled, so spell the attacker's input out literally rather than describing it.

The PoC must be **honest about being unexecuted**. Write it as real code against the crate's actual APIs — check the function signatures, types and module paths by reading the source, and match the conventions of the crate's existing tests — but say in the README that it has never been compiled and may need mechanical fixes. A test that names a function that does not exist is worse than no test: verify every identifier you use against the checkout.

### 2.2 Check the remediation options

For each finding, read `## Remediation options` and assess whether at least one is **sound**: does it actually close the mechanism the Critic confirmed; does it break a documented runtime contract (`core::state`'s "transitions are pure and never fail", "effects may run more than once", "resume ordering is undefined"); does it contradict the Solidity reference under A7; does it introduce a new failure mode. Where a proposed fix is wrong or incomplete, say so plainly and give a better one. Where a fix needs a test hook that does not exist, say what would have to change.

You may **read** as much of the codebase as you need. You may **not** modify any tracked file — the PoC lives only under `rust-audit/poc/`.

### 2.3 Append a `## QA (<your agent name>)` section

Use exactly one of these outcomes, and never overstate:

- **Not attempted (no toolchain)** — the correct outcome for every execution claim this run. Say what would be run and what it would show.
- **Reproduced by inspection** — only when you traced the exact code path end to end and can cite each step; this does **not** raise the finding into the 90–100 band, which needs `E1`. Say so explicitly.
- **Not reproduced** — you traced the path and it does not behave as claimed. This is a real result: report it, cite the counter-evidence, and say the Critic's certainty should drop.

You may **raise** a certainty only within the `E2` ceiling of **89%**, and only with new evidence of your own. You may **lower** one, with reasons. Record the PoC path in the finding's `## QA` section.

## 3. Settle the dependency questions you can

Several findings are blocked on behaviour of `frost-core`, `alloy-sol-types`, `serde` and `sqlx` whose sources are **not on disk**. You cannot resolve those (A6 keeps them class `I`). What you _can_ do is write down, per finding, the **exact check** that would settle it — the file and item to read in the vendored source, or the three-line program to run — so the team can close it in minutes. Collect these into `rust-audit/poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md`.

## 4. Boundaries

Write only under `rust-audit/`. Never modify, create or delete a tracked repository file — the PROMPT's allowance for temporary PoC edits to tracked files is **void this run**, because without a toolchain there is nothing to gain from them and every edit is a risk. Never commit, branch, stash or push. Never fix code in the repository. Do not copy secrets or database contents anywhere.

## 5. Reporting back

Return at most ten lines: PoC directories written, findings whose remediation you judged unsound, findings whose certainty you moved and in which direction, and any blocker.
