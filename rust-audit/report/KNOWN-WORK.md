# Known work: live findings mapped onto issues, TODOs and known items

This file maps the 90 live findings at `361aaf3` ([`RECONCILIATION.md`](RECONCILIATION.md) section 2) onto what the team already tracks. Sources: the open issues on the Rust services as fetched on 2026-10-08 ([`../state/run3/map.md`](../state/run3/map.md) section 5.2: #20, #69, #113, #465, #549, #961, #973), the closed issues the earlier mapping cited (closed means absent from that fetch's open-issue list), the two TODO markers in scope (map section 5.1) and the known items in [`../state/run3/settled.md`](../state/run3/settled.md) section 2. The earlier version of this file mapped run 1's 108 findings at `2893917`. Rows that left the folder in run 3 are dropped here and listed in [`REPORT.md`](REPORT.md) Section 7. Bands come from the ledger.

| Relationship | Rows |
| --- | --: |
| ALREADY TRACKED (an open issue, TODO or known item covers it) | 11 |
| TRACKED BUT UNDERSTATED (an open issue names the class; the finding shows more) | 3 |
| CLOSED BUT STILL PRESENT (a closed issue was meant to settle it) | 8 |
| PARTIALLY OVERLAPS (shares scope with an open or closed issue and adds something) | 13 |
| NEW (no issue, TODO or known item) | 55 |
| **Total** | **90** |

## 1. Closed but still present

### #820 (closed by PR #834, `40467c5`): Ensure correct handling of reorgs exceeding max reorg depth

The issue asked that deep reorgs be flagged and trigger an error instead of silently continuing. A deep reorg seen while the service runs now exits. These paths still continue silently or exit unobserved.

| Finding | Band | What still reproduces at `361aaf3` |
| --- | --- | --- |
| [`F-CORE-001`](../findings/F-CORE-001.md) | Low 99 | The closing PR's own body says the block hash of the last safe block is not persisted, so a reorg that coincides with a restart may cause unexpected behaviour. Across a restart the deep reorg is still accepted silently (rv-core-a, real binary). Low since run 3: no reorg of 3 or more blocks is verified on Gnosis |
| [`F-CORE-030`](../findings/F-CORE-030.md) | Medium 95, `known` | The deliberate exit leaves the process with status 0 and `/health` answers a constant OK, so no supervisor sees the flag the issue asked for |
| [`F-CORE-005`](../findings/F-CORE-005.md) | Low 85 | With `max_reorg_depth = 0` the `-32001` recovery spins silently instead of failing loudly |
| [`F2-CORE-011`](../findings/F2-CORE-011.md) | Low 92 | A fresh start has no rollback anchor; an in-window reorg exits `MissingSnapshot` and the restart resumes from the orphaned block |

[`F2-CORE-007`](../findings/F2-CORE-007.md) (Low 95) and [`F-VAL-064`](../findings/F-VAL-064.md) (Low 72) overlap the same issue in part: a restart against a lagging node exits with `BadUpdate` and status 0, and the shipped deployment cannot see a halted validator.

### #656 (closed by PR #686, `80f747c`): Only Bump Fees on Underpriced Transactions

The issue's goal was to avoid an "exponential death spiral on gas prices". The fix stopped bumping on unrelated RPC failures. The spiral is still reachable, and the fix introduced a second defect.

| Finding | Band | What still reproduces at `361aaf3` |
| --- | --- | --- |
| [`F-CORE-060`](../findings/F-CORE-060.md) | High 98 | The underpriced path still raises both fee fields by 10 % every block with no ceiling and overrides `priority_fee_cap_percentage`; run 3 re-ran run 1's PoC with the same numbers |
| [`F-CORE-061`](../findings/F-CORE-061.md) | Medium 65 | Introduced by the fix: both patterns match only replacement rejections ("replacement transaction ... underpriced", "could not replace existing tx"), so a first-submission rejection is re-signed at the same fee and blocks every later nonce |

### #614 (closed by the non-blocking-effects epic, `19d3815`): Evaluate Parallel Execution of Effects

The issue body quotes an analysis with two horns. The head-of-line horn was fixed by going async. The second was not:

> _"A `NewRequest` event arriving while the dynamic check is still pending will be entirely ignored… The node will never vote on the proposal. **Recommendation:** You must introduce a `WaitingForDynamicCheck` variant to `RequestState`."_

The variant exists (`RequestState::WaitingForEngineCheck`), but events that arrive in it are still discarded, not held.

| Finding | Band | What still reproduces at `361aaf3` |
| --- | --- | --- |
| [`F-SEN-001`](../findings/F-SEN-001.md) | Medium 99, `known` | The replay of the sentinel's own `Committed` is ignored in `WaitingForEngineCheck` after a restart or a reorg, so no reveal follows. Open #961 and #973 now track it |
| [`F-SEN-002`](../findings/F-SEN-002.md) | Medium 90 | Other sentinels' commitments are ignored while the sentinel's own check runs, and the early finalize drops the bonded entry |
| [`F-SEN-015`](../findings/F-SEN-015.md) | Medium 95 | Partly fixed by #978 and #979 (live reorgs); the restart window remains |
| [`F-CORE-031`](../findings/F-CORE-031.md) | Medium 93, `known` | A rollback onto the spawning block discards the resume and never re-runs the effect |
| [`F-CORE-033`](../findings/F-CORE-033.md) | Medium 70 | The parallel execution landed with no cap, queue or backpressure |
| [`F-SEN-004`](../findings/F-SEN-004.md) | Medium 78 | No cap on concurrent engine checks or outstanding bonds |
| [`F-SEN-011`](../findings/F-SEN-011.md) | Low 82, `known` | A restart orphans an in-flight check whose proposal predates the rollback anchor. Open #973 tracks it |
| [`F-CORE-032`](../findings/F-CORE-032.md) | Low 45 | A panicking effect task is logged and skipped |
| [`F-CORE-040`](../findings/F-CORE-040.md) | Low 65, `known` | The driver's `select!` drops the watcher's in-flight request on every resume |

### Other closed issues with a live row

- **#777** (fixed by #1008): [`F-VAL-040`](../findings/F-VAL-040.md) (Low 70). #1008 removed the nonce-round restart that read `last_signer`. The share-round rule remains.
- **#799**: [`F-VAL-061`](../findings/F-VAL-061.md) (Medium 90), [`F-VAL-004`](../findings/F-VAL-004.md) (Medium 69), [`F-CORE-031`](../findings/F-CORE-031.md) and [`F-CORE-032`](../findings/F-CORE-032.md) shared its effect-lifecycle scope, and its engine-timeout bullet matched [`F-SEN-009`](../findings/F-SEN-009.md), which the TODO at `crates/sentinel/src/main.rs:48-52` still carries.
- **#666**, and **#801** (closed by #803, regression test #807): [`F-VAL-066`](../findings/F-VAL-066.md) (Low 65) and [`F-VAL-035`](../findings/F-VAL-035.md) (Informational 70) shared the secret-retention scope. The row that #801 itself reproduced, `F-VAL-005`, is Fixed.
- **#546**: its proposal, deriving the commit count from the oracle's registered sentinel set, is the shape of [`F-SEN-002`](../findings/F-SEN-002.md)'s fix.

Rows the earlier mapping tied to #667 (`F-SEN-003`) and #118 (`F-VAL-067`) are resolved. #118 (FROSTCoordinator test coverage) is still open and concerns the contracts. No live row maps to it.

## 2. Open issues on the services

| Issue | Title | Live rows | Relationship |
| --- | --- | --- | --- |
| #20 | KeyGen Scaling on Secret Share Reveals | none | a protocol change to share transport; no live row touches it |
| #69 | Optimize Key Generation facing Multiple Dishonest Participants | [`F-VAL-003`](../findings/F-VAL-003.md) | partial overlap: complaint-round accounting. [`F3-XC-100`](../findings/F3-XC-100.md) is not covered: #69 is about removing one dishonest participant per failed key generation, not a rule that removes an honest participant together with the attacker |
| #113 | Prevent sensitive information from being logged | [`F3-XC-130`](../findings/F3-XC-130.md), [`F3-XC-004`](../findings/F3-XC-004.md) (`known`); [`F3-XC-040`](../findings/F3-XC-040.md), [`F-CORE-036`](../findings/F-CORE-036.md), [`F-VAL-062`](../findings/F-VAL-062.md) (understated) | the body is "RPC Keys"; #1007 and #1046 covered part of it. Run 3 adds the key printed in a typed-field parse error (F3-XC-040) and the URL printed by alloy's IPC fallback (F3-XC-130). Key shares behind derived `Debug` are redacted by `frost-core` today |
| #465 | Extend Alloy Asserter to Check Inputs | none | test infrastructure; no live row depends on it |
| #549 | Reevaluate sentinel bond slashing | [`F-SEN-005`](../findings/F-SEN-005.md) | partial overlap: funds stuck behind a timeout path nobody calls. #974 fixed the dispute half; rmeissner kept the full refund on timeout for the initial version (2026-08-03) |
| #961 | Sentinel logged "ignoring unexpected commitment" and missed the reveal | [`F-SEN-001`](../findings/F-SEN-001.md) and [`F-CORE-031`](../findings/F-CORE-031.md) (`known`); [`F-SEN-002`](../findings/F-SEN-002.md) and [`F-CORE-003`](../findings/F-CORE-003.md) in part | F-SEN-001 matches the incident's symptom, the sentinel's own commit ignored in `waiting_for_engine_check` ([`F3-SEN-004`](../findings/F3-SEN-004.md) reproduced it on the real core warp). F-SEN-002 ignores other sentinels' commitments on the same code path. F-CORE-003 reaches the same hole after a false rollback ([`F3-XC-160`](../findings/F3-XC-160.md)) |
| #973 | Sentinel: Retrigger in-flight engine check effects on restart | [`F-SEN-011`](../findings/F-SEN-011.md), [`F-SEN-001`](../findings/F-SEN-001.md), [`F-CORE-031`](../findings/F-CORE-031.md) (`known`) | run 3 adds two limits of the proposal: a re-emitted check is not enough once the sentinel's own commit is part of the replay (F3-SEN-004), and it does not touch F-SEN-002 |

## 3. TODOs and known items

| Item | Where | Live rows |
| --- | --- | --- |
| TODO: pick and document a default for `voting_window`; keep the addresses required | `crates/sentinel/src/config.rs:37-41` | [`F-SEN-009`](../findings/F-SEN-009.md), [`F-SEN-010`](../findings/F-SEN-010.md) |
| TODO: derive the engine timeout from effect lifecycle data | `crates/sentinel/src/main.rs:48-52` | [`F-SEN-009`](../findings/F-SEN-009.md) |
| The process exits with status 0 after a fatal driver error | `settled.md` section 2 | [`F-CORE-030`](../findings/F-CORE-030.md); the exit half of [`F-VAL-064`](../findings/F-VAL-064.md) |
| The ECDH share pad is the raw x-coordinate; the team chose the proof of possession (#1027) and closed #1026 | `settled.md` section 2 | [`F-VAL-002`](../findings/F-VAL-002.md) |
| A key-generation secret leaves the store by two routes (`take` documentation) | `settled.md` section 2 | claim 2 of [`F3-VAL-032`](../findings/F3-VAL-032.md) |
| Cancel safety of `Driver::next_input` | `settled.md` section 2 | [`F-CORE-040`](../findings/F-CORE-040.md) |
| A bond stranded at `revealDeadline + 1` (#1017 follow-up) | `settled.md` section 2 | [`F2-SEN-003`](../findings/F2-SEN-003.md) |
| In-flight engine checks lost on a restart (#973); an own commit ignored while the check runs (#961) | `settled.md` section 2 | [`F-SEN-001`](../findings/F-SEN-001.md), [`F-CORE-031`](../findings/F-CORE-031.md), [`F-SEN-011`](../findings/F-SEN-011.md) |
| Genesis-only liveness (A16) | `settled.md` section 1 | the genesis instances of [`F-VAL-004`](../findings/F-VAL-004.md) and [`F-XC-050`](../findings/F-XC-050.md), Informational and `known` |
| The validator flow-test epic | `epics/2026_07_14_validator_state_machine_flow_test_harness.md` | none. It is not implemented at `361aaf3` and still describes the 1024-nonce chunks Direct Nonce removed (map section 5.4). The two earlier rows mapped to it are resolved |

The ledger also carries `known` on [`F-CORE-065`](../findings/F-CORE-065.md) and [`F-CORE-067`](../findings/F-CORE-067.md), a tag from run 1. No open issue, TODO or `settled.md` item lists either, so the table below counts them as NEW. The known item that #1062 records (oracle fees paid for results the validators ignore) has no live row.

## 4. Every live row

| Finding | Band | Open issue | Closed issue it outlives | TODO or known item | Relationship |
| --- | --- | --- | --- | --- | --- |
| [`F-CORE-001`](../findings/F-CORE-001.md) | Low 99 |  | #820 |  | CLOSED BUT STILL PRESENT |
| [`F-CORE-002`](../findings/F-CORE-002.md) | Medium 99 |  |  | the handbook note at `docs/validator-handbook.md:37` is stale for Nethermind 1.39+ | NEW |
| [`F-CORE-003`](../findings/F-CORE-003.md) | Medium 80 | #961, #973 (the consequence after the false rollback) |  |  | PARTIALLY OVERLAPS |
| [`F-CORE-004`](../findings/F-CORE-004.md) | Medium 90 |  |  |  | NEW |
| [`F-CORE-005`](../findings/F-CORE-005.md) | Low 85 |  | #820 |  | CLOSED BUT STILL PRESENT |
| [`F-CORE-006`](../findings/F-CORE-006.md) | Low 70 |  |  |  | NEW |
| [`F-CORE-007`](../findings/F-CORE-007.md) | Low 70 |  |  |  | NEW |
| [`F-CORE-008`](../findings/F-CORE-008.md) | Low 92 |  |  |  | NEW |
| [`F-CORE-009`](../findings/F-CORE-009.md) | Low 85 |  |  |  | NEW |
| [`F-CORE-010`](../findings/F-CORE-010.md) | Low 45 |  |  |  | NEW |
| [`F-CORE-011`](../findings/F-CORE-011.md) | Low 80 |  |  |  | NEW |
| [`F-CORE-012`](../findings/F-CORE-012.md) | Medium 85 |  |  |  | NEW |
| [`F-CORE-030`](../findings/F-CORE-030.md) | Medium 95 |  | #820 | `settled.md` section 2 (exit status 0) | ALREADY TRACKED |
| [`F-CORE-031`](../findings/F-CORE-031.md) | Medium 93 | #961, #973 | #614, #799 | `settled.md` section 2 | ALREADY TRACKED |
| [`F-CORE-032`](../findings/F-CORE-032.md) | Low 45 |  | #614, #799 |  | PARTIALLY OVERLAPS |
| [`F-CORE-033`](../findings/F-CORE-033.md) | Medium 70 |  | #614 |  | PARTIALLY OVERLAPS |
| [`F-CORE-034`](../findings/F-CORE-034.md) | Medium 85 |  |  |  | NEW |
| [`F-CORE-035`](../findings/F-CORE-035.md) | Medium 78 |  |  |  | NEW |
| [`F-CORE-036`](../findings/F-CORE-036.md) | Low 85 | #113 |  |  | TRACKED BUT UNDERSTATED |
| [`F-CORE-038`](../findings/F-CORE-038.md) | Informational 90 |  |  |  | NEW |
| [`F-CORE-039`](../findings/F-CORE-039.md) | Low 72 |  |  |  | NEW |
| [`F-CORE-040`](../findings/F-CORE-040.md) | Low 65 |  | #614 | `settled.md` section 2 (cancel safety of `next_input`) | ALREADY TRACKED |
| [`F-CORE-060`](../findings/F-CORE-060.md) | High 98 |  | #656 |  | CLOSED BUT STILL PRESENT |
| [`F-CORE-061`](../findings/F-CORE-061.md) | Medium 65 |  | #656 (introduced by its fix) |  | CLOSED BUT STILL PRESENT |
| [`F-CORE-062`](../findings/F-CORE-062.md) | Medium 60 |  |  |  | NEW |
| [`F-CORE-063`](../findings/F-CORE-063.md) | Medium 55 |  |  |  | NEW |
| [`F-CORE-064`](../findings/F-CORE-064.md) | Medium 90 |  |  |  | NEW |
| [`F-CORE-065`](../findings/F-CORE-065.md) | Low 90 |  |  | `known` tag carried from run 1; no issue, TODO or settled item lists it | NEW |
| [`F-CORE-066`](../findings/F-CORE-066.md) | Informational 90 |  |  |  | NEW |
| [`F-CORE-067`](../findings/F-CORE-067.md) | Medium 98 |  |  | `known` tag carried from run 1; no issue, TODO or settled item lists it | NEW |
| [`F2-CORE-003`](../findings/F2-CORE-003.md) | Informational 80 |  |  |  | NEW |
| [`F2-CORE-007`](../findings/F2-CORE-007.md) | Low 95 |  | #820 |  | PARTIALLY OVERLAPS |
| [`F2-CORE-011`](../findings/F2-CORE-011.md) | Low 92 |  | #820 |  | CLOSED BUT STILL PRESENT |
| [`F2-CORE-067`](../findings/F2-CORE-067.md) | Informational 85 |  |  |  | NEW |
| [`F-VAL-002`](../findings/F-VAL-002.md) | Low 90 |  |  | `settled.md` section 2 (the ECDH pad; the team chose the proof of possession and closed #1026) | ALREADY TRACKED |
| [`F-VAL-003`](../findings/F-VAL-003.md) | Medium 82 | #69 |  |  | PARTIALLY OVERLAPS |
| [`F-VAL-004`](../findings/F-VAL-004.md) | Medium 69 (rollover) |  | #799 | genesis instance accepted under A16 | PARTIALLY OVERLAPS |
| [`F-VAL-034`](../findings/F-VAL-034.md) | Low 55 |  |  |  | NEW |
| [`F-VAL-035`](../findings/F-VAL-035.md) | Informational 70 |  | #666 |  | PARTIALLY OVERLAPS |
| [`F-VAL-037`](../findings/F-VAL-037.md) | Informational 60 |  |  |  | NEW |
| [`F-VAL-040`](../findings/F-VAL-040.md) | Low 70 |  | #777 (fixed for the nonce round by #1008) |  | CLOSED BUT STILL PRESENT |
| [`F-VAL-060`](../findings/F-VAL-060.md) | Medium 90 |  |  |  | NEW |
| [`F-VAL-061`](../findings/F-VAL-061.md) | Medium 90 |  | #799 |  | PARTIALLY OVERLAPS |
| [`F-VAL-062`](../findings/F-VAL-062.md) | Informational 90 | #113 |  |  | TRACKED BUT UNDERSTATED |
| [`F-VAL-063`](../findings/F-VAL-063.md) | Low 90 |  |  |  | NEW |
| [`F-VAL-064`](../findings/F-VAL-064.md) | Low 72 |  | #820 | its exit-status half is the `settled.md` section 2 item | PARTIALLY OVERLAPS |
| [`F-VAL-065`](../findings/F-VAL-065.md) | Low 70 |  |  |  | NEW |
| [`F-VAL-066`](../findings/F-VAL-066.md) | Low 65 |  | #666, #801 |  | PARTIALLY OVERLAPS |
| [`F2-VAL-007`](../findings/F2-VAL-007.md) | Low 65 |  |  |  | NEW |
| [`F2-VAL-064`](../findings/F2-VAL-064.md) | Informational 95 |  |  |  | NEW |
| [`F3-VAL-030`](../findings/F3-VAL-030.md) | High 97 |  |  |  | NEW |
| [`F3-VAL-031`](../findings/F3-VAL-031.md) | Low 60 |  |  |  | NEW |
| [`F3-VAL-032`](../findings/F3-VAL-032.md) | Informational 90 |  |  | claim 2: `settled.md` section 2 (`take` documentation) | PARTIALLY OVERLAPS |
| [`F3-VAL-062`](../findings/F3-VAL-062.md) | Medium 90 |  |  |  | NEW |
| [`F3-VAL-100`](../findings/F3-VAL-100.md) | Low 88 |  |  |  | NEW |
| [`F-SEN-001`](../findings/F-SEN-001.md) | Medium 99 | #961, #973 | #614 | `settled.md` section 2 | ALREADY TRACKED |
| [`F-SEN-002`](../findings/F-SEN-002.md) | Medium 90 | #961 (same code path; #961 is about the own commit) | #614, #546 |  | CLOSED BUT STILL PRESENT |
| [`F-SEN-004`](../findings/F-SEN-004.md) | Medium 78 |  | #614 |  | PARTIALLY OVERLAPS |
| [`F-SEN-005`](../findings/F-SEN-005.md) | Medium 95 | #549 |  |  | PARTIALLY OVERLAPS |
| [`F-SEN-006`](../findings/F-SEN-006.md) | Low 90 |  |  |  | NEW |
| [`F-SEN-007`](../findings/F-SEN-007.md) | Low 82 |  |  |  | NEW |
| [`F-SEN-008`](../findings/F-SEN-008.md) | High 97 |  |  |  | NEW |
| [`F-SEN-009`](../findings/F-SEN-009.md) | Low 82 |  | #799 (its engine-timeout bullet) | TODOs at `crates/sentinel/src/config.rs:37-41` and `crates/sentinel/src/main.rs:48-52` | ALREADY TRACKED |
| [`F-SEN-010`](../findings/F-SEN-010.md) | Informational 85 |  |  | TODO at `crates/sentinel/src/config.rs:37-41` | ALREADY TRACKED |
| [`F-SEN-011`](../findings/F-SEN-011.md) | Low 82 | #973 | #614 | `settled.md` section 2 | ALREADY TRACKED |
| [`F-SEN-012`](../findings/F-SEN-012.md) | Low 90 |  |  |  | NEW |
| [`F-SEN-014`](../findings/F-SEN-014.md) | Informational 95 |  |  |  | NEW |
| [`F-SEN-015`](../findings/F-SEN-015.md) | Medium 95 |  | #614 |  | CLOSED BUT STILL PRESENT |
| [`F2-SEN-003`](../findings/F2-SEN-003.md) | Medium 95 |  | #614 | `settled.md` section 2 (a bond stranded at `revealDeadline + 1`, #1017 follow-up) | ALREADY TRACKED |
| [`F2-SEN-011`](../findings/F2-SEN-011.md) | Informational 80 |  |  |  | NEW |
| [`F3-SEN-060`](../findings/F3-SEN-060.md) | Low 85 |  |  | introduced by #974 | NEW |
| [`F-XC-001`](../findings/F-XC-001.md) | Informational 93 |  |  |  | NEW |
| [`F-XC-003`](../findings/F-XC-003.md) | Informational 96 |  |  |  | NEW |
| [`F-XC-004`](../findings/F-XC-004.md) | Low 85 |  |  |  | NEW |
| [`F-XC-006`](../findings/F-XC-006.md) | Low 90 |  |  |  | NEW |
| [`F-XC-007`](../findings/F-XC-007.md) | Informational 92 |  |  |  | NEW |
| [`F-XC-008`](../findings/F-XC-008.md) | Informational 90 |  |  |  | NEW |
| [`F-XC-009`](../findings/F-XC-009.md) | Low 75 |  |  |  | NEW |
| [`F-XC-011`](../findings/F-XC-011.md) | Informational 90 |  |  |  | NEW |
| [`F-XC-050`](../findings/F-XC-050.md) | Medium 48 (rollover) |  |  | genesis instance accepted under A16 | NEW |
| [`F-XC-051`](../findings/F-XC-051.md) | Informational 90 |  |  |  | NEW |
| [`F2-XC-002`](../findings/F2-XC-002.md) | Low 95 |  |  |  | NEW |
| [`F2-XC-005`](../findings/F2-XC-005.md) | Informational 90 |  |  |  | NEW |
| [`F2-XC-007`](../findings/F2-XC-007.md) | Informational 75 |  |  |  | NEW |
| [`F2-XC-050`](../findings/F2-XC-050.md) | Low 92 |  |  |  | NEW |
| [`F3-XC-003`](../findings/F3-XC-003.md) | Informational 95 |  |  |  | NEW |
| [`F3-XC-004`](../findings/F3-XC-004.md) | Informational 90 | #113 |  | a #1046 trade-off | ALREADY TRACKED |
| [`F3-XC-040`](../findings/F3-XC-040.md) | Informational 90 | #113 |  | the residual of #1046 | TRACKED BUT UNDERSTATED |
| [`F3-XC-100`](../findings/F3-XC-100.md) | High 95 |  |  | no issue; the team's PR review holds it as an item from the review of #1025 (Medium); #69 does not cover it | NEW |
| [`F3-XC-130`](../findings/F3-XC-130.md) | Informational 95 | #113 |  | #1046's "Not covered" list | ALREADY TRACKED |
