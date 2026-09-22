# Known work — audit findings mapped onto existing issues, TODOs and epics

> **Scope note:** sentinel-engine findings (`F-ENG-*`, `F-XC-005/010/052`) were removed after the engine was placed out of scope; the counts in this document predate that removal and are recomputed in the final `REPORT.md`. Run 2 has since been reconciled with run 1 in [`RECONCILIATION.md`](RECONCILIATION.md); Section 5 below adds the run-2 counterpart and the combined status of every row in this document, and lists the `known`-tagged run-2 items.

What the team already has a ticket for, and what it does not. Sources: 24 GitHub issues (16 open, 8 closed), 8 in-code `TODO`s, 3 epics in [`epics/`](../../epics/). The mapping was made over run 1's 108 findings at `2893917` (85 remain in [`../findings/`](../findings/) after the engine's removal); the severities quoted in Sections 1–3 are run 1's, and the combined severities are in [`RECONCILIATION.md`](RECONCILIATION.md) §2.

| Relationship                 |  Count |
| ---------------------------- | -----: |
| ALREADY TRACKED              | **11** |
| TRACKED BUT UNDERSTATED      |  **8** |
| **CLOSED BUT STILL PRESENT** |  **9** |
| PARTIALLY OVERLAPS           | **25** |
| **NEW**                      | **55** |
| Total                        |    108 |

One of the 55 new findings ([`F-SEN-013`](../findings/F-SEN-013.md)) was refuted by execution during the audit; 54 stand.

**Sentinel caveat.** `crates/sentinel` changed by +559 lines in the merge from `main` (verdict aggregation, meta transactions, waiting-for-outcome state, oracle events over local inference). Every `F-SEN-*` row below records the _issue mapping only_, established against the audit commit `2893917`. Whether the merge changed the behaviour is settled in each finding's `## Post-merge revalidation` section, not here.

---

## 1. Closed but still present

Nine findings reproduce a defect that a closed issue was meant to have settled. Four closed issues are involved: **#820**, **#801**, **#656**, **#614**.

### #820 — _Ensure correct handling of reorgs exceeding max reorg depth_ (closed by PR #834, commit `40467c5`)

The issue asked that deep reorgs "be flagged and trigger an error instead of silently continuing". The running case now exits. Three separate paths still continue silently.

| Finding | Sev | What still reproduces |
| --- | --- | --- |
| [`F-CORE-001`](../findings/F-CORE-001.md) | High 99% | **The closing PR's own body says it:** _"currently the block hash of the last safe block is not persisted, which means a reorg that falls together with a restart might cause unexpected behavior."_ No follow-up issue was filed. Reproduced in Phase 8 as a controlled A/B on one Anvil chain with a real `SentinelOracle` and a real bonded request: the identical reorg is fatal while running, and across a restart the service comes back **alive with 0 WARN / 0 ERROR**, replaying canonical logs onto state derived from orphaned blocks. Self-reinforcing — #834's deliberate exit plus `restart: always` walks the process straight into the silent path. |
| [`F-CORE-030`](../findings/F-CORE-030.md) | Medium 85% | `Driver::run` returns `()`. The `ExceededMaxReorgDepth` exit #834 added leaves the process with **status 0**, indistinguishable from an operator shutdown. `/health` answers a constant `OK` and nothing in the repo consumes it. The flag the issue asked for is not observable by any supervisor. |
| [`F-CORE-005`](../findings/F-CORE-005.md) | Low 75% | #834 changed `max_reorg_depth = 0` from "don't handle reorgs" to "fail on any reorg", and the doc comment now promises it fails _loudly_. With depth 0 the `recent` deque is permanently empty, so `revalidate_last_block` can never invalidate anything and the `-32001` "resource not found" recovery path spins silently instead. The strict setting is loud on one path and mute on the other. |

### #801 — _Nonces might not be retained during a reorg_ (closed by PR #803 `f01a3ea`, regression test PR #807 `cf5891e`)

| Finding | Sev | What still reproduces |
| --- | --- | --- |
| [`F-VAL-005`](../findings/F-VAL-005.md) | High 99% | `f01a3ea` broadened `retain_nonces` to cover DKG groups — the exact fix the issue proposed — but left `retain_keygen_secrets(keygen)` in the same function untouched, so the same reorg window still deletes the group's `keygen_secrets` row. **The regression test added to close the issue exhibits the residual inside its own passing run.** `scripts/run_validator_reorg_nonce_test.sh` exits 0 and prints `SUCCESS` while: it uncles the `KeyGenSecretShared` block (9) below the epoch-1 group's `KeyGen` block (10) — F-VAL-005's exact trigger — then asserts only on the _genesis_ group; both validators log `failed to advance key generation, skipping to next epoch :: "The participant's commitment is incorrect."`; validator A's epoch-1 commitment differs before (`0343738943…`) and after (`03308eece3…`) the reorg. **Epoch 1 was lost network-wide while the suite reported SUCCESS.** The script's header comment and SUCCESS message also both claim a restart of validator A that the script does not perform. Open **#666** describes the general class and its remediation would close this; nothing links the two. |

### #656 — _Only Bump Fees on Underpriced Transactions_ (closed by PR #686, commit `80f747c`)

The issue's stated goal was to avoid an "exponential death spiral on gas prices". The fix stopped bumping on unrelated RPC failures. The spiral remains reachable, and the fix introduced a second defect.

| Finding | Sev | What still reproduces |
| --- | --- | --- |
| [`F-CORE-060`](../findings/F-CORE-060.md) | High 98% | The retained underpriced path compounds ×1.1 **per block**, unbounded, and silently overrides `priority_fee_cap_percentage` — the one knob documented to bound overpayment. Measured live: tip 1 → 11,527 → 201,207 wei, max fee 4,239 gwei against a real base fee of 772 wei, the cap bypassed **~28,700×**. The only brake is the signer's balance. At Gnosis' ~5 s blocks that is ×3.1/minute. |
| [`F-CORE-061`](../findings/F-CORE-061.md) | Medium 58% | **Introduced by the fix.** #656 asked to bump when "we get a _transaction underpriced_ error from the node". Both regexes `80f747c` added require the rejection to be about a _replacement_. A first-submission rejection below the node's txpool floor matches neither, records no fee floor, and is re-signed at the same fee forever — and because the row holds an allocated nonce that is never released, every later transaction is blocked behind it. |

### #614 — _Evaluate Parallel Execution of Effects_ (closed by merging the non-blocking-effects epic, `19d3815`; epic implemented and deleted in `e346b94`; closed "done")

The issue body quotes an analysis with two horns. The head-of-line-blocking horn was fixed by going async. **The second horn was not, and it is what the audit reproduced with value moving:**

> _"A `NewRequest` event arriving while the dynamic check is still pending will be entirely ignored… The node will never vote on the proposal. **Recommendation:** You must introduce a `WaitingForDynamicCheck` variant to `RequestState`."_

The state variant exists (`RequestState::WaitingForEngineCheck`). The queuing the recommendation called for does not — events arriving in it are discarded, not held.

| Finding | Sev | What still reproduces |
| --- | --- | --- |
| [`F-SEN-001`](../findings/F-SEN-001.md) | High 98% | A replayed `Committed(self)` log arriving while the entry is back in `WaitingForEngineCheck` is discarded with a `warn`; `commit_vote` then rebuilds the entry with `self_committed: false`, the sentinel never reveals, and its bond is slashed. Measured on chain: **−4,000 fee tokens, 2,000 slashed**. An ordinary deploy restart is enough; the warp-ordering control test passed, so there is no race to win — the loss is unconditional. |
| [`F-SEN-002`](../findings/F-SEN-002.md) | High 98% | Peers' `Committed` logs seen before the engine answers are discarded, so `committed_count` under-counts, early finalisation fires with `self_revealed == false`, and the entry is deleted with no `Finalize` and no `Claim`. **4,500 left unclaimed.** No restart needed — a merely slower engine does it. |
| [`F-SEN-015`](../findings/F-SEN-015.md) | High 97% | The replayed check re-decides an already-committed vote; the second verdict overwrites the `reason` the commitment hash was built from, so `reveal` fails the onchain hash check or is never sent, and the bond is slashed. |

---

## 2. Mapping table

| Finding | Sev | Tracked by | Relationship | Note |
| --- | --- | --- | --- | --- |
| [`F-SEN-009`](../findings/F-SEN-009.md) | Low | `crates/sentinel/src/main.rs:45` TODO + #799 | ALREADY TRACKED | TODO and #799 bullet 1 ("accurate timeouts for sentinel engine requests") both name it. Audit adds the `voting_window ∈ {0,1}` silent-never-votes case and the `main.rs` / `config.rs` doc mismatch. |
| [`F-SEN-010`](../findings/F-SEN-010.md) | Info | `crates/sentinel/src/config.rs:44` TODO | ALREADY TRACKED | Audit adds that the guard is only against a _missing_ field — a present zero address starts cleanly. |
| [`F-XC-002`](../findings/F-XC-002.md) | Low | #113 | TRACKED BUT UNDERSTATED | #113's entire body is "- RPC Keys". The audit found FROST key shares and DKG secret polynomials reaching `warn!` — a level the default `log_filter = "info"` emits — through derived `Debug` on `Effect`/`Resume`. |
| [`F-VAL-062`](../findings/F-VAL-062.md) | Info | #113 | TRACKED BUT UNDERSTATED | `ReconcileGroupSecrets` is emitted on **every block** and carries every tracked epoch's key share; one transient SQLite error prints them all. |
| [`F-CORE-036`](../findings/F-CORE-036.md) | Low | #113 | TRACKED BUT UNDERSTATED | The `Debug` bound is on the `Service` trait, so no service can opt out; core prints effects and resumes at `trace` in five sites. #113 does not reach the framework. |
| [`F-VAL-061`](../findings/F-VAL-061.md) | High | #799 | PARTIALLY OVERLAPS | Shared: effect-handler lifecycle. New: **every** effect error maps to `Resume::Noop` with no retry, no back-off and no state marker — block awareness does not supply a retry path. |
| [`F-VAL-030`](../findings/F-VAL-030.md) | High | #799, #666 | PARTIALLY OVERLAPS | Shared: an effect whose result never arrives. New: the _phantom reservation_ is counted as 1024 nonces of capacity, so `handle_nonce_topup` never fires again and the validator silently skips up to 1024 signing ceremonies. |
| [`F-VAL-033`](../findings/F-VAL-033.md) | High | [flow-test epic](../../epics/2026_07_14_validator_state_machine_flow_test_harness.md), reorg row P0 | PARTIALLY OVERLAPS | Epic plans "branch burns a nonce for message A; alternate branch uses the same sequence for message B" — the in-process reorg. New: the same un-burn via an **operator database restore**, which the validator handbook instructs twice with no caveat. Harness is not implemented (Phase 9B). |
| [`F-VAL-004`](../findings/F-VAL-004.md) | High | #799 | PARTIALLY OVERLAPS | Shared: a lost effect. New: the _genesis_ rollover state has no deadline, no timeout arm and no retry, so one lost `KeyGenSetup` stalls the validator forever. |
| [`F-VAL-064`](../findings/F-VAL-064.md) | Medium | #820 | PARTIALLY OVERLAPS | Shares F-CORE-030's exit-0 defect; adds that `/health` is unreachable in the shipped deployment and the container runs as root. |
| [`F-VAL-066`](../findings/F-VAL-066.md) | Medium | #666, #801 | PARTIALLY OVERLAPS | Shared: reorg-unsafe secret pruning, in the exact function `f01a3ea` edited. New: the retention set is computed _before_ the block's logs and runs **concurrently** with the store writes those logs cause; before genesis the set is empty, degrading to a bare `DELETE FROM keygen_secrets` on every block. |
| [`F-CORE-031`](../findings/F-CORE-031.md) | Medium | #614, #799 | PARTIALLY OVERLAPS | Shared: async effect lifecycle. New: the snapshot recording an effect as pending is committed _before_ the effect is spawned, so a rollback onto the spawning block reverts the resume and never re-runs it — the runtime's documented at-least-once contract is not implemented. |
| [`F-CORE-033`](../findings/F-CORE-033.md) | Medium | #614 | PARTIALLY OVERLAPS | The parallel execution #614 asked for landed. New: it landed with no cap, no queue and no backpressure, while the transaction queue next to it is explicitly bounded at 16. One warp page can spawn a task per log. |
| [`F-SEN-003`](../findings/F-SEN-003.md) | Medium | #667 | PARTIALLY OVERLAPS | #667's fix (`dcc6fcf`) stopped the `MissingSnapshot` exit but did not touch the warp arm, which still returns `vec![]`: **no `Message::NewBlock` is produced for any warped block**, so no service FSM advances a deadline across the replayed range. Reveals are discarded, `finalize` takes the timeout branch, and a frozen request's bond is never claimed. |
| [`F-SEN-004`](../findings/F-SEN-004.md) | Medium | #614 | PARTIALLY OVERLAPS | Sentinel-side instance of F-CORE-033: no cap on concurrent engine checks, outstanding bonds or reveal throughput. |
| [`F-SEN-005`](../findings/F-SEN-005.md) | Medium | #549 | PARTIALLY OVERLAPS | Shared: funds stuck behind a timeout path nobody calls — #549's "user funds require manual intervention". New: the sentinel's own bond, because `WaitingForDisputeResolution` never expires and it never calls the permissionless `timeoutArbitration`. |
| [`F-VAL-003`](../findings/F-VAL-003.md) | Medium | #69 | PARTIALLY OVERLAPS | Shared: complaint-round accounting. New: no check that the plaintiff could have received a share, no per-plaintiff bound, no deadline — a participant that published nothing can compel a plaintext reveal. |
| [`F-VAL-067`](../findings/F-VAL-067.md) | Medium | #118, #69 | PARTIALLY OVERLAPS | Shared: FROSTCoordinator complaint semantics and their (missing) coverage. New: the Rust test counts complaints **cumulatively** while `FROSTParticipantMap` decrements on every `respond`, so the validator can abandon a keygen the coordinator still considers healthy. |
| [`F-CORE-032`](../findings/F-CORE-032.md) | Low | #799, #614 | PARTIALLY OVERLAPS | A panicking effect task is logged and skipped, silently removing a resume the state machine waits for — core's counterpart to F-VAL-061. |
| [`F-CORE-040`](../findings/F-CORE-040.md) | Low | #614 | PARTIALLY OVERLAPS | A cost of the driver `select!` loop the non-blocking-effects work produced: a wide fan-out is paid for in abandoned `eth_getLogs` calls. |
| [`F-SEN-011`](../findings/F-SEN-011.md) | Low | #614 | PARTIALLY OVERLAPS | Same async-check-versus-replay class as F-SEN-001: a restart orphans an in-flight check whose proposal predates the rollback anchor, and the request expires unvoted. |
| [`F-VAL-035`](../findings/F-VAL-035.md) | Low | #666 | PARTIALLY OVERLAPS | Shared: retired-group nonce erasure. New: abandoned chunks are never pruned, secret nonce material is copied into unzeroised JSON, and the erase path depends on an unasserted SQLite pragma. |
| [`F-VAL-038`](../findings/F-VAL-038.md) | Low | flow-test epic Phase 4 | PARTIALLY OVERLAPS | The epic's nonce-generation performance seam addresses test runtime; the finding is that production generation saturates every core and holds the shared SQLite writer for 1025 statements against the driver's own snapshot commits. |
| [`F-VAL-040`](../findings/F-VAL-040.md) | Low | #777 | PARTIALLY OVERLAPS | #777's sub-selection optimisation would remove the restart round this exploits. New: `last_signer` is overwritten by every accepted reveal and neither side deduplicates, so a signer can make itself "responsible" for restarting a stalled ceremony and then do nothing. |

---

## 3. Genuinely new

55 findings have no issue, TODO or epic. This is what the audit adds.

**Critical (1)** — [`F-VAL-001`](../findings/F-VAL-001.md) DKG encryption key `q` has no proof of possession; republishing a peer's `q` recovers their complete FROST signing share while the group finalises normally. Driven against real `FROSTCoordinator`/`FROSTParticipantMap` bytecode, 5/5 fresh seeds — the contracts block nothing. Neither #69 nor #20 touches `q`; nothing in the repo mentions a proof of possession.

**High (5)**

| ID | Claim |
| --- | --- |
| [`F-CORE-002`](../findings/F-CORE-002.md) | `use_client_filtering`'s log-completeness check disables itself after three failures — and the `IncompleteLogs` errors it raises are what exhaust the budget. Three HTTP 429s at the shipped default strip the integrity check off the next attempt. (#667 does not cover this.) |
| [`F-VAL-032`](../findings/F-VAL-032.md) | A `Sign` event whose sequence has no linked nonce chunk permanently discards the signing session. |
| [`F-VAL-039`](../findings/F-VAL-039.md) | The nonce top-up threshold gives ~100 sequences of headroom against a permissionless group-wide counter. |

---

## 4. Issues with no matching finding

| Issue | Why |
| --- | --- |
| #681, #670 | `explorer` — out of the audit's scope (`crates/core`, `crates/validator`, `crates/sentinel`, `crates/sentinel-engine`). |
| #669, #657 (closed) | Solidity/protocol changes; contracts were out of scope except as a reference oracle. |
| #785 (closed) | **Verified fixed.** `Provider::mocked` / `mocked_with_chain` and the `Asserter` import are behind `#[cfg(any(test, feature = "test-util"))]`. |
| #20 | Blob-storage redesign of KeyGen share distribution — a protocol change; no Rust finding touches the transport. |
| #546 | No direct finding. Adjacent: `F-SEN-002`'s failure is an under-counted local commitment tally, and #546's proposal — derive the count from the oracle's registered sentinel set — is the shape of the fix. Run 2 added a second instance of the same under-count, [`F2-SEN-010`](../findings/F2-SEN-010.md) (commits mined in the deadline block), introduced by #914; #546's proposal would close both. |

---

## 5. Run-2 counterparts (added after reconciliation)

Every row of Sections 1–3 with its run-2 counterpart and the combined status from [`RECONCILIATION.md`](RECONCILIATION.md) §2. The issue mapping of Sections 1–4 is unchanged; a run-2 counterpart that confirms a row confirms its mapping. "miss" means run 2 did not re-file the defect (still valid at `fe9e84c` unless stated).

| Run-1 row | Section | Run-2 counterpart(s) | Combined status |
| --- | --- | --- | --- |
| `F-CORE-001` | 1 (#820) | `F2-CORE-001` confirms | High 99, Confirmed |
| `F-CORE-030` | 1 (#820) | `F2-CORE-031`, `F2-VAL-066` confirm | Medium 85, Confirmed |
| `F-CORE-005` | 1 (#820) | `F2-CORE-008` confirms | Low 75, Confirmed |
| `F-VAL-005` | 1 (#801) | `F2-VAL-035`, `F2-VAL-031` (residuals) | **Fixed by #909/#910** for the reproduced trigger (13/13 store tests); residual D2 Informational `known`; the restart race is `F-VAL-066` Medium 78 |
| `F-CORE-060` | 1 (#656) | `F2-CORE-060` extends | High 98, Confirmed |
| `F-CORE-061` | 1 (#656) | `F2-CORE-062` extends | Medium 65, Plausible |
| `F-SEN-001` | 1 (#614) | `F2-SEN-001` confirms | High 99, Confirmed — unassigned |
| `F-SEN-002` | 1 (#614) | `F2-SEN-002` confirms; `F2-SEN-010` is a second source | High 98, Confirmed — assigned |
| `F-SEN-015` | 1 (#614) | `F2-SEN-001` (c), inference only | High 98, Confirmed — assigned |
| `F-SEN-009` | 2 | `F2-SEN-006` claim 6, `F2-SEN-008` claim 3 | Low 82, Confirmed, `known` |
| `F-SEN-010` | 2 | `F2-SEN-009` b1 (via `F-XC-009`) | Informational 85, Confirmed, `known` |
| `F-XC-002` | 2 | miss (soft) | Low 88, union counted under `F-VAL-062` and `F-CORE-036` |
| `F-VAL-062` | 2 | miss | Informational 88, Confirmed |
| `F-CORE-036` | 2 | examined, not filed | Low 85, Verified |
| `F-VAL-061` | 2 | `F2-VAL-030/031/062/063` (policy leg) | High 98, Confirmed |
| `F-VAL-030` | 2 | `F2-VAL-030`, `F2-VAL-062` extend | High 97, Confirmed |
| `F-VAL-033` | 2 | — | **Out of scope (A17)** |
| `F-VAL-004` | 2 | `F2-VAL-063`, `F2-VAL-005` extend | genesis instance accepted (A16, Informational `known`); rollover instance Medium 69, Plausible |
| `F-VAL-064` | 2 | `F2-VAL-066` (→ `F-CORE-030`), `F2-VAL-068` (→ `F-XC-004`) | Low 72, split |
| `F-VAL-066` | 2 | `F2-VAL-035` extends | Medium 78, Confirmed (changed shape by #909) |
| `F-CORE-031` | 2 | `F2-CORE-030` extends | **High 92**, Confirmed (raised) |
| `F-CORE-033` | 2 | `F2-CORE-035` extends | Medium 70, Confirmed; team question answered |
| `F-SEN-003` | 2 | `F2-SEN-003` (b) executes the route | Low 80, Confirmed (residual) |
| `F-SEN-004` | 2 | `F2-SEN-005` settles the trigger | Medium 78, Confirmed (was Plausible) |
| `F-SEN-005` | 2 | `F2-SEN-004` confirms | Medium 95, Confirmed — assigned |
| `F-VAL-003` | 2 | `F2-VAL-006` confirms | Medium 82, Confirmed |
| `F-VAL-067` | 2 | `F2-VAL-006` para. 2 | Low 70, Confirmed (was Medium 48) |
| `F-CORE-032` | 2 | miss | Low 45, Plausible |
| `F-CORE-040` | 2 | partial (cost missed) | Low 65, Plausible |
| `F-SEN-011` | 2 | mechanism in `F2-CORE-030` | Low 82, Confirmed |
| `F-VAL-035` | 2 | miss | Low 35, Observation |
| `F-VAL-038` | 2 | miss | Low 55, Plausible |
| `F-VAL-040` | 2 | `F2-VAL-033` confirms | Low 88, Confirmed (was 50) |
| `F-VAL-001` | 3 | `F2-VAL-001` confirms | Critical 97, Confirmed |
| `F-CORE-002` | 3 | `F2-CORE-002` confirms | High 99, Confirmed |
| `F-VAL-032` | 3 | `F2-VAL-032` extends | High 93, Confirmed — accepted |
| `F-VAL-039` | 3 | miss | High 58, Plausible |

Run-2 canonical findings with a relationship to existing work (the rest of the 18 are NEW in the sense of Section 3):

| Run-2 finding | Sev / cert | Tracked by | Relationship | Note |
| --- | --- | --- | --- | --- |
| [`F2-SEN-010`](../findings/F2-SEN-010.md) | High 93 | PR #914 (introduced), #546 (adjacent) | PARTIALLY OVERLAPS | second under-count source next to `F-SEN-002`; the STOPGAP note in `service.rs:390-402` ties it to safe-research/safenet#471 (#915) |
| [`F2-CORE-011`](../findings/F2-CORE-011.md) | Low 92 | #820 (closed by #834) | CLOSED BUT STILL PRESENT (class) | no rollback anchor on a fresh start; the fourth silent-continue path next to `F-CORE-001`/`030`/`005` |
| [`F2-CORE-007`](../findings/F2-CORE-007.md) | Low 75 | #820 | PARTIALLY OVERLAPS | restart against a lagging node exits with an opaque `BadUpdate` |
| [`F2-VAL-031`](../findings/F2-VAL-031.md) | Medium 80 | #799, #666; introduced by #909/#910 | PARTIALLY OVERLAPS | cold nonce generators after every restart; predicted by run 1 as `F-VAL-068` D1 |
| [`F2-VAL-034`](../findings/F2-VAL-034.md) | Informational 85 | #913 (deleted the devnet migration) | ALREADY TRACKED (`known`) | no schema-version check; a pre-#908 database fails every reconciliation |
| [`F2-SEN-003`](../findings/F2-SEN-003.md) | Medium 90 | #614 | PARTIALLY OVERLAPS | the same discard class as `F-SEN-002`; `finalize()` drops a bonded entry whenever our reveal was not observed |
| [`F2-XC-001`](../findings/F2-XC-001.md), [`F2-XC-002`](../findings/F2-XC-002.md), [`F2-XC-005`](../findings/F2-XC-005.md), [`F2-XC-007`](../findings/F2-XC-007.md), [`F2-XC-050`](../findings/F2-XC-050.md), [`F2-CORE-003`](../findings/F2-CORE-003.md), [`F2-CORE-067`](../findings/F2-CORE-067.md), [`F2-VAL-003`](../findings/F2-VAL-003.md), [`F2-VAL-004`](../findings/F2-VAL-004.md), [`F2-VAL-007`](../findings/F2-VAL-007.md), [`F2-VAL-064`](../findings/F2-VAL-064.md), [`F2-SEN-011`](../findings/F2-SEN-011.md) | — | none | NEW | no issue, TODO or epic found |

`known`-tagged items after both runs (A12): `F-CORE-065` (with `F2-CORE-066`), `F-SEN-009`, `F-SEN-010`, `F-CORE-067`'s tag from run 1, `F2-VAL-034`, the zero-address sub-item of `F-XC-009`/`F2-XC-008` (`config.rs:44` TODO), and the A16 genesis instances of `F-VAL-004`, `F-XC-050` and `F2-VAL-004` (b), plus `F-VAL-005`'s documented residual (`secrets/store.rs:38-41`).
