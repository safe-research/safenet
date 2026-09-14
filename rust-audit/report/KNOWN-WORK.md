# Known work — audit findings mapped onto existing issues, TODOs and epics

What the team already has a ticket for, and what it does not. Sources: 24 GitHub issues (16 open, 8 closed), 8 in-code `TODO`s, 3 epics in [`epics/`](../../epics/). Findings are the 108 in [`../findings/`](../findings/); see [`REPORT.md`](REPORT.md) for severities and evidence.

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
| [`F-ENG-030`](../findings/F-ENG-030.md) | Critical | [verdict-composition epic](../../epics/2026_09_04_sentinel_verdict_composition.md), Phase 3 | ALREADY TRACKED | The epic states the fix verbatim — `NestedSafeChecker` claims `To\|Data\|Operation`, "it does not inspect `value`, so it no longer affirms a nested `execTransaction` carrying native value". Audit adds the Critical severity and 1000 ETH drained off a real Safe 1.5.0 proxy. |
| [`F-ENG-044`](../findings/F-ENG-044.md) | High | #817 + verdict-composition epic | ALREADY TRACKED | The issue quotes the exact combinator. The epic is the designed fix. Audit adds executed proof that `[Secure, denial]` returns `Secure` in the production chain. |
| [`F-ENG-001`](../findings/F-ENG-001.md) | Medium | #825 | ALREADY TRACKED | #825 is precisely "make the sentinel engine actually implement the base checks as specified in the charter". |
| [`F-ENG-003`](../findings/F-ENG-003.md) | Medium | #825 | ALREADY TRACKED | Same scope: R-4.2 restated as a target allow-list that admits migrations the Charter does not except. |
| [`F-ENG-039`](../findings/F-ENG-039.md) | Medium | #825 | ALREADY TRACKED | The issue's literal subject — "it allows configuration changes, but should not allow them". |
| [`F-ENG-006`](../findings/F-ENG-006.md) | Low | [batching epic](../../epics/2026_09_04_sentinel_batch_meta_transactions.md) | ALREADY TRACKED | Epic names the unbounded `decode_target_effects` recursion and introduces `MAX_BATCH_DEPTH`. |
| [`F-SEN-009`](../findings/F-SEN-009.md) | Low | `crates/sentinel/src/main.rs:45` TODO + #799 | ALREADY TRACKED | TODO and #799 bullet 1 ("accurate timeouts for sentinel engine requests") both name it. Audit adds the `voting_window ∈ {0,1}` silent-never-votes case and the `main.rs` / `config.rs` doc mismatch. |
| [`F-XC-052`](../findings/F-XC-052.md) | Low | batching epic, Phase 1 | ALREADY TRACKED | "Deleting the zeroed-`SafeTransaction` synthesis" is an explicit deliverable. |
| [`F-ENG-040`](../findings/F-ENG-040.md) | Info | `checkers/base.rs:198` TODO + batching epic Phase 4 | ALREADY TRACKED | TODO states the mis-citation and why it was deferred; Phase 4 resolves it. |
| [`F-ENG-041`](../findings/F-ENG-041.md) | Info | `checkers/address_poisoning.rs:303` TODO | ALREADY TRACKED | TODO describes the first-time-recipient abstention exactly. |
| [`F-SEN-010`](../findings/F-SEN-010.md) | Info | `crates/sentinel/src/config.rs:44` TODO | ALREADY TRACKED | Audit adds that the guard is only against a _missing_ field — a present zero address starts cleanly. |
| [`F-ENG-031`](../findings/F-ENG-031.md) | Critical | #817 + `checkers/refund.rs:83`/`:93` TODOs + epic follow-ups F6/F7 | TRACKED BUT UNDERSTATED | The TODO names the drain exactly ("can now drain unbounded native currency to `refundReceiver`"), but it is an in-code note with no issue and no severity, and epic Phase 4 _keeps_ both holes as abstentions while deferring the amount policy to F7 — a follow-up that is only "track as: a GitHub issue" and does not yet exist. Audit reproduced **100.0003 ETH and 0.503 tokens** actually paid out. |
| [`F-ENG-033`](../findings/F-ENG-033.md) | Critical | verdict-composition epic Phase 5 + follow-up F2 | TRACKED BUT UNDERSTATED | Phase 5 fixes the `value` half. The forged-history half is recorded only as F2, a deferred "doc comment plus an issue" item describing the ERC-20 inference as "reasonable" and merely unverified. Audit forged a `Transfer` from the attacker's own EOA on their own non-token contract, got _"genuine prior interaction found"_ → `secure`, and **drained 1000 ETH**. |
| [`F-ENG-032`](../findings/F-ENG-032.md) | Medium 99% | batching epic, Overview | TRACKED BUT UNDERSTATED | The epic names the exact trap in the conditional — a check "can silently read `chain_id == 0` and reject it — which is exactly what `AddressPoisoningChecker`'s chain-id guard **would** do if it were ever given one". It misses that `RefundChecker` already synthesises with `chain_id = 0`, so the checker has been **dead since it was written**, not hypothetically at risk. |
| [`F-ENG-037`](../findings/F-ENG-037.md) | High 96% | #651 | TRACKED BUT UNDERSTATED | #651 asks to refactor the approved-amount relation away from an exact match because "this doesn't always make sense" — a usability ticket. The tolerance that shipped (`total + (n-1)`, PR #838) is sized by attacker-supplied `n`: `partSellAmount = 0`, `n = U256::MAX` makes `approve(relayer, 2^256-2)` return `secure`. |
| [`F-ENG-005`](../findings/F-ENG-005.md) | Low | `crates/sentinel-engine/src/api/mod.rs:48` TODO | TRACKED BUT UNDERSTATED | TODO covers only "pass `x-request-timeout` to the engine". Nothing tracks the absent server timeout, the absent concurrency limit, or the two outbound clients built with no timeout at all. |
| [`F-XC-002`](../findings/F-XC-002.md) | Low | #113 | TRACKED BUT UNDERSTATED | #113's entire body is "- RPC Keys". The audit found FROST key shares and DKG secret polynomials reaching `warn!` — a level the default `log_filter = "info"` emits — through derived `Debug` on `Effect`/`Resume`. |
| [`F-VAL-062`](../findings/F-VAL-062.md) | Info | #113 | TRACKED BUT UNDERSTATED | `ReconcileGroupSecrets` is emitted on **every block** and carries every tracked epoch's key share; one transient SQLite error prints them all. |
| [`F-CORE-036`](../findings/F-CORE-036.md) | Low | #113 | TRACKED BUT UNDERSTATED | The `Debug` bound is on the `Service` trait, so no service can opt out; core prints effects and resumes at `trace` in five sites. #113 does not reach the framework. |
| [`F-VAL-061`](../findings/F-VAL-061.md) | High | #799 | PARTIALLY OVERLAPS | Shared: effect-handler lifecycle. New: **every** effect error maps to `Resume::Noop` with no retry, no back-off and no state marker — block awareness does not supply a retry path. |
| [`F-VAL-030`](../findings/F-VAL-030.md) | High | #799, #666 | PARTIALLY OVERLAPS | Shared: an effect whose result never arrives. New: the _phantom reservation_ is counted as 1024 nonces of capacity, so `handle_nonce_topup` never fires again and the validator silently skips up to 1024 signing ceremonies. |
| [`F-VAL-033`](../findings/F-VAL-033.md) | High | [flow-test epic](../../epics/2026_07_14_validator_state_machine_flow_test_harness.md), reorg row P0 | PARTIALLY OVERLAPS | Epic plans "branch burns a nonce for message A; alternate branch uses the same sequence for message B" — the in-process reorg. New: the same un-burn via an **operator database restore**, which the validator handbook instructs twice with no caveat. Harness is not implemented (Phase 9B). |
| [`F-ENG-034`](../findings/F-ENG-034.md) | High | verdict epic Phase 3, #793 | PARTIALLY OVERLAPS | Shared: the ordering half — under the epic's fold a `Secure` no longer masks `BlocklistChecker`'s denial. New: the checker still places **no constraint on `to`**, while the Guard's `_isAutoAllowed` requires `to == address(this)`; the epic hands it `ACTION` coverage including `To`. |
| [`F-ENG-035`](../findings/F-ENG-035.md) | High | batching epic Phase 5 | PARTIALLY OVERLAPS | Shared: MultiSend sub-call destinations ("closes a live gap"). New: ERC-20 payees, approval spenders, `gas_token`, `refund_receiver`, and the inner `to` of a nested `execTransaction` — which the epic deliberately does not flatten. |
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
| [`F-ENG-009`](../findings/F-ENG-009.md) | Low | verdict epic follow-up F4 | PARTIALLY OVERLAPS | Shared: per-request RPC fan-out, which F4 exists to bound after the epic removes the early exit. New: `EngineConfig` validates nothing, so the lookback × max-range pair sets the fan-out with no bound and no startup log **today**, before the epic lands. |
| [`F-ENG-038`](../findings/F-ENG-038.md) | Low | #651 | PARTIALLY OVERLAPS | Shared: CoW order recognition. New: the shape recognisers are strictly looser than their paired decoders, so a decoy trigger turns a dangling relayer approval from `insecure` into `abstain`. |
| [`F-ENG-004`](../findings/F-ENG-004.md) | Info | #825 | PARTIALLY OVERLAPS | Same file and same work, but two wrong Charter citations rather than wrong behaviour — separable and zero-risk. |
| [`F-SEN-011`](../findings/F-SEN-011.md) | Low | #614 | PARTIALLY OVERLAPS | Same async-check-versus-replay class as F-SEN-001: a restart orphans an in-flight check whose proposal predates the rollback anchor, and the request expires unvoted. |
| [`F-VAL-035`](../findings/F-VAL-035.md) | Low | #666 | PARTIALLY OVERLAPS | Shared: retired-group nonce erasure. New: abandoned chunks are never pruned, secret nonce material is copied into unzeroised JSON, and the erase path depends on an unasserted SQLite pragma. |
| [`F-VAL-038`](../findings/F-VAL-038.md) | Low | flow-test epic Phase 4 | PARTIALLY OVERLAPS | The epic's nonce-generation performance seam addresses test runtime; the finding is that production generation saturates every core and holds the shared SQLite writer for 1025 statements against the driver's own snapshot commits. |
| [`F-VAL-040`](../findings/F-VAL-040.md) | Low | #777 | PARTIALLY OVERLAPS | #777's sub-selection optimisation would remove the restart round this exploits. New: `last_signer` is overwritten by every accepted reveal and neither side deduplicates, so a signer can make itself "responsible" for restarting a stalled ceremony and then do nothing. |
| [`F-XC-010`](../findings/F-XC-010.md) | Low | verdict epic Phase 7 | PARTIALLY OVERLAPS | Phase 7 creates `crates/sentinel-engine/src/metrics.rs` (the crate has none) for **one** coverage metric. Nothing tracks checker, verdict or failure metrics — which is why F-ENG-032, a checker dead since it was written, is invisible in production. |

---

## 3. Genuinely new

55 findings have no issue, TODO or epic. This is what the audit adds.

**Critical (1)** — [`F-VAL-001`](../findings/F-VAL-001.md) DKG encryption key `q` has no proof of possession; republishing a peer's `q` recovers their complete FROST signing share while the group finalises normally. Driven against real `FROSTCoordinator`/`FROSTParticipantMap` bytecode, 5/5 fresh seeds — the contracts block nothing. Neither #69 nor #20 touches `q`; nothing in the repo mentions a proof of possession.

**High (5)**

| ID | Claim |
| --- | --- |
| [`F-CORE-002`](../findings/F-CORE-002.md) | `use_client_filtering`'s log-completeness check disables itself after three failures — and the `IncompleteLogs` errors it raises are what exhaust the budget. Three HTTP 429s at the shipped default strip the integrity check off the next attempt. (#667 does not cover this.) |
| [`F-ENG-002`](../findings/F-ENG-002.md) | R-4.5 is implemented as an unconditional denial of `setApprovalForAll`; the Charter makes operator approval-for-all conditional, so the engine denies standard NFT-marketplace approvals. Opposite direction from #825. |
| [`F-ENG-036`](../findings/F-ENG-036.md) | R-4.5 is an exact `U256::MAX` comparison, so `approve(X, 2^256-2)` evades it. |
| [`F-VAL-032`](../findings/F-VAL-032.md) | A `Sign` event whose sequence has no linked nonce chunk permanently discards the signing session. |
| [`F-VAL-039`](../findings/F-VAL-039.md) | The nonce top-up threshold gives ~100 sequences of headroom against a permissionless group-wide counter. |

**Medium (18)** — `F-CORE-067` (no replay contract on `Command::Action`; every rollback replay enqueues duplicate onchain transactions — canonical for `F-VAL-065`/`F-SEN-006`), `F-XC-005` (sample engine config silently disables the address-poisoning check), `F-VAL-002` (ECDH share pad is an unhashed x-coordinate used in both directions), `F-CORE-062` (an allocated nonce is never released, wedging the queue permanently), `F-CORE-063`, `F-CORE-064`, `F-CORE-066`, `F-CORE-003`, `F-CORE-004`, `F-CORE-011`, `F-CORE-012`, `F-CORE-034`, `F-CORE-035`, `F-ENG-042`, `F-VAL-060`, `F-VAL-063`, `F-VAL-065`, `F-XC-050`.

**Low (24)** — `F-XC-011` (four live RUSTSEC advisories; exactly one, `h2`, is reachable from a network-facing surface — and it is not the highest CVSS), `F-SEN-006`, `F-SEN-007`, `F-SEN-008`, `F-SEN-012`, `F-CORE-006`, `F-CORE-007`, `F-CORE-008`, `F-CORE-009`, `F-CORE-010`, `F-CORE-037`, `F-CORE-039`, `F-CORE-065`, `F-ENG-007`, `F-ENG-043`, `F-VAL-031`, `F-VAL-034`, `F-VAL-036`, `F-XC-003`, `F-XC-004`, `F-XC-006`, `F-XC-008`, `F-XC-009`, `F-XC-051`.

**Informational (7)** — `F-CORE-038`, `F-ENG-008`, `F-SEN-014`, `F-VAL-037`, `F-XC-001` (no `[profile.release]` at all, so overflow checks and debug assertions are off in every shipped binary), `F-XC-007`, and `F-SEN-013` (**refuted by execution** — `alloy-sol-types` 1.6.0 decodes invalid UTF-8 lossily, so the predicted indexer stall does not occur).

---

## 4. Issues with no matching finding

| Issue | Why |
| --- | --- |
| #681, #670 | `explorer` — out of the audit's scope (`crates/core`, `crates/validator`, `crates/sentinel`, `crates/sentinel-engine`). |
| #669, #657 (closed) | Solidity/protocol changes; contracts were out of scope except as a reference oracle. |
| #785 (closed) | **Verified fixed.** `Provider::mocked` / `mocked_with_chain` and the `Asserter` import are behind `#[cfg(any(test, feature = "test-util"))]`. |
| #20 | Blob-storage redesign of KeyGen share distribution — a protocol change; no Rust finding touches the transport. |
| #465 | No finding, but the audit's open question **Q-ENG-A** confirms the gap the issue describes: `Asserter::is_empty` does not exist in `alloy-transport` 2.0.5, and `F-ENG-032`'s PoC had to reach for `read_q.is_empty()` to prove no RPC was issued. |
| #546 | No direct finding. Adjacent: `F-SEN-002`'s failure is an under-counted local commitment tally, and #546's proposal — derive the count from the oracle's registered sentinel set — is the shape of the fix. |
| #793 | Adjacent to `F-ENG-034`, but the audit did not assess the Guard's `_isAutoAllowed` path or the 1-of-N phishing surface the issue is actually about; that is contract- and product-level. Worth noting `F-ENG-034` makes the concern worse: even announcement-shaped calls that _do_ reach the sentinel are rated `secure` regardless of `to`. |
