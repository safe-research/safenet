# Audit state (run 2)

| Field | Value |
| --- | --- |
| Commit | `fe9e84cc59b65367b31d5a3121774383cc422234` (merge of `origin/main` `8b6a75d` into `audit/rust-services`, local, **not pushed**); Phases 0–3 so far were run at `3ec8bc5`, whose `crates/core` and `crates/validator` trees are byte-identical to this commit — only `crates/sentinel` (`service.rs`, `bindings.rs`), the reference contracts and the workspace manifests changed (PR #914 deadline handling, #939–#945 oracle-audit fixes, engine phases out of scope) |
| Mode | full |
| Phase | 4 — Documentation (REPORT.md rewrite) running; reconciliation complete |
| Gate status | Gate 3 passed (`continue`) |

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
| R1 | Reviewer | core indexing and reorgs — 6 files, 4,106 lines (`baseline.md` Section 4) | done | `state/run2/agents/R1.md`, `findings/F2-CORE-*.md` |
| R2 | Reviewer | core runtime, state, effects, observability — 12 files, 2,055 lines | done | `state/run2/agents/R2.md`, `findings/F2-CORE-*.md` |
| R3 | Reviewer | core transaction queue — 5 files, 1,540 lines | done | `state/run2/agents/R3.md`, `findings/F2-CORE-*.md` |
| R4 | Reviewer | validator DKG path — 10 files, 3,228 lines | done | `state/run2/agents/R4.md`, `findings/F2-VAL-*.md` |
| R5 | Reviewer | validator signing path and secrets — 10 files, 3,329 lines (+522 unmapped in `secrets/store.rs`) | done | `state/run2/agents/R5.md`, `findings/F2-VAL-*.md` |
| R6 | Reviewer | validator service, wiring, config — 8 `.rs` + sample config + Dockerfile, 2,285 lines | done | `state/run2/agents/R6.md`, `findings/F2-VAL-*.md` |
| R7 | Reviewer | sentinel — 10 `.rs` + sample config + Dockerfile, 3,971 lines (+524 unmapped in `service.rs`/`state.rs`) | done | `state/run2/agents/R7.md`, `findings/F2-SEN-*.md` |
| R8 | Reviewer | cross-cutting: manifests, `Cargo.lock`, `cargo audit`/`tree -d` results, secrets/panic/cast censuses, CI gaps (no engine); labelled R8 in run 2 (`reviewer-split.md`) | done | `state/run2/agents/R8.md`, `findings/F2-XC-*.md` |
| C2-CORE-A | Critic | `F2-CORE-001..010` (R1) | done — 10 Confirmed (70–80), 0 Refuted; 002 held at Medium (degraded RPC is A4), 003 → Informational; promoted `F2-CORE-011` (Draft, Low: no snapshot at the rollback anchor on a fresh start → `MissingSnapshot` exit); alloy `__Invalid` note checked for F2-CORE-004: no Basis row depends on decode failure being an error, no `H`, verdict unchanged | Critic sections in finding files |
| C2-CORE-B | Critic | `F2-CORE-030..035`, `F2-CORE-060..067` (R2, R3) | done — 12 Confirmed, 2 Plausible (061, 062), 0 Refuted; 030 High 80 with an extended deterministic trigger (own `Committed` applied before effect spawn on replay) — canonical for the core defect, `F2-SEN-001` for the sentinel loss; 031 → Low; 032/063 one defect, 063 canonical; 060 cap sub-claim: R3's refutation arithmetically right; promoted `F2-CORE-036` (Draft, Low, 45: commit-then-prune-then-enqueue loses a warp page's actions on crash) — **same defect as Coverage Critic's `F2-XC-050`**, canonical to be named by C2-XC | Critic sections in finding files |
| C2-VAL-A | Critic | `F2-VAL-001..007` (R4) | done — 5 Confirmed, 1 Plausible, 0 Refuted; F2-VAL-001 Critical 85 (all links code-verified; PoC for QA); F2-VAL-002 property confirmed, standalone Low 70; F2-VAL-005 duplicate → canonical `F2-VAL-063` | Critic sections in finding files |
| C2-VAL-B | Critic | `F2-VAL-030..034`, `F2-VAL-060..068` (R5, R6) | done — 8 Confirmed, 1 Plausible (063), 5 Info confirmed, 0 Refuted, no `H`; 030 and 031 High → Medium (single validator, self-inflicted restart, not attacker-triggerable); 032 stays High 80 (attacker-owned group can call `sign(gid,msg)` and drop honest rollover sessions); 060 High 92 (E1 reproduced); 064 95 (E1) and identical in the sentinel binary; 061 no `H` (truncated-log scenario genuinely fails `abi_decode`); canonicals 030 over 062, 063 over 005, 066 → CORE-031, 065 → XC-002, 068 → XC-003 | Critic sections in finding files |
| C2-SEN | Critic | `F2-SEN-001..009` (R7) | done — 8 Confirmed, 1 Plausible (005), 0 Refuted; 001 High 82 (real irreversible slash under routine restart/depth-1 reorg), 002 High 80 (locked not slashed; parkable by a registered sentinel); 009 → canonical `F2-XC-008`; no promotions | Critic sections in finding files |
| C2-XC | Critic | `F2-XC-001..009` + `F2-XC-050` | done — 10 Confirmed, 0 Refuted; 001 → High 95 (E1, three runs), 002 E1 92, 004 E1 85 (h2 has no connection path at all — exporter is HTTP/1, reqwest has no `http2`); canonicals: XC-001 over VAL-060, XC-002 over VAL-065, XC-003 over VAL-068/SEN-009 b4, XC-007/008 over SEN-009 b2/b1, **XC-050 over CORE-036**; no promotions; `--config-file=` E1 extended to the sentinel (log `c2-xc-config-file-flag.txt`) | Critic sections in finding files |
| Coverage Critic | Coverage Critic | every in-scope file vs reviewer logs | done — 57 covered / 8 thin (all spot-read, no defect) / 0 unverified; 7 seams examined, 0 seeded leads unexamined; filed `F2-XC-050` (Draft, Low, snapshot committed before actions enqueued — crash window with single retained snapshot); `coverage.md` §6 flags an alloy `__Invalid` enum-decode refinement for F2-VAL-061/F2-CORE-004 and §7 lists 7 QA items | `state/run2/coverage.md`, `findings/F2-XC-050.md`, `state/run2/logs/CC-core-state-tests.txt` |
| QA2-CORE | QA | core findings per Phase 3 plan; Anvil 8545–8547 | done — 11 PoC dirs, 22 tests all pass (= defect present); Reproduced: 030 → 92 (reorg, restart, ordering, real `BlockWatcher`), 001 → 90, 002 → 90, 005 → 90, 060 → 92 (60 bumps → ×311.9 max fee), 011 → 92, 063/064/065 → 90; 061 → 55 and 062 → 65 stay Plausible (trigger narrowed: gap closes unless an action is queued within retention; node wording unexecuted); seven remediation options flagged unsound or needing repair; tree clean of its hunks, no Anvil used | `poc/F2-CORE-*/`, QA sections |
| QA2-SEN | QA | sentinel findings; Anvil 8548–8549 | done — 8/8 Reproduced at `StateMachine` level (12 PoC tests, all fail as predicted): 001 → 92, 002 → 90, 003 → 90, 004 → 90, 007 → 90, 008 → 90, 006 → 82, 005 → 68, then re-judged by C2-SEN after QA: **Confirmed (branch ii), 85, Medium** (E1 for the sentinel-side mechanism; flood feasibility and onchain slash traced, not executed); C2-SEN concurs option 3 unsound in 001 and 004; duplicate `commit` reverts `AlreadyCommitted` (`Commitments.sol:92`), no funds move; unsound remediations flagged in 001 (option 3) and 004 (option 3); two fixes compiled in a scratch copy, 42 tests still pass; tree clean, no Anvil used | `poc/F2-SEN-*/`, QA sections |
| QA2-VAL-A | QA | `F2-VAL-001` Critical PoC, 002, 003, 006; Anvil 8645–8647 | done — **F2-VAL-001 Reproduced (E1): pure frost test recovers A's full signing share from public ciphertexts + complaint reveals, `s_a_recovered == s_a_ref`; 85 → 95, Critical**; 002 → 90 (two-time pad shown), 003 → 85, 006 → 82; 004/007 Not attempted with unexecuted sketches (50/65 unchanged); all remediations sound — PoP fix needs a coordinator/`keyGenChallenge` change, does not break honest late joiners, pair with KDF-bound pad; tree clean, no Anvil used | `poc/F2-VAL-*/`, QA sections |
| QA2-VAL-B | QA | `F2-VAL-030..033`, `061..063`, `067`; re-save E1 for 060/064; Anvil 8648–8649 | **stopped by operator request** before writing QA sections — PoC dirs with `run.txt` exist for 030–034, 060–064, 067; relaunched as QA2-VAL-B (resume) | `poc/F2-VAL-*/`, QA sections |
| QA2-XC | QA | `F2-XC-050`, `006`, `009`, `005`, re-save 001/002/004 E1; Coverage §7 leftovers; Anvil 8745–8749 | **stopped by operator request** before writing QA sections — PoC dirs exist for 001, 002, 004, 005, 006, 009, 050; its stranded test paste in `crates/core/src/index/blocks.rs` (F2-XC-009 PoC) was reverted by the Manager, diff kept in the session scratchpad; relaunched as QA2-XC (resume) | `poc/F2-XC-*/`, QA sections |
| Recon-Δ | Recon | five baseline commands, inventory and anchor drift at `fe9e84c` | done — exit codes identical (0/0/0/1/0); sentinel tests 42 → 44 (in-scope 185); advisories and 14 version splits byte-identical; lock delta engine-only (`bitflags` already reachable via `tower-http`, `metrics` already direct) — no in-scope reachability change; anchors moved in 13 findings (`baseline-delta.md` §3), F2-CORE-030 / F2-SEN-001 cite old `service.rs:396` whose text changed | `state/run2/baseline-delta.md`, `logs/10–14` |
| R7Δ | Reviewer | sentinel + contracts delta `3ec8bc5..fe9e84c`; re-validate `F2-SEN-001..009`; new `F2-SEN-010+` | done — 001/003/004/005/006/007/008/009 Still valid (anchors re-based; 004 claim 5 superseded by the documented non-revealer slash; 007 duplicate commit now rejected via `vote == NONE`, same `AlreadyCommitted`), **002 Changed** (trigger broadened by the deadline-block tally gap); new **`F2-SEN-010` High 85** (commits mined in block `commitDeadline` never tallied: the `CollectingVotes` switch now runs at `NewBlock(commit_deadline)` before that block's logs → early local finalize, `FinalizeTooEarly` parks forever, one late committer stalls every honest sentinel) and `F2-SEN-011` Info 80 (arbitration-timeout doc/metric assume full refund); no action-boundary off-by-one; `NewRequest` ABI absorbed; 44 tests pass | `## Re-validation` sections, `state/run2/agents/R7-delta.md` |
| QA2-SEN-Δ | QA | re-run the 12 QA2-SEN PoC tests at `fe9e84c` | done — 001/002/003/004/006/007/008 **Same** (byte-identical or identical after a labelled block-number adaptation of the setup: reveal now fires at the deadline block, 120 not 121); **005 Different — narrowed, not fixed**: with the reveal at `p+CW` the model gives 0 slashable at CW=RW for R≤16 but still starves for RW<CW and R>16; 85 → 78, Medium unchanged; no signature repairs needed; tree clean | `poc/F2-SEN-*/…rerun-fe9e84c.txt`, QA sections |
| QA2-VAL-B (resume) | QA | finish QA sections for 030–034, 060–064, 067 from the saved runs, re-run once | done — 13 findings QA'd: Reproduced 032 → 92, 061 → 90, 030 → 90 (062 → 85), 033 → 88, 034 → 85 (correction: error is `no such column: delete_at_block`), 067 → 75, 060 → 95, 064 → 95, 065 → 88 (new PoC); partial 031 → 80, 063 → 69 (Plausible cap); 066 Not attempted (sketch); six remediation options judged unsound (030 opt 1 half, 061 opt 3 `eth_call`, 031 opt 1 `retain`, the reorder halves, 030 opt 3 backoff); 060 opt 2 is a one-liner (`toml::de::Error::set_input(None)`); tree clean | QA sections, `rerun.txt` |
| QA2-XC (resume) | QA | finish QA sections for 001/002/004/005/006/009/050 (+CORE-036), §7 leftovers | done — Reproduced: 050 → 92 (CORE-036 mirrored → 92), 006 → 90 (two local Anvil chains, request re-signed on chain B), 009 → 90 (debug panic, release silent wrap), 005 → 90 **with an `H` on Basis row 6** (sqlx `sqlite` = bundled SQLite 3.51.3 without `SECURE_DELETE`; Dockerfile packages stale — strengthens the conclusion; C2-XC re-judged after QA: row 6 marked `H`, retracted its Supported, mechanism corrected, 90 / Informational confirmed), 001/002/004 re-reproduced; §7: 7.2/7.4/7.6/7.7 done, **7.3 executed at the driver seam: spawn-then-inline-housekeeping loses a still-needed secret in 19/20 rounds** (`coverage.md` §8, promoted by C2-VAL-B as **`F2-VAL-035` Confirmed Medium 78**: driver spawns `ReconcileGroupSecrets` at `driver.rs:278` then awaits `prune_scheduled_secrets(safe)` inline at 292–294 with no join, so a still-needed secret is pruned before reconciliation lands — distinct from 030/031, shared remediation: synchronous first reconciliation on resume); five remediation options judged unsound (incl. XC-004 opt 1: `lru` fix unreachable via `cargo update`); tree clean, Anvil stopped | QA sections, `rerun.txt` |
| C2-SEN-Δ | Critic | `F2-SEN-010`, `F2-SEN-011`, R7Δ re-validation sections, `F2-CORE-030` row 11 | done — **F2-SEN-010 Confirmed High 82** (ordering verified: `NewBlock(n)` precedes block n's logs; all 12 rows Supported; trigger prose corrected — honest ordering means everyone takes outcome 2: reverted `Finalize`, parked; distinct from 002 which stays canonical for outcomes), F2-SEN-011 Confirmed Info 80; all nine re-validations agreed, headers unchanged; F2-CORE-030 row 11 quote/anchor stale but prose true, 92/High stands; QA recipe for 010 handed to QA2-SEN-Δ; Prettier drift in F2-CORE-030 is pre-existing (housekeeping pass) | Critic sections |
| REC-CORE / REC-VAL / REC-SEN / REC-XC | Reconciliation | per-crate F2→F1 mapping, misses, A16/A17, team questions, ledger rows | done (see Phase 4 results) | `state/run2/reconciliation/<crate>.md`, `## Reconciliation (run 2)` sections |
| REC-merge | Reconciliation | `report/RECONCILIATION.md` from the four parts; `KNOWN-WORK.md`/`IN-FLIGHT.md`/`pr-review-threads.md` refresh | done — combined ledger: **95 live canonical findings** (77 run-1 IDs + 18 run-2 IDs): Critical 1, High 14, Medium 24, Low 38, Info 18; Confirmed 76, Plausible 18, Observation 1; E1 rows 49; non-live: Fixed 1 (`F-VAL-005`), Out of scope 1 (`F-VAL-033`), Superseded 1 (`F-VAL-068`), Refuted 1 (`F-SEN-013`), forward-looking 3 files; folded run-2 files 51; agreement: run 2 rediscovered 52/77 live run-1 findings (68 %), 19 misses all still valid; run 1 missed 14 of run 2's 18 canonical items; CONTRADICTS 0, three sub-claim contradictions settled by execution; 12 merge notes; 11 draft thread replies; links and Prettier clean | `report/RECONCILIATION.md` |
| Documentation | Documentation | `report/REPORT.md` rewrite for the combined result (committed last, alone) | running | `report/REPORT.md` |

## Findings

| ID | Title | Status | Severity | Certainty |
| --- | --- | --- | --- | --- |
| `F2-CORE-001` | Reorg-depth protection is not persisted: a restart resumes f | QA'd | Medium | 90 |
| `F2-CORE-002` | `use_client_filtering` integrity protection is silently aban | QA'd | Medium | 90 |
| `F2-CORE-003` | Default new-block log fetch has no completeness check althou | Critiqued | Informational | 70 |
| `F2-CORE-004` | Event filtering and decoding are address-agnostic: core cann | Critiqued | Low | 70 |
| `F2-CORE-005` | Event-fetch failures retry forever without bound, backoff or | QA'd | Medium | 90 |
| `F2-CORE-006` | `revalidate_last_block` treats a `null` header as "uncled", | Critiqued | Low | 70 |
| `F2-CORE-007` | A restart against an RPC node lagging more than `max_reorg_d | Critiqued | Low | 75 |
| `F2-CORE-008` | With `max_reorg_depth = 0` the documented "fails loudly on a | Critiqued | Low | 75 |
| `F2-CORE-009` | Initialization range scan restarts without bound or delay on | Critiqued | Low | 70 |
| `F2-CORE-010` | The per-topic fallback multiplies request volume (x17 valida | Critiqued | Low | 70 |
| `F2-CORE-011` | No rollback anchor is persisted for the first block indexed | QA'd | Low | 92 |
| `F2-CORE-030` | Rollback and restart discard effect resumes applied since th | QA'd | High | 92 |
| `F2-CORE-031` | Fatal driver errors terminate the process with exit status 0 | Critiqued | Low | 85 |
| `F2-CORE-032` | Restart and rollback replay re-queues and re-submits already | Critiqued | Low | 75 |
| `F2-CORE-033` | `/health` is an unconditional `OK`; a stalled driver (indefi | Critiqued | Low | 80 |
| `F2-CORE-034` | Graceful shutdown is only observed between inputs; an unboun | Critiqued | Low | 72 |
| `F2-CORE-035` | Effect concurrency is unbounded: one task per `Command::Effe | Critiqued | Low | 70 |
| `F2-CORE-036` | A log range's actions and effects are dispatched only after | QA'd | Low | 92 |
| `F2-CORE-060` | Replacement fee escalation is geometric with no absolute cei | QA'd | Medium | 92 |
| `F2-CORE-061` | Execution is inferred from a single `eth_getTransactionCount | QA'd | Medium | 55 |
| `F2-CORE-062` | First-submission "underpriced" rejections are not recognised | QA'd | Medium | 65 |
| `F2-CORE-063` | Actions replayed after a restart or rollback are enqueued an | QA'd | Low | 90 |
| `F2-CORE-064` | `expires_at` stops applying the moment a nonce is allocated: | QA'd | Low | 90 |
| `F2-CORE-065` | `tx::Config` accepts degenerate values silently: zero in-fli | QA'd | Low | 90 |
| `F2-CORE-066` | The transactions table is bound to neither chain id, signer | Critiqued | Low | 70 |
| `F2-CORE-067` | The transaction queue exports no metrics and does not persis | Critiqued | Informational | 85 |
| `F2-SEN-001` | A rollback past the engine resume strands an onchain commitm | QA'd | High | 92 |
| `F2-SEN-002` | Commitments that land before the engine verdict are not tall | QA'd | High | 90 |
| `F2-SEN-003` | `finalize()` abandons a bonded request whenever our own reve | QA'd | Medium | 90 |
| `F2-SEN-004` | `WaitingForOutcome` and `WaitingForDisputeResolution` never | QA'd | Medium | 90 |
| `F2-SEN-005` | No bound on concurrent engine checks or outstanding bonds; a | QA'd | Medium | 78 |
| `F2-SEN-006` | No startup or per-request pre-flight: an unregistered, unfun | QA'd | Low | 82 |
| `F2-SEN-007` | Actions are not idempotent under reorg or restart replay: du | QA'd | Low | 90 |
| `F2-SEN-008` | Engine client robustness: single attempt, timeout derived fr | QA'd | Low | 90 |
| `F2-SEN-009` | Hygiene: sample config ships a well-known private key and a | Critiqued | Informational | 85 |
| `F2-SEN-010` | Commitments mined in the commit-deadline block are never tal | QA'd | High | 93 |
| `F2-SEN-011` | `handle_arbitration_timeout` still documents and meters an a | Critiqued | Informational | 80 |
| `F2-VAL-001` | DKG encryption key `q` has no proof of possession: a member | QA'd | Critical | 95 |
| `F2-VAL-002` | ECDH share pads are raw x-coordinates, reused in both direct | QA'd | Low | 90 |
| `F2-VAL-003` | A restart whose replay warp covers an epoch-boundary key-gen | QA'd | Medium | 85 |
| `F2-VAL-004` | The late-setup branch of `handle_key_gen_setup` derives a di | QA'd | Low | 50 |
| `F2-VAL-005` | A `KeyGenSetup` resume lost to a restart is never re-issued: | Critiqued | Medium | 65 |
| `F2-VAL-006` | Complaint responses are unconditional and unbounded per plai | QA'd | Low | 82 |
| `F2-VAL-007` | `active_epoch` only advances through epochs this validator s | QA'd | Low | 65 |
| `F2-VAL-030` | A failed `NonceTree` effect leaves a phantom chunk reservati | QA'd | Medium | 90 |
| `F2-VAL-031` | Nonce generator streams are cold after every restart and not | QA'd | Medium | 80 |
| `F2-VAL-032` | `handle_sign` drops a pending `WaitingForRequest` session on | QA'd | High | 92 |
| `F2-VAL-033` | A signer can re-reveal its nonce to become `last_signer` and | QA'd | Low | 88 |
| `F2-VAL-034` | No schema version check: a database created before the pruni | QA'd | Informational | 85 |
| `F2-VAL-035` | The driver spawns a block's `ReconcileGroupSecrets` and then | Critiqued | Medium | 78 |
| `F2-VAL-060` | Any configuration parse error prints the whole config file, | QA'd | High | 95 |
| `F2-VAL-061` | Coordinator and Consensus events are accepted from any watch | QA'd | High | 90 |
| `F2-VAL-062` | A failed `NonceTree` effect strands a phantom chunk reservat | QA'd | Medium | 85 |
| `F2-VAL-063` | A failed or lost `KeyGenSetup` effect is never re-issued; th | QA'd | Medium | 69 |
| `F2-VAL-064` | The documented `--config-file=<path>` spelling is rejected b | QA'd | Informational | 95 |
| `F2-VAL-065` | The sample `database` URL omits `?mode=rwc`, so the document | QA'd | Low | 88 |
| `F2-VAL-066` | The process exits with status 0 after unrecoverable errors, | QA'd | Low | 72 |
| `F2-VAL-067` | No cross-validation of `blocks_per_epoch` against the key-ge | QA'd | Low | 75 |
| `F2-VAL-068` | Runtime image runs the validator as root on floating base-im | Critiqued | Informational | 90 |
| `F2-XC-001` | Configuration parse errors print the whole config file, sign | QA'd | High | 95 |
| `F2-XC-002` | The sample `database` paths cannot be opened on a fresh volu | QA'd | Low | 92 |
| `F2-XC-003` | Runtime images run as root on floating base tags, with no to | Critiqued | Low | 82 |
| `F2-XC-004` | Dependency advisories are not gated in CI; none is reachable | QA'd | Informational | 85 |
| `F2-XC-005` | Secret deletion is logical only: pruned and rolled-back secr | QA'd | Informational | 90 |
| `F2-XC-006` | Persistent state is not bound to the chain, the deployment o | QA'd | Low | 90 |
| `F2-XC-007` | The signer private key lingers in un-zeroized configuration | Critiqued | Informational | 75 |
| `F2-XC-008` | Sample configurations and startup validation: a publicly kno | Critiqued | Informational | 75 |
| `F2-XC-009` | No release profile: overflow checks are off in shipped binar | QA'd | Informational | 90 |
| `F2-XC-050` | The snapshot is committed before the block's actions are enq | QA'd | Low | 92 |

## Decisions and open questions

- Phase 0 baseline: build 0, test 0 (183 in-scope tests pass; 280 workspace-wide), clippy `-D warnings` 0, `cargo audit` 1 (5 vulnerabilities: `crossbeam-epoch` 0.9.18, `h2` 0.4.14, `quinn-proto` 0.11.14, `ruint` 1.18.0, `rustls` 0.23.40; 11 warnings), `cargo tree -d` 0 (14 version splits). Details and verbatim advisories in `baseline.md` Section 2; reachability is R10's task.
- Inventory: 61 `.rs` files, 20,301 lines, 183 tests in scope (map: 19,090 lines, 169 tests). Eleven files drifted from the map — exactly the files changed since run 1 (`baseline.md` Sections 3.4 and 5); map line numbers in those files are stale.
- The audited `crates/`, `Cargo.toml` and `Cargo.lock` are byte-identical to `origin/main` `5cc096e`; the dependency set is unchanged since run 1 (`Cargo.lock` unchanged since `2893917`).
- Branch HEAD is now `4ac6838` (commits `1f3fe52`, `4ac6838` landed during Phase 0, touching only `rust-audit/`). `crates/`, `Cargo.toml`, `Cargo.lock` are identical to `3ec8bc5` (tree/blob hashes match; `baseline.md` Section 5), so the Commit field stays `3ec8bc5` and A14 holds. Logs 03/04/05/08 carry `4ac6838` in their headers for this reason.
- Reviewer numbering keeps the map's labels: R1–R7 plus R10; R8/R9 are not launched (engine). Suggested watch: R7 and R5 carry the unmapped new code and may need splitting.
- Toolchain notes: `sqlite3` CLI absent; Foundry is 1.8.1 rather than the 1.5.1 written in A9; every cargo command needs `CARGO_TARGET_DIR=/home/shebin.guest/.cache/safenet-run2/target CARGO_BUILD_JOBS=2` and the PATH prefix; `/tmp` is a 6 G tmpfs — never build or log there.

## Gate 1 (PASSED — operator answered `compact`)

Phase 1 complete: 8/8 reviewers done, 0 failed, 0 relaunched. **63 run-2 findings**, all Draft: CORE 24 (R1 10, R2 6, R3 8), VAL 21 (R4 7, R5 5, R6 9), SEN 9 (R7), XC 9 (R8). Reviewer self-estimates: Critical 1 (`F2-VAL-001`), High 8, Medium 15, Low 29, Info 10 — Critics set the real numbers. All 61 in-scope files read to 100%; 8 coverage logs with ~150 rejected hypotheses. Tree clean outside `rust-audit/`; **nothing committed** (two-commit rule). Structural pre-check: all 63 files have the template sections and parseable headers, all IDs inside their reviewer's range, no collisions. **Gap for C2-VAL-B:** `F2-VAL-061`…`068` (R6) cite no commit hash in their text; the Critic should confirm every citation against `3ec8bc5` and note the omission.

## Manager log

- Gate 0: `continue`. Gate 1: `compact`; Phase 2 launched after compaction (seven Critics, in parallel). Gate 2: `continue`; Phase 3 launched (five QA agents, in parallel, shared `CARGO_TARGET_DIR` — cargo serialises builds on its lock). Operator pause requested during Phase 3 with QA2-CORE, QA2-SEN, QA2-VAL-A done and QA2-VAL-B, QA2-XC still running: in-flight agents run to completion (stopping them would strand their temporary tracked edits), their completions are recorded here, **no Gate 3 and no new launch until the operator says resume**. Operator then asked to stop the agents outright: both killed, their PoC directories kept, the one stranded tracked edit reverted, no Anvil left running. Operator resumed with "check the current main and continue": `origin/main` had moved `5cc096e` → `8b6a75d` (20 commits); merged locally into the branch (A14: tree change → delta recon). In-scope delta: `crates/sentinel/src/service.rs` (+199: deadline comparisons `<=` → `<`, dropping deferred until the deadline block is indexed, two new tests), `bindings.rs` (`NewRequest` gains `daoFeeShare`), `contracts/src/SentinelOracle*.sol` (`InvalidCommitHash`, duplicate-commit check now on `vote == NONE`, DAO fee-cut rounding, documented non-revealer slash never refunded), workspace `bitflags` (engine only). Core and validator untouched, so the QA2-CORE / QA2-VAL-A / QA2-VAL-B E1 results carry over; sentinel findings need re-validation and QA2-SEN's PoCs a re-run. Apparent cross-reviewer duplicates handed to the Critics for canonical naming: `F2-VAL-030`/`F2-VAL-062` (phantom chunk reservation), `F2-VAL-005`/`F2-VAL-063` (lost `KeyGenSetup`), `F2-VAL-060`/`F2-XC-001` (config-error key leak), `F2-VAL-065`/`F2-XC-002` (sample database URL), `F2-VAL-068`/`F2-XC-003` (root image), `F2-CORE-031`/`F2-VAL-066` (exit status 0), `F2-CORE-032`/`F2-CORE-063` (replay re-submission).

## Gate 2 tally (after all seven Critics)

66 findings, all Critiqued (the two promoted Drafts got a second-Critic pass). Refuted 0, Unsupported 0, `H` rows 0. Final severity: **Critical 1** (`F2-VAL-001`, 85), **High 7** (`F2-CORE-030` 80, `F2-SEN-001` 82, `F2-SEN-002` 80, `F2-VAL-032` 80, `F2-VAL-060` 92 E1, `F2-VAL-061` 74, `F2-XC-001` 95 E1), **Medium 15**, **Low 32** (incl. CORE-011 78, CORE-036 75), **Informational 11**. Certainty bands: 90–100 ×5 (all E1: VAL-060, VAL-064, VAL-068, XC-001, XC-002), 70–89 ×50, 40–69 ×11 (Plausible). Severity moved by Critics: down — CORE-002 (High claim rejected), CORE-003 → Info, CORE-031 → Low, VAL-002 → Low, VAL-030/031 High → Medium; up — XC-001 → High, VAL-005 → Medium (as dup). Duplicate pairs with canonical named: VAL-005→VAL-063, VAL-062→VAL-030, VAL-060→XC-001 (both Critics now High), VAL-065→XC-002, VAL-068→XC-003, VAL-066→CORE-031, CORE-032→CORE-063, CORE-036→XC-050, SEN-009 bullets→XC-003/007/008. Distinct defects ≈ 58. Executed evidence so far: Critic logs `C2-CORE-A-cargo-test-index.txt`, `c2-xc-*.txt`, `CC-core-state-tests.txt`; C2-CORE-B ran `tx:: state:: effects::` (41 pass), C2-SEN ran the 42 sentinel tests.

Second-Critic passes: `F2-CORE-011` Confirmed, Low, 78 (C2-CORE-B) — distinct from 001/007; row 6 E1 (`state::` 13/13, log `C2-CORE-B-cargo-test-state.txt`); one grep briefly surfaced one-line fragments of run-1 files, none opened. `F2-CORE-036` Confirmed, Low, 75 (C2-CORE-A) — duplicate of `F2-XC-050` (canonical); fold in the `start_block` variant and the 'move `prune` below `queue`' fix; log `c2-core-a-036-state-test.txt`.

Housekeeping deferred to the end of the review (agents keep appending): Prettier `--write` over all run-2 files; date scrub of the reviewers' dated log-line prefixes (exact-string edit, `rust-audit/` only); the `Started` field of this file.

## Gate 3 tally (after QA, the delta review and all follow-ups, at `fe9e84c`)

**69 findings** (63 reviewer + 6 promoted: `F2-CORE-011`, `F2-CORE-036`, `F2-XC-050`, `F2-VAL-035`, `F2-SEN-010`, `F2-SEN-011`): 47 QA'd, 22 Critiqued without a QA pass (all Low/Informational or duplicates, plus `F2-VAL-005` dup and `F2-VAL-035`, whose evidence is the executed seam run). Refuted 0; one `H` row (`F2-XC-005` row 6, corrected — conclusion strengthened). **Critical 1, High 8, Medium 16, Low 32, Informational 12.** Certainty bands: 90–100 ×31 (all executed), 70–89 ×32, 40–69 ×6. 47 PoC directories under `poc/F2-*`. Delta from `origin/main` (`3ec8bc5` → `fe9e84c`): sentinel findings all still valid, one new High (`F2-SEN-010`, 93) and one Informational; core/validator unchanged. Tree clean outside `rust-audit/`; Prettier clean; agent-written dates removed (tool-output dates in verbatim logs kept); no HTML comments; nothing committed.

## Phase 4 — reconciliation results

- **REC-VAL done:** 22 F2-VAL → CONFIRMS 8, EXTENDS 7, NEW 7 (003, 004, 007, 034, 060, 064, 065), CONTRADICTS 0; five in-row severity disagreements settled (`F-VAL-030` stays High — run 1 reproduced the phantom reservation live after a reorg; `F-VAL-060` High 90 conditional on a malicious allow-listed oracle); run-1 not rediscovered and still valid: `F-VAL-031/034/036/037/038/039/062/065`, parts of 063/035/068; fixed by merge: `F-VAL-005` main trigger, `F-VAL-066`; out of scope: `F-VAL-033` (A17); superseded: `F-VAL-068`; A16: `F-VAL-004` genesis → Info/known but its rollover instance `F2-VAL-063` Medium 69 stays; F-VAL-005 answer: fixed for the reorg trigger (13/13 store tests), not for absence beyond `max_reorg_depth` nor the restart reconcile-vs-prune race (`F2-VAL-035`); ledger 31 canonical rows: Critical 1, High 5, Medium 7, Low 10, Info 4 (+6 folded); unsettled: `F-VAL-039` cost model, `F-VAL-038` duration (need execution).
- **REC-CORE done:** 26 F2-CORE → CONFIRMS 13, EXTENDS 8, NEW 4 (003, 007, 011, 067), folded 1 (036 → XC-050); 35 canonical core findings: High 4, Medium 13, Low 15, Info 3; run-1 core not rediscovered: `F-CORE-012` (bloom-equality blind spot, the significant miss), `035`, `032`, `040`, parts of `009`; all still valid (core drift is only the housekeeping hook); contradictions settled: 001/002 carry run 1's High 99 (run 2's Medium recorded as dissent), `F-CORE-031`/`F2-CORE-030` → High 92, `F-CORE-060` cap sub-claim both right (own-fee ratio preserved, base-fee-relative cap bypassed 28,744×; `tx::fees` 4/4), `F-CORE-011` → Low 80; team answers: F-CORE-033 no (deadline bounds `Commit` rows, not `EngineCheck` effects), F-CORE-067 yes with five qualifications.
- **REC-SEN done:** 11 F2-SEN → CONFIRMS 7, EXTENDS 3, NEW 1, CONTRADICTS 0 (one weighting disagreement on `F2-SEN-001` (c) settled in run 1's favour); run-1 not rediscovered: `F-SEN-008` (full miss, still valid), `F-SEN-011` (carried by `F2-CORE-030`), partials `F-SEN-003/014/015`; `F-SEN-013` refutation holds; #914: run 1's "worsens F-SEN-002" and `F2-SEN-010` are the same hunk — canonical `F2-SEN-010` High 93, `F-SEN-002` stays High 98 with broadened trigger; `F-SEN-003` answered (warp `NewBlock` gap executed and current); `F-SEN-016` D1 superseded by `F2-SEN-010`, D2–D4 forward-looking (#915 unmerged); ledger 17 live rows: High 4, Medium 3, Low 7, Info 3.
- **REC-XC done:** 10 F2-XC → CONFIRMS 3, EXTENDS 2, NEW 5 (`F2-XC-001` High 95 is a run-1 miss), CONTRADICTS 0; run-1 XC not rediscovered: `F-XC-002` (soft miss), `F-XC-003` (test gap), `F-XC-050` (still valid; run 2's `F2-VAL-061` is its precondition; split under A16: genesis Info/known, rollover Medium), `F-XC-051` (settled from frost sources → Informational 85); `F-XC-008` I→E2, → Informational; ledger 15 counted rows; canonical rows for all duplicate clusters written; stale comment at `state/keygen.rs:451-452` noted for the report.

## Phase 3 plan (as launched)

Five QA agents per `state/run2/qa-brief.md`, told to write log lines without dates. Priority order inside each: Critical/High first, then Medium, then anything a Critic named as liftable to E1.

| QA agent | Findings (E2→E1 targets) | Anvil ports |
| --- | --- | --- |
| QA2-CORE | `F2-CORE-030` (restart/rollback resume discard incl. the deterministic replay trigger), `F2-CORE-001`, `002`, `005`, `060` (escalation arithmetic), `061`, `062`, `011`, `063`/`064`/`065` if cheap | 8545–8547 |
| QA2-SEN | `F2-SEN-001` (StateMachine-level restart test; mirrors CORE-030), `002`, `003`, `004`, `005` (queue starvation), `007` | 8548–8549 |
| QA2-VAL-A | `F2-VAL-001` **Critical PoC** (copied `q`, complaint flow, share recovery — pure frost/keygen test), `002` (pad symmetry), `003` (warp drops keygen start), `006` | 8645–8647 |
| QA2-VAL-B | `F2-VAL-032` (pure-transition PoC), `030`/`062` (phantom reservation), `031` (cold generators after restart), `061` (forced race), `063`, `033`, `067`; `060`/`064` already E1 — re-save under `poc/` | 8648–8649 |
| QA2-XC | `F2-XC-050` (fault-inject the commit-then-enqueue window), `006` (Driver start against a foreign-`chain_id` store), `009` (overflow-check profile), `005`; Coverage Critic §7 items not covered above | 8745–8749 |

## Phase 2 plan (launched)

Launch **seven Critics in parallel**, each told to read `state/run2/critic-brief.md` first (independence rule: no run-1 material; form own view from the cited code before reading the reviewer; re-open every citation; `H` for anything not in the checkout; certainty per PROMPT §8 — 90–100 only with executed evidence; re-judge severity, then apply A16/A17; mine the reviewer's rejected list and promote into the reviewer's free `F2-` IDs; append-only; return ≤10 lines):

| Critic | Findings | Coverage log(s) to mine | Emphasis |
| --- | --- | --- | --- |
| C2-CORE-A | `F2-CORE-001..010` | `agents/R1.md` | reorg/downtime anchor claims; log-completeness under degraded RPC (A4 line) |
| C2-CORE-B | `F2-CORE-030..035`, `F2-CORE-060..067` | `agents/R2.md`, `agents/R3.md` | resume-discard-on-rollback mechanism; fee-escalation cap claims (R3 refuted a sub-claim — verify the arithmetic); replay idempotency |
| C2-VAL-A | `F2-VAL-001..007` | `agents/R4.md` | **re-derive the Critical `F2-VAL-001` link by link from code and `contracts/src` before reading R4**; A16 on genesis-only items |
| C2-VAL-B | `F2-VAL-030..034`, `F2-VAL-060..068` | `agents/R5.md`, `agents/R6.md` | the three R5 Highs on the pruning store (restart-cold generators; phantom reservation; sybil `Sign`); R6's config-error key leak (E1 — verify the command); missing commit citations in 061–068 |
| C2-SEN | `F2-SEN-001..009` | `agents/R7.md` | bond-loss pair vs `contracts/src/libraries/SentinelOracleRequests.sol` deadline/slash semantics; waiting-state expiry |
| C2-XC | `F2-XC-001..009` | `agents/R8.md` | advisory reachability claims (re-run `cargo tree -i` yourself); the 0-panic census (spot-check the 5 wrapping sites); no CVSS-as-severity |
| Coverage Critic | — | all 8 logs vs `reviewer-split.md` | per-file matrix for the 61 files; seams between reviewers (R1↔R2 warp/restart, R4↔R5 secret store, R6↔R7 sentinel recurrences); seeded leads never examined; promote into `F2-XC-050+` |

Environment for all: `export PATH="$HOME/.foundry/bin:$HOME/.cargo/bin:$PATH" CARGO_TARGET_DIR=/home/shebin.guest/.cache/safenet-run2/target CARGO_BUILD_JOBS=2`; foreground commands; no tracked-file edits; no Anvil. Then record completions here, hold **Gate 2** (operator `/usage`), and launch Phase 3 QA per `state/run2/qa-brief.md` for every crate with a Confirmed/Plausible finding (Anvil ports: core/sentinel 8545–8549, validator 8645–8649, cross-cutting 8745–8749). After QA: Gate 3 → Phase 4 (Coverage Critic output + run-2 REPORT inputs) → **Reconciliation** (`state/run2/reconciliation-brief.md`) → final report (`state/run2/report-brief.md`) → the two commits.
