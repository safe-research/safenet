# Run 3 coverage matrix

| Field | Value |
| --- | --- |
| Author | C3-COV, Coverage Critic (reviewer department), Phase 2 of run 3, discovery track |
| Date | 2026-10-08 |
| Audit commit | `361aaf3` (crates byte-identical to `c355af1`; `git diff --stat 361aaf3 -- crates Cargo.toml Cargo.lock contracts/src docs scripts` is empty in the main checkout) |
| Inventory | `state/run3/map.md` section 7: 75 files, 64 `.rs` files with 23,319 lines and 11 other files with 6,526 lines. Re-counted with `git grep -c '' 361aaf3 -- Cargo.toml Cargo.lock crates/core crates/validator crates/sentinel`: 75 files, every count equal to the map. Non-test lines are the lines before a file's `#[cfg(test)] mod tests`, as in the map; recomputed, all equal |
| Sources | The ten Phase 1 coverage logs (`state/run3/agents/{R1..R8, L-crypto, L-rust}.md`: their file tables and context lists), the `Location` field of the 32 `findings/F3-*.md` files (31 Phase 1 drafts plus `F3-XC-160`), the Critic assignments in the Phase 2 brief, and C3-COV's own spot-reads |
| Not read | `rv-*` logs, `state/run3/revalidation/`, `STATE.md`, earlier-run findings, report, PoCs, the old codebase map and analyses (independence rule) |

## 1. Summary

- Every in-scope file had at least one reviewer who read it in full, except `Cargo.lock`, which R8 read with tools (`cargo tree`, `cargo audit`, a source and checksum census).
- Full readers per file: 0 for 1 file, 1 for 12 files, 2 for 41, 3 for 12, 4 for 5, 5 for 2, 6 for 2.
- Of the 12 single-reader files, seven had their non-test lines read again as context by another reviewer (with small gaps). The non-test lines that only one reviewer had read were `index/bloom.rs` 1-41, `index/events.rs` 1-39, `consensus/epoch.rs` 61-77, validator `metrics.rs`, sentinel `metrics.rs`, `consensus/mod.rs` and `secrets/mod.rs`. C3-COV read all of them (section 3); nothing missed was found there.
- 38 of the 75 files are named by no F3 finding, so no Critic re-reads them through a finding. Section 5 lists the ones that matter most.
- Observation mining: 97 observation entries in the ten logs (section 6). One is promoted: `F3-XC-160` (R1's O5, a lagging RPC backend makes the watcher invent an uncle). 17 are covered by an F3 finding or confirm one, 11 are settled or known, and 68 stay observations, each with its reason.

## 2. Matrix

How to read it. Role: `P` primary reader or lens owner, `2nd` assigned second reader, `C` read as context by a reviewer not assigned to the file. Share: as the reviewer logged it; ranges are line ranges at `361aaf3`. Full readers: reviewers whose logged share is the whole file. F3 findings: `at:` the finding's main location, `related:` its related list, with the Phase 2 Critic that owns the finding. Critics: the owners of those findings, plus C3-COV where it spot-read the file or owns a finding there.

| # | File | Lines | Non-test | Reviewer logs (role, share) | Full readers | F3 findings naming the file (Critic) | Critics |
| --: | --- | --: | --: | --- | --: | --- | --- |
| 1 | `Cargo.lock` | 6,141 | n/a | R8 P tools (cargo tree, cargo audit, source and checksum census) | 0 | at: F3-XC-007 (C3-XC) | C3-XC |
| 2 | `Cargo.toml` | 26 | n/a | R8 P 100%; R6 C 100%; L-rust C 100% | 3 | at: F3-XC-003 (C3-XC) | C3-XC |
| 3 | `crates/core/Cargo.toml` | 33 | n/a | R8 P 100%; R3 C 100%; L-rust C 100% | 3 | none | none |
| 4 | `crates/core/src/config.rs` | 157 | 77 | R2 P 100%; L-rust P 100%; R6 C 1-80; R8 C 1-80 | 2 | none | none |
| 5 | `crates/core/src/driver.rs` | 328 | 328 | R2 P 100%; L-rust P 100%; R1 C 100%; R3 C 100%; R6 C 100%; R7 C 100%; R4 C 255-270; R5 C 100-300; R8 C 230-300 | 6 | at: F3-CORE-032 (C3-CORE); related: F3-CORE-001 (C3-CORE), F3-CORE-002 (C3-CORE), F3-CORE-003 (C3-CORE), F3-CORE-004 (C3-CORE), F3-CORE-030 (C3-CORE), F3-SEN-002 (C3-SEN), F3-SEN-004 (C3-SEN), F3-VAL-062 (C3-VAL), F3-XC-160 (C3-COV) | C3-CORE, C3-COV, C3-SEN, C3-VAL |
| 6 | `crates/core/src/effects.rs` | 267 | 112 | R2 P 100%; L-rust P 100%; R5 C 1-110 | 2 | none | none |
| 7 | `crates/core/src/index/blocks.rs` | 1,429 | 610 | R1 P 100%; R2 C 1-620; R3 C 140-600; R4 C 296-345; R5 C 129-138, 280-440; R6 C 75-160, 290-425; R7 C 290-425; R8 C 346-395, 498-565; L-rust C 140-300, 424-610 | 1 | at: F3-CORE-003 (C3-CORE), F3-CORE-004 (C3-CORE), F3-CORE-005 (C3-CORE), F3-CORE-030 (C3-CORE), F3-XC-160 (C3-COV); related: F3-CORE-032 (C3-CORE), F3-SEN-004 (C3-SEN), F3-VAL-062 (C3-VAL) | C3-CORE, C3-COV, C3-SEN, C3-VAL |
| 8 | `crates/core/src/index/bloom.rs` | 523 | 41 | R1 P 100% (test vector read for structure) | 1 | none | C3-COV (spot-read) |
| 9 | `crates/core/src/index/clock.rs` | 103 | 63 | R1 P 100%; R2 C 1-63; R8 C 28-63; L-rust C 60-103 | 1 | related: F3-CORE-005 (C3-CORE) | C3-CORE |
| 10 | `crates/core/src/index/events.rs` | 2,051 | 787 | R1 P 100%; R2 C 40-800; R6 C 40-120, 350-440; R7 C 480-800; R8 C 415-432, 636-668, 670-790; L-rust C 116-500, 650-720 | 1 | at: F3-CORE-001 (C3-CORE), F3-CORE-002 (C3-CORE) | C3-CORE |
| 11 | `crates/core/src/index/mod.rs` | 485 | 143 | R1 P 100%; R2 C 1-160; R3 C 1-145; R6 C 1-60; R7 C 1-60; L-rust C 1-145 | 1 | related: F3-XC-160 (C3-COV) | C3-COV |
| 12 | `crates/core/src/kdf.rs` | 81 | 28 | R2 P 100%; L-crypto P 100%; R3 C 1-30; L-rust C 1-30 | 2 | none | none |
| 13 | `crates/core/src/lib.rs` | 26 | 26 | R2 P 100%; R3 C 100% | 2 | none | none |
| 14 | `crates/core/src/metrics.rs` | 107 | 107 | R2 P 100%; R6 C 100%; L-rust C 80-107 | 2 | none | none |
| 15 | `crates/core/src/observability/logging.rs` | 21 | 21 | R2 P 100%; R6 C 100%; L-rust C 100% | 3 | none | none |
| 16 | `crates/core/src/observability/metrics.rs` | 80 | 49 | R2 P 100%; R6 C 100%; R8 C 100%; L-rust C 100%; R1 C 1-30, 60-80 | 4 | none | none |
| 17 | `crates/core/src/observability/mod.rs` | 94 | 64 | R2 P 100%; R6 C 100%; R8 C 100%; L-rust C 1-64 | 3 | none | none |
| 18 | `crates/core/src/provider/mod.rs` | 182 | 182 | R2 P 100%; L-rust P 100%; R1 C 100%; R6 C 100%; R8 C 100%; R3 C 100-182 | 5 | at: F3-CORE-031 (C3-CORE), F3-XC-130 (C3-CORE); related: F3-XC-004 (C3-XC) | C3-CORE, C3-XC |
| 19 | `crates/core/src/serialization.rs` | 218 | 144 | R2 P 100%; L-rust P 100%; R6 C 1-150; R8 C 1-145 | 2 | none | none |
| 20 | `crates/core/src/state/mod.rs` | 731 | 326 | R2 P 100%; R1 C 100%; R4 C 100-300; R5 C 100-300; R6 C 30-300; R7 C 1-305; R8 C 30-110, 160-295; L-rust C 1-330 | 2 | at: F3-CORE-032 (C3-CORE); related: F3-CORE-003 (C3-CORE), F3-CORE-030 (C3-CORE), F3-SEN-003 (C3-SEN), F3-SEN-004 (C3-SEN), F3-VAL-031 (C3-VAL), F3-VAL-062 (C3-VAL), F3-XC-002 (C3-SEN), F3-XC-160 (C3-COV) | C3-CORE, C3-COV, C3-SEN, C3-VAL |
| 21 | `crates/core/src/state/storage.rs` | 294 | 169 | R2 P 100%; R1 C 100%; R4 C 120-165; R5 C 40-170; R7 C 1-60; L-rust C 40-170 | 2 | related: F3-CORE-004 (C3-CORE), F3-CORE-032 (C3-CORE), F3-XC-160 (C3-COV) | C3-CORE, C3-COV |
| 22 | `crates/core/src/tx/bundle.rs` | 407 | 210 | R3 P 100%; R8 2nd 100%; R7 C 1-210; L-rust C 1-130 | 2 | related: F3-SEN-001 (C3-SEN) | C3-SEN |
| 23 | `crates/core/src/tx/config.rs` | 194 | 136 | R3 P 100%; R2 C 1-140; R7 C grep | 1 | at: F3-CORE-061 (C3-CORE); related: F3-CORE-060 (C3-CORE), F3-SEN-001 (C3-SEN) | C3-CORE, C3-SEN, C3-COV (spot-read) |
| 24 | `crates/core/src/tx/fees.rs` | 109 | 57 | R3 P 100%; R8 C 1-60 | 1 | at: F3-CORE-060 (C3-CORE); related: F3-CORE-061 (C3-CORE) | C3-CORE, C3-COV (spot-read) |
| 25 | `crates/core/src/tx/mod.rs` | 1,286 | 438 | R3 P 100%; R2 2nd 100%; R5 C 160-240; R6 C 1-440; R7 C 1-440; R8 C 30-104; L-rust C 1-104 | 2 | related: F3-CORE-060 (C3-CORE), F3-CORE-061 (C3-CORE), F3-VAL-030 (C3-VAL) | C3-CORE, C3-VAL |
| 26 | `crates/core/src/tx/signer.rs` | 297 | 179 | R3 P 100%; L-crypto P 100%; R2 C 100-180; L-rust C 1-60, 110-180 | 2 | none | none |
| 27 | `crates/core/src/tx/storage.rs` | 990 | 423 | R3 P 100%; L-rust P 100%; R2 C 1-380; R5 C 100-230; R7 C 1-440 | 2 | related: F3-CORE-060 (C3-CORE), F3-VAL-030 (C3-VAL) | C3-CORE, C3-VAL |
| 28 | `crates/core/src/tx/types.rs` | 166 | 166 | R3 P 100%; R7 C 100% | 2 | related: F3-CORE-060 (C3-CORE), F3-SEN-001 (C3-SEN) | C3-CORE, C3-SEN |
| 29 | `crates/core/src/utils.rs` | 97 | 97 | R2 P 100%; L-rust P 100%; R6 C 100%; R7 C 100%; R8 C 100%; R3 C 50-62; R5 C 40-62 | 5 | related: F3-SEN-005 (C3-XC), F3-VAL-060 (C3-XC), F3-XC-005 (C3-XC) | C3-XC |
| 30 | `crates/sentinel/Cargo.toml` | 24 | n/a | R7 P 100%; R8 2nd 100% | 2 | none | none |
| 31 | `crates/sentinel/Dockerfile` | 38 | n/a | R7 P 100%; R8 2nd 100% | 2 | at: F3-XC-003 (C3-XC) | C3-XC |
| 32 | `crates/sentinel/Dockerfile.dockerignore` | 4 | n/a | R7 P 100%; R8 2nd 100% | 2 | none | none |
| 33 | `crates/sentinel/sentinel.sample.toml` | 89 | n/a | R7 P 100%; R8 2nd 100%; R3 C [transactions] | 2 | at: F3-SEN-005 (C3-XC), F3-XC-005 (C3-XC); related: F3-CORE-061 (C3-CORE), F3-VAL-060 (C3-XC) | C3-CORE, C3-XC |
| 34 | `crates/sentinel/src/action.rs` | 46 | 46 | R7 P 100%; R8 C 100%; L-rust C 100% | 3 | none | none |
| 35 | `crates/sentinel/src/bindings.rs` | 176 | 176 | R7 P 100%; R1 C 100%; R2 C event declarations; L-crypto C 80-165 | 2 | none | none |
| 36 | `crates/sentinel/src/config.rs` | 135 | 59 | R7 P 100%; R2 C 100%; R8 C 1-70 | 2 | related: F3-XC-004 (C3-XC) | C3-XC |
| 37 | `crates/sentinel/src/effect.rs` | 263 | 165 | R7 P 100%; L-rust P 100%; R2 C 1-170; R8 C 80-160 | 2 | none | none |
| 38 | `crates/sentinel/src/engine.rs` | 430 | 206 | R7 P 100%; L-rust P 100%; R8 C 1-260 | 2 | at: F3-XC-004 (C3-XC) | C3-XC |
| 39 | `crates/sentinel/src/hashing.rs` | 224 | 103 | R7 P 100%; L-crypto P 100%; R2 C 1-60 | 2 | none | C3-COV (spot-read) |
| 40 | `crates/sentinel/src/main.rs` | 93 | 93 | R7 P 100%; R2 C 100%; R8 C 100%; L-rust C 100%; R1 C 46-93; R5 C 44-66 | 4 | related: F3-CORE-031 (C3-CORE), F3-SEN-005 (C3-XC), F3-XC-005 (C3-XC), F3-XC-130 (C3-CORE) | C3-CORE, C3-XC |
| 41 | `crates/sentinel/src/metrics.rs` | 143 | 143 | R7 P 100% | 1 | none | C3-COV (spot-read) |
| 42 | `crates/sentinel/src/service.rs` | 2,214 | 1051 | R7 P 100%; R8 2nd 100%; R2 C 100-210, 300-345, 396-440; R3 C 225-255, 425-450, 670-690, 860-1010; R5 C 400-480; L-crypto C 205-253, 420-450; L-rust C 159-205, 265-300, 580-620, 706-760, 819-862 | 2 | at: F3-SEN-001 (C3-SEN), F3-SEN-002 (C3-SEN), F3-SEN-003 (C3-SEN), F3-SEN-004 (C3-SEN), F3-XC-001 (C3-SEN), F3-XC-002 (C3-SEN), F3-XC-006 (C3-SEN); related: F3-CORE-032 (C3-CORE), F3-XC-160 (C3-COV) | C3-CORE, C3-COV, C3-SEN |
| 43 | `crates/sentinel/src/state.rs` | 383 | 181 | R7 P 100%; R8 2nd 100%; L-rust C 15-100 | 2 | related: F3-SEN-003 (C3-SEN), F3-XC-001 (C3-SEN) | C3-SEN |
| 44 | `crates/sentinel/src/verdicts.rs` | 268 | 148 | R7 P 100%; L-rust P 100% | 2 | none | C3-COV (spot-read) |
| 45 | `crates/validator/Cargo.toml` | 26 | n/a | R6 P 100%; R8 2nd 100% | 2 | none | none |
| 46 | `crates/validator/Dockerfile` | 37 | n/a | R6 P 100%; R8 2nd 100%; R2 C 100%; L-rust C 100% | 4 | at: F3-XC-003 (C3-XC) | C3-XC |
| 47 | `crates/validator/Dockerfile.dockerignore` | 4 | n/a | R6 P 100%; R8 2nd 100% | 2 | none | none |
| 48 | `crates/validator/src/bindings.rs` | 245 | 245 | R6 P 100% (plus executed ABI comparison); R1 C 100%; L-crypto C 100%; R2 C event declarations; R4 C 1-110; R5 C 75-250 | 3 | none | none |
| 49 | `crates/validator/src/config.rs` | 281 | 109 | R6 P 100%; R2 C 100%; R4 C 93-108; L-crypto C 90-108 | 2 | none | none |
| 50 | `crates/validator/src/consensus/epoch.rs` | 95 | 77 | R4 P 100%; R5 C 1-60; L-crypto C 1-40 | 1 | none | C3-COV (spot-read) |
| 51 | `crates/validator/src/consensus/group.rs` | 459 | 298 | R4 P 100%; L-crypto P 100%; R6 C 1-300 | 2 | related: F3-VAL-061 (C3-VAL) | C3-VAL |
| 52 | `crates/validator/src/consensus/hashing.rs` | 249 | 170 | R5 P 100%; L-crypto P 100% | 2 | none | none |
| 53 | `crates/validator/src/consensus/mod.rs` | 5 | 5 | R4 P 100% | 1 | none | C3-COV (spot-read) |
| 54 | `crates/validator/src/frost/ecdh.rs` | 316 | 241 | R4 P 100%; L-crypto P 100%; R5 C 60-90; R8 C 55-130; L-rust C 1-90, 200-245 | 2 | none | none |
| 55 | `crates/validator/src/frost/error.rs` | 46 | 46 | R4 P 100%; L-crypto P 100%; R8 C 100% | 3 | none | none |
| 56 | `crates/validator/src/frost/keygen.rs` | 515 | 515 | R4 P 100%; L-crypto P 100%; R2 C 1-60, 140-170, 420-470; R5 C 20-60, 420-460; R6 C 1-60, 145-450; R8 C 18-70, 145-170, 425-450; L-rust C 1-60, 140-170, 420-515 | 2 | at: F3-VAL-001 (C3-VAL), F3-XC-100 (C3-VAL) | C3-VAL |
| 57 | `crates/validator/src/frost/marshal.rs` | 181 | 181 | R4 P 100%; L-crypto P 100%; R8 C 100%; R5 C 64-181 | 3 | related: F3-VAL-001 (C3-VAL), F3-XC-100 (C3-VAL) | C3-VAL |
| 58 | `crates/validator/src/frost/mod.rs` | 325 | 16 | R5 P 100%; L-crypto P 100%; R6 C 1-120 | 2 | related: F3-XC-100 (C3-VAL) | C3-VAL |
| 59 | `crates/validator/src/frost/participants.rs` | 33 | 20 | R4 P 100%; L-crypto P 100%; R5 C 100% | 3 | none | none |
| 60 | `crates/validator/src/frost/serialization.rs` | 85 | 85 | R4 P 100%; L-crypto P 100% | 2 | none | C3-COV (spot-read) |
| 61 | `crates/validator/src/frost/sign.rs` | 236 | 212 | R5 P 100%; L-crypto P 100%; R8 C 18-75; L-rust C 15-60 | 2 | none | none |
| 62 | `crates/validator/src/main.rs` | 99 | 99 | R6 P 100%; R2 C 100%; R8 C 100%; L-rust C 100%; R1 C 40-99; R3 C 30-99 | 4 | at: F3-VAL-061 (C3-VAL); related: F3-CORE-031 (C3-CORE), F3-VAL-060 (C3-XC), F3-XC-005 (C3-XC), F3-XC-130 (C3-CORE) | C3-CORE, C3-VAL, C3-XC |
| 63 | `crates/validator/src/merkle.rs` | 142 | 94 | R5 P 100%; L-crypto P 100% | 2 | none | none |
| 64 | `crates/validator/src/metrics.rs` | 173 | 173 | R6 P 100% | 1 | none | C3-COV (spot-read) |
| 65 | `crates/validator/src/secrets/mod.rs` | 5 | 5 | R5 P 100% | 1 | none | C3-COV (spot-read) |
| 66 | `crates/validator/src/secrets/store.rs` | 1,056 | 426 | R5 P 100%; L-rust P 100%; R2 C 1-430; R4 C 1-215; R6 C 1-300; L-crypto C 1-426 | 2 | at: F3-VAL-032 (C3-VAL); related: F3-VAL-031 (C3-VAL) | C3-VAL |
| 67 | `crates/validator/src/service/action.rs` | 353 | 353 | R6 P 100%; R3 C 100%; L-crypto C 100-353; R5 C 1-140; R2 C gas and expiry lines; R4 C Sign and KeyGen actions | 2 | related: F3-VAL-030 (C3-VAL) | C3-VAL |
| 68 | `crates/validator/src/service/effect.rs` | 235 | 235 | R5 P 100%; R6 2nd 100%; L-rust P 100%; R2 C 100%; R8 C 100%; L-crypto C 100%; R4 C listed, share not stated | 6 | related: F3-VAL-032 (C3-VAL) | C3-VAL |
| 69 | `crates/validator/src/service/mod.rs` | 129 | 129 | R6 P 100%; R1 C 100%; R2 C 60-129; L-crypto C 30-80; R4 C listed, share not stated | 2 | at: F3-VAL-061 (C3-VAL) | C3-VAL |
| 70 | `crates/validator/src/state/keygen.rs` | 1,447 | 1447 | R4 P 100%; R6 2nd 100%; L-crypto C 100%; R2 C 36-275, 920-1190; R5 C 425-650, 1252-1370 | 3 | at: F3-VAL-062 (C3-VAL); related: F3-CORE-032 (C3-CORE), F3-VAL-001 (C3-VAL), F3-VAL-061 (C3-VAL), F3-XC-100 (C3-VAL) | C3-CORE, C3-VAL |
| 71 | `crates/validator/src/state/mod.rs` | 487 | 487 | R6 P 100%; R4 C 100%; R5 C 100%; R2 C 100%; L-rust C grep | 4 | none | none |
| 72 | `crates/validator/src/state/secrets.rs` | 86 | 86 | R5 P 100%; R2 C 100% | 2 | none | C3-COV (spot-read) |
| 73 | `crates/validator/src/state/sign.rs` | 836 | 836 | R5 P 100%; L-crypto P 100%; R6 C 100%; R2 C 1-152; R3 C 740-836; L-rust C 21-112 (grep), 233-260, 318-390 | 3 | at: F3-VAL-030 (C3-VAL), F3-VAL-031 (C3-VAL); related: F3-VAL-032 (C3-VAL) | C3-VAL |
| 74 | `crates/validator/src/state/transactions.rs` | 102 | 102 | R5 P 100%; R6 C 100%; L-crypto C 100%; L-rust C 1-80 | 3 | none | none |
| 75 | `crates/validator/validator.sample.toml` | 104 | n/a | R6 P 100%; R8 2nd 100%; R2 C 1-40; R3 C [transactions] | 2 | at: F3-VAL-060 (C3-XC), F3-XC-005 (C3-XC); related: F3-CORE-061 (C3-CORE) | C3-CORE, C3-XC |

## 3. Least-covered files and spot-reads

Ranking per crate: fewest full readers, then fewest readers of any part, then whether a second reviewer read the non-test lines. C3-COV read each file below in full at `361aaf3` (`git show 361aaf3:<path>`), plus the cited callers.

| Crate | File | Coverage before Phase 2 | Spot-read result |
| --- | --- | --- | --- |
| core | `index/bloom.rs` (41 non-test lines) | R1 only; nobody else opened it | `may_contain_log` has no production caller (R1's O8 already); `compute_logs_bloom` is the client-filtering check (`crates/core/src/index/events.rs:540-548`). The `Events` trait doc still says the topics are used "for the bloom-filter skip check" (`crates/core/src/index/events.rs:33-35`), which no production path runs: stale doc, Informational, not filed |
| core | `tx/fees.rs` (57) | R3 in full; R8 1-60 for the panic census | Bounded arithmetic: `cap_priority_fee` never exceeds the original max fee (`:21-30`), `bump` keeps the priority fee at or below the max fee (`:39-56`). Edge values of the cap are F3-CORE-061, the missing ceiling F3-CORE-060. Nothing new |
| core | `tx/config.rs` (136) | R3 in full; R2 1-140 as context; R7 by grep | Only the mode combination is validated (`:101-135`); the unchecked values are F3-CORE-061; `max_batch_gas = 0` is accepted on purpose (`:55-58`). Nothing new |
| validator | `metrics.rs` (173) | R6 only | Read with every call site (`git grep -n 'metrics::' 361aaf3 -- crates/validator/src`): static labels, no address, key or group id, gauges follow the store's counts (`crates/validator/src/secrets/store.rs:158-159, 205, 249, 376-377`). Nothing new |
| validator | `consensus/epoch.rs` (77) | R4 in full; R5 1-60; L-crypto 1-40 (lines 61-77, the serde impls, had one reader) | `next_number` and the raw-value serde are correct. Observation O-COV-1: the contract does not store the epoch length (`contracts/src/Consensus.sol:378-397` only compares `rolloverBlock`), so `blocks_per_epoch` must agree across validators by configuration alone (`crates/validator/src/config.rs:61-63`); a validator with another value proposes other epochs and rollover blocks without any error, the same silent class as F3-VAL-061. Operator configuration (A1); handed to C3-VAL, not filed |
| validator | `state/secrets.rs` (86) | R5 in full; R2 in full as context | Retention is correct, including a commitment round whose setup is still outstanding: `KeyGenCommitment::Participating` covers `secrets: None` (`crates/validator/src/state/mod.rs:165-172`) and maps to `RetainedSecrets::All` (`:58-72`). Nothing new |
| validator | `frost/serialization.rs` (85), read as well because it is new since run 2 and sits under F3-VAL-001 and F3-XC-100 | R4 and L-crypto only | The identity-tolerant serde covers the published commitments and the group commitment (`crates/validator/src/frost/keygen.rs:71-76, 110-116`); verifying shares are recomputed, never persisted. The module doc's premise (`:4-9`, "the misbehaving participant cannot produce valid shares and is identified") leaves out the honest victim that F3-XC-100 and F3-VAL-001 describe; handed to C3-VAL for that finding's remediation, not filed |
| validator | `consensus/mod.rs`, `secrets/mod.rs` (5 lines each) | R4, R5 only | Module declarations and one re-export. Nothing to find |
| sentinel | `metrics.rs` (143) | R7 only | Read with every call site: `RevealMissed` can never be recorded on the event order the oracle produces (F3-SEN-003, F3-XC-002); `requests_proposed_total` counts replays again (R7's O10). Nothing new |
| sentinel | `verdicts.rs` (148) | R7 and L-rust only | First verdict wins in one statement (`:110-119`), `get` raises the retention block (`:79-89`), pruning uses the snapshot boundary (`:126-132`). Nothing new |
| sentinel | `hashing.rs` (103) | R7, L-crypto; R2 1-60 | The commit preimage packs `reason` as raw bytes, as `abi.encodePacked(string)` does (`:31-37`); `safe_tx_hash` is test-only (`:57`). L-crypto's executed parity covers the rest. Nothing new |
| core | `index/events.rs` lines 1-39 | R1 only (R2's context read starts at 40) | Module doc, imports and the `Events` trait; see the bloom doc note above |
| workspace | `Cargo.lock` | R8 by tools; Recon3 `cargo audit` and `cargo tree` | Not read line by line. R8's census stands: 570 packages, 566 from crates.io with checksums, 4 workspace members, no git sources, no `[patch]` |

## 4. Files no reviewer read in full

- `Cargo.lock` (6,141 lines): tools only (R8, Recon3). A line-by-line read adds nothing the tools do not check.
- `crates/core/src/index/bloom.rs`: R1 logged 100 %, but read the 450-line Gnosis test vector "for structure, not value by value". The vector is test data; the production lines are 1-41.

Files with exactly one full reader before Phase 2: `core/src/index/{blocks,bloom,clock,events,mod}.rs`, `core/src/tx/{config,fees}.rs`, `sentinel/src/metrics.rs`, `validator/src/consensus/{epoch,mod}.rs`, `validator/src/metrics.rs`, `validator/src/secrets/mod.rs`. All of R1's five index files fall here: R1 was the only primary reader of indexing, and the other reviewers read the production halves as context.

## 5. Files no finding names

38 files: `crates/core/Cargo.toml`, core `config.rs`, `effects.rs`, `index/bloom.rs`, `kdf.rs`, `lib.rs`, `metrics.rs`, `observability/{logging,metrics,mod}.rs`, `serialization.rs`, `tx/signer.rs`; `crates/sentinel/Cargo.toml`, `Dockerfile.dockerignore`, sentinel `action.rs`, `bindings.rs`, `effect.rs`, `hashing.rs`, `metrics.rs`, `verdicts.rs`; `crates/validator/Cargo.toml`, `Dockerfile.dockerignore`, validator `bindings.rs`, `config.rs`, `consensus/{epoch,hashing,mod}.rs`, `frost/{ecdh,error,participants,serialization,sign}.rs`, `merkle.rs`, `metrics.rs`, `secrets/mod.rs`, `state/{mod,secrets,transactions}.rs`.

No Critic re-reads these through a finding, so their review rests on Phase 1. The security-relevant ones each had two full readers, and several have executed evidence:

- Secret handling: core `config.rs` and `serialization.rs` (R2 and L-rust in full, R6 and R8 in part; R2 and L-rust ran multibyte and fuzzed TOML errors, `logs/R2-toml-span-fuzz.txt`, `logs/L-rust-config-multibyte-spans.txt`), `tx/signer.rs` (R3 and L-crypto).
- Cryptography: `consensus/hashing.rs`, `frost/sign.rs`, `frost/ecdh.rs`, `merkle.rs`, sentinel `hashing.rs` (each with L-crypto in full and executed Rust and Solidity parity, `logs/L-crypto-parity.txt`).
- Bindings: validator `bindings.rs` (R6's executed ABI comparison, 33 signatures, `logs/R6-abi-compare.txt`); sentinel `bindings.rs` (R7 and R1 in full, compared by reading only).
- Sentinel stores: `verdicts.rs` and `effect.rs` (R7 and L-rust in full, existing unit tests run, `logs/L-rust-tests.txt`).

## 6. Observations mined

Every observation entry in the ten coverage logs, with its disposition. "Kept" means it stays an unverified or hardening observation; the reason is given. Settled and known items refer to `settled.md` sections 1 and 2.

| Source | Observation | Disposition |
| --- | --- | --- |
| R1 O1 | Warp pages are trusted as complete; no bloom or count check, even with `use_client_filtering` | Kept. A4 allows incomplete `eth_getLogs`, but the documented failure is a query "too soon" after a block appears (`docs/validator-handbook.md:37`), and warps stop at least `max_reorg_depth` blocks below the head |
| R1 O2 | Bloom equality is not completeness; junk logs can push the all-logs query over provider caps | Kept. The documented failure returns an empty array, which the bloom check catches; stuffing only forces the unchecked fallback F3-CORE-002 already describes |
| R1 O3 | `max_reorg_depth = 0` loops instead of failing | Kept. Non-default configuration (A1) |
| R1 O4 | Fresh start, reorg of the first indexed block, `MissingSnapshot` | Kept. Theoretical: a 5-deep reorg within one block time of the first start (same as R2's O13) |
| R1 O5 | `revalidate_last_block` treats a missing block as uncled, so a lagging backend causes a one-block rollback | **Promoted: `F3-XC-160`** (Medium, tagged `known` for the #973 consequence). It fell between reviewers: R1 handed the consequences to R2 and R3, and F3-CORE-032 covers restarts and real reorgs only |
| R1 O6 | With `max_logs_per_query` set to M, M watched logs of one event in one block stall the node; `sign` is permissionless | Kept. Opt-in and undocumented (`crates/core/src/index/events.rs:96-99`, default `None`). R6's Foundry gas report gives 53,397 gas for one `sign` call at Aegis size (`logs/R6-gas-coordinator.txt`), so a 17M-gas block holds a few hundred `Sign` events at most; only a cap below that stalls, and provider caps are larger |
| R1 O7 | One undecodable watched log stalls the watcher forever | Kept. No trigger with the current bindings (R1 Q3, R7 Q54, L-rust O1) |
| R1 O8 | No bloom skip: `may_contain_log` is dead code | Kept. Performance; C3-COV adds the stale `Events` doc line (section 3) |
| R1 O9 | Uncled-block recovery needs code -32001; other codes keep the event watcher on an orphaned hash while the block watcher is not polled | Kept, cross-referenced in `F3-XC-160` (Considered and rejected). Needs the Gnosis clients' answers for an orphaned hash before it can be a finding |
| R1 O10 | Unbounded header fan-out on warp pages | Kept. Adds request pressure to F3-CORE-001 |
| R1 O11 | RPC calls have no timeout | Kept. A4 trusts the RPC for liveness; the engine-client analogue is settled as Informational |
| R1 O12 | Zero timings make the block loop spin | Kept. Configuration (A1) |
| R1 cross-reference | No-expiry actions replayed during a `start_block` back-fill | Covered by R2's Q16 answer and O9: gas only |
| R2 O1 | RPC amplification during warps from the unbiased `select!` | Kept. Combines two known items (cancel safety of `next_input`, the per-block reconcile write); E2 only |
| R2 O2 | SIGTERM drops ready resumes; a lost `UseNonces` costs one share | Covered by F3-VAL-032 (item 2) and the known cancel-safety item |
| R2 O3 | Effect failures are never retried | Kept. Needs a SQLite error past the 5 s busy timeout; no trigger at Aegis load (same as R5 O2, L-rust O10) |
| R2 O4 | `/health` always answers `OK` | Kept. Monitoring; no document presents `/health` as a progress probe (`grep -rn health docs/` is empty). Same as R6 O3, L-rust O1 and O3; it aggravates F3-CORE-001 and the known exit-status item |
| R2 O5 | The RPC client has no request timeout | Kept (A4, as R1 O11) |
| R2 O6 | The watcher retry logs every 100 ms | Covered by F3-CORE-001 |
| R2 O7 | alloy's debug span records the RPC URL | Known (#1046, issue #113) |
| R2 O8 | Unknown keys in flattened tables report line 1, column 1 | Kept. UX (same as R6 O4) |
| R2 O9 | Restart and reorg duplicates of no-expiry actions | Kept. Gas only |
| R2 O10 | No `secure_delete` or journal mode | A1 (plaintext at rest) |
| R2 O11 | `create_if_missing` is false and the samples omit `mode=rwc` | Covered by F3-VAL-060, F3-SEN-005, F3-XC-005 |
| R2 O12 | No release profile: release builds wrap on overflow | Kept. No concrete overflow found (R8's census agrees) |
| R2 O13 | Two `MissingSnapshot` paths | Kept. Theoretical |
| R2 O14 | Metrics listener: one task per connection, tight `accept` retry | Kept. Hardening, only when exposed |
| R2 O15 | Warps apply resumes only between pages; late-setup share deadline | Covered by F3-VAL-062 |
| R3 O1 | Batched throughput below Direct mode at the default `max_batch_gas`, while the handbooks say batching improves throughput | Kept. Matters only under sustained load (about 7 sentinel actions per batch against 16 in flight); docs nuance, Informational at most |
| R3 O2 | Underpriced replacement wording of Nethermind and Erigon | Kept. Not verifiable offline |
| R3 O3 | An executed marker from a sibling block is never re-checked | Kept. Needs a by-number answer from a fork plus the transaction leaving every mempool |
| R3 O4 | Late actions after expiry | Kept. Gas only |
| R3 O5 | Batched-mode latency of about two blocks per action | Kept. Design; no concrete missed window computed |
| R3 O6 | geth-family authorization tracking limits in-flight transactions | Kept. Gnosis client rules not verifiable offline |
| R3 O7 | The queue does not deduplicate | Kept. Gas (R2's Q16) |
| R3 O8 | No queue metrics for in-flight count, fee level or balance | Kept. Operability |
| R3 O9 | Startup prunes before it unmarks, so rows from a reorg during downtime are deleted unchecked | Kept. Queue twin of F3-CORE-004; needs a reorg deeper than the window while down |
| R4 O1 | Off-curve non-constant coefficients: the contract accepts, Rust rejects | Kept. One lost round and the sender is excluded (self-harm); L-crypto R9 agrees |
| R4 O2 | A late rollover makes the confirm callback revert and excludes the honest last confirmer | Kept. R6's Q29 answer disagrees (nobody is excluded: the revert boundary is the one at which every validator abandons the attempt); handed to C3-VAL |
| R4 O3 | A bogus on-curve `y` accepted onchain, rejected locally | Kept. Self-harm, the group never finalizes |
| R4 O4 | `Secrets`, `SharingState`, `KeyShare` derive `Debug` and `Serialize` | Covered: every secret field redacts in `Debug` (R5 H5, R6, R8 Q57, L-crypto R11, L-rust Q57); plaintext at rest is A1 |
| R5 O1 | A taken nonce pair is burned on shutdown | Covered by F3-VAL-032 |
| R5 O2 | `GenerateNonces` and `UseNonces` are not retried | Kept (as R2 O3) |
| R5 O3 | A collected row is recreated with a fresh pair | Covered by F3-VAL-032 |
| R5 O4 | The reconciliation marker orders by block number only | Kept. Documented trade-off (`crates/validator/src/secrets/store.rs:38-41`), liveness only |
| R5 O5 | A member can finish with plain `signShare` and force the attestation fallback | Kept. By design, bounded delay and gas |
| R5 O6 | Every remaining signer sends `Sign` after a request timeout | Kept. Gas only |
| R5 O7 | Nonce JSON in a `String` is not zeroized | A1 |
| R6 O1 | An empty `oracles` list silently disables attestation | Kept. Operator configuration (A1); `docs/configuration.md:84-88` shows the Aegis value, the sample marks it optional. Same silent class as F3-VAL-061 |
| R6 O2 | Coordinator and Consensus events are dispatched without checking `log.address` | Kept. Defense in depth: an allow-listed oracle can already approve anything |
| R6 O3 | `/health` is static | Kept (as R2 O4) |
| R6 O4 | Typo position line 1, column 1 | Kept (as R2 O8) |
| R6 O5 | `start_block` is not in the sample or handbook | Kept. Genesis only (A16) |
| R6 O6 | A complaint mined after the complaint deadline needs no response, yet onchain the plaintiff cannot confirm, so the honest plaintiff is excluded while the accused stays | Kept. Needs the plaintiff's own complaint transaction delayed past the deadline (`expires_at` is at most the complaint deadline, `crates/validator/src/state/keygen.rs:337-340, 371-379`), about 10 minutes. Contradicts R4's Q30 answer ("matches the contract in every phase"); handed to C3-VAL |
| R6 O7 | Every validator submits the attestation fallback | Kept. Gas only |
| R6 O8 | Floating base images, root user | Kept. Hardening (R8 O-3) |
| R6 O9 | Live and late-setup share deadlines differ | Covered by F3-VAL-062 |
| R7 O1 | The early `Finalize` is always rejected | Covered by F3-XC-006 |
| R7 O2 | Engine response size is unbounded | Kept. A3 |
| R7 O3 | Engine errors log the URL | Covered by F3-XC-004 |
| R7 O4 | No concurrency limit on engine checks | Kept. A3 (engine capacity) |
| R7 O5 | `handle_arbitration_timeout` claims without the outcome guard | Kept. The states it would wrongly claim from are unreachable by then |
| R7 O6 | No startup check that the oracle's proposer is the configured `consensus` | Kept. Operator configuration (A1); same silent class as F3-VAL-061 |
| R7 O7 | `main.rs` timeout comment and the sample's `voting_window` disagree | Known (the TODO markers, `settled.md` section 2) |
| R7 O8 | A storage error on `VerdictStore::get` during a replay drops a committed request | Kept. Needs a SQLite fault |
| R7 O9 | `WaitingForOutcome` entries are never cleaned up | Covered by F3-SEN-002 and F3-XC-001 (case 2 and variant B) |
| R7 O10 | `requests_proposed_total` counts replays | Kept. Metric only |
| R7 O11 | Dockerfile root user, `.dockerignore` admits all of `crates/**` | Kept. Hardening |
| R8 O-1 | How often F3-XC-001 happens on Aegis | Covered: a QA question for F3-XC-001 |
| R8 O-2 | `frost-core` with `internals` under `^3` | Kept. Dependency hygiene; the lock pins 3.0.0 and both images build with `--locked` |
| R8 O-3 | Docker hardening | Kept |
| R8 O-4 | Both samples set `signer` to private key 1 | Kept. Hardening (A1): a node started with it uses a public key; the operator funds a well-known address |
| R8 O-5 | Backup and restore advice in the samples and handbook | A17 |
| R8 O-6 | Metrics listener without authentication when exposed | Kept. Hardening |
| R8 O-7 | An oracle whose `OracleResult` uses other `indexed` flags would fail decoding | Kept. Operator configuration |
| R8 O-8 | Release builds wrap on overflow | Kept (as R2 O12) |
| R8 O-9 | The startup error for a TLS failure carries no URL | Not a defect; supports the scope of F3-CORE-031 and F3-XC-130 |
| L-crypto O1 | A complaint response publishes the pair's pad | Known (ECDH pad, `settled.md` section 2) |
| L-crypto O2 | PoK and PoP challenges carry no group, epoch or chain context | Kept. Needs a contract change; identifier binding makes replay harmless |
| L-crypto O3 | Cleared nonces stay in free SQLite pages | A1 |
| L-crypto O4 | HKDF PRK not zeroized | A1 |
| L-crypto O5 | A reorg can split honest shares across roots | Covered by F3-VAL-031 |
| L-crypto O6 | Shares computed on a reorged-out approval still land | Covered by F3-VAL-030 |
| L-crypto O7 | Hand-rolled proposal hashing is pinned | Not a defect |
| L-rust O1 | Permanent watcher errors are retried forever and `/health` stays `OK` | Kept. No trigger found (see R1 O7 and O9) |
| L-rust O2 | No cap on concurrent effects | Kept. A3 |
| L-rust O3 | `/health` is static | Kept (as R2 O4) |
| L-rust O4 | Secret copies are not zeroized | A1 |
| L-rust O5 | `ObservabilityTransport` calls a clone without `poll_ready` | Kept. Harmless with alloy's always-ready transports |
| L-rust O6 | Both `Config` types derive `Debug` with the RPC URL | Kept. Nothing logs them today |
| L-rust O7 | The images run the binary as PID 1 without an init | Kept. Harmless |
| L-rust O8 | RPC calls inside `Driver::update` have no timeout | Kept (as R1 O11) |
| L-rust O9 | A driver panic exits 101, a handled fatal error exits 0 | Known (exit status item, `settled.md` section 2) |
| L-rust O10 | Failed validator effects are not retried | Kept (as R2 O3) |
| L-rust O11 | No concurrent-call tests for the secret and verdict stores | Kept. Test gap |
| L-rust O12 | The reconcile `UPDATE` rewrites every secret row on each `NewBlock` | Known (per-block reconcile performance item, extended to live blocks) |
| L-rust O13 | Engine response body without a size cap | Kept. A3 |

Totals: 97 entries; 1 promoted, 17 covered by an F3 finding or confirming one (two of them "not a defect"), 11 settled or known (A1, A17 or a `settled.md` section 2 item), 68 kept as observations.

## 7. Hand-offs to the other Critics

- C3-CORE: `F3-XC-160` is a trigger for F3-CORE-032's live rollback that needs no reorg; the two should be read together, and a fix for F3-CORE-032 removes `F3-XC-160`'s consequence. R3's O9 is the queue-side twin of F3-CORE-004.
- C3-SEN: `F3-XC-160` reaches the same end state as F3-SEN-004 and issue #961 without a restart or a reorg.
- C3-VAL: two disagreements between Phase 1 reviewers to settle: R4's O2 against R6's Q29 (does a late rollover exclude the honest last confirmer), and R4's Q30 against R6's O6 (late complaints). Two additions: `blocks_per_epoch` agreement rests on configuration alone (O-COV-1, a sibling of F3-VAL-061), and the `frost/serialization.rs` module doc premise that F3-XC-100 and F3-VAL-001 contradict.
- C3-XC: R8's O-4 (sample signer key 1) sits next to the sample findings F3-VAL-060, F3-SEN-005 and F3-XC-005 if those are merged into one canonical sample-config finding.
