# Reconciliation — run 1 and run 2 of the Safenet Rust services audit

| Field | Value |
| --- | --- |
| Audited commit (combined) | `fe9e84cc59b65367b31d5a3121774383cc422234` — `origin/main` `8b6a75d` merged into `audit/rust-services`; run 2's Phases 0–3 ran at `3ec8bc5`, whose `crates/core` and `crates/validator` are byte-identical to `fe9e84c`; `crates/sentinel` and the reference contracts were re-validated at `fe9e84c` (R7Δ, C2-SEN-Δ, QA2-SEN-Δ) |
| Run 1 | audited `2893917` (post-merge re-validation at `a7f3915`); 85 surviving finding files `F-CORE-*` (33), `F-VAL-*` (25), `F-SEN-*` (16), `F-XC-*` (11); engine findings removed with the engine's scope |
| Run 2 | a different model, no access to run-1 material; 69 finding files `F2-CORE-*` (26), `F2-VAL-*` (22), `F2-SEN-*` (11), `F2-XC-*` (10); Gate 3 tally: Critical 1, High 8, Medium 16, Low 32, Informational 12; 47 PoC directories |
| Inputs to this document | the four per-crate reconciliation parts — [`core`](../state/run2/reconciliation/core.md), [`validator`](../state/run2/reconciliation/validator.md), [`sentinel`](../state/run2/reconciliation/sentinel.md), [`cross-cutting`](../state/run2/reconciliation/cross-cutting.md) — plus [`pr-review-threads.md`](../state/pr-review-threads.md), [`KNOWN-WORK.md`](KNOWN-WORK.md), [`IN-FLIGHT.md`](IN-FLIGHT.md), [`STATE.md`](../state/run2/STATE.md), [`coverage.md`](../state/run2/coverage.md) §6–§8, [`baseline-delta.md`](../state/run2/baseline-delta.md) §2 |
| What this document is | the single ledger every number in `REPORT.md` is built from: one canonical ID per defect, its final severity, certainty, basis class, status and team disposition; every `path:line` is at `fe9e84c` unless marked |
| Rules applied | canonical ID is the run-1 ID where both runs have the defect (the team's links keep working), else the run-2 ID; the run with executed evidence carries the number; where the runs disagree only on the severity band, the reconciliation states both positions and the reason for the carried one; every `## Reconciliation (run 2)` section in the 154 finding files states that file's final combined status |

## 1. Method and headline

Two independent runs, different models, same prompt, engine out of scope in both (run 1's engine artefacts were removed). Run 2 could not read run 1. After run 2's QA, four reconciliation agents (one per crate) mapped every run-2 finding to run 1, listed what run 2 did not rediscover, applied A16/A17 to run 1's surviving findings, answered the team's open questions and wrote per-crate ledger rows; this document merges the four parts and settles the conflicts between them.

### 1.1 Agreement rate

| Crate | Run-2 files | CONFIRMS | EXTENDS | CONTRADICTS | NEW | Folded within run 2 | Run-1 live findings | Not re-filed by run 2 | Run-2 canonical items | of which reachable at `2893917` (run-1 misses) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| core | 26 | 13 | 8 | 0 | 4 | 1 (`F2-CORE-036` → `F2-XC-050`) | 31 | 10 (+1 partial) | 4 | 4 |
| validator | 22 | 8 | 7 | 0 | 7 | 6 (2 in-crate duplicates, 4 cross-crate canonicals) | 22 | 9 (+1 partial) | 6 | 4 |
| sentinel | 11 | 7 | 3 | 0 | 1 | 1 (`F2-SEN-009` → cross-cutting) | 14 | 2 (+3 partial) | 3 | 1 |
| cross-cutting | 10 | 3 | 2 | 0 | 5 | 0 | 10 | 4 | 5 | 5 |
| **total** | **69** | **31** | **20** | **0** | **17** | 8 | **77** | **25** (+5 partial) | **18** | **14** |

- **Existence and mechanism:** no run-2 finding contradicts a run-1 finding as its primary relation. Three sub-claim contradictions were found and settled by executed evidence (Section 1.3). Every run-1 finding that run 2 did not re-file is still valid at `fe9e84c` except where the merge fixed it (`F-VAL-005`), A17 removed it (`F-VAL-033`), or a five-minute source read settled it lower (`F-XC-051`, `F-VAL-067`).
- **Rediscovery:** run 2 rediscovered 52 of run 1's 77 live findings (68 %), fully or in part. Of the 25 it did not re-file, 6 were examined by a run-2 reviewer and deliberately not filed (`F-CORE-008`, `F-CORE-010`, `F-CORE-036`, `F-CORE-037`, `F-CORE-038`, `F-XC-003`) and 19 are misses proper (Section 3). In the other direction, run 2 added 18 canonical findings of which 14 were reachable at `2893917` and are run-1 misses; the other 4 exist because of code run 1 did not have (`F2-VAL-031`, `F2-VAL-034`, `F2-SEN-010`, `F2-SEN-011`).
- **Severity bands, before reconciliation:** over the 48 mapped pairs where run 1 had a rated counterpart, the two runs' bands agreed on 23, run 2 was one band lower on 23 (mostly core, where the run-2 Critics read A1/A4 and "bounded cost" strictly) and higher on 2 (`F2-CORE-030`, `F2-VAL-061`). The sentinel agreed on every pair. After reconciliation the carried band follows run 1 in most disagreements (run 1 holds the live-stack executed evidence); run 2's number is carried where its evidence was decisive: `F-CORE-031` up to High, `F-VAL-060` up to High (conditional), `F-CORE-011` down to Low, `F-VAL-064` split and down to Low, `F-XC-008`/`F-XC-011` down to Informational (engine out of scope), `F-XC-051` down to Informational (pinned-source read), `F-VAL-067` down to Low.
- **Certainty:** run 2's execution raised eleven run-1 numbers (`F-CORE-004` 75 → 90, `F-CORE-006` 55 → 70, `F-CORE-007` 60 → 70, `F-CORE-011` 60 → 80, `F-CORE-039` 55 → 72, `F-CORE-061` 58 → 65, `F-CORE-064` 72 → 90, `F-CORE-066` 78 → 90, `F-VAL-040` 50 → 88, `F-VAL-060` 50 → 90, `F-XC-006` 78 → 90) and lifted four Plausible run-1 findings to Confirmed (`F-CORE-006`, `F-CORE-007`, `F-CORE-011`, `F-SEN-004` branch ii).

### 1.2 How conflicts were settled

1. **Executed evidence wins.** Where one run executed the mechanism and the other traced it, the executed number is carried. Where both executed, the wider execution (live binaries against a real oracle in run 1's Phase 8; `StateMachine`/`Watcher`-level PoCs in run 2) carries the certainty, and the other is recorded as the independent confirmation.
2. **Severity is settled against PROMPT §8's scale, not by averaging.** Each disagreement is written out in the per-crate part (core §1.1, validator §1 rows, sentinel §1, cross-cutting §1) with both positions; the ledger's trail column names the dissenting number so the report can show it.
3. **Where nothing settles it, both positions stand.** That happened nowhere on existence or mechanism; the only open items are the ones the audit could not execute (Section 7).

### 1.3 The three sub-claim contradictions, settled

| Where | Run 1 said | Run 2 said | Settled |
| --- | --- | --- | --- |
| `F-CORE-031` / `F2-CORE-030` | Critic note: `F-SEN-001`'s replay case is "the opposite … mutually exclusive" with the resume-loss defect | when the emitting block _is_ replayed, the replayed own `Committed` is applied synchronously before the re-spawned effect can resume, so the replayed case loses too | **run 2**, by execution (`poc/F2-CORE-030`, "replayed block 3 committed … resumes: []"); this is what lifts `F-CORE-031` to High 92 |
| `F-CORE-060` / `F2-CORE-060` | "the cap bypassed ~28,700×" | the cap ratio is preserved through bumps (algebra verified) | **both right, about different quantities**: `cap_priority_fee` (`crates/core/src/tx/fees.rs:12-31`) bounds tip / own `maxFeePerGas` and that ratio survives bumping; it never bounds the tip against the chain's base fee once the ratchet runs (measured 28,744× the base-fee-derived cap). Both bump cadences are real (underpriced arm every block, accepted arm every `blocks_before_resubmit`). `cargo test -p safenet-core --lib -- tx::fees tx::tests::failed_replacements`: 4 passed. Wording for the report is in core §1.2 |
| `F-SEN-015` / `F2-SEN-001` (c) | executed twice (in-process and on Anvil: `AlreadyCommitted` then `InvalidReveal`, −4,000) | "at most `I` (engine out of scope)" | **run 1**: variant 2 (`Unknown` on replay) needs no engine determinism, only a co-deployed engine still booting when the replayed effect fires (A3); E1 at the state-machine level; code unchanged at `fe9e84c` (`service.rs:156-171`, `176-179`, `214-224`) |

## 2. Combined ledger

One row per canonical defect. **Runs**: `1` run 1 only, `2` run 2 only, `1+2` both. **Basis**: strongest class across both runs (`E1` executed, `E2` code-traced, `I` inference). **Status**: Confirmed (certainty ≥ 70), Plausible (40–69), Observation (< 40), Refuted, Fixed, Out of scope, Superseded, Forward-looking; "Verified" marks a run-1 finding whose headline leg was refuted by execution and whose residual stands. Links go to the canonical finding file; every run-2 file named in the counterpart column is itself listed in the fold index (Section 2.2).

### 2.1 Canonical rows

| Canonical | Counterpart(s) | Runs | Short title | Severity | Cert. | Basis | Status | Team disposition | Trail |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| [`F-CORE-001`](../findings/F-CORE-001.md) | `F2-CORE-001` | 1+2 | Reorg-depth protection not persisted; restart resumes from orphaned state | High | 99 | E1 | Confirmed | #820 closed-but-present; no thread | run 1 E1 live 99 High; run 2 E1 90 Medium (dissent recorded); A17 narrows only the restored-backup aside |
| [`F-CORE-002`](../findings/F-CORE-002.md) | `F2-CORE-002` | 1+2 | Client-filtering completeness check disables itself after three failures | High | 99 | E1 | Confirmed | new; no thread | run 1 E1 live 99 (−4,500 on chain) High; run 2 E1 90 Medium (dissent recorded) |
| [`F-CORE-003`](../findings/F-CORE-003.md) | `F2-CORE-006` | 1+2 | `null` header treated as an uncle | Medium | 70 | E2 | Confirmed | — | run 1 E2 70 Medium; run 2 E2 70 Low; Medium carried (routine A4 trigger, on-chain duplicates) |
| [`F-CORE-004`](../findings/F-CORE-004.md) | `F2-CORE-005` | 1+2 | Event-fetch failures retry forever; block watcher starved, reorg detection stops | Medium | 90 | E1 | Confirmed | #820 class | run 1 E2 75; run 2 executes the starvation 90 |
| [`F-CORE-005`](../findings/F-CORE-005.md) | `F2-CORE-008` | 1+2 | `max_reorg_depth = 0`: `recent` always empty, `-32001` path spins silently | Low | 75 | E2 | Confirmed | #820 closed-but-present | both E2 75 |
| [`F-CORE-006`](../findings/F-CORE-006.md) | `F2-CORE-004` (service side: `F-VAL-060`) | 1+2 | Filtering and decoding are address-agnostic (core half) | Low | 70 | E2 | Confirmed | — | run 1 Plausible 55; run 2 70 with the alloy `__Invalid` precision |
| [`F-CORE-007`](../findings/F-CORE-007.md) | `F2-CORE-009` | 1+2 | Initialization range scan restarts without bound or delay | Low | 70 | E2 | Confirmed | — | run 1 Plausible 60; run 2 70 |
| [`F-CORE-008`](../findings/F-CORE-008.md) | — | 1 | Block polling scheduled against the host wall clock | Low | 70 | E2 | Confirmed | — | run 1 E2 70; run 2 examined (R1 O3), not filed |
| [`F-CORE-009`](../findings/F-CORE-009.md) | — | 1 | Block-watcher config unvalidated (`block_time = 0`, unbounded depth, `start_block > head`) | Low | 78 | E2 | Confirmed | — | run 1 78; run 2 covers case 1 only (R1 O4) — miss for cases 2–3 |
| [`F-CORE-010`](../findings/F-CORE-010.md) | — | 1 | `-32001` recovery commits the rewind before the event watcher validates it | Low | 45 | E2 | Plausible | — | run 1 45 (no trigger); run 2 R1 rejected 2 independently finds no trigger |
| [`F-CORE-011`](../findings/F-CORE-011.md) | `F2-CORE-033` | 1+2 | Indexer hangs behind a live-but-silent peer; nothing signals it | Low | 80 | E2 | Confirmed (narrowed) | — | run 1 Plausible 60 Medium; run 2 settles the reqwest premise (`tcp_user_timeout` 30 s on Linux) → Low 80 |
| [`F-CORE-012`](../findings/F-CORE-012.md) | — | 1 | Bloom equality blind to the loss of a repeated-shape log; `check_logs_limit` skipped on the client-filtered path | Medium | 70 | E2 | Confirmed | — | run 1 70; **run 2 missed** (the most significant core miss) |
| [`F-CORE-030`](../findings/F-CORE-030.md) | `F2-CORE-031`, `F2-VAL-066`, exit-status half of `F-VAL-064` | 1+2 | Fatal driver errors exit with status 0; `/health` is a constant `OK` | Medium | 85 | E2 | Confirmed | #820 closed-but-present | run 1 85 Medium; run 2 85 Low (its Critic offered Medium); Medium carried |
| [`F-CORE-031`](../findings/F-CORE-031.md) | `F2-CORE-030` (sentinel loss: `F-SEN-001`; in-flight case: `F-SEN-011`) | 1+2 | Snapshot commits before the effect spawns; rollback and replay discard resumes | High | 92 | E1 | Confirmed | #614/#799 partial | run 1 E2 78 Medium; run 2 E1 92 High (ordering variant executed; deterministic across a ≥ 25 s restart); **raised** |
| [`F-CORE-032`](../findings/F-CORE-032.md) | — | 1 | A panicking effect task is logged and skipped; the resume is gone | Low | 45 | E2 | Plausible | #799/#614 partial | run 1 45; **run 2 missed** |
| [`F-CORE-033`](../findings/F-CORE-033.md) | `F2-CORE-035` (sentinel side: `F-SEN-004`) | 1+2 | Effect fan-out unbounded; no cap, queue or backpressure | Medium | 70 | E2 | Confirmed | **question answered** (§5.2); #614 partial | run 1 70 Medium; run 2 70 Low, adds "no `NewBlock` in a warp page" |
| [`F-CORE-034`](../findings/F-CORE-034.md) | `F2-CORE-010` | 1+2 | Retry storm without backoff; deterministic errors never escalate | Medium | 80 | E2 | Confirmed | — | run 1 80; run 2 quantifies ×17 validator / ×11 sentinel per 100 ms |
| [`F-CORE-035`](../findings/F-CORE-035.md) | (`F2-CORE-067` metrics leg) | 1 | Every `tx::Error::Rpc` is "intermittent" and swallowed forever | Medium | 78 | E2 | Confirmed | — | run 1 78; **run 2 missed** the swallow |
| [`F-CORE-036`](../findings/F-CORE-036.md) | (`F-XC-002` union) | 1 | `Debug` bound on `Effect`/`Resume`; five trace sinks | Low | 85 | E1 (secret leg refuted) | Verified | #113 understated | run 1 Phase 5 refuted the secret leg; run 2 R2 rejected 1 / O7 agree, not filed |
| [`F-CORE-037`](../findings/F-CORE-037.md) | (`F2-CORE-066` version half) | 1 | Snapshots are unversioned JSON | Low | 62 | E2 | Plausible | — | run 1 62; run 2 R2 O10 examined, not filed |
| [`F-CORE-038`](../findings/F-CORE-038.md) | — | 1 | `kdf::derive_key` multi-part `info` is plain concatenation | Informational | 85 | E2 | Confirmed | — | run 1 85; run 2 R2 O9 adds HMAC salt-normalisation nuance |
| [`F-CORE-039`](../findings/F-CORE-039.md) | `F2-CORE-034` | 1+2 | Graceful shutdown blocked inside `update`; inline `housekeeping` is a second unbounded await | Low | 72 | E2 | Confirmed | — | run 1 55; run 2 72, closes run 1's dependency question |
| [`F-CORE-040`](../findings/F-CORE-040.md) | — | 1 | `next_input` `select!` drops the watcher's in-flight RPC on every resume | Low | 65 | E2 | Plausible | #614 partial | run 1 65; run 2 examined cancel-safety only — cost **missed** |
| [`F-CORE-060`](../findings/F-CORE-060.md) | `F2-CORE-060` | 1+2 | Fee ratchet unbounded; `priority_fee_cap_percentage` never bounds against the base fee | High | 98 | E1 | Confirmed | #656 closed-but-present | run 1 E1 live 98 High; run 2 E1 92 Medium (accepted arm, ×311.9 after 60 bumps); cap sentence settled both ways (§1.3); A17 removes the restored-backup starter |
| [`F-CORE-061`](../findings/F-CORE-061.md) | `F2-CORE-062` | 1+2 | First-submission "underpriced" rejection retried forever at the same fee | Medium | 65 | E1 (in-crate) | Plausible | #656 (introduced by the fix) | run 1 58; run 2 65 (zero-tip arithmetic); node wording still `I` |
| [`F-CORE-062`](../findings/F-CORE-062.md) | (`F2-CORE-061` gap, `F2-CORE-064` no-cancellation) | 1 | Allocated nonce never released; one bad nonce wedges the queue | Medium | 60 | E2 (gap E1) | Plausible | — | run 1 60; run 2 rediscovers both halves, not as a standalone |
| [`F-CORE-063`](../findings/F-CORE-063.md) | `F2-CORE-061` | 1+2 | Execution inferred from the account nonce; mark irrevocable | Medium | 55 | E1 (irrevocability) | Plausible | — | run 1 55; run 2 executes the mark and narrows the trigger (gap closes unless an action is queued inside retention) |
| [`F-CORE-064`](../findings/F-CORE-064.md) | `F2-CORE-064` | 1+2 | `expires_at` void once a nonce is allocated | Medium | 90 | E1 | Confirmed | — | run 1 72 Medium; run 2 90 Low (never-accepted case executed; "clear the nonce" fix branch unsafe); Medium carried |
| [`F-CORE-065`](../findings/F-CORE-065.md) | `F2-CORE-066` | 1+2 | Transactions table bound to neither chain id nor signer | Low | 70 | E2 | Confirmed, `known` | `known` (A12) | run 1 55; run 2 70; **A17: trigger 1 (reused/restored database) out of scope** |
| [`F-CORE-066`](../findings/F-CORE-066.md) | `F2-CORE-065` | 1+2 | `tx::Config` accepts degenerate values (zero in-flight, zero window, `nan`) | Medium | 90 | E1 | Confirmed | — | run 1 78 Medium; run 2 90 Low (all three values executed); Medium carried |
| [`F-CORE-067`](../findings/F-CORE-067.md) | `F2-CORE-063`, `F2-CORE-032` (service sides: `F-VAL-065`, `F-SEN-006`) | 1+2 | No idempotency key on `enqueue`; replay re-submits actions | Medium | 98 | E1 | Confirmed | **question answered** (§5.2); `known` tag retained | run 1 E1 live 98 Medium; run 2 E1 90 Low; Medium carried (replayed `Sign` and the `F-SEN-001` path are not gas-only) |
| [`F-CORE-068`](../findings/F-CORE-068.md) | — | 1 | UNMERGED batex: a batch's execution status is unobservable | High / Medium (forward-looking) | 85 | static | Forward-looking | re-validate when #904 merges | not in the tree at `fe9e84c` |
| [`F-CORE-069`](../findings/F-CORE-069.md) | — | 1 | UNMERGED batex: two-nonce delegation writes a permanent nonce gap | High / Medium (forward-looking) | 80 | static | Forward-looking | re-validate when #904 merges | not in the tree at `fe9e84c` |
| [`F2-CORE-003`](../findings/F2-CORE-003.md) | — | 2 | Default-path log fetch has no completeness check; dead `may_contain_log` | Informational | 70 | E2 | Confirmed | — | run 2 only; run-1 miss |
| [`F2-CORE-007`](../findings/F2-CORE-007.md) | — | 2 | Restart against a lagging node exits with an opaque `BadUpdate` | Low | 75 | E2 | Confirmed | — | run 2 only; run-1 miss |
| [`F2-CORE-011`](../findings/F2-CORE-011.md) | — | 2 | No rollback anchor persisted on a fresh start; in-window reorg exits `MissingSnapshot` | Low | 92 | E1 | Confirmed | — | run 2 only; run-1 miss |
| [`F2-CORE-067`](../findings/F2-CORE-067.md) | (`F-CORE-035` observability leg) | 2 | Transaction queue exports no metrics; hash never persisted | Informational | 85 | E2 | Confirmed | — | run 2 only |
| [`F-VAL-001`](../findings/F-VAL-001.md) | `F2-VAL-001` | 1+2 | DKG encryption key `q` has no proof of possession; peer's signing share recoverable | Critical | 97 | E1 | Confirmed | none yet | run 1 E1 97 (crypto + real bytecode 5/5 + post-merge); run 2 independent E1 95 (`s_a_recovered == s_a_ref`) |
| [`F-VAL-002`](../findings/F-VAL-002.md) | `F2-VAL-002` | 1+2 | ECDH share pad is a raw symmetric x-coordinate | Medium | 93 | E1 | Confirmed | none | run 1 E1 93 Medium; run 2 E1 90 Low (standalone); Medium kept for the executed second-share leak |
| [`F-VAL-003`](../findings/F-VAL-003.md) | `F2-VAL-006` | 1+2 | Complaint responses unconditional and unbounded per plaintiff | Medium | 82 | E1 | Confirmed | #69 partial | run 1 E2 80 Medium; run 2 E1 82 Low; Medium kept (on-demand disclosure primitive plus honest-side exclusion race) |
| [`F-VAL-004`](../findings/F-VAL-004.md) | `F2-VAL-063`, `F2-VAL-005` | 1+2 | Lost or failed `KeyGenSetup` never re-issued (genesis: permanent halt; rollover: epoch forfeited) | Medium (rollover); genesis Informational `known` | 69 (rollover) / 93 (genesis) | E1 gap / E2 trigger | Plausible (rollover); genesis accepted | **accepted (genesis exception, A16)**; rollover instance is the "other flow" the team asked about | run 1 E1 93 High (genesis live); A16 → Informational `known`; run 2 E2 69 Medium for numbered epochs |
| [`F-VAL-005`](../findings/F-VAL-005.md) | `F2-VAL-035`, `F2-VAL-031`, `F-VAL-068` D2 | 1+2 | Reorg across the `KeyGen` block deleted the DKG secrets | — (High at `2893917`) | 99 at `2893917`; fix E1 | E1 | **Fixed by merge** (#909/#910) for the reproduced trigger; residual D2 Informational `known` | **fix claimed — confirmed** (§5.2) | 13/13 store tests at `fe9e84c`; the restart-window race survives under `F-VAL-066` |
| [`F-VAL-030`](../findings/F-VAL-030.md) | `F2-VAL-030`, `F2-VAL-062` | 1+2 | Failed `NonceTree` leaves a phantom reservation; `expected_chunk` cascade | High | 97 | E1 | Confirmed | #799/#666 partial | run 1 E1 97 live, unforced after a reorg; run 2 E1 90 plus the cascade (~90 % of later ceremonies); C2-VAL-B's Medium overruled by the reorg trigger |
| [`F-VAL-031`](../findings/F-VAL-031.md) | — | 1 | Dead nonce-generation worker never detected or restarted | Low | 42 | E2 | Plausible | none | run 1 42; **run 2 missed** |
| [`F-VAL-032`](../findings/F-VAL-032.md) | `F2-VAL-032` | 1+2 | `handle_sign` drops a `WaitingForRequest` session on a `Sign` from any untracked group | High | 93 | E1 | Confirmed | **accepted** | run 1 E1 93; run 2 E1 92 with an unconditional attacker trigger (attacker-created 2-of-2 group) |
| [`F-VAL-033`](../findings/F-VAL-033.md) | — | 1 | Database restore after a reorg reuses a burned nonce | (High measured) | 72 | E1 mechanism | **Out of scope (A17)** | objection → A17; remediation-1 objection upheld | mechanism real (`store.rs:281-291`); trigger is an operator restore |
| [`F-VAL-034`](../findings/F-VAL-034.md) | — | 1 | `handle_nonces` applies a resume without checking the signature id | Low | 55 | E1 (benign) | Plausible (outcome executed benign) | none | run 1 executed; **run 2 missed** |
| [`F-VAL-035`](../findings/F-VAL-035.md) | (`F2-XC-005` adjacent) | 1 | Unzeroised nonce JSON; abandoned chunks of retained groups never reclaimed | Low | 35 | E2 / E1 (leg c refuted) | Observation | #666 partial | run 1 35; run 2 did not re-raise (a)/(b) |
| [`F-VAL-036`](../findings/F-VAL-036.md) | (`F2-VAL-061` scenario b, forward only) | 1 | `observe` accepts a non-monotonic sequence and rewinds | Low | 40 | E2 | Plausible | none | run 1 40; **run 2 missed** |
| [`F-VAL-037`](../findings/F-VAL-037.md) | — | 1 | Merkle trees pad with `B256::ZERO`; no leaf/internal separation | Informational | 60 | E2 | Plausible | none | run 1 60; **run 2 missed** |
| [`F-VAL-038`](../findings/F-VAL-038.md) | — | 1 | Chunk generation saturates every core; 1025 statements hold the SQLite writer | Low | 55 | E2 | Plausible | flow-test epic partial | run 1 55; **run 2 missed**; pruning adds an inline writer |
| [`F-VAL-039`](../findings/F-VAL-039.md) | (`F2-VAL-032` names the endpoint) | 1 | Top-up threshold gives ~100 sequences of headroom against a permissionless counter | High | 58 | E2 | Plausible | new | run 1 Critic-drafted 58; **run 2 missed**; cost model unexecuted (§7) |
| [`F-VAL-040`](../findings/F-VAL-040.md) | `F2-VAL-033` | 1+2 | A signer re-reveals to become `last_signer`, then does nothing | Low | 88 | E1 | Confirmed | #777 partial | run 1 E2 50; run 2 E1 88 |
| [`F-VAL-060`](../findings/F-VAL-060.md) | `F2-VAL-061` (core half: `F-CORE-006`) | 1+2 | Coordinator/Consensus events accepted from any watched address | High (conditional on a malicious or compromised allow-listed oracle) | 90 | E1 | Confirmed | none | run 1 E2 50 Medium (precondition); run 2 E1 90 High, three scenarios; **raised** |
| [`F-VAL-061`](../findings/F-VAL-061.md) | `F2-VAL-030/031/062/063` (policy leg) | 1+2 | Every effect error maps to `Resume::Noop`; no retry, no marker | High | 98 | E1 | Confirmed | #799 partial | run 1 E1 98 live; run 2 confirms the policy in four files and widens the window (`F2-VAL-031`) |
| [`F-VAL-062`](../findings/F-VAL-062.md) | (`F-XC-002` union) | 1 | Secret-bearing effects and resumes derive `Debug`, printed at `warn` | Informational | 88 | E1 (leak refuted) / E2 | Confirmed (hygiene) | #113 understated | run 1 88; **run 2 missed** the hygiene gap |
| [`F-VAL-063`](../findings/F-VAL-063.md) | `F2-VAL-067` (timing), `F2-XC-008` item 3, `F2-XC-006` | 1+2 | Consensus-critical config unvalidated; rollover livelock when `blocks_per_epoch <= key_gen_timeout` | Medium | 72 | E2 | Confirmed; points (1), (3) run-2 miss | none | run 1 72; run 2 75 on the timing relation, folded in |
| [`F-VAL-064`](../findings/F-VAL-064.md) | `F2-VAL-066` (→ `F-CORE-030`), `F2-VAL-068` (→ `F-XC-004`) | 1+2 | Validator deployment: exit 0, `/health` unreachable in the shipped deployment, root container | Low | 72 | E2 | Confirmed (split; residual row) | #820 partial | run 1 68 Medium composite; exit-status half counted under `F-CORE-030`, root half under `F-XC-004`; the residual keeps this row at Low (merge note 1) |
| [`F-VAL-065`](../findings/F-VAL-065.md) | (core: `F-CORE-067`) | 1 | `SetValidatorStaker` accumulates across restarts; `Preprocess` never expires; a replayed `Sign` burns a sequence for the whole group | Low | 70 | E2 | Confirmed | none | run 1 70; **run 2 missed** the validator legs |
| [`F-VAL-066`](../findings/F-VAL-066.md) | `F2-VAL-035`, `F-VAL-068` D3 | 1+2 | Retention set computed before the block's logs; reconcile-vs-prune race across a restart | Medium | 78 | E1 | Confirmed (changed shape) | #666/#801 partial | run 1 E1 92 (unqualified `DELETE`, removed by #909); run 2 E1 78 (19/20 rounds at the driver seam) |
| [`F-VAL-067`](../findings/F-VAL-067.md) | `F2-VAL-006` para. 2 | 1+2 | Rust counts complaints cumulatively, the contract nets them | Low | 70 | E2 | Confirmed (impact bounded) | #118/#69 partial | run 1 Critic 48 Medium; run 2 "consistency issue"; `restart_key_gen_excluding` check settles Low 70 |
| [`F-VAL-068`](../findings/F-VAL-068.md) | `F2-VAL-031` (D1), `F-VAL-005` residual (D2), `F2-VAL-035` (D3), `F2-XC-006` (D4) | 1 | UNMERGED pruning branch D1–D4 | — | — | — | **Superseded** (branch merged; split) | none | every part now has a critiqued home except D4's specific form |
| [`F2-VAL-003`](../findings/F2-VAL-003.md) | — | 2 | Replay warp over an epoch boundary drops the key-gen round; epoch forfeited | Medium | 85 | E1 / E2 | Confirmed | none | run 2 QA E1 85; run-1 miss |
| [`F2-VAL-004`](../findings/F2-VAL-004.md) | — | 2 | Late-setup branch derives a divergent share-round deadline | Low (genesis case Informational `known`) | 50 | E2 | Plausible | none | run 2 50, QA not attempted; run-1 miss |
| [`F2-VAL-007`](../findings/F2-VAL-007.md) | — | 2 | `active_epoch` only advances through self-staged epochs; phantom rollover session | Low | 65 | E2 | Plausible | none | run 2 65, QA not attempted; run-1 miss |
| [`F2-VAL-031`](../findings/F2-VAL-031.md) | `F-VAL-068` D1, `F-VAL-061` | 2 (predicted by run 1 on the branch) | Nonce generators cold after every restart; below-marker return precedes `generator.start` | Medium | 80 | E1 (partial) | Confirmed | none | new code (#909/#910); run 1 predicted it as D1 |
| [`F2-VAL-034`](../findings/F2-VAL-034.md) | — | 2 | No schema-version check; a pre-#908 database opens then fails every reconciliation | Informational `known` | 85 | E1 | Confirmed | none | new code since run 1 |
| [`F2-VAL-064`](../findings/F2-VAL-064.md) | — | 2 | Documented `--config-file=<path>` spelling rejected by `argh` (both binaries) | Informational | 95 | E1 | Confirmed | none | run-1 miss |
| [`F-SEN-001`](../findings/F-SEN-001.md) | `F2-SEN-001` (a, b) (core root cause: `F-CORE-031`) | 1+2 | Replay or rollback discards own `Committed`; no reveal; bond slashed | High | 99 | E1 | Confirmed | **none yet** — highest-certainty irreversible loss in the crate, unassigned; #614 closed-but-present | run 1 E1 Anvil 99; run 2 independent E1 92, re-run at `fe9e84c` |
| [`F-SEN-002`](../findings/F-SEN-002.md) | `F2-SEN-002` (sibling: `F2-SEN-010`) | 1+2 | Commits before the verdict not tallied; early finalize; bond never claimed or `Finalize` reverts and parks | High | 98 | E1 | Confirmed | **assigned @rmeissner** — must also close `F2-SEN-010` | run 1 E1 98; run 2 E1 90 at `fe9e84c` (adds the adversarial stall) |
| [`F-SEN-003`](../findings/F-SEN-003.md) | `F2-SEN-003` (b), `F2-SEN-011` trigger | 1+2 | Warp delivers no `NewBlock`; reveals discarded; bogus `timed_out` finalize | Low | 80 | E1 | Confirmed (residual; funds impact fixed by `199629e`) | **question answered** (§5.2); #667 partial | run 1 80 (Medium → Low post-merge); run 2 executed the route, did not re-file |
| [`F-SEN-004`](../findings/F-SEN-004.md) | `F2-SEN-005` | 1+2 | No bound on concurrent engine checks or outstanding bonds; flood → abstention or expired reveals | Medium | 78 | E1 | Confirmed (branch ii) | #614 partial | run 1 Plausible 62; run 2 E1 78 (starvation onset ≈ 8 proposals/block at `3ec8bc5`; narrowed, not fixed, at `fe9e84c`) |
| [`F-SEN-005`](../findings/F-SEN-005.md) | `F2-SEN-004` | 1+2 | Waiting states never expire; arbitration deadline discarded; no `timeoutArbitration` | Medium | 95 | E1 | Confirmed | **assigned @rmeissner (easy fix)** — do not expire-and-drop | run 1 95; run 2 E1 90 at `fe9e84c` |
| [`F-SEN-006`](../findings/F-SEN-006.md) | `F2-SEN-007` (+ bondless `Claim`), `F-SEN-014` | 1+2 | Actions not idempotent under replay; `handle_arbitration_timeout` claims without a bond | Low | 90 | E1 | Confirmed | — | run 1 E2 85; run 2 E1 90, option 3 compiled |
| [`F-SEN-007`](../findings/F-SEN-007.md) | `F2-SEN-006` | 1+2 | No startup or per-request pre-flight; silent per-request gas burn | Low | 82 | E1 (partial) | Confirmed | — | run 1 E2 80; run 2 82 (`voting_window` sub-claim executed) |
| [`F-SEN-008`](../findings/F-SEN-008.md) | — | 1 | Hard-coded gas limits; unconditional non-zero `approve`; non-plain fee token | Low | 52 | E2 | Plausible | — | run 1 52; **run 2 missed**; the contract now documents the fee-token requirements (`SentinelOracle.sol:62-68`) |
| [`F-SEN-009`](../findings/F-SEN-009.md) | `F2-SEN-006` claim 6, `F2-SEN-008` claim 3, `F2-SEN-009` b3 | 1+2 | Engine timeout derived from unvalidated `voting_window` | Low | 82 | E1 | Confirmed, `known` | `main.rs` TODO + #799 | run 1 82; run 2 executes `voting_window = 0` acceptance and the 1 s floor |
| [`F-SEN-010`](../findings/F-SEN-010.md) | `F2-SEN-009` b1 (via `F-XC-009`) | 1+2 | Sample ships zero addresses and a placeholder key that parse and start | Informational | 85 | E2 | Confirmed, `known` | `config.rs` TODO | both 85 |
| [`F-SEN-011`](../findings/F-SEN-011.md) | (`F2-CORE-030` in-flight sub-case) | 1 | Restart orphans an in-flight check whose proposal predates the anchor | Low | 82 | E2 | Confirmed | #614 partial | run 1 82; run 2 carried the mechanism generically, not as a sentinel finding |
| [`F-SEN-012`](../findings/F-SEN-012.md) | `F2-SEN-008` | 1+2 | Engine client: single attempt; any failure is a permanent abstention | Low | 90 | E1 | Confirmed | — | run 1 E2 85; run 2 E1 90 |
| [`F-SEN-013`](../findings/F-SEN-013.md) | — | 1 | Non-UTF-8 `reason` stalls the indexer | Informational (refuted as filed) | 98 (refutation) | E1 | **Refuted** | — | run 1 refuted by source read and execution; `alloy-sol-types` unchanged at `fe9e84c` |
| [`F-SEN-014`](../findings/F-SEN-014.md) | (`F2-SEN-007`, one sentence) | 1 | Every participant submits `finalize`; `K − 1` revert | Informational | 88 | E2 | Confirmed | — | run 1 88; run 2 mentioned, not filed |
| [`F-SEN-015`](../findings/F-SEN-015.md) | `F2-SEN-001` (c) | 1+2 | A replayed engine check re-decides a committed vote; `InvalidReveal` or untracked; slashed | High | 98 | E1 | Confirmed | **assigned @rmeissner** — same fix as `F-SEN-001` | run 1 E1 98 (Anvil); run 2 inference only (under-weighted, §1.3) |
| [`F-SEN-016`](../findings/F-SEN-016.md) | D1 → `F2-SEN-010` | 1 | UNMERGED #914/#915 bundle | — | — | — | D1 **Superseded**; D2–D4 Forward-looking (#915 unmerged) | — | run 2 confirms D1 on the merged tree |
| [`F2-SEN-003`](../findings/F2-SEN-003.md) | `F-SEN-002` basis 4, `F-SEN-003` route, `F-SEN-004` C&R | 2 | `finalize()` drops a bonded entry whenever our own reveal was not observed | Medium | 90 | E1 | Confirmed | none — pairs with the `F-SEN-002`/`F-SEN-005` fixes | run 1 recorded the drop as a consequence only; run 2 E1 90, both routes at `fe9e84c`, option 1 compiled (42 tests) |
| [`F2-SEN-010`](../findings/F2-SEN-010.md) | `F-SEN-016` D1 | 2 (probed by run 1 on the branch) | Commits mined in the commit-deadline block are never tallied (#914) | High | 93 | E1 | Confirmed | none — route to `F-SEN-002`'s assignee | run 1 probe E1 85 on the unmerged branch; run 2 93 (Rust + forge), option 1 compiled 47/47 |
| [`F2-SEN-011`](../findings/F2-SEN-011.md) | — | 2 | `handle_arbitration_timeout` doc and metric assume a full refund | Informational | 80 | E2 | Confirmed | none — fold into the `F-SEN-005` fix | exposed by the delta's contract comments |
| [`F-XC-001`](../findings/F-XC-001.md) | `F2-XC-009` | 1+2 | No release profile; overflow checks off in shipped binaries | Informational | 93 | E1 | Confirmed | — | run 1 E1 93 (rustc flags); run 2 E1 90 (dev panic / release wrap) |
| [`F-XC-002`](../findings/F-XC-002.md) | — | 1 | Debug-derived secret types reach log sinks (union of `F-VAL-062` and `F-CORE-036`) | Low | 88 | E1 (leak refuted) / E2 | Verified (reduced); **not counted** — counted under `F-VAL-062` and `F-CORE-036` | #113 understated | run 1 88; run 2 checked the redaction, filed nothing |
| [`F-XC-003`](../findings/F-XC-003.md) | — | 1 | `deny_unknown_fields` + `flatten`: behaviour correct, no in-tree test | Informational | 96 | E1 | Verified (test gap) | — | run 1 six tests pass; run 2 re-executed as a rejected hypothesis |
| [`F-XC-004`](../findings/F-XC-004.md) | `F2-XC-003`, `F2-VAL-068`, `F2-SEN-009` b4, root half of `F-VAL-064` | 1+2 | Runtime images run as root on floating tags; no toolchain pin | Low | 85 | E2 | Confirmed | — | run 1 E2 85; run 2 E2 82 (+ `--version`, no `rust-toolchain.toml`) |
| [`F-XC-006`](../findings/F-XC-006.md) | `F2-XC-006` (related: `F-CORE-065`) | 1+2 | Persistent state not bound to chain, deployment or signer | Low | 90 | E1 | Confirmed | — | run 1 E2 78; run 2 E1 90 (two Anvil chains, request re-signed on chain B); A17 drops the restore trigger only |
| [`F-XC-007`](../findings/F-XC-007.md) | (`F2-XC-004` gate and surface extension) | 1+2 | No `cargo audit` gate in CI; feature width | Informational | 92 | E1 | Confirmed (item 2 refuted in run 1) | — | run 1 92; run 2 extends |
| [`F-XC-008`](../findings/F-XC-008.md) | (`F2-SEN-008` items 2/3) | 1 | Unconfigured `reqwest` clients follow redirects and honour proxy env | Informational (was Low) | 80 | E2 | Confirmed, narrowed; item 1 out of scope (engine) | — | items 2/3 `I` → E2 from pinned `reqwest`/`hyper-util` |
| [`F-XC-009`](../findings/F-XC-009.md) | `F2-XC-008`, `F2-SEN-009` b1 (items 3/4: `F-VAL-063`) | 1+2 | Samples ship a parseable well-known key and a live mainnet RPC | Low | 75 | E2 | Confirmed (item 2 refuted in run 1) | — | run 1 72; run 2 75 Informational; Low carried (the key parses and the service signs while the adjacent zero address fails loudly) |
| [`F-XC-011`](../findings/F-XC-011.md) | `F2-XC-004` | 1+2 | Dependency advisories: linked, no in-scope connection path | Informational (was Low) | 90 | E1 | Confirmed | — | run 1 95 Low (engine h2); engine out of scope → Informational; run 2 adds `rustls`, `lru`; `cargo tree -i` re-run agrees |
| [`F-XC-050`](../findings/F-XC-050.md) | (`F2-VAL-061` is its precondition) | 1 | No DKG membership check; pure-cardinality close; silent `None` finalisation | Medium (rollover) / Informational `known` (genesis, A16) | 48 | E2 | Plausible | — | run 1 48; **run 2 missed** the consequence while strengthening the precondition; stale comment `state/keygen.rs:451-452` |
| [`F-XC-051`](../findings/F-XC-051.md) | — | 1 | `verify_commitment` delegates structural checks to `frost-core` | Informational (was Low) | 85 | E2 | Settled in the code's favour | — | run 1 42; empty `c` → `Err(MissingCommitment)`, identity `c[0]` → `Err(InvalidIdentityElement)`; hardening option 1 stands |
| [`F2-XC-001`](../findings/F2-XC-001.md) | `F2-VAL-060` | 2 | Any configuration parse error prints the whole file, signer key included | High | 95 | E1 | Confirmed | — | run 2 five executions on both binaries; **run-1 miss** (the most consequential one) |
| [`F2-XC-002`](../findings/F2-XC-002.md) | `F2-VAL-065` | 2 | Sample `database` URL lacks `?mode=rwc`; documented first start fails | Low | 92 | E1 | Confirmed | — | run-1 miss |
| [`F2-XC-005`](../findings/F2-XC-005.md) | — | 2 | Secret deletion is logical only: bundled SQLite without `SECURE_DELETE` | Informational | 90 | E1 | Confirmed | — | one `H` row corrected in QA; conclusion strengthened |
| [`F2-XC-007`](../findings/F2-XC-007.md) | `F2-SEN-009` b2 | 2 | Signer key lingers in un-zeroized configuration buffers | Informational | 75 | E2 | Confirmed | — | run-1 lead SEN-H15 recorded, never filed |
| [`F2-XC-050`](../findings/F2-XC-050.md) | `F2-CORE-036` | 2 | Snapshot committed and pruned before the block's actions are enqueued; crash window | Low | 92 | E1 | Confirmed | — | fault-injected at the warp page and depth 0; sentinel `Claim` loss executed; run-1 miss |

### 2.2 Fold index — every run-2 file that is not itself canonical

| Run-2 ID | Canonical | Relation | Run-2 ID | Canonical | Relation |
| --- | --- | --- | --- | --- | --- |
| `F2-CORE-001` | `F-CORE-001` | CONFIRMS | `F2-VAL-001` | `F-VAL-001` | CONFIRMS |
| `F2-CORE-002` | `F-CORE-002` | CONFIRMS | `F2-VAL-002` | `F-VAL-002` | CONFIRMS |
| `F2-CORE-004` | `F-CORE-006` | CONFIRMS | `F2-VAL-005` | `F-VAL-004` | CONFIRMS (in-crate duplicate of `F2-VAL-063`) |
| `F2-CORE-005` | `F-CORE-004` | EXTENDS | `F2-VAL-006` | `F-VAL-003` | CONFIRMS |
| `F2-CORE-006` | `F-CORE-003` | CONFIRMS | `F2-VAL-030` | `F-VAL-030` | EXTENDS |
| `F2-CORE-008` | `F-CORE-005` | CONFIRMS | `F2-VAL-032` | `F-VAL-032` | EXTENDS |
| `F2-CORE-009` | `F-CORE-007` | CONFIRMS | `F2-VAL-033` | `F-VAL-040` | CONFIRMS |
| `F2-CORE-010` | `F-CORE-034` | EXTENDS | `F2-VAL-035` | `F-VAL-066` | EXTENDS (post-refactor shape) |
| `F2-CORE-030` | `F-CORE-031` | EXTENDS | `F2-VAL-060` | `F2-XC-001` | duplicate |
| `F2-CORE-031` | `F-CORE-030` | CONFIRMS | `F2-VAL-061` | `F-VAL-060` | EXTENDS |
| `F2-CORE-032` | `F-CORE-067` | CONFIRMS (in-crate duplicate of `F2-CORE-063`) | `F2-VAL-062` | `F-VAL-030` | CONFIRMS (in-crate duplicate of `F2-VAL-030`) |
| `F2-CORE-033` | `F-CORE-011` | CONFIRMS, narrows | `F2-VAL-063` | `F-VAL-004` | EXTENDS (rollover instance) |
| `F2-CORE-034` | `F-CORE-039` | CONFIRMS | `F2-VAL-065` | `F2-XC-002` | duplicate |
| `F2-CORE-035` | `F-CORE-033` | EXTENDS | `F2-VAL-066` | `F-CORE-030` | CONFIRMS (via `F2-CORE-031`) |
| `F2-CORE-036` | `F2-XC-050` | duplicate | `F2-VAL-067` | `F-VAL-063` | EXTENDS |
| `F2-CORE-060` | `F-CORE-060` | EXTENDS | `F2-VAL-068` | `F-XC-004` | CONFIRMS (via `F2-XC-003`) |
| `F2-CORE-061` | `F-CORE-063` | EXTENDS (also `F-CORE-062`) | `F2-SEN-001` | `F-SEN-001` | CONFIRMS (variant c → `F-SEN-015`) |
| `F2-CORE-062` | `F-CORE-061` | EXTENDS | `F2-SEN-002` | `F-SEN-002` | CONFIRMS |
| `F2-CORE-063` | `F-CORE-067` | CONFIRMS | `F2-SEN-004` | `F-SEN-005` | CONFIRMS |
| `F2-CORE-064` | `F-CORE-064` | EXTENDS (also `F-CORE-062`) | `F2-SEN-005` | `F-SEN-004` | CONFIRMS, settles the trigger |
| `F2-CORE-065` | `F-CORE-066` | CONFIRMS | `F2-SEN-006` | `F-SEN-007` | CONFIRMS |
| `F2-CORE-066` | `F-CORE-065` | CONFIRMS | `F2-SEN-007` | `F-SEN-006` | EXTENDS |
| `F2-XC-003` | `F-XC-004` | CONFIRMS | `F2-SEN-008` | `F-SEN-012` | CONFIRMS |
| `F2-XC-004` | `F-XC-011` | EXTENDS (gate half cross-referenced from `F-XC-007`) | `F2-SEN-009` | `F-XC-009` | CONFIRMS (b2 → `F2-XC-007`, b3 → `F-SEN-009`, b4 → `F-XC-004`) |
| `F2-XC-006` | `F-XC-006` | CONFIRMS | `F2-XC-008` | `F-XC-009` | EXTENDS |
| `F2-XC-009` | `F-XC-001` | CONFIRMS |  |  |  |

### 2.3 Merge notes — conflicts between the four parts and how they were resolved

1. **`F-VAL-064` versus `F-CORE-030` (one defect, two severities).** REC-VAL aligned `F-VAL-064` to Low to match the run-2 canonical `F2-CORE-031` (Low), while REC-CORE carried `F-CORE-030` at Medium 85 with reasons (#820 asked for an observable failure; the fix is two lines). Resolved: the exit-status defect is canonical under `F-CORE-030` at **Medium 85**; `F-VAL-064` keeps a row of its own at Low 72 only for its validator-deployment residual (`/health` unreachable in the shipped deployment), with its root-container sub-claim under `F-XC-004`.
2. **Chained folds.** Several run-2 files pointed at another run-2 file that is itself folded (`F2-VAL-066` → `F2-CORE-031` → `F-CORE-030`; `F2-VAL-068` → `F2-XC-003` → `F-XC-004`; `F2-VAL-005` → `F2-VAL-063` → `F-VAL-004`; `F2-VAL-062` → `F2-VAL-030` → `F-VAL-030`; `F2-CORE-032` → `F2-CORE-063` → `F-CORE-067`). Each now points at exactly one canonical (Section 2.2).
3. **`F2-SEN-009` split four ways.** REC-SEN folded its bullets into `F2-XC-008` (b1, b3), `F2-XC-007` (b2) and `F2-XC-003` (b4); REC-XC said REC-SEN keeps b3. Resolved: one canonical, `F-XC-009` (the file's primary claim, via `F2-XC-008`); b2, b3 and b4 are cross-references to `F2-XC-007`, `F-SEN-009` and `F-XC-004`.
4. **`F2-XC-004` folded into two run-1 rows** (`F-XC-011` and `F-XC-007`). Resolved: canonical `F-XC-011` (advisory reachability is the file's claim); the CI-gate and feature-width extension is cross-referenced from `F-XC-007`.
5. **`F2-CORE-010` named two run-1 parents** (`F-CORE-034`, `F-CORE-011`); `F2-CORE-061` and `F2-CORE-064` each named two (`F-CORE-063`/`F-CORE-062`, `F-CORE-064`/`F-CORE-062`). Resolved to the first-named parent in each case, the other kept as a cross-reference; `F-CORE-062` stays a canonical row of its own (the wedge is not stated by either run-2 file).
6. **`F2-CORE-036` / `F2-XC-050` claimed by two parts.** REC-CORE and REC-XC both discussed it; REC-XC wrote the ledger row and REC-CORE excluded it from its counts. Counted once, under cross-cutting.
7. **`F2-VAL-035` has no canonical row of its own.** `STATE.md` lists it as Confirmed Medium 78 in its own right; REC-VAL carried it under `F-VAL-066` (same root cause — retention set computed before the block's logs; same severity). Resolved as REC-VAL did; the `F2-VAL-035` ID remains the citation for the executed 19/20 evidence and is the ID the `F-VAL-005` reply names.
8. **`F-XC-002` union row.** REC-XC does not count it (its two halves are `F-VAL-062` and `F-CORE-036`); recorded here so the total of 95 is reproducible.
9. **`F-CORE-006` and `F-VAL-060` both counted.** Different layers (core cannot bind an event to an address; the validator accepts events from any watched address) and different remediations; both parts agree.
10. **`F-SEN-016`.** REC-SEN's status is "D1 superseded; D2–D4 forward-looking". Counted under Forward-looking with `F-CORE-068`/`069`; D1's pointer is `F2-SEN-010`.
11. **Status vocabulary unified by certainty band.** REC-VAL's ledger labelled `F-VAL-034` "Confirmed (benign)" at 55 and `F-VAL-037` "Confirmed" at 60 while its own status tally counted both as Plausible; here Plausible is 40–69 throughout, as PROMPT §8 defines it.
12. **`F2-SEN-001` variant (c).** REC-SEN keeps `F-SEN-015` as its own High row and records variant (c) as run 2's under-weighted counterpart; `F2-SEN-001`'s primary counterpart stays `F-SEN-001`.

### 2.4 Final counts

**Live canonical findings: 95** — 77 run-1 IDs and 18 run-2 IDs. Excluded from "live": `F-VAL-005` (Fixed), `F-VAL-033` (Out of scope, A17), `F-VAL-068` (Superseded), `F-SEN-013` (Refuted), `F-CORE-068`, `F-CORE-069`, `F-SEN-016` (Forward-looking), `F-XC-002` (union counted under its halves), and the 51 folded run-2 files.

| Severity | Count | Canonical IDs |
| --- | --- | --- |
| Critical | 1 | `F-VAL-001` |
| High | 14 | `F-CORE-001`, `F-CORE-002`, `F-CORE-031`, `F-CORE-060`, `F-VAL-030`, `F-VAL-032`, `F-VAL-039` (Plausible), `F-VAL-060` (conditional), `F-VAL-061`, `F-SEN-001`, `F-SEN-002`, `F-SEN-015`, `F2-SEN-010`, `F2-XC-001` |
| Medium | 24 | `F-CORE-003`, `004`, `012`, `030`, `033`, `034`, `035`, `061`, `062`, `063`, `064`, `066`, `067`; `F-VAL-002`, `003`, `004` (rollover), `063`, `066`, `F2-VAL-003`, `F2-VAL-031`; `F2-SEN-003`, `F-SEN-004`, `F-SEN-005`; `F-XC-050` (rollover) |
| Low | 38 | `F-CORE-005`, `006`, `007`, `008`, `009`, `010`, `011`, `032`, `036`, `037`, `039`, `040`, `065`, `F2-CORE-007`, `F2-CORE-011`; `F-VAL-031`, `034`, `035` (observation), `036`, `038`, `040`, `064`, `065`, `067`, `F2-VAL-004`, `F2-VAL-007`; `F-SEN-003`, `006`, `007`, `008`, `009`, `011`, `012`; `F-XC-004`, `006`, `009`, `F2-XC-002`, `F2-XC-050` |
| Informational | 18 | `F-CORE-038`, `F2-CORE-003`, `F2-CORE-067`; `F-VAL-037`, `062`, `F2-VAL-034`, `F2-VAL-064`; `F-SEN-010`, `014`, `F2-SEN-011`; `F-XC-001`, `003`, `007`, `008`, `011`, `051`, `F2-XC-005`, `F2-XC-007` |

| Status | Count | Notes |
| --- | --- | --- |
| Confirmed (incl. Verified, narrowed, residual, split, `known`) | 76 | of the live 95 |
| Plausible (40–69) | 18 | `F-CORE-010`, `032`, `037`, `040`, `061`, `062`, `063`; `F-VAL-004` (rollover), `031`, `034`, `036`, `037`, `038`, `039`, `F2-VAL-004`, `F2-VAL-007`; `F-SEN-008`; `F-XC-050` |
| Observation (< 40) | 1 | `F-VAL-035` |
| Fixed by merge | 1 | `F-VAL-005` (residual D2 Informational `known`; restart race under `F-VAL-066`) |
| Out of scope (A17) | 1 | `F-VAL-033` |
| Superseded | 1 | `F-VAL-068` (+ `F-SEN-016` D1) |
| Refuted | 1 | `F-SEN-013` |
| Forward-looking (unmerged code) | 3 files | `F-CORE-068`, `F-CORE-069`, `F-SEN-016` D2–D4 |
| Folded run-2 files | 51 | Section 2.2 |

`F-CORE-037` (62) is Plausible by band although REC-CORE's row labelled it Confirmed; the severity table counts it under Low either way. The per-crate split is core 35 (High 4, Medium 13, Low 15, Informational 3), validator 28 (Critical 1, High 5, Medium 7, Low 11 incl. the observation, Informational 4), sentinel 17 (High 4, Medium 3, Low 7, Informational 3), cross-cutting 15 (High 1, Medium 1, Low 5, Informational 8).

**Executed evidence (E1) among the live 95: 49 rows** — core 12 (`F-CORE-001`, `002`, `004`, `031`, `036`, `060`, `061`, `063`, `064`, `066`, `067`, `F2-CORE-011`), validator 16 (`F-VAL-001`, `002`, `003`, `004`, `030`, `032`, `034`, `040`, `060`, `061`, `062`, `066`, `F2-VAL-003`, `F2-VAL-031`, `F2-VAL-034`, `F2-VAL-064`), sentinel 12 (`F-SEN-001`, `002`, `003`, `004`, `005`, `006`, `007`, `009`, `012`, `015`, `F2-SEN-003`, `F2-SEN-010`), cross-cutting 9 (`F-XC-001`, `003`, `006`, `007`, `011`, `F2-XC-001`, `002`, `005`, `050`). The remaining 46 are E2; no live row rests on inference alone, though several carry an `I` sub-claim (Section 7). Every Critical and High row except `F-VAL-039` is E1.

### 2.5 Moves made by reconciliation

- **Severity up:** `F-CORE-031` Medium → High (run 2's executed ordering extension makes the loss deterministic across a restart); `F-VAL-060` Medium → High, conditional (run 2 executed three scenarios); `F-VAL-032` Medium/High → High unconditional (run 2's attacker-created group removes the precondition).
- **Severity down:** `F-CORE-011` Medium → Low (pinned `reqwest` source: `tcp_user_timeout` 30 s); `F-VAL-064` Medium → Low (split; its halves are canonical elsewhere); `F-VAL-067` Medium → Low (code check plus A16); `F-VAL-066` Medium/High → Medium (the unqualified `DELETE` is gone; the race remains); `F-XC-008` and `F-XC-011` Low → Informational (engine out of scope); `F-XC-051` Low → Informational (pinned `frost-core`/`frost-secp256k1`); `F-VAL-004` High → Medium rollover / Informational genesis (A16); `F-XC-050` genesis instance → Informational `known` (A16); `F-VAL-033` High → out of scope (A17).
- **Lifted from Plausible to Confirmed:** `F-CORE-006`, `F-CORE-007`, `F-CORE-011`, `F-SEN-004` (branch ii).
- **Run-2 numbers not carried, recorded as dissent in the finding files:** Medium on `F-CORE-001`/`002`; Low on `F-CORE-003`/`030`/`033`/`064`/`066`/`067`, `F-VAL-002`/`003`; Medium on `F-VAL-030` (C2-VAL-B); Informational on `F2-XC-008` (→ `F-XC-009`) and `F2-VAL-068` (→ `F-XC-004`).
- **Run-1 numbers not carried:** Medium 78 on `F-CORE-031`; Medium 60 on `F-CORE-011`; Medium 50 on `F-VAL-060`; Plausible 62 on `F-SEN-004`; the "cap bypassed ~28,700×" sentence of `F-CORE-060` is rewritten (§1.3), the number itself stands.
- **Nothing was dropped.** Every run-1 finding keeps a row; the only removals are by fix (`F-VAL-005`), by assumption (`F-VAL-033`) or by supersession (`F-VAL-068`, `F-SEN-016` D1), each with a pointer.

## 3. Run-1 findings run 2 did not rediscover

A miss is information about run 2's coverage, not a reason to drop the finding. "Examined" means a run-2 reviewer log records the topic and a decision not to file; "missed" means no trace of it. Status is at `fe9e84c`.

| Run-1 finding | Severity / cert. | Run-2 coverage | Classification |
| --- | --- | --- | --- |
| `F-CORE-008` wall-clock polling | Low 70 | examined (R1 O3, ~40 %), not filed | still valid |
| `F-CORE-009` block-watcher config | Low 78 | case 1 only (R1 O4) | still valid; **miss** for cases 2–3 |
| `F-CORE-010` `-32001` rewind before validation | Low 45 | examined and rejected (R1 rejected 2) — independent confirmation of "no trigger" | still valid as filed |
| `F-CORE-012` bloom-equality blind spot | Medium 70 | **missed** | still valid — the second independent route to `F-CORE-002`'s outcome |
| `F-CORE-032` panicking effect task | Low 45 | **missed** (R2 rejected 9 covers cancel-safety only) | still valid |
| `F-CORE-035` `Rpc` errors swallowed forever | Medium 78 | **missed** the mechanism (`F2-CORE-067` covers the metrics leg) | still valid |
| `F-CORE-036` `Debug` bound, trace sinks | Low 85 | examined (R2 rejected 1, O7) | still valid |
| `F-CORE-037` unversioned snapshots | Low 62 | examined (R2 O10; `F2-CORE-066` asks for a schema version) | still valid |
| `F-CORE-038` KDF `info` concatenation | Informational 85 | examined (R2 rejected 13, O9 adds a nuance) | still valid |
| `F-CORE-040` abandoned `eth_getLogs` per resume | Low 65 | safety examined, cost **missed** | still valid |
| `F-CORE-062` nonce never released (as a standalone) | Medium 60 | halves rediscovered (`F2-CORE-061`, `F2-CORE-064`) | still valid; partial |
| `F-VAL-031` dead nonce worker | Low 42 | **missed** | still valid (`nonces.rs:101`, `117`) |
| `F-VAL-034` nonce resume without signature-id check | Low 55 | **missed** | still valid (`state/sign.rs:359-377`) |
| `F-VAL-036` non-monotonic `observe` | Low 40 | **missed** (forward jump only, in `F2-VAL-061` b) | still valid (`preprocess.rs:194`) |
| `F-VAL-037` Merkle padding | Informational 60 | **missed** | still valid |
| `F-VAL-038` chunk generation saturates cores | Low 55 | **missed** | still valid; the pruning series adds an inline writer |
| `F-VAL-039` top-up headroom | High 58 | **missed** (both runs' R5 declined the drain framing; run 1's Critic filed it) | still valid; needs an executed cost model (§7) |
| `F-VAL-062` `Debug` on secret-bearing effects | Informational 88 | **missed** (hygiene gap) | still valid |
| `F-VAL-065` validator legs of non-idempotent actions | Low 70 | **missed** (core side rediscovered) | still valid (`action.rs:252-254`, `368-378`, `main.rs:82-92`) |
| `F-VAL-063` points (1) and (3) | Medium 72 | partial (timing relation, `genesis_salt`, chain binding rediscovered) | still valid; partial |
| `F-VAL-035` (a)/(b) | Low 35 | **missed** as an observation | still true, below threshold |
| `F-VAL-068` D4 (marker is a number, no hash) | — | not filed in that form (`F2-XC-006` nearest) | superseded (branch merged) |
| `F-VAL-005` | High 99 | residual rediscovered (`F2-VAL-035`) | **fixed by the merge** for the reproduced trigger |
| `F-VAL-033` | High 72 | not looked for | **out of scope (A17)** — not a miss |
| `F-SEN-008` gas limits and fee-token assumptions | Low 52 | **missed** (`F2-SEN-006`/`007` cite the literals as facts) | still valid; contract now documents the fee-token precondition |
| `F-SEN-011` restart orphans an in-flight check | Low 82 | mechanism carried by `F2-CORE-030`, sentinel fact not restated | still valid |
| `F-SEN-003` warp residual | Low 80 | route executed in `F2-SEN-003` (b), residual not re-filed | still valid; partial |
| `F-SEN-014` `finalize` herd | Informational 88 | one sentence in `F2-SEN-007` | still valid; partial |
| `F-SEN-015` replayed check re-decides | High 98 | `F2-SEN-001` (c), inference only | still valid; partial (under-weighted) |
| `F-SEN-013` | refuted | correctly not re-filed | refutation holds |
| `F-XC-002` union | Low 88 | redaction checked, hygiene gap not filed | still valid (soft miss) |
| `F-XC-003` unknown-fields test gap | Informational 96 | behaviour re-executed as a rejected hypothesis | still valid as a test gap |
| `F-XC-050` DKG membership check | Medium 48 | **missed** (precondition strengthened by `F2-VAL-061`, consequence not filed) | still valid; A16 split |
| `F-XC-051` structural checks delegated | Low 42 | **missed**, then settled from pinned sources | settled → Informational 85 |
| `F-CORE-068`, `F-CORE-069`, `F-SEN-016` D2–D4 | forward-looking | not in the tree | not a miss |

## 4. A16 and A17 — consolidated status changes

A16 (genesis need not be recoverable; genesis-only liveness findings are Informational and `known`) and A17 (only the services touch their databases; out-of-band database triggers are out of scope) were proposed by the team on PR #905 and adopted for run 2. Applied to run 1's surviving findings:

| Finding | Assumption | Change | Justification |
| --- | --- | --- | --- |
| `F-VAL-004` | A16 | High 93 → genesis instance Informational `known`; rollover instance **Medium 69** (in scope) | the filed impact was genesis liveness (team: accepted); `F2-VAL-063` shows the same lost `KeyGenSetup` forfeits a numbered epoch |
| `F-VAL-033` | A17 | High 72 → **out of scope** | trigger is an operator database restore; mechanism recorded as real |
| `F-VAL-067` | A16 + evidence | Medium 48 → **Low 70** | genesis `Halted` branch is A16-Informational; for numbered epochs the abort is one coordinated extra ceremony |
| `F-XC-050` | A16 | **split**: genesis instance Informational `known`; rollover instance Medium / Plausible 48 | the cardinality close also feeds `finalize_key_gen` for rollovers (`state/keygen.rs:512`, `552-557`, `599`), reaching later epochs |
| `F-CORE-065` | A17 | trigger 1 (reused or restored database) out of scope; Low and `known` unchanged | triggers 2–3 are in-band configuration changes |
| `F-CORE-060` | A17 | "database restored from backup" starter out of scope; High unchanged | the foreign-transaction and stricter-node starters remain |
| `F-CORE-033` | A17 | "restore from backup" dropped from the trigger list; unchanged | `start_block` back-fill and restart after an outage remain |
| `F-CORE-001` | A17 | restored-backup aside out of scope; unchanged | primary triggers are process restarts |
| `F-XC-006` | A17 | "restore into a differently-configured host" trigger out of scope; Low unchanged | the config-edit trigger is the one run 2 executed |
| `F-VAL-005`, `F-VAL-060`, `F-VAL-066`, `F-VAL-063` point 2 | A16/A17 | no change | not genesis-only; config-only repoint stays in scope |
| all sentinel findings, all other core and cross-cutting findings | — | no change | none is genesis-scoped; every restart trigger is a plain process restart against an intact database |
| run-2 files | — | as set by their Critics | `F2-VAL-004` (b) genesis case Informational `known`; `F2-VAL-003` regular epochs only; `F2-VAL-034` in scope (a service upgrade is not out-of-band access); `F2-SEN-001`/`003`'s A17 tag is informational only |

No finding was removed by A16; one (`F-VAL-033`) was removed by A17.

## 5. Team dispositions and draft replies

### 5.1 Dispositions from PR #905, folded in

| Finding | Reviewer | Disposition | Run-2 evidence bearing on it | Combined status |
| --- | --- | --- | --- | --- |
| `F-ENG-030` | @rmeissner | scope | — | engine out of scope in both runs |
| `F-VAL-005` | @rmeissner | fix claimed (prune refactor) | store tests 13/13 at `fe9e84c`; `F2-VAL-035` (19/20), `F2-VAL-031` | **fixed** for the reproduced trigger; two residuals (§5.2) |
| `F-SEN-002` | @rmeissner | assigned | `F2-SEN-002` E1; `F2-SEN-010` second undercount source | Confirmed High 98; fix must also close `F2-SEN-010` |
| `F-SEN-015` | @rmeissner | assigned | code unchanged; `F2-SEN-001` (c) | Confirmed High 98 |
| `F-VAL-004` | @rmeissner | accepted (genesis exception) | `F2-VAL-063` rollover instance | genesis accepted (A16); rollover Medium 69 in scope |
| `F-VAL-032` | @rmeissner | accepted | `F2-VAL-032` unconditional trigger | Confirmed High 93 |
| `F-CORE-067` | @rmeissner | question | `F2-CORE-063`/`032`; `F2-CORE-030`; contract at `fe9e84c` | answered (§5.2) |
| `F-SEN-005` | @rmeissner | assigned (easy fix) | `F2-SEN-004` E1; `F2-SEN-003` trap; `F2-SEN-011` | Confirmed Medium 95 |
| `F-SEN-003` | @rmeissner → @nlordell | design question | `F2-SEN-003` (b) executed at `fe9e84c`; `F2-VAL-003` | answered with the executed consequence (§5.2) |
| `F-VAL-033` (three threads) | @nlordell | objection + proposed assumption | — | out of scope (A17) |
| `F-CORE-033` | @nlordell | question | `F2-CORE-035`, `F2-SEN-005` | answered: the deadline does not bound the effects (§5.2) |

Not yet assigned or commented: every other High (`F-CORE-001`, `F-CORE-002`, `F-CORE-031`, `F-CORE-060`, `F-VAL-030`, `F-VAL-039` (Plausible), `F-VAL-060`, `F-VAL-061`, `F-SEN-001`, `F2-SEN-010`, `F2-XC-001` — eleven) and the Critical `F-VAL-001`.

### 5.2 Draft replies (one per open thread; addressed to the reviewer; the operator can paste them)

**`F-ENG-030` — @rmeissner.** Done: the engine is out of scope for both runs and every engine artefact was removed from the PR.

**`F-VAL-005` — @rmeissner ("this should be fixed with the prune refactor").** Confirmed fixed at `fe9e84c` for the case the audit reproduced (a reorg within `max_reorg_depth`, no restart): reconciliations are ordered by the stored marker (`crates/validator/src/secrets/store.rs:323-328`), collection waits for `safe` (`store.rs:356-375`), and the store tests pin it (13/13). Two residuals remain. (1) A group absent for more than `max_reorg_depth` blocks still loses the ceremony — the module doc concedes it (`store.rs:38-41`); we record it as known/Informational. (2) The driver spawns the block's `ReconcileGroupSecrets` (`crates/core/src/driver.rs:278`) and then prunes inline (`driver.rs:292-294`) with no join between them, so on the first live block after a restart a schedule the old branch wrote is collected before the replayed branch's reconciliation can cancel it — executed at the driver seam, secret lost in 19 of 20 rounds (`F2-VAL-035`, carried under `F-VAL-066`, Medium 78). Awaiting the reconciliation before `housekeeping`, or a synchronous first reconciliation on resume, closes (2) and also `F2-VAL-031`'s cold-generator window (`service/effect.rs:245-256`, where the below-marker early return precedes `generator.start`).

**`F-SEN-002` — @rmeissner (assigned).** Run 2 confirmed this independently at `fe9e84c` (`F2-SEN-002`). Two things before designing the fix. (1) #914 added a second undercount source: commits mined in block `commitDeadline` are discarded because the `CollectingVotes` switch at `crates/sentinel/src/service.rs:423-425` runs before that block's logs (`F2-SEN-010`, High). Counting from `NewRequest` onward closes this finding but not that one; keeping the tally live in `CollectingVotes` (`F2-SEN-010` option 1 — compiled, 47/47 tests) or not early-finalizing on local counts closes both. (2) The mirror case is now a stall, not only wasted gas: a reverted `Finalize` parks the entry in `WaitingForOutcome` with no deadline and no retry (`service.rs:657-671`, `484`), so a single registered sentinel that commits late and never reveals parks every honest sentinel. A deadline on `WaitingForOutcome` that re-emits `Finalize` belongs in the same change as `F-SEN-005`. Independently of the tally fix, "never drop a bonded entry" (`F2-SEN-003` option 1, compiled, all existing tests pass) converts the silent loss into a recoverable park.

**`F-SEN-015` — @rmeissner (assigned).** Unchanged at `fe9e84c` (`service.rs:156-171`, `176-179`, `214-224`; `hashCommitment` is bound at `bindings.rs:54-60` and never called). Both runs converge on the same fix as `F-SEN-001`: either a three-state `getCommitment(id, self)` effect (`Committed(hash)` / `NotCommitted` / `Unavailable`, where `Unavailable` keeps the entry and reveals from stored state), or persist `(request_id, approve, reason)` idempotently when the check resolves and let `handle_committed` in `WaitingForEngineCheck` record `self_committed` and emit a verdict-recovery effect. Both runs reject "snapshot the resume": the restart anchor is `latest − max_reorg_depth` (`crates/core/src/index/blocks.rs:255-266`), so a snapshot at `latest` is discarded anyway.

**`F-VAL-004` — @rmeissner (accepted, genesis exception).** Agreed for genesis — adopted as A16; the genesis instance is Informational/known. On "if this affects other flows this could change": it does. The same swallowed error and missing re-emission (`crates/validator/src/service/effect.rs:264-276`) forfeits a numbered epoch when a `KeyGenSetup` is failed or lost during a rollover, and a rolling upgrade escalates it to an epoch skip (`F2-VAL-063`: structural gap executed in `poc/F2-VAL-063`, trigger Plausible 69). We carry the rollover instance under this ID at Medium.

**`F-VAL-032` — @rmeissner (accepted).** Noted. For the record, run 2 removed the precondition: an attacker-created 2-of-2 group plus one `sign(G_att, m)` in the deterministic post-timeout block drops every honest validator's `WaitingForRequest` session (`crates/validator/src/state/sign.rs:106-114`), and a timed-out rollover has no re-proposal path (`F2-VAL-032`, executed). The finding no longer depends on `F-VAL-030` or `F-VAL-039`; High is unconditional.

**`F-CORE-067` — @rmeissner ("different nonces and one of them will revert, right?").** Yes. At `fe9e84c` `contracts/src/libraries/SentinelOracleCommitments.sol:92-96` rejects a duplicate `commit` with `AlreadyCommitted()` (now via `vote == NONE`), and run 1 observed exactly that live (nonces 2 and 3 in one block, revert `0xbfec5558`). Five qualifications: (1) the paired duplicate `approve` succeeds — one successful and one reverted transaction per replayed request; (2) the revert is invisible to the service — execution is inferred from the account nonce (`crates/core/src/tx/mod.rs:185-197`, `tx/storage.rs:224-231`), nothing is logged or metered; (3) it is not always gas only — the replayed duplicate `Commit` is what pins `self_committed = false` on the `F-SEN-001` bond-loss path (`F2-CORE-030`, executed), and the validator's replayed `Sign` burns a nonce sequence for the whole group (`F-VAL-065`); (4) the surface grew with #914 — `Claim` rows with `expires_at: None` from four handlers are never pruned; (5) the dedup must be keyed on identity — `(block hash, log index, action kind)` with `INSERT OR IGNORE` on a `UNIQUE` column — not on calldata, since a `request`-keyed dedup would drop a legitimate repeated `approve` and fail the following `Commit`.

**`F-SEN-005` — @rmeissner (assigned, easy fix).** Confirmed at `fe9e84c` (`F2-SEN-004`): both waiting states are retained unconditionally (`service.rs:484-485`), `DisputeTriggered.deadline` is decoded (`bindings.rs:44`) and dropped (`760-768`), and there is no `timeoutArbitration` binding or action kind. One trap: expiring the state by dropping the entry recreates the bonded-entry abandonment of `F2-SEN-003`, because every terminal handler ignores an untracked id (`521-527`, `576-582`, `692-698`, `798-804`). Fix shape both runs agree on: store the deadline in `WaitingForDisputeResolution`, emit a `TimeoutArbitration { id }` action once at `block > deadline`, keep the entry until `ArbitrationTimedOut`/`DisputeResolved`, and give `WaitingForOutcome` a deadline that re-emits `Finalize`. Fix the doc and metric of `handle_arbitration_timeout` in the same change (`service.rs:550-557`, `585`): the contract now states the non-revealer's slash is never refunded (`SentinelOracleRequests.sol:288-296`; `F2-SEN-011`).

**`F-SEN-003` — @rmeissner → @nlordell ("why no `NewBlock` when warping?").** The design intent is yours to state; what run 2 adds is that the consequence is executed and current at `fe9e84c`: the warp arm returns no `NewBlock` (`crates/core/src/state/mod.rs:173-181`), so no deadline advances across a replayed range. In `poc/F2-SEN-003` (b) our own `Revealed@122` inside the warp page is discarded (`service.rs:347-362`), `NewBlock(125)` re-emits a `Reveal` that reverts `AlreadyRevealed`, and the entry is later dropped with no `Claim` — bond plus fee share unclaimed, no metric. The validator has the same gap (`F2-VAL-003`: a warp over an epoch boundary drops the key-gen round and forfeits the epoch). Two remediations both runs endorse: synthesise `Message::NewBlock(to)` at the end of a warp page, emitted before the page's logs (a core change the sentinel's comparisons already tolerate), or, sentinel-side, never drop a bonded entry (`F2-SEN-003` option 1, compiled, all tests pass) and reconcile a `Revealed(self)` seen in `CollectingCommitments`.

**`F-VAL-033` — @nlordell (three threads).** Adopted as A17: the database is modified only by the services, so this finding is out of scope. The mechanism it claims is real — `take_nonce` deletes the row on the spot and nothing records consumption (`crates/validator/src/secrets/store.rs:281-291`), so a restored file re-offers a burned nonce — but the only trigger is an operator restore, which A17 excludes. Your objection to remediation option 1 stands: a restore can drop the consumed-nonce row as easily as the chunk row. In the live run the validator self-halted before any reuse.

**`F-CORE-033` — @nlordell ("I would expect the transaction deadline to prevent this fan-out").** Checked in run 2: it does not. The deadline bounds the `ApproveToken`/`Commit` rows (`expires_at = commit_deadline`, `crates/sentinel/src/service.rs:226-242`; expired rows skipped at allocation, `crates/core/src/tx/storage.rs:150-155`), not the `EngineCheck` effects that precede them: the effect is emitted unconditionally on `TransactionProposed` (`service.rs:137-144`) and carries no deadline (`crates/sentinel/src/effect.rs:21-25`); deadlines are consulted only in `handle_block_advance` on `NewBlock`, and a warp page delivers `Event`s only (`crates/core/src/state/mod.rs:173-181`, `200-239`), so every proposal in a catch-up page (up to 100 blocks) is checked, including ones whose deadline passed during the downtime. The only per-effect bound is `engine_timeout` (≈ 371 s with the sample config), which bounds duration, not count. Executed downstream in `poc/F2-SEN-005`: reveals starve behind commits from about 8 proposals per block at parity windows. Fix: a concurrency cap in `EffectManager` (option 1 in both runs), plus either passing the request deadline into the effect so expired checks resolve immediately, or a synthetic `NewBlock(to)` at the end of each warp page — which is also the answer to the `F-SEN-003` question.

## 6. The merge of `main` (`3ec8bc5` → `fe9e84c`)

`origin/main` moved `5cc096e` → `8b6a75d` (20 commits) during run 2's Phase 3 and was merged locally. In scope, only two files changed: `crates/sentinel/src/service.rs` (+189) and `crates/sentinel/src/bindings.rs` (+2); `crates/core` and `crates/validator` are byte-identical, `Cargo.lock` differs by two engine-only lines ([`baseline-delta.md`](../state/run2/baseline-delta.md) §2).

**PR #914 "Adjust deadline handling for quicker reaction" (`8b6a75d`).** Every deadline comparison in `handle_block_advance` moved one block earlier (`<=` → `<`): `WaitingForEngineCheck` and `WaitingForRequest` drop at `block == deadline`; `CollectingCommitments` switches to `CollectingVotes` and emits `Reveal` at `block == commit_deadline` (previously `+ 1`), while an entry with `self_committed == false` is kept through the deadline block so a `Committed` of ours mined in that block is not forfeited. A `STOPGAP` note (`service.rs:390-402`) says the compensation must be reverted with safe-research/safenet#471 (block transitions against the _pending_ block, PR #915). Two tests were added.

- **Fixed: nothing of ours.** R7Δ re-validated all nine pre-merge sentinel findings as still valid; QA2-SEN-Δ re-ran the twelve sentinel PoC tests: seven byte-identical, `F2-SEN-005` narrowed (the one-block reveal shift moves the FIFO knife-edge, 85 → 78) but not fixed. `F-SEN-005` is untouched (`event.deadline` still unread, waiting states still never expire); `F-SEN-003`'s restart variant narrowed by one block; `F-SEN-002`'s trigger set broadened.
- **Introduced: `F2-SEN-010`, High 93.** A `Committed` accepted on chain at `block.number <= commitDeadline` (`SentinelOracleRequests.sol:117`) but mined in the deadline block arrives after the local switch to `CollectingVotes` (`service.rs:423-425` precedes that block's logs, `state/mod.rs:190-216`) and is discarded; the undercount feeds `revealed_count >= committed_count` and `finalize()`. Honest ordering takes outcome 2 (reverted `Finalize`, parked forever in `WaitingForOutcome`); the peer-first ordering takes outcome 1 (entry dropped, bond unclaimed). Executed in Rust and forge; option 1 (keep tallying in `CollectingVotes`) compiled in-tree, 47/47 tests.
- **Relation to run 1's in-flight warning.** Run 1 wrote, of the unmerged branch, "#914 alone worsens `F-SEN-002`: a valid peer commit in the deadline block is discarded" (`F-SEN-016` D1, executed probe 85). Same hunk, same consequence chain; run 2 books it as a separate defect because root cause and remediation are disjoint from `F-SEN-002`'s (a counter born at zero on the engine resume versus a tally frozen at the phase switch). Only "stop early-finalizing on local tallies" closes both. `F-SEN-016` D1 is superseded by `F2-SEN-010`; D2–D4 stay forward-looking because #915 is not merged (`origin/feat/optimistic_block_transition` `b2aad06`, 2 ahead / 31 behind `main`).

**Oracle-audit contract fixes (#939–#945) and #946–#950.** `SentinelOracleCommitments.sol` gained `InvalidCommitHash()` and now checks `vote == NONE` for `AlreadyCommitted()`: the duplicate `commit` of `F-CORE-067`/`F-SEN-006`/`F2-SEN-007` reverts exactly as before (`F2-SEN-007` re-validated). `SentinelOracle.sol` comment-only changes document the fee-token requirements ("standard, non-rebasing ERC20 … no transfer hooks") — the contract-side half of `F-SEN-008` option 4; the hooked-token variant is now a documented precondition, the proxy-gas and allowance-reset variants are not — and state that a non-revealer's slash is never refunded, which exposed `F2-SEN-011` and superseded `F2-SEN-004` claim 5. `SentinelOracleRequests.sol`'s DAO-fee rounding has no Rust counterpart. The `NewRequest` event gained `daoFeeShare` (mirrored in `bindings.rs`; ABI absorbed, no finding). Nothing in this series touched a core or validator finding. The engine phases (#918–#928) are out of scope; #917 (SEF veto epic) merged a document only.

### 6.1 Remediation convergence

Both runs, independently, arrive at the same fixes. The report can present them as one list; each closes several canonical rows.

1. **A durable pending-effect set re-issued on resume, plus a concurrency cap in `EffectManager`** — `F-CORE-031`, `F-CORE-033`, `F-SEN-004`, `F-SEN-011`, `F-VAL-004` (rollover), `F-VAL-061`'s policy. Run-1 QA's caveat: a queued-but-unspawned effect is lost on shutdown exactly like an in-flight one, so the cap's pending queue must be the same durable structure.
2. **A verdict-reconciliation effect (`getCommitment(id, self)` as a three-state result, or persisting the verdict idempotently when the check resolves)** — `F-SEN-001` and `F-SEN-015` together; helps `F-SEN-006`. Both runs reject "snapshot the resume".
3. **Never drop a bonded entry; deadlines on `WaitingForOutcome` and `WaitingForDisputeResolution`; a `TimeoutArbitration` action** — `F2-SEN-003`, `F-SEN-005`, `F-SEN-002` outcome 1, `F2-SEN-010` outcome 2's parking, `F2-SEN-011`'s doc/metric. Compiled by run-2 QA, existing tests pass.
4. **Tally through block `commitDeadline`, or stop early-finalizing on local counts** — `F-SEN-002` and `F2-SEN-010`. Only the second closes both and run 1's A4 trigger (a `Committed` lost to an incomplete `eth_getLogs`).
5. **A synthetic `Message::NewBlock(to)` at the end of each warp page, emitted before the page's logs** — `F-SEN-003`'s route, `F2-VAL-003`, the warp half of `F-CORE-033`. The validator needs the same `<`/`<=` check the sentinel got in #914 before this lands.
6. **An identity-keyed idempotency column on `enqueue` (`(block hash, log index, action kind)`, `UNIQUE`, `INSERT OR IGNORE`), retained beyond the row's lifetime or with `F-CORE-063` fixed first** — `F-CORE-067`, `F-VAL-065`, `F-SEN-006`.
7. **Await the block's reconciliation before `housekeeping`, or run a synchronous first reconciliation on resume** — `F-VAL-066`/`F2-VAL-035`, `F2-VAL-031`.
8. **A proof of possession for `q` and a KDF-bound share pad** — `F-VAL-001`, `F-VAL-002`. Needs a coordinator/`keyGenChallenge` change; does not break honest late joiners (run-2 QA).
9. **Persist and verify the rollback anchor's hash, including on a fresh start; return a non-zero status from `main` on a fatal driver error** — `F-CORE-001`, `F2-CORE-011`, `F-CORE-030`.
10. **Configuration: `toml::de::Error::set_input(None)` (one line), validation of `tx::Config` and of the consensus timing relations, `?mode=rwc` in the samples** — `F2-XC-001`, `F-CORE-066`, `F-VAL-063`, `F2-XC-002`.
11. **An absolute or base-fee-relative ceiling on the fee ratchet, and recognition of first-submission underpriced rejections** — `F-CORE-060`, `F-CORE-061`.

## 7. Unsettled items

Items the audit could not execute; each is carried at the certainty the parts assigned, with the position of both runs where they differ.

| Item | What is missing | Both runs' position |
| --- | --- | --- |
| `F-VAL-039` (High, Plausible 58) | an executed cost model of the top-up griefing against a live group | run 1's Critic filed it; both runs' R5 declined the drain framing as cost-of-attack; run 2 names the endpoint in `F2-VAL-032` |
| `F-VAL-038` (Low 55) | the duration of the 1025-statement transaction under load | unmeasured in both runs; the pruning series added an inline writer |
| `F2-VAL-035` / `F-VAL-066` (Medium 78) | the live occurrence rate of the precondition (a schedule written by the old branch that the replayed branch still needs) | executed only at the driver seam (19/20 when the precondition holds); the Anvil restart with a real group drop during the outage needs a multi-validator devnet |
| `F-CORE-061`, `F-CORE-062`, `F-CORE-063` (Plausible 65/60/55) | the decisive trigger step: node-side rejection wording (`I` in both runs) and a live wedge | in-crate mechanisms executed by run 2; both runs agree on mechanism |
| `F-SEN-004` branch (i) (engine saturation) | engine-dependent; out of scope | branch (ii) executed (queue starvation); branch (i) `I` in both runs |
| `F-SEN-015` variant 1 (a different verdict on replay) | engine determinism — out of scope | variant 2 (`Unknown` while the engine boots) is E1 and carries the finding |
| `F-VAL-004` rollover instance (Plausible 69) | a lost effect in a numbered epoch, live | structural gap executed (`poc/F2-VAL-063`); trigger traced |
| `F2-VAL-004`, `F2-VAL-007` (Plausible 50/65) | QA not attempted | run 2 only |
| `F-CORE-010` (Plausible 45) | any trigger | both runs independently found none |
| `F-XC-050` (Plausible 48) | the window (a complaint outstanding while confirmations arrive) | run 2 strengthened the precondition (`F2-VAL-061`) but did not file the consequence |
| `F-XC-051` residual | a short non-empty `c` under the `F-VAL-060` precondition | settled Informational from pinned sources; run-1 PoC left unrun |
| `F-SEN-008` (Plausible 52) | behaviour against a proxied or hooked fee token | run 2 missed it; the contract now documents the precondition |
| `F-CORE-060` accepted arm as a standalone trigger | an independent blocking condition | run 2 executed the arithmetic (×311.9); run 1's underpriced arm carries the High |
| `F-SEN-016` D2–D4, `F-CORE-068`, `F-CORE-069` | the merges of #915 and #904 | forward-looking; re-validate on merge ([`IN-FLIGHT.md`](IN-FLIGHT.md)) |

Engine-dependent `I` items (engine out of scope under Section 11): `F-SEN-004` (i), `F-SEN-015` variant 1, the engine-side half of `F-SEN-007`'s pre-flight, and `F-XC-008` item 1. None of them decides a severity: each finding's carried band rests on its executed or code-traced leg.

## 8. Reading guide for `REPORT.md`

- **IDs.** Use the canonical IDs of Section 2.1; a run-2 ID appears in the report only when it is canonical (18 of them) or as the counterpart named in a row's trail. The fold index (Section 2.2) answers "where did `F2-…` go".
- **Numbers.** Live findings 95: Critical 1, High 14, Medium 24, Low 38, Informational 18; by status Confirmed 76, Plausible 18, Observation 1; E1 49. Non-live rows: Fixed 1, Out of scope 1, Superseded 1, Refuted 1, Forward-looking 3 files, union 1, folded run-2 files 51. Files: 85 run-1, 69 run-2, 154 total.
- **Where each number's reasoning lives.** The severity and certainty settlements are written out per row in the per-crate parts ([core](../state/run2/reconciliation/core.md) §1.1–1.2 and §5, [validator](../state/run2/reconciliation/validator.md) §1 and §5, [sentinel](../state/run2/reconciliation/sentinel.md) §1 and §5, [cross-cutting](../state/run2/reconciliation/cross-cutting.md) §1 and §5); the `## Reconciliation (run 2)` section appended to every finding file states that file's final combined status and is authoritative where this ledger and a file differ.
- **Dissents.** Where the two runs disagreed on the band, the report should print the carried band and name the other run's band in the trail, as Section 2.1 does.
- **Sections to carry over.** Misses in both directions (Section 3), the A16/A17 changes (Section 4), the team dispositions and the draft replies (Section 5), the merge and `F2-SEN-010` (Section 6), the remediation convergence (Section 6.1), and the unsettled items (Section 7).
