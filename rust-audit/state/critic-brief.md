# Shared brief — Phase 2 Critics

Repository root: `/home/shebin.guest/safe/safenet`. Commit `2893917`. Your job is **falsification**, not endorsement. A Critic who confirms everything has done nothing.

## 1. Read before starting

1. `rust-audit/PROMPT.md` — Sections 1 (boundaries), 2 (evidence discipline), 6 (the **Critic** role), 8 (finding format, **certainty rubric**, severity scale).
2. `rust-audit/state/reviewer-brief.md` — what the reviewers were told, including the run constraints you must hold them to.
3. `rust-audit/state/baseline.md` — Phase 0 facts. Section 3 is the verified inventory; section 6 is the static dependency analysis.
4. The coverage logs of the reviewers you cover (`rust-audit/state/agents/R<n>.md`) — **including their "hypotheses considered and rejected" lists**, which you must mine (see §5).

## 2. Run mode — READ ONLY

No Rust toolchain exists on this machine: no `cargo`, `rustc`, `forge`, `anvil`, `just`. Do not run or install any of them; no network.

This has a hard consequence you must enforce: **`E1` is unreachable this run, so no finding may be certified above 89%.** A reviewer claiming `E1`, or citing build/test/clippy/audit output, is claiming something that does not exist — mark that claim `H`. Dependency-advisory claims are likewise unverifiable (`cargo audit` was not run and no dependency source is on disk): any assertion about a CVE, a RUSTSEC ID, or the internals of `frost-core`, `alloy`, `sqlx` or `k256` is class `I` at best.

## 3. Method — form your own view first

For each finding assigned to you, in this order:

1. Read **only its title and `Location`**. Do not read the reviewer's reasoning yet.
2. Open the cited code yourself and work out what it does and whether anything is wrong with it. Write down your own view.
3. _Then_ read the reviewer's `## Claim`, `## Basis`, `## Trigger` and `## Considered and rejected`, and compare against what you derived.

This ordering is the point of the role. Reading the reviewer's argument first makes you its editor rather than its adversary.

## 4. Verdicts you must record

**Re-open every citation.** For each row of the `## Basis` table, check that the quoted lines exist at that `path:line-range` in this checkout and say what they actually do. Per-claim verdict:

- **Supported** — the quote is accurate and supports the claim.
- **Unsupported** → mark the claim **`H`** (hallucination). Assign `H` when: the cited location does not contain the quoted code; the claim contradicts the code; or it relies on an identifier, API, type, method or dependency behaviour that does not exist in this checkout or in the versions pinned in `Cargo.lock`. Quote the real lines as counter-evidence.

Then a **finding verdict**:

| Verdict | Meaning | Certainty |
| --- | --- | --- |
| **Confirmed** | mechanism **and** trigger verified against the code | 70-89 (`E2`; 90+ impossible this run) |
| **Plausible** | mechanism verified, trigger unproven | 40-69 |
| **Refuted** | you have counter-evidence, with citations | 0 |
| **Unsupported** | depends on a claim you marked `H` | 0 |

Set the certainty number yourself using PROMPT.md Section 8's rubric — **do not inherit the reviewer's self-estimate**, and do not split the difference to be polite. Anything you land below 40 is not a finding: say so, and it moves to the unverified-observations list.

**Also re-judge severity** against Section 8's scale for _this_ system. Severity inflation is the most common defect in this kind of report: a panic that no untrusted input can reach is Low or Informational however alarming it looks; a stall that an attacker can trigger deliberately is High. Where the reviewer's severity is wrong, state the corrected one and why. Check too that the finding respects the confirmed assumptions — under **A1** "the operator can read the key file on disk" is not a finding; under **A3** missing engine auth or rate limiting is Informational absent a bypass inside the deployment; under **A4** "a malicious RPC could lie" is out of scope, while "a stale, rate-limited or incomplete RPC response" is in scope; under **A7** a Rust/Solidity mismatch is a **Rust** finding.

## 5. Mine the rejected hypotheses — this is half your value

Read every reviewer's "hypotheses considered and rejected" list in their coverage log. For each, check whether the citation given actually refutes it. **Promote anything wrongly dismissed into a new Draft finding attributed to you**, using the same template and the next free ID in that reviewer's range (state in the Trail that you, the Critic, drafted it). A lead dismissed with no citation, or with a citation that does not say what the reviewer claims, is the most likely place a real bug is hiding — the reviewer already looked at it and talked themselves out of it.

## 6. How to write it

**Append** a `## Critic (<your agent name>)` section to the finding file. **Never edit, soften or delete the reviewer's text** — the file is an append-only trail showing how the result was reached, and a disagreement left visible is more useful to the team than a tidy consensus. Your section contains: per-claim verdicts (with your counter-quotes where relevant), the finding verdict, the certainty number, the corrected severity if you changed it, and your reasoning.

Also update the finding's header table: `Status` → `Critiqued`, `Certainty` → your number, `Severity` → `<reviewer> / <your final>`.

If two findings from different reviewers are the same defect, say so in both and name the one that should be canonical; do not merge or delete either file.

## 7. Boundaries

Write only under `rust-audit/`. Never modify a tracked repository file, never commit, branch, stash or push. Never fix code. Do not copy secrets or database contents anywhere.

## 8. Reporting back

Return **at most ten lines**: counts by verdict (Confirmed / Plausible / Refuted / Unsupported), the IDs you refuted with a three-word reason each, any new findings you promoted, and any blocker. Never paste finding text into chat.
