# Run 2 — coverage (Coverage Critic)

Audited commit `3ec8bc57dc35d1e9e65075ae9424bff427c47833` (working tree identical for `crates/`). Inputs: `state/run2/reviewer-split.md` (61 `.rs` files, 20,301 lines; two sample configs, two Dockerfiles), `state/run2/baseline.md` §3–5, the eight coverage logs `state/run2/agents/R1.md`…`R8.md`, the 63 `F2-*` finding files (titles and `Location` rows only, plus F2-CORE-030 in full for the overlap check in §5), and my own reads of the seam code listed in §3. Independence kept: no `findings/F-*.md`, `report/`, or `state/*.md` outside `state/run2/` was opened. Critics run concurrently with this log, so no Critic column is given; the Manager can add it at Gate 2.

Verdict rubric. `covered`: the owning log cites file-specific functions or lines and records at least one hypothesis raised or rejected about the file (or a finding is anchored in it). `thin`: the owning log lists the file with a percentage but no file-specific hypothesis; secondary evidence (another log, a finding, my spot-read) is noted. `unverified`: no evidence anywhere. All eight `thin` files were spot-read in full by me (§4); none held a defect, so nothing is `unverified`.

## 1. Per-file matrix

"Claimed" is the percentage the owning log states. "Evidence in log" is the owning log's file-specific material (finding IDs it anchors there, rejected hypotheses by number, observations by number). "Findings touching" lists every `F2-*` whose `Location` row (primary or related) names the file; "(body)" marks a mention outside the `Location` row.

### 1.1 `crates/core` (R1: index; R2: runtime/state/observability; R3: tx)

| File | Lines | Rev | Claimed | Evidence in log | Findings touching | Verdict |
| --- | --: | --- | --- | --- | --- | --- |
| `crates/core/src/index/blocks.rs` | 1330 | R1 | 100 % | 23 tests read; rejected 2–4, 7, 8, 13, 15–17, 22, 23 cite lines; E1 `cargo test --lib index::` (51 passed) | CORE-001/003/006/007/008/009/030/032, SEN-001/007, VAL-003/031, XC-006/009 | covered |
| `crates/core/src/index/events.rs` | 1516 | R1 | 100 % | 19 tests and macro read; rejected 5, 6, 10, 12, 18, 19; O1, O2, O6, O7, O14 | CORE-002/003/004/005/010, VAL-061 | covered |
| `crates/core/src/index/bloom.rs` | 523 | R1 | 100 % | data vector 111–490 grep-scanned; rejected 11, 12 (alloy `logs_bloom` verified in registry); E1 block vector test | CORE-003 | covered |
| `crates/core/src/index/clock.rs` | 103 | R1 | 100 % | O3 (`clock.rs:33-38`), O4 | XC-009 | covered |
| `crates/core/src/index/mod.rs` | 468 | R1 | 100 % | rejected 1 (`86-95`), 2; F2-CORE-005 anchored at `85-130` | CORE-003/005/006/008 | covered |
| `crates/core/src/provider/mod.rs` | 166 | R1 | 100 % | O8 (`129-137`, `135`, `163-165`, `86-94`), rejected 21 | CORE-033, XC-006 | covered |
| `crates/core/src/driver.rs` | 328 | R2 | 100 % | housekeeping diff audited hunk by hunk (§4); rejected 3, 4, 15; O4, O11, O13, O14; F2-CORE-031/034 anchored | CORE-001/002/005/007/008/010/030/031/032/033/034/035/063, SEN-001/007, XC-006, **XC-050** | covered |
| `crates/core/src/effects.rs` | 267 | R2 | 100 % | 8 tests read; rejected 9, 10, 22; §4 concurrency notes; F2-CORE-035 anchored | CORE-034/035, SEN-005, VAL-063 | covered |
| `crates/core/src/kdf.rs` | 81 | R2 | 100 % | rejected 13 (HKDF usage, E1 vector test), O9 (salt normalisation) | — | covered |
| `crates/core/src/lib.rs` | 25 | R2 | 100 % | listed only | — | thin |
| `crates/core/src/metrics.rs` | 90 | R2 | 100 % | rejected 12 (`metrics.rs:89` spawn inside runtime) | CORE-067 | covered |
| `crates/core/src/observability/logging.rs` | 21 | R2 | 100 % | listed only | — | thin |
| `crates/core/src/observability/metrics.rs` | 80 | R2 | 100 % | F2-CORE-033 anchored (`/health`, E1 `serves_metrics_and_health`) | CORE-033, VAL-066, XC-004 | covered |
| `crates/core/src/observability/mod.rs` | 94 | R2 | 100 % | rejected 17 (`29-35`, `17`) | CORE-067 (body) | covered |
| `crates/core/src/serialization.rs` | 34 | R2 | 100 % | rejected 14 (`19-33`) | — | covered |
| `crates/core/src/state/mod.rs` | 644 | R2 | 100 % | 6 tests; rejected 3, 5–8, 20; O3, O5; F2-CORE-030 anchored | CORE-001/006/007/030/032/063, SEN-001/003, VAL-003/005/031/063, **XC-050** | covered |
| `crates/core/src/state/storage.rs` | 294 | R2 | 100 % | 7 tests; rejected 4, 18, 20, 21; O4, O10 | CORE-001/030, SEN-004, XC-005/006, **XC-050** | covered |
| `crates/core/src/utils.rs` | 97 | R2 | 100 % | rejected 2, 11; O1 (sqlx defaults verified in registry) | CORE-034, XC-002/005 | covered |
| `crates/core/src/tx/fees.rs` | 109 | R3 | 100 % | rejected R7–R9 (ratio-cap proof, overflow); O7 | CORE-060/062/065 | covered |
| `crates/core/src/tx/mod.rs` | 719 | R3 | 100 % | rejected R10–R14, R16, R18–R20; E1 `tx::` (20 passed); F2-CORE-061/062/065/067 anchored | CORE-032/060/061/062/063/064/065/066/067, SEN-005 | covered |
| `crates/core/src/tx/signer.rs` | 118 | R3 | 100 % | rejected R17 (`96-100`, `60-63`, `89-92`); O8, O10 | VAL-060, XC-007 | covered |
| `crates/core/src/tx/storage.rs` | 507 | R3 | 100 % | rejected R2–R6, R15, R16; F2-CORE-063/064/066 anchored | CORE-030/032/060/061/063/064/066/067, SEN-005/007, **XC-050** | covered |
| `crates/core/src/tx/types.rs` | 87 | R3 | 100 % | rejected R5 (`81-86`, `bump(fresh, None)`) | CORE-060 | covered |

### 1.2 `crates/validator` (R4: DKG; R5: signing/secrets; R6: service/wiring)

| File | Lines | Rev | Claimed | Evidence in log | Findings touching | Verdict |
| --- | --: | --- | --- | --- | --- | --- |
| `crates/validator/src/frost/mod.rs` | 258 | R4 | 100 % | E1 `frost::tests::ceremony`; O5 (happy path only). R5 relied on it without reading | — | covered |
| `crates/validator/src/frost/keygen.rs` | 516 | R4 | 100 % | assignment answers cite `86-93`, `196-214`, `316-324`, `420-427`; rejected 1, 5, 7, 13 | VAL-001/002, XC-005 | covered |
| `crates/validator/src/frost/ecdh.rs` | 181 | R4 | 100 % | pad derivation `115-119`; E1 `ecdh_is_commutative`; rejected 4 | VAL-001/002 | covered |
| `crates/validator/src/frost/participants.rs` | 33 | R4 | 100 % | rejected 12 (`16-19` expect probability) | — | covered |
| `crates/validator/src/frost/marshal.rs` | 176 | R4 | 100 % | rejected 6, 9 (`125-148`, `136-138`, curve checks vs `Secp256k1.sol`) | VAL-001 (body) | covered |
| `crates/validator/src/frost/error.rs` | 46 | R4 | 100 % | listed only; R6 rejected 1 cites `10-20` (Display carries no secret bytes) | — | thin |
| `crates/validator/src/state/keygen.rs` | 1459 | R4 | 100 % | seven findings anchored; rejected 2, 3, 5, 8, 10, 11, 13–15; O1, O3, O9 | VAL-001/003/004/005/006/007/031/032/061/063/067, XC-009, **XC-050** | covered |
| `crates/validator/src/consensus/mod.rs` | 5 | R4 | 100 % | listed only | — | thin |
| `crates/validator/src/consensus/group.rs` | 459 | R4 | 100 % | encoding parity (`352-364`), rejected 1 (`186-208`, `368-377`); O4; R8 census (`146`, `231-237`) | XC-009 | covered |
| `crates/validator/src/consensus/epoch.rs` | 95 | R4 | 100 % | E1 test run only; no hypothesis. F2-VAL-067 (R6) discusses it | VAL-067 (body) | thin |
| `crates/validator/src/frost/preprocess.rs` | 189 | R5 | 100 % | rejected R3 (`134-141`, `164-172`), R7 (`28-33`), R9 (`112-121`), R22 (`117`) | — | covered |
| `crates/validator/src/frost/sign.rs` | 204 | R5 | 100 % | rejected R1 (`60-82`), R3 (`129-136`, `165-179`); E1 `cast` leaf recomputation | — | covered |
| `crates/validator/src/merkle.rs` | 142 | R5 | 100 % | rejected R3, R4 (`13-29`, `49-59`, `87-93`); O8 | — | covered |
| `crates/validator/src/secrets/mod.rs` | 6 | R5 | 100 % | listed only | — | thin |
| `crates/validator/src/secrets/nonces.rs` | 348 | R5 | 100 % | rejected R10 (`194-218`), R22 (`36-39`, `100-121`); O5; E1 3 tests | VAL-030/031/062 | covered |
| `crates/validator/src/secrets/store.rs` | 969 | R5 | 100 % | pruning diff and pre-merge version read; rejected R1, R5, R17–R21, R23; O1, O6, O9; E1 13 tests; F2-VAL-034 anchored | VAL-005/031/034, XC-005/006 | covered |
| `crates/validator/src/state/preprocess.rs` | 253 | R5 | 100 % | rejected R7 (`185-199`), R11 (`79`), R24 (`194`, `234`); F2-VAL-030 anchored | VAL-007/030/061/062 | covered |
| `crates/validator/src/state/sign.rs` | 868 | R5 | 100 % | rejected R1, R2, R8, R12–R16, R25 with lines; F2-VAL-032/033 anchored | VAL-007/032/033/061 | covered |
| `crates/validator/src/state/transactions.rs` | 101 | R5 | 100 % | listed; §6 notes "no test at all"; no hypothesis. R6 O4 cites `32-40`; F2-VAL-032 related `52-73` | VAL-032 | thin |
| `crates/validator/src/consensus/hashing.rs` | 249 | R5 | 100 % | rejected R6: every type hash and vector recomputed with `cast` (E1) | — | covered |
| `crates/validator/src/state/mod.rs` | 516 | R6 | 100 % | dispatch and `NewBlock` ordering; rejected 4, 8; F2-VAL-061 anchored (`415-460`) | CORE-004, VAL-003/006/031/032/061/062, XC-005 | covered |
| `crates/validator/src/service/action.rs` | 381 | R6 | 100 % | O1 (gas constants `276-299`), O2 (`252-254`, `368-378`); rejected 9 | — (R5 O10 body) | covered |
| `crates/validator/src/service/effect.rs` | 328 | R6 | 100 % | pruning diff read; rejected 1, 5, 8; O3; F2-VAL-062/063 anchored | VAL-030/031/034/062/063 | covered |
| `crates/validator/src/service/mod.rs` | 129 | R6 | 100 % | rejected 11 (`51-57` `InvalidValidators`) | VAL-061/067 | covered |
| `crates/validator/src/bindings.rs` | 247 | R6 | 100 % | E1 `cast sig-event`/`cast sig` over 17 events and 18 functions vs compiled artifacts; rejected 3 | VAL-006, CORE-010 (body) | covered |
| `crates/validator/src/config.rs` | 290 | R6 | 100 % | rejected 2 (E1 serde probe); O4; F2-VAL-067 anchored; E1 `config::tests` | VAL-060/065/067, XC-001/002/007/008 | covered |
| `crates/validator/src/main.rs` | 99 | R6 | 100 % | E1 binary runs (`R6-config-error-leak.txt`); rejected 6, 7; F2-VAL-060/064/066 anchored | CORE-004/031, VAL-060/061/064/065/066, XC-001/006 | covered |
| `crates/validator/src/metrics.rs` | 181 | R6 | 100 % | pruning diff read; checklist 5 answer (static labels) | CORE-067 (body) | covered |
| `crates/validator/validator.sample.toml` | 77 | R6 | 100 % | O5 (`:15` well-known key); F2-VAL-064/065 anchored; R8 100 % | VAL-064/065, XC-002/008 | covered |
| `crates/validator/Dockerfile` | 37 | R6 | 100 % | F2-VAL-068 anchored; R8 100 % | VAL-064/068, XC-003/005/009 | covered |

### 1.3 `crates/sentinel` (R7)

| File | Lines | Rev | Claimed | Evidence in log | Findings touching | Verdict |
| --- | --: | --- | --- | --- | --- | --- |
| `crates/sentinel/src/action.rs` | 43 | R7 | 100 % | listed only; R8 §4.1/O9 cover the `Reveal { salt }` `Debug`/persistence question | — | thin |
| `crates/sentinel/src/bindings.rs` | 173 | R7 | 100 % | diff vs `2893917` (new events); rejected 13, 19 (`14-21` enum order) | SEN-004, CORE-004/010 (body) | covered |
| `crates/sentinel/src/config.rs` | 144 | R7 | 100 % | tests read; O9, O10; F2-SEN-009 anchored (`54-56`, `62-66`) | SEN-006/009, XC-001/007/008 | covered |
| `crates/sentinel/src/effect.rs` | 134 | R7 | 100 % | rejected 3 (`57-68`) | CORE-035, SEN-008 | covered |
| `crates/sentinel/src/engine.rs` | 392 | R7 | 100 % | rejected 4; O1–O3; F2-SEN-008 anchored; R8 rejected 15, 16 | SEN-008 | covered |
| `crates/sentinel/src/hashing.rs` | 224 | R7 | 100 % | rejected 2 (`31-37` vs `SentinelOracleCommitments.sol:55`), 8 (`49-53`); E1 parity tests | — | covered |
| `crates/sentinel/src/main.rs` | 89 | R7 | 100 % | O9, O10 (`:39`); F2-SEN-006 anchored (`37-86`) | CORE-031/035, SEN-006/008, XC-001/006/008 | covered |
| `crates/sentinel/src/metrics.rs` | 143 | R7 | 100 % | O5 (`Timeout` label semantics), O8 (casts feeding it) | CORE-067 (body) | covered |
| `crates/sentinel/src/service.rs` | 2158 | R7 | 100 % | 16 flow tests read; rejected 3, 5–7, 9–11, 13, 14, 17, 18; O5, O11, O13, O14; nine findings anchored | CORE-030/032/035, SEN-001–008, **XC-050** | covered |
| `crates/sentinel/src/state.rs` | 372 | R7 | 100 % | rejected 5 (`38-55`), 14 (`118-141`), 19 | SEN-002/003/004 | covered |
| `crates/sentinel/sentinel.sample.toml` | 61 | R7 | 100 % | F2-SEN-009 anchored (`:10`, `:16`, `33-35`); R8 100 % | SEN-009, XC-002/008 | covered |
| `crates/sentinel/Dockerfile` | 38 | R7 | 100 % | F2-SEN-009 row 6 (`:5`, `25-38`); R8 100 % | SEN-009, XC-003 | covered |

### 1.4 Non-inventory files (R8)

`Cargo.toml` (100 %), `Cargo.lock` (~1 % read, analysed with `cargo audit`/`cargo tree`, logs `r8-tree-i.txt`, `r8-tree-features.txt`), the three crate `Cargo.toml`s (100 %), both `Dockerfile.dockerignore` files, CI workflows (reference). Findings: F2-XC-003/004/009. R8's sweeps (secrets census, panic/cast census, config E1 runs) touched all 61 `.rs` files mechanically; its per-file read percentages are in `R8.md` §2 and are **not** counted as coverage here.

### 1.5 Counts

| Verdict | Files | Which |
| --- | --: | --- |
| covered | 57 | 53 `.rs` + 2 sample configs + 2 Dockerfiles |
| thin | 8 | `core/src/lib.rs`, `core/src/observability/logging.rs`, `validator/src/consensus/mod.rs`, `validator/src/secrets/mod.rs`, `validator/src/frost/error.rs`, `validator/src/consensus/epoch.rs`, `validator/src/state/transactions.rs`, `sentinel/src/action.rs` |
| unverified | 0 | — |

The eight `thin` files total 372 lines (1.8 % of the inventory); five are declaration-only modules under 50 lines. The two with logic (`consensus/epoch.rs`, `state/transactions.rs`) are the ones a QA agent should exercise (§7).

## 2. Seeded leads versus the logs

Every "Start from" entry of `reviewer-split.md` was checked against the owning log's lead-disposition table, rejected list and findings.

| Reviewer | Leads examined (finding / rejected / observation) | Not examined |
| --- | --- | --- |
| R1 | H1→001, H2→002, H4→004, H8→005, H9→006, H14→008; core checklist 1 (001/007), 3 (002/003, O1, O7), 4 (004), 8 (005/008), 9 (006) | none |
| R2 | H3→031, H5→032, H10→O5 (inverse filed as 030), H12→033/034, M9→033, M10→rejected with citations; checklist 2, 10, 11, 12 (O1/O2/O12), 14 (O8). **H13 declared "not assessed here"** (R1's files) — examined by R1 as O3 (~40 %, Low) and O4 | none |
| R3 | H6→060 (compounding confirmed, cap sub-claim refuted in R7), H7→062, H11→061; checklist 5 (063/064), 6 (060/062), 7 (061) | none |
| R4 | H1→001, H4→005 + rejected 15 (A16), H5→002, H7→rejected 3, H9→rejected 2, M1→rejected 1, M3→O7/002; validator checklist 1, 3 (004/005), 7 (006), 8 (O4), 12 (004) | none |
| R5 | H3→030/031, H6→032, H8→033, H11→O7 (A17), M4→rejected R3, M5→rejected R1, M6→rejected R5, M7→rejected R9; checklist 3, 4, 5, 10 (§6) | none |
| R6 | H2→061, H7→rejected 5/O3, H10→rejected 1, CORE-H4→061; checklist 2, 6, 9, 11 (O1), 13 | none |
| R7 | H1→002, H2/H10→001, H3→003 + rejected 14, H4→005, H5/H12→006, H6→004, H7/H13→007, H8→O6/O7, H9→O12, H11/H14→008, H15→009; sentinel checklist 1–13 each mapped. **M8 is logged as "n/a to this crate's files"** — wrong label (M8 is `sentinel/src/hashing.rs:16-40`), but its substance is rejected 5 | none |
| R8 | VAL-H10→refuted E2 (§4.1), SEN-H14→O2 (half refuted with feature-tree evidence), SEN-H15→XC-007, CORE-H17→XC-006; manifests, sweeps, audit reachability (XC-004), CI gaps (XC-004, O3) | none |

Leads that the split assigned to nobody but that the map lists: CORE-H15 (`fallible_events`) — covered by R1 O14; CORE-H16 (HKDF info concatenation) — covered by R2 O9 and rejected 13; core checklist 13 (clock) — R1 O3/O4; 15 (config validation) — R3 F2-CORE-065, R1 O4; 16 (offline dependency assumptions) — resolved by R2/R3/R8 registry reads at the pinned versions; validator checklist 14 (test debt) — R4 O5, R5 §6. The cross-cutting checklist (map §5, items 1–10) is answered across the logs: 1 (R8 census + R1 rejected 3–6, R3 R16, R4 11–12, R7 12), 2 (every log), 3 (R7 rejected 1, R6 rejected 2/10, R1 O6), 4 (R5 R6 E1, R4 parity answers, R7 rejected 2), 5 (R8 §4), 6 (R1 7, R2 5/6/9, R5 R23), 7 (CORE-010/035, SEN-005), 8 (R6/R8 E1), 9 (R8 §6), 10 (R2 §4, R4 O5, R5 §6).

**Unexamined seeded leads: none.** Two bookkeeping defects in the logs (R2's H13 hand-off, R7's M8 label) do not leave a lead unexamined.

## 3. Seams

For each seam I opened the code myself; "examined by" names the logs whose rejected lists, observations or findings actually reason about the crossing.

### 3.1 R1↔R2 — restart replay and block warp (`index/blocks.rs` ↔ `driver.rs`, `state/mod.rs`, `state/storage.rs`)

Code: `initialize` (`blocks.rs:244-289`) queues `Uncle { safe+1 }` only when a newer snapshot exists, then `Warp { safe+1 .. node_safe }`, then `New` for `recent`; `status()` derives `latest` from `pending` (`376-381`) and the queue is drained before polling (`385-389`). The state machine accepts `Warp` from `Initialized` or `BlockPending { pending == from }` and `Uncle` only below `pending` (`state/mod.rs:173-189`); `prune` keeps `>= safe` plus `MAX` (`storage.rs:151-161`); the driver prunes with the watcher's status captured before `handle_update` (`driver.rs:242, 263`).

Examined by both sides: R1 (F2-CORE-001/007, rejected 16, 22, 23, O12; read `driver.rs:165-240`, `state/mod.rs:96-262`, `storage.rs:44-165`) and R2 (F2-CORE-030, rejected 4, 8, O3, O4; read `blocks.rs` excerpts), plus R3 (rejected R1: warp keeps the real head; O2), R4 (F2-VAL-003: warp delivers no `NewBlock`), R5 (O2), R7 (rejected 14, 16). The one consequence both R2 and R3 saw and neither filed — the single-snapshot state during a warp — is filed by me as **F2-XC-050** (§5). Verdict: examined.

### 3.2 R2↔R3 — effect manager / state machine ↔ transaction queue hand-off and replay

Code: `driver.rs:272-290` splits commands into actions (encoded, then `transactions.queue`, i.e. `enqueue` INSERT then `submit_pending`) and effects (`effects.spawn`); `queue()` submits only once `block_status` is set (`tx/mod.rs:132-141`); `update_block_status` runs before `handle_update` (`driver.rs:249`) and unmarks past `safe` on startup (`tx/mod.rs:167-179`); `is_intermittent` is `Rpc` only (`47-54`).

Examined by both: R2 (F2-CORE-032, rejected 18, M10 with citations, O4) and R3 (F2-CORE-063/064, rejected R1, R11–R13, O2, O11). **F2-CORE-032 and F2-CORE-063 describe one defect** (replayed actions re-enqueued without an idempotency key); the Critic for R2+R3 should name the canonical one. The crash-window between commit and enqueue was noted by both (R2 O4 45 %, R3 O2 35 %) and by neither filed — **F2-XC-050**. Verdict: examined; one duplicate pair; one gap now filed.

### 3.3 R4↔R5 — DKG output → secret store; `frost/mod.rs` shared types

Code: `KeyGenSetup` → `frost::keygen::setup` → `store_keygen_secrets` (UPDATE-then-INSERT in one transaction, never overwrites, clears `delete_at_block`; `store.rs:176-212`) → `Resume::Setup` → `handle_key_gen_setup` (`state/keygen.rs:65-82`, guarded by `group_id` and `secrets: None`). `frost/mod.rs` contains only module declarations and the end-to-end `ceremony` test.

Examined by both: R4 (rejected 1 — same gid reuses the same polynomial, 13, 14; read `store.rs:170-212, 296-411`; O1, O2) and R5 (rejected R17 — no-overwrite test, R18 — retention of keygen rows; read `state/keygen.rs:496-560, 575-640, 1280-1370`); R8 F2-XC-005 covers the plaintext `SharingState` in snapshots; R2 O8 the same. `frost/mod.rs` was read by R4 only (R5 "relied on its existence"); it holds no production code. Verdict: examined.

### 3.4 R5↔R6 — `secrets/store.rs` scheduling/collection ↔ `service/effect.rs` housekeeping wiring

Code: `ReconcileGroupSecrets { block, groups }` (spawned per `NewBlock`, `state/mod.rs:469-470`) → `schedule_group_secrets_deletion` (monotonic marker, `store.rs:311-345`) under the generator lock (`effect.rs:240-256`); `housekeeping(status)` → `prune_scheduled_secrets(status.safe)` (`store.rs:356-375`), awaited inline by the driver right after spawning that block's effects (`driver.rs:292-294`).

Examined by four logs: R5 (rejected R18, R20, R21; O1 — long-outage race, `known` per `store.rs:38-41`; O3, O4), R6 (rejected 5, O3), R2 (§4 hunk audit, rejected 19, O13 for depth 0), R4 (rejected 3, O1). All four converge on the same residual: after a restart longer than `max_reorg_depth`, the first `housekeeping` races the first reconciliation's cancellation; harmful only if the replayed branch differs from the one that scheduled. The module doc acknowledges it (`store.rs:38-41`), so under A12 it is `known` and Informational; not filed. Verdict: examined thoroughly.

### 3.5 R6↔R7 — validator vs sentinel service wiring: recurrences

I read both `main.rs` files in full and both `config.rs` loaders. Recurrence table (V = validator finding, S = sentinel finding, XC = cross-cutting finding covering both):

| Defect | Validator | Sentinel | Status |
| --- | --- | --- | --- |
| Config parse error echoes the file incl. `signer` | F2-VAL-060 (E1) | F2-XC-001 rows 2, 4 (E1 `sen-typo-table`) | covered both; VAL-060 and XC-001 are one defect — Critic to name canonical |
| Exit status 0 after fatal error | F2-VAL-066 | F2-CORE-031 (cites `sentinel/main.rs:85-88`) | covered both; VAL-066 duplicates CORE-031/033 for one crate |
| `/health` constant | F2-VAL-066 | F2-CORE-033 (R2 rejected 16 cites `sentinel/main.rs`) | covered both |
| Address binding of decoded events | F2-VAL-061, F2-CORE-004 | not applicable: `main.rs:80` watches exactly `[oracle, consensus]`, no operator-extensible list (R7 rejected 13, R1 rejected 20; verified by me) | covered |
| `database` URL without `?mode=rwc` | F2-VAL-065 | F2-XC-002 (E1 both) | covered both; VAL-065 ⊂ XC-002 |
| Root runtime image, floating tags | F2-VAL-068 | F2-SEN-009 row 6, F2-XC-003 | covered both; three files for one defect |
| `--config-file=<path>` rejected by `argh` | F2-VAL-064 (E1) | **not filed**; VAL-064 line 36 notes `sentinel.sample.toml:4`, `docs/sentinel-handbook.md:76`; the sentinel Dockerfile comment (`:36`) also uses `=` | same code (`#[argh(option)]`, `sentinel/main.rs:17-27`); recommend the Critic widen F2-VAL-064's Location rather than a second Informational file |
| Sample key `0x…01`, live mainnet RPC | R6 O5, F2-XC-008 | F2-SEN-009, F2-XC-008 | covered both |
| Timing values unchecked | F2-VAL-067 | F2-SEN-006 (`voting_window`), F2-XC-008 | covered both |
| `Config` derives `Debug` | R6 rejected 6 (never logged) | R7 O10 (never logged) | covered both |

Verdict: examined; the R8 cross-cutting findings are what closed this seam. Five duplicate clusters for the Critics to canonicalise (listed in §6).

### 3.6 R1↔R7 — the sentinel's use of core event decoding

Code: `watcher_events!` decodes with `decode_raw_log` (non-validating; `events.rs:577-591`), any failure is `Error::DecodeLog` for the whole batch (`491-516`) and is retried forever (`driver.rs:218-224`). Examined by R7 in depth (rejected 1: alloy 1.6.0 `String` detokenize is `from_utf8_lossy`, registry-cited; rejected 13, 19) and by R1 (rejected 9, 20).

My own addition, from the pinned `alloy-sol-macro-expander-1.6.0/src/expand/enum.rs:40-66`: a `sol!` enum with fewer than 256 variants gets a hidden `__Invalid` variant and `detokenize` is `try_from(u8).unwrap_or(Self::__Invalid)`; only a 256-variant enum uses `expect("unreachable")`. So an out-of-range byte in `Operation` (`validator/bindings.rs:35`, `sentinel/bindings.rs:77`) or `RequestState` (`sentinel/bindings.rs:14`) decodes silently to `__Invalid` on the `decode_raw_log` path — **neither a decode error nor a panic**. Consequences: R6 rejected 10 is right that a protocol contract cannot emit it (solc validates enums on calldata decode), and F2-VAL-061 scenario 3 stands because it uses truncated data (a real `DecodeLog`); but any wording that an out-of-range enum "fails decoding" would be `H`. For the sentinel (two protocol addresses) nothing is reachable. Verdict: examined; one mechanism refinement for the C2-VAL-B and C2-CORE-A Critics.

### 3.7 Additional cross-reviewer thread checked: permissionless `proposeTransaction` spam (R1 O13 → R4/R5/R6)

`Consensus.proposeTransaction` is `public`, calls `_COORDINATOR.sign(...)` and `IOracle(oracle).postRequest(message, msg.sender, oracleData)` (`Consensus.sol:253-267`); `postRequest` pulls `currentFee` from the sponsor (`SentinelOracle.sol:208-225`). On the validator, `Sign` for a `Packet::Transaction` only moves the session to `WaitingForOracle` (`state/sign.rs:45-71`) — no action or effect is emitted until an approved `OracleResult` (`172-221`) — and the session requires `oracles.contains(event.oracle)` (`state/transactions.rs:32-40`). So a proposal costs the attacker a fee plus gas and costs each validator one nonce sequence (`observe(event.sequence)`), i.e. 1/1024 of a `Preprocess` transaction, not a gas drain; the sequence-burning half is F2-VAL-030/032 (R5), the sentinel half is F2-SEN-005 (R7). Verdict: examined across R1, R5, R7; no new finding.

## 4. Spot-reads of the `thin` files

All eight read in full by me at `3ec8bc5`.

- `crates/core/src/lib.rs` (25): module declarations and `pub use Driver`; `metrics` is private. Nothing to find.
- `crates/core/src/observability/logging.rs` (21): registry + `EnvFilter` from config (A1); JSON when stdout is not a terminal. Nothing to find.
- `crates/validator/src/consensus/mod.rs` (5), `crates/validator/src/secrets/mod.rs` (6): declarations only.
- `crates/validator/src/frost/error.rs` (46): `Display` prints culprit address and the `frost_secp256k1::Error`; no secret bytes flow through it (agrees with R6 rejected 1). The `InternalError(String)` path in `service/effect.rs:321-328` is the only consumer.
- `crates/validator/src/consensus/epoch.rs` (95): `next_number = 1 + block / blocks_per_epoch` (saturating); `EpochId` orders by raw value and (de)serialises as `u64`; `Genesis == 0`. Consistent with its callers in `state/keygen.rs`; nothing to find.
- `crates/validator/src/state/transactions.rs` (101): `handle_transaction_proposed` gates on a participating epoch and the oracle allow-list, opens `WaitingForRequest` keyed by `transaction_packet_hash`, refuses to reset an existing session (the comment says why). Memory is bounded by `signing_timeout` blocks of proposals (R5 rejected R13/R14 cover the cleanup). `handle_transaction_attested` recomputes the message with `transaction_proposal_hash(…, oracleDataHash, safeTxHash)` and delegates. Nothing new; the file has no tests (R5 §6) — QA candidate.
- `crates/sentinel/src/action.rs` (43): action enum with `Reveal { salt }` deriving `Debug`; R8 §4.1/O9 already established that no sentinel tracing site formats an action with `?`. Nothing new.

## 5. Findings filed by the Coverage Critic

| ID | Title (short) | Severity (mine) | Self-estimate | Seam |
| --- | --- | --- | --: | --- |
| **F2-XC-050** | Snapshot committed before actions are enqueued/effects spawned; with a single retained snapshot (every warp page during catch-up, or `max_reorg_depth = 0`) a crash or fatal storage error in the window loses the page's actions and effects with no replay | Low | 55 % | R2↔R3 |

Overlap check done against F2-CORE-030 (resumes discarded by rollback; anchor-block effects), F2-CORE-032/063 (duplicate enqueue — the opposite failure), F2-VAL-005/063 and F2-SEN-001 (service-level effect loss). Row 4 of the finding is E1 (existing test `warps_and_prunes_intermediate_snapshots`, run this session, log `state/run2/logs/CC-core-state-tests.txt`).

Not filed, with reasons: the R5 O1 housekeeping race (`known`, module doc); R2 O10 snapshot schema versioning (no trigger beyond an upgrade, and F2-CORE-066 already asks for a schema version); the SQLite rollback-journal/`SQLITE_BUSY`-is-fatal thread (R2 O1, R3 O1, R5 R23/O6 — no lock holder above 5 s identified by anyone); the sentinel `--config-file=` recurrence (same Informational defect as F2-VAL-064).

## 6. Notes for the Critics and the Manager

- Duplicate clusters to canonicalise (one defect, several files): F2-CORE-032 ≡ F2-CORE-063; F2-VAL-060 ≡ F2-XC-001; F2-VAL-065 ⊂ F2-XC-002; F2-VAL-068 ≡ F2-XC-003 (≡ F2-SEN-009 row 6); F2-VAL-066 ⊂ F2-CORE-031 + F2-CORE-033; F2-VAL-062 overlaps F2-VAL-030 (both VAL-H3: R5 from `state/preprocess.rs`, R6 from `service/effect.rs`); F2-VAL-063 overlaps F2-VAL-005 (lost `KeyGenSetup`, R6 and R4).
- Mechanism refinement for F2-VAL-061 (C2-VAL-B) and F2-CORE-004 (C2-CORE-A): out-of-range `sol!` enum values decode to `__Invalid`, not to a `DecodeLog` error (§3.6).
- F2-VAL-064's Location should include `crates/sentinel/src/main.rs:17-27`, `crates/sentinel/sentinel.sample.toml:3-5`, `crates/sentinel/Dockerfile:34-38`, `docs/sentinel-handbook.md:76`.
- Log bookkeeping: R7 mislabels M8 as not applicable (its rejected 5 is the M8 check); R2 declares CORE-H13 not assessed (R1 O3/O4 hold it).

## 7. Recommended QA exercises (files and seams)

1. **F2-XC-050 window** (core): a driver-level test with a mock service that aborts (or whose `enqueue` fails) after the warp page's `commit` + `prune`; assert the page's actions are absent after restart. Cheapest route is extending `crates/core/src/state/mod.rs::warps_and_prunes_intermediate_snapshots`.
2. **R1↔R2 restart replay with a file-backed database** (core, Anvil): stop/start across a reorg inside `max_reorg_depth` and across a longer outage; observe the `Uncle`/`Warp` sequence, the duplicate submissions (F2-CORE-032/063), and the unverified anchor (F2-CORE-001).
3. **R5↔R6 housekeeping vs reconciliation on restart** (validator, Anvil): schedule a group's deletion, stop for more than `max_reorg_depth` blocks, restart, and log whether `prune_scheduled_secrets` runs before the first reconciliation's cancellation (R5 O1); also `max_reorg_depth = 0` (R2 O13).
4. **Sentinel catch-up with terminal events** (sentinel, Anvil): bond, stop, let the request time out or be disputed, restart after more than `max_reorg_depth` blocks; confirm `Claim` is emitted from the warp page (F2-SEN-003/004/007 and the F2-XC-050 window).
5. **`state/transactions.rs` and `consensus/epoch.rs`** (validator): no unit tests exist for either; a proposal → `Sign` → `OracleResult` flow test and an `EpochId` ordering/serde round-trip would turn the two logic-bearing `thin` files into tested ones.
6. **Enum decode behaviour** (core/validator): a unit test feeding `decode_raw_log` a `TransactionProposed` log with `operation = 2` and asserting `Operation::__Invalid` (not an error), to pin the §3.6 refinement for F2-VAL-061.
7. **`--config-file=` for the sentinel** (trivial E1): run the sentinel binary with `--config-file=<path>` to extend F2-VAL-064 to both binaries.

## 8. QA2-XC results for section 7

Recorded by QA2-XC (resume) at `fe9e84c`; `crates/core` and `crates/validator` are byte-identical to `3ec8bc5`, `crates/sentinel` is the post-#914 tree. Every test below asserts the behaviour it observed (passing = the observation holds); sources and logs under `poc/`.

| # | Item | Done | Where | Outcome |
| --- | --- | --- | --- | --- |
| 1 | F2-XC-050 window | yes | `poc/F2-XC-050/` (`run.txt`, `rerun.txt`) | Reproduced: fault-injected enqueue after commit + prune loses the page's action on a warp page and at depth 0; depth-2 control recovers it through `Uncle`. F2-XC-050 → 92 %, canonical over F2-CORE-036. |
| 2 | Restart replay, file-backed DB, Anvil | yes | `poc/F2-XC-050/coverage-7.2/` (`run.txt`, `rerun.txt`) | Four runs of the real `BlockWatcher` + `StateMachine` + `TransactionQueue` over one file against local Anvil (8747): fresh start → `New` only; depth-1 reorg during downtime → `Uncle { 10 }` + replay, the two dropped transactions re-enqueued next to the still-pending rows; 10-block outage → `Uncle { 10 }, Warp { 10..=19 }`, 12 actions; a depth-4 reorg replacing the persisted `safe` anchor (19) → start proceeds with `Uncle { 20 }, Warp { 20..=21 }` and **no** `ExceededMaxReorgDepth` (F2-CORE-001, executed). Duplicates mined on chain: calldata `…0a` ×3, `…0b`, `…14`, `…15` ×2 each (F2-CORE-032/063). No new finding; all three are QA'd by QA2-CORE. |
| 3 | Housekeeping vs reconciliation on restart (R5 O1, R2 O13) | partly | `poc/F2-XC-050/coverage-7.3/` (`qa2_cov_7_3_effect_test.rs`, `run.txt`) | Executed at the seam with the real `Handler`, `SecretStore` and core `EffectManager`, under R5 O1's injected precondition (old branch scheduled GROUP at 100; replayed branch retains it at 105; first live block `{ latest: 110, safe: 105 }`): the two orders decide the result (`order A (reconcile, then housekeeping): row = Some(None)` retained; `order B (housekeeping, then reconcile): row = None` lost), and the driver's actual construction — `effects.spawn(ReconcileGroupSecrets)` then inline `housekeeping(status)` (`driver.rs:278, 292-294`) — on a 2-worker runtime: **`secret lost in 19, retained in 1` of 20 rounds**. The prune on the driver task almost always beats the spawned reconciliation's cancellation. Not done: the Anvil restart with a real group drop/reorg during the outage (needs a multi-validator devnet and an epoch rollover across a reorg) and the depth-0 variant (R2 O13; code-traced: `housekeeping({ safe: n })` runs before snapshot `n` is committed). **For the Manager/Critic:** R5 O1 is filed at 35 % / `known`; the executed part shows that whenever its precondition holds the loss is the common outcome, not a coincidence — worth a promotion decision (QA does not file). |
| 4 | Sentinel catch-up with terminal events | yes | `poc/F2-XC-050/coverage-7.4/` (`qa2_cov_7_4_sentinel_test.rs`, `run.txt`; scratch copy of the workspace, tracked `service.rs` was in use by another agent) | Three bonded requests, stop at `{ latest: 110, safe: 105 }`, terminal events `RequestTimedOut`, `DisputeTriggered`+`ArbitrationTimedOut`, `OracleResult` during the outage: `Uncle { 106 }`, `Warp { 106..=195 }`, page → three `Claim { … } expires_at: None` **from inside the page**, entries gone from committed snapshot 195; `prune(195)` → `[195]`; next start `latest == safe == 195`, no `Uncle`, `claims re-emitted = []`. Confirms F2-SEN-003/004/007's premise that the catch-up page is where the claims are produced, and executes F2-XC-050 row 9. |
| 5 | `state/transactions.rs`, `consensus/epoch.rs` unit tests | no | — | Test-gap item, no defect claim; not assigned to a QA agent. |
| 6 | Enum decode `__Invalid` | yes | `poc/F2-CORE-004/enum-decode/` (`run.txt`, `rerun.txt`) | `operation byte 2 decoded to __Invalid`; `decode_raw_log` returns `Ok`, the validator's `Event::decode_log` accepts the log, truncated data is still an error (F2-VAL-061 scenario 3 stands). Refinement for F2-VAL-061 / F2-CORE-004 pinned. |
| 7 | Sentinel `--config-file=` | yes | `poc/F2-VAL-064/sentinel/` (`run.txt`, `rerun.txt`) | `Unrecognized argument: --config-file=…` / exit 1 for the sentinel built from `fe9e84c`; space-separated form parses; identical in the validator. F2-VAL-064 covers both binaries. |
