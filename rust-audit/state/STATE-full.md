# Audit state

| Field | Value |
| --- | --- |
| Commit | 2893917757ae518ebb91154712cf3e401cb68d33 (`AI review changes`) |
| Started |  |
| Mode | phases 0-4 read-only; **phase 5 executing** (toolchain installed) |
| Phase | **COMPLETE** - 8 phases; findings validated end-to-end on local Anvil |
| Gate status | **CLOSED - run finished; 8 phases, report signed off** |

Runtime: Claude Code, Claude Opus 5 (1M context), Agent-tool subagents, no Workflow. Operator instruction: run the prompt; **no commits, no branches, no PRs**.

## Assumptions confirmed

All fifteen signed off at Gate 0 on (operator answers recorded below). A8 and A9 are forced FALSE by the environment.

| ID | State | Note |
| --- | --- | --- |
| A1 | [x] TRUE | Trusted operator; plaintext secrets at rest documented in `docs/validator-handbook.md`. |
| A2 | [x] TRUE | Adversarial chain data within the <1/3 fault bound; Safe tx contents attacker-controlled. |
| A3 | [x] TRUE | operator confirmed: engine API reachable only by the co-deployed sentinel. Missing auth/rate-limiting stays Informational unless a bypass exists inside that deployment. |
| A4 | [x] TRUE | operator confirmed: malicious RPC **out of scope**. Stale, rate-limited and incomplete `eth_getLogs` results remain in scope. |
| A5 | [x] TRUE | Reorgs to `max_reorg_depth` handled; deeper = deliberate exit (PR #834). |
| A6 | [x] TRUE | Crypto libraries trusted; review Safenet's usage/adaptations only. Note: dependency sources are NOT on disk (no cargo registry), so upstream behaviour claims stay class `I`. |
| A7 | [x] TRUE | Solidity in `contracts/src` is audited and is the reference for hashing/encoding/protocol rules. |
| A8 | [x] FALSE (forced) | `sentinel-test-vectors` not available AND no toolchain to run it. Engine checker findings cannot reach `E1`. |
| A9 | [x] FALSE | Evidence: `cargo`, `rustc`, `rustup`, `forge`, `anvil`, `just` absent; 3 GB RAM. Run downgraded to **read-only**; the report must say so. |
| A10 | [x] TRUE | Gnosis Chain ~5 s blocks; documented defaults per `docs/overview.md` and sample configs. |
| A11 | [x] TRUE | Scope is exactly PROMPT.md Section 4. |
| A12 | [x] TRUE | Known TODOs (codebase-map Section 4) + validator flow-test epic reported tagged `known`, reduced priority. |
| A13 | [x] TRUE | Confirmed by the operator in the start message: no branches, no commits, no PRs. |
| A14 | [x] TRUE | `git diff 82b3e0d..HEAD` touches only `rust-audit/`; no drift in `crates/`, `Cargo.toml`, `Cargo.lock`. Map line numbers valid. |
| A15 | [x] TRUE | Operator supplied the public Charter (github.com/safe-research/safenet-charter). Cloned read-only to the session scratchpad at commit `44a1e53`: `<scratch>/safenet-charter/Safenet_Arbitration_Charter.md`, 909 lines, defining R-4.1 .. R-4.6 — the exact set `engine/rule.rs` cites. Verdict-policy findings (ENG-H2..H7) **may** reach Confirmed. Cite as `safenet-charter@44a1e53:Safenet_Arbitration_Charter.md:<lines>` with a verbatim quote, since the path is session-local. |

## Agents

| Agent | Role | Assignment | Status | Output paths |
| --- | --- | --- | --- | --- |
| Recon | Recon | baseline, inventory, lockfile dupes | **done** | `state/baseline.md` (407 lines), `state/logs/` (11 logs) |
| R1 | Reviewer | core indexing + reorgs, 4,106 lines, IDs F-CORE-001..029 | **done** (9 findings, 19 hypotheses rejected, 8 observations) | `../state/coverage-logs.md#r1` |
| R2 | Reviewer | core runtime/state/effects/observability, 1,998 lines, IDs F-CORE-030..059 | **done** (10 findings, CORE-H10 + M10 refuted, 11 observations) | `../state/coverage-logs.md#r2` |
| R3 | Reviewer | core transaction queue, 1,540 lines, IDs F-CORE-060..089 | **done** (7 findings, 28 hypotheses rejected, 7 observations) | `../state/coverage-logs.md#r3` |
| R4 | Reviewer | validator DKG path, 3,228 lines, IDs F-VAL-001..029 | **done** (4 findings, M1 refuted, 10 observations) | `../state/coverage-logs.md#r4` |
| R5 | Reviewer | validator signing + secrets, 2,802 lines, IDs F-VAL-030..059 | **done** (9 findings, M4/M5/M7 refuted, 67 excerpts byte-verified) | `../state/coverage-logs.md#r5` |
| R6 | Reviewer | validator service/wiring/config, 2,068 lines, IDs F-VAL-060..089 | **done** (7 findings, 19 hypotheses refuted, 8 observations) | `../state/coverage-logs.md#r6` |
| R7 | Reviewer | sentinel (all), 3,348 lines, IDs F-SEN-001..049 | **done** (14 findings, M8 refuted, 13 observations) | `../state/coverage-logs.md#r7` |
| R8 | Reviewer | engine API/chain/decoding, 1,642 lines, IDs F-ENG-001..029 | **done** (9 findings, 4 of them Charter mismatches) | `../state/coverage-logs.md#r8` |
| R9 | Reviewer | engine checkers, 3,471 lines, IDs F-ENG-030..069 | **done** (14 findings inc. 3 Critical, all 9 seeded leads confirmed, 20 rejected) | `../state/coverage-logs.md#r9` |
| C-CORE-A | Critic | R1's 9 findings F-CORE-001..009 + R1's 19 rejected hypotheses; promote into F-CORE-010..029 | **done**: 9/9 critiqued, promoted F-CORE-010/011/012 | appended `## Critic` sections |
| C-CORE-B | Critic | R2+R3's 17 findings + both rejected lists + the CORE-H5 seam | **done**: 8 Confirmed, 8 Plausible, 0 Refuted, **0 `H`**; promoted F-CORE-040 and F-CORE-067 | `## Critic` sections |
| C-VAL-A | Critic | R4's 4 findings F-VAL-001..004 + rejected list | **done**: 4 Confirmed, 0 Refuted, **0 `H` claims** (all 41 citations verbatim-accurate), 1 promoted | `## Critic` sections + `findings/F-VAL-005.md` |
| C-VAL-B | Critic | R5+R6's 16 findings + both rejected lists + the dangling VAL observations | **done**: 8 Confirmed, 8 Plausible, 0 Refuted, 1 `H` struck; 6 severities corrected; promoted F-VAL-039/040/067 | `## Critic` sections |
| C-SEN | Critic | R7's 14 findings F-SEN-001..014 + rejected list; promote into F-SEN-015..049 | **done**: 14/14 critiqued, promoted F-SEN-015 | appended `## Critic` sections |
| C-ENG-A | Critic | R8's 9 findings F-ENG-001..009 + rejected list; promote into F-ENG-010..029 | **done**: 9/9 critiqued | appended `## Critic` sections |
| C-ENG-B | Critic | R9's 14 findings F-ENG-030..043 (**3 Criticals**) + rejected list | **done**: 14 Confirmed, 0 Refuted, **1 `H`** struck; 5 severities re-judged; promoted F-ENG-044 | `## Critic` sections + `findings/F-ENG-044.md` |
| C-XC | Critic | R10's 9 findings F-XC-001..009 + rejected list | **done**: 8 Confirmed, 2 Plausible, **0 `H`** in R10's own text; promoted F-XC-010; caught an `H` in another Critic's file | `## Critic` sections + `findings/F-XC-010.md` |
| Coverage | Coverage Critic | full 83-file matrix, reviewer seams, unexamined leads, toolchain-blocked questions | **done**: `coverage.md` 423 lines, **zero uncovered files**, 4 uncovered seams, 22 blocked questions; filed F-XC-050/051/052 | `state/coverage.md` |
| R10 | Reviewer | cross-cutting: manifests, Dockerfiles, configs, secret/panic/dep sweeps, IDs F-XC-001..049 | **done** (9 findings, 10 observations, census re-derived) | `../state/coverage-logs.md#r10` |

## Findings

Certainty column is the **reviewer self-estimate** until a Critic sets it in Phase 2.

| ID | Title | Status | Severity | Certainty |
| --- | --- | --- | --- | --- |
| F-CORE-001 | Snapshots store only block numbers, so a **downtime reorg silently defeats `max_reorg_depth`**; pruning puts the retained anchor at exactly the depth that is fatal while running, so the deliberate exit + restart is self-defeating (CORE-H1) | Draft | High | 85% |
| F-CORE-002 | `use_client_filtering`'s bloom check is gated on `retries < 3` and its own `IncompleteLogs` errors exhaust the budget; attempt 4 accepts an empty node-filtered result as complete (CORE-H2, and `validator-handbook.md:33-35` documents the failure mode) | Draft | High | 85% |
| F-CORE-003 | `revalidate_last_block` flattens `Option` before the hash compare, so a lagging backend's `null` becomes a spurious uncle + rollback + replay (CORE-H9 raised) | Draft | Medium | 75% |
| F-CORE-004 | No terminal error state: one `LOG1` at a configured oracle address whose topic0 is a watched selector **stalls indexing forever, deterministically, across every validator**, with `/health` OK (CORE-H8 escalated) | Draft | High | 80% |
| F-CORE-005 | `max_reorg_depth = 0` leaves `recent` permanently empty, so the `-32001` recovery can never fire and loops instead of failing loudly (CORE-H14) | Draft | Low | 80% |
| F-CORE-006 | Address x topic cross-product matching; decode ignores the emitter (CORE-H4; validator-side severity left to R6/VAL-H2) | Draft | Medium | 80% |
| F-CORE-007/008/009 | Unbounded undelayed mid-init rescan (new); wall-clock block scheduling causing permanent silent lag (new argument); block-watcher config with no range validation incl. `start_block > head` silently ignored (new) | Draft | Low | — |
| F-CORE-030 | `Driver::run` returns ``, so every fatal error exits status 0; `/health` is liveness-only and has no consumer in the repo (CORE-H3) | Draft | — | 85% |
| F-CORE-031 | **An effect is spawned only after the snapshot recording it, and a resume is persisted only in a later snapshot, so any rollback landing on the spawning block reverts the resume and never re-runs the effect** — hits on every restart and any reorg; the sentinel then silently drops the request without voting | Draft | — | 80% |
| F-CORE-032/033 | Panicked effect tasks logged and skipped (a test pins the behaviour); unbounded effect concurrency — one 100-block warp page spawns a task per log | Draft | — | 70% |
| F-CORE-034 | Fixed 100 ms retry forever + one warn line per attempt: retry storm, log flood, deterministic errors never escalate (M9) | Draft | — | 80% |
| F-CORE-035 | Every `TransportError` including JSON-RPC error responses is treated as intermittent and swallowed forever, so onchain action stops while all metrics keep advancing (M10) | Draft | — | 75% |
| F-CORE-036/037 | `Debug` required on `Effect`/`Resume` and printed in five `trace` sinks, validator `Secrets` a plain derive; unversioned snapshot JSON with no migration or decode-failure recovery | Draft | — | 60-75% |
| F-CORE-038/039 | `kdf` info parts concatenate — safe today, doc misleads (CORE-H17, Informational); shutdown branch unreachable during `update`'s un-timed RPC calls (CORE-H12) | Draft | — | 65-85% |
| F-CORE-060 | Underpriced-rejection fee ratchet unbounded, runs every block, bypasses `priority_fee_cap_percentage` | Draft | — | 75% |
| F-CORE-061 | `is_transaction_underpriced` matches only replacement rejections; first-submission rejection retries forever at an unchanged fee and head-of-line blocks later nonces | Draft | — | 60% |
| F-CORE-062 | An allocated nonce is never released; one bad nonce wedges the queue permanently across restarts with no error, metric or recovery path | Draft | — | 55% |
| F-CORE-063 | Execution inferred from the account nonce but invalidated only by a block-number regression; a displaced transaction is marked executed, pruned and silently lost | Draft | — | 60% |
| F-CORE-064 | `expires_at` is void once a nonce is allocated, contradicting `queue`'s own doc contract; deadline-bearing actions land arbitrarily late at escalating fees | Draft | — | 70% |
| F-CORE-065 | No chain-id/signer binding on `transactions`; `get_chain_id` serves a connect-time cache, so a chain or key change is undetectable (`known`, extends CORE-H17) | Draft | — | 65% |
| F-CORE-066 | `tx::Config` accepts `max_in_flight_transactions = 0`, `blocks_before_resubmit = 0`, and `nan`/negative fee cap | Draft | — | 75% |
| **F-VAL-001** | **No proof of possession on the DKG encryption key `q`: an attacker republishes an honest peer's `q`, files a complaint against each other peer during the deadline-free sharing round, harvests every pad from the unconditional plaintext responses, and reconstructs the victim's complete FROST signing share — in a group that finalizes normally** | Draft | **Critical** | 85% |
| F-VAL-002 | Pad is an unhashed x-coordinate used in both directions, so each pad encrypts two shares and a complaint response also exposes the plaintiff's share; contradicts `docs/overview.md:48`. Root cause of F-VAL-001 | Draft | Medium | 88% |
| F-VAL-003 | Complaint policy: no check the plaintiff ever received a share, no per-plaintiff bound, no deadline in `CollectingShares` | Draft | Medium | 75% |
| F-VAL-030 | Lost/failed `NonceTree` effect leaves a durable phantom chunk reservation that `available` counts as 1024, so top-ups never re-fire; trigger is a restart while the effect is in flight (VAL-H3) | Draft | High | 75% |
| F-VAL-031 | A dead nonce worker thread is never joined, logged or replaced, so `NonceTree` fails forever for that group | Draft | Medium | 60% |
| F-VAL-032 | `handle_sign` removes the session before resolving the nonce and never re-inserts it on the unlinked-chunk arm; rollover packets unrecoverable (VAL-H6) | Draft | Medium | 70% |
| F-VAL-033 | **A restore spanning a reorg reuses a burned nonce for a second message**; no consumed-sequence ledger exists (VAL-H11/M5 re-derived) | Draft | Medium | 55% |
| F-VAL-034/035/036/037/038 | Nonce-store and Merkle hardening: M6+M7 absorbed (035), M4 resolved as Informational (037) | Draft | Low/Info | 55-80% |
| F-ENG-001 | `RuleId::R4_1SettingsChange`'s doc states an allow-list far wider than Charter R-4.1's two-function exception (which also requires `value == 0` and non-batched), and `base.rs` implements the doc — Charter-insecure owner/threshold/guard/module changes get `abstain`, not `insecure` | Draft | Medium | 80% |
| F-ENG-002 | R-4.5's doc claims `setApprovalForAll` is an unconditional failure "per §2.5"; the Charter makes it conditional, so the engine deterministically casts a **denying** vote on standard NFT-marketplace approvals (wrong-vote / slashing exposure; arguably High if such traffic exists) | Draft | Medium | 80% |
| F-ENG-003/004 | R-4.2's doc restates a storage-effect rule as a target allow-list, dropping the `signedMessages` exception; R-4.3's verbatim quote is attributed to a §2.4 Notes passage that does not contain it | Draft | Medium/Info | 70-80% |
| F-ENG-005 | No deadline anywhere: `x-request-timeout` discarded, `tower`/`tower-http` timeout, limit and catch-panic middlewares not compiled in, both outbound clients untimed, and the proposer picks which one runs (`known`) | Draft | Medium | 90% |
| F-ENG-006/007 | Unbounded recursion in `decode_target_effects` (reachability **refuted** — max depth 2, but the shield is undocumented and untested); shutdown drops the serve future instead of draining, so in-flight checks become lost votes on every deploy | Draft | Low | 85% |
| F-ENG-008/009 | `openapi.yaml` documents only `200` while the extractors provably return `400`s; `EngineConfig` unvalidated — the lookback/max-range ratio silently sets `eth_getLogs` fan-out (25,001 calls with plausible values) | Draft | Info/Low | 75-90% |
| **F-ENG-030** | **`NestedSafeChecker` affirms any `execTransaction`-shaped `Call` regardless of `value`: a full ETH drain is rated `secure`** | Draft | **Critical** | 88% |
| **F-ENG-031** | **Four affirmers never read the refund leg; a pure, RPC-free `StakingChecker` `claim(safe,..)` + `baseGas=1e12` native drain is rated `secure`** (`known`, refund.rs:83/:93) | Draft | **Critical** | 90% |
| **F-ENG-033** | **Address poisoning affirms from history on an attacker-chosen `to` and ignores `value`** | Draft | **Critical** | 85% |
| F-ENG-032 | `RefundChecker` is dead: the synthetic transfer has `chainId=0` so the delegated check always abstains (ENG-H1 confirmed) | Draft | High | 93% |
| F-ENG-034/035 | Escape hatch affirms for any `to` before the blocklist (vs `_isAutoAllowed` / Charter §2.18); blocklist sees only top-level `to`, and a flagged recipient with prior history gets `secure` | Draft | High | 82-84% |
| F-ENG-036/037/038 | R-4.5 is `== U256::MAX` only, with no `increaseAllowance` decoding; CoW TWAP tolerance sized by attacker-chosen `n`; CoW recognisers looser than their decoders | Draft | High/Med/Low | 80-84% |
| F-ENG-039 | `BaseChecker`'s R-4.1/R-4.2 allow-lists far exceed the Charter's two exceptions (owner/threshold/`setGuard(0)`/`enableModule`/migrations, batched, non-zero `value`) | Draft | Medium | 78% |
| F-ENG-040/041/042/043 | Two `known` items; false `insecure` from recency- and truncation-bounded evidence (new); untimed client overlapping ENG-H10 | Draft | Med/Low | 70-95% |
| F-SEN-001 | **Restart/reorg replay discards our own `Committed`, so no `Reveal` is sent and the bond is slashed** | Draft | High | 80% |
| F-SEN-002 | Commits seen before the engine verdict are discarded; early finalise drops the entry without ever claiming | Draft | High | 85% |
| F-SEN-003 | Warp delivers no `NewBlock`, so reveals are discarded into the timeout branch and a frozen request's bond is never claimed | Draft | Medium | 70% |
| F-SEN-004 | No bound on concurrent checks, outstanding bonds or reveal throughput | Draft | Medium | 60% |
| F-SEN-005 | `WaitingForDisputeResolution` never expires; no `timeoutArbitration` binding | Draft | Medium | 90% |
| F-SEN-006/007/008 | Non-idempotent action replay; no balance/registration/chain pre-checks (silent gas burn); 55k `approve` gas + unconditional non-zero approve | Draft | Low-Medium | 50-90% |
| F-SEN-009/010 | Engine timeout from unvalidated `voting_window`; sample zero addresses — both tagged `known` | Draft | — | — |
| F-SEN-011/012/014 | Restart orphans an in-flight check; single-attempt engine client; finalize storm | Draft | Low/Info | 75-95% |
| F-SEN-013 | Non-UTF-8 `Revealed.reason` may stall every indexer — **pivotal `alloy` claim is class `I`; QA must settle it first** | Draft | — | 45% |
| F-VAL-060 | Coordinator/Consensus events dispatched without checking the emitting address; `Transition` structurally has no coordinator address to check against (VAL-H2 / CORE-H4 validator half) | Draft | High | 80% mechanism, ~25% that the precondition is reachable today |
| F-VAL-061 | Every effect error becomes `Resume::Noop` with no retry; `NonceTree`/`KeyGenSetup` write their placeholder **before** the effect, so a failure strands it permanently | Draft | High | 75% |
| F-VAL-062 | `Effect`/`Resume` derive `Debug` and are printed at `warn`, carrying `Arc<KeyShare>`/`Secrets`, unlike the crate's own redacted secret types | Draft | Low (Critical if `frost-core` does not redact — unresolvable offline) | 85% / 25% |
| F-VAL-063 | `blocks_per_epoch`/`genesis_salt`/`oracles` unvalidated with no onchain anchor, and **the defaults are the unsafe ones** (empty `oracles` = never attest; zero salt = no deployment separation) | Draft | Medium | 80% |
| F-VAL-064/065/066 | Fatal exits return 0, `/health` on an ephemeral loopback port, container runs as root, no `.dockerignore`; `Preprocess`/`SetValidatorStaker` never expire and nothing dedupes (a replayed `Sign` burns a nonce sequence for the whole group); `ReconcileGroupSecrets` deletes from a pre-log retention set concurrently with the block's store writes, and pre-genesis it is an unqualified `DELETE FROM` | Draft | Medium/Low | 45-90% |
| **F-VAL-005** | **Promoted by C-VAL-A**: `retain_keygen_secrets` deletes the row `store_keygen_secrets:101-104` promises is never overwritten. A reorg restores a pre-ceremony snapshot, `Message::NewBlock` runs before that block's logs, so reconciliation deletes the secrets, the replayed `KeyGen` resamples them, and the replayed own-commitment fails `frost/keygen.rs:179-183` -> `EpochSkipped`, or `Halted` forever at genesis | **Confirmed** | High | 72% |
| F-XC-001 | No `[profile.release]`: overflow checks and `debug_assert`s off in every shipped binary | Draft | Low | 78% |
| F-XC-002 | `Secrets`/`KeyShare` derive `Debug` and reach `warn!`/`trace!` via `core/effects.rs`; four sibling types hand-redact (VAL-H10 broadened) | Draft | Medium | 62% |
| F-XC-003 | `deny_unknown_fields` + `#[serde(flatten)]` in validator/sentinel configs; only the engine has a `rejects_unknown_field` test | Draft | Low | 55% |
| F-XC-004 | All three images run as root, no digest pins, no Dockerfile declares the `COMMIT_SHA` the pipeline passes | Draft | Low | 80% |
| F-XC-005 | Engine sample's 50,000-block single-call lookback + public RPC => address-poisoning check abstains on every request | Draft | Medium | 66% |
| F-XC-006 | No chain-id binding in any config or table; legacy config had `CHAIN_ID` (`known`, CORE-H17) | Draft | Low | 72% |
| F-XC-007 | No `cargo audit`/deny/Miri/fuzz gate (no advisory status asserted); `sqlx` defaults link MySQL+Postgres unused | Draft | Informational | 82% |
| F-XC-008 | Both `reqwest::Client::new` sites use defaults; the CoW client has no timeout at all (SEN-H14 broadened) | Draft | Low | 70% |
| F-XC-009 | Samples ship a well-known private key, `0.0.0.0` binds, and a silently-defaulted `genesis_salt` | Draft | Low | 74% |
| F-VAL-004 | Genesis has no deadline, so all four timeout arms and the rollover clock are no-ops; `Effect::KeyGenSetup` is emitted once and never retried, so one transient SQLite error permanently stalls genesis network-wide | Draft | High | 80% |

## Headline result — VERIFIED (C-VAL-A, Phase 2)

**F-VAL-001: Confirmed, Critical, 88%** — one point below the run's `E2` ceiling. C-VAL-A re-derived the whole attack from the code **before** reading R4's write-up and reached the same chain; **all six independent steps hold**, and every one of R4's 41 citations exists verbatim at the stated location (**zero `H` claims**).

The step flagged as most likely to fail — "does the attacker's share still verify, so the group finalizes normally?" — **holds decisively**, with contract-level evidence: `keyGenComplain` requires only `REGISTERED`, not a published share (`FROSTParticipantMap.sol:185`); `SHARING` persists until the last share lands, so complain-then-share is legal; a valid reveal only decrements `unresponded` (`state/keygen.rs:834-841`) with **no penalty for a false plaintiff**; and `confirm_key_gen` (`state/keygen.rs:1330-1358`) **never consults `complaints`**. The prior analysis's VAL-H1 exclusion assumption is refuted by the code. Threshold arithmetic checks out: `group_threshold = count/2+1`, so n=7 gives t=4 and m=2 attackers < 7/3, inside A2's fault bound.

Two corrections C-VAL-A made without changing the verdict: the "no deadline guard" framing is **not** the enabler (the share round's own deadline bounds the window); the real enablers are complaints-during-`SHARING` + plaintiff-need-not-have-shared + unconditional response. And genesis removes the time bound entirely, so **F-VAL-001 composes with F-VAL-004**.

Root cause **F-VAL-002 Confirmed 86%**, with evidence R4 missed: `docs/overview.md:48` describes `C[0]` as the ECDH key, where the existing proof-of-knowledge **was** a possession proof — moving to a separate `q` silently dropped that binding.

## Useful negative results (record in the report; they bound the search)

- **No attacker-reachable panic exists in the workspace.** R10 re-derived the census independently: 0 `panic!`, `unreachable!`, `todo!` or `unsafe` in non-test code; 6 `unwrap` and 15 `expect`, all classified as unreachable from untrusted input. No census finding was filed. None of the four analyses' section-8 censuses overstates; the validator's omits four safe fixed-buffer groups.
- R3: transaction-queue crash-consistency write ordering is sound in **both** directions, and live duplicate nonces are impossible.
- R6: **all 18 event topic0s and 18 selectors in `bindings.rs` verified against the Solidity** using a self-tested pure-Python keccak (no toolchain needed) — no mismatches, no collisions. The coordinator address **cannot** change mid-run: `Consensus._COORDINATOR` is `immutable` (`contracts/src/Consensus.sol:60,119-120`), refuting that worry. No panic reachable from chain input exists in R6's scope.
- R8: **no reachable panic, unwrap, expect, index, slice or cast in non-test code anywhere in the engine API/decoding scope**; `decode_multi_send`'s attacker-supplied `dataLength` allocates nothing (bounds-checked before the copy); header parsing, JSON strictness and the sentinel<->engine wire contract all hold. Two items could not be discharged offline and need network: `MultiSendVersion` per-deployment tagging (Safe's `MultiSend.sol` is not in this repo) and whether alloy's `uint256[]` decode pre-allocates on an attacker-supplied length.
- R2: CORE-H10 refuted (both services guard stale resumes); M10's drop/double-submit question refuted (queue state is safe); cancel-safety of `EffectManager::next`, the prune-vs-rollback race and metric-label cardinality all refuted with citations.
- R7: **M8 refuted and closed** — `reason` is produced once in `handle_engine_check_result`, hashed at `service.rs:213`, stored verbatim and `std::mem::take`n into the `Reveal` at `service.rs:424`; no re-query, reformat, locale/float or truncation path exists between commit and reveal. Salt reuse also refuted: a `requestId` can host exactly one game (`SentinelOracleRequests.sol:388`, `Consensus.sol:262`). The commit-reveal integrity worry is closed.
- R5: **M5's premise is wrong** — the secret nonce never enters snapshot state, only `NonceIndex{root, offset}`, and the onchain sequence->offset binding is a second independent single-use guard. This closes the audit's second-biggest seeded worry (snapshot rollback reviving an unspent nonce). M7 refuted (streams `1..=size`, no off-by-one, seed never logged or persisted). M4's `proof(index)` bounds issue is unreachable — all three callers verified.
- R5: `consensus/hashing.rs` matches `ConsensusMessages.sol` and `SafeTransaction.sol` **exactly** — domain, type hashes, hand-rolled encoding, `0x1901` framing. The EIP-712 parity worry is closed.
- R4: seeded lead M1 (one-time-pad reuse across keygens) **refuted** with citations — one caller, one emission site, key resampled across group ids. Threshold arithmetic, group-id/context/leaf encoding vs Solidity, marshal canonicality and identifier forgery also refuted.

## Decisions and open questions

- **Read-only run.** No `cargo build/test/clippy/audit`, no Anvil scripts, no PoC execution. Ceiling for any finding is `E2` + Critic Confirmed = 70–89% certainty. The 90–100% band is unreachable this run; QA writes PoCs as source only, marked "Not attempted (no toolchain)".
- Dependency source is not on disk, so all claims about `frost-core`/`alloy`/`sqlx` internals are class `I` (A6).
- Manager holds no agent report text in context; all substance lives in files.
- **R1 flagged a possible coverage seam**: `CORE-H12` (no RPC timeout or retry layer) lives in `provider/mod.rs:129-137`, which is R1's file, but the map assigns the lead to R2. If R2 did not file it, the Critic must promote it. R1 also left observation **O-1** (`UnexpectedBlockInvalidation` would deadlock the recovery path) ruled out only by case analysis, not by an enforced invariant — a Critic should re-derive that.
- **R10 self-reported two gaps for a Critic to close**: `Cargo.lock` was parsed programmatically rather than read line by line, and the three service handbooks in `docs/` were not read — the latter must be checked before F-XC-009 (sample private key, `0.0.0.0` binds, defaulted `genesis_salt`) is finalised.
- **Operator-authorised exception to PROMPT.md Section 1's no-network rule**: a single read-only `git clone --depth 1` of the public Safenet Charter into the session scratchpad, made because the operator answered A15 with that URL. No other network use; no RPC endpoints; nothing was written into the repository by it.

## VM restart

The session's VM restarted mid-Phase-2, killing eight background agents. Recon had already finished (`state/baseline.md`, 26,939 bytes). Nothing was lost: **all agent output is on disk**, which is exactly what the file-based state design is for.

Survived: **100 finding files** (92 from Phase 1 + 8 Critic-promoted), **67 with `## Critic` sections appended**, 69 marked `Critiqued`. Verdict tallies across the appended sections so far: **99 Confirmed, 20 Plausible, 7 Refuted** claim-level and finding-level verdicts combined.

Resumed rather than restarted, so each agent keeps the reading it had already done.

## Operator pause and resume (closed)

The operator asked to pause. **All four running Critics were stopped cleanly and no agent is running.** Verified at the pause: all **101 finding files structurally intact** (every one has Claim / Basis / Trigger / Remediation / Trail), tree clean outside `rust-audit/`, no tracked file touched.

Progress at pause: **78/101 findings critiqued.**

| Agent | State at pause | What is outstanding |
| --- | --- | --- |
| C-CORE-A, C-CORE-B, C-VAL-A, C-SEN, C-ENG-A | **complete** | nothing |
| C-VAL-B (`a9011155a99de9a82`) | stopped just as it began writing sections; had finished its reading and analysis | most of F-VAL-030..038, 060..066 |
| C-ENG-B (`a2a8c60d33d9ccdde`) | stopped during coverage-log mining; all 14 finding sections believed written | verify its cross-references landed; promote from R9's log |
| C-XC (`ac13e1b06b7e8452e`) | stopped having just read F-CORE-036 | eight sections F-XC-002..009 |
| Coverage (`a69fcb90ed87f6a0d`) | stopped mid-write of `coverage.md` | matrix is partial; seams, unexamined leads and the toolchain-blocked question list are not yet written |

**Resumed on operator instruction.** All four stopped Critics were brought back by agent id with their transcripts intact; no reading was repeated. Writes that landed after the pause count was taken reduced the outstanding work further: C-VAL-B had 12 findings left (F-VAL-034..038, 060..066), C-XC 3 (F-XC-007/008/009), C-ENG-B none (closing out citation checks), Coverage still owed the bulk of `coverage.md`.

**C-ENG-B promoted `F-ENG-044` (High, 85%) — the architectural root cause under the three engine Criticals**: the first-non-abstain-wins combinator in `engine/mod.rs:57-72` cannot implement Charter §3.7, so a single over-broad affirmer overrides every rule that never ran. This confirms rather than assumes the premise the Criticals rest on.

## The engine Criticals hold — verdict-chain premise CONFIRMED (C-ENG-B)

All three engine Criticals rest on one premise: that the first affirming checker suppresses every later denial. C-ENG-B was told to re-derive that **before** judging any of them, because if it were false all three collapsed together. **It holds, with the codebase's own test as evidence**: `engine/mod.rs:57-72` breaks at the first non-`Abstain` verdict and returns it unmodified, and the crate's test `stops_at_the_first_non_abstaining_verdict` (`engine/mod.rs:104-120`) asserts a `Secure` beats a later `Insecure`. A single over-broad affirmer **is** sufficient.

**F-ENG-030 / 031 / 033 stand independently _and_ are instances of the promoted F-ENG-044.** Each has its own concrete vector and must be fixed on its own; but the combinator is the shared root cause, so F-ENG-044 must be fixed **as well, not instead** — per-checker fixes are whack-a-mole against the next affirmer. That framing is the single most useful thing Phase 2 produced for the engine.

**F-ENG-031 kept Critical rather than reduced under A12**: the `known` TODOs at `refund.rs:83` and `:93` cover only the native-currency and zero-`refundReceiver` cases. The **ERC-20 refund path, which has no `tx.gasprice` cap, is not `known` at all** — so the `known` tag does not cover the exploitable path.

Severities re-judged by C-ENG-B: F-ENG-037 Medium->**High**; F-ENG-032 High->**Medium** (fail-closed, narrow reachable class); F-ENG-040/041 Low->**Informational**. F-ENG-042 deliberately **kept Medium** after re-derivation against the High band — its two paths are not independent, so it is a targeted DoS rather than "wrong votes at scale". All 20 of R9's rejected hypotheses re-checked and correctly refuted; none promotable.

**Charter restored.** The VM restart destroyed the session-local clone; it was re-cloned at the same upstream commit `44a1e53` to `<scratch>/safenet-charter/`. C-ENG-B verified §2.5, §2.18, §3.7, §3.8, R-4.1, R-4.2 and R-4.5 verbatim while the first copy existed, but flagged R9's **§2.15 "Affected Sentinel"** citation in F-ENG-042 as never re-opened. That section does exist (line 329) — **QA must verify it supports the claim**; if a wrong denial does expose an honest sentinel's bond, F-ENG-042 should be re-scored upward.

## The panic census is EARNED, and a factual error was caught (C-XC)

C-XC re-derived R10's headline negative **independently**, with a Rust-aware lexer (comments, strings and raw strings blanked; brace-tracked `#[cfg(test)]` exclusion, 8,740 of 24,286 lines excluded) across all 83 files, and matched R10 **site for site**: 0 `panic!`/`unreachable!`/`todo!`/`unimplemented!`/`unsafe`, 6 `unwrap`, 15 `expect`, 1 `assert!`, 2 `debug_assert`, 25 casts. All 21 sites re-checked in context — 4 `const fn` literals, 2 startup signal handlers, 9 guarded by an in-function check it verified, 5 resting on `frost-core`/`k256` (class `I`), and `cow.rs:130` the one latent site, correctly attributed to R9. **The assurance can go in the report as earned, not asserted.**

It also closed the `F-XC-001` interaction: an exhaustive non-test arithmetic sweep found **no site is safe only because of release wrapping, and none is unsafe because of it**. `F-XC-001` accordingly dropped Low -> **Informational, 66%**, because its A4 trigger over-reaches (a fabricated `u64::MAX` timestamp is neither stale, rate-limited nor incomplete — that is a malicious RPC, out of scope).

**A claim was refuted by reading the docs**: `F-XC-009`'s `0.0.0.0`-bind item is **documented deliberate** — all four sample lines are commented out, and `validator-handbook.md:44-49`, `sentinel-handbook.md:38-43` and `sentinel-engine.md:49-51`/`:105-114` each instruct the operator to set them, with `sentinel-engine.md:35` carrying an explicit "do not expose publicly" warning. The signer-placeholder half is **not** documented and was strengthened.

**And C-XC caught an `H` claim in another Critic's open assignment**: `F-VAL-064` asserts "there is no `.dockerignore` anywhere in the repository". **Four exist**, under the per-Dockerfile convention a plain search misses — `contracts/`, `crates/validator/`, `crates/sentinel/`, `crates/sentinel-engine/` each hold a `Dockerfile.dockerignore`. Manager-verified. Forwarded to C-VAL-B, which still had F-VAL-064 open, with the note that the underlying concern may survive in narrower form since the file's allow-list re-admits `/crates/**` wholesale with no pattern for `*.toml` or `*.db`.

**Promoted `F-XC-010`** (Confirmed, Low, 80%): the engine crate takes **no `metrics` dependency and has no `metrics.rs`**, yet serves a Prometheus endpoint — so verdicts and checker failures are unmeasurable, recorded only at `trace` and suppressed by the shipped `info` level. This is _why_ `F-ENG-032` (a checker dead since it was written) and `F-XC-005` are both invisible in production.

## Coverage is complete: zero unread files, four uncovered seams

`state/coverage.md` (423 lines) verifies the matrix at **83 rows / 24,203 lines** plus 13 non-Rust rows. **No file went unread** — every one of the ten logs claims 100% including tests on every file it owns. 16 files carry no anchored finding and 10 no citation of any kind; the Coverage Critic read all of those itself.

**Four seams were genuinely uncovered**, which is the report's honest-limitations material:

1. **CORE-H5's duplicate-action half has no home at the core layer** — R2 deferred it to R3, R3 wrote "not mine to file", _both confirmed the behaviour_, neither filed. Its impact survives only through `F-VAL-065` and `F-SEN-006`, so two service-level symptoms are in the report with no core-level cause. **Sent back to C-CORE-B to file or refute.**
2. **SEN-H9** — dismissed by R7 under A3; no finding mentions a loss budget or kill switch. Conscious, and rests entirely on A3 holding.
3. **SEN-H15** (un-zeroised config `String`) — a literal mutual deferral: R7 said "owned by R10", R10's `F-XC-009` said "Left with R7". Verified present in all three services; both owners judged it Informational under A1, so it is recorded with citations rather than filed.
4. **R4's observations O7 and O8** still dangling on VAL-H2, plus **VAL-H8**, which R5 handed to "the Critic" and no Critic took. **Sent to C-VAL-B**, which is still running and owns that scope.

**All 67 seeded leads were examined — none was missed.** Six are covered by no finding: CORE-H15 and VAL-H8 (observation only), SEN-H9 (dismissed under A3), ENG-H14 and M3 (documentation defects, unfileable under the scope rules), and M8 (refuted with a full trace).

**22 toolchain-blocked questions** are catalogued and tiered, each naming the finding whose certainty it would unblock; **six are answerable in under ten minutes once `cargo` exists**.

## The CORE-H5 seam is closed, and two overstatements were corrected (C-CORE-B)

**`F-CORE-067` filed - Confirmed, Medium, 80%.** The core layer does violate its own replay contract: every replay sentence in `state/mod.rs:54-73` and `effects.rs:20-25` is scoped to **effects**; `Command::Action` gets one line and no contract at all. `enqueue` is an unconditional `INSERT` with no idempotency key and no unique constraint (`tx/storage.rs:89-104`, `:69-80`), and since `SnapshotStore::reorg` never touches `transactions`, a replay inserts duplicates beside surviving rows - each taking its own higher nonce, so it is **a second onchain transaction, not a replacement**. Named **canonical** over `F-VAL-065` and `F-SEN-006`, which cite it as related and cannot fix it from inside their own crates. The seam the Coverage Critic found now has a home.

**Two reviewer overstatements corrected - both in the direction of less alarm:**

1. **`F-CORE-031`'s "hits on every restart" framing is refuted.** The replay re-emits effects _above_ the anchor; loss requires the effect to sit **on** the anchor block - a one-to-two-block coincidence, not a per-restart certainty. Certainty 78%.
2. **`F-CORE-063`'s trigger instance 3 is refuted**: `unmark_executed` runs before the only swallowable error, and the uncle path always regresses `latest` - contradicted by R3's **own** coverage-log item 26.

**F-CORE-031 was load-bearing, and it turns out it is not.** C-CORE-B checked each supposed dependant and found none: `F-SEN-001` needs the effect to _be_ re-spawned (the opposite case), `F-SEN-002` is about `committed_count`, `F-SEN-003` is the warp arm emitting no `NewBlock`, and `F-VAL-061` is the validator's own `Resume::Noop` path. **Nothing weakens with it**, and it carries its own weight independently. The earlier working assumption that four findings would fall with it was wrong.

**Severity corrected once**: `F-CORE-062` High -> **Medium** (no attacker steers either trigger; the High band requires attacker-controlled input or a reorg). **A4 boundary check**: `F-CORE-034`/`035` sit correctly on the in-scope side (a rate-limited or erroring provider); `F-CORE-062`/`063` lean on a **fork-inconsistent backend**, which stretches A4's letter - flagged in both files rather than silently accepted.

**R3's two key negatives re-verified sound**: no duplicate live nonces (re-derived across all four cases including pruned rows), and the transaction-queue write ordering is crash-safe in both directions.

## Gate 2 (CLOSED)

**Phase 2 complete: 9 Critics, every reviewer finding adversarially checked.** 107 findings; every one of the 95 reviewer findings carries a `## Critic` section, and the 12 Critic-promoted findings are attributed in their Trails.

| Final severity | Count |
| --- | --- |
| **Critical** | **5** |
| **High** | **19** |
| Medium | 34 |
| Low | 38 |
| Informational | 10 |
| Conditional | 1 (F-SEN-013 - genuinely conditional: "High if basis 8 holds, else Informational", pending the `alloy` non-UTF-8 question QA is documenting) |

Certainty spans **40-88%**, 73 findings at >=70%, **none below 40**, and **none at 90+** - correct, since `E1` is unreachable without a toolchain. The ceiling was respected by every Critic.

**Hallucinated claims found: 3**, all struck without collapsing their findings - F-ENG-038's closing paragraph (contradicted by `multi_send.rs:143`), F-VAL-064's "no `.dockerignore` anywhere" (four exist), and F-XC-052's basis row 7 (cites another audit finding, so `I` not `E2`). Across ~1,000 citations in 107 files that is a low rate, and every one was caught by a Critic re-opening the source rather than by chance.

**Findings refuted outright: 0.** No reviewer finding was demolished, but **many were corrected**: 12+ severities re-judged in both directions, and several trigger claims narrowed (F-CORE-031's "every restart", F-CORE-063's third trigger, F-XC-009's `0.0.0.0` half, F-VAL-064's dockerignore claim).

### The five Criticals

| ID | Certainty | Claim |
| --- | --- | --- |
| F-VAL-001 | 88% | DKG key `q` has no proof of possession; a peer's complete FROST signing share is recoverable while the group finalises normally |
| F-ENG-030 | 86% | `NestedSafeChecker` rates any `execTransaction`-shaped call `secure` while ignoring `value` |
| F-ENG-031 | 85% | The gas-refund leg is never vetted on any transaction an affirming checker approves |
| F-ENG-033 | 84% | `AddressPoisoningChecker` affirms `secure` from event history on an attacker-chosen `to` |
| F-VAL-033 | 60% | Restoring the validator database after a reorg reuses a burned signing nonce for a second message |

The three engine Criticals are independently exploitable **and** instances of `F-ENG-044` (the first-non-abstain-wins combinator). Fixing them per-checker without fixing the combinator is whack-a-mole against the next affirmer.

## The last three findings, and a bias caught in their evidence (C-VAL-A)

The three Coverage-Critic promotions were the only findings never adversarially checked. C-VAL-A took the two DKG ones and **cut both down**:

- **`F-XC-050`** ("no DKG handler checks group membership, so one injected `KeyGenConfirmed` closes the round") -> **Plausible, 48%, High -> Medium**. The mechanism is real and `E2`, but **arbitrary injection is refuted**: the log stream _is_ address-filtered (`core/index/events.rs:407,417,460`), so only a watched **oracle** address works, which makes the trigger rest entirely on `F-VAL-060` — set at 50% by C-VAL-B. C-VAL-A added a third condition the drafter missed: `confirm_key_gen` sets `Confirmed` at share-round close, so `key_share` is `None` only while a complaint is outstanding.
- **`F-XC-051`** -> **Plausible, 42%, Medium -> Low**. **Its title is wrong**: `require(c.length == threshold)` runs at `FROSTCoordinator.sol:378` and the `emit` at `:382`, so the contract _is_ the emitter and _is_ on the path. The identity-coefficient half is reachable but bounded — an all-identity `c` yields `y = (0,0)`, which `FROSTParticipantMap.set:162`'s strict `requireNonZero` rejects, leaving only `a_0 = 0`. Non-exploitable.

**Neither is a second path to F-VAL-001.** F-XC-050 desynchronises one validator's local view and leaks nothing — liveness, not key compromise. F-XC-051's identity attack needs the commitment to _be_ the key, which is the **abandoned** `docs/overview.md:48` design; `q` is separate and `from_point` rejects the identity. F-XC-050 does compose with `F-VAL-004`: an early genesis close is unrecoverable because every timeout arm needs a deadline genesis lacks.

**A systematic bias was caught rather than a hallucination.** No `H` claims in either file, but **both systematically under-class their own counter-evidence as `I` while every supporting row is `E2`** — a thumb on the scale toward the finding. Flagged in both files so the report does not inherit the tilt. Four citation ranges also start one to two lines before the quoted code: imprecise, not hallucinated.

**All 107 findings are now critiqued.** Only `F-SEN-013` remains conditional, legitimately so.

## QA-XC done - and one question was actually ANSWERED

**`poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md`: 24 questions**, ordered by value-per-minute with an explicit "diminishing returns begin here" line. Each names the finding(s) it unblocks with current certainty, the exact command or program that settles it, what **every** possible answer implies including the re-scoring direction, and an effort estimate. **9 are answerable in ten minutes or less, 5 in five or less.** Top of the list: `frost-core` `Debug` redaction (5 min, decides Critical-vs-Informational for `F-XC-002`/`F-VAL-062`/`F-CORE-036`) and the `alloy-sol-types` non-UTF-8 `Revealed.reason` decode (the single biggest answer, `F-SEN-013`, with the log bytes written out ready to use).

**Q22 is answered, not deferred.** The HKDF reference-vector check needed only `python3`, which this machine has — so QA-XC ran it rather than filing it: `de66ad87...ab0d` reproduces exactly, script and output saved in `poc/F-CORE-038/`. That is the only executed evidence in the entire run, and it was obtained by noticing the question did not actually need Rust.

**Two remediations judged unsound**, which is the point of the remediation check:

- `F-XC-052` option 1 as written - propagating the outer refund fields and `nonce` onto sub-calls would make **one refund look like five**; only the `chain_id` half is correct.
- `F-XC-001` option 3 - promoting `group.rs:236`'s `debug_assert!` to `assert!` would put the epoch-rollover path behind a **validator crash**; an `if`-guarded `error!` is the right shape.
- Also: `F-XC-009` option 2 must be dropped along with the refuted `0.0.0.0` item, keeping only its startup-`warn!` clause; `F-XC-003` option 1 is a **detector, not a fix** (the fix is `deny_unknown_fields` on `core::driver::Config`).

**`F-XC-007` independently confirmed clean** - the advisory grep re-run from scratch: no RUSTSEC id, no CVE, no claim that any pinned version is or is not vulnerable. The no-advisory-asserted discipline held.

**7 PoCs written**, all marked never-compiled. QA-XC deliberately wrote **no** PoC for `F-XC-001`/`004`/`006`/`007`/`050`, on the ground that a test for "the Dockerfile runs as root" is padding - reasons recorded in each QA section. **No certainty was moved**, correctly: QA had no new evidence of its own and `E1` stays unreachable.

## QA-ENG - the open Charter question is settled, and a fix that would have hurt was caught

**Charter §2.15 resolved, then taken further.** Verbatim at line 329: _"An affected Sentinel is a Sentinel whose vote may result in Council-directed slashing in the arbitration."_ That supports `F-ENG-042`'s citation but says only **"may"**, and §6.5 defers bonds and slashing to the protocol rules - so QA-ENG went to the protocol rules **in this repo** and found the answer: `contracts/src/libraries/SentinelOracleRequests.sol:298-305` charges `terms.slashAmount` whenever `approved != (state == RESOLVED_APPROVED)`, **with no good-faith exception**. So a wrong denial _does_ expose an honest sentinel's bond. `F-ENG-042` moved 72% -> **78%** on that new `E2` evidence.

**But it declined to escalate the severity, and the reasoning is right**: the slash is `fee x slashingMultiplier` - roughly 4 USDC against an 800 USDC bond at launch parameters - and it needs a split vote plus arbitration, with a conjunctive, targeted trigger. That meets neither "at scale" nor "unbounded", so it sits at the **top of Medium** with the two conditions that would escalate it recorded. Resisting an escalation that the evidence does not carry is as valuable as finding the evidence.

**A packaging fact that shapes every engine test**: `sentinel-engine` is a **binary-only crate** - no `src/lib.rs`, no `[lib]` - so `tests/*.rs` integration tests **cannot reach the checkers at all**. Every PoC is therefore an appended in-crate `#[cfg(test)]` module, and adding a thin `lib.rs` is probably the right infrastructure change. 10 PoCs, 45 tests, **17 expected to fail today**, each with the exact `cat >>` / `cargo test` / `git checkout --` commands.

**Six remediations judged unsound** - the most important being **`F-ENG-031` option 2**: _denying_ an unvettable refund leg would deny honest relayed traffic; the correct behaviour is to **abstain, not deny**. A fix that turns a missed-detection bug into a wrong-vote bug is worse than the bug. Also: `F-ENG-044` opt 3 (encodes a hand-maintained ordering table - the same reasoning that already failed for `EscapeHatchChecker`), `F-ENG-002` opt 3 and `F-ENG-041` opt 2 (using history as a "plausibly required" proxy denies the first-time honest user), `F-ENG-032` opt 3 (`debug_assert!` is compiled out of the release binary - and `F-XC-001` shows there is no `[profile.release]` at all), and `F-ENG-037` opt 3's `approved >= total` clause (silently reverses `cow.rs:348-350`'s deliberate policy, in the denying direction).

**Fix sequencing recorded in `poc/INDEX-ENG.md`**: `F-ENG-044`'s conjunctive fix _increases_ RPC fan-out, so `F-ENG-005` (timeouts) and `F-ENG-009` (fan-out bound) are **preconditions**, not follow-ups. `F-ENG-031` opt 1 makes `RefundChecker` unreachable, so `F-ENG-032`'s fix must not be read as "no longer needed". The documentation halves (`F-ENG-001`/`003`/`004`) are separated and shippable today at zero risk.

## QA-CORE-SEN - a popular fix that would lose data, and a testing prerequisite nobody stated

**The most important result is a remediation refutation.** `F-CORE-031` option 1 ("commit the resume") is the fix that **`F-SEN-001` opt 3, `F-SEN-015` opt 3 and `F-CORE-002` all point at** - and it is unsound: it commits a snapshot at `latest` while status is `BlockEvents`, so a crash makes the machine resume at `latest+1` and **lose that block's logs permanently**. Four findings were converging on a change that trades a lost effect for lost logs. All four are redirected.

`F-CORE-031` option 3 ("anchor at `uncle-2`, turn loss into duplication") rests on a **false premise** - actions have no at-least-once contract - so it would _worsen_ `F-CORE-067`.

**Other unsound remediations**: `F-CORE-067` opt 3 and `F-CORE-062` opt 3 both rely on `submitted_at IS NULL` meaning "never submitted", when it **also** means "rejected as underpriced" (proved in the `F-CORE-060` PoC part 3) - they would delete or release rows sitting in a mempool. `F-SEN-015` opt 2 is **not implementable as written** (a SQLite write inside the pure, non-`async` `apply_transition`). `F-CORE-060` opt 2 cannot bound an absolute fee and naively creates a _second_ ratchet loop. `F-CORE-001` opt 2 has nothing to walk back to - PoC test 2 proves the retained window **is** exactly the fatal depth. And **`F-CORE-004` opt 2 / `F-SEN-013` opt 2 directly contradict `F-CORE-002` opt 1**: same code path, opposite policies, with the required boundary now recorded in `F-CORE-004`.

**A testing prerequisite nobody had stated**: `sentinel` and `validator` are **binary-only crates** (no `lib.rs`) and `core`'s `tx::storage` is **private**, so **no PoC in these crates can live in a `tests/` directory**. Every one must be pasted into an existing `#[cfg(test)] mod tests` block and reverted. Turning them into permanent regression tests requires adding a `lib.rs` to `sentinel` first - an unstated prerequisite that invalidates the "tests to add" line in six `F-SEN` findings. Combined with QA-ENG's identical finding for `sentinel-engine`, **three of the four crates cannot be integration-tested as they are packaged today.**

**C-CORE-B's three-way question answered precisely**: `F-CORE-031` / `F-VAL-030` / `F-VAL-061` do not contradict, but fixing `F-CORE-031` does **not** fix the other two - their triggers are deterministic _failure_, not lost delivery - and it would make `F-VAL-061` **harder to see**, by generating `Resume::Noop`s that read as successes.

7 PoCs, 17 tests, every identifier hand-checked against commit `2893917`. **No certainty moved**, correctly.

## QA-VAL - the Critical's PoC, and the finding that the fix is not enough

**`poc/F-VAL-001/`** is the deliverable the team will actually use: a **7-party DKG ceremony** - pad harvest, impostor share accepted, and Lagrange recovery of the victim's signing share. `poc/F-VAL-033/` is a literal file backup and restore that un-burns a nonce, signs two messages with it, and recovers the key 3x3. Also `F-VAL-005-066` (one harness, two cases, driving the real `StateMachine` through `Uncle -> NewBlock -> Logs`), `F-VAL-004`, `F-VAL-030-032-061`, and `F-VAL-062` - the one-line `KeyShare::dummy` redaction test, the cheapest `E1` in the run.

**The most consequential result is about the remediation, not the finding.** QA-VAL checked `F-VAL-002`'s KDF fix against all six links C-VAL-A verified: it **closes links 4 and 5**, makes the attacker's share _fail_ so the attack goes **noisy instead of silent**, and is Solidity-compatible (`FROSTCoordinator` stores `f` and the revealed `secretShare` opaquely). But it **does not close links 1-3**, leaving a liveness variant that only the proof of possession (`F-VAL-001` opt 2) fixes. **The team needs both changes, not either.** A reader who took the KDF fix alone would believe the Critical was closed when it is not.

**Nine remediations judged unsound or wrong as written**, notably `F-VAL-003` opt 3, whose text **claims it makes F-VAL-001 impossible** - it does not: the harvest supplies the valid ciphertexts. Others: `F-VAL-005` opt 4 (a commitment-hash key makes the resample invisible, not impossible), `F-VAL-066` opt 4 (a post-write read races the delete it is meant to catch), `F-VAL-033` opt 2 (a high-water mark rejects legitimate lower offsets, reintroducing `F-VAL-030`'s harm), `F-VAL-004` opt 2 (a genesis deadline reaches `Halted` - worse than the stall), and the `F-VAL-030`/`061` "reorder the commands" halves (the driver spawns concurrently).

**Two cross-finding hazards nobody had flagged**: `F-VAL-038` opt 3 (a separate pool or file for secrets) **invalidates `F-VAL-033`'s benign case** and must not be taken before `F-VAL-033` opt 1/3; and the `F-VAL-061` opt 2 / `F-VAL-004` opt 1 retries **must wait for `F-VAL-005`**, or the "idempotent" retry resamples into a deleted row.

**Certainty raised on two findings** with new evidence of its own: `F-VAL-030` 78 -> **80%** and `F-VAL-061` 76 -> **78%**, on an exhaustive check that `Effect::NonceTree` and `KeyGenSetup` have exactly two and one emission sites respectively - which is what makes "nothing ever repairs it" a complete claim rather than a plausible one. None lowered.

## Data loss and recovery

**QA-VAL overwrote `poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md`**, which QA-XC had already populated with 24 ordered questions, believing it was creating the file. Unrecoverable from disk: untracked, so no git object, and no copy elsewhere. QA-VAL handled it correctly - noticed, moved its own content to `-VAL.md`, left a prominent labelled damage notice, and rebuilt an index from surviving citations in the finding files. Numbers 6, 11-16 and 20 were cited nowhere and were lost from disk.

**Recovered from agent context, not from disk.** All three surviving QA agents were resumed and asked to restore their entries **to separate files**, so no further concurrent write can clobber: `-XC.md` (QA-XC, the main 24), `-ENG.md` (QA-ENG, `Q-ENG-A`, **done**), `-CORE-SEN.md` (QA-CORE-SEN, the Q2 additions). The Manager merges afterwards. This is the second time this run that agent transcripts, not the filesystem, were the recovery path.

## Gate 3 (CLOSED)

**Phase 3 complete: 4 QA agents, all 107 findings carry a `## QA` section, 31 PoC directories.** Tree verified clean: **0 files outside `rust-audit/`, 0 tracked files modified.**

Because `E1` was unreachable, QA's value came from three things the team can act on directly:

1. **Ready-to-run PoCs** for every Critical and High - ~90 tests across 31 directories, every identifier hand-checked against commit `2893917`, each with the exact command, literal fixtures and pass/fail meaning, all honestly marked never-compiled.
2. **~25 remediations judged unsound**, several of which multiple findings were converging on. The two that would have done real damage: `F-CORE-031` opt 1 (pointed at by `F-SEN-001`, `F-SEN-015` and `F-CORE-002`) trades a lost effect for **permanently lost logs**; `F-ENG-031` opt 2 turns a missed-detection bug into a **wrong-vote** bug by denying instead of abstaining.
3. **A packaging blocker nobody had stated**: `sentinel`, `validator` and `sentinel-engine` are all **binary-only crates** with no `lib.rs`, and `core`'s `tx::storage` is private - so **no PoC in three of the four crates can live in a `tests/` directory**. Every one must be pasted into an existing `#[cfg(test)]` block and reverted. Making them permanent regression tests requires adding a `lib.rs` first. This silently invalidates the "tests to add" line in six `F-SEN` findings and explains the coverage gap the validator flow-test epic was chasing.

**Certainties moved by QA: 3, all upward, all within the ceiling** - `F-ENG-042` 72->78% (new `E2` from the Solidity), `F-VAL-030` 78->80%, `F-VAL-061` 76->78%. None lowered. No QA section claims anything was executed, except the one question that genuinely was (Q22, HKDF, via `python3`).

**`poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md`** is merged and canonical at 1,103 lines: 24 main questions in original numbering, `Q-ENG-A`, the two questions no registry can answer, and `VAL-Q1..9`. **9 are answerable in ten minutes or less.**

## Gate 4 (CLOSED) - run complete, Manager signed off

`report/REPORT.md`, **2,341 lines**. Manager verification, run from the repository root:

- **All 107 findings appear in the report** - checked id by id.
- **Every relative link resolves** - no broken reference into `findings/`, `poc/` or `state/`.
- **0 files outside `rust-audit/`**, **0 tracked files modified**, **0 commits**, still on `rust-audit` at `2893917`. A13 held for the whole run.

**One error caught at sign-off and corrected.** The report initially presented R4's observations O7 and O8 as never taken and listed them in next steps as needing an owner. They had in fact been examined by C-VAL-B and **deliberately not promoted**, because both are consequences of `F-VAL-060`'s single precondition and dissolve with its fix - documented at `findings/F-VAL-060.md:216-244`. The Documentation agent had inherited the Coverage Critic's snapshot, which was accurate when written and was superseded when C-VAL-B resolved them. Corrected in five places, including the executive summary and the next-steps list.

**Final disposition of the coverage seams**: four were flagged, **two closed before the run ended** (CORE-H5 by `F-CORE-067`; O7/O8 as above), and **two remain genuinely uncovered** - `SEN-H9` (consciously dismissed under A3) and `SEN-H15` (mutual deferral, judged Informational under A1 by both owners).

## Phase 5 - Verification (after the operator installed the toolchain)

**A9 is now partly TRUE.** What changed and what did not:

| Tool | Phase 0 | Now |
| --- | --- | --- |
| `cargo` / `rustc` | absent | **1.98.1 / 1.98.1**, `stable-aarch64-unknown-linux-gnu`, at `~/.cargo/bin` (**not on the default PATH** - every command must export it) |
| `just` | absent | **1.40.0** |
| RAM / disk | 3 GB / 83 GB | **11 GB / 94 GB free** (the README's 8 GB minimum is now met) |
| `forge` / `anvil` / `cast` | absent | **still absent** - no `~/.foundry`; the Anvil integration scripts remain unrunnable |
| `cargo-audit` / `cargo-llvm-cov` | absent | **still absent** - `cargo install cargo-audit --locked` is explicitly allowed by PROMPT.md Section 1 |

**What this unlocks**: `E1` becomes reachable, so findings can leave the 89% ceiling and enter the 90-100 band. What it does not unlock: anything needing Anvil, and the `sentinel-test-vectors` corpus (**A8 stays FALSE**).

Ordered plan: (1) the baseline Recon could not run - `build`, `test`, `clippy`, `audit`, `tree -d` - into `state/logs/`, updating `state/baseline.md`; (2) the 9 ten-minute questions from `poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md`, several of which decide a severity band; (3) the 31 PoC directories, whose results move certainties and may refute findings; (4) fold every result back into the finding files and `report/REPORT.md`.

**`cargo tree -d` already run** (`state/logs/cargo-tree-dupes.txt`, exit 0): 76 duplicate entries, headed by `alloy-json-abi`/`alloy-core`/`alloy-dyn-abi`/`alloy-sol-types` at **v1.6.0** under `alloy` **v2.0.5**. This answers question 8.

**Discipline for this phase**: a PoC that fails to compile, or passes when the finding says it should fail, is **evidence against the finding** and must be recorded as such - not quietly fixed until it agrees. Certainty may move **down** as well as up.

## Phase 5 results — sentinel-engine: 10/10 reproduced, nothing refuted (V-ENG)

**Every one of the 17 expected-to-fail tests failed, each for the claimed reason.** Every PoC compiled **first try with zero mechanical repairs**, apart from one flagged identifier — a striking result for tests written blind against a checkout that could not be compiled.

| Finding | Certainty | What execution showed |
| --- | --- | --- |
| `F-ENG-032` | 88 -> **99%** | **Strongest result in the run.** `RefundChecker` issues **no `eth_getLogs` at all** — the queued mock response was never consumed. The checker is dead, confirmed by execution, not inference. |
| `F-ENG-044` | 85 -> **98%** | The verdict is a **function of checker registration order**: `[Secure, denial]` returns `Secure`. The production chain rates a **blocklisted `to`** as `secure`. |
| `F-ENG-030` | 86 -> **97%** | **1000 ETH to an attacker EOA rated `secure`**, on selector alone. |
| `F-ENG-034` | 85 -> **97%** | Blocklist bypass reproduced; calldata decoding to _nothing_ is affirmed. |
| `F-ENG-031` | 85 -> **96%** | All three refund legs affirmed. The **ERC-20 leg** (`gasToken=USDC`, `gasPrice=1e18`) — the path C-ENG-B showed is _not_ covered by the `known` TODOs — proven `Secure`. |
| `F-ENG-033` | 84 -> **96%** | Both halves: `value` never read, and the evidence pool keyed by an attacker-chosen `to` (a self-emitted log affirms). |
| `F-ENG-037` | 84 -> **96%** | `max_approval_for_twap_total(0, MAX) == MAX-1`, affirmed through the real checker. |
| `F-ENG-002` | 85 -> **95%** | Denying direction: an honest **Seaport `setApprovalForAll`** returns `Insecure R-4.5`. Honest traffic denied. |
| `F-ENG-036` | 82 -> **94%** | `2^256-2` abstains — one bit short of a denial; `increaseAllowance` decodes to zero effects. |
| `F-ENG-035` | 80 -> **93%** | All four positions abstain: recipient, spender, MultiSend sub-call, `refundReceiver`. |

**`Q-ENG-A` settled**: `Asserter::is_empty` does **not** exist in `alloy-transport` 2.0.5 — QA-ENG was right to flag it rather than assume. `read_q.is_empty` does exist and asserts the same fact; V-ENG ran both, and the `read_q` form is the discriminator proving no RPC is issued.

**Honest caveat recorded in the findings**: `F-ENG-031`/`035`'s loop-based regression tests report only their first failing iteration; the remaining legs and positions are proven by the paired pin tests that passed.

**Concurrency hazard observed**: three verification agents held in-flight edits to tracked files simultaneously, and one non-compiling `cow.rs` edit briefly blocked V-ENG's build until its owner reverted. No damage — each agent touched only its own files and reverted per file — but concurrent PoC agents editing one tree is a real coordination cost worth knowing before repeating this phase.

## Phase 5 results — core + sentinel: 7/7 reproduced, and ONE FINDING REFUTED (V-CORE-SEN)

All seven PoCs **compiled unmodified** — no mechanical repair, no assertion changed — and **every predicted number matched**.

| Finding | Certainty | What execution showed |
| --- | --- | --- |
| `F-CORE-060` | 82 -> **97%** | Fee ratchet: priority **10 -> 4,037**, max **210 -> 59,550** over 60 blocks; realised **3.17%** against a **1% cap**. Also confirmed `submitted_at IS NULL` short-circuits `blocks_before_resubmit` — the conflation that makes three other remediations unsound. |
| `F-CORE-002` | 85 -> **97%** | At the **shipped default budget of 3**, three HTTP 429s strip the integrity check and `EventUpdate { blocks: 1337..=1337, logs: [] }` is accepted **for a block whose bloom asserts a watched log**. Logs committed as complete when they are not. |
| `F-CORE-067` | 80 -> **96%** | An identical action enqueued twice takes nonces **7 and 8** — two onchain transactions. A real `StateMachine` restart replay after `Uncle{2}` leaves **2 rows for one event**. |
| `F-CORE-001` | 85 -> **96%** | Canonical and orphaned-anchor resumes produce the **identical** plan and identical RPCs; the retained window is **exactly** `max_reorg_depth`, so remediation option 2 cannot work alone. |
| `F-SEN-001` | 85 -> **96%** | The sentinel's own `Committed` leaves no trace; the entry is re-created `self_committed: false` and `NewBlock(21)` emits **zero commands**. The warp-ordering test **passed**, so there is no race to win — the loss is unconditional. |
| `F-SEN-002` | 84 -> **96%** | Both variants: the entry is deleted with no actions **while 500 is bonded**; and `Finalize`+`Claim` fire at block 22 with a commitment still unrevealed. |
| `F-SEN-015` | 78 -> **95%** | Both variants: same salt, reason `"R-2.1"` -> `"R-3.4"` against stored hash `0xbb6eb049...`; and the `Unknown` arm drops a bonded request. |

### `F-SEN-013` is REFUTED — the audit's one conditional finding, settled downward

**Question 2 answered: NO.** `alloy-sol-types` 1.6.0 decodes invalid UTF-8 **lossily** — `detokenize` is `from_utf8_lossy`, and the checked `valid_token` path is only reached through the `*_validate` family, which `watcher_events!` does not use. Executed through `SentinelEvents::decode_log`, byte `0x80` returns `Some(... reason: "\u{fffd}")`.

So **basis 8 is refuted**, the conditional severity `"High if basis 8 holds, else Informational"` resolves to **Informational**, and the status is **Refuted-as-filed** (98% confidence _in the refutation_). R7 was right to mark that leg class `I` rather than assert it, and C-SEN was right to forbid its upgrade to `E2`. **The audit's widest-blast-radius hypothesis — one attacker-controlled string stalling every indexer — is false.**

Important boundary the agent recorded: this does **not** close `F-CORE-004`; the batch-poisoning mechanism survives a lossy decoder.

### Two more questions answered

- **Q11**: empty, `None` and all-zero `reward` all yield `max_priority_fee_per_gas: 1` (`EIP1559_MIN_PRIORITY_FEE`), `max_fee = 2·base + 1`. `F-CORE-060`'s mock **over-states absolute wei by ~10x** — recorded honestly; the finding is unaffected because the ratchet compounds off the previous submission.
- **Q10**: **YES** — axum's 422 echoes the offending _field name_ verbatim plus the expected schema and a column offset; values are not echoed. Statuses observed: 422 / 415 / 405 / 404. Incidental discovery: **the engine calls `Provider::connect` before binding**, so it will not start without a reachable RPC.

## Phase 5 results — validator: the Critical REPRODUCED, three legs REFUTED (V-VAL)

### `F-VAL-001` reproduced end to end — 88 -> **96%**, `E1`, Verified

**The impostor recovers the victim's complete FROST signing share, six honest peers accept the impostor's share, and the group finalizes.** 6/6 runs. One mechanical repair was needed (QA had the attacker run the honest-verify path against itself; skipping it changed no assertion). The audit's headline claim is no longer an inference — it executes. `F-VAL-002`, the root cause, reproduced through the same test's pad-reuse and pad-symmetry assertions: 86 -> **93%**.

`F-VAL-033`, the second Critical, also reproduced with no repair — restore un-burns the nonce, one nonce signs two messages, and a 3x3 solve recovers the signing share exactly. **Held at 85%, deliberately below 90**, because its trigger is an operator restore that cannot be tested. Reproducing a mechanism is not the same as demonstrating its trigger, and the agent kept that line.

| Finding | Certainty |  |
| --- | --- | --- |
| `F-VAL-004` | 84 -> **93%** | reproduced, no repair, control passes |
| `F-VAL-061` | 78 -> **93%** | reproduced, 9/9 |
| `F-VAL-030` / `F-VAL-032` | 80 -> **92%** / 76 -> **92%** | reproduced, no repair |
| `F-VAL-005` / `F-VAL-066` | 72 -> **91%** / 70 -> **91%** | reproduced, 5/5 |

### The secret-leak cluster is REFUTED — Informational, not Critical

The question flagged as _"decides Critical vs Informational for three findings"_ is answered, and the answer is the reassuring one. **`frost-core` 3.0.0 does redact**: `signing_share: SigningShare("<redacted>")`, `coefficients: "<redacted>"`.

QA's two apparent "failures" were a **false positive**: `KeyShare::dummy` gives the _identifier_ the same `0000...0001` bytes as the scalar, so a naive substring search matched the identifier, not the secret. Only execution could have caught that.

- `F-VAL-062` **60 -> 88%**, Medium -> **Informational/Low** — leak leg refuted, hygiene leg confirmed
- `F-XC-002` **74 -> 88%**, Medium -> **Low**
- `F-CORE-036` **50 -> 85%**

### Two more legs refuted, and a cross-cutting correction

- **`F-VAL-035` leg (c) refuted** (45 -> **35%**, now below the 40 reporting bar): `sqlx-sqlite` 0.9 sets `foreign_keys=ON` itself, and the cascade was observed firing. R5's seeded lead M6 guessed this was "most likely fine" — it is.
- **`F-VAL-034`**: VAL-Q6 settled — `Err(Unexpected(IncorrectCommitment))`. The outcome is benign and the finding **cannot** be escalated. 40 -> 55%.
- **`F-VAL-038`**: `busy_timeout=5000`, and WAL is **not** enabled (`journal_mode=delete`).

**A cross-cutting caveat the whole report must absorb**: `busy_timeout = 5000` **raises the bar for every "transient SQLite error" trigger in the audit** — including `F-VAL-004`'s genesis stall. Several findings assume a SQLite error is easy to induce; a five-second busy timeout makes that materially harder.

**Also**: every PoC README's `cargo test --lib` command is wrong — `validator` is binary-only, so it must be `--bins`.

## Phase 5 results — advisories: the scary numbers are NOT the real ones (V-XC)

**Two of the Manager's own readings were wrong and were corrected by execution.** Both had been handed to V-XC as a starting table with an explicit instruction to verify rather than trust; it did, and overturned them.

| Crate | Manager's briefing | **Verified verdict** |
| --- | --- | --- |
| `ruint` 1.18.0 | _"The one that matters"_ — reaches all four crates, and the engine does `U256` arithmetic throughout | **NOT reachable.** The advisory covers only **8 shift methods** — not comparisons, add or mul, which is nearly all of the engine's `U256` use. **Zero calls to any of the 8** in `crates/`. The 72 `<<`/`>>` grep hits reduce to 3 real shifts, and the only `U256` one (`sentinel/src/service.rs:922`) sits **inside `#[cfg(test)] mod tests`** (opens at `:840`). Latent; upgrade anyway. |
| `h2` 0.4.14 | reaches the engine server **and** the metrics endpoint | **Reachable, but not where expected.** V-XC sent a raw HTTP/2 preface at both servers: the engine's axum API **replied with a 55-byte SETTINGS frame** — it speaks h2 via `axum/tokio -> hyper-util/server-auto` unification **despite axum's own `http2` feature being off**. The metrics endpoint **replied with 0 bytes**: `metrics-exporter-prometheus` uses `hyper::server::conn::http1::Builder` only, so it is **not** affected. |
| `quinn-proto` **7.5 HIGH** | not reachable | **Confirmed not reachable** — empty `cargo tree -i`, 0 artifacts; an _optional_ `reqwest` dep behind `http3`, and reqwest's activated features are only `json`/`rustls`/`__tls`. **Do not lead the report with it.** |
| `crossbeam-epoch` | probably unreachable | **Not reachable** — neither `rayon-core` nor `metrics-util` `Debug`- or `{:p}`-formats an `Atomic`/`Shared`, and no Safenet code holds one. Checked, not assumed. |

**Warnings are 4 + 3 + 4, not as briefed**: 4 unmaintained; 3 unsound — `anyhow` is **not built at all**, `event-listener` and `lru` are compiled but latent; and **4 yanked**, of which only **`spin` 0.9.8** is actually compiled (via `sqlx-sqlite` -> `flume`, into all three binaries) — the other three are lockfile-only.

**`F-XC-011` filed: Low / Low, 95%** — scored on **reachable impact** (a single loopback-default, A3-gated DoS), _not_ on the 7.5 CVSS sitting in the tool output. A report that led with the HIGH would have sent the team at the one advisory that cannot affect them.

### Questions settled, two of them against existing findings

- **Q19 refutes `F-XC-007` item 2**: `sqlx-mysql`/`sqlx-postgres` are **never compiled** — 0 symbols across all three release binaries. That remediation is lockfile hygiene, **not** attack-surface reduction.
- **Q6 -> no finding.** alloy _does_ `vec_try_with_capacity(len)` before validating (`token.rs:430`), but measured across 2^20..2^64 **every case errored with dVSZ = 0 kB and dRSS <= 192 kB** — fallible `try_reserve`, pages never touched. The supplied 2^68 input is the _safest_ case, rejected outright by the `usize` check. The memory-exhaustion worry is dead.
- **Q15** _lowers_ the audit's secret-at-rest language: `SigningKey`/`SecretKey` are `ZeroizeOnDrop` and the `to_bytes` copies are already zeroized at `signer.rs:60-63`/`:88-91`; the only residual is that the wipes are not unwind-safe.
- **Q3**: all 6 PoC tests pass on both crates -> `F-XC-003` **Low -> Informational, 96%**.
- **Q4**: an un-timed `reqwest` **does** exceed 5 s and the control bounds it -> `F-XC-008` item 1 **`E1`, 94%** (unblocks `F-ENG-005`/`043`, `F-CORE-011`/`039`).
- **Q7**: confirmed from real rustc flags -> `F-XC-001` **`E1`, 93%**.
- **`F-XC-010`**: PoC passes and the metrics scrape is **completely empty** -> **`E1`, 97%**. The engine really is unmeasurable in production, which is why a checker dead since it was written went unnoticed.

`F-XC-007` received an appended Verification section only — original text untouched, process claim now `E1` at 84 -> **92%**, item 2 marked Refuted.

## Gate 5 (CLOSED) — verification complete

**108 findings. 39 verified by execution. 27 now in the 90-100 band that was unreachable for the whole read-only run.** Tree clean: 0 modified tracked files, 0 untracked outside `rust-audit/`; `cargo test --workspace` back to 266 passed.

| Final severity | Count  |
| -------------- | ------ |
| **Critical**   | **5**  |
| **High**       | **19** |
| Medium         | 32     |
| Low            | 41     |
| Informational  | 11     |

**All five Criticals are now `E1`**: `F-ENG-030` 97%, `F-ENG-031` 96%, `F-ENG-033` 96%, `F-VAL-001` 96%, and `F-VAL-033` **held at 85%** because its trigger is an operator restore that cannot be tested.

### What execution changed

**Reproduced, nothing refuted**: all 10 engine PoCs (17/17 expected failures, each for the claimed reason), all 7 core/sentinel PoCs (every predicted number matched), and the validator cluster including the headline DKG attack.

**Refuted or reduced by execution — five claims:**

1. `F-SEN-013`'s basis 8 — `alloy-sol-types` decodes invalid UTF-8 **lossily**, so the "one string stalls every indexer" hypothesis is **false**; conditional severity resolves to Informational.
2. The **secret-leak cluster** (`F-VAL-062`, `F-XC-002`, `F-CORE-036`) — `frost-core` 3.0.0 **does** redact. QA's apparent failures were a false positive: `KeyShare::dummy` gives the identifier the same `0000...0001` bytes as the scalar.
3. `F-VAL-035` leg (c) — `sqlx-sqlite` 0.9 sets `foreign_keys=ON` itself; dropped to 35%, below the reporting bar.
4. `F-XC-007` item 2 — `sqlx-mysql`/`postgres` are **never compiled** (0 symbols in all three release binaries).
5. Question 6's memory-exhaustion worry — alloy uses fallible `try_reserve`; measured 2^20..2^64, **every case dRSS <= 192 kB**.

**Two Manager readings overturned** by agents told to verify rather than trust: `ruint` is **not** reachable (the advisory covers 8 shift methods; zero calls, the one `U256` shift is inside `#[cfg(test)]`), and `h2` does **not** reach the metrics endpoint (proved by sending a raw HTTP/2 preface at both servers).

**New finding**: `F-XC-011` (Low, 95%) — advisory exposure scored on **reachable impact**, not on the 7.5 CVSS of an advisory that cannot fire here.

**Cross-cutting caveat for the report**: `busy_timeout = 5000` and `journal_mode = delete` raise the bar for every "transient SQLite error" trigger in the audit, `F-VAL-004`'s genesis stall included.

**Repair note**: 31 header-table rows across 15 files lost their trailing `|` during Phase 5 edits, breaking Markdown rendering. Repaired by the Manager; content was never affected.

## Phase 7 — the repository's own regression test is GREEN while the bug fires inside it

Foundry **1.8.1** was installed (A9 assumed 1.5.1 — gap recorded). Three of four Anvil suites pass; the sentinel suite cannot run on 1.8.1 for harness reasons documented in `baseline.md`.

### `run_validator_reorg_nonce_test.sh` does not cover `F-VAL-005` — **it exhibits it**

The suite reports `SUCCESS`. It is wrong, in three separate ways V-INT established from the harness source and the validator logs:

1. **The harness never restarts validator A.** Its comment and its SUCCESS message both claim a restart; there is a single `starting validator service` line. The test does not do what it says.
2. **It uncles the `KeyGenSecretShared` block (9), which sits below the epoch-1 group's `KeyGen` block (10)** — _exactly_ `F-VAL-005`'s trigger — but it only ever asserts on the **genesis** group, which lands in a retained rollover arm. The affected group is never checked.
3. **Both validators logged** `failed to advance key generation, skipping to next epoch :: "The participant's commitment is incorrect."`, and validator A's epoch-1 commitment **differs before (`0343738943...`) and after (`03308eece3...`) the reorg** — the row was deleted and resampled, which is the finding's exact mechanism.

**Epoch 1 was lost network-wide while the suite printed SUCCESS.** A green regression test is not evidence of correctness when it asserts on the wrong group.

V-INT also closed the loop on re-inclusion: the stale commitment came back through the validator's **own** `resubmitting stale transaction` path, not chain behaviour — it verified `anvil_reorg` drops reorged transactions permanently. So the mechanism does not depend on anvil's semantics.

### `F-CORE-001`: my downtime-vs-live reading was verified, not accepted

I had argued the passing deep-reorg suite was compatible with `F-CORE-001` because the suite reorgs while the validator is **running**. V-INT tested rather than trusted: it built a **downtime-reorg probe** with identical parameters and observed **0 `ExceededMaxReorgDepth`, the process surviving, and zero WARN/ERROR** — using the repo's own passing suite as the live control. The finding is confirmed by direct experiment.

| Finding | Certainty |  |
| --- | --- | --- |
| `F-VAL-005` | 91 -> **99%** | reproduced _by the passing suite itself_ |
| `F-CORE-001` | 96 -> **99%** | downtime probe silent where the live control fails loudly |
| `F-VAL-061` | 93 -> **98%** | unforced `failed to perform effect NonceTree ... "nonce generator is unavailable"` -> `Resume::Noop`, no retry |
| `F-VAL-030` | 92 -> **97%** | that failure strands the chunk reservation; no chunk beyond 0 ever linked |
| `F-VAL-032` | 92 -> **93%** | precondition observed live; discard arm still source-only |
| `F-VAL-066` | 91 -> **92%** | structure observed; harmful inversion not observed |
| `F-VAL-004` | **93%** unchanged | compatible — the happy path never induces the failure |
| `F-VAL-033`, `F-CORE-067` | **85%**, **96%** unchanged | **not testable by any suite** — no harness restarts a validator, and `anvil_reorg`'s empty blocks mean no log replay |

**Nothing was refuted.** The suites that pass do not contradict a single finding.

### A correction to the Manager's own cross-cutting caveat

I stated broadly that `busy_timeout = 5000` raises the bar for _every_ "transient SQLite error" trigger. V-INT corrected `F-VAL-066`'s basis claim 10: `busy_timeout` and `journal_mode` **do not protect it at all**, because it is an **ordering** hazard, not an error hazard. The caveat applies only to findings whose trigger is genuinely a transient error — `F-VAL-004` among them, and there effect failures are now shown to occur **unforced**.

## Phase 8 — sentinel-engine: every finding reproduced against a live service, with real value moving

**Safety verified**: local Anvil only on `127.0.0.1:8545`, startup log confirms `eth_chainId -> 0x7a69` (31337). The engine config was **copied** from the sample with `rpc` rewritten to loopback — the sample's live Gnosis mainnet endpoint was never used, and `run_sentinel_engine_integration_test.sh` (which defaults to public Ethereum mainnet) was never run.

These are no longer mock-transport unit tests. A real engine service, real Safe 1.5.0 proxy, real deployed contracts, real `eth_getLogs`, payloads POSTed to the loopback API exactly as the co-deployed sentinel would (A3 untouched, A2 payloads proposer-supplied).

| Finding | Certainty | What actually happened |
| --- | --- | --- |
| `F-ENG-030` | 97 -> **99%** | `{"verdict":"secure"}` for **1000 ETH** to a codeless EOA; **executed on the real Safe proxy, balance `1000e18` -> `0`.** First attempt. |
| `F-ENG-031` | 96 -> **99%** | Both legs. ERC-20 refund `secure`, **0.503 tokens really paid out**; native refund `secure`, **100.0003 ETH really paid out** (attacker relays at 100 gwei, `baseGas` unbounded). `RefundChecker` never reached, at position 9. |
| `F-ENG-033` | 96 -> **99%** | Attacker forged `Transfer(safe->attacker,1)` **from their own EOA on their own non-token contract**; the engine logged _"address-poisoning: genuine prior interaction found"_ -> `secure`; executed, **1000 ETH drained.** |
| `F-ENG-044` | 98 -> **99%** | Production wiring, operator-populated blocklist: same `to` with plain calldata -> `insecure R-4.6`; **prefix it with `announceTransaction` (`0x7b328c10`) -> `secure`**, `BlocklistChecker` never ran. Severity **kept High** deliberately — its Critical impacts are already carried by the separately-filed instances. |
| `F-ENG-002` | 95 -> **99%** | An honest `setApprovalForAll(Seaport conduit, true)` from a Safe that **really owns the NFT** -> `insecure R-4.5`, and the transaction then executes fine. Controls confirm the ERC-20 arm is correct, so the defect is confined exactly where claimed. **No config can exempt it.** |
| `F-ENG-032` | **99%** | Against a real reachable node the checker issued **zero `eth_getLogs`** — the single call logged belonged to `address_poisoning`. Misleading warn: `tx_chain_id: "0", provider_chain_id: 31337`. |

### The `F-ENG-033` vs `F-XC-005` tension — resolved, and it does not let anyone off

Both tested back to back on the same chain (mined to block 15,014) behind a **local proxy** mimicking a 10,000-block cap:

- Shipped sample values -> **`abstain`** with _"range 15014 exceeds limit of 10000"_. `F-XC-005` reproduces.
- Set `address_poisoning_max_block_range = 10000` — **the remedy the sample file itself documents** — change nothing else -> **`secure`** again. `F-ENG-033` reproduces.

So `F-XC-005` is a **config defect that masks `F-ENG-033` in one corner of the config space**. The masked state is **not** safe: it silently switches off the engine's only lookalike _denial_, while `F-ENG-030`/`031`/`044` still affirm drains **without touching the RPC at all**. Neither severity moves. `F-XC-005` rose 76 -> **92%**.

### Incidental, and operationally nasty

The engine calls `Provider::connect` **before** `TcpListener::bind`, so with no reachable RPC it exits 1 and never listens. But the **Prometheus/health listener binds first** — so a health probe can observe a live process that will never serve the API. And a provider that is reachable but merely **range-caps** passes startup cleanly and then fails **every request forever**.

## Phase 8 — validator: the headline Critical survives real contracts, and a second Critical FALLS

### `F-VAL-001` holds against real contract bytecode — 96 -> **97%**

The in-process ceremony could have been skipping a check the contracts enforce. It was not. Driven against the **real `FROSTCoordinator` / `FROSTParticipantMap` bytecode**, the whole attack is accepted: the duplicate-`q` commit, `n-1` complaints from a single plaintiff (who is **never marked `COMPROMISED`**), the impostor's own `keyGenConfirm`, and the group **finalizing with the impostor holding a participant slot** — **5/5 fresh seeds**. Nothing onchain blocks it.

### `F-VAL-033` did NOT reproduce as nonce reuse — Critical -> **High**, 85 -> **72%**

Two well-formed live runs. **The un-burn is real** — that part of the mechanism stands. But the restore-across-reorg drove the validator into a **permanent genesis self-halt / epoch non-participation _before_ any nonce could be reused**. The validator breaks itself before it can leak anything.

So the impact is **self-inflicted denial of service, not key leakage** — a materially different and less severe defect than filed. This is the phase working as intended: the finding was Critical on the strength of "nonce reuse leaks the FROST key", and under real conditions the system never gets there. **The audit now has four Criticals, not five.**

### The rest

| Finding | Certainty | Result |
| --- | --- | --- |
| `F-VAL-005` | **99%** | **Reproduced end-to-end** on the epoch-1 group: group `0x6765b9e6...` resamples after the reorg and **both** validators fail with `IncorrectCommitment` / `next_epoch:"1"`. A 2-of-2 group, so **network-wide epoch-1 loss** confirmed live. |
| `F-VAL-061` | **98%** | **Reproduced live, unforced**: `failed to perform effect NonceTree ... "nonce generator is unavailable"` -> `Resume::Noop`, **zero** later `NonceTree` spawns. |
| `F-VAL-004` | **93%** | **Reproduced** — a mid-genesis restart leaves genesis unfinalized, the validator "permanently halted", and no retry arm fires. |
| `F-VAL-030` | **97%** | Stranded phantom chunk **reproduced live**; the 1024-sequence sign-refusal consequence is **not testable locally**. |
| `F-VAL-032` | **93%** | Unlinked-chunk precondition **reproduced live**; the session-discard needs a `Sign` at sequence >=1024 (~1024 signs of griefing) — **not testable locally**. |

Two findings are now honestly marked **partially testable**: the mechanism is live-verified, the downstream consequence is not reachable in a local harness. That is a better description than either "reproduced" or "unproven".

**Safety**: every service on local Anvil (chain 31337, `127.0.0.1`, printed in each log); no sample config used unmodified; all processes killed. The agent noted two **pre-existing stray anvils from other sessions** on 8545/8645 and correctly **left them untouched** as not its own.

## Phase 8 — sentinel + core: every money finding reproduced, with the loss quantified

**The sentinel harness now runs.** `run_sentinel_integration_test.sh` was copied to the scratchpad and repaired — green on Foundry 1.8.1. It needed **three** fixes, not the two found earlier: the `cast wallet new --json` envelope (`.data[N]`), bare contract names needing `<file>.s.sol:<Contract>`, and — new — **`--root <dir>` no longer resolving a _relative_ script path**, so the `.sol` path must be absolute. `cast block`/`receipt --json` gained the same envelope. Isolated on ports 8645-8649 with renamed binary copies, because a sibling agent held 8545 and its `pkill` killed one run.

| Finding | Certainty | What it actually cost |
| --- | --- | --- |
| `F-SEN-001` | 96 -> **98%** | Restart -> own `Committed` discarded -> no reveal. Contract **slashed 2,000** to the funds receiver, **2,000 locked** in the oracle: sentinel **-4,000 fee tokens**. |
| `F-SEN-002` | 96 -> **98%** | Slow engine -> undercounted `committed_count` -> early finalise deleted the entry. Sentinel sent no `finalize`/`claim`, leaving **4,500 (bond + reward) unclaimed**. |
| `F-SEN-015` | 95 -> **97%** | Engine re-decided on replay; duplicate commit reverted `AlreadyCommitted`, **reveal reverted `InvalidReveal 0x9ea6d127`**, bond slashed. At default `max_reorg_depth` the re-decision still fires but `F-SEN-001` wins the race for the same 4,000. |
| `F-CORE-067` | 96 -> **98%** | Reproduced **via a real restart, not a reorg**: duplicate `approve`+`commit` at **nonces 2 and 3 — two onchain transactions, not one replacement** — in 3 independent runs. The reorg trigger stays **not testable locally**. |
| `F-CORE-001` | **99%** | A/B on the same depth-11 reorg: **running** -> `ERROR ExceededMaxReorgDepth(5)`, exits. **Across a restart** -> alive, **0 WARN / 0 ERROR in 2,731 lines**, resuming on orphaned block numbers. |
| `F-CORE-002` | 97 -> **99%** | A/B: 3x HTTP 429 then one empty `eth_getLogs` -> **accepted silently, logs lost, sentinel -4,000 with 2,000 slashed**. Control with the budget intact -> the same empty answer **rejected** (`incomplete logs served for block, bloom filter mismatch`) and recovered, **+500**. |
| `F-CORE-060` | 97 -> **98%** | Real Anvil fee market: tip **1 -> 11,527 -> 201,207 wei**, max fee **4,239 gwei against a real base fee of 772 wei** — `priority_fee_cap_percentage = 1` bypassed by **~28,700x**. |

**An honest constraint recorded on `F-CORE-060`**: Anvil _accepts_ the code's own 10%-both-components bump, so **the ratchet does not self-start on a healthy node** — it needs a stale fee floor (a restored database, or a foreign transaction sitting at the nonce). The balance-brake sub-claim is not testable locally. That materially narrows the trigger without touching the mechanism.

## Gate 9 (CLOSED) — final sign-off after real-world validation

`report/REPORT.md`, **3,120 lines**. Verified from the repository root: **all 108 findings present**, **every relative link resolves**, **0 modified tracked files, 0 untracked outside `rust-audit/`, 0 commits**, still on `rust-audit` at `2893917`. `cargo test --workspace`: **266 passed, 0 failed**. No stray listeners on 8545-8549, 8645-8649 or 5473.

The report now carries a **Safety boundary** block in the executive summary, a **"What live testing did not establish"** block holding all four honest limits, §4.12 (Phase 8 results with losses quantified, the repaired sentinel harness with its three fixes named, the health-listener trap) and §9.8. The `F-VAL-033` downgrade has its own subsection — _"A Critical fell, and that is the phase working as intended"_.

## Gate 8 (CLOSED) — final tally after real-world validation

| Final severity | Count                                    |
| -------------- | ---------------------------------------- |
| **Critical**   | **4** (was 5 — `F-VAL-033` fell to High) |
| **High**       | **20**                                   |
| Medium         | 32                                       |
| Low            | 41                                       |
| Informational  | 11                                       |

**108 findings; 21 validated end-to-end against real contracts on local Anvil; 21 at >=95%.**

The four surviving Criticals, all reproduced live with value moving: `F-ENG-030` 99%, `F-ENG-031` 99%, `F-ENG-033` 99%, `F-VAL-001` 97%.

**Safety held throughout**: every scenario on local Anvil (chain 31337, loopback, endpoint printed in each log). No sample config used unmodified — all three ship `rpc = "https://rpc.gnosischain.com"`. `run_sentinel_engine_integration_test.sh`, which defaults to **public Ethereum mainnet**, was never run. **No testnet or mainnet endpoint was contacted at any point.**

## Gate 7 (CLOSED) — final sign-off

`report/REPORT.md`, **2,896 lines**. Verified from the repository root:

- **All 108 findings represented**; **every relative link resolves**.
- **0 modified tracked files, 0 untracked outside `rust-audit/`, 0 commits**, still on `rust-audit` at `2893917`. `contracts/build/` is gitignored, so the forge and anvil artefacts leave no trace.
- **`cargo test --workspace`: 266 passed, 0 failed** — every temporary PoC edit reverted.
- **No stray processes**, nothing listening on 8545/8546 — the `fakerpc.py` litter that caused a false integration failure is gone.

The green-test-hides-the-bug result now opens the executive summary as "The single most important thing in this report", and drives the first next-step. `A9`'s 1.5.1 -> 1.8.1 gap is recorded as a confounder; **A8 is the only remaining hard blocker**.

## Gate 6 (CLOSED) - verified report signed off

`report/REPORT.md`, **2,715 lines**. Manager verification from the repository root:

- **All 108 findings represented**, checked id by id. **Every relative link resolves.**
- **Zero stale ceiling language** - no surviving claim that 89% is a cap or that the 90-100 band is unreachable.
- **Tree clean**: 0 modified tracked files, 0 untracked outside `rust-audit/`, **0 commits**, still on `rust-audit` at `2893917`. A13 held across all six phases.
- **`cargo test --workspace` back to 266 passed, 0 failed** - every temporary PoC edit reverted, the repository is exactly as the run found it.

**Two Manager errors caught by the Documentation agent**, both recorded rather than quietly fixed:

1. I reported **40** findings with a `## Verification` section; it is **39**. My `grep -l` matched a `###` QA subsection in `F-CORE-062` whose heading contains the word "Verification". The agent checked instead of accepting the number.
2. `STATE.md`'s "Final numbers" table was left pre-Phase-5. The agent used Gate 5 and the finding headers instead, and flagged the staleness. Now corrected, with the superseded line kept for provenance.

That makes four Manager readings corrected by agents over the run (`ruint` reachability, `h2` on the metrics endpoint, the verification count, the stale table) - each caught because the agent was told to verify rather than trust.

## Superseded next action

Phase 6: the Documentation agent updates `report/REPORT.md` to reflect Phase 5 — new certainties and statuses, the executed baseline, the refutations, `F-XC-011`, and a rewritten executive summary and next-steps section.

## Final numbers (post-Phase-5)

|  |  |
| --- | --- |
| Findings | **108** (95 from reviewers, 12 promoted by Critics, 1 from Phase 5) |
| Critical / High / Medium / Low / Informational | **5 / 19 / 32 / 41 / 11** |
| Certainty | **35-99%**, **27 at >=90% (`E1`)**, 79 at >=70% |
| Verified by execution | **39** findings carry a Phase 5 `## Verification` section |
| Superseded pre-Phase-5 line | 107 findings; 5/19/34/38/10 + 1 conditional; 40-88%, none >=90% |
| Hallucinated claims found and struck | **3**, none collapsing its finding |
| Findings refuted outright | **0** - but 12+ severities re-judged in both directions and several triggers narrowed |
| Remediations judged unsound | **~25**, several with multiple findings converging on them |
| PoC directories | **31** (~90 tests, all honestly marked never-compiled) |
| Toolchain-blocked questions catalogued | **24 + 12**, 9 answerable in <=10 minutes |
| Files read | **83 of 83** in-scope `.rs` files, 24,203 lines, every reviewer log claiming 100% |
| Agents | 1 Recon, 10 Reviewers, 9 Critics, 4 QA, 1 Documentation |

## Run history worth keeping

The file-based state design was tested three times and held every time: a **VM restart** killed eight agents mid-Phase-2, an **operator pause** stopped four more, and **QA-VAL overwrote a shared file**. In each case the recovery path was the same - agent transcripts and on-disk files, never re-derivation. Nothing was re-read and no analysis was repeated.

The one operator-authorised deviation from PROMPT.md Section 1 was a read-only `git clone` of the **public Safenet Charter**, which turned A15 TRUE and made the entire engine verdict-policy finding class (`F-ENG-001`-`004`, `039`, `044`) possible. Without it those would have stayed Plausible at best.

## Superseded next action

Phase 4: the Documentation agent compiles `report/REPORT.md` from the 107 finding files **without changing any verdict, number or wording of a claim**, builds the coverage matrix from `state/coverage.md`, lists unverified observations and rejected claims, and writes the executive summary last. Then the Manager verifies every finding file is represented and the tree is clean, and the run ends.

Wait for the Recon agent to write `state/baseline.md`, then hold Gate 0: print the baseline summary, ask the operator to tick A1–A15 (A3, A4, A15 need answers) and to run `/usage`.
