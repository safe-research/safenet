> **Scope note (run 2):** `crates/sentinel-engine` is out of scope (PROMPT.md Section 11). Its files, reviewers R8/R9, findings and seeded leads were removed from the tables below; any prose that still mentions the engine describes run-1 history only.

# Coverage matrix and gap report — Coverage Critic

| Field | Value |
| --- | --- |
| Agent | Coverage Critic (Phase 2) |
| Commit | `2893917` (branch `rust-audit`), verified unchanged during this run |
| Mode | READ-ONLY. No toolchain (A9 FALSE): no `cargo`, `rustc`, `forge`, `anvil`, `just`, no network, no dependency source on disk. `E1` is unreachable, so **no finding in this run may be certified above 89%**. |
| Snapshot | , taken with **101 finding files**, of which **89 carry a `## Critic` section**. C-CORE-A, C-CORE-B, C-VAL-A, C-SEN, C-ENG-A and C-ENG-B are complete; **C-VAL-B and C-XC were still running** when this was written, so a Critic column reading "in progress" is a snapshot artefact, not an uncovered finding. |

## 0. The denominator

**83 in-scope `.rs` files, 24,203 lines.** Re-measured this session with `find crates -name '*.rs' -type f | sort | xargs wc -l`: 83 files, 24,203 lines — identical to `baseline.md` §3 and to `codebase-map.md` §2 row for row.

`codebase-map.md` §10 prose says "all 81 Rust files". That is a **prose miscount**, already resolved in `baseline.md` §3: the map's own §2 tables enumerate 83 rows (23 core + 28 validator + 10 sentinel

- 22 sentinel-engine), the four per-crate analyses sum to 24,203, and the map documents no exclusion anywhere. **The denominator is 83, not 81.** Nothing was skipped because of the miscount — every one of the 83 is assigned and claimed — but any percentage computed against 81 is wrong by two files.

Assignment completeness re-verified this session by reconstructing the reviewer-to-file map from `codebase-map.md` §9 and diffing it against the filesystem: **83 rows, 0 unassigned files, 0 double-assigned files.** Two of the map's §9 line totals are arithmetic slips (R2 is 1,998 not 1,993; R9 is 3,471 not 3,275), already corrected in `baseline.md` §3; neither corresponds to an omitted file.

Also in scope and covered in §2 below: `Cargo.toml`, `Cargo.lock`, the four `crates/*/Cargo.toml`, three Dockerfiles, three `*.sample.toml`, and `crates/sentinel-engine/openapi.yaml`. `crates/core/Dockerfile` does not exist — `core` is a library crate, so PROMPT.md §4's `crates/*/Dockerfile` glob resolves to three files, not four (`baseline.md` §4).

---

## 1. Matrix — every in-scope `.rs` file

Columns:

- **Reviewer** — from `codebase-map.md` §9.
- **Claimed read** — what that reviewer's own coverage log states. All ten logs carry an explicit files-read table and every one claims **100%, tests included**, for every assigned file. Verbatim: R1 "No file left unfinished"; R2 "No assigned file was left unfinished"; R3 per-file "100% (incl. tests …)"; R4 "All ten assigned files read 100%, including tests"; R5 "Total 2,802 lines, 100% covered"; R6 "Every assigned file was read **100%**, including tests"; R7 "**No assigned file was left unfinished**"; R8 "1,642 / 1,642 = 100%"; R9 "**11 of 11 files, 100%**"; R10 "all read 100%" (one caveat, §2). **No reviewer claims a partial read of any `.rs` file**, so there is no partially-claimed `.rs` file to flag — the honest caveat is that these are self-reports, and §3 below is how they were tested.
- **Findings anchored** — findings whose `Location` header names the file (the defect lives here).
- **Also cited** — findings citing the file in a `Basis` row but anchored elsewhere.
- **Critic** — Critic(s) that have appended a `## Critic` section to at least one anchored finding.

| File | Lines | Reviewer | Claimed read | Findings anchored | Also cited | Critic |
| --- | --: | --- | --- | --- | --- | --- |
| `crates/core/src/driver.rs` | 318 | R2 | 100% | F-CORE-004 F-CORE-011 F-CORE-030 F-CORE-031 F-CORE-032 F-CORE-033 F-CORE-034 F-CORE-035 F-CORE-036 F-CORE-039 F-SEN-001 F-SEN-006 F-SEN-013 F-VAL-038 F-VAL-062 F-VAL-064 F-VAL-066 F-XC-002 | F-CORE-002 F-CORE-010 F-CORE-062 F-CORE-063 F-SEN-012 F-VAL-061 F-XC-003 | C-CORE-A C-CORE-B C-SEN C-XC (+5 pending) |
| `crates/core/src/effects.rs` | 220 | R2 | 100% | F-CORE-031 F-CORE-032 F-CORE-033 F-CORE-036 F-SEN-004 F-SEN-011 F-VAL-062 F-XC-002 | F-SEN-012 F-VAL-004 F-VAL-030 F-VAL-035 | C-CORE-B C-SEN C-XC (+1 pending) |
| `crates/core/src/index/blocks.rs` | 1330 | R1 | 100% | F-CORE-001 F-CORE-003 F-CORE-005 F-CORE-007 F-CORE-008 F-CORE-009 F-CORE-010 F-CORE-011 F-CORE-031 F-CORE-034 F-SEN-001 F-SEN-003 F-SEN-011 F-SEN-015 F-XC-001 | F-CORE-004 F-SEN-006 F-SEN-009 F-XC-006 | C-CORE-A C-CORE-B C-SEN C-XC (+3 pending) |
| `crates/core/src/index/bloom.rs` | 523 | R1 | 100% | F-CORE-012 | — | none yet (+1 pending) |
| `crates/core/src/index/clock.rs` | 103 | R1 | 100% | F-CORE-008 | — | C-CORE-A |
| `crates/core/src/index/events.rs` | 1516 | R1 | 100% | F-CORE-002 F-CORE-004 F-CORE-006 F-CORE-010 F-CORE-011 F-CORE-012 F-CORE-033 F-SEN-013 F-VAL-060 | F-CORE-009 F-CORE-066 | C-CORE-A C-CORE-B C-SEN (+4 pending) |
| `crates/core/src/index/mod.rs` | 468 | R1 | 100% | F-CORE-003 F-CORE-004 F-CORE-005 F-CORE-010 F-XC-003 | F-CORE-002 F-CORE-009 | C-CORE-A C-XC (+1 pending) |
| `crates/core/src/kdf.rs` | 81 | R2 | 100% | F-CORE-038 | F-VAL-001 F-VAL-002 F-XC-007 | C-CORE-B |
| `crates/core/src/lib.rs` | 25 | R2 | 100% | **none** | — | none yet |
| `crates/core/src/metrics.rs` | 90 | R2 | 100% | F-CORE-034 F-CORE-035 | F-CORE-004 F-CORE-007 F-CORE-032 F-CORE-060 F-XC-002 | C-CORE-B |
| `crates/core/src/observability/logging.rs` | 21 | R2 | 100% | **none** | F-CORE-034 | none yet |
| `crates/core/src/observability/metrics.rs` | 80 | R2 | 100% | F-CORE-030 F-XC-009 | F-CORE-004 F-CORE-011 | C-CORE-B (+1 pending) |
| `crates/core/src/observability/mod.rs` | 94 | R2 | 100% | F-CORE-007 F-VAL-064 | F-CORE-037 F-VAL-062 F-XC-002 F-XC-009 | C-CORE-A (+1 pending) |
| `crates/core/src/serialization.rs` | 34 | R2 | 100% | **none** | — | none yet |
| `crates/core/src/state/mod.rs` | 644 | R2 | 100% | F-CORE-031 F-CORE-032 F-CORE-033 F-CORE-037 F-SEN-001 F-SEN-003 F-SEN-015 F-VAL-005 F-VAL-030 F-VAL-034 F-VAL-038 | F-CORE-001 F-CORE-002 F-CORE-003 F-CORE-006 F-CORE-012 F-SEN-005 F-SEN-006 F-VAL-004 F-VAL-036 F-VAL-061 F-VAL-066 | C-CORE-B C-SEN C-VAL-A C-VAL-B (+3 pending) |
| `crates/core/src/state/storage.rs` | 294 | R2 | 100% | F-CORE-001 F-CORE-031 F-CORE-037 F-XC-006 | F-SEN-001 F-VAL-004 F-VAL-005 F-VAL-063 F-XC-007 | C-CORE-A C-CORE-B C-XC |
| `crates/core/src/tx/fees.rs` | 109 | R3 | 100% | F-CORE-060 F-CORE-061 F-CORE-066 | — | C-CORE-B |
| `crates/core/src/tx/mod.rs` | 719 | R3 | 100% | F-CORE-035 F-CORE-039 F-CORE-060 F-CORE-061 F-CORE-062 F-CORE-063 F-CORE-064 F-CORE-065 F-CORE-066 F-SEN-004 F-SEN-007 F-XC-007 | F-CORE-033 F-SEN-006 F-SEN-008 F-SEN-014 F-VAL-060 F-VAL-065 | C-CORE-B C-SEN (+1 pending) |
| `crates/core/src/tx/signer.rs` | 118 | R3 | 100% | F-CORE-038 F-XC-002 | F-CORE-063 F-VAL-062 | C-CORE-B C-XC |
| `crates/core/src/tx/storage.rs` | 507 | R3 | 100% | F-CORE-060 F-CORE-061 F-CORE-062 F-CORE-063 F-CORE-064 F-CORE-065 F-SEN-004 F-SEN-006 F-VAL-065 F-XC-006 | F-CORE-003 F-SEN-003 F-SEN-014 | C-CORE-B C-SEN C-XC (+1 pending) |
| `crates/core/src/tx/types.rs` | 87 | R3 | 100% | F-CORE-060 F-CORE-061 | F-CORE-065 | C-CORE-B |
| `crates/sentinel/src/action.rs` | 43 | R7 | 100% | F-SEN-005 | — | C-SEN |
| `crates/sentinel/src/bindings.rs` | 170 | R7 | 100% | F-SEN-005 F-SEN-013 | F-CORE-006 F-SEN-008 | C-SEN |
| `crates/sentinel/src/config.rs` | 144 | R7 | 100% | F-SEN-009 F-SEN-010 F-XC-003 | F-CORE-036 F-XC-006 F-XC-008 F-XC-009 | C-SEN C-XC |
| `crates/sentinel/src/hashing.rs` | 224 | R7 | 100% | F-CORE-038 | F-SEN-010 F-XC-007 | C-CORE-B |
| `crates/sentinel/src/main.rs` | 89 | R7 | 100% | F-CORE-030 F-SEN-007 F-SEN-009 | F-CORE-006 F-CORE-007 F-CORE-011 F-CORE-033 F-SEN-001 F-SEN-010 | C-CORE-B C-SEN |
| `crates/sentinel/src/metrics.rs` | 134 | R7 | 100% | **none** | F-SEN-012 F-XC-002 | none yet |
| `crates/sentinel/src/state.rs` | 167 | R7 | 100% | F-CORE-037 F-SEN-011 | — | C-CORE-B C-SEN |
| `crates/validator/src/bindings.rs` | 247 | R6 | 100% | F-CORE-012 | F-CORE-004 F-CORE-006 | none yet (+1 pending) |
| `crates/validator/src/config.rs` | 290 | R6 | 100% | F-VAL-063 F-XC-003 F-XC-009 | F-CORE-004 F-CORE-006 F-CORE-036 F-CORE-066 F-VAL-035 F-XC-006 | C-XC (+2 pending) |
| `crates/validator/src/consensus/epoch.rs` | 95 | R4 | 100% | **none** | F-VAL-004 F-VAL-063 | none yet |
| `crates/validator/src/consensus/group.rs` | 459 | R4 | 100% | F-VAL-037 F-VAL-063 F-XC-001 | F-XC-009 | C-XC (+2 pending) |
| `crates/validator/src/consensus/hashing.rs` | 249 | R5 | 100% | **none** | — | none yet |
| `crates/validator/src/consensus/mod.rs` | 5 | R4 | 100% | **none** | — | none yet |
| `crates/validator/src/frost/ecdh.rs` | 181 | R4 | 100% | F-VAL-001 F-VAL-002 F-XC-002 | F-VAL-062 F-XC-007 F-XC-051 | C-VAL-A C-XC |
| `crates/validator/src/frost/error.rs` | 46 | R4 | 100% | **none** | F-XC-002 | none yet |
| `crates/validator/src/frost/keygen.rs` | 516 | R4 | 100% | F-CORE-036 F-VAL-001 F-VAL-002 F-VAL-003 F-VAL-005 F-VAL-062 F-XC-002 F-XC-051 | F-VAL-060 | C-CORE-B C-VAL-A C-XC (+2 pending) |
| `crates/validator/src/frost/marshal.rs` | 176 | R4 | 100% | F-XC-051 | — | C-XC in progress |
| `crates/validator/src/frost/mod.rs` | 258 | R4 | 100% | **none** | — | none yet |
| `crates/validator/src/frost/participants.rs` | 33 | R4 | 100% | **none** | — | none yet |
| `crates/validator/src/frost/preprocess.rs` | 189 | R5 | 100% | F-VAL-035 F-VAL-038 F-XC-002 | F-CORE-036 F-VAL-031 F-VAL-034 F-VAL-037 F-VAL-062 | C-XC (+2 pending) |
| `crates/validator/src/frost/sign.rs` | 204 | R5 | 100% | **none** | F-VAL-034 F-VAL-037 | none yet |
| `crates/validator/src/main.rs` | 99 | R6 | 100% | F-CORE-007 F-CORE-030 F-VAL-038 F-VAL-060 F-VAL-064 F-VAL-065 | F-CORE-001 F-CORE-004 F-CORE-006 F-VAL-004 F-VAL-033 F-VAL-034 F-XC-003 F-XC-006 | C-CORE-A C-CORE-B (+4 pending) |
| `crates/validator/src/merkle.rs` | 142 | R5 | 100% | F-VAL-037 | — | C-VAL-B in progress |
| `crates/validator/src/metrics.rs` | 132 | R6 | 100% | F-VAL-061 | F-VAL-004 F-VAL-031 F-XC-002 | C-VAL-B in progress |
| `crates/validator/src/secrets/mod.rs` | 6 | R5 | 100% | **none** | — | none yet |
| `crates/validator/src/secrets/nonces.rs` | 348 | R5 | 100% | F-VAL-030 F-VAL-031 F-VAL-038 | F-CORE-032 F-VAL-061 | C-VAL-B (+1 pending) |
| `crates/validator/src/secrets/store.rs` | 447 | R5 | 100% | F-VAL-005 F-VAL-033 F-VAL-035 F-VAL-038 F-VAL-066 F-XC-006 | F-CORE-037 F-VAL-004 F-VAL-030 F-VAL-034 F-VAL-036 | C-VAL-A C-VAL-B C-XC (+3 pending) |
| `crates/validator/src/service/action.rs` | 381 | R6 | 100% | F-VAL-065 | F-CORE-060 F-CORE-064 F-VAL-003 F-VAL-032 | C-VAL-B in progress |
| `crates/validator/src/service/effect.rs` | 275 | R6 | 100% | F-CORE-036 F-VAL-004 F-VAL-005 F-VAL-030 F-VAL-031 F-VAL-033 F-VAL-034 F-VAL-061 F-VAL-062 F-VAL-066 F-XC-002 | F-CORE-032 F-VAL-035 F-VAL-036 F-VAL-038 | C-CORE-B C-VAL-A C-VAL-B C-XC (+4 pending) |
| `crates/validator/src/service/mod.rs` | 129 | R6 | 100% | F-VAL-060 F-VAL-063 | F-CORE-006 | C-VAL-B in progress |
| `crates/validator/src/state/keygen.rs` | 1459 | R4 | 100% | F-VAL-001 F-VAL-002 F-VAL-003 F-VAL-004 F-VAL-005 F-VAL-060 F-VAL-061 F-VAL-063 F-XC-001 F-XC-050 F-XC-051 | F-VAL-032 | C-VAL-A C-XC (+5 pending) |
| `crates/validator/src/state/mod.rs` | 515 | R6 | 100% | F-CORE-037 F-VAL-030 F-VAL-036 F-VAL-060 F-VAL-061 F-VAL-066 F-XC-050 | F-CORE-006 F-CORE-031 F-VAL-004 F-VAL-005 F-VAL-033 | C-CORE-B C-VAL-B (+5 pending) |
| `crates/validator/src/state/preprocess.rs` | 248 | R5 | 100% | F-VAL-005 F-VAL-030 F-VAL-032 F-VAL-033 F-VAL-036 F-VAL-061 F-VAL-066 | F-VAL-031 | C-VAL-A C-VAL-B (+3 pending) |
| `crates/validator/src/state/sign.rs` | 868 | R5 | 100% | F-VAL-032 F-VAL-034 F-VAL-036 F-VAL-065 | F-CORE-031 F-VAL-033 F-VAL-037 | C-VAL-B (+3 pending) |
| `crates/validator/src/state/transactions.rs` | 101 | R5 | 100% | F-VAL-032 F-VAL-063 | — | C-VAL-B (+1 pending) |

**Rust subtotal: 83 files, 24,203 lines, all assigned, all claimed 100%.**

---

## 2. Matrix — non-Rust in-scope files (all R10)

R10 is the only reviewer with a findings-eligible non-Rust scope. Line counts re-measured this session; all thirteen match `baseline.md` §4 exactly.

| File | Lines | Reviewer | Claimed read | Findings anchored | Also cited | Critic |
| --- | --: | --- | --- | --- | --- | --- |
| `Cargo.lock` | 6169 | R10 | 100% | F-XC-007 | F-SEN-013 F-VAL-062 F-VAL-064 F-XC-002 F-XC-004 | C-XC in progress |
| `crates/core/Cargo.toml` | 31 | R10 | 100% | F-XC-007 | — | C-XC in progress |
| `crates/validator/Cargo.toml` | 28 | R10 | 100% | **none** | F-XC-002 | C-XC in progress |
| `crates/sentinel/Cargo.toml` | 24 | R10 | 100% | **none** | — | C-XC in progress |
| `crates/validator/Dockerfile` | 37 | R10 | 100% | F-VAL-064 F-XC-001 F-XC-004 | — | C-XC |
| `crates/sentinel/Dockerfile` | 38 | R10 | 100% | F-XC-001 F-XC-004 | — | C-XC |
| `crates/validator/validator.sample.toml` | 77 | R10 | 100% | F-VAL-063 F-VAL-064 F-XC-006 F-XC-009 | F-CORE-006 F-CORE-007 F-CORE-060 F-CORE-066 F-XC-003 F-XC-004 | C-XC |
| `crates/sentinel/sentinel.sample.toml` | 61 | R10 | 100% | F-SEN-010 F-XC-006 F-XC-009 | F-CORE-007 | C-SEN C-XC |

**One claimed read is not a line-by-line read, and R10 says so:** `Cargo.lock` (6,169 lines) is logged as "**not line-by-line**; parsed in full with `python3` (package/version/dependency extraction, 573 `[[package]]` blocks) and read directly at the `sqlx` block (`4599-4620`). Stated honestly: I did not read 6,169 lines by eye." That is the correct treatment for a generated lockfile and is recorded here so the report does not overclaim.

`crates/{validator,sentinel,sentinel-engine}/Dockerfile.dockerignore` (4 lines each) are not named by PROMPT.md §4 and are therefore not findings-eligible; R10 read all three alongside the Dockerfiles and recorded what they do not exclude as Observation 1 in its log. Not counted in any denominator.

---

## 3. Files with no coverage — the loud list

**No file in the audit is uncovered in the strict sense.** Every one of the 83 `.rs` files and all thirteen non-Rust files is assigned to exactly one reviewer, and every reviewer log claims 100% on every file it owns. There is no file that no log claims. That is the headline and it is a good result.

The weaker forms of the question are more informative, and this is where the real holes are.

### 3.1 Ten files that no finding cites anywhere — zero findings, zero basis rows

These are the files where a claimed 100% read produced no citation of any kind. They are the candidates for a skim, and they are the files this agent spot-read (§4).

| File | Lines | Reviewer | Verdict after spot-read |
| --- | --: | --- | --- |
| `crates/core/src/index/bloom.rs` (see note) | 523 | R1 | Was on this list; now cited by `F-CORE-012`, which C-CORE-A promoted from R1's own rejected hypothesis 18. **Resolved during Phase 2.** |
| `crates/validator/src/frost/mod.rs` | 258 | R4 | **Read by this agent.** Entirely `mod` declarations (lines 1–16) plus one 234-line happy-path ceremony test. Nothing to find; the observation is that the crate's only end-to-end DKG+signing test exercises **no adversarial input at all**. |
| `crates/validator/src/consensus/hashing.rs` | 249 | R5 | **Read by this agent.** Consensus-critical EIP-712. Checked against Solidity: `EIP712Domain(uint256 chainId,address verifyingContract)` matches `ConsensusMessages.sol:16` and `SafeTransaction.sol:60-62`; `TransactionProposal(uint64 epoch,address oracle,bytes oracleData,bytes32 safeTxHash)` and `EpochRollover(uint64 activeEpoch,uint64 proposedEpoch,uint64 rolloverBlock,uint256 groupKeyX,uint256 groupKeyY)` match the precomputed type hashes at `ConsensusMessages.sol:21,27`; `SafeTx` field order matches `SAFE_TX_TYPEHASH`. The hand-rolled `transaction_proposal_hash` is pinned against alloy's canonical `SolStruct` by `proposal_hash_matches_solstruct`. **Clean — verified, not merely unexamined.** |
| `crates/validator/src/frost/marshal.rs` | 176 | R4 | **Read by this agent. Two gaps found → `F-XC-051`.** |
| `crates/validator/src/frost/participants.rs` | 33 | R4 | Trivial (address→identifier derivation). R4 traced it for M1. No gap. |
| `crates/validator/src/secrets/mod.rs` | 6 | R5 | Module declaration only. |
| `crates/validator/src/consensus/mod.rs` | 5 | R4 | Module declaration only. |
| `crates/core/src/lib.rs` | 25 | R2 | **Read by this agent.** Crate doc plus twelve `pub mod` lines. Only note: `metrics` is the one private module, so `core::metrics` is not part of the public API while `observability::metrics` is — deliberate, matches R2's reading. |
| `crates/core/src/serialization.rs` | 34 | R2 | **Read by this agent.** One `from_str` serde helper. R2's rejected hypothesis 8 already established its three call sites and that none carries a secret. Clean. |

### 3.2 Six more files with no _anchored_ finding (cited only in others' basis rows)

`crates/core/src/observability/logging.rs` (21, R2), `crates/sentinel/src/metrics.rs` (134, R7), `crates/validator/src/consensus/epoch.rs` (95, R4), `crates/validator/src/frost/error.rs` (46, R4), `crates/validator/src/frost/sign.rs` (204, R5), `crates/sentinel-engine/src/engine/transaction.rs` (170, R8).

Spot-read results: `logging.rs` is a 21-line subscriber init, clean. `sentinel/src/metrics.rs` is seven metric accessors, every label a `&'static str` from a closed enum — no cardinality exposure, corroborating R10's repo-wide sweep. `engine/transaction.rs` is clean and unusually well tested: `deny_unknown_fields`, a rejecting `Operation` deserialiser, EIP-55 output with case-insensitive input, and negative tests for all three (`:159-169`).

### 3.3 One file that scored zero on the anchor metric but is genuinely well covered

`crates/sentinel-engine/src/checkers/staking.rs` (183, R9) has **no anchored finding and zero tests in file**, which is exactly the signature of a skim. It is not one: `F-ENG-031` uses `StakingChecker` as its **primary trigger** and cites `staking.rs:88-116` and `:156-161` in two basis rows, and R9's log carries two specific rejected hypotheses about it (native value in sub-calls; summing two `approve` calls), each with citations. Recorded so the report does not mis-flag it. It remains the largest non-trivial file in the workspace with zero tests of its own.

---

## 4. Spot-reads and the findings they produced

Method: pick the three least-covered files per crate by combined weakest evidence (no anchored finding, no basis-row citation, fewest mentions in any log's rejected-hypothesis or observation list), read them in full, and file whatever was missed. Three findings resulted.

| Crate | Files spot-read | Outcome |
| --- | --- | --- |
| `core` | `index/bloom.rs`, `serialization.rs`, `lib.rs` (+ `observability/logging.rs`) | Clean. `may_contain_log` (`bloom.rs:23-35`) has no production caller and the `#[allow(dead_code)]` at `index/mod.rs:5-6` hides that from the compiler; R1 already recorded it as the cheapest remediation for F-CORE-002 and C-CORE-A has since promoted `F-CORE-012` over the same file. No new finding. |
| `validator` | `frost/marshal.rs`, `frost/mod.rs`, `consensus/hashing.rs` | **`F-XC-051`** from `marshal.rs`. `hashing.rs` verified clean against Solidity. `frost/mod.rs` is declarations plus a happy-path test. |
| `sentinel` | `action.rs`, `metrics.rs`, `hashing.rs` | Clean. Every sentinel file already carried at least one citation before this pass — `sentinel` is the best-covered crate in the audit by this metric. `hashing.rs`'s `commit_hash` preimage order matches the documented `SentinelOracleCommitments.computeHash`, and R7 closed M8 over it with a full trace. No new finding. |
| cross-cutting | `state/keygen.rs:445-482` and `:1295-1325`, reached from R4's dangling observations | **`F-XC-050`**. |

### Findings filed

| ID | Title | Sev (drafted) | Certainty (drafted) | Provenance |
| --- | --- | --- | --- | --- |
| `F-XC-050` | No DKG event handler checks group membership, so one injected `KeyGenConfirmed` closes the confirmation round early and silently finalises genesis with no key share | High | 58% | Promotes R4's Observation **O9**, which R4 parked as conditional on VAL-H2 and which R6's `F-VAL-060` left dangling. `F-VAL-060` stays canonical for the injection mechanism; this is the consequence it does not enumerate. Mechanism `E2`, trigger inherits F-VAL-060's unproven precondition → **Plausible**, not Confirmed. |
| `F-XC-051` | `verify_commitment` deliberately delegates the DKG commitment's only structural validation to a contract that is not in the event path, and accepts identity coefficients | Medium | 45% | Promotes R4's Observations **O3** and **O4**, same dangling condition. Mechanism `E2`; both load-bearing consequences (`frost-core` behaviour on an empty `c`; the contract's `require`) are class `I` under A6/A7, which is what caps it. |

All three are Draft, awaiting a Critic. Every citation in them was re-opened in this checkout at commit `2893917`. Nothing was executed.

---

## 5. Seams between reviewers

A split assignment creates a defect each side assumes the other owns. Each seam below is resolved to a finding ID or to **not covered**.

| Seam | Owner(s) | Resolution |
| --- | --- | --- |
| **CORE-H12** — no RPC timeout or retry layer. Code is `provider/mod.rs:127-137`, R1's file; the lead was assigned to R2. R1's log says explicitly: "if R2 does not file it, the Coverage Critic should promote this line." | R1 / R2 | **COVERED, and the seam closed itself during Phase 2.** R2 filed `F-CORE-039` for the shutdown half, whose **basis row 4 quotes `provider/mod.rs:127-137` verbatim** and states "No timeout, retry or rate-limit layer is configured on the provider; the observability layer is the only one" — so the absence _is_ cited. The liveness half — a stalled connection freezing indexing indefinitely with `/health` still OK — was **not** claimed by F-CORE-039, and C-CORE-A promoted it as **`F-CORE-011`**, whose Trail says so in as many words ("Lead CORE-H12 was assigned to R2 … R2 filed the shutdown half"). Between `F-CORE-039` and `F-CORE-011` the lead is fully covered. No action. |
| **CORE-H5** — replay re-queues actions with fresh nonces, no dedup. R2 deferred the duplicate-action half to R3's `tx/`. | R2 / R3 | **NOT COVERED at the core layer.** R2: "I did **not** file a duplicate-action finding: the enqueue/allocate side is `tx/storage.rs:89-104`/`145-156`, explicitly R3's scope." R3, rejected hypothesis 28: "**not mine to file.** `enqueue` is an unconditional `INSERT` with no de-duplication (`tx/storage.rs:96-100`), which I confirm, but the _replay_ half lives in `index/blocks.rs` and `state/mod.rs`." Each confirmed its half and each filed nothing. `F-CORE-031` is the **mirror** defect (effects performed _zero_ times), not this one. Neither C-CORE-A nor C-CORE-B promoted it. **The core-level claim "replay re-enqueues an identical action and the queue offers no de-duplication hook at all" exists in no finding file.** It is covered only at the service layer, by `F-VAL-065` (validator duplicate actions, a duplicate `Sign` burns a nonce sequence for the whole group) and `F-SEN-006` (sentinel `approve`/`commit`/`reveal`/`finalize`/`claim` duplicated on every restart and reorg). Those two carry the impact, so nothing is lost from the report — but the shared root cause has no home, and a fix applied in one service will not fix the other. **Recommended: the Manager assigns the core-layer claim to C-CORE-B or QA, anchored at `tx/storage.rs:96-100` with `index/blocks.rs:255-266` as the replay driver.** |
| **CORE-H13** — wall-clock polling. R2 deferred it entirely to R1's `index/`. | R2 / R1 | **COVERED** by `F-CORE-008` (R1, "Block polling is scheduled by comparing chain timestamps against the host wall clock…"), critiqued by C-CORE-A. R2 recorded the mechanism with citations (`index/clock.rs:31-38`, `blocks.rs:538-551`) and filed nothing to avoid duplicating — correct call, and R1 did file. Landed cleanly. |
| **SEN-H9** — engine has unbounded authority over bond exposure; no local loss budget or kill switch. | R7 → (map §9 leaves it with R7) | **NOT COVERED.** R7 examined it and declined: "Under A3 the engine is a trusted co-deployed component … the residual point (no local loss budget for engine _mistakes_) is a design gap, not a defect, and is not supported by a code-level guard that is missing" (Observation 5.1, cited at `service.rs:173-180`). C-SEN has finished and promoted `F-SEN-015` from a different rejected hypothesis, not this one. No finding mentions a loss budget, kill switch or circuit breaker anywhere in `findings/`. **This agent agrees with R7's substance** — under A3 it is a resilience gap, not a defect — but records that it is a _conscious_ omission resting entirely on A3, so if the team ever marks A3 FALSE this is the first item to revisit. |
| **VAL-H7** — `ReconcileGroupSecrets` computes its retention set from pre-log state and races the same block's store writes. R4's log says it was left to R6. | R4 → R6 | **COVERED** by `F-VAL-066` (R6, "`ReconcileGroupSecrets` deletes from a retention set computed before the block's logs, and runs concurrently with the store writes those logs cause"). R4 recorded the effect-emission sites it could see (`state/keygen.rs:1149-1153`, `:1320`, `:1352-1355`) as Observation O2 and said "I could not construct a deterministic interleaving from my files alone. Left to R6." R6 constructed it. Textbook hand-off. |

### 5.1 R4's four observations left conditional on R6's VAL-H2 — **two landed, two are still dangling**

R4 parked four observations as explicitly conditional on VAL-H2 being real. R6 filed VAL-H2 as `F-VAL-060` and did not pick any of them up; C-VAL-A promoted `F-VAL-005` from R4's log but from **M1/O2**, not from these. The condition is met. Status:

| R4 obs | Substance | Status |
| --- | --- | --- |
| **O4** | `verify_commitment` trusts the contract for `\|c\| == threshold`; behaviour on an empty `c` is `frost-core` internal | **RESOLVED — promoted by this agent as `F-XC-051`** (together with O3, the identity-coefficient half). |
| **O9** | No membership check on the addresses carried by DKG events (`handle_key_gen_committed:173-175`, `handle_key_gen_confirmed:448`, `handle_key_gen_complained:714`) | **RESOLVED — promoted by this agent as `F-XC-050`.** Note the remediation was already half-recorded: `F-VAL-003` remediation option 4 lists exactly these three membership checks as defence in depth. The _consequence_ was what nobody claimed. |
| **O7** | Restart with an empty exclusion delta: an injected `KeyGenComplained` naming a **non-member** makes `also_exclude` a no-op, `participants_set` returns the _same_ group id, `start_key_gen` re-enters `CollectingCommitments` with an empty map for a group the contract will never re-emit `KeyGenCommitted` for; the round then stalls to timeout, `exclude_all_others(∅)` excludes everyone, and the epoch is skipped or genesis halts | **STILL DANGLING.** `F-VAL-060` cites `also_exclude` (basis row 6b, `state/keygen.rs:728-730`) but its claim is the _member_-accused path reaching genesis `Halted`. The non-member no-op variant — a different and arguably cheaper stall, since it needs no threshold of complaints — is claimed nowhere. |
| **O8** | `handle_epoch_staged`'s `WaitingForGenesis` recovery trusts the event's `proposedEpoch` (`state/keygen.rs:613-635`), so a staged epoch observed before genesis jumps the validator straight to `EpochSkipped { next_epoch }` on an attacker-influenced number | **STILL DANGLING.** `F-VAL-060` names `EpochStaged` in its prose list of "the same unguarded `match` arms" but claims no consequence for it. `handle_epoch_staged` is mentioned in `F-VAL-066` and `F-VAL-004` only in passing, for unrelated claims. |

**Recommended: O7 and O8 go to C-VAL-B**, which is still running and already holds the validator range. Both are one-paragraph promotions with R4's citations already gathered; neither needs new analysis, only an owner. This agent did not file them because C-VAL-B is live in that range and a collision would produce two findings for one defect.

---

## 6. Seeded leads — disposition of all 67

The map seeds **57 hypotheses** (CORE-H1…H17, VAL-H1…H11, SEN-H1…H15, ENG-H1…H14, in §6.1–6.4 and the four per-crate analyses) plus **10 Manager leads M1–M10** (§7). Every one was cross-referenced against all 101 finding files and all ten rejected-hypothesis lists.

**No lead was never examined. All 67 carry a recorded disposition in at least one reviewer log.**

Eleven leads are cited by no finding _by ID string_. Five of those are covered in substance — the reviewer names the lead in its log and the finding it produced, but the finding file never repeats the ID. Resolving those, **six leads are covered by no finding at all**:

| Lead | Disposition | Where it went |
| --- | --- | --- |
| **CORE-H15** — `fallible_events` silently discards logs without marking the gap | **Confirmed, observation only. No finding.** | The only lead whose ID appears in **no** log and **no** finding. Its substance is R1's Observation **O-6** and R1's §5 enumeration row, which says the dropped topic's logs are omitted "and the update is still emitted as complete for the range", and that no service configures it (`grep -rn "fallible_events" crates/` matches only `core/src/index/events.rs` and one test). R1 downgraded it to a documentation issue: the `Config` doc comment ("Use this to mark events as noncritical", `events.rs:89-91`) understates what enabling it accepts. **This agent agrees it is not a finding today** — unreachable, no service sets it — but the doc-comment fix is real and cheap. |
| **VAL-H8** — re-revealing nonces lets a signer appoint itself "responsible" and stall each ceremony by two timeouts | **Mechanism confirmed, deliberately not filed. No finding.** | R5: "Confirmed mechanically (`last_signer = Some(event.participant)` is overwritten on every accepted reveal, `state/sign.rs:284`, and the contract does not dedupe reveals, `FROSTCoordinator.sol:554-558`). **REJECTED as a finding at my severity bar** because the impact is bounded at one extra `signing_timeout` (6 blocks) … **Left to the Critic as a Low observation; I did not out-rank R4/R6 on it.**" C-VAL-A finished without promoting it; C-VAL-B is still running. **This is the clearest hand-off with no receiver in the run** — a confirmed mechanism, explicitly passed to "the Critic", and no Critic has yet taken it. **Recommended: C-VAL-B.** |
| **SEN-H9** — engine has unbounded authority over bond exposure; no local loss budget or kill switch | **Examined, dismissed under A3. No finding.** | See §5. Conscious omission resting entirely on A3. |
| **M3** — documentation drift: `docs/overview.md` still says `C[0]` is reused for ECDH and its one-time-pad argument no longer matches the code | **Confirmed at 90%, unfileable by scope rule.** | R4 confirmed it against `frost/keygen.rs:42` and `bindings.rs:60` while filing `F-VAL-002` for the code half (VAL-H5/M2). The doc fix should ship with `F-VAL-002`'s remediation; it belongs in the report's documentation section, not as a finding. |
| **M8** — the commit hash binds `reason`; the reveal must present the identical string | **Refuted / closed, with a full trace.** | R7: `reason` is produced once (`service.rs:173-180`), hashed (`:213`), stored verbatim (`:218`), carried unchanged (`:261-263`), and finally **moved** — not re-derived — into the `Reveal` action with `std::mem::take` (`:424`, `:432`). The only formatting site is `RuleId::Display` (`engine.rs:45-49`), ASCII, locale-independent, no floats, no truncation, run once before the hash. "**M8 is closed.**" This agent re-checked the `mem::take` and agrees. |

### 6.1 The five that look uncited but are covered

Recorded so the report does not double-count them as gaps:

| Lead | Covered by | Evidence |
| --- | --- | --- |
| **CORE-H7** | `F-CORE-061` | R3: "CORE-H7 … **mechanism CONFIRMED**, filed as `F-CORE-061`." |
| **CORE-H11** | `F-CORE-062` + `F-CORE-063` | R3: "**split into two findings.** The 'silently dropped action' half is `F-CORE-063`; the 'permanent gap' half is `F-CORE-062`." |
| **VAL-H1** | `F-VAL-001` | R4: "Confirmed and strengthened … → **F-VAL-001**." |
| **VAL-H5** (and **M2**) | `F-VAL-002` | R4: "**VAL-H5 / M2 / M3** — two-time pad, raw x-coordinate, doc drift. Confirmed … → **F-VAL-002**." |
| **SEN-H14** | `F-XC-008` | R10: "**Promoted and broadened** into F-XC-008." |

### 6.2 M1 — worth a note

R4 **refuted** M1 (a replayed keygen reusing the same encryption key) with a long, well-cited argument. C-VAL-A then mined that same refutation and promoted **`F-VAL-005`** — a reorg across the key-generation block deleting DKG secrets the store promises never to overwrite. That is the rejected-hypothesis mining in the Critic brief working exactly as designed, and it is the single best argument in this run for keeping refutations verbose and cited.

---

## 7. Toolchain-blocked questions — one list, for the first person with `cargo`

Consolidated from R10 §5 (eight), the gaps sections of R1, R2, R3, R4, R5, R6, R7, R9, and the `## Critic` sections written in Phase 2. **Twenty-two questions.** None can be answered in this checkout: there is no toolchain and `~/.cargo/registry` does not exist, so no dependency source is on disk (`baseline.md` §1–2). Every one of them is currently carried as a class `I` leg inside a finding, and each entry names the finding whose certainty moves when it is answered.

Ordered by how much a finding's certainty moves.

### Tier 1 — each one unblocks a specific finding's pivotal claim

| # | Question | Blocks | How to answer |
| --- | --- | --- | --- |
| 1 | Does `frost-core` 3.0.0 redact in `Debug` for `round1::SecretPackage`, `keys::KeyPackage`, `SigningShare` and `round2::Package`? R10 calls this "the single highest-value question in this list". | `F-XC-002`, `F-CORE-036`, `F-VAL-062`, VAL-H10 | A five-line test that `format!("{:?}", pkg)` contains no scalar bytes. Does not require reading upstream at all — `F-XC-002`'s Critic section already spells out the local test. |
| 2 | Does `alloy-sol-types` 1.6.0 reject invalid UTF-8 in a `string` field, or decode it lossily? | `F-SEN-013` (entire finding — a single undecodable `Revealed.reason` would stall every sentinel's indexer permanently) | Decode a hand-built log with a `string` containing `0xFF` and see whether it errors. |
| 3 | Does `serde_derive` accept, reject, or silently ignore `deny_unknown_fields` on a container that also has a `#[serde(flatten)]` field, in the locked version? | `F-XC-003` (its open leg), R6's H-R6-15 | One `#[test]` feeding a mistyped key to the real `Config` type. |
| 4 | Does `alloy`'s ABI decoder pre-allocate a `Vec` from the declared length of a dynamic array (`uint256[] f`, `Point[] c`) before validating it against the actual payload size? | `F-XC-051`, `F-VAL-001`, `F-VAL-003` — these arrays arrive from event data that F-VAL-060 shows is injectable | Decode a log whose array length prefix is `2^32` with a two-word payload; watch RSS. |
| 5 | What does `frost_core::keys::dkg::verify_proof_of_knowledge` do with a **zero-length** commitment vector — return `Err`, or index `[0]` and panic? A panic on that path is not caught anywhere: there is no `catch_unwind` in the driver. | `F-XC-051` (basis row 8), R4's Observation O4 | Call `verify_commitment` with `c = vec![]`. |

### Tier 2 — dependency behaviour that several findings lean on

| # | Question | Blocks |
| --- | --- | --- |
| 7 | Any advisory or CVE status for any of the 516 locked packages (`cargo audit`). **No advisory status is asserted anywhere in this audit** — `F-XC-007` says so explicitly. | `F-XC-007` |
| 8 | `reqwest 0.13.4`'s actual default redirect and proxy policy. | `F-XC-008` basis row 8 |
| 9 | Which TLS root source `reqwest` selects under `default-features = false, features = ["json", "rustls"]` — and therefore whether the images' `ca-certificates` package is load-bearing or dead weight. | `F-XC-004`, R10 Observation 3 |
| 11 | `sqlx` 0.9 defaults for `journal_mode`, `synchronous`, `busy_timeout`, `foreign_keys`, `create_if_missing` and pool size — `connect_sqlite` sets only the two recycling knobs (`utils.rs:56-62`). | `F-VAL-035` (the `ON DELETE CASCADE` leg, M6), R2 Observations O-6/O-7 |
| 12 | Do alloy's `PrivateKeySigner` and k256's `SigningKey` zeroize on drop, and does `to_bytes` leave an unzeroized intermediate? | R3 Observation O4; raises or lowers every secret-at-rest severity |
| 13 | What does alloy's `estimate_eip1559_fees` issue and return when `reward` is empty? `F-CORE-060`'s starting fee level currently comes from the tests' mock, not the real estimator. | `F-CORE-060` |
| 14 | `alloy` 2.0.5 specifics R1 could not read: `Filter::at_block_hash` encoding, `EthRpcErrorCode::ResourceNotFound == -32001`, `logs_bloom` accumulation semantics, `SolEventInterface::decode_raw_log` failure conditions, whether `ProviderCall::Ready` short-circuits every internal chain-id fetch. | `F-CORE-002`, `F-CORE-010`, `F-CORE-012`, `F-CORE-065` |

### Tier 3 — build, tooling and hygiene

| # | Question | Blocks |
| --- | --- | --- |
| 17 | Does `cargo clippy --workspace --all-targets --locked -- -D warnings` currently pass? CI says it must; nothing here observed it. | Phase 0's unfilled `E1` slot |
| 18 | Does the linker strip the unused `sqlx-mysql` / `sqlx-postgres` code from the release binaries — i.e. is `F-XC-007` item 2 a real surface reduction or only a build-time one? | `F-XC-007` |
| 19 | Does `cargo tree -d`'s real output match the lockfile-derived duplicate list in `baseline.md` §6? The substitute was a text parse and cannot see feature-gated edges. | `baseline.md` §6 |
| 20 | What are Cargo's stock `release` profile defaults on the pinned toolchain — confirming `overflow-checks = false` and `debug-assertions = false` in shipped binaries? | `F-XC-001` (its `I` leg) |
| 21 | Is the HKDF reference vector at `kdf.rs:36-46` reproducible with Python `hmac`/`hashlib`? C-CORE-B flagged that the reviewer declined to re-derive it. | `F-CORE-038` |

**Consequence for the report, stated once:** `E1` was reached zero times in this run. Per the Critic brief §2 the whole audit is capped at **89%**, and every finding above depends on at least one of these twenty-two answers for its remaining margin. Questions **1, 2, 3, 5, 17 and 21** are each answerable in under ten minutes once `cargo` exists.

---

## 8. Process notes for the Manager

1. **The 83-vs-81 miscount in `codebase-map.md` §10** should be corrected before the report quotes it. No coverage was lost; the number is simply wrong, as are §9's line totals for R2 and R9.
2. **Three findings carry `Status: Critiqued` with no `## Critic` section** in this snapshot (`F-ENG-042`, `F-ENG-043`, `F-SEN-015`) — the header was updated but the per-claim verdict trail is absent. C-ENG-B is reported complete, so these should be checked at the gate rather than assumed in flight. An earlier snapshot showed eleven in this state and eight resolved on their own, so this is most likely a write-ordering artefact — but the brief requires the verdicts to be _visible_, and for these three they are not.
3. **Two dangling observations need an owner:** R4's **O7** and **O8** (§5.1). Both are one-paragraph promotions with citations already gathered. **C-VAL-B** is the right owner and is still running.
4. **VAL-H8 was explicitly handed to "the Critic" by R5 and no Critic has taken it** (§6). **C-VAL-B** again.
5. **CORE-H5's core-layer claim has no home** (§5). Recommend assigning it, anchored at `tx/storage.rs:96-100`.
6. **SEN-H15's un-zeroised config `String` is a live mutual deferral** (§5), recorded here with citations rather than filed, and it belongs in the report's unverified-observations list.
7. Findings this agent filed — `F-XC-050`, `F-XC-051`, `F-XC-052` — are Draft and need a Critic. They are in the `F-XC` range but were **not** written by R10, so C-XC should critique them as fresh drafts rather than as promotions of R10's work.

## 9. Method and honest limits

- Every line count, assignment, finding-to-file mapping and Critic attribution in this document was computed from the filesystem this session, not copied from `codebase-map.md`.
- The "Claimed read" column is a **self-report**. This run has no way to verify that a reviewer read a file it says it read. What it can verify — and what §3 and §4 do — is whether the claimed read left evidence: findings, basis-row citations, rejected hypotheses, observations. Sixteen files produced no anchored finding and ten produced no citation at all; this agent read those and found three real gaps in them. That is the strongest available test of the self-reports, and it passed for the files that mattered.
- No file was executed, compiled or tested. Nothing outside `rust-audit/` was written or modified.
