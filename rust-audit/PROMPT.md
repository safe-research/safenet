# Safenet Rust Audit Prompt

| Field | Value |
| --- | --- |
| Version | 1.0, against commit `82b3e0d` on `main` |
| Target | Rust services under `crates/`: `core`, `validator`, `sentinel`, `sentinel-engine` (about 24,200 lines) |
| Runtime | Claude Code, Claude Fable 5.1, `/effort max`, subagents via the Agent tool. The Workflow tool is optional (Section 10). |
| Companion | [README.md](./README.md) (operator guide, mode recommendation), [codebase-map.md](./codebase-map.md) (architecture, hotspots, seeded leads, reviewer assignments), `analysis/` (full per-crate analyses) |

## 0. Operator start message

Paste this as the first message of the session after completing the checklist in Section 3:

```text
Read rust-audit/PROMPT.md in full and act as its Manager. If rust-audit/state/STATE.md exists, resume from its "Next action" section; otherwise start Phase 0. I am a maintainer of this repository reviewing our own Rust services. The goal is to strengthen the code by finding, verifying and documenting bugs and security weaknesses, with proposed fixes for the team to apply. Nothing is committed; all output goes under rust-audit/.
```

## 1. Mission and boundaries

You are the Manager of a multi-agent security and robustness review. Coordinate specialist agents so that every in-scope file is read completely, every finding is adversarially verified by a fresh agent, everything reproducible is reproduced, and the result is `rust-audit/report/REPORT.md`, where each finding carries a certainty percentage and an explicit Evidence / Inference / Hallucination trail showing how it was finalised.

Boundaries for every agent:

- Write only under `rust-audit/` and the session scratch directory. Never commit, branch, stash, push, or edit git configuration. At every gate `git status --short` must list nothing outside `rust-audit/` (and `target/`, which is ignored).
- Temporary edits to tracked files are allowed only to QA agents for proof-of-concept tests. Revert them with `git checkout -- <file>` once `git diff <file>` shows only the PoC hunk. Keep the PoC source and its output under `rust-audit/poc/<finding-id>/`.
- Allowed commands: read-only inspection; `cargo build`, `test`, `clippy`, `doc`, `tree`, `audit`; the Anvil-based scripts under `scripts/` (QA only). Not allowed: `podman`, `docker`, live RPC endpoints, any network beyond the Cargo registry, `rm` outside `rust-audit/` and `target/`.
- Never change code to fix a finding. Remediation is described in the finding file.
- Do not copy secrets or database contents into any file; sample keys in the repo are placeholders.
- Do not add features, refactor, or tidy anything. The deliverable is documentation.

## 2. Evidence discipline

- Every claim about code cites `path:line-range` in this checkout and quotes the relevant lines verbatim (at most 15 lines). A claim without a citation is an inference and is labelled `I`.
- Basis classes: `E1` executed evidence (a test, PoC, or tool output saved under `rust-audit/`); `E2` code-traced evidence (cited code plus a concrete input, state, or event sequence that triggers the behaviour); `I` inference (pattern, documentation, protocol knowledge, or reasoning without a concrete trigger). `H` (hallucination) is assigned only by a Critic or QA agent when a cited location does not contain the quoted code, the claim contradicts the code, or it relies on an identifier, API, or dependency behaviour absent from this checkout or from the versions pinned in `Cargo.lock`.
- Before reporting progress, audit each claim against a tool result from this session. Write "not read" or "not run" rather than assuming. If a step was skipped, say so.
- Solidity under `contracts/src` is the reference for hashing, encoding and protocol rules. A Rust/Solidity mismatch is a Rust finding; Solidity itself receives no findings.
- Prefer a few well-evidenced findings over many vague ones, but record every plausible concern as an observation so nothing is silently dropped.
- When you have enough information to act, act. Do not re-derive facts already recorded in `state/` files.

## 3. Assumptions the team verifies before running

Tick each box, or replace it with `FALSE:` and a note. The Manager stops at Gate 0 while any box is unticked. Findings cite the assumption IDs they depend on.

| ID | Assumption | If false |
| --- | --- | --- |
| A1 | [ ] Trusted operator: config files, the signer private key, the SQLite files and the host filesystem are provisioned by an honest operator. Plaintext secrets at rest are a documented design choice (`docs/validator-handbook.md`). | Local secret protection enters scope; raise severity of every secret-handling finding. |
| A2 | [ ] Adversarial chain data: every onchain message from other participants (validators, sentinels, proposers, users) is attacker-controlled within the protocol's fault bound (fewer than one third of validators dishonest). Safe transaction contents are attacker-controlled. | Add malicious-majority scenarios to the Critic briefs. |
| A3 | [ ] Engine deployment: the sentinel engine's HTTP API is reachable only by its co-deployed sentinel (`docs/sentinel-engine.md`). Missing authentication or rate limiting on it is Informational unless a bypass exists within that deployment. | Escalate API hardening findings to High. |
| A4 | [ ] RPC provider: trusted for liveness and for eventual block and log correctness, but may be stale, rate limited, or return incomplete `eth_getLogs` results (handbooks, `use_client_filtering`). An actively malicious RPC is out of scope. TEAM TO CONFIRM. | Add "malicious RPC" to the briefs of the `core` reviewers. |
| A5 | [ ] Reorgs up to the configured `max_reorg_depth` must be handled; deeper reorgs cause a deliberate exit (PR #834). | The deliberate exit becomes a liveness finding. |
| A6 | [ ] Libraries `frost-core`, `frost-secp256k1` (v3), `k256`, `sha2`, `hkdf`, `alloy`, `sqlx` are trusted. Review Safenet's usage and adaptations (address-derived identifiers, ECDH share encryption, Merkle nonce commitments, EIP-712 hashing), not library internals. | Add a dependency-internals reviewer. |
| A7 | [ ] The Solidity contracts are audited (Certora, `contracts/audits/`) and are the reference for hashing, encoding and protocol rules. | Mismatches need a joint decision on which side is wrong. |
| A8 | [ ] The `sentinel-test-vectors` corpus is not available locally. TEAM TO CONFIRM whether QA may clone it and run `just test-integration-sentinel-engine <path>`. | Checker findings can reach `E1`. |
| A9 | [ ] Environment: Rust stable, Foundry 1.5.1 (`anvil`, `forge`, `cast`), `just`, `jq`, at least 8 GB RAM and 15 GB free disk; network to the Cargo registry only. | Phase 0 downgrades the run to read-only review and the report says so. |
| A10 | [ ] Chain parameters: Gnosis Chain (about 5 s blocks); defaults `blocks_per_epoch` 1440, `key_gen_timeout` 120, `signing_timeout` 6, `oracle_timeout` 12, nonce chunk 1024, threshold n/2+1 (`docs/overview.md`, sample configs). | Re-evaluate every timing finding. |
| A11 | [ ] Scope is exactly Section 4. | Edit Section 4 before starting. |
| A12 | [ ] Known items: the `TODO` markers listed in `codebase-map.md` and the validator flow-test epic (`epics/`). Report them tagged `known` at reduced priority rather than omitting them. | Omit them instead. |
| A13 | [ ] No git branches or commits; fixes are proposed inside finding files only. | Not applicable to this run. |
| A14 | [ ] The tree at the commit recorded in `state/STATE.md` does not change during the run. | Restart from Phase 0. |
| A15 | [ ] The Safenet Charter text that the engine rule codes (`R-x.y`) cite is available to reviewers; without it, verdict-policy findings stay Plausible. | Verdict-policy leads (ENG-H2 to H7) cannot be Confirmed. |
| A16 | [x] **(run 2)** Genesis does not need to be recoverable. The genesis ceremony is closely observed by the operator and is manually restarted with a new deployment on abnormal participation. Narrow reading: findings whose only impact is genesis-ceremony liveness are Informational and tagged `known`; findings that reach later epochs (rollover key generations, epoch-1 loss) remain in scope at full severity. | Restore genesis-liveness findings to their measured severity. |
| A17 | [x] **(run 2)** Only the services (`validator`, `sentinel`) access their respective databases. Triggers that require out-of-band database access — backup restore, copying a database to another chain or deployment, manual edits — are out of scope. Crash-consistency and reorg defects caused by the service itself remain in scope. | Operator-restore triggers (e.g. nonce reuse after a restore across a reorg) return to scope. |

## 4. Scope

- Findings allowed (run 1; **run 2 excludes `crates/sentinel-engine` — see Section 11**): `crates/core`, `crates/validator`, `crates/sentinel`, `crates/sentinel-engine` (every `.rs` file), `Cargo.toml`, `Cargo.lock`, `crates/*/Cargo.toml`, `crates/*/Dockerfile`, `crates/*/*.sample.toml`, `crates/sentinel-engine/openapi.yaml`.
- Reference only (read to understand, no findings): `contracts/src`, `docs/`, `scripts/`, `AGENTS.md`, `epics/`.
- Out of scope: `explorer/`, `examples/`, `certora/`, `.github/`, findings on Solidity.

## 5. Phases and gates

| Phase | Agents | Output |
| --- | --- | --- |
| 0 | Recon (1) | `state/baseline.md`: toolchain versions, `cargo build`, `test`, `clippy`, `audit` results with log paths, per-file line counts compared with `codebase-map.md`, drift notes. `STATE.md` created. |
| 1 | Reviewers R1 to R10 in parallel (assignments in `codebase-map.md`) | `findings/F-*.md` in status Draft, `state/agents/<agent>.md` coverage logs, observations. |
| 2 | Critics: one per crate, one cross-cutting, one Coverage Critic | A Critic section appended to every finding; new Draft findings for wrongly dismissed items; `state/coverage.md`. |
| 3 | QA: one per crate that has a Confirmed or Plausible finding | PoCs under `poc/<id>/`, a QA section appended to each finding, `cargo clippy` and `cargo audit` findings. |
| 4 | Documentation (1) | `report/REPORT.md`; the Manager verifies every finding file is represented and the tree is clean. |

Gate protocol. At the end of every phase, and whenever a batch of agents completes:

1. Update `state/STATE.md` (Section 7) before doing anything else.
2. Print a gate summary: phase, agents done and failed, findings by status, files written, next action.
3. Ask the operator to run `/usage` and wait for one word: `continue` (proceed), `pause` (write `PAUSED (usage)` into STATE.md and end the turn; the operator returns after the reset with `resume`), or `compact` (the operator runs `/compact` with the text in Section 7; afterwards re-read STATE.md and continue).

You cannot measure plan usage yourself. If the harness shows a remaining-token budget, treat 10% remaining as an operator `pause`. Do not stop, summarise, or suggest a new session for context reasons otherwise.

Launch rules:

- Launch every agent of a phase at once unless the operator sets a smaller batch. Each launch prompt contains the role name, the assignment (crate, files, finding IDs), the paths of `PROMPT.md` and `codebase-map.md`, and the instruction to read Sections 1, 2, 6 (its role) and 8 before starting. Run agents in the background and keep working while they run.
- Agents never return their report in chat. They write files and return at most ten lines: paths written, counts, blockers.
- Record every completion in STATE.md immediately. Re-launch a failed agent once with the same assignment plus a pointer to its partial log, then record the failure and continue.
- If a Critic refutes a finding whose reviewer self-estimate was 70% or higher, send the counter-evidence back to that reviewer agent for one rebuttal round and record the outcome in the finding's trail.

## 6. Roles

Manager (this session). Owns STATE.md, launches agents, enforces gates, resolves reviewer and Critic disagreements (second Critic if needed), signs off the report. Never writes findings itself and never holds agent reports in context.

Recon. Verify assumption A9 with real commands and record versions. Run `cargo build --workspace --all-targets --locked`, `cargo test --workspace`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo audit` (install it with `cargo install cargo-audit --locked` if missing), saving full logs under `state/logs/`. Compare the file list and line counts with `codebase-map.md` and list any drift. Record the commit hash. If the toolchain is missing, say exactly what is missing and mark the run read-only.

Reviewer (R1 to R10). Read every assigned file completely, including tests. Answer the crate checklist in `codebase-map.md` for your assignment, treating its seeded hypotheses as leads to confirm or refute, never as findings. Write one finding file (Section 8) per defect or weakness the moment it is drafted, and one observation entry per plausible but unverified concern in your coverage log. Run `cargo test -p <crate>` and targeted tests when a claim can be checked cheaply. Priorities, in order: consensus-critical correctness, secret handling, reorg and crash consistency, input validation at trust boundaries, panics reachable from untrusted input, resource exhaustion, dependency advisories. Your coverage log lists every file read with its line count, every command run, and every hypothesis considered and rejected.

Critic. Falsify. For each assigned finding read only its title and location first, form your own analysis of the cited code, then read the reviewer's reasoning and compare. Re-open every citation and check that the quoted code exists and behaves as claimed. Give a per-claim verdict (Supported, or Unsupported which marks it `H`) and a finding verdict: Confirmed (mechanism and trigger verified), Plausible (mechanism verified, trigger unproven), Refuted (with counter-evidence and citations), or Unsupported (depends on an `H` claim). Set the certainty percentage per Section 8. Append your section; never edit the reviewer's text. Also read the reviewer's rejected hypotheses and promote anything wrongly dismissed as a new Draft finding attributed to you.

Coverage Critic. Build `state/coverage.md`: every in-scope file, its line count, which reviewer logs list it, which Critic touched it. Spot-read the three least-covered files per crate and file findings or observations for anything missed. Flag files no reviewer read.

QA. Turn `E2` into `E1` where feasible: write a failing test or PoC, run it, save the source and output under `poc/<id>/`, and append a QA section (Reproduced, Not reproduced, or Not attempted with the reason). Check that at least one remediation option is sound, compiling it in a scratch copy when cheap. Run the Anvil integration scripts only when a finding claims runtime behaviour they exercise. Revert every temporary edit before finishing.

Documentation. Compile `report/REPORT.md` (Section 9) from the finding files without changing any verdict, number, or wording of a claim. Link every finding to its file, build the coverage matrix from `state/coverage.md`, list unverified observations and rejected findings, and write the executive summary last.

## 7. State and resumption

`state/STATE.md` is the single source of truth and overrides conversational memory. On any start, restart, or after compaction, read it first and continue from "Next action". Update it after every agent completion and before every gate.

```text
# Audit state

| Field       | Value                                             |
| ----------- | ------------------------------------------------- |
| Commit      | <hash>                                            |
| Started     | <date>                                            |
| Mode        | full or read-only                                 |
| Phase       | <n> (<name>)                                      |
| Gate status | open, waiting-for-operator, or PAUSED (usage)     |

## Assumptions confirmed

A1 to A15 ticked, or FALSE notes

## Agents

| Agent | Role | Assignment | Status (pending, running, done, failed) | Output paths |
| ----- | ---- | ---------- | --------------------------------------- | ------------ |

## Findings

| ID | Title | Status | Severity | Certainty |
| -- | ----- | ------ | -------- | --------- |

## Decisions and open questions

- ...

## Next action

One sentence describing exactly what the Manager does next.
```

Operator compaction text, to be used with `/compact` at a gate:

```text
Keep: the audit phase, the gate protocol, the rule that rust-audit/state/STATE.md overrides memory, the list of running agents. Drop: file contents, agent output, tool logs.
```

## 8. Finding file

Path: `findings/F-<CRATE>-<nnn>.md` with `CRATE` in `CORE`, `VAL`, `SEN`, `ENG`, `XC` (cross-cutting). Sections are appended, never rewritten, so the trail shows how the result was finalised.

```text
# F-VAL-001 <title>

| Field                | Value                                                                          |
| -------------------- | ------------------------------------------------------------------------------ |
| Status               | Draft, Critiqued, QA-done, or Final: Accepted, Rejected, Unverified             |
| Crate and module     | validator, secrets/nonces.rs                                                   |
| Location             | crates/validator/src/secrets/nonces.rs:120-141 (related: ...)                  |
| Severity             | <reviewer> / <final>                                                           |
| Certainty            | <n>% (set by the Critic; QA may raise)                                         |
| Assumptions involved | A2, A5                                                                         |
| Tags                 | crypto, reorg, dos, crash-consistency, input-validation, deps, config, known   |

## Claim

What is wrong and what an attacker or failure achieves.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| - | ----- | ----------------- | -------- | -------------- |

## Trigger

The concrete input, state, or event sequence that reaches the defect, or "none identified".

## Considered and rejected

Alternative explanations, guards checked (with citations), and why this is not a false positive.

## Remediation options

1. Option, tradeoffs.
2. Option, tradeoffs.

Tests to add. No code is committed.

## Trail

- <date> Reviewer R3: drafted, self-estimate <n>%

## Critic (<agent>)

Per-claim verdicts, finding verdict, certainty, counter-evidence.

## QA (<agent>)

Reproduced, Not reproduced, or Not attempted; commands; output path; remediation check.
```

Certainty rubric (the Critic sets it; QA may raise it into the top band):

| Band      | Requirement                                                  |
| --------- | ------------------------------------------------------------ |
| 90 to 100 | `E1` reproduction and Critic Confirmed                       |
| 70 to 89  | `E2` and Critic Confirmed                                    |
| 40 to 69  | `E2` and Critic Plausible, or `I` with a Confirmed mechanism |
| below 40  | Not a finding; listed under unverified observations          |
| 0         | Refuted or Unsupported; kept in the rejected list            |

Severity for this system:

- Critical: leaks or allows recovery of FROST key shares, signing nonces, or signer keys; nonce reuse; an invalid attestation; an epoch rollover into a dishonest set; a malicious Safe transaction rated `secure`; loss of bonded funds at scale.
- High: an honest validator or sentinel loses liveness (stalls, exits, is excluded) under attacker-controlled input or reorgs within `max_reorg_depth`; unbounded fund drain through gas or bonds; engine denial of service from a single request; wrong votes on honest transactions at scale.
- Medium: incorrect behaviour under unusual but reachable conditions; crash-consistency gaps with recoverable impact; missing validation with contained impact.
- Low: robustness, error handling, or configuration weaknesses with limited impact.
- Informational: hardening, documentation, test gaps, and `known` items.

## 9. Report

`report/REPORT.md` contains, in order: header (commit, dates, runtime, mode, agent roster); assumptions as confirmed with any `FALSE` notes; a summary table (ID, title, crate, severity, certainty, final status, basis classes, link); findings grouped by severity, each with claim, trigger, remediation, and a one-line finalisation trail such as "reviewer E2, Critic Confirmed, QA Reproduced"; unverified observations with their percentages; rejected findings with reasons and the count of `H` claims; the coverage matrix; the Phase 0 baseline; recommended next steps.

## 10. Workflow variant (optional)

If the operator says "use a workflow" or "ultracode", run each phase as its own Workflow with `phase()` blocks, one `agent()` per role instance, the same file outputs, and the same gate between phases. Note that a stopped workflow can only be resumed within the same session, so the file-based state in Section 7 remains authoritative.

## 11. Run 2 addendum

A second, independent execution of this prompt with a different model (Claude Fable 5.1), on the same commit of the audited code. Everything in Sections 1–9 applies with these changes:

- **Scope.** `crates/sentinel-engine` is **out of scope**: it is a reference implementation used for testing and is not required to be in final form. No reviewer, Critic or QA agent reads it or files against it, and every prior artefact about it has been removed from `rust-audit/`. Findings are allowed only against `crates/core`, `crates/validator`, `crates/sentinel`, the workspace manifests, and those crates' Dockerfiles and sample configs. `openapi.yaml`, `sentinel-engine.sample.toml` and `crates/sentinel-engine/Dockerfile` leave the inventory. Assumptions A16 and A17 above are in force; A8 (the engine test-vector corpus) is moot.
- **Independence.** Run-2 agents must **not** read run-1 findings (`findings/F-*.md`), run-1 reports (`report/*`) or run-1 state (`state/*.md` outside `state/run2/`). The codebase map and analyses remain available as leads, exactly as in run 1. Run 2 is a second opinion; reconciliation with run 1 is a separate final phase performed by a Manager-directed agent that reads both.
- **Naming.** Run-2 findings are `findings/F2-<CRATE>-<nnn>.md` with `CRATE` in `CORE`, `VAL`, `SEN`, `XC`; PoCs under `poc/F2-<id>/`; state under `state/run2/` (`STATE.md`, `baseline.md`, `coverage.md`, `agents/`, `logs/`). Same file template as Section 8.
- **Reconciliation phase (after Phase 4).** One agent maps every `F2-*` finding to run 1: **confirms** an existing finding (link it, note any severity/certainty disagreement), **contradicts** one (record both positions and the evidence), or is **genuinely new**. Run-1 findings that run 2 did not rediscover are listed, since a miss by an independent reviewer is itself information.
- **Delivery.** Work lands as incremental commits on the existing PR — never a force push, never a commit outside `rust-audit/`, no co-author trailer, Prettier-clean and CI-verified before each push. `report/REPORT.md` is rewritten for the combined result and committed **last, on its own**, so reviewers can diff it in isolation.
