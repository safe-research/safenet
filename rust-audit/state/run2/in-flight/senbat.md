# In-flight assessment — MetaTransaction / Proposal / CallCoverage stack (`feat/senbat_1` … `senbat_7c`, PRs #947 → #958)

**Assessor:** IF-SENBAT. **Audit HEAD:** `cfabcaa` (`crates/` equal to `origin/main` `8b6a75d`). **Method:** read-only — `git diff origin/main...origin/feat/senbat_N`, `git show origin/feat/senbat_N:<path>`, and the legacy no-write form of `git merge-tree`. No branch was checked out, merged, rebased, fetched or modified. This document assesses **unmerged** code; nothing here is a finding against `main`, and no finding file or report was edited.

## 1. Verdict in one paragraph

The stack changes **no in-scope file**. Every one of its 19 changed files is under `crates/sentinel-engine/` (out of the audit's scope) or is the epic `epics/2026_09_04_sentinel_batch_meta_transactions.md`. `crates/core`, `crates/sentinel`, `crates/validator`, `Cargo.toml`, `Cargo.lock` and `contracts/src` are byte-identical between the stack's merge-base and its tip. The sentinel ↔ engine wire contract (`POST /v1/security-check`, `{block, transaction}` in, `{"verdict": "secure" | "insecure" + "rule" | "abstain"}` out) is untouched on both sides: the sentinel's `Request`/`Response` types (`crates/sentinel/src/engine.rs:59-72` on `main`) and the engine's `Verdict` enum (`crates/sentinel-engine/src/engine/mod.rs:38-48`, `api/mod.rs`) do not appear in the diff, and the epic states the same ("The wire contract does not change. `openapi.yaml` and `crates/sentinel` are untouched"). Consequently **every ledger row is Unrelated**: nothing is resolved, partially resolved, changed, worsened or introduced in scope. The only things worth the team's attention are (a) the stack's base is two commits behind `main` and does not contain PR #914, and (b) one indirect, engine-behaviour interaction with `F-SEN-015` (variant 1) that the audit already classifies as class `I`, restated in Section 5 for completeness.

## 2. Stack summary and base

| Step | Branch | Head | Commit title | Merge-base with `origin/main` |
| --- | --- | --- | --- | --- |
| 1 | `feat/senbat_1` | `e7187e7` | [Phase 1] Introduce MetaTransaction struct | `bfecf1d` |
| 2 | `feat/senbat_2` | `3dc5746` | [Phase 2] Adapt the remaining batch consumers | `bfecf1d` |
| 3 | `feat/senbat_3` | `afd5826` | [Phase 3] Introduce Proposal to expose decoded meta transactions to checks | `bfecf1d` |
| 4 | `feat/senbat_4` | `2663481` | [Phase 4] BaseChecker evaluates calls, and cites the right rule | `bfecf1d` |
| 5 | `feat/senbat_5` | `6591fe3` | [Phase 5] Adjust BlocklistChecker and ExcessiveApprovalChecker | `bfecf1d` |
| 6 | `feat/senbat_6` | `297f0c1` | [Phase 6] Use Proposal in remaining checks | `bfecf1d` |
| 7a | `feat/senbat_7a` | `26260ee` | [Phase 7a] Add new CallCoverage | `bfecf1d` |
| 7b (tests) | `feat/senbat_7b_test` | `2d44f74` | [Phase 7b] Part 1: Migrate tests and proposal cov utils | `bfecf1d` |
| 7b (checks) | `feat/senbat_7b_checks` | `1650090` | [Phase 7b] Part 2: Migrate Checks | `bfecf1d` |
| 7c | `feat/senbat_7c` | `2aa2a2f` | [Phase 7c] Remove legacy code and rename | `bfecf1d` |

**Base status: not current.** Every step's merge-base with `origin/main` is `bfecf1d` ("[Phase 7] Coverage metric (#928)"). `origin/main` `8b6a75d` is two commits ahead: `add6f02` "Add Sentinel Oracle audit (#950)" (a PDF plus `contracts/audits/audit.md`) and `8b6a75d` "Adjust deadline handling for quicker reaction (#914)" (`crates/sentinel/src/service.rs`, +198 lines). **The stack does not include PR #914.** Because the stack touches neither file, a no-write `git merge-tree <merge-base> origin/main origin/feat/senbat_7c` reports **zero conflict markers**: the stack merges cleanly and the merged tree's in-scope paths are exactly `main`'s. Nothing in this stack therefore changes the ground the sentinel findings were re-validated on at `fe9e84c`.

**Stack hygiene (out of scope, noted for the operator):** `feat/senbat_7b_test`'s head `2d44f74` is _not_ the commit that `feat/senbat_7b_checks` and `feat/senbat_7c` build on (`3a9cc44`, same commit message). The two differ by 3 engine files / 6+ 17− (`3a9cc44` still carries the `#![allow(dead_code)]` bridge in `engine/coverage.rs` and a `#[allow(dead_code)]` on `impl Proposal` in `engine/proposal.rs`, and re-exports `AspectSet, CallCoverage`; `2d44f74` has them removed). Whichever of the two the 7b-tests PR points at, its parent PR in the chain will show a stale diff until the branch is re-pushed. No in-scope consequence.

## 3. Per-step in-scope file list

Command run for each step: `git diff --stat origin/main...origin/feat/senbat_N -- crates/core crates/sentinel crates/validator Cargo.toml Cargo.lock contracts/src`.

| Step      | In-scope files changed |
| --------- | ---------------------- |
| 1         | none                   |
| 2         | none                   |
| 3         | none                   |
| 4         | none                   |
| 5         | none                   |
| 6         | none                   |
| 7a        | none                   |
| 7b_test   | none                   |
| 7b_checks | none                   |
| 7c        | none                   |

`git log --name-only origin/main..origin/feat/senbat_7c -- <in-scope paths>` is also empty. The only way to make an in-scope path appear is the two-dot form `git diff origin/main origin/feat/senbat_1 -- crates/sentinel` (−193 lines in `service.rs`), which merely shows that `main` gained #914 after the stack branched; it is not a stack change.

For the record, the full changed set of the stack (all 19 files, 1353+ / 946−) is: `crates/sentinel-engine/src/checkers/{address_poisoning,base,blocklist,cancellation,cow,escape_hatch,excessive_approval,mod,nested,refund,staking}.rs`, `crates/sentinel-engine/src/contracts/{multi_send,target_effects}.rs`, `crates/sentinel-engine/src/engine/{coverage,mod,proposal,transaction}.rs`, `crates/sentinel-engine/src/metrics.rs`, `epics/2026_09_04_sentinel_batch_meta_transactions.md`.

## 4. What the engine side does (read for boundary effects only)

Read in full because the brief asks specifically about decoding, hashing, what the sentinel sends to the engine and what it accepts back. None of it crosses the boundary:

- `engine/transaction.rs` adds `MetaTransaction { to, value, data, operation }` and `SafeTransaction::as_meta_transaction()`; `SafeTransaction` (the type the sentinel's JSON is deserialised into) is unchanged in fields and serde.
- `engine/proposal.rs` (new) adds `Proposal { transaction, calls }` and `parse()`, which flattens a recognised MultiSend batch recursively with `MAX_BATCH_DEPTH = 4` (`proposal.rs:20`, `:75-78`, `:105`); deeper nesting is a `ParseError`.
- `engine/mod.rs:85-90`: `security_check` now calls `proposal::parse` first and returns `Verdict::Abstain` if it fails; checkers receive `&Proposal` instead of `&SafeTransaction`. `Verdict` itself and its serde tags are not in the diff.
- `contracts/multi_send.rs`: `decode_multi_send` returns `Vec<MetaTransaction>`; `sub_transactions` is deleted. `contracts/target_effects.rs` stops recursing (the engine has already flattened).
- `checkers/base.rs`: a batch's failing sub-call is now cited under its own rule (`R-4.1` where `main` cites `R-4.2`; test `cites_the_failing_call_s_own_rule_within_a_batch`, `base.rs:328`). `RuleId`'s wire form (`"R-<section>.<rule>"`, parsed by `crates/sentinel/src/engine.rs:25-30`) is unchanged; `engine/rule.rs` is not in the diff.
- `engine/coverage.rs`: per-call `Coverage` (`Vec<AspectSet> + refund`), engine-internal.

The sentinel does not decode meta-transactions anywhere (`grep -ri 'multisend\|multi_send' crates/sentinel/src` is empty); `crates/sentinel/src/hashing.rs` hashes the `SafeTransaction` struct and the commitment, not sub-calls. The sentinel does not depend on the `sentinel-engine` crate (`crates/sentinel/Cargo.toml` dependencies: `alloy, argh, metrics, reqwest, safenet-core, serde, serde_json, sqlx, thiserror, tokio, toml, tracing, url`). So no ABI, binding, hashing or aggregation path in `crates/sentinel` can be affected by the stack.

## 5. Ledger findings versus the stack

Every live sentinel row of `RECONCILIATION.md` §2 and `REPORT.md` §5–§6 is listed; the core rows that carry a sentinel leg are listed once. "Evidence" cites the branch where the branch is relevant and `main` (`8b6a75d`) where the point is that the anchor is untouched — for every in-scope anchor the two are the same bytes.

| Finding | Relation | Evidence (path:line on the branch) | Action needed |
| --- | --- | --- | --- |
| `F-SEN-001` / `F2-SEN-001` (a, b) — replay discards own `Committed`, bond slashed | Unrelated | `crates/sentinel/src/service.rs:307-319`, `413-417` identical on `feat/senbat_7c` and `main`; stack has no `crates/sentinel` hunk | none from this stack; fix still unassigned |
| `F-SEN-002` / `F2-SEN-002` — commits before the verdict not tallied, early finalize | Unrelated | `service.rs:307-319`, `372-384`, `626-633` untouched; engine latency profile is the only engine-side input and `parse()` is O(calldata) | none |
| `F2-SEN-010` — commits in the deadline block never tallied (#914) | Unrelated (base note) | the stack's base `bfecf1d` predates #914, so `service.rs:423-425`, `456-466` on the branch are the pre-#914 text; after a clean merge they are `main`'s | none; the finding is against `main`, not the stack |
| `F-SEN-015` / `F2-SEN-001` (c) — replayed engine check re-decides a committed vote | Unrelated (context) | sentinel mechanism `service.rs:150-194`, `198-244` untouched; `reason` is still `rule.to_string()` at `service.rs:175`. Engine side: `checkers/base.rs:83`, `:95` now cite `R4_1SettingsChange` for a batch sub-call where `main` cites `R4_2DelegatecallIntegrity`; `engine/mod.rs:85-90` adds an `Abstain` for batches nested > 4 | none in scope. Note for the fix owner: deploying this stack while a request is in `WaitingForEngineCheck`/committed and a replay follows is a concrete instance of variant 1's "rule list updated during the deploy" trigger (the finding's basis 11, class `I`). Option 1 (read `getCommitment` before acting on a verdict) covers it |
| `F2-SEN-003` — `finalize()` drops a bonded entry when own reveal not observed | Unrelated | `service.rs:631-637`, `347-362`, `438-447` untouched | none |
| `F-SEN-003` — warp delivers no `NewBlock` | Unrelated | `service.rs:347-362`, `450-465`, `626-671` untouched | none |
| `F-SEN-004` / `F2-SEN-005` — no bound on concurrent engine checks or bonds | Unrelated | `service.rs:137-144`, `181-183`, `226-242` untouched; engine per-request cost changes by a bounded parse (depth ≤ 4) | none |
| `F-SEN-005` / `F2-SEN-004` — waiting states never expire, no `timeoutArbitration` | Unrelated | `service.rs:466-467`, `723-752`; `bindings.rs:42`, `50-60` untouched | none |
| `F2-SEN-011` — arbitration-timeout doc/metric assume full refund | Unrelated | `service.rs:550-557`, `585`, `760-768` untouched | none |
| `F-SEN-006` / `F2-SEN-007` — actions not idempotent under replay | Unrelated | `service.rs:226-242`, `426-437`, `519-528`, `545-575`, `635-645` untouched | none |
| `F-SEN-007` / `F2-SEN-006` — no pre-flight, per-request gas burn | Unrelated | `main.rs:37-86`, `service.rs:226-242`, `820-848` untouched | none |
| `F-SEN-008` — hard-coded gas limits, unconditional `approve` | Unrelated | `service.rs:676-740` untouched | none |
| `F-SEN-009` — engine timeout from unvalidated `voting_window` | Unrelated | `main.rs:45-62`, `config.rs:41-59` untouched; `x-request-timeout` header logic `engine.rs:124-130` untouched | none |
| `F-SEN-010` / `F2-SEN-009` b1 — sample config zero addresses / well-known key | Unrelated | `config.rs`, `sentinel.sample.toml` untouched | none |
| `F-SEN-011` — restart orphans an in-flight check older than the anchor | Unrelated | `state.rs:25-32`, `effect.rs:17-26`, `service.rs:393-399` untouched | none |
| `F-SEN-012` / `F2-SEN-008` — engine client: single attempt, default `reqwest`, unbounded response | Unrelated | `engine.rs:104-115`, `132-157` untouched; `Request`/`Response` serde `engine.rs:59-72` unchanged and the engine's `Verdict` (`engine/mod.rs:38-48`) not in the diff | none. Claim 6 (unbounded response) is about the client's read, not the engine's payload, and the payload shape is unchanged |
| `F-SEN-014` — every participant submits `finalize` | Unrelated | `service.rs:635-641` untouched | none |
| `F-SEN-016` D2–D4 — forward-looking on #915 | Unrelated | the stack neither includes nor touches `feat/optimistic_block_transition` | none |
| `F-SEN-013` (Refuted) — non-UTF-8 `reason` | Unrelated | `RuleId` wire form unchanged; `reason` is a parsed rule code, never engine free text | none |
| `F-CORE-031` / `F2-CORE-030`, `F-CORE-033` / `F2-CORE-035`, `F-CORE-067` / `F2-CORE-063` (sentinel legs) | Unrelated | no `crates/core` hunk in the stack | none |
| `F-XC-008` item 1, `F-XC-011` (engine `reqwest` / `h2` — already out of scope) | Unrelated | `Cargo.lock` unchanged; no engine dependency change (no `Cargo.toml` hunk anywhere in the stack) | none |
| All remaining `F-CORE-*`, `F2-CORE-*`, `F-VAL-*`, `F2-VAL-*`, `F-XC-*`, `F2-XC-*` rows | Unrelated | no `crates/core`, `crates/validator`, `Cargo.*` or `contracts/src` hunk | none |

Tally: Resolves 0, Partially resolves 0, Changes 0, Worsens / introduces 0, Unrelated: all rows.

**New in-scope defects introduced by the stack:** none. There is no in-scope code to introduce one in.

**Engine-side observations (out of scope, not findings):** (1) the new `MAX_BATCH_DEPTH = 4` bound replaces `main`'s unbounded recursion in `target_effects.rs::decode_target_effects` with a hard cap and an `Abstain`; an `Abstain` reaches the sentinel as `CheckOutcome::Unknown` and is dropped at `service.rs:176-179` exactly like any other abstention, so the sentinel-visible behaviour for such a batch is "no vote", as before for anything the engine would not affirm. (2) `BaseChecker`'s corrected rule citation changes the `reason` string the sentinel commits to for one class of batch (`R-4.1` instead of `R-4.2`); this is the intended behaviour change of Phase 4 and matters to the sentinel only through the `F-SEN-015` replay path noted above.

## 6. PoCs

Not re-run. Rationale: the audit's sentinel PoCs (`rust-audit/poc/F-SEN-001`, `F-SEN-002`, `F-SEN-015`, `rust-audit/poc/F2-SEN-001` … `F2-SEN-010`) drive `crates/sentinel` and `crates/core` against Anvil / forge, and those crates are byte-identical between the stack's tip and its merge-base; the merged result's in-scope tree equals `main`'s (clean merge, no in-scope hunk). A re-run on the stack would therefore reproduce either the `fe9e84c` results (against a merge with `main`) or the pre-#914 state (against the raw branch, which would make `F2-SEN-010`'s PoC not apply for the trivial reason that #914 is absent) — neither says anything about the stack. Building the branch's engine crate is out of scope and was not done.

## 7. Actions for the team

Before merge:

1. **Rebase or merge `origin/main` `8b6a75d` into `feat/senbat_1`** (and re-push the chain) so the stack's CI runs against the post-#914 sentinel and the reviewers see the true diff. The merge is conflict-free, so this is mechanical.
2. **Re-push `feat/senbat_7b_test`** to the commit `7b_checks` actually builds on (`3a9cc44`) or vice versa, so the PR chain is consistent.
3. No change to `crates/sentinel`, `crates/core`, `crates/validator`, `contracts/src` or the lockfile is requested by this assessment.

After merge (verify, all owned by the `F-SEN-015` fix, not by this stack):

4. Roll the engine out in a window with no in-flight sentinel requests, or land `F-SEN-015` option 1 (`getCommitment` read before acting on a replayed verdict) first — the rule-citation change in Phase 4 is a concrete "engine answers differently after a deploy" event for any request that gets re-checked across the deploy.
5. The audit's sentinel rows and their `fe9e84c` re-validation remain valid verbatim; no finding file needs an update because of this stack.
