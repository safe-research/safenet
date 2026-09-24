# Safenet Rust services — security and robustness review, combined result of two independent runs

This report replaces the run-1 report. It compiles the result of **two independent audits of the same code by two different models** — run 1 at `2893917`, run 2 at `3ec8bc5` and re-validated at `fe9e84c` — reconciled into one ledger, [`RECONCILIATION.md`](RECONCILIATION.md). Every number, severity, certainty and status below comes from that ledger and the finding files; nothing was re-judged here. Where this report and a finding file differ, **the finding file's `## Reconciliation (run 2)` section is authoritative**.

Companion documents: [`RECONCILIATION.md`](RECONCILIATION.md) (the combined ledger, fold index, draft replies), [`KNOWN-WORK.md`](KNOWN-WORK.md) (findings mapped onto the team's issues, TODOs and epics), [`IN-FLIGHT.md`](IN-FLIGHT.md) (unmerged PR stacks), [`../state/pr-review-threads.md`](../state/pr-review-threads.md) (the team's review comments and their dispositions).

## 1. Verdict

| Field | Value |
| --- | --- |
| Target | `crates/core`, `crates/validator`, `crates/sentinel`, the workspace manifests, those crates' Dockerfiles and sample configs — 61 `.rs` files, 20,492 lines, 185 tests at `fe9e84c`. `crates/sentinel-engine` is **out of scope** (Section 2.1) |
| Commit | `fe9e84cc59b65367b31d5a3121774383cc422234` — `origin/main` `8b6a75d` merged into `audit/rust-services`. Run 2's Phases 0–3 ran at `3ec8bc5`, whose `crates/core` and `crates/validator` are byte-identical to `fe9e84c`; `crates/sentinel` and the reference contracts were re-validated at `fe9e84c`. Run 1 audited `2893917` and re-validated at `a7f3915` |
| Runs | Run 1: 10 reviewers, 9 Critics, 4 QA, 4 verification agents, live Anvil validation with value moving. Run 2: a different model, no access to run-1 material, 8 reviewers, 7 Critics, 5 QA agents, a delta review of the merge, then four reconciliation agents that read both runs |
| Evidence | **49 of the 95 live findings carry executed evidence (`E1`)**; the other 46 are code-traced (`E2`); no live finding rests on inference alone. Every Critical and High except `F-VAL-039` is `E1` |
| Deliverables | 154 finding files in [`../findings/`](../findings/) (85 run-1 `F-*`, 69 run-2 `F2-*`), PoC directories under [`../poc/`](../poc/), run narratives in [`../state/STATE.md`](../state/STATE.md) and [`../state/run2/STATE.md`](../state/run2/STATE.md) |

**95 live canonical findings: Critical 1 / High 14 / Medium 24 / Low 38 / Informational 18.** By status: Confirmed 76, Plausible 18, Observation 1. Not counted as live: 1 fixed by the merge (`F-VAL-005`), 1 out of scope under A17 (`F-VAL-033`), 1 superseded (`F-VAL-068`), 1 refuted (`F-SEN-013`), 3 forward-looking files on unmerged code (`F-CORE-068`, `F-CORE-069`, `F-SEN-016`), 1 union row counted under its halves (`F-XC-002`), and the 51 run-2 files folded into a canonical row ([ledger §2.2](RECONCILIATION.md#22-fold-index--every-run-2-file-that-is-not-itself-canonical)).

### 1.1 How far two independent audits agree

The agreement rate is itself a result. Run 2 could not read run 1; after run 2's QA, one reconciliation agent per crate mapped every run-2 finding to run 1.

| Crate | Run-2 files | Confirms a run-1 finding | Extends one | Contradicts one | Genuinely new | Run-1 live findings | Not re-filed by run 2 | Run-2 canonical items | of which reachable at `2893917` (run-1 misses) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| core | 26 | 13 | 8 | 0 | 4 | 31 | 10 (+1 partial) | 4 | 4 |
| validator | 22 | 8 | 7 | 0 | 7 | 22 | 9 (+1 partial) | 6 | 4 |
| sentinel | 11 | 7 | 3 | 0 | 1 | 14 | 2 (+3 partial) | 3 | 1 |
| cross-cutting | 10 | 3 | 2 | 0 | 5 | 10 | 4 | 5 | 5 |
| **total** | **69** | **31** | **20** | **0** | **17** | **77** | **25** (+5 partial) | **18** | **14** |

- **Existence and mechanism: zero contradictions.** No run-2 finding contradicts a run-1 finding as its primary relation. Three sub-claim disagreements were found and all three were settled by executed evidence (Section 7.1).
- **Rediscovery: 52 of 77** run-1 live findings (68 %) were rediscovered in full or in part. Of the 25 not re-filed, 6 were examined by a run-2 reviewer and deliberately not filed; **19 are misses proper, and all 19 are still valid at `fe9e84c`** (Section 7.3).
- **The other direction: run 1 missed 14** of run 2's 18 canonical findings that were reachable at `2893917`, including one High (`F2-XC-001`); the other 4 exist because of code run 1 did not have (`F2-VAL-031`, `F2-VAL-034`, `F2-SEN-010`, `F2-SEN-011`).
- **Severity bands:** over the 48 pairs where both runs rated the same defect, the bands agreed on 23; run 2 was one band lower on 23 (mostly core, where its Critics read A1/A4 and "bounded cost" strictly) and higher on 2. The sentinel agreed on every pair. Executed evidence settled each disagreement; the dissenting band is recorded in the finding file and shown in Section 6.
- **Certainty:** run 2's execution raised eleven run-1 numbers and lifted four run-1 findings from Plausible to Confirmed (`F-CORE-006`, `F-CORE-007`, `F-CORE-011`, `F-SEN-004`).

### 1.2 The Critical

| ID | Runs | Cert. | Claim | What executed |
| --- | --- | --- | --- | --- |
| [`F-VAL-001`](../findings/F-VAL-001.md) | 1+2 | 97 | DKG encryption key `q` has no proof of possession: a registered participant that republishes a peer's `q` recovers that peer's **complete FROST signing share** while the group finalises normally | Run 1: the crypto in-process, then the full onchain sequence against the real `FROSTCoordinator`/`FROSTParticipantMap` bytecode, 5/5 fresh seeds — the contracts block nothing ([`poc/F-VAL-001`](../poc/F-VAL-001/), [`poc/F-VAL-001-onchain`](../poc/F-VAL-001-onchain/)). Run 2, independently: a pure `frost` test recovers A's full share from public ciphertexts plus complaint reveals, `s_a_recovered == s_a_ref` ([`poc/F2-VAL-001`](../poc/F2-VAL-001/)) |

**Team disposition: none yet.** It is the only Critical and it is unassigned.

### 1.3 What moved on chain

All on local Anvil (Section 9.3). Sentinel bond lost on an ordinary restart: **−4,000 fee tokens, 2,000 slashed** (`F-SEN-001`); the same 4,000 through three HTTP 429s stripping the log-integrity check (`F-CORE-002`); **4,500 left unclaimed** (`F-SEN-002`); a replayed engine check reverting `InvalidReveal 0x9ea6d127` (`F-SEN-015`); the fee ratchet at **28,744×** the base-fee-derived cap (`F-CORE-060`: tip 1 → 11,527 → 201,207 wei, max fee 4,239 gwei against a base fee of 772 wei); the pruning race losing a still-needed DKG secret in **19 of 20** rounds at the driver seam (`F2-VAL-035`, under `F-VAL-066`).

### 1.4 By crate

| Crate | Live | Critical / High | Executed (`E1`) | What matters most |
| --- | --- | --- | --- | --- |
| core | 35 | 0 / 4 | 12 | resumes discarded on rollback and restart (`F-CORE-031`), the unverified restart anchor (`F-CORE-001`), the self-disabling log-integrity check (`F-CORE-002`), the fee ratchet (`F-CORE-060`) |
| validator | 28 | 1 / 5 | 16 | the DKG proof of possession (`F-VAL-001`); every effect failure forgotten (`F-VAL-061`, `F-VAL-030`); a session dropped by any untracked group's `Sign` (`F-VAL-032`) |
| sentinel | 17 | 0 / 4 | 12 | bond lost on an ordinary restart (`F-SEN-001`, `F-SEN-015`); bond never claimed or every honest sentinel parked (`F-SEN-002`, `F2-SEN-010`) |
| cross-cutting | 15 | 0 / 1 | 9 | the signer key printed on any config typo (`F2-XC-001`); no chain binding of persistent state (`F-XC-006`); the live-RPC samples (`F-XC-009`) |

## 2. What changed since the team last read this report

### 2.1 The engine is out of scope

At @rmeissner's request ("I would ignore all engine changes for now") `crates/sentinel-engine` was removed from scope for both runs. Every engine artefact left `rust-audit/`: the 24 `F-ENG-*` files, the engine-specific cross-cutting findings (`F-XC-005`, `F-XC-010`, `F-XC-052`), the engine PoCs and the engine dependency questions. Consequences for the remaining findings: `F-XC-008` item 1 (the CoW client) is out; `F-XC-008` and `F-XC-011` drop from Low to Informational because no remaining advisory has an in-scope connection path; assumption A8 (the engine test-vector corpus) is moot; `F-SEN-004` branch (i) and `F-SEN-015` variant 1 keep their engine-dependent legs as `I`, and neither decides a severity. Run 1's 108 findings became 85 surviving files.

### 2.2 A16 and A17, from the team's own comments

Two assumptions proposed in the review threads were adopted for run 2 and applied to run 1's surviving findings ([ledger §4](RECONCILIATION.md#4-a16-and-a17--consolidated-status-changes)):

| Assumption | Text | Findings it moved |
| --- | --- | --- |
| **A16** | Genesis need not be recoverable; genesis-only liveness findings are Informational and `known` | `F-VAL-004` High 93 → genesis instance Informational `known`, **rollover instance stays, Medium 69**; `F-VAL-067` Medium 48 → Low 70 (with the `restart_key_gen_excluding` code check); `F-XC-050` split — genesis instance Informational `known`, rollover instance Medium / Plausible 48; `F2-VAL-004` (b) genesis case `known` |
| **A17** | Only the services modify their databases; out-of-band database triggers are out of scope | `F-VAL-033` High 72 → **out of scope** (mechanism real, `secrets/store.rs:281-291`; the only trigger is an operator restore); the "restored backup" starter or trigger dropped from `F-CORE-001`, `F-CORE-033`, `F-CORE-060`, `F-CORE-065`, `F-XC-006` — severities unchanged, the in-band triggers carry each one |

No finding was removed by A16; one was removed by A17.

### 2.3 The pruning merge (#906–#913)

The Scheduled Secret Pruning stack is on `main`. Effect on the findings, verified at `fe9e84c`:

- **Fixed: [`F-VAL-005`](../findings/F-VAL-005.md)** for the trigger the audit reproduced (a reorg within `max_reorg_depth`, no restart). Reconciliations are ordered by the stored marker (`crates/validator/src/secrets/store.rs:323-328`), collection waits for `safe` (`store.rs:356-375`), and the store tests pin it (13/13, `state/run2/logs/REC-VAL-store-tests.txt`).
- **Not fixed:** a group absent for more than `max_reorg_depth` blocks still loses the ceremony — the module doc concedes it (`store.rs:38-41`); recorded as `known`/Informational.
- **Not fixed, executed: the restart reconcile-vs-prune race.** The driver spawns the block's `ReconcileGroupSecrets` (`crates/core/src/driver.rs:278`) and then prunes inline (`driver.rs:292-294`) with no join, so on the first live block after a restart a schedule the old branch wrote is collected before the replayed branch's reconciliation can cancel it: secret lost in 19 of 20 rounds ([`poc/F2-XC-050/coverage-7.3`](../poc/F2-XC-050/coverage-7.3/); `F2-VAL-035`, carried under [`F-VAL-066`](../findings/F-VAL-066.md), Medium 78).
- **Introduced:** [`F2-VAL-031`](../findings/F2-VAL-031.md) (Medium 80) — nonce generators are cold after every restart and the below-marker early return in `service/effect.rs:245-256` precedes `generator.start`; run 1 predicted it on the branch as `F-VAL-068` D1. [`F2-VAL-034`](../findings/F2-VAL-034.md) (Informational `known`) — no schema-version check, a pre-#908 database opens and then fails every reconciliation.
- `F-VAL-068` is **superseded**: D1 → `F2-VAL-031`, D2 → `F-VAL-005`'s documented residual, D3 → `F2-VAL-035`, D4 nearest `F2-XC-006`.

### 2.4 PR #914 merged alone, and what the merge of `main` changed

`origin/main` moved 20 commits during run 2's Phase 3 and was merged locally. In scope only two files changed: `crates/sentinel/src/service.rs` (+189) and `crates/sentinel/src/bindings.rs` (+2); core and validator are byte-identical ([`baseline-delta.md`](../state/run2/baseline-delta.md) §2).

- **#914 "Adjust deadline handling for quicker reaction" fixed nothing of ours.** All nine pre-merge sentinel findings were re-validated and the twelve sentinel PoC tests re-run at `fe9e84c`: seven byte-identical, `F-SEN-004` narrowed (85 → 78) but not fixed, `F-SEN-005` untouched (`event.deadline` still never read, waiting states still never expire), `F-SEN-003`'s restart variant narrowed by one block, `F-SEN-002`'s trigger set broadened.
- **#914 introduced [`F2-SEN-010`](../findings/F2-SEN-010.md), High 93.** The `CollectingVotes` switch now runs at `NewBlock(commit_deadline)` (`service.rs:423-425`), before that block's logs (`crates/core/src/state/mod.rs:190-216`), while the contract accepts a commit at `block.number <= commitDeadline` (`SentinelOracleRequests.sol:117`). Every commit mined in the deadline block is discarded, the local tally undercounts, and the early finalize fires: honest ordering takes a reverted `Finalize` parked forever in `WaitingForOutcome` (`service.rs:657-671`, `484`); peer-first ordering drops the entry with the bond unclaimed. Executed in Rust and forge; option 1 compiled in-tree, 47/47 tests.
- **Run 1 warned about this exact hunk.** `F-SEN-016` D1, executed on the unmerged branch at 85: "#914 alone worsens `F-SEN-002`: a valid peer commit in the deadline block is discarded". Same consequence chain; run 2 books it as its own defect because the root cause (a tally frozen at the phase switch) and the remediation are disjoint from `F-SEN-002`'s (a counter born at zero on the engine resume). Only "stop early-finalizing on local counts" closes both. `F-SEN-016` D1 is superseded by `F2-SEN-010`; D2–D4 stay forward-looking because #915 is unmerged.
- **Contract fixes (#939–#945):** `SentinelOracleCommitments.sol` now checks `vote == NONE` for `AlreadyCommitted()` — the duplicate `commit` of `F-CORE-067`/`F-SEN-006` reverts exactly as before. `SentinelOracle.sol` documents the fee-token requirements (the contract-side half of `F-SEN-008`) and states that a non-revealer's slash is never refunded, which exposed [`F2-SEN-011`](../findings/F2-SEN-011.md). Nothing touched a core or validator finding.

### 2.5 Team dispositions, and the list that remains

| Finding | Reviewer | Disposition | Combined status |
| --- | --- | --- | --- |
| `F-ENG-030` | @rmeissner | scope | engine out of scope in both runs |
| [`F-VAL-005`](../findings/F-VAL-005.md) | @rmeissner | fix claimed (prune refactor) | **fixed** for the reproduced trigger; two residuals (Section 2.3) |
| [`F-SEN-002`](../findings/F-SEN-002.md) | @rmeissner | **assigned** | High 98 Confirmed — the fix must also close `F2-SEN-010` |
| [`F-SEN-015`](../findings/F-SEN-015.md) | @rmeissner | **assigned** | High 98 Confirmed; same fix as `F-SEN-001` |
| [`F-SEN-005`](../findings/F-SEN-005.md) | @rmeissner | **assigned** (easy fix) | Medium 95 Confirmed; do not expire-and-drop (Section 4) |
| [`F-VAL-004`](../findings/F-VAL-004.md) | @rmeissner | accepted (genesis exception) | genesis accepted (A16); the rollover instance is the "other flow" the team asked about — Medium 69, in scope |
| [`F-VAL-032`](../findings/F-VAL-032.md) | @rmeissner | accepted | High 93 Confirmed; run 2 removed the precondition (an attacker-created 2-of-2 group suffices) |
| [`F-CORE-067`](../findings/F-CORE-067.md) | @rmeissner | question | answered (Section 8.1) |
| [`F-SEN-003`](../findings/F-SEN-003.md) | @rmeissner → @nlordell | design question | answered with the executed consequence (Section 8.1) |
| [`F-VAL-033`](../findings/F-VAL-033.md) | @nlordell (three threads) | objection, proposed assumption | out of scope (A17); the objection to remediation option 1 stands |
| [`F-CORE-033`](../findings/F-CORE-033.md) | @nlordell | question | answered: the deadline does not bound the effects (Section 8.1) |

None of the 13 team threads has a reply on GitHub yet; a paste-ready reply per thread is in [ledger §5.2](RECONCILIATION.md#52-draft-replies-one-per-open-thread-addressed-to-the-reviewer-the-operator-can-paste-them). **Unassigned and uncommented: the Critical `F-VAL-001` and eleven of the fourteen Highs** — `F-CORE-001`, `F-CORE-002`, `F-CORE-031`, `F-CORE-060`, `F-VAL-030`, `F-VAL-039`, `F-VAL-060`, `F-VAL-061`, `F-SEN-001`, `F2-SEN-010`, `F2-XC-001`.

## 3. Fix these first

Ordered by loss and by dependency. Both runs converged on the same fixes independently ([ledger §6.1](RECONCILIATION.md#61-remediation-convergence)); each item below closes several rows. Read Section 4 before choosing an option.

**Sequencing constraints at a glance.**

- The `F-VAL-001` proof of possession needs a coordinator/`keyGenChallenge` change before the Rust can enforce it; the KDF-bound pad (`F-VAL-002`) is a wire-format change for the TypeScript client — land both in one ceremony version.
- `F-SEN-002`'s fix must close both undercount sources (the engine-resume counter and the #914 phase-switch tally, `F2-SEN-010`); a fix that counts from `NewRequest` onward closes only one.
- `F-SEN-005`'s expiry must be an action, never a drop; it lands with `F2-SEN-003`'s "never drop a bonded entry" or it recreates that finding.
- The synthetic warp `NewBlock` (item 7) needs the validator's `<`/`<=` deadline check first; the sentinel already has it from #914.
- `F-CORE-067`'s idempotency key is retained beyond the row's lifetime, or `F-CORE-063` is fixed first.
- An absolute fee ceiling (`F-CORE-060` option 1) needs a cancellation path (`F2-CORE-064`) or it wedges the queue.
- Fixing `F-CORE-031` does not fix `F-VAL-030` or `F-VAL-061`, and would make `F-VAL-061` harder to see by producing `Resume::Noop`s that read as successes.

1. **`F-VAL-001` and `F-VAL-002` together — owner needed.** A proof of possession for `q` (a Schnorr proof over `(gid, participant, q)`, or `q` folded into the DKG challenge preimage as `F2-VAL-001` proposes) **and** a KDF-bound share pad (`HKDF-SHA256(x(shared), info = gid ‖ sender ‖ recipient)`, `F-VAL-002` option 1). The KDF alone leaves a liveness variant; the proof alone leaves the two-time pad. **Sequencing:** the proof needs a coordinator/`keyGenChallenge` change and is a wire-format change for the TypeScript client; run-2 QA checked it does not break honest late joiners. A duplicate-`q` check is not a substitute (`q_M = k · q_A` defeats it), and `F-VAL-003` option 3 substitutes for neither. Defence in depth in the same change: refuse a complaint response for a plaintiff absent from `public_keys` and bound responses per plaintiff (`F-VAL-003`/`F2-VAL-006` option 1).
2. **Sentinel bond safety, one change set — @rmeissner already holds three of the five.** (a) [`F-SEN-002`](../findings/F-SEN-002.md) **and** [`F2-SEN-010`](../findings/F2-SEN-010.md): counting from `NewRequest` onward closes the first but not the second; keeping the tally live in `CollectingVotes` (`F2-SEN-010` option 1, compiled, 47/47) or **not early-finalizing on local counts** closes both, and only the second also closes run 1's A4 trigger (a `Committed` lost to an incomplete `eth_getLogs`). (b) **Never drop a bonded entry** ([`F2-SEN-003`](../findings/F2-SEN-003.md) option 1, compiled, all existing tests pass) turns every silent loss in the crate into a recoverable park. (c) [`F-SEN-005`](../findings/F-SEN-005.md): store the `DisputeTriggered.deadline` that `bindings.rs:44` already decodes, emit a `TimeoutArbitration { id }` action once at `block > deadline`, keep the entry until `ArbitrationTimedOut`/`DisputeResolved`, and give `WaitingForOutcome` a deadline that re-emits `Finalize` — that deadline is also what un-parks `F2-SEN-010` outcome 2. Fix the `handle_arbitration_timeout` doc and metric in the same change (`service.rs:550-557`, `585`; [`F2-SEN-011`](../findings/F2-SEN-011.md)). (d) [`F-SEN-001`](../findings/F-SEN-001.md) and [`F-SEN-015`](../findings/F-SEN-015.md), same fix: a three-state `getCommitment(id, self)` effect (`Committed(hash)` / `NotCommitted` / `Unavailable`, where `Unavailable` keeps the entry and reveals from stored state), or persist `(request_id, approve, reason)` idempotently in the effect handler when the check resolves and let `handle_committed` record `self_committed` in `WaitingForEngineCheck`. `hashCommitment` is already bound (`bindings.rs:54-60`) and never called.
3. **Core effect durability — [`F-CORE-031`](../findings/F-CORE-031.md), raised to High by run 2's execution.** A durable pending-effect set re-issued after every `Uncle` and after the startup restore (`F2-CORE-030` option 1, a `pending_effects` hook on the restored state), plus a concurrency cap in `EffectManager`. Closes `F-CORE-031`, `F-CORE-033`, `F-SEN-004` branch (ii), `F-SEN-011` and `F-VAL-004`'s rollover instance, and gives `F-VAL-061` its retry path. Run-1 QA's caveat: a queued-but-unspawned effect is lost on shutdown exactly like an in-flight one, so the cap's pending queue must be the same durable structure. **Not** `F-CORE-031` option 1 or 3 (Section 4). Fixing `F-CORE-031` does not fix `F-VAL-030` or `F-VAL-061` — their triggers are deterministic failure, not lost delivery.
4. **Validator effect-failure policy — [`F-VAL-061`](../findings/F-VAL-061.md), [`F-VAL-030`](../findings/F-VAL-030.md), [`F-VAL-032`](../findings/F-VAL-032.md), [`F-VAL-039`](../findings/F-VAL-039.md).** A per-effect failure policy (`Resume::Failed { effect_kind, group_id }` or `Result<Resume, _>`), an explicit `Resume::NonceTreeFailed` that removes a _trailing_ `None` reservation (`F2-VAL-030` option 1, first half), the chunk index carried through `Effect::NonceTree`, and self-healing on `NewBlock` (re-emit `NonceTree` for a stale `None` reservation, re-emit `KeyGenSetup` while `Participating { secrets: None }`). Re-emission is idempotent at the store level (`store_keygen_secrets` is insert-or-return-existing, `store.rs:183-197`), which is what run 1's "retry must wait for `F-VAL-005`" hazard turned on. `F-VAL-032` is two lines: re-insert the session in the `(None, Some(WaitingForRequest { .. }))` arm of `state/sign.rs:106-114`, or `get` before `remove` and require `session.group_id == event.gid`. `F-VAL-039`: keep a linked chunk in reserve and give `Action::Preprocess` an expiry and a priority (`service/action.rs:252-254`).
5. **A synchronous first reconciliation on resume, or await the block's reconciliation before `housekeeping`.** One ordering change in `crates/core/src/driver.rs:278-294` serves three findings: the restart reconcile-vs-prune race ([`F-VAL-066`](../findings/F-VAL-066.md) / `F2-VAL-035`), the cold generators ([`F2-VAL-031`](../findings/F2-VAL-031.md) — pair it with option 2, carry the `KeyShare` in `Effect::NonceTree` and `start` before `next`), and the restart-ordering trigger of `F-VAL-030`/`F-VAL-061`. It is also the remaining `F-VAL-005` residual the team's reply names.
6. **Core reorg and indexing.** [`F-CORE-001`](../findings/F-CORE-001.md): persist the anchor's identity (`block_hash`, plus `chain_id` and a watched-address digest — the `meta` row `F-XC-006`/`F2-XC-006` asks for) and compare it in `BlockWatcher::initialize`; option 2 alone cannot work — the retained window is exactly `max_reorg_depth`. [`F2-CORE-011`](../findings/F2-CORE-011.md): commit the anchor on a fresh start too. [`F-CORE-030`](../findings/F-CORE-030.md): `driver.run().await?` in both `main`s (two lines) and a `/health` that reflects the last processed block. [`F-CORE-002`](../findings/F-CORE-002.md): never drop the completeness check while `use_client_filtering` is set, bloom-check the fallback with the existing `may_contain_log`, separate the transport and integrity budgets — and close the second route, [`F-CORE-012`](../findings/F-CORE-012.md) (bloom equality blind to a repeated `(address, topics)` shape; `check_logs_limit` skipped on the client-filtered path), which run 2 missed. Ship [`F-CORE-004`](../findings/F-CORE-004.md)'s escalation with it, since option 1 turns a silent loss into a visible stall.
7. **A synthetic `Message::NewBlock(to)` at the end of each warp page, emitted before the page's logs** (`crates/core/src/state/mod.rs:173-181`). Closes [`F-SEN-003`](../findings/F-SEN-003.md)'s route, [`F2-VAL-003`](../findings/F2-VAL-003.md) (a warp over an epoch boundary forfeits the epoch) and the warp half of [`F-CORE-033`](../findings/F-CORE-033.md), and is the answer to the team's `F-SEN-003` question. **Sequencing:** the validator needs the same `<`/`<=` deadline check the sentinel got in #914 before this lands.
8. **An identity-keyed idempotency column on `enqueue` — [`F-CORE-067`](../findings/F-CORE-067.md).** Key `(block hash, log index, action kind)` with a `UNIQUE` column and `INSERT OR IGNORE` in `crates/core/src/tx/storage.rs`; retain the key beyond the row's lifetime, or fix [`F-CORE-063`](../findings/F-CORE-063.md) first. Not keyed on calldata (Section 4). Closes `F-VAL-065` and `F-SEN-006` (neither can fix it from inside its crate) and the unpruned `Claim` rows with `expires_at: None` that #914 added.
9. **Fees — [`F-CORE-060`](../findings/F-CORE-060.md) and [`F-CORE-061`](../findings/F-CORE-061.md).** Bound the number of bumps and bump only the lowest in-flight nonce (`F2-CORE-060` option 2), or an absolute or base-fee-relative ceiling — the absolute ceiling is sound **only together with a cancellation path** (`F2-CORE-064`), otherwise the capped row stays in flight and blocks every later nonce. Recognise a first-submission underpriced rejection and raise a zero component explicitly (`max(bumped, previous + 1)`). Validate [`F-CORE-066`](../findings/F-CORE-066.md)'s `tx::Config` at load (`NonZeroUsize`, `blocks_before_resubmit >= 1`, a finite cap in `(0, 100]`).
10. **Configuration and secrets.** [`F2-XC-001`](../findings/F2-XC-001.md) is one line — `toml::de::Error::set_input(None)` in `Config::load` (public in the pinned `toml 1.1.2`, `de/error.rs:78-80`) — then print `Display` and exit non-zero; longer term take the key out of the TOML (`F2-XC-007`). [`F-VAL-063`](../findings/F-VAL-063.md): reject the timing relation at load, worst case `blocks_per_epoch > 4 * key_gen_timeout + signing_timeout`. [`F2-XC-002`](../findings/F2-XC-002.md): `?mode=rwc` in both samples. [`F-XC-009`](../findings/F-XC-009.md): a placeholder that cannot parse, and a loopback RPC in the samples (Section 9.3). [`F-XC-004`](../findings/F-XC-004.md): non-root images, pinned tags, a `rust-toolchain.toml`.
11. **Dependencies and CI — [`F-XC-011`](../findings/F-XC-011.md), [`F-XC-007`](../findings/F-XC-007.md).** No advisory has an in-scope connection path; upgrade as hygiene (`h2 >= 0.4.16`, `rustls >= 0.23.45`, `ruint >= 1.20.0`, `crossbeam-epoch >= 0.9.20`, `event-listener >= 5.4.2` are in-range bumps; `lru` is **not** reachable via `cargo update`, Section 4) and add the `cargo audit`/`cargo deny` gate to `just check`.
12. **Observability.** [`F2-CORE-067`](../findings/F2-CORE-067.md) (the queue exports no metrics and never persists the hash), [`F-CORE-035`](../findings/F-CORE-035.md) (every `tx::Error::Rpc` swallowed forever), and a `success`/`noop`/`failure` result label on `effects_total` (`F-VAL-061` option 5). Every silent failure above stays silent until these land.

## 4. Do not ship these "fixes"

Options proposed in the finding files that a QA or Critic agent of either run judged unsound. A fix that makes things worse is more urgent than a finding.

**The one both runs reject independently: "commit the resume".** [`F-CORE-031`](../findings/F-CORE-031.md) option 1, which [`F-SEN-001`](../findings/F-SEN-001.md) option 3, [`F-SEN-015`](../findings/F-SEN-015.md) option 3 and `F2-SEN-001` option 3 all reach for. Run 1: a commit inside `handle_resume` writes a snapshot at `latest` while the status is `BlockEvents { latest }`, i.e. for a block whose logs have not been applied; a crash in that window resumes at `latest + 1` and **loses that block's logs permanently**. Run 2: the restart anchor is `latest − max_reorg_depth` (`crates/core/src/index/blocks.rs:246`, `255-266`) and `SnapshotStore::reorg` deletes every snapshot above it (`state/storage.rs:130-133`), so a snapshot written at `latest` is discarded anyway for any restart within the next `max_reorg_depth` blocks. `F-CORE-031` option 3 (anchor the rollback at `uncle − 2`) rests on a false premise — actions have no at-least-once contract — and would worsen `F-CORE-067`.

**Sentinel.** Expiring `WaitingForOutcome`/`WaitingForDisputeResolution` by dropping the entry (`F-SEN-005`/`F2-SEN-004` option 3) recreates `F2-SEN-003`'s abandonment, because every terminal handler ignores an untracked id (`service.rs:521-527`, `576-582`, `692-698`, `798-804`) — expire into an action, keep the entry. `F-SEN-015` option 2 as written (a SQLite write inside the pure, non-`async` `apply_transition`) is not implementable; the persistence belongs in the effect handler when the verdict is produced. `F-SEN-013` option 2 and `F-CORE-004` option 2 directly contradict `F-CORE-002` option 1 — same code path, opposite policies. `F2-CORE-005` option 2 (skip an undecodable log) is unsound for protocol contracts: an undecodable `Consensus`/`FROSTCoordinator`/`SentinelOracle` log means an ABI mismatch, and skipping it silently derives a wrong state.

**The `submitted_at IS NULL` misreading, three times.** `F-CORE-067` option 3 and `F-CORE-062` option 3 read `submitted_at IS NULL` as "never submitted"; it also means "rejected as underpriced" (proved in `poc/F-CORE-060`, part 3) — they would delete or release rows sitting in a mempool. The "clear the nonce / delete the row" branch of `F2-CORE-064` option 1 is the same error: a crash between `send_raw_transaction` and `record_submission` (`tx/mod.rs:265-266`) leaves a pooled transaction with `submitted_at NULL`. `F-CORE-060` option 5 (a real `submitted_at` on the underpriced branch plus a `retry_immediately` flag) removes the overload that makes all three unsound.

**Core, the rest.** A `request`-keyed deduplication (`F2-CORE-063` option 1) drops a legitimate repeated `approve` and fails the following `Commit`; the key must be identity — and not `(block number, log index)` alone, since after a real reorg a different event can occupy the same position: include the block hash. `F-CORE-001` option 2 ("walk back") has nothing to walk back to unless snapshot retention is decoupled from `max_reorg_depth` (`F-SEN-011` option 3 is that change). `F-CORE-060` option 2 (re-apply the cap after the bump) cannot bound an absolute fee and creates a second ratchet loop; option 1 without a cancellation path leaves the capped row in flight. `F-CORE-033` option 2 (a `Semaphore` inside each service's handler) is a service-local mitigation, not a core fix — it neither bounds memory nor survives being forgotten.

**Validator.** `F-VAL-003` option 3 does not make `F-VAL-001` impossible — the pad harvest supplies the valid ciphertexts. `F-VAL-004` option 2 (a genesis deadline) reaches `Halted`, worse than the stall, because `restart_key_gen_excluding` refuses for genesis (`state/keygen.rs:1195-1210`). `F-VAL-066` option 4 (a post-write read) races the delete it is meant to catch. The "reorder the commands" halves of `F-VAL-030`, `F-VAL-061` option 4 and `F2-VAL-062` option 2 are ineffective on their own — the driver spawns the commands as independent tasks with no ordering guarantee. `F2-VAL-030` option 1, second half (exclude `None` reservations from `available()` without changing `handle_nonce_topup`): the top-up then runs every block while a request is in flight, each run reserves a _new_ `None` at `last + 1`, and `expected_chunk` runs away from the contract by one chunk per block — the desync the finding describes; if capacity is to ignore reservations, the top-up must reuse the trailing one. `F2-VAL-030` option 3 needs a backoff. `F2-VAL-031` option 1 as written (`generator.retain` from a below-marker reconciliation's older set) stops the newest group's stream — `retain` must stay behind the accepted-schedule check; `start` for every group is safe. `F2-VAL-061` option 3 in its `eth_call` form breaks the pure, non-failing transition contract. `F-VAL-005` option 4 (key the row by commitment hash) makes a resample invisible, not impossible — moot for the fixed trigger, still wrong. `F-VAL-033` is out of scope; if anyone picks it up, option 2 (a per-group high-water mark) rejects legitimate lower offsets and reintroduces `F-VAL-030`'s harm, and the team's objection to option 1 (a restore drops the consumed-nonce row as easily as the chunk row) is correct.

**Cross-cutting.** `F-XC-001` option 3 would put the epoch-rollover path behind a validator crash; an `if`-guarded `error!` is the right shape. `F-XC-009` option 2 must drop the refuted `0.0.0.0` item and keep only its startup-`warn!` clause. `F-XC-003` option 1 is a detector, not a fix — the fix is `deny_unknown_fields` on `core::driver::Config`. `F-XC-007` item 2 (unused SQL drivers as attack surface) is refuted, not merely mis-emphasised: 0 symbols in every release binary. `F2-XC-004` option 1 (`cargo update -p …`) is right for five of the six advisories and **wrong for `lru`**: the patched release is `>= 0.18.2` while `alloy-provider 2.0.5` pins `lru = "0.16"`, so it needs an `alloy` release or a reviewed ignore entry.

## 5. All findings

One row per live canonical defect, 95 rows. **Runs:** `1` run 1 only, `2` run 2 only, `1+2` both, with the run-2 counterpart in parentheses. **Basis:** strongest class across both runs (`E1` executed, `E2` code-traced). Status vocabulary is by certainty band (Confirmed ≥ 70, Plausible 40–69, Observation < 40). "Where did `F2-…` go" is answered by the [fold index](RECONCILIATION.md#22-fold-index--every-run-2-file-that-is-not-itself-canonical).

| Crate | Canonical | Runs | Short title | Severity | Cert. | Basis | Status | Team disposition |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| core | [`F-CORE-001`](../findings/F-CORE-001.md) | 1+2 (`F2-CORE-001`) | Reorg-depth protection not persisted; restart resumes from orphaned state | High | 99 | E1 | Confirmed | #820 closed-but-present; unassigned |
| core | [`F-CORE-002`](../findings/F-CORE-002.md) | 1+2 (`F2-CORE-002`) | Client-filtering completeness check disables itself after three failures | High | 99 | E1 | Confirmed | new; unassigned |
| core | [`F-CORE-003`](../findings/F-CORE-003.md) | 1+2 (`F2-CORE-006`) | `null` header treated as an uncle | Medium | 70 | E2 | Confirmed | — |
| core | [`F-CORE-004`](../findings/F-CORE-004.md) | 1+2 (`F2-CORE-005`) | Event-fetch failures retry forever; block watcher starved, reorg detection stops | Medium | 90 | E1 | Confirmed | #820 class |
| core | [`F-CORE-005`](../findings/F-CORE-005.md) | 1+2 (`F2-CORE-008`) | `max_reorg_depth = 0`: `recent` always empty, `-32001` path spins silently | Low | 75 | E2 | Confirmed | #820 closed-but-present |
| core | [`F-CORE-006`](../findings/F-CORE-006.md) | 1+2 (`F2-CORE-004`) | Filtering and decoding are address-agnostic (core half of `F-VAL-060`) | Low | 70 | E2 | Confirmed | — |
| core | [`F-CORE-007`](../findings/F-CORE-007.md) | 1+2 (`F2-CORE-009`) | Initialization range scan restarts without bound or delay | Low | 70 | E2 | Confirmed | — |
| core | [`F-CORE-008`](../findings/F-CORE-008.md) | 1 | Block polling scheduled against the host wall clock | Low | 70 | E2 | Confirmed | — |
| core | [`F-CORE-009`](../findings/F-CORE-009.md) | 1 | Block-watcher config unvalidated (`block_time = 0`, unbounded depth, `start_block > head`) | Low | 78 | E2 | Confirmed | — |
| core | [`F-CORE-010`](../findings/F-CORE-010.md) | 1 | `-32001` recovery commits the rewind before the event watcher validates it | Low | 45 | E2 | Plausible | — |
| core | [`F-CORE-011`](../findings/F-CORE-011.md) | 1+2 (`F2-CORE-033`) | Indexer hangs behind a live-but-silent peer; nothing signals it | Low | 80 | E2 | Confirmed (narrowed) | — |
| core | [`F-CORE-012`](../findings/F-CORE-012.md) | 1 | Bloom equality blind to the loss of a repeated-shape log; `check_logs_limit` skipped on the client-filtered path | Medium | 70 | E2 | Confirmed | — |
| core | [`F-CORE-030`](../findings/F-CORE-030.md) | 1+2 (`F2-CORE-031`, `F2-VAL-066`) | Fatal driver errors exit with status 0; `/health` is a constant `OK` | Medium | 85 | E2 | Confirmed | #820 closed-but-present |
| core | [`F-CORE-031`](../findings/F-CORE-031.md) | 1+2 (`F2-CORE-030`) | Snapshot commits before the effect spawns; rollback and replay discard resumes | High | 92 | E1 | Confirmed | #614/#799 partial; unassigned |
| core | [`F-CORE-032`](../findings/F-CORE-032.md) | 1 | A panicking effect task is logged and skipped; the resume is gone | Low | 45 | E2 | Plausible | #799/#614 partial |
| core | [`F-CORE-033`](../findings/F-CORE-033.md) | 1+2 (`F2-CORE-035`) | Effect fan-out unbounded; no cap, queue or backpressure | Medium | 70 | E2 | Confirmed | question answered; #614 partial |
| core | [`F-CORE-034`](../findings/F-CORE-034.md) | 1+2 (`F2-CORE-010`) | Retry storm without backoff; deterministic errors never escalate | Medium | 80 | E2 | Confirmed | — |
| core | [`F-CORE-035`](../findings/F-CORE-035.md) | 1 | Every `tx::Error::Rpc` is "intermittent" and swallowed forever | Medium | 78 | E2 | Confirmed | — |
| core | [`F-CORE-036`](../findings/F-CORE-036.md) | 1 | `Debug` bound on `Effect`/`Resume`; five trace sinks (secret leg refuted) | Low | 85 | E1 | Verified | #113 understated |
| core | [`F-CORE-037`](../findings/F-CORE-037.md) | 1 | Snapshots are unversioned JSON | Low | 62 | E2 | Plausible | — |
| core | [`F-CORE-038`](../findings/F-CORE-038.md) | 1 | `kdf::derive_key` multi-part `info` is a plain concatenation | Informational | 85 | E2 | Confirmed | — |
| core | [`F-CORE-039`](../findings/F-CORE-039.md) | 1+2 (`F2-CORE-034`) | Graceful shutdown blocked inside `update`; inline `housekeeping` is a second unbounded await | Low | 72 | E2 | Confirmed | — |
| core | [`F-CORE-040`](../findings/F-CORE-040.md) | 1 | `next_input` `select!` drops the watcher's in-flight RPC on every resume | Low | 65 | E2 | Plausible | #614 partial |
| core | [`F-CORE-060`](../findings/F-CORE-060.md) | 1+2 (`F2-CORE-060`) | Fee ratchet unbounded; `priority_fee_cap_percentage` never bounds against the base fee | High | 98 | E1 | Confirmed | #656 closed-but-present; unassigned |
| core | [`F-CORE-061`](../findings/F-CORE-061.md) | 1+2 (`F2-CORE-062`) | First-submission "underpriced" rejection retried forever at the same fee | Medium | 65 | E1 | Plausible | #656 (introduced by the fix) |
| core | [`F-CORE-062`](../findings/F-CORE-062.md) | 1 | Allocated nonce never released; one bad nonce wedges the queue | Medium | 60 | E2 | Plausible | — |
| core | [`F-CORE-063`](../findings/F-CORE-063.md) | 1+2 (`F2-CORE-061`) | Execution inferred from the account nonce; mark irrevocable | Medium | 55 | E1 | Plausible | — |
| core | [`F-CORE-064`](../findings/F-CORE-064.md) | 1+2 (`F2-CORE-064`) | `expires_at` void once a nonce is allocated | Medium | 90 | E1 | Confirmed | — |
| core | [`F-CORE-065`](../findings/F-CORE-065.md) | 1+2 (`F2-CORE-066`) | Transactions table bound to neither chain id nor signer | Low | 70 | E2 | Confirmed, `known` | `known` (A12); A17 drops trigger 1 |
| core | [`F-CORE-066`](../findings/F-CORE-066.md) | 1+2 (`F2-CORE-065`) | `tx::Config` accepts degenerate values (zero in-flight, zero window, `nan`) | Medium | 90 | E1 | Confirmed | — |
| core | [`F-CORE-067`](../findings/F-CORE-067.md) | 1+2 (`F2-CORE-063`, `F2-CORE-032`) | No idempotency key on `enqueue`; replay re-submits actions | Medium | 98 | E1 | Confirmed | question answered; `known` tag retained |
| core | [`F2-CORE-003`](../findings/F2-CORE-003.md) | 2 | Default-path log fetch has no completeness check; dead `may_contain_log` | Informational | 70 | E2 | Confirmed | — |
| core | [`F2-CORE-007`](../findings/F2-CORE-007.md) | 2 | Restart against a lagging node exits with an opaque `BadUpdate` | Low | 75 | E2 | Confirmed | — |
| core | [`F2-CORE-011`](../findings/F2-CORE-011.md) | 2 | No rollback anchor persisted on a fresh start; in-window reorg exits `MissingSnapshot` | Low | 92 | E1 | Confirmed | — |
| core | [`F2-CORE-067`](../findings/F2-CORE-067.md) | 2 | Transaction queue exports no metrics; hash never persisted | Informational | 85 | E2 | Confirmed | — |
| validator | [`F-VAL-001`](../findings/F-VAL-001.md) | 1+2 (`F2-VAL-001`) | DKG encryption key `q` has no proof of possession; peer's signing share recoverable | **Critical** | 97 | E1 | Confirmed | none yet |
| validator | [`F-VAL-002`](../findings/F-VAL-002.md) | 1+2 (`F2-VAL-002`) | ECDH share pad is a raw symmetric x-coordinate | Medium | 93 | E1 | Confirmed | — |
| validator | [`F-VAL-003`](../findings/F-VAL-003.md) | 1+2 (`F2-VAL-006`) | Complaint responses unconditional and unbounded per plaintiff | Medium | 82 | E1 | Confirmed | #69 partial |
| validator | [`F-VAL-004`](../findings/F-VAL-004.md) | 1+2 (`F2-VAL-063`, `F2-VAL-005`) | Lost or failed `KeyGenSetup` never re-issued — rollover instance (genesis: Informational `known`) | Medium | 69 | E1 gap, E2 trigger | Plausible | accepted for genesis (A16) |
| validator | [`F-VAL-030`](../findings/F-VAL-030.md) | 1+2 (`F2-VAL-030`, `F2-VAL-062`) | Failed `NonceTree` leaves a phantom reservation; `expected_chunk` cascade | High | 97 | E1 | Confirmed | #799/#666 partial; unassigned |
| validator | [`F-VAL-031`](../findings/F-VAL-031.md) | 1 | Dead nonce-generation worker never detected or restarted | Low | 42 | E2 | Plausible | — |
| validator | [`F-VAL-032`](../findings/F-VAL-032.md) | 1+2 (`F2-VAL-032`) | `handle_sign` drops a `WaitingForRequest` session on a `Sign` from any untracked group | High | 93 | E1 | Confirmed | accepted |
| validator | [`F-VAL-034`](../findings/F-VAL-034.md) | 1 | `handle_nonces` applies a resume without checking the signature id | Low | 55 | E1 | Plausible (outcome benign) | — |
| validator | [`F-VAL-035`](../findings/F-VAL-035.md) | 1 | Unzeroised nonce JSON; abandoned chunks of retained groups never reclaimed | Low | 35 | E2 | Observation | #666 partial |
| validator | [`F-VAL-036`](../findings/F-VAL-036.md) | 1 | `observe` accepts a non-monotonic sequence and rewinds | Low | 40 | E2 | Plausible | — |
| validator | [`F-VAL-037`](../findings/F-VAL-037.md) | 1 | Merkle trees pad with `B256::ZERO`; no leaf/internal separation | Informational | 60 | E2 | Plausible | — |
| validator | [`F-VAL-038`](../findings/F-VAL-038.md) | 1 | Chunk generation saturates every core; 1025 statements hold the SQLite writer | Low | 55 | E2 | Plausible | flow-test epic partial |
| validator | [`F-VAL-039`](../findings/F-VAL-039.md) | 1 | Top-up threshold gives ~100 sequences of headroom against a permissionless counter | High | 58 | E2 | Plausible | new; unassigned |
| validator | [`F-VAL-040`](../findings/F-VAL-040.md) | 1+2 (`F2-VAL-033`) | A signer re-reveals to become `last_signer`, then does nothing | Low | 88 | E1 | Confirmed | #777 partial |
| validator | [`F-VAL-060`](../findings/F-VAL-060.md) | 1+2 (`F2-VAL-061`) | Coordinator/Consensus events accepted from any watched address | High (conditional on a malicious or compromised allow-listed oracle) | 90 | E1 | Confirmed | unassigned |
| validator | [`F-VAL-061`](../findings/F-VAL-061.md) | 1+2 (`F2-VAL-030/031/062/063`) | Every effect error maps to `Resume::Noop`; no retry, no marker | High | 98 | E1 | Confirmed | #799 partial; unassigned |
| validator | [`F-VAL-062`](../findings/F-VAL-062.md) | 1 | Secret-bearing effects and resumes derive `Debug`, printed at `warn` (leak refuted, hygiene stands) | Informational | 88 | E1 | Confirmed | #113 understated |
| validator | [`F-VAL-063`](../findings/F-VAL-063.md) | 1+2 (`F2-VAL-067`) | Consensus-critical config unvalidated; rollover livelock when `blocks_per_epoch <= key_gen_timeout` | Medium | 72 | E2 | Confirmed | — |
| validator | [`F-VAL-064`](../findings/F-VAL-064.md) | 1+2 (`F2-VAL-066`, `F2-VAL-068`) | Validator deployment residual: `/health` unreachable in the shipped deployment | Low | 72 | E2 | Confirmed (split) | #820 partial |
| validator | [`F-VAL-065`](../findings/F-VAL-065.md) | 1 | `SetValidatorStaker` accumulates across restarts; `Preprocess` never expires; a replayed `Sign` burns a sequence for the whole group | Low | 70 | E2 | Confirmed | — |
| validator | [`F-VAL-066`](../findings/F-VAL-066.md) | 1+2 (`F2-VAL-035`) | Retention set computed before the block's logs; reconcile-vs-prune race across a restart | Medium | 78 | E1 | Confirmed (changed shape) | #666/#801 partial |
| validator | [`F-VAL-067`](../findings/F-VAL-067.md) | 1+2 (`F2-VAL-006` para. 2) | Rust counts complaints cumulatively, the contract nets them | Low | 70 | E2 | Confirmed | #118/#69 partial |
| validator | [`F2-VAL-003`](../findings/F2-VAL-003.md) | 2 | Replay warp over an epoch boundary drops the key-gen round; epoch forfeited | Medium | 85 | E1 | Confirmed | — |
| validator | [`F2-VAL-004`](../findings/F2-VAL-004.md) | 2 | Late-setup branch derives a divergent share-round deadline | Low | 50 | E2 | Plausible | — |
| validator | [`F2-VAL-007`](../findings/F2-VAL-007.md) | 2 | `active_epoch` only advances through self-staged epochs; phantom rollover session | Low | 65 | E2 | Plausible | — |
| validator | [`F2-VAL-031`](../findings/F2-VAL-031.md) | 2 | Nonce generators cold after every restart; below-marker return precedes `generator.start` | Medium | 80 | E1 | Confirmed | introduced by #909/#910; predicted as `F-VAL-068` D1 |
| validator | [`F2-VAL-034`](../findings/F2-VAL-034.md) | 2 | No schema-version check; a pre-#908 database opens then fails every reconciliation | Informational | 85 | E1 | Confirmed, `known` | #913 |
| validator | [`F2-VAL-064`](../findings/F2-VAL-064.md) | 2 | Documented `--config-file=<path>` spelling rejected by `argh` (both binaries) | Informational | 95 | E1 | Confirmed | — |
| sentinel | [`F-SEN-001`](../findings/F-SEN-001.md) | 1+2 (`F2-SEN-001` a, b) | Replay or rollback discards own `Committed`; no reveal; bond slashed | High | 99 | E1 | Confirmed | none yet; #614 closed-but-present |
| sentinel | [`F-SEN-002`](../findings/F-SEN-002.md) | 1+2 (`F2-SEN-002`) | Commits before the verdict not tallied; early finalize; bond never claimed or `Finalize` reverts and parks | High | 98 | E1 | Confirmed | assigned @rmeissner |
| sentinel | [`F-SEN-003`](../findings/F-SEN-003.md) | 1+2 (`F2-SEN-003` b) | Warp delivers no `NewBlock`; reveals discarded; bogus `timed_out` finalize | Low | 80 | E1 | Confirmed (residual) | question answered; #667 partial |
| sentinel | [`F-SEN-004`](../findings/F-SEN-004.md) | 1+2 (`F2-SEN-005`) | No bound on concurrent engine checks or outstanding bonds; flood → abstention or expired reveals | Medium | 78 | E1 | Confirmed (branch ii) | #614 partial |
| sentinel | [`F-SEN-005`](../findings/F-SEN-005.md) | 1+2 (`F2-SEN-004`) | Waiting states never expire; arbitration deadline discarded; no `timeoutArbitration` | Medium | 95 | E1 | Confirmed | assigned @rmeissner (easy fix) |
| sentinel | [`F-SEN-006`](../findings/F-SEN-006.md) | 1+2 (`F2-SEN-007`) | Actions not idempotent under replay; `handle_arbitration_timeout` claims without a bond | Low | 90 | E1 | Confirmed | — |
| sentinel | [`F-SEN-007`](../findings/F-SEN-007.md) | 1+2 (`F2-SEN-006`) | No startup or per-request pre-flight; silent per-request gas burn | Low | 82 | E1 | Confirmed | — |
| sentinel | [`F-SEN-008`](../findings/F-SEN-008.md) | 1 | Hard-coded gas limits; unconditional non-zero `approve`; non-plain fee token | Low | 52 | E2 | Plausible | — |
| sentinel | [`F-SEN-009`](../findings/F-SEN-009.md) | 1+2 (`F2-SEN-006` claim 6, `F2-SEN-008` claim 3) | Engine timeout derived from unvalidated `voting_window` | Low | 82 | E1 | Confirmed, `known` | `main.rs` TODO + #799 |
| sentinel | [`F-SEN-010`](../findings/F-SEN-010.md) | 1+2 (`F2-SEN-009` b1) | Sample ships zero addresses and a placeholder key that parse and start | Informational | 85 | E2 | Confirmed, `known` | `config.rs` TODO |
| sentinel | [`F-SEN-011`](../findings/F-SEN-011.md) | 1 | Restart orphans an in-flight check whose proposal predates the anchor | Low | 82 | E2 | Confirmed | #614 partial |
| sentinel | [`F-SEN-012`](../findings/F-SEN-012.md) | 1+2 (`F2-SEN-008`) | Engine client: single attempt; any failure is a permanent abstention | Low | 90 | E1 | Confirmed | — |
| sentinel | [`F-SEN-014`](../findings/F-SEN-014.md) | 1 | Every participant submits `finalize`; `K − 1` revert | Informational | 88 | E2 | Confirmed | — |
| sentinel | [`F-SEN-015`](../findings/F-SEN-015.md) | 1+2 (`F2-SEN-001` c) | A replayed engine check re-decides a committed vote; `InvalidReveal` or untracked; slashed | High | 98 | E1 | Confirmed | assigned @rmeissner |
| sentinel | [`F2-SEN-003`](../findings/F2-SEN-003.md) | 2 | `finalize()` drops a bonded entry whenever our own reveal was not observed | Medium | 90 | E1 | Confirmed | pairs with the `F-SEN-002`/`F-SEN-005` fixes |
| sentinel | [`F2-SEN-010`](../findings/F2-SEN-010.md) | 2 | Commits mined in the commit-deadline block are never tallied (#914) | High | 93 | E1 | Confirmed | route to `F-SEN-002`'s assignee |
| sentinel | [`F2-SEN-011`](../findings/F2-SEN-011.md) | 2 | `handle_arbitration_timeout` doc and metric assume a full refund | Informational | 80 | E2 | Confirmed | fold into the `F-SEN-005` fix |
| cross-cutting | [`F-XC-001`](../findings/F-XC-001.md) | 1+2 (`F2-XC-009`) | No release profile; overflow checks off in shipped binaries | Informational | 93 | E1 | Confirmed | — |
| cross-cutting | [`F-XC-003`](../findings/F-XC-003.md) | 1 | `deny_unknown_fields` + `flatten`: behaviour correct, no in-tree test | Informational | 96 | E1 | Verified (test gap) | — |
| cross-cutting | [`F-XC-004`](../findings/F-XC-004.md) | 1+2 (`F2-XC-003`) | Runtime images run as root on floating tags; no toolchain pin | Low | 85 | E2 | Confirmed | — |
| cross-cutting | [`F-XC-006`](../findings/F-XC-006.md) | 1+2 (`F2-XC-006`) | Persistent state not bound to chain, deployment or signer | Low | 90 | E1 | Confirmed | — |
| cross-cutting | [`F-XC-007`](../findings/F-XC-007.md) | 1+2 (`F2-XC-004`) | No `cargo audit` gate in CI; feature width | Informational | 92 | E1 | Confirmed | — |
| cross-cutting | [`F-XC-008`](../findings/F-XC-008.md) | 1 | Unconfigured `reqwest` clients follow redirects and honour proxy env | Informational | 80 | E2 | Confirmed (narrowed) | — |
| cross-cutting | [`F-XC-009`](../findings/F-XC-009.md) | 1+2 (`F2-XC-008`) | Samples ship a parseable well-known key and a live mainnet RPC | Low | 75 | E2 | Confirmed | — |
| cross-cutting | [`F-XC-011`](../findings/F-XC-011.md) | 1+2 (`F2-XC-004`) | Dependency advisories: linked, no in-scope connection path | Informational | 90 | E1 | Confirmed | — |
| cross-cutting | [`F-XC-050`](../findings/F-XC-050.md) | 1 | No DKG membership check; pure-cardinality close — rollover instance (genesis: Informational `known`) | Medium | 48 | E2 | Plausible | — |
| cross-cutting | [`F-XC-051`](../findings/F-XC-051.md) | 1 | `verify_commitment` delegates structural checks to `frost-core` | Informational | 85 | E2 | Settled in the code's favour | — |
| cross-cutting | [`F2-XC-001`](../findings/F2-XC-001.md) | 2 (`F2-VAL-060`) | Any configuration parse error prints the whole file, signer key included | High | 95 | E1 | Confirmed | unassigned |
| cross-cutting | [`F2-XC-002`](../findings/F2-XC-002.md) | 2 (`F2-VAL-065`) | Sample `database` URL lacks `?mode=rwc`; documented first start fails | Low | 92 | E1 | Confirmed | — |
| cross-cutting | [`F2-XC-005`](../findings/F2-XC-005.md) | 2 | Secret deletion is logical only: bundled SQLite without `SECURE_DELETE` | Informational | 90 | E1 | Confirmed | — |
| cross-cutting | [`F2-XC-007`](../findings/F2-XC-007.md) | 2 | Signer key lingers in un-zeroized configuration buffers | Informational | 75 | E2 | Confirmed | — |
| cross-cutting | [`F2-XC-050`](../findings/F2-XC-050.md) | 2 (`F2-CORE-036`) | Snapshot committed and pruned before the block's actions are enqueued; crash window | Low | 92 | E1 | Confirmed | — |

Per crate: core 35 (High 4, Medium 13, Low 15, Informational 3), validator 28 (Critical 1, High 5, Medium 7, Low 11 including the observation, Informational 4), sentinel 17 (High 4, Medium 3, Low 7, Informational 3), cross-cutting 15 (High 1, Medium 1, Low 5, Informational 8).

**Rows not counted as live:**

| ID | Runs | Status | Why |
| --- | --- | --- | --- |
| [`F-VAL-005`](../findings/F-VAL-005.md) | 1+2 (`F2-VAL-035`, `F2-VAL-031`) | **Fixed by the merge** (#909/#910) for the reproduced trigger | residuals: beyond-depth absence (`known`), restart race under `F-VAL-066` |
| [`F-VAL-033`](../findings/F-VAL-033.md) | 1 | **Out of scope (A17)** | mechanism real (`store.rs:281-291`); trigger is an operator restore |
| [`F-VAL-068`](../findings/F-VAL-068.md) | 1 | **Superseded** | branch merged; D1–D4 split as in Section 2.3 |
| [`F-SEN-013`](../findings/F-SEN-013.md) | 1 | **Refuted** (98) | `alloy-sol-types` decodes invalid UTF-8 lossily; unchanged at `fe9e84c` |
| [`F-SEN-016`](../findings/F-SEN-016.md) | 1 | D1 superseded by `F2-SEN-010`; D2–D4 forward-looking | #915 unmerged |
| [`F-CORE-068`](../findings/F-CORE-068.md), [`F-CORE-069`](../findings/F-CORE-069.md) | 1 | Forward-looking (High / Medium, 85 / 80, static) | batex #904 unmerged; not in the tree |
| [`F-XC-002`](../findings/F-XC-002.md) | 1 | Union row, not counted | its halves are `F-VAL-062` and `F-CORE-036` |

## 6. Findings by severity

Trail format: run 1's number and band · run 2's number and band · what settled it. The dissenting band is named wherever the runs differed.

### Critical (1)

#### [`F-VAL-001`](../findings/F-VAL-001.md) — Critical, 97 · DKG encryption key `q` has no proof of possession

`crates/validator/src/frost/keygen.rs:79-100`, `frost/ecdh.rs:110-121`. Nothing binds `q` to its publisher: the Rust checks only that it decodes to a non-identity point, the coordinator only `q != 0` (`FROSTCoordinator.sol:377`), and the proof of knowledge covers the polynomial commitment `c`, not `q`. The share pad is the plain, unhashed ECDH x-coordinate and therefore symmetric, so a registered participant `M` publishing `q_M := q_A` makes every peer's pad to `M` identical to its pad to `A`. `M` harvests those pads at the price of one complaint each — filed against the other `n − 1` participants _before_ publishing its own share, so no honest peer ever complains about `M`, and every accused sits at `total == 1` — then publishes a fully valid share, confirms (`FROSTParticipantMap.sol:219-222` requires only the confirmer's own `complaints == 0`), decrypts both directions of every pair and interpolates `A`'s polynomial. Result: `s_A`, `A`'s complete signing share, in a group that finalised normally with `A` still an active member. With `m` colluders copying `m` different peers the coalition holds `2m` shares; at `n = 7, m = 2` that is `4 = threshold`, inside the `m < n/3` bound the system claims to tolerate. A duplicate-`q` check is not a fix (`q_M = k · q_A`).

**Executed, both runs.** Run 1: crypto in-process, then the full onchain sequence against the real bytecode — duplicate-`q` commit, `n − 1` complaints from a plaintiff never marked `COMPROMISED`, the impostor's own `keyGenConfirm`, the group finalising with the impostor holding a slot — 5/5 fresh seeds, re-validated post-merge. Run 2, independently and without run 1's material: `poc/F2-VAL-001` recovers A's full share from public ciphertexts and complaint reveals, `s_a_recovered == s_a_ref` (85 → 95).

**Fix:** Section 3 item 1 — proof of possession **and** KDF-bound pad; needs a coordinator/`keyGenChallenge` change; does not break honest late joiners. **Trail:** run 1 E1 97 Critical · run 2 E1 95 Critical · agreement.

### High (14)

#### [`F-CORE-001`](../findings/F-CORE-001.md) — High, 99 · A reorg during downtime silently defeats `max_reorg_depth`

`crates/core/src/index/blocks.rs:244-289`; `state/storage.rs:51-57`. `snapshots` stores `(block_number, state)` and no hash; `BlockWatcher::initialize` re-anchors on whatever the RPC calls `latest`. While running, a reorg replacing the `safe` anchor is fatal (`ExceededMaxReorgDepth`); across a restart the same reorg produces no error and the oldest retained snapshot is accepted as the anchor whether or not its block is canonical. Self-reinforcing with `restart: always`. **Run 1, A/B on one chain, depth-11 reorg:** running → error and exit; across a restart → alive, 0 WARN / 0 ERROR in 2,731 lines, replaying canonical logs onto orphaned state. **Run 2** (`poc/F2-XC-050/coverage-7.2`): a depth-4 reorg replacing the persisted `safe` anchor (19) → start proceeds with `Uncle { 20 }, Warp { 20..=21 }` and no `ExceededMaxReorgDepth`. **Fix:** persist the anchor's identity; option 2 alone cannot work. **Trail:** run 1 E1 99 High · run 2 E1 90 Medium (dissent recorded) · run 1's band carried on the live-stack evidence; A17 narrows only the restored-backup aside.

#### [`F-CORE-002`](../findings/F-CORE-002.md) — High, 99 · The log-completeness check disables itself on the failures it exists to detect

`crates/core/src/index/events.rs:362-398`. The bloom-equality check runs only while `retries < block_single_query_retry_count` (default 3); every failure — including the `IncompleteLogs` error the check itself raises, and any HTTP 429 — increments the counter, after which the block falls back permanently to node-filtered per-topic queries with no completeness check. **Run 1, measured:** three 429s then one empty `eth_getLogs` accepted silently, sentinel −4,000 with 2,000 slashed; the control with the budget intact rejected the same answer and finished +500. **Run 2** executed the mechanism independently. The second route to the same outcome, [`F-CORE-012`](../findings/F-CORE-012.md), run 2 missed. **Fix:** Section 3 item 6. **Trail:** run 1 E1 99 High · run 2 E1 90 Medium (its Critic: degraded RPC is A4; dissent recorded) · High carried on the on-chain loss.

#### [`F-CORE-031`](../findings/F-CORE-031.md) — High, 92 · Snapshot commits before the effect spawns; rollback and replay discard resumes

`crates/core/src/state/mod.rs:246-258`, `182-189`; `driver.rs:255-274`. The snapshot recording "effect E is pending" is committed inside `handle_update`, before the driver spawns E; a resume never commits; so for any rollback anchor between E's block and the next commit the state reverts to "pending" while E has run and is never re-spawned — every reorg, and every restart's synthetic `Uncle { safe + 1 }`. Run 1's Critic had noted `F-SEN-001`'s replay case as "mutually exclusive" with this loss; **run 2 executed the opposite** (`poc/F2-CORE-030`: when the emitting block is replayed, the replayed own `Committed` is applied synchronously before the re-spawned effect can resume — "replayed block 3 committed … resumes: []"), deterministic across a ≥ 25 s restart with the real `BlockWatcher`. Root cause of `F-SEN-001` and `F-SEN-011`. **Fix:** Section 3 item 3; not option 1 or 3. **Trail:** run 1 E2 78 Medium · run 2 E1 92 High · **raised** by execution.

#### [`F-CORE-060`](../findings/F-CORE-060.md) — High, 98 · Fee ratchet unbounded; the cap never bounds against the base fee

`crates/core/src/tx/fees.rs:52-56`; `tx/storage.rs:293-298`. A transaction the node keeps rejecting as an underpriced replacement has both fee fields multiplied by 1.1 **every block** (an underpriced rejection writes `submitted_at = NULL`, which `stale_submissions` treats as unconditionally stale), with no ceiling; the only brake is the signer's balance. **Run 1, measured live:** tip 1 → 11,527 → 201,207 wei, max fee 4,239 gwei against a base fee of 772 wei. **Run 2** added the accepted arm (bump every `blocks_before_resubmit`, ×311.9 after 60 bumps, `poc/F2-CORE-060`) and refuted the sentence "cap bypassed ~28,700×" as written; the reconciliation settled both as right about different quantities (Section 7.1). It does not self-start on a healthy node: it needs a stale fee floor (a foreign transaction at the nonce, or a stricter node; the restored-backup starter is out under A17). **Fix:** Section 3 item 9. **Trail:** run 1 E1 98 High · run 2 E1 92 Medium (dissent recorded) · High carried.

#### [`F-VAL-030`](../findings/F-VAL-030.md) — High, 97 · A failed `NonceTree` leaves a phantom reservation counted as capacity

`crates/validator/src/state/preprocess.rs:85-103`, `234-247`. The reservation is durable, the effect that fills it is not and is never re-issued; `available` counts the `None` reservation as 1,024 nonces, so the top-up never fires again and `observe` returns `None` for every sequence in the chunk — silent exclusion from up to 1,024 ceremonies, and via `F-VAL-032` a dropped session. **Run 1:** the stranded reservation reproduced live and unforced after a reorg, inside a passing suite. **Run 2** (`poc/F2-VAL-030`): offsets 0..924 of the next chunk dropped and `expected_chunk` one ahead of the contract for every following chunk — about 90 % of later ceremonies. **Fix:** Section 3 item 4; not the second half of `F2-VAL-030` option 1. **Trail:** run 1 E1 97 High · run 2 E1 90, its Critic Medium (single validator, self-inflicted) · High carried on the reorg trigger.

#### [`F-VAL-032`](../findings/F-VAL-032.md) — High, 93 · `handle_sign` drops a session on a `Sign` from any untracked group — **accepted**

`crates/validator/src/state/sign.rs:106-114`. The session is removed before the nonce is known to exist; the `(None, Some(WaitingForRequest { .. }))` arm logs and never re-inserts; a rollover packet has no re-proposal path. **Run 2 removed the precondition:** an attacker-created 2-of-2 group plus one `sign(G_att, m)` in the deterministic post-timeout block drops every honest validator's session (`poc/F2-VAL-032`, executed); the finding no longer depends on `F-VAL-030` or `F-VAL-039`. **Fix:** two lines (Section 3 item 4). **Trail:** run 1 E1 93 · run 2 E1 92 · High unconditional.

#### [`F-VAL-039`](../findings/F-VAL-039.md) — High, 58, Plausible · ~100 sequences of headroom against a permissionless counter

`crates/validator/src/state/preprocess.rs:15-17` (`NONCE_TOPUP_THRESHOLD = 100`), `:91`; `FROSTCoordinator.sol:530-542` (`sign` is permissionless and increments the group sequence). The attacker does not exhaust a chunk; they keep the group's sequence ahead of the validators' linked chunk for one `preprocess` round trip, and `Action::Preprocess` is queued with no expiry or priority (`service/action.rs:252-254`). Drafted by run 1's Critic; both runs' R5 declined the drain framing as cost-of-attack; **not executed by either run** — the only Critical/High row without `E1`. **Fix:** keep a linked chunk in reserve. **Trail:** run 1 58 · run 2 miss (`F2-VAL-032` names the endpoint) · needs an executed cost model (Section 8.2).

#### [`F-VAL-060`](../findings/F-VAL-060.md) — High, 90, conditional · Coordinator/Consensus events accepted from any watched address

`crates/validator/src/state/mod.rs:415-462`. `apply_transition` routes every `Coordinator::*`/`Consensus::*` event on `topic0` alone; `log.address` reaches only `handle_oracle_result`; `state::Transition` is not even given the coordinator address. Any address in `config.validator.oracles` therefore has full write access to the state machine: `threshold` injected `KeyGenComplained` logs restart a ceremony or halt genesis, `Sign` burns sequences, a self-accusation queues a plaintext `KeyGenComplaintResponse` to the mempool. **Run 2** (`poc/F2-VAL-061`): an identical `Sign` from the coordinator address and from an oracle address produce identical state and commands; three scenarios. Conditional on a malicious or compromised allow-listed oracle. **Fix:** carry `coordinator`/`consensus` in `Transition` and gate the `match` on `log.address`; not the `eth_call` form. **Trail:** run 1 E2 50 Medium (precondition) · run 2 E1 90 High · **raised**, conditional stated.

#### [`F-VAL-061`](../findings/F-VAL-061.md) — High, 98 · Every effect error becomes `Resume::Noop`

`crates/validator/src/service/effect.rs:243-256`. One failure policy — forget it happened — safe only for effects whose state is written after the resume; `NonceTree` and `KeyGenSetup` write a placeholder first, and nothing re-issues them. **Run 1, live and unforced inside a passing suite:** `failed to perform effect NonceTree … "nonce generator is unavailable"` → `Resume::Noop`, zero later spawns. **Run 2** confirmed the policy in four files and widened the restart window (`F2-VAL-031`: the generators are cold after every restart). **Fix:** Section 3 items 4 and 5. **Trail:** run 1 E1 98 High · run 2 E1 (policy leg) · agreement.

#### [`F-SEN-001`](../findings/F-SEN-001.md) — High, 99 · Replay or rollback discards our own `Committed`; no reveal; bond slashed — **unassigned**

`crates/sentinel/src/service.rs:307-319` (a `Committed` outside `CollectingCommitments` is discarded) and the `!self_committed` drop in `handle_block_advance`; root cause `F-CORE-031`. Every restart and every in-depth reorg replays the range, re-spawns the engine check, and the replayed `Committed(self)` arrives in `WaitingForEngineCheck` and is dropped; at the deadline the entry is dropped with no `Reveal` while the commitment is live onchain. **Run 1, on Anvil:** 2,000 slashed, 2,000 locked — **−4,000 fee tokens**; the warp-ordering control passed, so the loss is unconditional. **Run 2** (`poc/F2-SEN-001`, re-run at `fe9e84c`): variant (a), the anchor snapshot holds `WaitingForEngineCheck` and no effect is re-spawned; variant (b), the replayed case, which `F2-CORE-030` shows also loses. The highest-certainty irreversible loss in the crate, and nobody owns it. **Fix:** Section 3 item 2(d); not option 3. **Trail:** run 1 E1 99 High · run 2 E1 92 High · agreement.

#### [`F-SEN-002`](../findings/F-SEN-002.md) — High, 98 · Commits before the verdict are not tallied; early finalize — **assigned @rmeissner**

`crates/sentinel/src/service.rs:307-319`, `372-375`, `649-655`. `committed_count` starts at 0 when `commit_vote` runs, so any peer commitment landing before our engine answers is invisible; `revealed_count >= committed_count` fires early and `finalize()` drops the entry with no `Finalize` and no `Claim`. The mirror case is now a stall: a reverted `Finalize` parks the entry in `WaitingForOutcome` with no deadline and no retry (`service.rs:657-671`, `484`). **Run 1:** 4,500 (bond plus reward) left unclaimed on a real `SentinelOracle`, no restart needed — a merely slower engine. **Run 2** at `fe9e84c`: confirmed, and a registered sentinel that commits late and never reveals parks every honest sentinel. **Fix must also close `F2-SEN-010`** — Section 3 item 2(a). **Trail:** run 1 E1 98 · run 2 E1 90 · agreement.

#### [`F-SEN-015`](../findings/F-SEN-015.md) — High, 98 · A replayed engine check re-decides a committed vote — **assigned @rmeissner**

`crates/sentinel/src/service.rs:156-171`, `176-179`, `214-224`; `hashCommitment` is bound at `bindings.rs:54-60` and never called. `reveal` recomputes `keccak256(abi.encodePacked(approve, salt, sentinel, requestId, reason))`; a replay repeats the live HTTP call and the second verdict overwrites the reason the commitment was built from. **Run 1, on Anvil:** duplicate commit `AlreadyCommitted`, then **`InvalidReveal 0x9ea6d127`**, bond slashed. Variant 2 (`Unknown` while a co-deployed engine is still booting, A3) removes the entry at `:156` and never re-inserts it — same slash, no engine determinism needed. **Run 2** folded it into `F2-SEN-001` (c) as inference; the reconciliation kept run 1's executed number (Section 7.1). **Fix:** same as `F-SEN-001`; not option 2 as written, not option 3. **Trail:** run 1 E1 98 · run 2 I (under-weighted) · run 1 carried.

#### [`F2-SEN-010`](../findings/F2-SEN-010.md) — High, 93 · Commits mined in the commit-deadline block are never tallied — introduced by #914

`crates/sentinel/src/service.rs:423-425`, `456-466`, `657-671`, `484`; `SentinelOracleRequests.sol:117`. Section 2.4 has the mechanism. Adversarial form: one registered sentinel places its `commit` when the head is `commitDeadline − 1` and never reveals; every honest sentinel undercounts by one, emits `Finalize` early on the last honest reveal, reverts `FinalizeTooEarly`, parks — no `OracleResult`, every honest bond locked until manual intervention. **Run 2:** executed in Rust and forge (`poc/F2-SEN-010`), option 1 compiled 47/47. **Run 1** probed the same hunk on the unmerged branch at 85 (`F-SEN-016` D1). The `STOPGAP` note at `service.rs:390-402` defers the compensation revert to #915. **Fix:** Section 3 item 2(a). **Trail:** run 1 probe E1 85 on the branch · run 2 E1 93 (Critic 82) · canonical `F2-SEN-010`.

#### [`F2-XC-001`](../findings/F2-XC-001.md) — High, 95 · Any configuration parse error prints the whole file, signer key included

`crates/validator/src/main.rs:33-40`, `config.rs:13-17`, `43-47`; `crates/sentinel/src/main.rs:30-37`. Both `main`s return `Result<(), Box<dyn Error>>`, so `Termination` prints the error with `{:?}`; `config::Error::Parse` wraps `toml::de::Error`, whose derived `Debug` includes `input` — the complete file, `signer = "0x<32-byte key>"` included — on any failure anywhere in the file (an unknown key, a `0` for a `NonZeroU64`, a 31-byte key). The handbooks tell operators to diagnose with `docker logs`. **Run 2:** five executions on both binaries (`state/run2/logs/r8-config-tests.txt`, `c2-xc-001-config-leak.txt`). **Run 1 missed it** — the most consequential run-1 miss. **Fix:** one line (Section 3 item 10). **Trail:** run 2 reviewer Medium (operator-error trigger) · Critic High 95 E1 · High carried.

### Medium (24)

| ID | Runs | Cert. | Basis | Claim |
| --- | --- | --- | --- | --- |
| [`F-CORE-067`](../findings/F-CORE-067.md) | 1+2 | 98 | E1 | Unconditional `INSERT` on `enqueue`; every replay re-submits (nonces 2 and 3 live); the replayed `Commit` pins `self_committed = false` on the `F-SEN-001` path — run 2 Low, Medium carried |
| [`F-VAL-002`](../findings/F-VAL-002.md) | 1+2 | 93 | E1 | ECDH pad is an unhashed x-coordinate used in both directions; one complaint response exposes two shares — run 2 Low standalone, Medium kept for the executed second-share leak |
| [`F-SEN-005`](../findings/F-SEN-005.md) | 1+2 | 95 | E1 | `WaitingForOutcome`/`WaitingForDisputeResolution` retained unconditionally (`service.rs:484-485`); `DisputeTriggered.deadline` decoded (`bindings.rs:44`) and dropped (`760-768`); no `timeoutArbitration` binding — assigned |
| [`F-CORE-004`](../findings/F-CORE-004.md) | 1+2 | 90 | E1 | Event-fetch failures retry forever at 100 ms; the block watcher starves and reorg detection stops — run 2 executed the starvation (75 → 90) |
| [`F-CORE-064`](../findings/F-CORE-064.md) | 1+2 | 90 | E1 | `expires_at` void once a nonce is allocated; never-accepted case executed — run 2 Low, Medium carried |
| [`F-CORE-066`](../findings/F-CORE-066.md) | 1+2 | 90 | E1 | `tx::Config` accepts zero in-flight, zero window and `nan`; all three executed — run 2 Low, Medium carried |
| [`F2-SEN-003`](../findings/F2-SEN-003.md) | 2 | 90 | E1 | `finalize()` (`service.rs:649-655`) drops a bonded entry whenever our own reveal was not observed; both routes executed at `fe9e84c`; option 1 compiled, 42 tests pass |
| [`F-CORE-030`](../findings/F-CORE-030.md) | 1+2 | 85 | E2 | `Driver::run` returns `()`; `ExceededMaxReorgDepth` exits with status 0; `/health` is a constant `OK` nothing consumes — run 2 Low, Medium carried (#820 asked for an observable failure) |
| [`F2-VAL-003`](../findings/F2-VAL-003.md) | 2 | 85 | E1 | A restart whose warp covers an epoch-boundary key-gen start drops the round's commitments; the validator is excluded from the epoch (`state/keygen.rs:912-981`) |
| [`F-VAL-003`](../findings/F-VAL-003.md) | 1+2 | 82 | E1 | A complaint compels a plaintext share reveal with no check the plaintiff could have received a share, no per-plaintiff bound, no deadline — run 2 Low, Medium kept |
| [`F-CORE-034`](../findings/F-CORE-034.md) | 1+2 | 80 | E2 | Fixed 100 ms retry forever, one warning per attempt; run 2 quantifies ×17 validator / ×11 sentinel request volume per 100 ms |
| [`F2-VAL-031`](../findings/F2-VAL-031.md) | 2 | 80 | E1 | Nonce generators cold after every restart; the below-marker return in `service/effect.rs:245-256` precedes `generator.start`; no reconciliation runs during a warp |
| [`F-CORE-035`](../findings/F-CORE-035.md) | 1 | 78 | E2 | Every `tx::Error::Rpc` classified intermittent and swallowed forever; a permanently failing node silently stops all onchain action — run 2 missed the swallow |
| [`F-SEN-004`](../findings/F-SEN-004.md) | 1+2 | 78 | E1 | No cap on concurrent engine checks or outstanding bonds; reveals starve behind commits from ≈ 8 proposals per block at parity windows (`poc/F2-SEN-005`; narrowed, not fixed, by #914) |
| [`F-VAL-066`](../findings/F-VAL-066.md) | 1+2 | 78 | E1 | Retention set computed before the block's logs; across a restart the inline prune beats the spawned reconciliation, secret lost 19/20 (`F2-VAL-035`) — the unqualified `DELETE` is gone since #909 |
| [`F-VAL-063`](../findings/F-VAL-063.md) | 1+2 | 72 | E2 | Consensus-critical config unvalidated: rollover livelock when `blocks_per_epoch <= key_gen_timeout`, `genesis_salt` zero in the sample, no chain binding — points (1), (3) run-2 miss |
| [`F-CORE-003`](../findings/F-CORE-003.md) | 1+2 | 70 | E2 | A lagging backend answering `null` for a block is treated as a reorg: spurious uncle, rollback, full replay, on-chain duplicates — run 2 Low, Medium carried |
| [`F-CORE-012`](../findings/F-CORE-012.md) | 1 | 70 | E2 | Bloom equality (`events.rs:441-466`, `bloom.rs:37-40`) is idempotent over repeated `(address, topics)` shapes — the shape of every per-participant ceremony event; `check_logs_limit` skipped on the client-filtered path — **run 2 missed** |
| [`F-CORE-033`](../findings/F-CORE-033.md) | 1+2 | 70 | E2 | One task per matching log, no cap, queue or backpressure; a warp page delivers no `NewBlock`, so expired proposals are checked too — team question answered |
| [`F-VAL-004`](../findings/F-VAL-004.md) | 1+2 | 69 | E1 gap | Rollover instance: a failed or lost `KeyGenSetup` (`service/effect.rs:264-276`) forfeits a numbered epoch; a rolling upgrade escalates it to an epoch skip (`poc/F2-VAL-063`) — genesis instance accepted (A16) |
| [`F-CORE-061`](../findings/F-CORE-061.md) | 1+2 | 65 | E1 | Both underpriced regexes require a _replacement_; a first-submission rejection records no floor and is re-signed at the same fee forever, blocking every later nonce; node wording still `I` |
| [`F-CORE-062`](../findings/F-CORE-062.md) | 1 | 60 | E2 | Allocated nonce never released, allocation floored at `MAX(nonce) + 1`; one bad nonce wedges the queue — halves rediscovered by run 2, the wedge not stated |
| [`F-CORE-063`](../findings/F-CORE-063.md) | 1+2 | 55 | E1 | Execution inferred from the account nonce alone and never revoked; run 2 executed the mark and narrowed the trigger (the gap closes unless an action is queued inside retention) |
| [`F-XC-050`](../findings/F-XC-050.md) | 1 | 48 | E2 | No DKG handler checks group membership; a pure-cardinality close also feeds `finalize_key_gen` for rollovers (`state/keygen.rs:512`, `552-557`, `599`); stale comment at `:451-452` — run 2 strengthened the precondition (`F2-VAL-061`), missed the consequence |

### Low (38)

| ID | Runs | Cert. | Claim |
| --- | --- | --- | --- |
| [`F2-CORE-011`](../findings/F2-CORE-011.md) | 2 | 92 | No rollback anchor on a fresh start (`blocks.rs:279-289`); an in-window reorg exits `MissingSnapshot` and the restart resumes from the orphaned block — the fourth silent path under #820 |
| [`F2-XC-002`](../findings/F2-XC-002.md) | 2 | 92 | Sample `database` URLs lack `?mode=rwc`; the documented first start on a fresh volume fails |
| [`F2-XC-050`](../findings/F2-XC-050.md) | 2 | 92 | Snapshot committed and pruned before the page's actions are enqueued; with one retained snapshot (every warp page, or depth 0) a crash loses them — fault-injected; sentinel `Claim` loss executed |
| [`F-SEN-006`](../findings/F-SEN-006.md) | 1+2 | 90 | Duplicate `approve`/`commit`/`reveal`/`finalize`/`claim` on every replay; `handle_arbitration_timeout` claims without a bond |
| [`F-SEN-012`](../findings/F-SEN-012.md) | 1+2 | 90 | Engine client makes exactly one attempt; any failure inside a window with blocks left is a permanent abstention |
| [`F-XC-006`](../findings/F-XC-006.md) | 1+2 | 90 | Nothing binds a database to chain, deployment or signer; run 2 re-signed a request on a second Anvil chain from the same store |
| [`F-VAL-040`](../findings/F-VAL-040.md) | 1+2 | 88 | `last_signer` overwritten by every accepted reveal; a signer re-reveals to become responsible for the restart, then does nothing (50 → 88 by execution) |
| [`F-CORE-036`](../findings/F-CORE-036.md) | 1 | 85 | `Debug` bound on every service `Effect`/`Resume`, printed at `trace` in five sites; `frost-core` redacts, so the secret leg is refuted |
| [`F-XC-004`](../findings/F-XC-004.md) | 1+2 | 85 | Runtime images run as root on floating tags, no digest, no `rust-toolchain.toml` |
| [`F-SEN-007`](../findings/F-SEN-007.md) | 1+2 | 82 | No balance, allowance, registration or `voting_window` pre-flight; a sentinel that cannot commit pays for an `approve` and a reverting `commit` per request |
| [`F-SEN-009`](../findings/F-SEN-009.md) | 1+2 | 82 | Engine timeout derived from an unvalidated `voting_window`; `voting_window = 0` accepted, 1 s floor (`known`) |
| [`F-SEN-011`](../findings/F-SEN-011.md) | 1 | 82 | A restart orphans an in-flight check whose proposal predates the rollback anchor; the request expires unvoted |
| [`F-CORE-011`](../findings/F-CORE-011.md) | 1+2 | 80 | Indexer hangs behind a live-but-silent peer with no error or metric; run 2 settled the `reqwest` premise (`tcp_user_timeout` 30 s on Linux) — Medium → Low |
| [`F-SEN-003`](../findings/F-SEN-003.md) | 1+2 | 80 | Warp delivers no `NewBlock`, so reveals in the replayed range are discarded and `finalize` takes the `timed_out` branch; the funds impact was fixed by `199629e`, the replayed reveals are still discarded (`poc/F2-SEN-003` b at `fe9e84c`) — team question answered |
| [`F-CORE-009`](../findings/F-CORE-009.md) | 1 | 78 | `block_time = 0` with empty retry delays is a delay-free poll loop; unbounded depth; `start_block > head` — cases 2–3 run-2 miss |
| [`F-CORE-005`](../findings/F-CORE-005.md) | 1+2 | 75 | `max_reorg_depth = 0` promises a loud failure but disables the uncled-block recovery: `recent` is always empty and the `-32001` path spins silently |
| [`F2-CORE-007`](../findings/F2-CORE-007.md) | 2 | 75 | A restart against a node lagging more than `max_reorg_depth` exits with an opaque `BadUpdate` |
| [`F-XC-009`](../findings/F-XC-009.md) | 1+2 | 75 | Both samples ship `signer = 0x…01` (parses; the service signs) and `rpc = "https://rpc.gnosischain.com"`; the `0.0.0.0` item was refuted in run 1 |
| [`F-CORE-039`](../findings/F-CORE-039.md) | 1+2 | 72 | Shutdown is observed only between inputs; inline `housekeeping` is a second unbounded await |
| [`F-VAL-064`](../findings/F-VAL-064.md) | 1+2 | 72 | Residual after the split: `/health` unreachable in the shipped deployment (exit status under `F-CORE-030`, root container under `F-XC-004`) |
| [`F-CORE-006`](../findings/F-CORE-006.md) | 1+2 | 70 | Filter is the cross product of watched addresses and topics; out-of-range `sol!` enum values decode to `__Invalid`, not an error (core half of `F-VAL-060`) |
| [`F-CORE-007`](../findings/F-CORE-007.md) | 1+2 | 70 | A node disagreeing with itself during startup puts `initialize` in an unbounded, undelayed loop |
| [`F-CORE-008`](../findings/F-CORE-008.md) | 1 | 70 | Block polling compares chain timestamps against the host wall clock; skew silently delays indexing |
| [`F-CORE-065`](../findings/F-CORE-065.md) | 1+2 | 70 | `transactions` bound to neither chain id nor signer; `chain_id` cached at connect (`known`; the reused-database trigger is out under A17) |
| [`F-VAL-065`](../findings/F-VAL-065.md) | 1 | 70 | `SetValidatorStaker` accumulates across restarts, `Preprocess` never expires (`action.rs:252-254`, `368-378`, `main.rs:82-92`); a replayed `Sign` burns a sequence for the whole group |
| [`F-VAL-067`](../findings/F-VAL-067.md) | 1+2 | 70 | The Rust test counts complaints cumulatively while `FROSTParticipantMap` nets them; for numbered epochs the abort is one coordinated extra ceremony (Medium → Low) |
| [`F-CORE-040`](../findings/F-CORE-040.md) | 1 | 65 | The driver's inner `select!` abandons the watcher's in-flight `eth_getLogs` on every resume — cost run-2 miss |
| [`F2-VAL-007`](../findings/F2-VAL-007.md) | 2 | 65 | `active_epoch` advances only through self-staged epochs; a phantom rollover session (QA not attempted) |
| [`F-CORE-037`](../findings/F-CORE-037.md) | 1 | 62 | Snapshots are an unversioned JSON dump with no migration path; an upgrade that changes a state type bricks the store |
| [`F-VAL-034`](../findings/F-VAL-034.md) | 1 | 55 | `handle_nonces` (`state/sign.rs:359-377`) applies a resume without checking the signature id; outcome executed benign |
| [`F-VAL-038`](../findings/F-VAL-038.md) | 1 | 55 | Chunk generation saturates every core, then holds the shared SQLite writer for 1,025 statements; the pruning series adds an inline writer |
| [`F-SEN-008`](../findings/F-SEN-008.md) | 1 | 52 | Hard-coded gas limits and an unconditional non-zero `approve` assume a plain ERC-20; the contract now documents the fee-token precondition (`SentinelOracle.sol:62-68`) |
| [`F2-VAL-004`](../findings/F2-VAL-004.md) | 2 | 50 | The late-setup branch derives a divergent share-round deadline (QA not attempted; genesis case `known`) |
| [`F-CORE-010`](../findings/F-CORE-010.md) | 1 | 45 | The `-32001` recovery commits the rewind before the event watcher validates it — both runs independently found no trigger |
| [`F-CORE-032`](../findings/F-CORE-032.md) | 1 | 45 | A panicking effect task is logged and skipped; the resume the state machine waits for is gone |
| [`F-VAL-031`](../findings/F-VAL-031.md) | 1 | 42 | A dead nonce-generation worker (`nonces.rs:101`, `117`) is never detected, logged or restarted |
| [`F-VAL-036`](../findings/F-VAL-036.md) | 1 | 40 | `observe` (`preprocess.rs:194`) accepts a non-monotonic sequence and rewinds `next_sequence` |
| [`F-VAL-035`](../findings/F-VAL-035.md) | 1 | 35 | Observation: unzeroised nonce JSON, abandoned chunks of retained groups never reclaimed; leg (c) refuted (`sqlx-sqlite` sets `foreign_keys = 1`) |

### Informational (18)

| ID | Runs | Cert. | Claim |
| --- | --- | --- | --- |
| [`F-XC-003`](../findings/F-XC-003.md) | 1 | 96 | `deny_unknown_fields` with `#[serde(flatten)]` works — six tests, re-executed by run 2 — but no in-tree test proves it |
| [`F2-VAL-064`](../findings/F2-VAL-064.md) | 2 | 95 | The documented `--config-file=<path>` spelling is rejected by `argh` in both binaries; six documents to fix |
| [`F-XC-001`](../findings/F-XC-001.md) | 1+2 | 93 | No `[profile.release]`: overflow checks off in shipped binaries (debug panics, release wraps) |
| [`F-XC-007`](../findings/F-XC-007.md) | 1+2 | 92 | No `cargo audit`/`cargo deny` gate in CI; feature width (item 2, unused SQL drivers, refuted: 0 symbols) |
| [`F-XC-011`](../findings/F-XC-011.md) | 1+2 | 90 | Five advisories and eleven warnings in `Cargo.lock`; none has an in-scope connection path (`cargo tree -i` re-run agrees; `h2` only reached the engine) |
| [`F2-XC-005`](../findings/F2-XC-005.md) | 2 | 90 | Secret deletion is logical only: bundled SQLite 3.51.3 without `SECURE_DELETE`; one `H` row corrected in QA, conclusion strengthened |
| [`F-VAL-062`](../findings/F-VAL-062.md) | 1 | 88 | `ReconcileGroupSecrets` carries every tracked key share and derives `Debug`, printed at `warn`; `frost-core` redacts — hygiene gap, run-2 miss |
| [`F-SEN-014`](../findings/F-SEN-014.md) | 1 | 88 | Every participating sentinel submits `finalize`; `K − 1` revert |
| [`F-CORE-038`](../findings/F-CORE-038.md) | 1 | 85 | `kdf::derive_key`'s multi-part `info` is a plain concatenation; the doc comment implies otherwise |
| [`F2-CORE-067`](../findings/F2-CORE-067.md) | 2 | 85 | The transaction queue exports no metrics and never persists the transaction hash |
| [`F2-VAL-034`](../findings/F2-VAL-034.md) | 2 | 85 | No `PRAGMA user_version` check; a pre-#908 database opens, then every reconciliation fails (`no such column: delete_at_block`) — `known` |
| [`F-SEN-010`](../findings/F-SEN-010.md) | 1+2 | 85 | The sample's zero addresses parse and start cleanly; the guard is only against a missing field (`config.rs:44` TODO) |
| [`F-XC-051`](../findings/F-XC-051.md) | 1 | 85 | `verify_commitment` delegates structural checks to `frost-core`, which rejects them: empty `c` → `MissingCommitment`, identity `c[0]` → `InvalidIdentityElement`; settled from the pinned sources |
| [`F-XC-008`](../findings/F-XC-008.md) | 1 | 80 | The engine client follows redirects and honours proxy environment (items 2/3, from the pinned `reqwest`/`hyper-util`); item 1 out of scope |
| [`F2-SEN-011`](../findings/F2-SEN-011.md) | 2 | 80 | `handle_arbitration_timeout`'s doc and metric (`service.rs:550-557`, `585`) assume a full refund; the contract states the non-revealer's slash is never refunded |
| [`F2-XC-007`](../findings/F2-XC-007.md) | 2 | 75 | The signer key lingers in un-zeroized configuration buffers (`Zeroizing<String>` costs no new dependency) |
| [`F2-CORE-003`](../findings/F2-CORE-003.md) | 2 | 70 | The default-path log fetch has no completeness check; `may_contain_log` is dead code |
| [`F-VAL-037`](../findings/F-VAL-037.md) | 1 | 60 | Merkle trees pad with `B256::ZERO` and have no leaf/internal domain separation |

## 7. Where the runs disagreed, and what the audit got wrong

### 7.1 The three sub-claim contradictions, all settled by execution

| Where | Run 1 said | Run 2 said | Settled |
| --- | --- | --- | --- |
| `F-CORE-031` / `F2-CORE-030` | Critic note: `F-SEN-001`'s replay case is "mutually exclusive" with the resume-loss defect | when the emitting block is replayed, the replayed own `Committed` is applied synchronously before the re-spawned effect can resume, so the replayed case loses too | **run 2**, by execution (`poc/F2-CORE-030`) — this is what lifts `F-CORE-031` to High 92 |
| `F-CORE-060` / `F2-CORE-060` | "the cap bypassed ~28,700×" | the cap ratio is preserved through bumps (algebra verified) | **both right, about different quantities**: `cap_priority_fee` (`tx/fees.rs:12-31`) bounds tip / own `maxFeePerGas`, and that ratio survives bumping; it never bounds the tip against the chain's base fee once the ratchet runs (measured 28,744× the base-fee-derived cap). `cargo test -p safenet-core --lib -- tx::fees tx::tests::failed_replacements`: 4 passed |
| `F-SEN-015` / `F2-SEN-001` (c) | executed twice (in-process and on Anvil: `AlreadyCommitted` then `InvalidReveal`, −4,000) | "at most `I` (engine out of scope)" | **run 1**: variant 2 (`Unknown` on replay) needs no engine determinism, only a co-deployed engine still booting (A3); E1 at the state-machine level; code unchanged at `fe9e84c` |

### 7.2 Severity bands: what moved and what was not carried

- **Up:** `F-CORE-031` Medium → High (run 2's executed ordering extension); `F-VAL-060` Medium → High, conditional (three scenarios executed); `F-VAL-032` Medium/High → High unconditional (attacker-created group).
- **Down:** `F-CORE-011` Medium → Low (pinned `reqwest`: `tcp_user_timeout` 30 s); `F-VAL-064` Medium → Low (split); `F-VAL-067` Medium → Low; `F-VAL-066` Medium/High → Medium (the unqualified `DELETE` is gone); `F-XC-008`, `F-XC-011` Low → Informational (engine out of scope); `F-XC-051` Low → Informational (pinned `frost-core`); `F-VAL-004` High → Medium rollover / Informational genesis (A16); `F-XC-050` genesis instance → Informational `known`; `F-VAL-033` High → out of scope (A17).
- **Run-2 bands not carried, recorded as dissent in the finding files:** Medium on `F-CORE-001`/`002`; Low on `F-CORE-003`/`030`/`033`/`064`/`066`/`067`, `F-VAL-002`/`003`; Medium on `F-VAL-030`; Informational on `F2-XC-008` (→ `F-XC-009`) and `F2-VAL-068` (→ `F-XC-004`). Run 1 holds the live-stack executed evidence in each of these.
- **Run-1 numbers not carried:** Medium 78 on `F-CORE-031`; Medium 60 on `F-CORE-011`; Medium 50 on `F-VAL-060`; Plausible 62 on `F-SEN-004`; the "cap bypassed ~28,700×" sentence of `F-CORE-060` is rewritten, the number stands.
- **Nothing was dropped.** Every run-1 finding keeps a row; the only removals are by fix, by assumption or by supersession, each with a pointer. No unsettled disagreement on existence or mechanism remains; the only open items are the ones neither run could execute (Section 8.2).

### 7.3 Run-1 findings run 2 did not rediscover

A miss is information about run 2's coverage, not a reason to drop the finding. All are still valid at `fe9e84c` unless stated.

| Run-1 finding | Sev / cert. | Run-2 coverage |
| --- | --- | --- |
| `F-CORE-012` bloom-equality blind spot | Medium 70 | **missed** — the most significant core miss; the second independent route to `F-CORE-002`'s outcome |
| `F-CORE-035` `Rpc` errors swallowed forever | Medium 78 | **missed** the mechanism (`F2-CORE-067` covers the metrics leg) |
| `F-CORE-062` nonce never released, as a standalone | Medium 60 | halves rediscovered (`F2-CORE-061`, `F2-CORE-064`); the wedge not stated |
| `F-CORE-032` panicking effect task | Low 45 | **missed** (cancel-safety examined only) |
| `F-CORE-040` abandoned `eth_getLogs` per resume | Low 65 | safety examined, cost **missed** |
| `F-CORE-009` block-watcher config | Low 78 | case 1 only; cases 2–3 **missed** |
| `F-CORE-008`, `F-CORE-010`, `F-CORE-036`, `F-CORE-037`, `F-CORE-038` | Low 70 / Low 45 / Low 85 / Low 62 / Informational 85 | examined by a run-2 reviewer and deliberately not filed; `F-CORE-010`'s "no trigger" independently confirmed |
| `F-VAL-039` top-up headroom | High 58 | **missed** (both runs' R5 declined the drain framing; run 1's Critic filed it) |
| `F-VAL-031` dead nonce worker | Low 42 | **missed** |
| `F-VAL-034` nonce resume without signature-id check | Low 55 | **missed** |
| `F-VAL-036` non-monotonic `observe` | Low 40 | **missed** (forward jump only, in `F2-VAL-061` b) |
| `F-VAL-037` Merkle padding | Informational 60 | **missed** |
| `F-VAL-038` chunk generation saturates cores | Low 55 | **missed**; the pruning series added an inline writer |
| `F-VAL-062` `Debug` on secret-bearing effects | Informational 88 | **missed** the hygiene gap |
| `F-VAL-065` validator legs of non-idempotent actions | Low 70 | **missed** (core side rediscovered) |
| `F-VAL-063` points (1) and (3) | Medium 72 | partial (timing relation, `genesis_salt`, chain binding rediscovered) |
| `F-VAL-035` (a)/(b) | Low 35 | **missed** as an observation; below threshold |
| `F-SEN-008` gas limits and fee-token assumptions | Low 52 | **missed** (`F2-SEN-006`/`007` cite the literals as facts) |
| `F-SEN-011` restart orphans an in-flight check | Low 82 | mechanism carried by `F2-CORE-030`, sentinel fact not restated |
| `F-SEN-003`, `F-SEN-014`, `F-SEN-015` | Low 80 / Informational 88 / High 98 | partial (route executed; one sentence; inference only) |
| `F-XC-002` union, `F-XC-003` test gap | Low 88 / Informational 96 | soft miss (redaction checked, hygiene gap not filed); behaviour re-executed as a rejected hypothesis |
| `F-XC-050` DKG membership check | Medium 48 | **missed** the consequence while strengthening the precondition (`F2-VAL-061`) |
| `F-XC-051` structural checks delegated | Low 42 → Informational 85 | **missed**, then settled from pinned sources |

Not misses: `F-VAL-005` (fixed by the merge; residual rediscovered as `F2-VAL-035`), `F-VAL-033` (A17, not looked for), `F-SEN-013` (correctly not re-filed), the forward-looking files.

### 7.4 Run-2 findings run 1 missed

Fourteen reachable at `2893917`: [`F2-XC-001`](../findings/F2-XC-001.md) (High — the most consequential), [`F2-XC-002`](../findings/F2-XC-002.md), [`F2-XC-005`](../findings/F2-XC-005.md), [`F2-XC-007`](../findings/F2-XC-007.md) (run-1 lead SEN-H15 recorded, never filed), [`F2-XC-050`](../findings/F2-XC-050.md), [`F2-CORE-003`](../findings/F2-CORE-003.md), [`F2-CORE-007`](../findings/F2-CORE-007.md), [`F2-CORE-011`](../findings/F2-CORE-011.md), [`F2-CORE-067`](../findings/F2-CORE-067.md), [`F2-VAL-003`](../findings/F2-VAL-003.md) (Medium), [`F2-VAL-004`](../findings/F2-VAL-004.md), [`F2-VAL-007`](../findings/F2-VAL-007.md), [`F2-VAL-064`](../findings/F2-VAL-064.md), [`F2-SEN-003`](../findings/F2-SEN-003.md) (Medium; run 1 recorded the drop as a consequence only). Four exist because of code run 1 did not have: `F2-VAL-031`, `F2-VAL-034`, `F2-SEN-010`, `F2-SEN-011`.

### 7.5 Refuted or reduced by execution

| Claim | Run | Outcome |
| --- | --- | --- |
| `F-SEN-013` — one undecodable `Revealed.reason` stalls every indexer | 1 | **False.** `alloy-sol-types` 1.6.0 decodes invalid UTF-8 lossily (byte `0x80` → `"\u{fffd}"`). Refuted-as-filed, 98; unchanged at `fe9e84c` |
| The secret-leak cluster — `frost-core`'s derived `Debug` prints scalars | 1 | **It redacts** (`SigningShare("<redacted>")`); QA's hits were a false positive in the PoC's own needle. Leak leg refuted, hygiene leg confirmed: `F-VAL-062`, `F-XC-002`, `F-CORE-036` |
| `F-VAL-035` leg (c) — the cascade depends on an unasserted SQLite pragma | 1 | **Refuted.** `sqlx-sqlite` sets `foreign_keys = 1` itself. 45 → 35, retained as an observation |
| `F-XC-007` item 2 — unused SQL drivers widen the attack surface | 1 | **Refuted.** 0 symbols in every release binary; the process claim stands |
| `F-XC-009` item 2 — `0.0.0.0` binds | 1 | **Refuted**; option 2 keeps only its startup-`warn!` clause |
| The ABI memory-exhaustion worry behind `F-XC-051`, `F-VAL-001`, `F-VAL-003` | 1 | **Dead.** alloy's `vec_try_with_capacity` is fallible; no finding filed |
| `F-VAL-033` Critical → High | 1 | The un-burn is real, but in two live runs the restore drove the validator into a permanent genesis self-halt before any reuse; now out of scope under A17 |
| `F-CORE-060`'s cap sentence | 2 | R3's arithmetic is right (own-fee ratio preserved); the base-fee-relative bypass is also right (Section 7.1) |
| `F-CORE-011`'s premise (a stalled connection hangs forever) | 2 | pinned `reqwest`: `tcp_user_timeout` 30 s on Linux — Medium → Low, Plausible → Confirmed |
| `F-XC-051` — `frost-core` accepts an empty or identity commitment | 2 | `Err(MissingCommitment)` / `Err(InvalidIdentityElement)` from the pinned sources; settled in the code's favour |
| `F2-CORE-002`'s High claim | 2 | rejected by its Critic (a degraded RPC is A4); the reconciliation carried run 1's High on the on-chain loss |
| `F2-SEN-004` claim 5 | 2 | superseded by the contract's now-documented non-revealer slash (`SentinelOracleRequests.sol:288-296`); became `F2-SEN-011` |
| out-of-range `sol!` enum values "fail decoding" | 2 | they decode to a hidden `__Invalid` variant (`poc/F2-CORE-004/enum-decode`); wording pinned in `F2-VAL-061`/`F2-CORE-004`, no `H` |

### 7.6 Hallucinated (`H`) claims caught

Run 1: three across roughly 1,000 citations — one in a surviving file (`F-VAL-064`: "there is no `.dockerignore` anywhere in the repository"; four per-Dockerfile ones exist, and the concern survives narrower), two in engine files since removed. Run 2: one (`F2-XC-005` basis row 6 — corrected in QA: `sqlx`'s `sqlite` feature bundles SQLite 3.51.3 without `SECURE_DELETE`; the correction strengthened the conclusion). None collapsed its finding. Run 2's Critics found 0 `H` rows at Gate 2 and 0 findings refuted outright.

### 7.7 What execution did not establish

1. **`F-CORE-060` does not self-start on a healthy node** — it needs a stale fee floor; the accepted arm is executed (×311.9) but has no independent blocking condition as a standalone trigger.
2. **`F-VAL-030` / `F-VAL-032`: mechanism executed, downstream consequence not locally reachable** — the 1,024-sequence sign refusal needs ~1,024 signs of griefing.
3. **`F-CORE-067` was reproduced by a restart, not a reorg** — `anvil_reorg` drops reorged transactions permanently; run 2's replay over a file-backed store mined duplicates (`…0a` ×3, `…0b`, `…14`, `…15` ×2) across restarts only.
4. **`F2-VAL-035`'s live rate** — 19/20 at the driver seam whenever the precondition holds; the Anvil restart with a real group drop during the outage needs a multi-validator devnet.
5. **`F-VAL-004` rollover instance** — structural gap executed, the lost effect in a numbered epoch traced, not observed live.

## 8. Open questions and still-blocked items

### 8.1 The team's four questions, answered

Full replies with citations in [ledger §5.2](RECONCILIATION.md#52-draft-replies-one-per-open-thread-addressed-to-the-reviewer-the-operator-can-paste-them).

1. **`F-CORE-067` (@rmeissner) — "different nonces and one of them will revert, right?" Yes.** `SentinelOracleCommitments.sol:92-96` rejects the duplicate with `AlreadyCommitted()` (now via `vote == NONE`), observed live at nonces 2 and 3 (`0xbfec5558`). Five qualifications: the paired duplicate `approve` succeeds; the revert is invisible (execution inferred from the account nonce, `tx/mod.rs:185-197`, `tx/storage.rs:224-231`); it is not always gas only (the replayed `Commit` pins `self_committed = false` on the `F-SEN-001` path, executed in `poc/F2-CORE-030`; a replayed `Sign` burns a sequence for the whole group); the surface grew with #914 (`Claim` rows with `expires_at: None` are never pruned); the dedup must be keyed on identity, not calldata.
2. **`F-CORE-033` (@nlordell) — "I would expect the transaction deadline to prevent this fan-out." No.** The deadline bounds the `ApproveToken`/`Commit` rows (`expires_at = commit_deadline`, `sentinel/service.rs:226-242`; skipped at allocation, `tx/storage.rs:150-155`), not the `EngineCheck` effects: the effect is emitted unconditionally on `TransactionProposed` (`service.rs:137-144`), carries no deadline (`sentinel/effect.rs:21-25`), and deadlines are consulted only on `NewBlock`, which a warp page never delivers — so every proposal in a catch-up page (up to 100 blocks) is checked, expired ones included. Executed downstream: reveals starve from ≈ 8 proposals per block (`poc/F2-SEN-005`).
3. **`F-SEN-003` (@rmeissner → @nlordell) — "why no `NewBlock` when warping?"** The design intent is the team's; the consequence is executed and current at `fe9e84c`: in `poc/F2-SEN-003` (b) our own `Revealed@122` inside the warp page is discarded (`service.rs:347-362`), `NewBlock(125)` re-emits a `Reveal` that reverts `AlreadyRevealed`, and the entry is dropped with no `Claim`. The validator has the same gap (`F2-VAL-003`). Two remediations both runs endorse: Section 3 item 7, or never drop a bonded entry.
4. **`F-VAL-033` (@nlordell) — "I still need to understand the nonce reuse issue."** The mechanism is real — `take_nonce` deletes the row and nothing records consumption (`secrets/store.rs:281-291`), so a restored file re-offers a burned nonce — but the only trigger is an operator restore, which A17 excludes. The objection to remediation option 1 stands; in the live run the validator self-halted before any reuse.

### 8.2 Unsettled items — neither run could execute these

| Item | What is missing | Both runs' position |
| --- | --- | --- |
| `F-VAL-039` (High, Plausible 58) | an executed cost model of the top-up griefing against a live group | run 1's Critic filed it; both R5s declined the drain framing |
| `F2-VAL-035` / `F-VAL-066` (Medium 78) | the live occurrence rate of the precondition | executed at the driver seam only; needs a multi-validator devnet |
| `F-CORE-061`, `F-CORE-062`, `F-CORE-063` (Plausible 65 / 60 / 55) | node-side rejection wording (`I` in both runs) and a live wedge | in-crate mechanisms executed by run 2; mechanism agreed |
| `F-VAL-004` rollover instance (Plausible 69) | a lost effect in a numbered epoch, live | structural gap executed |
| `F-VAL-038` (Low 55) | the duration of the 1,025-statement transaction under load | unmeasured in both runs |
| `F2-VAL-004`, `F2-VAL-007` (Plausible 50 / 65) | any QA | run 2 only, QA not attempted |
| `F-CORE-010` (Plausible 45) | any trigger | both runs independently found none |
| `F-XC-050` (Plausible 48) | the window (a complaint outstanding while confirmations arrive) | precondition strengthened, consequence not filed by run 2 |
| `F-SEN-008` (Plausible 52) | behaviour against a proxied or hooked fee token | the contract now documents the precondition |
| `F-SEN-004` branch (i), `F-SEN-015` variant 1, `F-SEN-007`'s engine-side pre-flight, `F-XC-008` item 1 | the engine | out of scope; none decides a severity |

### 8.3 Unmerged work

| Work | PRs | Status at `fe9e84c` | Effect |
| --- | --- | --- | --- |
| Optimistic block transition | #915 | unmerged — `origin/feat/optimistic_block_transition` `b2aad06`, 2 ahead / 31 behind `main`; #914's `STOPGAP` (`service.rs:390-402`) defers its compensation revert to it | `F-SEN-016` D2–D4 forward-looking; the warp arm at `state/mod.rs:173-181` is the pre-#915 form. Round 3 ([`IN-FLIGHT.md`](IN-FLIGHT.md#round-3--open-branches-after-run-2)): resolves the mechanism of `F2-SEN-010` when rebased and merged — executed |
| Batched Execution | #899–#904 | unmerged — `origin/feat/batex_4` `d6edbb6`, 5 ahead / 31 behind; Phases 5–10 on no pushed branch | `F-CORE-068`, `F-CORE-069` forward-looking; the stack fixes nothing and worsens `F-CORE-062`/`063`/`065`; re-run list in [`IN-FLIGHT.md`](IN-FLIGHT.md) |
| Sentinel deadlines | #914 | **merged alone** | `F2-SEN-010` (Section 2.4) |
| Scheduled Secret Pruning | #906–#913 | **merged** | Section 2.3 |
| SEF veto epic | #917 | merged (document only) | nothing to re-validate |

### 8.4 Still to do

- **Reply to the 13 threads** (Section 2.5) and assign the Critical and the eleven unassigned Highs.
- **Re-runs not yet made:** a reorg-nonce run that restarts a validator and asserts on the **epoch-1** group with a same-machine control on `main` — `scripts/run_validator_reorg_nonce_test.sh` still asserts on the genesis group and its header comment describes a restart of validator A that the script does not perform; the multi-validator restart with a real group drop during the outage (`F2-VAL-035`'s live rate); an executed cost model for `F-VAL-039`. Run 2 did not re-run run 1's Anvil PoCs (independence); its own equivalents ran at `fe9e84c` (`poc/F2-SEN-*` re-runs, `poc/F2-VAL-030`, `poc/F2-VAL-031`, `poc/F2-XC-050/coverage-7.3`, the store tests).
- **Dependency questions.** Run 1's toolchain-blocked list ([`../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md`](../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md)) was written before a toolchain existed; its in-scope questions were closed during run 1's later phases and run 2 re-executed the ones that bear on a live row (Section 7.5); the engine-only questions are withdrawn in that file's scope note. Nothing in either run asserts a clean bill of health for a dependency beyond what `cargo audit` and `cargo tree -i` showed.
- **Pin the Foundry version and make the PoCs permanent** by adding a thin `src/lib.rs` to the three binary-only crates; every PoC README's `cargo test --lib` must be `cargo test -p <crate> --bins`.

## 9. Scope, method, safety, assumptions

### 9.1 Scope and baseline

|  | Run 1 | Run 2 |
| --- | --- | --- |
| Commit | `2893917`, re-validated at `a7f3915` | `3ec8bc5` (Phases 0–3), `fe9e84c` (delta and reconciliation) |
| In scope | core, validator, sentinel, engine — 83 `.rs` files, 24,203 lines | core, validator, sentinel — 61 `.rs` files, 20,492 lines, 185 tests; two sample configs, two Dockerfiles, the workspace manifests |
| Baseline | read-only Phases 0–4 (no toolchain); toolchain arrived for Phase 5 onward | full: build 0, test 0 (183 in-scope tests at `3ec8bc5`, 185 at `fe9e84c`), `clippy -D warnings` 0, `cargo audit` 1 (5 vulnerabilities, 11 warnings, none reachable in scope), `cargo tree -d` 14 version splits; exit codes identical at `fe9e84c` ([`baseline.md`](../state/run2/baseline.md), [`baseline-delta.md`](../state/run2/baseline-delta.md)) |
| Coverage | [`../state/coverage.md`](../state/coverage.md) | 57 files covered, 8 thin (all spot-read, no defect), 0 unverified; 7 seams examined; 0 seeded leads unexamined ([`coverage.md`](../state/run2/coverage.md)) |
| Independence | — | no run-1 finding, report or state file opened; the codebase map and analyses shared as leads |

### 9.2 Assumptions — only the FALSE or changed ones explained

| ID | Run 1 | Run 2 | Note |
| --- | --- | --- | --- |
| A8 | **FALSE** — the engine test-vector corpus was unavailable | moot | the engine is out of scope |
| A9 | **FALSE** for Phases 0–4 (no toolchain, 3.8 GiB RAM), satisfied later but for a version gap | **TRUE** with versions | rustc/cargo 1.98.1 stable, unpinned (no `rust-toolchain.toml`); **Foundry 1.8.1 where A9 names 1.5.1**; `just` 1.40.0; 11 GiB RAM; 75 G free disk |
| A15 | TRUE mid-run (Charter text, engine leads only) | not exercised | — |
| A16 | — | **new**: genesis need not be recoverable | Section 2.2 |
| A17 | — | **new**: only the services access their databases | Section 2.2 |
| A1–A7, A10–A14 | TRUE | TRUE | trusted operator (A1); adversarial chain data within the fault bound (A2); the engine API reachable only by its co-deployed sentinel (A3); RPC trusted for liveness but possibly stale, rate-limited or incomplete (A4); reorgs to `max_reorg_depth` handled (A5); crypto libraries trusted, usage reviewed (A6); `contracts/src` authoritative (A7); Gnosis parameters (A10); scope as amended by Section 11 (A11); known items tagged (A12); no branches, commits or drift by agents (A13, A14) |

### 9.3 Safety boundary and the live-RPC trap

Every executed scenario in both runs ran on **local Anvil only: chain 31337, `127.0.0.1`**, endpoint printed in each log. No testnet or mainnet endpoint was contacted; run 2's only network use was `cargo audit` fetching the advisory database. This matters because the repository points at a live chain: **both shipped samples carry `rpc = "https://rpc.gnosischain.com"`** (`crates/validator/validator.sample.toml:10`, `crates/sentinel/sentinel.sample.toml:10`) next to the parseable well-known key `signer = "0x…01"` (`:15`, `:16`), so a config copied from the sample and started carelessly signs against Gnosis mainnet with a key everyone holds. Every config either run used was copied and rewritten to loopback. That is the operational core of [`F-XC-009`](../findings/F-XC-009.md); anyone reproducing this work should do the same.

### 9.4 Method, in one paragraph each

**Run 1.** Recon → 10 reviewers → 9 Critics (incl. a Coverage Critic) → 4 QA → Documentation → 4 verification agents (Phase 5, unit-level PoCs appended into tracked sources and reverted per file) → V-INT (the repo's Anvil suites) → RW-VAL (findings driven end to end against real contracts and binaries with value moving), a Manager gate between phases. Certainty per PROMPT §8; a PoC that fails to compile or passes when the finding says it should fail counts against the finding. Narrative: [`../state/STATE.md`](../state/STATE.md).

**Run 2.** Recon → 8 reviewers (R1–R8) → 7 Critics (incl. a Coverage Critic; canonical naming of duplicate clusters) → 5 QA agents (StateMachine-, Watcher- and driver-seam PoCs; remediation options compiled in scratch copies) → a delta Recon, delta reviewer and delta QA after the merge of `main` → four per-crate reconciliation agents ([core](../state/run2/reconciliation/core.md), [validator](../state/run2/reconciliation/validator.md), [sentinel](../state/run2/reconciliation/sentinel.md), [cross-cutting](../state/run2/reconciliation/cross-cutting.md)) → the merged ledger. Conflicts settled by three rules: executed evidence wins; severity against PROMPT §8's scale, never by averaging; where nothing settles it, both positions stand. Narrative and gates: [`../state/run2/STATE.md`](../state/run2/STATE.md); reviewer logs in [`../state/run2/agents/`](../state/run2/agents/); executed logs in [`../state/run2/logs/`](../state/run2/logs/).

**Not reproduced here:** every trigger, basis table, remediation option and QA transcript is in the finding file; every PoC source and output is under [`../poc/`](../poc/) (`F-*` run 1, `F2-*` run 2); the per-row settlement of every severity and certainty is in the per-crate reconciliation parts and in each finding's `## Reconciliation (run 2)` section, which is authoritative where this report and the ledger differ.

### 9.5 Where to look

| For | Go to |
| --- | --- |
| a finding's claim, basis table, trigger, remediation options, QA and Critic verdicts, both runs' final status | [`../findings/<ID>.md`](../findings/), section `## Reconciliation (run 2)` last |
| the run-1 PoC for a finding | [`../poc/F-<ID>/`](../poc/) — `README.md`, `poc*.rs`, `RESULT-*.txt` |
| the run-2 PoC | [`../poc/F2-<ID>/`](../poc/) — `run.txt` / `rerun.txt` or `output.txt`, `*.rerun-fe9e84c.txt` for the sentinel re-runs |
| the ledger row and the fold of any `F2-*` ID | [`RECONCILIATION.md`](RECONCILIATION.md) §2.1, §2.2 |
| why a severity or certainty was carried the way it was | the per-crate parts under [`../state/run2/reconciliation/`](../state/run2/reconciliation/) |
| run-2 reviewer logs, rejected hypotheses, executed test logs | [`../state/run2/agents/`](../state/run2/agents/), [`../state/run2/logs/`](../state/run2/logs/) |
| run-2 environment, baseline commands, inventory, drift | [`../state/run2/baseline.md`](../state/run2/baseline.md), [`../state/run2/baseline-delta.md`](../state/run2/baseline-delta.md) |
| run-1 narrative, coverage and baseline | [`../state/STATE.md`](../state/STATE.md), [`../state/coverage.md`](../state/coverage.md), [`../state/baseline.md`](../state/baseline.md) |
| the team's review threads and paste-ready replies | [`../state/pr-review-threads.md`](../state/pr-review-threads.md), [`RECONCILIATION.md`](RECONCILIATION.md) §5.2 |

_Compiled from the 154 finding files, [`RECONCILIATION.md`](RECONCILIATION.md), the run-1 and run-2 state, coverage and baseline documents, without changing any verdict, certainty, severity or claim._
