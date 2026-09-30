# Run 2 — shared brief for Critics

Repository root: `/home/shebin.guest/safe/safenet`. Audited commit: see `state/run2/STATE.md` (field **Commit**). Your job is **falsification**. A Critic who confirms everything has done nothing.

## 1. Read before starting

`PROMPT.md` Sections 1, 2, 6 (the **Critic** role), 8 (finding format, **certainty rubric**, severity scale) and **11 (run-2 addendum)**; `state/run2/reviewer-brief.md` (what the reviewers were told); `state/run2/baseline.md`; and the coverage logs of the reviewers you cover under `state/run2/agents/` — **including their "considered and rejected" lists**, which you must mine (§5).

## 2. Independence

Run 2 is a second opinion. **Do not open** `findings/F-*.md` (run 1), anything under `report/`, or `state/*.md` outside `state/run2/`. You judge `findings/F2-*.md` only. Reconciliation with run 1 is a later, separate phase.

## 3. Scope, assumptions, environment

- `crates/sentinel-engine` is **out of scope**. A claim that depends on engine behaviour is at most `I`, and a finding whose only subject is the engine is out of scope — say so.
- **A16**: a defect whose only impact is genesis-ceremony liveness is Informational, tagged `known`. Reaching later epochs keeps full severity. **A17**: triggers needing out-of-band database access are out of scope; service-caused corruption is in. Apply both when you re-judge severity — do not leave that to the report.
- **Executed evidence is reachable this run.** cargo/rustc 1.98.1 and Foundry 1.8.1 are installed. Start every bash call with `export PATH="$HOME/.foundry/bin:$HOME/.cargo/bin:$PATH" CARGO_TARGET_DIR=/home/shebin.guest/.cache/safenet-run2/target CARGO_BUILD_JOBS=2`. You may run existing tests and read-only commands to check a claim (`cargo test -p <crate> --bins <filter>` for `validator`/`sentinel`, `--lib` for `safenet-core`). **Do not edit tracked files** — writing PoCs is QA's job. `/tmp` is a small RAM-backed tmpfs: never build there. Foreground commands only, long timeout. Dependency sources are on disk under `~/.cargo/registry` — a claim about `frost-core`, `alloy` or `sqlx` internals can now be checked against the source and should be, instead of being left as `I`.

## 4. Method — form your own view first

For each assigned finding: read **only the title and `Location`**; open the cited code and decide for yourself what it does; _then_ read the reviewer's Claim, Basis, Trigger and Considered-and-rejected and compare. Reading the argument first makes you its editor, not its adversary.

## 5. Verdicts

**Re-open every citation.** Per Basis row: **Supported**, or **Unsupported → `H`** when the location does not contain the quoted code, the claim contradicts it, or it relies on an identifier, API or dependency behaviour absent from this checkout or from `Cargo.lock`'s pinned versions. Quote the real lines as counter-evidence.

Finding verdict and certainty per Section 8 — set the number yourself, never inherit the reviewer's estimate:

| Verdict | Meaning | Certainty |
| --- | --- | --- |
| Confirmed | mechanism **and** trigger verified | 70–89 on `E2`; **90–100 only with `E1`** (an executed reproduction saved under `rust-audit/`) |
| Plausible | mechanism verified, trigger unproven | 40–69 |
| Refuted | counter-evidence with citations | 0 |
| Unsupported | depends on an `H` claim | 0 |

Below 40 is not a finding: say so, and it goes to unverified observations. **Re-judge severity** against Section 8's scale for _this_ system, then apply A16/A17. Severity inflation is the commonest defect in reports like this; a panic no untrusted input reaches is Low however alarming it looks, while an attacker-triggerable stall is High.

**Mine the rejected hypotheses.** For each entry in your reviewers' rejected lists, check that the citation actually refutes it. Promote anything wrongly dismissed as a new Draft finding attributed to you, in the next free ID of that reviewer's `F2-` range. A lead dismissed without a citation is the likeliest place a real defect is hiding.

## 6. Recording

**Append** a `## Critic (<your name>)` section; never edit or soften the reviewer's text. Update the header: `Status → Critiqued`, `Certainty` → your number, `Severity → <reviewer> / <yours>`. Keep every header table row terminated with `|`. If two findings describe one defect, say so in both and name the canonical one; merge or delete nothing.

## 7. Boundaries and reporting

Write only under `rust-audit/`. Never modify tracked files outside it, never commit, branch, stash, push or fetch. Return at most ten lines: verdict counts, refuted IDs with a three-word reason each, promotions, blockers. Never paste finding text into chat.
