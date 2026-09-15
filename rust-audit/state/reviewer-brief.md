# Shared brief — Phase 1 Reviewers (R1–R10)

Repository root: `/home/shebin.guest/safe/safenet`. Commit `2893917` (`AI review changes`). This file is the Manager's standing brief. Your launch message adds only your assignment.

## 1. Read before starting

1. `rust-audit/PROMPT.md` — Sections 1 (boundaries), 2 (evidence discipline), 6 (the **Reviewer** role), 8 (finding file format + certainty rubric + severity scale). Skim Section 4 (scope).
2. `rust-audit/codebase-map.md` — Section 1 (architecture, trust boundaries), Section 4 (known items), Section 5 (cross-cutting checklist), **your** subsection of Section 6, Section 7 (Manager leads M1–M10), Section 8 (cross-crate themes).
3. `rust-audit/analysis/analysis-<your crate>.md` — the full prior analysis for your crate. **Treat every lead in it as an unverified hypothesis, never as a finding.** The prior pass had no toolchain and its citations are only spot-checked; a wrong line number or a quoted identifier that does not exist is exactly what you are here to catch. Re-open every line you rely on.

## 2. Run mode — READ ONLY (this is not the mode the prompt assumes)

There is **no Rust toolchain on this machine**: no `cargo`, `rustc`, `rustup`, `forge`, `anvil`, `just`. Assumption A9 is FALSE.

- Do **not** run or attempt to install `cargo` anything. Do not run `curl`/`rustup`/`apt`. No network.
- `cargo test -p <crate>` in the Reviewer role description is **not available**. Where that role says "run targeted tests", instead **read** the tests and say what they would prove.
- Consequence for you: `E1` is unreachable. Your best basis class is `E2` — cited code plus a concrete input, state or event sequence that reaches the behaviour. An `E2` claim must name the trigger concretely; "an attacker could perhaps" is `I`.
- `sqlite3`, `jq`, `python3`, `grep`, `rg` are available for static inspection of files under the repo. Reading the SQLite schema in `.sql`/migration files is fine.

## 3. Assumptions (all fifteen signed off — see `rust-audit/state/STATE.md`)

A1 trusted operator · A2 chain data adversarial within the <1/3 fault bound, Safe transaction contents fully attacker-controlled · A3 **engine API reachable only by its co-deployed sentinel** (missing auth/rate-limiting is Informational unless you find a bypass inside that deployment) · A4 **malicious RPC is OUT of scope**, but stale / rate-limited / incomplete `eth_getLogs` results are IN scope · A5 reorgs up to `max_reorg_depth` must be handled, deeper is a deliberate exit · A6 crypto libraries trusted — but their sources are **not on disk**, so any claim about `frost-core`/`alloy`/`sqlx`/`k256` internals is class `I`, never `E2` · A7 Solidity under `contracts/src` is the audited reference for hashing, encoding and protocol rules; a Rust/Solidity mismatch is a **Rust** finding · A10 Gnosis Chain ~5 s blocks, defaults `blocks_per_epoch` 1440, `key_gen_timeout` 120, `signing_timeout` 6, `oracle_timeout` 12, nonce chunk 1024, threshold n/2+1 · A11 scope is exactly PROMPT.md Section 4 · A12 the known TODOs in codebase-map Section 4 are **reported but tagged `known`** at reduced priority, never omitted · A13 no branches, no commits, no PRs.

**A15 is TRUE this run.** The Safenet Arbitration Charter is available locally at `/tmp/claude-501/-home-shebin-guest-safe-safenet/166c992f-87ea-4548-be85-5fcf1c13bf90/scratchpad/safenet-charter/Safenet_Arbitration_Charter.md` (909 lines, upstream commit `44a1e53`, defining R-4.1 … R-4.6 — exactly the set `crates/sentinel-engine/src/engine/rule.rs` cites). R8 and R9: hold the checkers to this text and cite it as `safenet-charter@44a1e53:Safenet_Arbitration_Charter.md:<lines>` **with a verbatim quote**, because the path is session-local and will not survive.

## 4. Hard boundaries

- Write **only** under `rust-audit/`. Never create, modify or delete a tracked repository file. Never commit, branch, stash, push, or touch git config. Never `git checkout`.
- Never fix code. Remediation is prose inside the finding file.
- Do not copy secrets or database contents anywhere. Sample keys in the repo are placeholders — say so rather than quoting them.
- Do not add features, refactor or tidy. The deliverable is documentation.
- Reference-only material (read freely, **no findings on it**): `contracts/src`, `docs/`, `scripts/`, `AGENTS.md`, `epics/`, and the Charter.

## 5. What you produce

### 5.1 Finding files — write each one the moment it is drafted

Path `rust-audit/findings/F-<CRATE>-<nnn>.md`, using **your allocated ID range** (in your launch message) so parallel reviewers never collide. Use the template in PROMPT.md Section 8 exactly: the header table (Status `Draft`, crate/module, location, severity, certainty, assumptions, tags), then `## Claim`, `## Basis` (the per-claim table with class E1/E2/I, `path:line-range`, and a **verbatim quote of at most 15 lines**), `## Trigger`, `## Considered and rejected`, `## Remediation options`, `## Trail`.

Rules that decide whether your finding survives Phase 2:

- Every claim cites `path:line-range` **in this checkout** and quotes the real lines. Open the file and copy them; do not reproduce a quote from the analysis file without re-opening it. A citation that does not contain the quoted code is marked `H` by the Critic and sinks the finding.
- `## Trigger` must give the concrete input, state or event sequence, or the literal words `none identified`.
- `## Considered and rejected` must name the guards you actually checked, with citations, and say why this is not a false positive. A finding without this section is weak by construction.
- Severity per PROMPT.md Section 8's scale for **this** system. Do not inflate: an `unwrap` that cannot be reached from untrusted input is Low or Informational, not High.
- Self-estimate a certainty percentage in the Trail line. Be honest and calibrated; the Critic sets the final number and will notice padding.
- Tag `known` on anything in codebase-map Section 4 and file it at reduced priority.

### 5.2 Coverage log — `rust-audit/state/agents/R<n>.md`

Write it as you go, not at the end. It must contain:

- **Files read**: every assigned file with its `wc -l`, and whether you read 100% of it. If you did not finish a file, say so explicitly — an honest gap is worth more than a false claim of coverage. The Coverage Critic checks this against the canonical inventory.
- **Commands run**: every command, verbatim.
- **Hypotheses considered and rejected**: one entry per lead from the map/analysis and per idea of your own, each with the citation that refuted it. The Critic re-reads this list and promotes anything wrongly dismissed, so a lead you dismiss without a citation is a liability.
- **Observations**: plausible but unverified concerns (below 40% by the rubric). These are not findings but must not be silently dropped.

## 6. How to review (priorities, in order)

1. Consensus-critical correctness — hashing, encoding, ordering, Merkle logic, EIP-712 and threshold arithmetic must match the Solidity reference exactly.
2. Secret handling — generation entropy, storage, zeroisation, `Debug`/`Display`, `tracing` fields, error messages, metrics labels.
3. Reorg and crash consistency — is every durable write ordered so a crash between the side effect and the record is safe? Is every effect idempotent under replay, as `core::state` requires?
4. Input validation at trust boundaries — chain logs, RPC responses, HTTP bodies, config, DB rows.
5. Panics reachable from untrusted input — `unwrap`, `expect`, indexing, slicing, `as` casts, unchecked arithmetic in non-test code. In the engine a panic is an HTTP 500 and a missing vote; in the validator or sentinel it can be a crash loop.
6. Resource exhaustion — per-request and per-block work, RPC fan-out, retry storms, unbounded maps, memory growth over long runs.
7. Concurrency — `select!` cancel-safety, task failure propagation, locks held across awaits, blocking work on the async runtime.
8. Configuration — defaults, validation, `deny_unknown_fields`, dangerous combinations, sample files matching the schema.
9. Tests — what they prove, what they mock away, whether a proposed fix has a test hook.

Quality bar: **a few well-evidenced findings beat many vague ones**, but record every plausible concern as an observation so nothing is silently dropped. Read every assigned file completely, including its tests — the tests often encode the intended invariant and are where a mismatch shows up.

## 7. Reporting back

Return **at most ten lines** in chat: the finding IDs you wrote with one-line titles and self-estimates, your coverage-log path, files you did not finish, and any blocker. Never paste finding text into chat — it lives in the file.
