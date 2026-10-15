# Reconciliation: runs 1, 2 and 3 of the Safenet Rust services audit

| Field | Value |
| --- | --- |
| Audited commit | `361aaf3b25e12d2e04f296184c326830968f3727`: merge of `origin/main` `c355af1` into `audit/rust-services`; outside `rust-audit/` byte-identical to `c355af1` |
| Earlier commits | run 1 at `2893917` (re-validated at `a7f3915`); run 2 at `3ec8bc5` and `fe9e84c`; every `path:line` here is at `361aaf3` unless marked |
| Runs | run 1; run 2 (Fable 5.1, blind to run 1); run 3 (Opus 5.5): blind discovery plus a separate re-validation track that met here |
| Inputs | the previous version of this file (git history), the team's 13 PR #905 threads (now section 6), `report/KNOWN-WORK.md`, `state/run3/revalidation/`, every `findings/F3-*.md` with its Critic, QA, Practicality (PC3) and Confidence (CA3) sections, `state/run3/phase4/`, `state/run3/coverage.md`, the reviewer logs in `state/run3/agents/`, and the department reports of run 3 |
| What this document is | the single source for every number in `REPORT.md`: one canonical ID per defect with its final severity, certainty, status and basis, the fold and resolved indexes, misses in both directions and the team's threads; it replaces the run-1 and run-2 ledger, which stays in git history |
| Removal list | [`state/run3/removal-manifest.md`](../state/run3/removal-manifest.md) |

## 1. Method and headline counts

### 1.1 Method

- **Two tracks.** Run 3's discovery agents (R1 to R8, L-crypto, L-rust, five Critics, QA with a second independent reproduction for every Medium or higher row, PC3, CA3) never read earlier material. The re-validation agents (rv-core-a, rv-core-b, rv-val, rv-sen, rv-xc) re-checked all 103 earlier rows at `361aaf3` and re-ran or adapted their PoCs. CA3-FIX re-verified every Fixed or Moot verdict on a row that was once High or Critical, plus `F-SEN-003` and `F2-VAL-003`. LT3 triaged the leads and filed `F3-SEN-060`, `F3-VAL-100` and `F3-XC-040`.
- **Mapping.** Every `F3` file is mapped to the earlier rows as CONFIRMS (same defect), EXTENDS (same root, new trigger, scope or impact), CONTRADICTS or NEW; its `## Reconciliation (run 3)` section states the result, and each earlier file it maps to carries a `## Run 3 counterpart` note.
- **Canonical choice.** An earlier ID stays canonical when run 3 confirms or extends a live row (longer trail, the team's links). A run-3 ID becomes canonical where the earlier row is resolved (`F2-XC-001` Fixed, so `F3-XC-040`) or where a merged change turned the earlier row's trigger into one only the run-3 file executed (`F2-VAL-004` folds into `F3-VAL-062`, section 3.3). Run-3 duplicates follow their run-3 canonical and stay as files this run.
- **Band.** Run-3 rows carry CA3's final values. Earlier rows carry the re-validation proposal, unless CA3 set the row itself (`F-SEN-002`, `F-SEN-008`, `F-SEN-015`, `F-VAL-060`), CA3 proposed one band for a cross-run twin set, or the run-3 counterpart's CA3 band covers the leg that carries the earlier band (`F-CORE-066`, `F-VAL-063`). A counterpart that covers only another leg does not move the band (`F-CORE-060`, section 7).
- **Certainty.** The strongest executed evidence across the twins carries the number, as in the earlier ledger. Where CA3 set an earlier row's certainty, that number stands unless the counterpart executed the same defect more strongly (`F-SEN-008` carries `F3-SEN-001`'s 97).
- **Status and basis.** Confirmed means certainty 70 or more, Plausible 40 to 69; the re-validation verdict (Still valid, Changed) or New follows; `known` marks an item `settled.md` section 2 or an open team issue already lists (A12). Basis is the leading class of the row's headline claim: E1 executed, E2 traced in code.

### 1.2 Headline counts

| Measure | Count | Notes |
| --- | --- | --- |
| Live canonical rows | 90 | 67 run-1 IDs, 12 run-2 IDs, 11 run-3 IDs |
| Critical | 0 | the only Critical, `F-VAL-001`, is Fixed by #1027 |
| High | 4 | `F-CORE-060`, `F-SEN-008`, `F3-VAL-030`, `F3-XC-100` |
| Medium | 26 | core 14, validator 5, sentinel 6, cross-cutting 1 |
| Low | 36 | core 15, validator 10, sentinel 6, cross-cutting 5 |
| Informational | 24 | core 4, validator 5, sentinel 3, cross-cutting 12 |
| Confirmed, Plausible | 77, 13 | no row below 40 |
| Basis E1, E2 | 73, 17 | every High row is E1 |
| Rows found by more than one run | 53 | 12 by all three runs |
| Resolved since run 2 | 22 | Fixed 8, Moot 7, Settled 3, Superseded 3, Refuted 1 (section 4) |
| Finding files | 190 before, 115 after | 75 leave: the 22 resolved rows, 52 folded run-2 files (sections 3.3 and 3.4) and the union row `F-XC-002` |
| PoC entries | 129 before, 108 after | 21 have no surviving citer; PoC folders of resolved rows that a surviving file cites stay |
| Removal manifest | 111 paths | grouped by reason, with the link check |

| Crate         | Rows   | High  | Medium | Low    | Informational |
| ------------- | ------ | ----- | ------ | ------ | ------------- |
| core          | 34     | 1     | 14     | 15     | 4             |
| validator     | 21     | 1     | 5      | 10     | 5             |
| sentinel      | 16     | 1     | 6      | 6      | 3             |
| cross-cutting | 19     | 1     | 1      | 5      | 12            |
| **total**     | **90** | **4** | **26** | **36** | **24**        |

### 1.3 Run 3 against the earlier runs

| Run-3 files | Count | IDs |
| --- | --- | --- |
| CONFIRMS an earlier live row | 16 | `F3-CORE-002`, `004`, `005`, `030`, `032`, `060`, `061`; `F3-SEN-001`, `002`, `003`, `004`; `F3-VAL-061`; `F3-XC-005`, `006`, `007`, `030` |
| EXTENDS an earlier row | 4 | `F3-CORE-001` and `F3-XC-160` (earlier ID stays canonical); `F3-VAL-062` and `F3-XC-040` (run-3 ID canonical) |
| CONTRADICTS an earlier row | 0 | one sub-claim contradiction, settled by execution (section 1.5) |
| NEW | 9 | `F3-SEN-060`, `F3-VAL-030`, `031`, `032`, `100`, `F3-XC-003`, `004`, `100`, `130` |
| Run-3 duplicates | 7 | follow their run-3 canonical (section 3.1) |
| **total** | **36** | 29 canonical run-3 files: 18 counterparts of earlier rows, 11 run-3 canonical rows |

- **Rediscovery.** Of the 80 earlier rows still live (the 79 kept plus `F2-VAL-004`), the blind track filed 20 as counterparts (25 %), reached 4 more inside other run-3 files, examined 48 without filing them (4 as known items) and never reached 8 (section 5.1). Run 2 had rediscovered 68 % of run 1, fully or in part; run 3 reaches 30 % on the same reading. Most of the gap is by design: the settled list told reviewers not to refile known items, and most examined rows were kept as observations under A1 or A3 or below the finding bar.
- **Reverse misses.** 9 run-3 rows are NEW; 7 of them describe defects already present at `fe9e84c`, among them both new High rows (section 5.2).

### 1.4 Band and certainty settlements

- **Cross-run twins, one band each (CA3, using PC3's practicality).** `F-CORE-001`, `F2-CORE-001`, `F3-CORE-004`: Low, was High (a reorg deeper than `max_reorg_depth`; none of 3 or more blocks verified on Gnosis in 294.8 days; one diverged node). `F-CORE-002`, `F2-CORE-002`, `F3-CORE-002`: Medium, was High (a provider behaviour A4 names, not attacker input). `F-CORE-031`, `F-SEN-001`, `F3-CORE-032`, `F3-SEN-004`: Medium, `known`, was High (a lost effect or reveal after a restart or a live one-block reorg, recoverable by a manual `claim`; daily orphans and routine restarts keep it above Low).
- **Earlier rows CA3 set itself.** `F-SEN-008` Low to High (the proxied fee token is now USDC.e); `F-SEN-002` High to Medium (funds locked, not lost); `F-SEN-015` High to Medium (restart-window residual only); `F-VAL-060` High to Medium (precondition false on Aegis).
- **Earlier rows moved by a counterpart's band.** `F-CORE-066` Medium to Informational (`F3-CORE-061`); `F-VAL-063` Medium to Low (`F3-VAL-061`); `F2-CORE-007` stays Low (`F3-CORE-030`, which PC3 moved from Medium).
- **Earlier rows moved by the re-validation.** `F-VAL-061` High to Medium; `F-VAL-066` Medium to Low; `F-VAL-002` Medium to Low, `known`; `F-VAL-035` from a Low observation to Informational.
- **Run-3 rows whose final band differs from the reviewer's.** `F3-VAL-030` Critical impact class to High; `F3-CORE-002` Low to Medium; `F3-CORE-032` Low to Medium (CA3 over PC3); `F3-CORE-030` Medium to Low; `F3-VAL-100` and `F3-SEN-060` Medium to Low; `F3-XC-130` and `F3-XC-040` Low to Informational.
- **Kept, with the dissent recorded (section 7).** `F-CORE-060` High (run 2 Medium; run 3's counterpart rates only the accepted arm, Low); `F-XC-050` Medium (its precondition is false on Aegis).
- **Certainty raised by run-3 execution.** `F-SEN-008` 52 to 97, `F-VAL-035` 35 to 70, `F2-CORE-007` 75 to 95, `F-CORE-008` 70 to 92, `F-CORE-065` 70 to 90, `F-VAL-063` 72 to 90, `F-CORE-012` 70 to 85, `F-CORE-030` 85 to 95, `F-CORE-003` 70 to 80, `F-CORE-005` 75 to 85, `F-CORE-009` 78 to 85, `F-CORE-034` 80 to 85, `F-CORE-038` 85 to 90, `F2-CORE-003` 70 to 80, `F-SEN-014` 88 to 95, `F2-SEN-003` 90 to 95, `F2-XC-002` 92 to 95, `F-XC-008` 80 to 90, `F-XC-051` 85 to 90. **Lowered:** `F-SEN-002` 98 to 90, `F-SEN-015` 98 to 95, `F-VAL-061` 98 to 90, `F-VAL-040` 88 to 70, `F-VAL-066` 78 to 65, `F-VAL-002` 93 to 90.

### 1.5 Contradictions settled

| Where | One position | The other | Settled |
| --- | --- | --- | --- |
| `F-SEN-015`, restart window | rv-sen: Fixed by #978 and #979 (6 variants); run 3's R7 rejected the verdict-prune race (Q52); LT3 first judged lead 14 unreachable | CA3-FIX: real binary under `integ`, 15 of 20 restarts inside the window never revealed, control 0 of 6 | by execution: not fixed in the restart window, fixed for live reorgs; CA3 16 of 16; LT3 withdrew its verdict; Medium 95 |
| `F2-XC-050` and `F3-XC-030` | R8 R-12: a crash after the snapshot commit replays from the safe snapshot | C3-XC: during a warp the prune leaves the page's own snapshot, so the restart resumes after it (executed tests) | by execution: the window exists in warps; `F3-XC-030` confirms `F2-XC-050` |
| `F-SEN-012`'s run-2 extension and `F3-XC-004` | run 2 (`F2-SEN-008` claim 6) and run 3's R7 O3: userinfo in the engine URL would be logged | `F3-XC-004` Basis 4: reqwest moves userinfo into the `Authorization` header | by execution (R8): userinfo is not logged; path and query tokens are |
| `F3-CORE-032` band | PC3: Low (restart route only) | CA3: Medium (the live one-block reorg route) | CA3's final band; the cross-run band follows it |
| `F3-SEN-002` variant a | PC3: practical | CA3: conditional (the honest race lost 0 of 4 runs) | the band is Medium either way; CA3's call stands |
| Late rollover confirmation | R4 O2: the honest last confirmer is excluded | R6 Q29: nobody is excluded | C3-VAL: R6 is right; an observation only |
| Divergent participant lists | R4 Q32: by design, not a finding | R6: a silent failure (`F3-VAL-061`) | C3-VAL Confirmed it; Low |

### 1.6 Re-validator notes settled

- **`F-CORE-068` folds into `F-CORE-063`.** The settled review low L4 covers the whole-batch revert. The residual, a call that fails inside a batch that succeeds and is marked executed by nonce, loses one action per failing call, the same loss a reverting standalone transaction has in Direct mode, which `F-CORE-063` already carries; its run-3 section records the batched-row remediation note. `F-CORE-068` leaves the folder as Superseded and its PoC folder stays, cited by `F-CORE-063`.
- **`F2-XC-050` widened by #1017.** Same row, Low 92. `F3-XC-030` confirms it blind. Since #1017 warp pages run every block's transition, so a crash between the prune and the queue insert now also loses block-driven actions: `KeyGenSecretShare`, `KeyGenConfirm` and complaint responses on the validator, `TimeoutArbitration` and `Claim` (no expiry) on the sentinel. The band stays Low: the window is milliseconds per page and needs an independent hard failure.
- **`F-XC-051` stays Informational 90 as hardening.** The truncated commitment is accepted only from an event the real coordinator would reject (the contract enforces `c.length`, L-crypto R14), so it needs `F-VAL-060`'s precondition, which PC3 showed false on Aegis. The length and identity check is cheap; take it with `F-VAL-060`'s emitter check.
- **Stale comment, a note and not a finding.** `crates/sentinel/src/service.rs:481-483` says warp catch-up does not deliver a `NewBlock` for every block. Since #1017 it does (`crates/core/src/state/mod.rs:230-239`); the `<=` comparison is still correct. The comment right below it (`:487-492`) states the assumption `F3-SEN-060` disproves.

## 2. Canonical rows

One row per live defect. **Runs**: which runs filed or confirmed it; parentheses in the counterpart column mark partial or cross-referenced rows. **Status**: certainty band, then the run-3 re-validation verdict or New. **Deciding evidence**: the one line that sets the band and the certainty. Every `F3` counterpart and duplicate is listed in section 3.

### 2.1 Core (34)

| Canonical | Counterparts | Runs | Short title | Severity | Cert. | Status | Basis | Deciding evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| [`F-CORE-001`](../findings/F-CORE-001.md) | `F2-CORE-001`, `F3-CORE-004` | 1+2+3 | Reorg-depth protection not persisted; a restart resumes from orphaned state | Low | 99 | Confirmed, Still valid | E1 | CA3 one band for the three twins: no reorg deeper than 5 blocks verified on Gnosis, one diverged node (was High); certainty from rv-core-a's real-binary run |
| [`F-CORE-002`](../findings/F-CORE-002.md) | `F2-CORE-002`, `F3-CORE-002` | 1+2+3 | Client-filtering completeness check gives up after three attempts and accepts the empty answer | Medium | 99 | Confirmed, Still valid | E1 | CA3 one band: a provider behaviour A4 names, live on Nethermind 1.32 to 1.35 and 1.39+ (PC3 consult) (was High) |
| [`F-CORE-003`](../findings/F-CORE-003.md) | `F2-CORE-006`, `F3-XC-160` | 1+2+3 | A lagging backend's `null` header is read as an uncle; spurious rollback, and the sentinel loses its reveal | Medium | 80 | Confirmed, Still valid | E1 | `F3-XC-160` executed the bond lock end to end (CA3 80); needs mixed backends: Reth, Nethermind up to 1.35.8, or 1RPC's `-32001` rate-limit answer (PC3) |
| [`F-CORE-004`](../findings/F-CORE-004.md) | `F2-CORE-005` (`F3-CORE-001` in part) | 1+2 | Event-fetch failures retry forever; the block watcher starves and reorg detection stops | Medium | 90 | Confirmed, Still valid | E1 | re-validation (#1049, #1050 add a `MissingBlock` route); PC3 fact: Erigon v3.7.0+ answers `-32000` for a side-chain hash, a concrete client for `F2-CORE-005`'s orphaned-hash trigger |
| [`F-CORE-005`](../findings/F-CORE-005.md) | `F2-CORE-008` | 1+2 | `max_reorg_depth = 0`: `recent` stays empty and the `-32001` path spins silently | Low | 85 | Confirmed, Still valid | E1 | re-validation; run 3's R1 kept it as a non-default configuration (O3) |
| [`F-CORE-006`](../findings/F-CORE-006.md) | `F2-CORE-004` | 1+2 | Filtering and decoding are address-agnostic (core half of `F-VAL-060`) | Low | 70 | Confirmed, Still valid | E2 (decode leg E1) | re-validation |
| [`F-CORE-007`](../findings/F-CORE-007.md) | `F2-CORE-009` | 1+2 | Initialization range scan restarts without bound or delay | Low | 70 | Confirmed, Still valid | E2 | re-validation |
| [`F-CORE-008`](../findings/F-CORE-008.md) | `F3-CORE-005` | 1+3 | Block polling paced by the host wall clock; a slow clock delays every block | Low | 92 | Confirmed, Still valid | E1 | `F3-CORE-005`'s real-binary reproduction (QA3-CORE, CA3 92) |
| [`F-CORE-009`](../findings/F-CORE-009.md) | none | 1 | Block-watcher configuration unvalidated (`block_time = 0`, unbounded depth, `start_block` above head) | Low | 85 | Confirmed, Still valid | E1 | re-validation (cases 1 and 3 executed) |
| [`F-CORE-010`](../findings/F-CORE-010.md) | none | 1 | `-32001` recovery commits the rewind before the event watcher validates it | Low | 45 | Plausible, Still valid | E2 | re-validation; no trigger in three runs (run 3: R1 Q6) |
| [`F-CORE-011`](../findings/F-CORE-011.md) | `F2-CORE-033` | 1+2 | Indexer hangs behind a live-but-silent peer | Low | 80 | Confirmed, Still valid | E2 | re-validation; run 3's L-rust called it outside A4 (section 7) |
| [`F-CORE-012`](../findings/F-CORE-012.md) | none | 1 | Bloom equality is blind to a lost repeated-shape log; `check_logs_limit` skipped on the client-filtered path | Medium | 85 | Confirmed, Still valid | E1 | re-validation's first executed PoC; run 3's R1 described the same loss and C3-COV kept it as an observation (section 7) |
| [`F-CORE-030`](../findings/F-CORE-030.md) | `F2-CORE-031`, `F2-VAL-066` | 1+2 | Fatal driver errors exit with status 0; `/health` is a constant OK | Medium | 95 | Confirmed, Still valid, `known` | E1 | re-validation on the real binary; `known` per `settled.md` section 2 |
| [`F-CORE-031`](../findings/F-CORE-031.md) | `F2-CORE-030`, `F3-CORE-032` | 1+2+3 | Snapshot commits before the effect spawns; a rollback or restart loses the resume | Medium | 93 | Confirmed, Still valid, `known` | E1 | CA3 one band with `F-SEN-001`, `F3-CORE-032`, `F3-SEN-004`: recoverable by hand, daily one-block orphans (was High); known #961, #973 |
| [`F-CORE-032`](../findings/F-CORE-032.md) | none | 1 | A panicking effect task is logged and skipped; the resume is gone | Low | 45 | Plausible, Still valid | E1 (mechanism) | re-validation; no reachable panic site (run 3: L-rust Q71) |
| [`F-CORE-033`](../findings/F-CORE-033.md) | `F2-CORE-035` | 1+2 | Effect fan-out unbounded: no cap, queue or backpressure | Medium | 70 | Confirmed, Still valid | E2 | re-validation (#1017 corrects one run-2 sub-claim only) |
| [`F-CORE-034`](../findings/F-CORE-034.md) | `F2-CORE-010`, `F3-CORE-001` | 1+2+3 | Watcher errors retried every 100 ms forever; under a rate limit the per-event fallback stalls the indexer for good | Medium | 85 | Confirmed, Changed | E1 | `F3-CORE-001`'s driver and real-binary stall (CA3 85); #1062 removed the handbook backoff sentence |
| [`F-CORE-035`](../findings/F-CORE-035.md) | (`F2-CORE-067` metrics leg) | 1 | Every `tx::Error::Rpc` is intermittent and swallowed forever | Medium | 78 | Confirmed, Changed | E2 | re-validation (#999 adds `eth_getProof`; one case overlaps settled L2) |
| [`F-CORE-036`](../findings/F-CORE-036.md) | (`F-XC-002` union) | 1 | `Debug` bound on `Effect` and `Resume`; five trace sinks (secret leg refuted) | Low | 85 | Confirmed, Still valid | E1 | re-validation (redaction holds for the Direct Nonce payloads) |
| [`F-CORE-038`](../findings/F-CORE-038.md) | none | 1 | `kdf::derive_key` multi-part `info` is plain concatenation | Informational | 90 | Confirmed, Still valid | E1 | re-validation; one caller (run 3: R2 H14) |
| [`F-CORE-039`](../findings/F-CORE-039.md) | `F2-CORE-034` | 1+2 | Graceful shutdown blocked inside `update`; inline housekeeping is a second unbounded await | Low | 72 | Confirmed, Still valid | E2 | re-validation |
| [`F-CORE-040`](../findings/F-CORE-040.md) | none | 1 | `next_input` `select!` drops the watcher's in-flight RPC on every resume | Low | 65 | Plausible, Still valid, `known` | E2 | re-validation; cancel safety of `next_input` is a known item (`settled.md` section 2) |
| [`F-CORE-060`](../findings/F-CORE-060.md) | `F2-CORE-060`, `F3-CORE-060` (accepted arm) | 1+2+3 | Fee ratchet unbounded; underpriced rejections bump every block; the cap never bounds against the base fee | High | 98 | Confirmed, Still valid | E1 | re-validation; run 3's counterpart rates only the accepted arm (Low); PC3 did not weigh the underpriced arm (section 7) |
| [`F-CORE-061`](../findings/F-CORE-061.md) | `F2-CORE-062` | 1+2 | First-submission underpriced rejection retried forever at the same fee | Medium | 65 | Plausible, Still valid | E1 (in-crate) | re-validation; node wording `I` (run 3: R3 O2) |
| [`F-CORE-062`](../findings/F-CORE-062.md) | (`F2-CORE-061` gap, `F2-CORE-064`) | 1 | Allocated nonce never released; one bad nonce wedges the queue | Medium | 60 | Plausible, Still valid | E2 (gap E1) | re-validation (a batched variant wedges the batched queue) |
| [`F-CORE-063`](../findings/F-CORE-063.md) | `F2-CORE-061`, `F-CORE-068` residual | 1+2 | Execution inferred from the account nonce; the mark is irrevocable, and a mid-batch `CallFailed` is marked executed | Medium | 55 | Plausible, Still valid | E1 (irrevocability) | re-validation; `F-CORE-068`'s residual folded here (section 1.6) |
| [`F-CORE-064`](../findings/F-CORE-064.md) | `F2-CORE-064` | 1+2 | `expires_at` void once a nonce is allocated | Medium | 90 | Confirmed, Still valid | E1 | re-validation |
| [`F-CORE-065`](../findings/F-CORE-065.md) | `F2-CORE-066` | 1+2 | Transaction tables bound to neither chain id nor signer | Low | 90 | Confirmed, Still valid, `known` | E1 | re-validation (rotated signer executed; 70 to 90) |
| [`F-CORE-066`](../findings/F-CORE-066.md) | `F2-CORE-065`, `F3-CORE-061` | 1+2+3 | `tx::Config` accepts degenerate values (zero in-flight, zero window, NaN cap) | Informational | 90 | Confirmed, Still valid | E1 | `F3-CORE-061`'s CA3 value: operator values under A1, PC3 theoretical (run 1 Medium, run 2 Low as dissent) |
| [`F-CORE-067`](../findings/F-CORE-067.md) | `F2-CORE-063`, `F2-CORE-032` | 1+2 | No idempotency key on `enqueue`; replays re-submit actions | Medium | 98 | Confirmed, Still valid, `known` | E1 | re-validation (#1015 adds a replay source) |
| [`F2-CORE-003`](../findings/F2-CORE-003.md) | none | 2 | Default-path log fetch has no completeness check; `may_contain_log` is dead code | Informational | 80 | Confirmed, Still valid | E1 | re-validation; PC3's Nethermind 1.39+ fact asks for a fresh practicality look (section 7) |
| [`F2-CORE-007`](../findings/F2-CORE-007.md) | `F3-CORE-030`, `F3-CORE-003` | 2+3 | A restart against a node behind the oldest snapshot exits with `BadUpdate` | Low | 95 | Confirmed, Still valid | E1 | `F3-CORE-030`'s real-binary reproduction (CA3 95); reviewer and Critic Medium, PC3 and CA3 Low |
| [`F2-CORE-011`](../findings/F2-CORE-011.md) | none | 2 | No rollback anchor on a fresh start; an in-window reorg exits `MissingSnapshot` | Low | 92 | Confirmed, Still valid | E1 | re-validation |
| [`F2-CORE-067`](../findings/F2-CORE-067.md) | (`F-CORE-035` observability leg) | 2 | Transaction queue exports no in-flight or fee metrics | Informational | 85 | Confirmed, Changed | E2 | re-validation (#1003 batch histograms narrow the gap) |

### 2.2 Validator (21)

| Canonical | Counterparts | Runs | Short title | Severity | Cert. | Status | Basis | Deciding evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| [`F-VAL-002`](../findings/F-VAL-002.md) | `F2-VAL-002` | 1+2 | ECDH share pad is a raw symmetric x-coordinate | Low | 90 | Confirmed, Changed, `known` | E1 | re-validation: the proof of possession (#1027) narrows the leak to the pair; the team closed the KDF change (#1026) |
| [`F-VAL-003`](../findings/F-VAL-003.md) | `F2-VAL-006` | 1+2 | Complaint responses unconditional and unbounded per plaintiff | Medium | 82 | Confirmed, Still valid | E1 | re-validation (plaintext response to an unshared plaintiff executed) |
| [`F-VAL-004`](../findings/F-VAL-004.md) | `F2-VAL-063`, `F2-VAL-005` | 1+2 | Lost or failed `KeyGenSetup` never re-issued: rollover epoch forfeited (genesis accepted under A16) | Medium (rollover); genesis Informational `known` | 69 | Plausible, Still valid | E1 (gap) | re-validation; run 3's `F3-CORE-032` reaches the same lost setup through a rollback |
| [`F-VAL-034`](../findings/F-VAL-034.md) | none | 1 | `handle_nonces` applies a resume without checking the signature id (outcome benign) | Low | 55 | Plausible, Still valid | E1 | re-validation; run 3's R5 reached the same benign outcome (Q33) |
| [`F-VAL-035`](../findings/F-VAL-035.md) | none | 1 | Nonce JSON in the secret store is not zeroized | Informational | 70 | Confirmed, Changed | E2 | re-validation (#1033, #1040 narrowed leg b and removed leg c) |
| [`F-VAL-037`](../findings/F-VAL-037.md) | none | 1 | Merkle trees pad with zero; no leaf and node separation | Informational | 60 | Plausible, Still valid | E1 | re-validation; run 3's L-crypto found it not exploitable (Q66: the zero leaf is `address(0)`) |
| [`F-VAL-040`](../findings/F-VAL-040.md) | `F2-VAL-033` | 1+2 | The last sharer becomes responsible for the share-round restart | Low | 70 | Confirmed, Changed | E1 | re-validation (#1008, #1036 removed the reveal-round leg) |
| [`F-VAL-060`](../findings/F-VAL-060.md) | `F2-VAL-061` (core half `F-CORE-006`) | 1+2 | Coordinator and Consensus events accepted from any watched address | Medium | 90 | Confirmed, Changed | E1 | PC3 and CA3: precondition false on Aegis, executed (the listed oracle is the immutable SentinelOracle, no `DELEGATECALL`, no shared topic0) (was High conditional) |
| [`F-VAL-061`](../findings/F-VAL-061.md) | (`F2-VAL-030`, `031`, `062`, `063` policy leg) | 1+2 | Every effect error maps to `Resume::Noop`; no retry, no marker | Medium | 90 | Confirmed, Changed | E1 | re-validation: #1038, #1040 removed the NonceTree placeholder and the generator race; a lost resume costs one ceremony (was High) |
| [`F-VAL-062`](../findings/F-VAL-062.md) | (`F-XC-002` union) | 1 | Secret-bearing effects derive `Debug` and are printed at `warn` (redacted) | Informational | 90 | Confirmed (hygiene), Changed | E1 | re-validation |
| [`F-VAL-063`](../findings/F-VAL-063.md) | `F2-VAL-067`, `F3-VAL-061` | 1+2+3 | Consensus-critical configuration unvalidated (participants, salt, `blocks_per_epoch`, oracles, timing relations) | Low | 90 | Confirmed, Still valid | E1 | `F3-VAL-061`'s CA3 value: operator configuration under A1, PC3 gate 3 (runs 1 and 2 and rv-val Medium as dissent) |
| [`F-VAL-064`](../findings/F-VAL-064.md) | (exit half `F-CORE-030`, root half `F-XC-004`) | 1+2 | Validator deployment residual: `/health` unreachable in the shipped deployment | Low | 72 | Confirmed, Still valid | E2 | re-validation |
| [`F-VAL-065`](../findings/F-VAL-065.md) | (core side `F-CORE-067`) | 1 | `SetValidatorStaker` re-queued on every restart before inclusion | Low | 70 | Confirmed, Changed | E2 | re-validation (Preprocess leg removed, duplicate `Sign` leg defused by #1036); run 3's R6 measured one transaction per restart (Q44) |
| [`F-VAL-066`](../findings/F-VAL-066.md) | `F2-VAL-035` | 1+2 | Unordered reconcile spawn and inline prune; a short outage loses a group's secrets | Low | 65 | Plausible, Changed | E1 | re-validation (#1015, #1017 changed the sequencing; the documented store concession remains) (was Medium) |
| [`F2-VAL-007`](../findings/F2-VAL-007.md) | none | 2 | `active_epoch` advances only through self-staged epochs; phantom rollover session | Low | 65 | Plausible, Still valid | E1 | re-validation (consequences executed, trigger unforced) |
| [`F2-VAL-064`](../findings/F2-VAL-064.md) | none | 2 | Documented `--config-file=<path>` spelling rejected by `argh` (both binaries) | Informational | 95 | Confirmed, Still valid | E1 | re-validation |
| [`F3-VAL-030`](../findings/F3-VAL-030.md) | none | 3 | Shares sent on a non-final oracle approval can attest a transaction on a branch where the oracle did not approve it | High | 97 | Confirmed, New | E1 | CA3: a real SentinelOracle at Aegis windows and a depth-4 reorg give `FROZEN` plus `TransactionAttested`; PC3 conditional (no reorg of 3 or more verified on Gnosis); present at `fe9e84c` |
| [`F3-VAL-031`](../findings/F3-VAL-031.md) | none | 3 | A one-block reorg at the nonce-commitment deadline splits honest shares; the restart rule drops the ceremony | Low | 60 | Plausible, New | E2 (drop rule E1) | CA3; PC3 conditional |
| [`F3-VAL-032`](../findings/F3-VAL-032.md) | none | 3 | Secret store documentation overstates the nonce guarantees (code safe) | Informational | 90 | Confirmed, New | E1 | CA3 |
| [`F3-VAL-062`](../findings/F3-VAL-062.md) | `F2-VAL-004` (folded) | 2+3 | A restart early in a key generation drops peer secret shares; the validator loses that epoch | Medium | 90 | Confirmed, New | E1 | CA3; PC3 conditional (a restart in the first 20 to 25 s after an epoch boundary with 30 s or more down) |
| [`F3-VAL-100`](../findings/F3-VAL-100.md) | none | 3 | A reorg that puts a participant's epoch-1 commitment ahead of the final genesis confirmation excludes it from epoch 1 | Low | 88 | Confirmed, New | E1 | CA3; PC3 conditional (one window per deployment, about one in a million) |

### 2.3 Sentinel (16)

| Canonical | Counterparts | Runs | Short title | Severity | Cert. | Status | Basis | Deciding evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| [`F-SEN-001`](../findings/F-SEN-001.md) | `F2-SEN-001` (a, b), `F3-SEN-004` | 1+2+3 | Replay or rollback ignores the sentinel's own `Committed`; no reveal; bond locked or slashed | Medium | 99 | Confirmed, Still valid, `known` | E1 | CA3 one band: recoverable by a manual `claim`, every restart in a commit phase (was High); known #961, #973 |
| [`F-SEN-002`](../findings/F-SEN-002.md) | `F2-SEN-002`, `F3-SEN-002`, `F3-XC-001` | 1+2+3 | Commits before the engine verdict are not tallied; early finalize drops or parks a bonded request | Medium | 90 | Confirmed, Changed | E1 | PC3 and CA3: funds locked, not lost; #964 recovers the mirror case (was High) |
| [`F-SEN-004`](../findings/F-SEN-004.md) | `F2-SEN-005` | 1+2 | No bound on concurrent engine checks or outstanding bonds | Medium | 78 | Confirmed, Changed | E1 | re-validation: at Aegis parameters starvation needs more than 16 proposals per block; bond exposure is the operative branch |
| [`F-SEN-005`](../findings/F-SEN-005.md) | `F2-SEN-004` | 1+2 | `WaitingForOutcome` never expires (the dispute half fixed by #974) | Medium | 95 | Confirmed, Changed | E1 | re-validation |
| [`F-SEN-006`](../findings/F-SEN-006.md) | `F2-SEN-007` | 1+2 | Actions not idempotent under replay; `handle_arbitration_timeout` claims without a bond | Low | 90 | Confirmed, Still valid | E1 | re-validation (#1015 adds a replay site) |
| [`F-SEN-007`](../findings/F-SEN-007.md) | `F2-SEN-006` | 1+2 | No startup or per-request pre-flight; silent per-request gas burn | Low | 82 | Confirmed, Still valid | E1 | re-validation |
| [`F-SEN-008`](../findings/F-SEN-008.md) | `F3-SEN-001` | 1+3 | Direct-mode `approve` gas limit 55,000 runs out of gas on USDC.e; no Aegis sentinel on the documented configuration commits | High | 97 | Confirmed, Changed | E1 | PC3 practical, CA3 High; `F3-SEN-001`'s real-binary run (97); the USDT-family half does not apply on Aegis (was Low 52) |
| [`F-SEN-009`](../findings/F-SEN-009.md) | (`F2-SEN-006` claim 6, `F2-SEN-008` claim 3, `F2-SEN-009` b3) | 1+2 | Engine timeout derived from an unvalidated `voting_window` | Low | 82 | Confirmed, Still valid, `known` | E1 | re-validation |
| [`F-SEN-010`](../findings/F-SEN-010.md) | (`F2-SEN-009` b1) | 1+2 | Sample ships zero addresses and a placeholder key that parse and start | Informational | 85 | Confirmed, Still valid, `known` | E1 | re-validation |
| [`F-SEN-011`](../findings/F-SEN-011.md) | (`F2-CORE-030` in-flight case, `F3-CORE-032` restore point) | 1 | A restart orphans an in-flight check whose proposal predates the anchor | Low | 82 | Confirmed, Still valid, `known` | E1 | re-validation; known #973; no bond at stake, so it stays Low |
| [`F-SEN-012`](../findings/F-SEN-012.md) | `F2-SEN-008` | 1+2 | Engine client makes one attempt; any failure is a permanent abstention | Low | 90 | Confirmed, Still valid | E1 | re-validation; `F3-XC-004` settles the userinfo sub-claim of the run-2 extension the other way |
| [`F-SEN-014`](../findings/F-SEN-014.md) | `F3-XC-006` | 1+3 | The early `Finalize` always reverts (all K since #964) | Informational | 95 | Confirmed, Changed | E1 | `F3-XC-006`'s CA3 value |
| [`F-SEN-015`](../findings/F-SEN-015.md) | `F2-SEN-001` (c) | 1+2 | A restart warp prunes the recorded verdict before the replayed check reads it; the committed vote is re-decided and never revealed | Medium | 95 | Confirmed, Changed | E1 | CA3-FIX Not fixed (15 of 20 restarts in the window, control 0 of 6), CA3 16 of 16; #978, #979 fixed the live-reorg route (was High) |
| [`F2-SEN-003`](../findings/F2-SEN-003.md) | `F3-SEN-003`, `F3-XC-002` | 2+3 | `finalize()` drops a bonded entry when the own reveal was not observed; the recovery path cannot run | Medium | 95 | Confirmed, Changed, `known` | E1 | `F3-SEN-003`'s real-binary run (CA3 95); #1017 closed trigger b |
| [`F2-SEN-011`](../findings/F2-SEN-011.md) | none | 2 | `handle_arbitration_timeout` doc and metric assume a full refund | Informational | 80 | Confirmed, Changed | E1 | re-validation (#1017 removes the warp trigger; #974 adds a second metric site) |
| [`F3-SEN-060`](../findings/F3-SEN-060.md) | none | 3 | The dispute-timeout arm drops the entry even when its own `timeoutArbitration` and `claim` revert (introduced by #974) | Low | 85 | Confirmed, New | E1 (sentinel half) | CA3; PC3 conditional (Circle blacklist, frozen dispute, 28 days of silence); the contract half is being re-derived offline (section 7) |

### 2.4 Cross-cutting (19)

| Canonical | Counterparts | Runs | Short title | Severity | Cert. | Status | Basis | Deciding evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| [`F-XC-001`](../findings/F-XC-001.md) | `F2-XC-009` | 1+2 | No release profile; overflow checks off in shipped binaries | Informational | 93 | Confirmed, Still valid | E1 | re-validation |
| [`F-XC-003`](../findings/F-XC-003.md) | none | 1 | `deny_unknown_fields` with `flatten` behaves correctly but has no in-tree test | Informational | 96 | Confirmed (test gap), Still valid | E1 | re-validation; run 3's R2 and R6 re-executed the behaviour |
| [`F-XC-004`](../findings/F-XC-004.md) | `F2-XC-003`, `F2-VAL-068`, `F2-SEN-009` b4 | 1+2 | Runtime images run as root on floating tags; no toolchain pin | Low | 85 | Confirmed, Still valid | E2 | re-validation |
| [`F-XC-006`](../findings/F-XC-006.md) | `F2-XC-006` | 1+2 | Persistent state not bound to chain, deployment or signer | Low | 90 | Confirmed, Still valid | E1 | re-validation (two local Anvil chains) |
| [`F-XC-007`](../findings/F-XC-007.md) | (`F2-XC-004` and `F3-XC-007` gate halves) | 1+2+3 | No `cargo audit` gate in CI | Informational | 92 | Confirmed, Still valid | E1 | re-validation |
| [`F-XC-008`](../findings/F-XC-008.md) | (`F2-SEN-008` items 2, 3) | 1 | Unconfigured `reqwest` engine client follows redirects and honours proxy variables | Informational | 90 | Confirmed, Still valid | E1 | re-validation (a 307 redirect re-posts the transaction) |
| [`F-XC-009`](../findings/F-XC-009.md) | `F2-XC-008`, `F2-SEN-009` b1 | 1+2 | Samples ship a parseable well-known key and a live mainnet RPC | Low | 75 | Confirmed, Still valid | E1 | re-validation |
| [`F-XC-011`](../findings/F-XC-011.md) | `F2-XC-004`, `F3-XC-007` | 1+2+3 | Dependency advisories linked, none reachable from the services | Informational | 90 | Confirmed, Still valid | E1 | re-validation and `F3-XC-007` (the same five advisories) |
| [`F-XC-050`](../findings/F-XC-050.md) | (`F2-VAL-061` is its precondition) | 1 | No DKG membership check; cardinality close; silent `None` finalisation | Medium (rollover); genesis Informational `known` | 48 | Plausible, Still valid | E2 | re-validation; its precondition is false on Aegis per PC3's `F-VAL-060` check (band unsettled, section 7) |
| [`F-XC-051`](../findings/F-XC-051.md) | none | 1 | `verify_commitment` accepts a short commitment (structural checks left to frost-core) | Informational | 90 | Confirmed, Still valid | E1 | re-validation (first run of the run-1 PoC); needs `F-VAL-060`'s precondition (section 1.6) |
| [`F2-XC-002`](../findings/F2-XC-002.md) | `F2-VAL-065`, `F3-XC-005`, `F3-VAL-060`, `F3-SEN-005` | 2+3 | Sample database URLs lack `?mode=rwc`; a fresh node fails to start | Low | 95 | Confirmed, Still valid | E1 | `F3-XC-005`'s CA3 value |
| [`F2-XC-005`](../findings/F2-XC-005.md) | none | 2 | Secret deletion is logical only (bundled SQLite without `SECURE_DELETE`) | Informational | 90 | Confirmed, Changed | E1 | re-validation |
| [`F2-XC-007`](../findings/F2-XC-007.md) | `F2-SEN-009` b2 | 2 | Signer key lingers in un-zeroized configuration buffers | Informational | 75 | Confirmed, Changed | E2 | re-validation (#1046 removed the error-retention leg) |
| [`F2-XC-050`](../findings/F2-XC-050.md) | `F2-CORE-036`, `F3-XC-030` | 2+3 | Snapshot committed and pruned before the actions are queued; a crash in the window loses them (wider since #1017) | Low | 92 | Confirmed, Still valid | E1 | re-validation's fault injection; run 3 rediscovered it (C3-XC over R8's R-12) |
| [`F3-XC-003`](../findings/F3-XC-003.md) | none | 3 | Dockerfiles say the binaries use the system SQLite; the build bundles 3.51.3 | Informational | 95 | Confirmed, New | E1 | CA3 |
| [`F3-XC-004`](../findings/F3-XC-004.md) | (`F2-SEN-008` claim 6, side claim) | 3 | Engine errors log the full engine URL, path and query tokens included | Informational | 90 | Confirmed, New, `known` | E1 | CA3; #1046 trade-off |
| [`F3-XC-040`](../findings/F3-XC-040.md) | (residual of the Fixed `F2-XC-001`) | 3 | A key pasted into a typed config field is printed in the startup parse error | Informational | 90 | Confirmed, New | E1 | CA3 |
| [`F3-XC-100`](../findings/F3-XC-100.md) | `F3-VAL-001` | 3 | A last committer forces an honest verifying share to the identity; the contract rejects it and every Aegis rollover is skipped | High | 95 | Confirmed, New | E1 | CA3, three routes; PC3 conditional (one Byzantine member, inside A2); the team's review of #1025 rates it Medium |
| [`F3-XC-130`](../findings/F3-XC-130.md) | `F3-CORE-031` | 3 | An RPC URL with an unsupported scheme is printed with its key at startup | Informational | 95 | Confirmed, New, `known` | E1 | CA3; #113, #1046 |

## 3. Fold index

### 3.1 Run-3 duplicates (files stay this run)

| Duplicate | Run-3 canonical | Ledger canonical |
| --- | --- | --- |
| [`F3-CORE-003`](../findings/F3-CORE-003.md) | [`F3-CORE-030`](../findings/F3-CORE-030.md) | [`F2-CORE-007`](../findings/F2-CORE-007.md) |
| [`F3-CORE-031`](../findings/F3-CORE-031.md) | [`F3-XC-130`](../findings/F3-XC-130.md) | [`F3-XC-130`](../findings/F3-XC-130.md) |
| [`F3-SEN-005`](../findings/F3-SEN-005.md) | [`F3-XC-005`](../findings/F3-XC-005.md) | [`F2-XC-002`](../findings/F2-XC-002.md) |
| [`F3-VAL-001`](../findings/F3-VAL-001.md) | [`F3-XC-100`](../findings/F3-XC-100.md) | [`F3-XC-100`](../findings/F3-XC-100.md) |
| [`F3-VAL-060`](../findings/F3-VAL-060.md) | [`F3-XC-005`](../findings/F3-XC-005.md) | [`F2-XC-002`](../findings/F2-XC-002.md) |
| [`F3-XC-001`](../findings/F3-XC-001.md) | [`F3-SEN-002`](../findings/F3-SEN-002.md) | [`F-SEN-002`](../findings/F-SEN-002.md) |
| [`F3-XC-002`](../findings/F3-XC-002.md) | [`F3-SEN-003`](../findings/F3-SEN-003.md) | [`F2-SEN-003`](../findings/F2-SEN-003.md) |

### 3.2 Run-3 counterparts of earlier canonical rows (files stay)

| Run-3 file | Relation | Ledger canonical |
| --- | --- | --- |
| [`F3-CORE-001`](../findings/F3-CORE-001.md) | EXTENDS | [`F-CORE-034`](../findings/F-CORE-034.md) |
| [`F3-CORE-002`](../findings/F3-CORE-002.md) | CONFIRMS | [`F-CORE-002`](../findings/F-CORE-002.md) |
| [`F3-CORE-004`](../findings/F3-CORE-004.md) | CONFIRMS | [`F-CORE-001`](../findings/F-CORE-001.md) |
| [`F3-CORE-005`](../findings/F3-CORE-005.md) | CONFIRMS | [`F-CORE-008`](../findings/F-CORE-008.md) |
| [`F3-CORE-030`](../findings/F3-CORE-030.md) | CONFIRMS | [`F2-CORE-007`](../findings/F2-CORE-007.md) |
| [`F3-CORE-032`](../findings/F3-CORE-032.md) | CONFIRMS | [`F-CORE-031`](../findings/F-CORE-031.md) |
| [`F3-CORE-060`](../findings/F3-CORE-060.md) | CONFIRMS (accepted arm only) | [`F-CORE-060`](../findings/F-CORE-060.md) |
| [`F3-CORE-061`](../findings/F3-CORE-061.md) | CONFIRMS | [`F-CORE-066`](../findings/F-CORE-066.md) |
| [`F3-SEN-001`](../findings/F3-SEN-001.md) | CONFIRMS (gas-limit half) | [`F-SEN-008`](../findings/F-SEN-008.md) |
| [`F3-SEN-002`](../findings/F3-SEN-002.md) | CONFIRMS | [`F-SEN-002`](../findings/F-SEN-002.md) |
| [`F3-SEN-003`](../findings/F3-SEN-003.md) | CONFIRMS | [`F2-SEN-003`](../findings/F2-SEN-003.md) |
| [`F3-SEN-004`](../findings/F3-SEN-004.md) | CONFIRMS | [`F-SEN-001`](../findings/F-SEN-001.md) |
| [`F3-VAL-061`](../findings/F3-VAL-061.md) | CONFIRMS (participant, salt and epoch-length legs) | [`F-VAL-063`](../findings/F-VAL-063.md) |
| [`F3-XC-005`](../findings/F3-XC-005.md) | CONFIRMS | [`F2-XC-002`](../findings/F2-XC-002.md) |
| [`F3-XC-006`](../findings/F3-XC-006.md) | CONFIRMS | [`F-SEN-014`](../findings/F-SEN-014.md) |
| [`F3-XC-007`](../findings/F3-XC-007.md) | CONFIRMS (gate half also under `F-XC-007`) | [`F-XC-011`](../findings/F-XC-011.md) |
| [`F3-XC-030`](../findings/F3-XC-030.md) | CONFIRMS | [`F2-XC-050`](../findings/F2-XC-050.md) |
| [`F3-XC-160`](../findings/F3-XC-160.md) | EXTENDS | [`F-CORE-003`](../findings/F-CORE-003.md) |

### 3.3 Earlier rows folded in run 3 (files leave)

| Earlier ID | Folded into | Why |
| --- | --- | --- |
| `F2-VAL-004` | [`F3-VAL-062`](../findings/F3-VAL-062.md) | same mechanism; #1017 made the trigger a deterministic restart path that only `F3-VAL-062` executed; no team thread. Its run-3 execution of both cases stays at `poc/F2-VAL-004/run3/` |
| `F-CORE-068` | [`F-CORE-063`](../findings/F-CORE-063.md) | the whole-batch half is settled L4; the residual is `F-CORE-063`'s loss (section 1.6). Also in the resolved index as Superseded |
| `F-XC-002` | [`F-VAL-062`](../findings/F-VAL-062.md) and [`F-CORE-036`](../findings/F-CORE-036.md) | union row, never counted; both halves are live rows |

### 3.4 Run-2 files folded in run 2 (51 files leave)

The relation is the one the run-2 reconciliation recorded; the canonical is the row that carries it now. A canonical marked Fixed or Moot left the folder in run 3 (section 4).

| Run-2 ID | Canonical now | Relation |
| --- | --- | --- |
| `F2-CORE-001` | [`F-CORE-001`](../findings/F-CORE-001.md) | CONFIRMS |
| `F2-CORE-002` | [`F-CORE-002`](../findings/F-CORE-002.md) | CONFIRMS |
| `F2-CORE-004` | [`F-CORE-006`](../findings/F-CORE-006.md) | CONFIRMS |
| `F2-CORE-005` | [`F-CORE-004`](../findings/F-CORE-004.md) | EXTENDS |
| `F2-CORE-006` | [`F-CORE-003`](../findings/F-CORE-003.md) | CONFIRMS |
| `F2-CORE-008` | [`F-CORE-005`](../findings/F-CORE-005.md) | CONFIRMS |
| `F2-CORE-009` | [`F-CORE-007`](../findings/F-CORE-007.md) | CONFIRMS |
| `F2-CORE-010` | [`F-CORE-034`](../findings/F-CORE-034.md) | EXTENDS |
| `F2-CORE-030` | [`F-CORE-031`](../findings/F-CORE-031.md) | EXTENDS |
| `F2-CORE-031` | [`F-CORE-030`](../findings/F-CORE-030.md) | CONFIRMS |
| `F2-CORE-032` | [`F-CORE-067`](../findings/F-CORE-067.md) | CONFIRMS |
| `F2-CORE-033` | [`F-CORE-011`](../findings/F-CORE-011.md) | CONFIRMS, narrows |
| `F2-CORE-034` | [`F-CORE-039`](../findings/F-CORE-039.md) | CONFIRMS |
| `F2-CORE-035` | [`F-CORE-033`](../findings/F-CORE-033.md) | EXTENDS |
| `F2-CORE-036` | [`F2-XC-050`](../findings/F2-XC-050.md) | duplicate |
| `F2-CORE-060` | [`F-CORE-060`](../findings/F-CORE-060.md) | EXTENDS |
| `F2-CORE-061` | [`F-CORE-063`](../findings/F-CORE-063.md) | EXTENDS (also `F-CORE-062`) |
| `F2-CORE-062` | [`F-CORE-061`](../findings/F-CORE-061.md) | EXTENDS |
| `F2-CORE-063` | [`F-CORE-067`](../findings/F-CORE-067.md) | CONFIRMS |
| `F2-CORE-064` | [`F-CORE-064`](../findings/F-CORE-064.md) | EXTENDS (also `F-CORE-062`) |
| `F2-CORE-065` | [`F-CORE-066`](../findings/F-CORE-066.md) | CONFIRMS |
| `F2-CORE-066` | [`F-CORE-065`](../findings/F-CORE-065.md) | CONFIRMS |
| `F2-VAL-001` | `F-VAL-001` (Fixed) | CONFIRMS |
| `F2-VAL-002` | [`F-VAL-002`](../findings/F-VAL-002.md) | CONFIRMS |
| `F2-VAL-005` | [`F-VAL-004`](../findings/F-VAL-004.md) | CONFIRMS |
| `F2-VAL-006` | [`F-VAL-003`](../findings/F-VAL-003.md) | CONFIRMS |
| `F2-VAL-030` | `F-VAL-030` (Moot) | EXTENDS |
| `F2-VAL-032` | `F-VAL-032` (Fixed) | EXTENDS |
| `F2-VAL-033` | [`F-VAL-040`](../findings/F-VAL-040.md) | CONFIRMS |
| `F2-VAL-035` | [`F-VAL-066`](../findings/F-VAL-066.md) | EXTENDS |
| `F2-VAL-060` | `F2-XC-001` (Fixed) | duplicate |
| `F2-VAL-061` | [`F-VAL-060`](../findings/F-VAL-060.md) | EXTENDS |
| `F2-VAL-062` | `F-VAL-030` (Moot) | CONFIRMS |
| `F2-VAL-063` | [`F-VAL-004`](../findings/F-VAL-004.md) | EXTENDS |
| `F2-VAL-065` | [`F2-XC-002`](../findings/F2-XC-002.md) | duplicate |
| `F2-VAL-066` | [`F-CORE-030`](../findings/F-CORE-030.md) | CONFIRMS |
| `F2-VAL-067` | [`F-VAL-063`](../findings/F-VAL-063.md) | EXTENDS |
| `F2-VAL-068` | [`F-XC-004`](../findings/F-XC-004.md) | CONFIRMS |
| `F2-SEN-001` | [`F-SEN-001`](../findings/F-SEN-001.md) (variant c: `F-SEN-015`) | CONFIRMS |
| `F2-SEN-002` | [`F-SEN-002`](../findings/F-SEN-002.md) | CONFIRMS |
| `F2-SEN-004` | [`F-SEN-005`](../findings/F-SEN-005.md) | CONFIRMS |
| `F2-SEN-005` | [`F-SEN-004`](../findings/F-SEN-004.md) | CONFIRMS |
| `F2-SEN-006` | [`F-SEN-007`](../findings/F-SEN-007.md) | CONFIRMS |
| `F2-SEN-007` | [`F-SEN-006`](../findings/F-SEN-006.md) | EXTENDS |
| `F2-SEN-008` | [`F-SEN-012`](../findings/F-SEN-012.md) | CONFIRMS |
| `F2-SEN-009` | [`F-XC-009`](../findings/F-XC-009.md) | CONFIRMS |
| `F2-XC-003` | [`F-XC-004`](../findings/F-XC-004.md) | CONFIRMS |
| `F2-XC-004` | [`F-XC-011`](../findings/F-XC-011.md) (gate half `F-XC-007`) | EXTENDS |
| `F2-XC-006` | [`F-XC-006`](../findings/F-XC-006.md) | CONFIRMS |
| `F2-XC-008` | [`F-XC-009`](../findings/F-XC-009.md) | EXTENDS |
| `F2-XC-009` | [`F-XC-001`](../findings/F-XC-001.md) | CONFIRMS |

## 4. Resolved index

One row per finding that leaves the folder as resolved (`PROMPT.md` Section 12). The evidence is in the run-3 review commit; the finding files and their uncited PoC folders are deleted after it.

| ID | Short title | Verdict | PR or decision | How verified | Evidence |
| --- | --- | --- | --- | --- | --- |
| `F-VAL-001` | DKG encryption key `q` had no proof of possession; a peer's signing share was recoverable (Critical 97) | Fixed | #1027 (`14750ea`) on #1022 (`010a09f`) | rv-val: original PoC adapted (0 of 3 pass), 8 Rust variants and 7 onchain tests pass, PoP-removal mutation brings the attack back; CA3-FIX 95: 6 new Rust and 3 onchain variants, mutation bites | the run-3 review commit |
| `F-VAL-005` | A reorg across the `KeyGen` block deleted the DKG secrets (High at `2893917`) | Fixed | #909 (`8b88f4a`), #910 (`c87c054`), before run 2 | rv-val: store tests 12 of 12; CA3-FIX 88: live reorg past the epoch-1 round start keeps the commitment byte-identical, 3 of 3; its lead L2 became `F3-VAL-100` | the run-3 review commit |
| `F-VAL-032` | `handle_sign` dropped a waiting session on a `Sign` from any untracked group (High 93; team-accepted risk) | Fixed (was accepted risk) | #1036 (`82d5419`), #1038 (`a9023ac`) | rv-val: 6 variants pass, restoring the drop arm fails 3; CA3-FIX 93: 4 new oracle-backed variants, the restored arm fails V1, V2, V5 and W3. The accepted-risk entry in `settled.md` no longer describes reachable behaviour | the run-3 review commit |
| `F-VAL-067` | Rust counted complaints cumulatively while the contract netted them (Low 70) | Fixed | #1019 (`97f4cd1`) | rv-val: Rust aborts at the threshold-th complaint, the contract test passes, a netting mutation fails it | the run-3 review commit |
| `F2-VAL-003` | A restart warp over an epoch boundary dropped the key-generation round (Medium 85) | Fixed | #1017 (`24eaf8d`) | rv-val 4 of 4 with mutation; CA3-FIX 90, deletion conditional on `F3-VAL-062` staying live (it does) | the run-3 review commit |
| `F-SEN-003` | A warp delivered no `NewBlock`; reveals were discarded and a bogus timeout finalize followed (Low 80) | Fixed | #1017 (`24eaf8d`) with #1015 | rv-sen route b passes; CA3-FIX 92: 3 variants, removing the warp tick fails all three; deletion conditional on `F2-SEN-003` and `F3-SEN-003` staying live (they do) | the run-3 review commit |
| `F2-SEN-010` | Commits mined in the commit-deadline block were never tallied (#914) (High 93) | Fixed | #1014 (`fef8cfd`) with #1015, #1017 | rv-sen 5 variants; CA3-FIX 92: warp ending at and warp split at the deadline both tally the peer; the stopgap and the warp-tick removal each fail them | the run-3 review commit |
| `F2-XC-001` | Any configuration parse error printed the whole file, signer key included (High 95) | Fixed | #1046 (`55d811e`); env route #1007 (`1a292af`) | rv-xc: 23 variants and a control, mutation check, 213,312-document span fuzz; CA3-FIX 93: 319,360-document echo fuzz through the real loader; the narrower residual is `F3-XC-040` | the run-3 review commit |
| `F-VAL-030` | A failed `NonceTree` left a phantom reservation and an `expected_chunk` cascade (High 97) | Moot | code removed by #1038 (`a9023ac`), #1040 (`392d7c8`); replacement #1035 (`3154a05`), #1036 | rv-val: a lost `GenerateNonces` costs one ceremony; CA3-FIX 92: handler-level replay, nonce-overwrite mutation fails the variant | the run-3 review commit |
| `F-VAL-031` | A dead nonce-generation worker was never detected (Low 42) | Moot | code removed by #1039 (`5e694c4`), #1040 (`392d7c8`) | rv-val: the file deletion in git history; no thread or generator left | the run-3 review commit |
| `F-VAL-036` | `observe` accepted a non-monotonic sequence (Low 40) | Moot | code removed by #1038 (`a9023ac`) | rv-val: `git log -S 'fn observe'`; the sequence is only logged | the run-3 review commit |
| `F-VAL-038` | Chunk generation saturated every core (Low 55) | Moot | code removed by #1040 (`392d7c8`) | rv-val: no `rayon` and no parallel iterator left | the run-3 review commit |
| `F-VAL-039` | The nonce top-up threshold left about 100 sequences of headroom (High 58) | Moot | code removed by #1038 (`a9023ac`), #1036 (`82d5419`) | rv-val V4; CA3-FIX 92: W1 and W4 (300 junk `Sign` calls) behave | the run-3 review commit |
| `F2-VAL-031` | Nonce generators were cold after every restart (Medium 80) | Moot | code removed by #1039 (`5e694c4`), #1040 (`392d7c8`) | rv-val: no generator left; consult tests on the replacement | the run-3 review commit |
| `F-CORE-069` | Unmerged batex two-nonce delegation would write a permanent nonce gap (forward-looking) | Moot | batex never merged; #993, #995, #996 cancel an unused authorization nonce in the same block | rv-core-b: 2 new tests, both fail with the recovery disabled; CA3-FIX 92: 4 variants | the run-3 review commit |
| `F-CORE-037` | Snapshots are unversioned JSON (Low 62) | Settled | Aegis fresh start | rv-core-a against `settled.md` section 1 | the run-3 review commit |
| `F-VAL-033` | A database restore after a reorg reused a burned nonce | Settled | A17 (PR #905 thread, nlordell) | rv-val: trigger unchanged (an operator restore) | the run-3 review commit |
| `F2-VAL-034` | No schema-version check; a pre-#908 database opened and failed (Informational) | Settled | Aegis fresh start | rv-val against `settled.md` section 1 | the run-3 review commit |
| `F-VAL-068` | Unmerged pruning branch, parts D1 to D4 | Superseded | branch merged; D1 by `F2-VAL-031` (Moot), D2 the store concession, D3 `F-VAL-066`, D4 `F-XC-006` | rv-val | the run-3 review commit |
| `F-SEN-016` | Unmerged #914 and #915 bundle, parts D1 to D4 | Superseded | D1 by `F2-SEN-010` (Fixed), D3 by `F-SEN-006`; D2 and D4 moot in the merged design (#1014, #1015, #1017) | rv-sen | the run-3 review commit |
| `F-CORE-068` | Unmerged batex: a batch's execution status is unobservable | Superseded | whole-batch revert settled as L4; the mid-batch `CallFailed` residual folded into `F-CORE-063` | rv-core-b: queue tests show the batch marked executed by nonce; executor Foundry tests for reference | the run-3 review commit |
| `F-SEN-013` | A non-UTF-8 `reason` stalls the indexer | Refuted | run 1 (alloy decodes `string` lossily) | rv-sen: `alloy-sol-types` 1.6.0 unchanged; run 3's R1, R7 and R8 rejected it independently | the run-3 review commit |

## 5. Misses both ways

### 5.1 Earlier live rows the blind track did not file (60)

Of 80 earlier live rows, 20 have a run-3 counterpart (section 3.2 and `F2-VAL-004`). The other 60, with the run-3 log line that shows whether a reviewer examined the topic:

- rediscovered in part inside another run-3 file: 4; known items examined as known: 4; examined and not filed (rejected, kept as an observation, or examined in part): 44; never reached: 8.

| Earlier row | Ledger band | Run-3 coverage | Evidence |
| --- | --- | --- | --- |
| [`F-CORE-004`](../findings/F-CORE-004.md) | Medium | rediscovered in part | `F3-CORE-001`'s stall is this row's starvation; R1 O9 (`state/run3/agents/R1.md:102`) described `F2-CORE-005`'s orphaned-hash trigger and kept it pending client data, which PC3 then supplied (Erigon v3.7.0+) |
| [`F-VAL-004`](../findings/F-VAL-004.md) | Medium | rediscovered in part | `F3-CORE-032`'s validator consequence (setup lost after a rollback); R2 O3 (`state/run3/agents/R2.md:92`) and L-rust O10 (`state/run3/agents/L-rust.md:117`) kept the failed-setup case as an observation |
| [`F-SEN-005`](../findings/F-SEN-005.md) | Medium | rediscovered in part | `F3-SEN-002` case 2 parks the request in `WaitingForOutcome`; R7 O9 (`state/run3/agents/R7.md:120`) |
| [`F-SEN-011`](../findings/F-SEN-011.md) | Low | rediscovered in part | `F3-CORE-032` covers the in-flight check at the restore point (known #973) |
| [`F-CORE-030`](../findings/F-CORE-030.md) | Medium | known item, examined | `settled.md` section 2 lists the exit status; L-rust O9 (`state/run3/agents/L-rust.md:116`), R2 O4 (`state/run3/agents/R2.md:93`) |
| [`F-CORE-040`](../findings/F-CORE-040.md) | Low | known item, examined | `settled.md` section 2 lists cancel safety of `next_input`; L-rust Q70 (`state/run3/agents/L-rust.md:51`), R2 O1 (`state/run3/agents/R2.md:90`) |
| [`F-VAL-002`](../findings/F-VAL-002.md) | Low | known item, examined | `settled.md` section 2 lists the ECDH pad; L-crypto Q63 and O1 (`state/run3/agents/L-crypto.md:59`, `:100`) |
| [`F-SEN-009`](../findings/F-SEN-009.md) | Low | known item, examined | TODO markers (`settled.md` section 2); R7 O7 (`state/run3/agents/R7.md:118`) |
| [`F-CORE-005`](../findings/F-CORE-005.md) | Low | examined, not filed | R1 O3 (`state/run3/agents/R1.md:96`): non-default configuration |
| [`F-CORE-006`](../findings/F-CORE-006.md) | Low | examined, not filed | R6 O2 (`state/run3/agents/R6.md:83`): defense in depth; R7 R4 (`state/run3/agents/R7.md:99`) |
| [`F-CORE-009`](../findings/F-CORE-009.md) | Low | examined in part | R1 O12 (`state/run3/agents/R1.md:105`): zero timings only, as configuration (A1); cases 2 and 3 not reached |
| [`F-CORE-010`](../findings/F-CORE-010.md) | Low | examined and rejected | R1 Q6 and rejected 6 (`state/run3/agents/R1.md:57`, `:81`): no trigger, the third independent agreement |
| [`F-CORE-011`](../findings/F-CORE-011.md) | Low | examined and rejected | L-rust R-5 and O8 (`state/run3/agents/L-rust.md:92`, `:115`): outside A4; R1 O11 (`state/run3/agents/R1.md:104`) |
| [`F-CORE-012`](../findings/F-CORE-012.md) | Medium | examined, not filed | R1 O2 (`state/run3/agents/R1.md:95`) described the repeated-shape loss; C3-COV kept it because the documented provider failure is an empty array (`state/run3/coverage.md:147`) |
| [`F-CORE-032`](../findings/F-CORE-032.md) | Low | examined, not filed | R2 H20 (`state/run3/agents/R2.md:86`) and L-rust Q71 (`state/run3/agents/L-rust.md:60`): the resume is lost, no reachable panic site |
| [`F-CORE-033`](../findings/F-CORE-033.md) | Medium | examined, not filed | L-rust O2 and Q74 (`state/run3/agents/L-rust.md:109`, `:72`): A3; R7 O4 (`state/run3/agents/R7.md:115`) |
| [`F-CORE-035`](../findings/F-CORE-035.md) | Medium | examined in part | R2 H17 (`state/run3/agents/R2.md:84`) noted that RPC errors are never fatal; the swallow itself was not raised |
| [`F-CORE-036`](../findings/F-CORE-036.md) | Low | examined, not filed | L-rust Q57 (`state/run3/agents/L-rust.md:80`), R5 H5 (`state/run3/agents/R5.md:86`), R4 H7 (`state/run3/agents/R4.md:64`): the leak is refuted, the hygiene gap not filed |
| [`F-CORE-038`](../findings/F-CORE-038.md) | Informational | examined and rejected | R2 H14 (`state/run3/agents/R2.md:81`) and L-crypto Q62 (`state/run3/agents/L-crypto.md:58`): one caller, sound |
| [`F-CORE-039`](../findings/F-CORE-039.md) | Low | examined, not filed | L-rust Q15 and O8 (`state/run3/agents/L-rust.md:78`, `:115`): outside A4 |
| [`F-CORE-061`](../findings/F-CORE-061.md) | Medium | examined, not filed | R3 Q20 and O2 (`state/run3/agents/R3.md:50`, `:84`): client wording not verifiable offline |
| [`F-CORE-062`](../findings/F-CORE-062.md) | Medium | examined in part | R3 O3 (`state/run3/agents/R3.md:85`): an unincludable nonce stalls later Direct-mode nonces; the wedge not filed |
| [`F-CORE-063`](../findings/F-CORE-063.md) | Medium | examined, not filed | R3 O3 (`state/run3/agents/R3.md:85`): a marker from a sibling block is never re-checked; the batch half is settled L4 |
| [`F-CORE-064`](../findings/F-CORE-064.md) | Medium | examined, not filed | R3 Q21 and O4 (`state/run3/agents/R3.md:51`, `:86`): gas only |
| [`F-CORE-067`](../findings/F-CORE-067.md) | Medium | examined and rejected | R3 O7 (`state/run3/agents/R3.md:89`) and R2 Q16 and O9 (`state/run3/agents/R2.md:60`, `:98`): gas only |
| [`F2-CORE-003`](../findings/F2-CORE-003.md) | Informational | examined, not filed | R1 O1 and O8 (`state/run3/agents/R1.md:94`, `:101`); `F3-CORE-002` Considered and rejected |
| [`F2-CORE-011`](../findings/F2-CORE-011.md) | Low | examined and rejected | R1 O4 (`state/run3/agents/R1.md:97`) and R2 O13 (`state/run3/agents/R2.md:102`): theoretical |
| [`F2-CORE-067`](../findings/F2-CORE-067.md) | Informational | examined, not filed | R3 O8 (`state/run3/agents/R3.md:90`): operability |
| [`F-VAL-003`](../findings/F-VAL-003.md) | Medium | examined in part | R4 Q30 and INV-V6 (`state/run3/agents/R4.md:42`, `:53`) and L-crypto Q63 (`state/run3/agents/L-crypto.md:59`); the response to an unshared plaintiff, which the re-validation executed, was not raised |
| [`F-VAL-034`](../findings/F-VAL-034.md) | Low | examined and rejected | R5 Q33 (`state/run3/agents/R5.md:50`): the same benign outcome as the row |
| [`F-VAL-035`](../findings/F-VAL-035.md) | Informational | examined, not filed | R5 O7 (`state/run3/agents/R5.md:109`), L-rust Q73 (`state/run3/agents/L-rust.md:70`): A1 |
| [`F-VAL-037`](../findings/F-VAL-037.md) | Informational | examined and rejected | L-crypto Q66 and R4 (`state/run3/agents/L-crypto.md:62`, `:81`), R5 H14 (`state/run3/agents/R5.md:95`): not exploitable, consistent with the row's rating |
| [`F-VAL-060`](../findings/F-VAL-060.md) | Medium | examined, not filed | R6 O2 (`state/run3/agents/R6.md:83`): defense in depth |
| [`F-VAL-061`](../findings/F-VAL-061.md) | Medium | examined, not filed | R2 O3 (`state/run3/agents/R2.md:92`), R5 O2 (`state/run3/agents/R5.md:104`), L-rust O10 (`state/run3/agents/L-rust.md:117`) |
| [`F-VAL-062`](../findings/F-VAL-062.md) | Informational | examined, not filed | R6 rejected 2 (`state/run3/agents/R6.md:67`), R5 H5 (`state/run3/agents/R5.md:86`): redaction holds |
| [`F-VAL-064`](../findings/F-VAL-064.md) | Low | examined, not filed | R6 O3 and O8 (`state/run3/agents/R6.md:84`, `:89`), R8 O-3 (`state/run3/agents/R8.md:203`) |
| [`F-VAL-065`](../findings/F-VAL-065.md) | Low | examined and rejected | R6 Q44 and rejected 9 (`state/run3/agents/R6.md:59`, `:74`): one extra transaction per restart |
| [`F-VAL-066`](../findings/F-VAL-066.md) | Low | examined, not filed | R5 O4 (`state/run3/agents/R5.md:106`): documented trade-off; L-rust R-12 (`state/run3/agents/L-rust.md:99`) |
| [`F-SEN-004`](../findings/F-SEN-004.md) | Medium | examined, not filed | R7 O4 (`state/run3/agents/R7.md:115`), L-rust O2 (`state/run3/agents/L-rust.md:109`): A3 |
| [`F-SEN-006`](../findings/F-SEN-006.md) | Low | examined and rejected | R7 R13 and O5 (`state/run3/agents/R7.md:108`, `:116`): duplicates revert; the guardless claim is unreachable |
| [`F-SEN-007`](../findings/F-SEN-007.md) | Low | examined in part | R7 O6 (`state/run3/agents/R7.md:117`): no startup check of the oracle's proposer (A1) |
| [`F-SEN-010`](../findings/F-SEN-010.md) | Informational | examined, not filed | R8 O-4 (`state/run3/agents/R8.md:204`) |
| [`F-SEN-012`](../findings/F-SEN-012.md) | Low | examined, not filed | R7 Q53 and INV-S3 (`state/run3/agents/R7.md:69`, `:77`): every failure maps to `Unknown`, treated as intended |
| [`F-SEN-015`](../findings/F-SEN-015.md) | Medium | examined and rejected, then overturned | R7 Q52 and table row R7 (`state/run3/agents/R7.md:68`, `:102`): same `safe` boundary for both prunes; CA3-FIX's execution overturned it (section 5.3) |
| [`F-XC-001`](../findings/F-XC-001.md) | Informational | examined, not filed | R2 O12 (`state/run3/agents/R2.md:101`), L-rust (`state/run3/agents/L-rust.md:66`), R8 O-8 (`state/run3/agents/R8.md:208`) |
| [`F-XC-003`](../findings/F-XC-003.md) | Informational | examined, behaviour correct | R2 Q13 and H1 (`state/run3/agents/R2.md:57`, `:68`), R6 rejected 1 (`state/run3/agents/R6.md:66`): the same result as the row (a test gap) |
| [`F-XC-004`](../findings/F-XC-004.md) | Low | examined, not filed | R6 O8 (`state/run3/agents/R6.md:89`), R7 O11 (`state/run3/agents/R7.md:122`), R8 O-3 (`state/run3/agents/R8.md:203`) |
| [`F-XC-009`](../findings/F-XC-009.md) | Low | examined, not filed | R8 O-4 (`state/run3/agents/R8.md:204`) |
| [`F-XC-050`](../findings/F-XC-050.md) | Medium | examined and rejected | R4 Q25 and H4 (`state/run3/agents/R4.md:37`, `:61`): the contract gatekeeps membership for genuine coordinator events; the row needs `F-VAL-060`'s precondition |
| [`F-XC-051`](../findings/F-XC-051.md) | Informational | examined, not filed | L-crypto R14 (`state/run3/agents/L-crypto.md:91`) and R4 O1 (`state/run3/agents/R4.md:68`): length is contract-enforced for genuine events |
| [`F2-XC-005`](../findings/F2-XC-005.md) | Informational | examined, not filed | R2 O10 (`state/run3/agents/R2.md:99`), L-crypto O3 (`state/run3/agents/L-crypto.md:102`): A1 |
| [`F2-XC-007`](../findings/F2-XC-007.md) | Informational | examined, not filed | L-rust Q73 and O4 (`state/run3/agents/L-rust.md:70`, `:111`): A1 |
| [`F-CORE-007`](../findings/F-CORE-007.md) | Low | never reached | R1 rejected 15 (`state/run3/agents/R1.md:90`) looked at a mid-initialization reorg, not the unbounded retry loop |
| [`F-CORE-065`](../findings/F-CORE-065.md) | Low | never reached | no trace in any run-3 log |
| [`F-VAL-040`](../findings/F-VAL-040.md) | Low | never reached | no trace of the share-round `last_signer` rule |
| [`F2-VAL-007`](../findings/F2-VAL-007.md) | Low | never reached | R4 INV-V7 (`state/run3/agents/R4.md:54`) checked the normal epoch timing only |
| [`F2-VAL-064`](../findings/F2-VAL-064.md) | Informational | never reached | R2 and L-rust ran `--config-file <cfg>` with a space (`state/run3/agents/R2.md:41`, `state/run3/agents/L-rust.md:42`) |
| [`F2-SEN-011`](../findings/F2-SEN-011.md) | Informational | never reached | R7 O5 (`state/run3/agents/R7.md:116`) looked at the same handler's guard, not its doc or metric |
| [`F-XC-006`](../findings/F-XC-006.md) | Low | never reached | no trace in any run-3 log |
| [`F-XC-008`](../findings/F-XC-008.md) | Informational | never reached | R7 verified the engine client's URL handling (`state/run3/agents/R7.md:85`), not redirects or proxy variables |

### 5.2 Run-3 NEW rows whose defect was present at `fe9e84c`

| Run-3 row | Band | Present at `fe9e84c` | Earlier trace |
| --- | --- | --- | --- |
| [`F3-VAL-030`](../findings/F3-VAL-030.md) | High | yes: the approving result was acted on at once (`F3-VAL-030` Basis 12) | none in runs 1 and 2 |
| [`F3-XC-100`](../findings/F3-XC-100.md) | High | yes: same `set`, same `y` computation, same share-timeout exclusion (L-crypto) | none; the team's PR review holds it as an item from the review of #1025 |
| [`F3-VAL-100`](../findings/F3-VAL-100.md) | Low | yes: the same drop arm and the log-opened epoch-1 round (`git show fe9e84c:crates/validator/src/state/keygen.rs`, lines 263 and 499) | none; found by CA3-FIX while verifying `F-VAL-005` |
| [`F3-VAL-031`](../findings/F3-VAL-031.md) | Low | in part: the all-committed path burned the nonce on entering the share round; the timeout path (#1008) and optimistic `NewBlock` (#1015) are new | none |
| [`F3-XC-003`](../findings/F3-XC-003.md) | Informational | yes: Dockerfiles, `Cargo.toml` and the locked `sqlx` unchanged | run 2's `F2-XC-005` treats the SQLite as bundled but did not flag the comments |
| [`F3-XC-004`](../findings/F3-XC-004.md) | Informational | yes: `engine.rs` logging unchanged | run 2's `F2-SEN-008` claim 6, as a side claim |
| [`F3-XC-130`](../findings/F3-XC-130.md) | Informational | yes: the same connect path and `alloy-rpc-client` 2.0.5 | none |

Not reverse misses: `F3-SEN-060` (introduced by #974), `F3-VAL-032` (Direct Nonce store, #1032 to #1044) and `F3-XC-040` (at `fe9e84c` the whole-document echo of `F2-XC-001` covered it).

### 5.3 Run-3 rejections overturned by later execution

- `F-SEN-015`: R7 rejected the verdict-prune race (`state/run3/agents/R7.md:68`, `:102`) and LT3 at first agreed; CA3-FIX reproduced it on the real binary (section 1.5) and LT3 withdrew (`state/run3/agents/LT3.md`, section Revisited).
- `F2-XC-050`: R8 rejected the family for the steady state (`state/run3/agents/R8.md:188`); C3-XC showed it holds in warps and promoted `F3-XC-030`.

## 6. Team dispositions

The 13 team threads on PR #905, status at `361aaf3`. Nothing was posted from the audit; the draft replies of the earlier ledger are in its git history.

| # | Finding | Reviewer | Thread at `57044af` | Disposition then | Status now |
| --- | --- | --- | --- | --- | --- |
| 1 | `F-ENG-030` | @rmeissner | `report/REPORT.md:123` | scope: ignore engine changes | engine out of scope in all three runs; nothing open |
| 2 | `F-VAL-005` | @rmeissner | `report/REPORT.md:131` | fix claimed (prune refactor) | Fixed by #909 and #910 (store tests 12 of 12, CA3-FIX 88); the restart race lives on as [`F-VAL-066`](../findings/F-VAL-066.md) (Low 65); CA3-FIX's lead on the same trigger is [`F3-VAL-100`](../findings/F3-VAL-100.md) (Low); the file leaves |
| 3 | [`F-SEN-002`](../findings/F-SEN-002.md) | @rmeissner | `report/REPORT.md:134` | assigned to @rmeissner | open, Medium 90: #964 recovers the mirror case, #1014 and #1015 removed the deadline-block undercount (`F2-SEN-010` Fixed); the primary path is unchanged; run 3 found it again blind |
| 4 | [`F-SEN-015`](../findings/F-SEN-015.md) | @rmeissner | `report/REPORT.md:137` | assigned to @rmeissner | partly fixed: #978 and #979 close the live-reorg route; the restart window is not fixed (CA3-FIX 15 of 20, CA3 16 of 16); open, Medium 95 |
| 5 | [`F-VAL-004`](../findings/F-VAL-004.md) | @rmeissner | `report/REPORT.md:142` | accepted (genesis exception), became A16 | genesis instance accepted; the rollover instance stays open, Medium 69, code unchanged |
| 6 | `F-VAL-032` | @rmeissner | `report/REPORT.md:143` | accepted (accepted risk, remedcu 2026-10-08) | Fixed by #1036 and #1038 (rv-val, CA3-FIX 93); the accepted-risk entry in `settled.md` no longer describes reachable behaviour; the file leaves |
| 7 | [`F-CORE-067`](../findings/F-CORE-067.md) | @rmeissner | `report/REPORT.md:148` | question: one of them reverts? | answered yes in run 2; still valid, Medium 98; #1015 adds a replay source |
| 8 | [`F-SEN-005`](../findings/F-SEN-005.md) | @rmeissner | `report/REPORT.md:151` | assigned (easy fix) | partly fixed: #974 times out the dispute state and claims; `WaitingForOutcome` still never expires, open, Medium 95; #974 introduced [`F3-SEN-060`](../findings/F3-SEN-060.md) (Low) |
| 9 | `F-SEN-003` | @rmeissner to @nlordell | `report/REPORT.md:164` | design question: why no `NewBlock` on warp | Fixed by #1017 (CA3-FIX 92): warp pages now apply every block's transition; the bond loss without an own reveal stays with [`F2-SEN-003`](../findings/F2-SEN-003.md); the file leaves |
| 10 | `F-VAL-033` | @nlordell | `report/REPORT.md:46` | objection, proposed assumption | Settled under A17; the file leaves |
| 11 | `F-VAL-033` | @nlordell | finding file, line 210 | objection to remediation option 1 | Settled under A17; the objection stands; the file leaves |
| 12 | `F-VAL-033` | @nlordell | finding file, line 1 | databases touched only by the services | adopted as A17; Settled |
| 13 | [`F-CORE-033`](../findings/F-CORE-033.md) | @nlordell | finding file, line 19 | question: the deadline should prevent the fan-out | answered no in run 2; still valid, Medium 70; #1017 makes warp pages run deadline transitions, but effects are still spawned for every proposal in a page |

Summary: 4 threads are closed by fixes (`F-VAL-005`, `F-VAL-032`, `F-SEN-003`, and the scope thread `F-ENG-030`), 3 by A17 (`F-VAL-033`), and 6 stay open (`F-SEN-002`, `F-SEN-015` and `F-SEN-005` partly fixed, `F-VAL-004` rollover, `F-CORE-067`, `F-CORE-033`). Of the earlier Highs nobody had commented on: `F-VAL-001` (Critical), `F-VAL-030`, `F-VAL-039`, `F2-SEN-010` and `F2-XC-001` are resolved; `F-CORE-001` is Low, `F-CORE-002`, `F-CORE-031`, `F-SEN-001`, `F-VAL-060` and `F-VAL-061` are Medium; `F-CORE-060` is still High.

## 7. Unsettled items

| Item | What is missing | Positions | What would settle it |
| --- | --- | --- | --- |
| [`F-CORE-060`](../findings/F-CORE-060.md) band (High) | a practicality call on the underpriced arm | run 1 executed the per-block ratchet against a real Anvil fee market with an emulated node (High); run 2 rated its accepted arm Medium; run 3's counterpart rates the accepted arm Low; PC3 did not weigh the underpriced arm | PC3 on the underpriced arm: does any Gnosis client or gateway keep answering `replacement transaction underpriced` or `could not replace existing tx` for a reason other than the fee |
| [`F-XC-050`](../findings/F-XC-050.md) band (Medium, Plausible 48) | the band was not re-weighed after PC3 settled its precondition | the precondition (`F-VAL-060`, an allow-listed oracle emitting coordinator events) is false on Aegis, executed; PC3 moved `F-VAL-060` one band for that reason; no agent re-rated this row | the head or PC3: one band down to Low is the consistent move |
| [`F2-CORE-003`](../findings/F2-CORE-003.md) band (Informational) | a practicality look PC3 asked for | Nethermind 1.39+ answers `eth_getLogs` for a processed but not yet canonical hash with an empty success (PR #11906; PublicNode ran 1.39.3 on 2026-09-27), and the default path has no check at all, so its exposure is now larger than `F-CORE-002`'s (Medium) | PC3 on the default fetch behind load-balanced 1.39+ backends |
| [`F3-SEN-060`](../findings/F3-SEN-060.md) contract half | settled 2026-10-09 | LT3's fork test contacted the public Gnosis RPC (`state/run3/STATE.md`, 2026-10-09); the sentinel half was executed offline | OFF3 reproduced the contract half with no network under `poc/F3-SEN-060/offline/` (same numbers and gas); the head's note on the finding records the breach; the fork test was taken out of the folder |
| Trigger frequencies | live measurements no agent could make offline | Gnosis reorg depth (`F3-VAL-030`, `F3-VAL-100`, `F3-VAL-031`, `F-CORE-001`), provider limits and empty-answer windows (`F-CORE-034`, `F-CORE-002`, `F-CORE-003`), the length of the Nethermind 1.39+ window | measurements on the operators' real endpoints; these decide bands, not existence |
| GIP-153 | a re-check of every timing call | Gnosis plans 2 s blocks around 2027-01 (genesis targeted December 2026 to January 2027); windows of 6 blocks, `oracle_timeout` 24 and the 5 s reorg record all change | re-run the timing-based practicality calls when GIP-153 is scheduled |
| [`F-CORE-012`](../findings/F-CORE-012.md) practicality | a provider that returns a partial non-empty log set | run 3's R1 described the loss and C3-COV kept it as an observation, because the documented provider failure is an empty array; the re-validation executed it at Medium 85 | a provider trace showing a partial answer |
| [`F-CORE-011`](../findings/F-CORE-011.md) scope | agreement on A4 | run 3's L-rust treats a live but silent peer as outside A4 (the RPC is trusted for liveness); runs 1 and 2 and the re-validation keep it in scope at Low | the team's reading of A4 |
| [`F-VAL-003`](../findings/F-VAL-003.md) disclosure leg | an execution that shows what a coalition gains | L-crypto: a complaint response publishes only values the complaining party already holds; the re-validation executed a plaintext response to a plaintiff that never received a share; Medium 82 kept | a coalition-below-threshold PoC |
| Configuration rows | consistency across bands | rows with a run-3 counterpart moved to PC3's gate-3 reading (`F-CORE-066` Informational, `F-VAL-063` Low); configuration rows without one keep their re-validation bands (`F-CORE-005`, `F-CORE-009` Low) | the head, if one reading for operator configuration is wanted |
| Carried from the earlier ledger | the decisive trigger step | `F-CORE-061`, `F-CORE-062`, `F-CORE-063` (node wording `I`); `F-VAL-004` rollover and `F2-VAL-007` (trigger unforced); `F-CORE-010` (no trigger in three runs); `F-SEN-004` branch i (engine-dependent, A3) | live node behaviour or a forced trigger |
| Model choice revisit trigger | remedcu's reading | The team's choice of model says to revisit if the reconciliation finds run 3 missing many still-valid findings; the blind track filed 20 of 80 (section 1.3), and 8 were never reached | remedcu |

Notes that are not findings:

- `docs/validator-handbook.md:37` limits the empty-answer bug to Nethermind before 1.36; PC3's consult shows 1.39+ returns an empty success again for a processed but not yet canonical hash.
- `settled.md` section 1 still lists the group-mismatch `Sign` drop as an accepted risk; #1036 and #1038 fixed it (`F-VAL-032`, CA3-FIX 93).
- The stale warp comment at `crates/sentinel/src/service.rs:481-483` (section 1.6).
- Several Prettier failures in finding files predate this ledger (for example the QA second-reproduction sections written as unbroken lines); they need the formatting pass.

Process note: contract changes after the audited commit `0be96367` are unaudited. Several resolutions here rest on them: `F-VAL-001` (the onchain proof of possession, #1022 and #1027), `F-VAL-067` (#1019), and the Batched-mode workaround for `F-SEN-008`, which depends on the post-audit 7702 executor. This is a process note, not a defect.
