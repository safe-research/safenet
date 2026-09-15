# Audit state (run 2)

| Field | Value |
| --- | --- |
| Commit | `3ec8bc57dc35d1e9e65075ae9424bff427c47833` (merge of `origin/main` `5cc096e` into `audit/rust-services`) |
| Started | 2026-09-15 |
| Mode | full |
| Phase | 1 (Reviewers R1–R8, running) |
| Gate status | Gate 0 passed (`continue`); Gate 1 pending |

Run 2 of PROMPT.md (Section 11): `crates/sentinel-engine` out of scope; findings are `findings/F2-<CRATE>-<nnn>.md` with `CRATE` in `CORE`, `VAL`, `SEN`, `XC`; PoCs under `poc/F2-<id>/`; state under `state/run2/`. Run-2 agents do not read `findings/F-*.md`, `report/*`, or `state/*.md` outside `state/run2/`.

## Assumptions confirmed

| ID | Status | Note |
| --- | --- | --- |
| A1 | [x] | Trusted operator. |
| A2 | [x] | Adversarial chain data within the fault bound. |
| A3 | [x] | Engine deployment assumption; the engine itself is out of scope in run 2, so it only informs how the sentinel's engine client is judged. |
| A4 | [x] | RPC trusted for liveness/eventual correctness; may be stale, rate limited, incomplete `eth_getLogs`. |
| A5 | [x] | Reorgs up to `max_reorg_depth` handled; deeper reorgs exit deliberately. |
| A6 | [x] | `frost-core`, `frost-secp256k1`, `k256`, `sha2`, `hkdf`, `alloy`, `sqlx` trusted; review usage only. |
| A7 | [x] | Solidity contracts are the reference. |
| A8 | moot | Engine test-vector corpus not needed: `crates/sentinel-engine` is out of scope (Section 11). |
| A9 | TRUE | With versions: rustc/cargo 1.98.1 stable (`stable-aarch64-unknown-linux-gnu`), Foundry 1.8.1 (newer than the 1.5.1 named), just 1.40.0, jq 1.8.1, cargo-audit 0.22.2, 11 GiB RAM, 75 G free disk. `baseline.md` Section 1. |
| A10 | [x] | Gnosis Chain parameters and defaults as listed. |
| A11 | [x] | Scope is Section 4 as amended by Section 11 (no engine). |
| A12 | [x] | Known items reported tagged `known`; the engine rows of map Section 4 no longer apply. |
| A13 | [x] | No branches or commits by agents; fixes proposed in finding files only. |
| A14 | [x] | Tree at `3ec8bc5` does not change during the run (a further merge restarts Phase 0). |
| A15 | [x] | Charter text availability concerned engine verdict-policy leads only; not exercised in run 2. |
| A16 | [x] | (run 2) Genesis need not be recoverable; genesis-only liveness findings are Informational and `known`. |
| A17 | [x] | (run 2) Only the services access their databases; out-of-band DB triggers are out of scope. |

## Agents

| Agent | Role | Assignment | Status | Output paths |
| --- | --- | --- | --- | --- |
| Recon | Recon | Toolchain, five baseline commands, inventory, reviewer split, drift | done | `state/run2/baseline.md`, `state/run2/logs/00-toolchain.log` … `08-future-incompat.log`, this file |
| R1 | Reviewer | core indexing and reorgs — 6 files, 4,106 lines (`baseline.md` Section 4) | pending | `state/run2/agents/R1.md`, `findings/F2-CORE-*.md` |
| R2 | Reviewer | core runtime, state, effects, observability — 12 files, 2,055 lines | pending | `state/run2/agents/R2.md`, `findings/F2-CORE-*.md` |
| R3 | Reviewer | core transaction queue — 5 files, 1,540 lines | pending | `state/run2/agents/R3.md`, `findings/F2-CORE-*.md` |
| R4 | Reviewer | validator DKG path — 10 files, 3,228 lines | pending | `state/run2/agents/R4.md`, `findings/F2-VAL-*.md` |
| R5 | Reviewer | validator signing path and secrets — 10 files, 3,329 lines (+522 unmapped in `secrets/store.rs`) | pending | `state/run2/agents/R5.md`, `findings/F2-VAL-*.md` |
| R6 | Reviewer | validator service, wiring, config — 8 `.rs` + sample config + Dockerfile, 2,285 lines | pending | `state/run2/agents/R6.md`, `findings/F2-VAL-*.md` |
| R7 | Reviewer | sentinel — 10 `.rs` + sample config + Dockerfile, 3,971 lines (+524 unmapped in `service.rs`/`state.rs`) | pending | `state/run2/agents/R7.md`, `findings/F2-SEN-*.md` |
| R10 | Reviewer | cross-cutting: manifests, `Cargo.lock`, `cargo audit`/`tree -d` results, secrets/panic/cast censuses, CI gaps (no engine) | pending | `state/run2/agents/R10.md`, `findings/F2-XC-*.md` |
| C-CORE | Critic | findings from R1–R3 | pending | Critic sections in finding files |
| C-VAL | Critic | findings from R4–R6 | pending | Critic sections in finding files |
| C-SEN | Critic | findings from R7 | pending | Critic sections in finding files |
| C-XC | Critic | findings from R10 | pending | Critic sections in finding files |
| Coverage Critic | Coverage Critic | every in-scope file vs reviewer logs | pending | `state/run2/coverage.md` |
| QA | QA | one per crate with a Confirmed/Plausible finding | pending | `poc/F2-*/`, QA sections |
| Documentation | Documentation | combined report after reconciliation | pending | `report/REPORT.md` |

## Findings

| ID  | Title | Status | Severity | Certainty |
| --- | ----- | ------ | -------- | --------- |

(none yet — Phase 0)

## Decisions and open questions

- Phase 0 baseline: build 0, test 0 (183 in-scope tests pass; 280 workspace-wide), clippy `-D warnings` 0, `cargo audit` 1 (5 vulnerabilities: `crossbeam-epoch` 0.9.18, `h2` 0.4.14, `quinn-proto` 0.11.14, `ruint` 1.18.0, `rustls` 0.23.40; 11 warnings), `cargo tree -d` 0 (14 version splits). Details and verbatim advisories in `baseline.md` Section 2; reachability is R10's task.
- Inventory: 61 `.rs` files, 20,301 lines, 183 tests in scope (map: 19,090 lines, 169 tests). Eleven files drifted from the map — exactly the files changed since run 1 (`baseline.md` Sections 3.4 and 5); map line numbers in those files are stale.
- The audited `crates/`, `Cargo.toml` and `Cargo.lock` are byte-identical to `origin/main` `5cc096e`; the dependency set is unchanged since run 1 (`Cargo.lock` unchanged since `2893917`).
- Branch HEAD is now `4ac6838` (commits `1f3fe52`, `4ac6838` landed during Phase 0, touching only `rust-audit/`). `crates/`, `Cargo.toml`, `Cargo.lock` are identical to `3ec8bc5` (tree/blob hashes match; `baseline.md` Section 5), so the Commit field stays `3ec8bc5` and A14 holds. Logs 03/04/05/08 carry `4ac6838` in their headers for this reason.
- Reviewer numbering keeps the map's labels: R1–R7 plus R10; R8/R9 are not launched (engine). Suggested watch: R7 and R5 carry the unmapped new code and may need splitting.
- Toolchain notes: `sqlite3` CLI absent; Foundry is 1.8.1 rather than the 1.5.1 written in A9; every cargo command needs `CARGO_TARGET_DIR=/home/shebin.guest/.cache/safenet-run2/target CARGO_BUILD_JOBS=2` and the PATH prefix; `/tmp` is a 6 G tmpfs — never build or log there.

## Next action

Manager reads `state/run2/baseline.md`, prints the Phase 0 gate summary, asks the operator for `continue`, and on `continue` launches R1–R7 and R10 with the assignments in `baseline.md` Section 4.

## Manager log

- **Gate 0 — `continue`** (operator, after `/usage`). Phase 0 substantively complete when asked: build 0, tests 280/0, inventory 61 files / 20,301 lines, A1–A17 confirmed (A8 moot, A9 true with versions).
- **Reviewer split in force:** `state/run2/reviewer-split.md` (Manager's assignment, 61 `.rs` files each under exactly one reviewer; sums verified). Recon's own proposal in `baseline.md` counts 70 files because it includes the 9 non-Rust in-scope files, and names the cross-cutting reviewer **R10** where the launched agent is **R8** — same scope, different label. The Coverage Critic verifies against `reviewer-split.md`.
- **Phase 1 launched** with the eight reviewers below, each told not to wait for `baseline.md` (it landed shortly after) and not to read run-1 material.

| Agent | Role | Assignment | Status | Output |
| --- | --- | --- | --- | --- |
| Recon | Recon | baseline, inventory, drift, five cargo commands | done | `state/run2/baseline.md`, `logs/00–08` |
| R1 | Reviewer | core indexing + reorgs (4,106) | running | `agents/R1.md`, `F2-CORE-001..029` |
| R2 | Reviewer | core runtime/state/effects/observability (2,055) | running | `agents/R2.md`, `F2-CORE-030..059` |
| R3 | Reviewer | core tx queue (1,540) | running | `agents/R3.md`, `F2-CORE-060..089` |
| R4 | Reviewer | validator DKG (3,228) | running | `agents/R4.md`, `F2-VAL-001..029` |
| R5 | Reviewer | validator signing + secret store incl. pruning (3,329) | running | `agents/R5.md`, `F2-VAL-030..059` |
| R6 | Reviewer | validator service/wiring/config (2,171) | running | `agents/R6.md`, `F2-VAL-060..089` |
| R7 | Reviewer | sentinel, whole crate (3,872) | running | `agents/R7.md`, `F2-SEN-001..049` |
| R8 | Reviewer | cross-cutting: manifests, sweeps, advisories, CI | running | `agents/R8.md`, `F2-XC-001..049` |

## Next action

Wait for R1–R8; record each completion above; then hold **Gate 1** (findings by status, files written, tree clean) and launch Phase 2 Critics per `reviewer-split.md` §"Critics and QA".
