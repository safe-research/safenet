# Audit state — COMPLETE

| Field | Value |
| --- | --- |
| Commit | audited at `2893917`; **`origin/main` since merged** — HEAD is merge commit `a7f3915` (21 commits, incl. Certora FROST fixes I-01..I-09 and the sentinel verdict/meta-tx epics) |
| Run | 8 phases + post-merge revalidation, closed |
| Mode | phases 0–4 read-only; **5–8 executed** (toolchain, then Foundry, installed mid-run) |
| Deliverable | [`report/REPORT.md`](../report/REPORT.md) |

Long-form run narrative, with every gate and agent report: [`STATE-full.md`](./STATE-full.md).

## Post-merge revalidation

Merged `origin/main` (21 commits). Only **`crates/sentinel`** changed in Rust (+559/−35: `service.rs` +371, `state.rs` +207); `core`, `validator` and `sentinel-engine` are **unchanged**. `contracts/src` took the Certora FROST audit fixes I-01..I-09. Merged tree builds; `cargo test --workspace` **271 passed / 0 failed**.

**Nothing was fixed outright. Two findings got worse.**

|  |  |
| --- | --- |
| `F-VAL-001` | **STILL VALID**, Critical 97% — PoC re-run **6/6 pass** against merged contracts. The `FROSTCoordinator.sol` +11 is **100% NatSpec**; `keyGenCommit`/`keyGenComplain` byte-identical; `FROSTParticipantMap.sol` untouched. **I-08 cannot block it**: its `assert(id != 0)` is in the _signing_ path, never reached from keygen, and asserts on a hash output rather than attacker input. |
| `F-SEN-001` | **STILL VALID, 98 -> 99%** — `[Part 7] Use oracle events over local inference` rewrote only the post-`finalize` path. `handle_committed`'s discard and the `!self_committed` no-reveal drop are **byte-identical**; the PoC still fails identically. The merge **propagates the defect to a second site**: the new `approve_and_slash_amount` recovery repeats the same false inference and emits no `Claim` either. |
| `F-SEN-005` | **STILL VALID and widened, 86 -> 95%** — `WaitingForOutcome` adds a **second** never-expiring state, and `874064c` puts the arbitration deadline on the wire while `handle_dispute_triggered` **never reads it**. |
| `F-SEN-002` | **PARTIALLY ADDRESSED** — primary drop intact (still High); the mirror case is fixed. |
| `F-SEN-003` | **PARTIALLY ADDRESSED**, Medium -> Low — frozen-bond loss closed; reveals still discarded. |
| `F-XC-051`, `F-XC-050` | **STILL VALID** — I-07 hardened `mulmuladd`, but the identity path runs through `Secp256k1.add`, which still admits `(0,0)`. |
| all other `F-VAL-*`, `F-CORE-*`, `F-ENG-*` | **STILL VALID**, certainties unchanged (their code is byte-identical). |

`I-02`'s new warning **corroborates** `F-VAL-033`: upstream's own words are that nonce reuse "makes it possible to recover" the share. Line-number remaps are recorded in the findings.

## In-flight work (unmerged) — see [`report/IN-FLIGHT.md`](../report/IN-FLIGHT.md)

**`origin/main` has not moved since the merge** (still `49d7e39`; newest merge into main is #892). So the revalidation above is current, and **no further merge was needed**.

The new work is a **stacked, unmerged PR chain** — the Batched Execution epic, PRs **#899 -> #904** (`feat/batex_0` .. `feat/batex_4`), each targeting the previous. It lands on `crates/core/src/tx/` (+394 lines), both sample configs, and swaps `Validator7702Account.sol` for `Safenet7702Executor.sol`.

**Nothing in the stack fixes any finding.** Important context the agent established: the pushed branches stop at **Phase 4 of a 10-phase epic** — Phases 5-7 (delegation enqueue, batch builder, queue wiring) are unpushed, so nothing sets `authorization` yet and Phases 3-4 are **behaviourally inert today**. The risks are forward-looking.

| Finding | Effect of the stack |
| --- | --- |
| `F-CORE-067` | unchanged in kind, **worse in radius** — `enqueue` is the identical unconditional `INSERT`; the +186 storage lines add a **non-unique** `nonce` index, span logic and tests, **no idempotency key**. After Phase 7 a replay duplicates a **batch**. |
| `F-CORE-062`, `F-CORE-063` | **worsened.** 062 now _creates_ two-nonce reservations, so the epic's own acknowledged "authorization didn't apply" path writes a **permanent unfillable gap**. 063 now covers a 6-8 action batch plus a new whole-batch revert. |
| `F-CORE-064` | **reshaped** — the delegation is `expires_at: None` by design; a late batch lands all its stale actions at one nonce. |
| `F-CORE-065` | **worsened** — the 7702 authorization's `chain_id` comes from the same connect-time cache; a stale id leaves a mined tx with an unapplied delegation. |
| `F-CORE-060`/`061`, `F-CORE-066`, `F-VAL-063`, `F-XC-009`, `F-VAL-065`, `F-SEN-006`/`007` | unchanged; several gain new surface (`max_batch_gas` accepts 0, `executor` accepts the zero address, neither validated). |

**Two new findings filed, both scoped to unmerged branches** and labelled as such in their `Location`: **`F-CORE-068`** (batch execution status unobservable — success, a swallowed `CallFailed` and a whole-batch `InsufficientGas` revert are indistinguishable to `mark_executed`, enlarging the loss unit to a whole batch) and **`F-CORE-069`** (the two-nonce reservation writes a permanent gap, and its `error!` alarm **self-clears** because `mark_executed` runs immediately after it). Total findings: **110**.

## Open-PR assessment, round 2 (#906–#917) — see [`report/IN-FLIGHT.md`](../report/IN-FLIGHT.md)

`main` unchanged, so every finding still stands there. The Scheduled Secret Pruning stack **partially fixes `F-VAL-005`** (stored-block ordering; epoch-1 commitments identical across a live reorg) and changes `F-VAL-066`'s shape; #914 and #915 **fix nothing** — `F-SEN-001` lost a bond again live on the #915 tip — and **#914 merged alone worsens `F-SEN-002`**. The SEF veto epic (#917) addresses no finding. Two forward-looking findings filed: `F-VAL-068` and `F-SEN-016`. Assessment was read-only and verified against a before/after snapshot of git state and `rust-audit/`.

## Already-known work — see [`report/KNOWN-WORK.md`](../report/KNOWN-WORK.md)

All 108 findings mapped onto 24 issues, 8 TODOs and 3 epics: **11 already tracked · 8 tracked but understated · 9 CLOSED BUT STILL PRESENT · 25 partial · 55 new.**

**Four closed issues whose defect still reproduces** — the most actionable result of this round:

- **#801** _Nonces might not be retained during a reorg_ (closed by PR #803 + regression test PR #807) -> `F-VAL-005`. The fix broadened `retain_nonces` but left `retain_keygen_secrets` in the same function, and **the regression test added to close it exhibits the residual inside its own passing run** — epoch 1 lost network-wide while the suite prints SUCCESS.
- **#820** _Reorgs exceeding max reorg depth_ (closed by PR #834) -> `F-CORE-001`, `F-CORE-030`, `F-CORE-005`. The closing PR's **own body flags the unpersisted safe-block hash**, and no follow-up was filed; the exit it added returns **status 0**.
- **#656** _Only Bump Fees on Underpriced Transactions_ (closed by PR #686) -> `F-CORE-060`, and `F-CORE-061` was **introduced by that fix**.
- **#614** _Evaluate Parallel Execution of Effects_ -> `F-SEN-001/002/015`: the `WaitingForEngineCheck` variant its analysis recommended exists, but the queuing does not.

## Result

**108 findings — Critical 4 · High 20 · Medium 32 · Low 41 · Informational 11.** Certainty 35–99%; 21 validated end-to-end on local Anvil.

| Critical |  |  |
| --- | --- | --- |
| `F-ENG-030` | 99% | `secure` verdict on 1000 ETH to an attacker EOA — drained on a real Safe proxy |
| `F-ENG-031` | 99% | refund leg never vetted — 100.0003 ETH and 0.503 tokens actually paid out |
| `F-ENG-033` | 99% | address-poisoning affirms on a forged history — 1000 ETH drained |
| `F-VAL-001` | 97% | DKG key `q` has no proof of possession — victim's FROST signing share recovered; real contract bytecode accepts it, 5/5 seeds |

All three engine Criticals are instances of **`F-ENG-044`** (first-non-abstain-wins combinator). Fixing them per-checker without it leaves the next over-broad affirmer exploitable.

## The result the team should act on first

**`run_validator_reorg_nonce_test.sh` passes while exhibiting `F-VAL-005`.** It never restarts the validator despite its comment and SUCCESS message claiming so; it uncles block 9 (below the epoch-1 group's `KeyGen` at 10 — the finding's exact trigger) but asserts only on the **genesis** group. Both validators log `IncorrectCommitment`; **epoch 1 is lost network-wide while the suite prints SUCCESS**. A green badge on that test is not reorg coverage.

## What the audit corrected in itself

- **`F-VAL-033` fell Critical → High** (85→72%): the un-burn is real, but the validator self-halts before any nonce reuse — self-inflicted DoS, not key leakage.
- **Five claims refuted by execution**: `F-SEN-013`'s indexer stall (alloy decodes invalid UTF-8 lossily); the secret-leak cluster (`frost-core` _does_ redact — QA's evidence was a false positive); `F-VAL-035` leg c; `F-XC-007` item 2; the alloy memory-exhaustion worry.
- **3 hallucinated claims** found and struck, none collapsing its finding.
- **0 findings refuted by any passing integration suite.**
- Four Manager readings were overturned by agents told to verify rather than trust.

## Method, in one paragraph

Ten reviewers read all 83 in-scope files (24,203 lines) to 100%. Nine Critics re-derived each finding from the cited code _before_ reading the reviewer's argument, and set certainty independently. Four QA agents wrote PoCs and checked remediations. Phases 5–8 executed them: unit PoCs, then the Anvil integration suites, then end-to-end scenarios with real contracts and real binaries. Findings are append-only trails — reviewer → Critic → QA → Verification → Real-world — so every conclusion is checkable and disagreements stay visible.

## Reference

| File |  |
| --- | --- |
| [`report/REPORT.md`](../report/REPORT.md) | the report |
| [`findings/`](../findings/) | 112 finding files (108 against `main`, 4 scoped to unmerged branches), full evidence trails |
| [`poc/`](../poc/) | 36 PoC directories |
| [`poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md`](../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md) | open questions |
| [`state/coverage.md`](./coverage.md) | per-file coverage matrix |
| [`state/baseline.md`](./baseline.md) | toolchain and executed baselines |
| [`report/KNOWN-WORK.md`](../report/KNOWN-WORK.md) | findings mapped onto existing issues, TODOs and epics |
| [`state/paren-repair-log.md`](./paren-repair-log.md) | a scrub pass stripped `()` from function references in generated files; 96 code-fence lines restored against source, 21 ambiguous lines listed for review |

## Still open

- **`sentinel-test-vectors` corpus** — the only hard blocker (A8 FALSE); the engine checkers' intended oracle.
- The reorg-nonce harness asserts on the wrong group.
- `run_sentinel_integration_test.sh` needs three Foundry 1.8.1 fixes (named in `baseline.md`).

## Safety note for any future testing

All three `*.sample.toml` ship `rpc = "https://rpc.gnosischain.com"` (**live Gnosis mainnet**), and `scripts/run_sentinel_engine_integration_test.sh:11` defaults to `https://ethereum-rpc.publicnode.com` (**live Ethereum mainnet**). Phase 8 ran on local Anvil (chain 31337) only; no testnet or mainnet was contacted. Copy configs and rewrite `rpc` to loopback before starting anything.
