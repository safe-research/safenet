# Safenet Rust services — security and robustness review

**Post-merge:** `origin/main` has since been merged (HEAD `a7f3915`, 21 commits). Every finding was re-validated against it — **nothing was fixed outright; `F-SEN-001` and `F-SEN-005` got worse.** See [`../state/STATE.md`](../state/STATE.md). **In-flight:** [`IN-FLIGHT.md`](IN-FLIGHT.md) assesses the unmerged Batched Execution stack (PRs #899–#904) — it fixes nothing, worsens `F-CORE-062`/`063`/`065`, and adds two forward-looking findings. A second round covers open PRs #906–#917: the pruning stack partially fixes `F-VAL-005`, #914/#915 fix nothing (and #914 alone worsens `F-SEN-002`), and two further forward-looking findings were filed. **Already-known work:** [`KNOWN-WORK.md`](KNOWN-WORK.md) maps all 108 findings onto the team's issues, TODOs and epics — 55 are new, and **9 sit under issues the team already closed**.

Long-form version, with every trigger, remediation option and verification transcript: [`REPORT-full.md`](REPORT-full.md). Where this report and a finding file differ, **the finding file is authoritative**.

## 1. Verdict

| Field | Value |
| --- | --- |
| Target | `crates/core`, `crates/validator`, `crates/sentinel`, `crates/sentinel-engine` — 83 `.rs` files, 24,203 lines, plus 13 non-Rust in-scope files |
| Commit | `2893917757ae518ebb91154712cf3e401cb68d33` (branch `rust-audit`), verified unchanged for the whole run |
| Mode | Phases 0–4 **read-only**; Phase 5 executed unit-level PoCs; Phase 7 ran the repo's Anvil suites; Phase 8 drove findings end-to-end against real contracts and binaries. **Local Anvil only** (chain 31337, `127.0.0.1`) |
| Evidence | **39** findings carry executed (`E1`) verification, **9** carry Anvil-suite verification, **21** carry real-world validation with value moving |
| Deliverables | 112 finding files (108 against `main`, 4 scoped to unmerged branches) in [`../findings/`](../findings/), 38 PoC directories in [`../poc/`](../poc/), run narrative in [`../state/STATE.md`](../state/STATE.md) |

**108 findings. Critical 4 / High 20 / Medium 32 / Low 41 / Informational 11.** Certainty spans **35–99 %**; 28 are at 90 % or above, 21 at 95 % or above. **No finding was refuted by any passing integration suite.** One Critical fell to High under live testing (`F-VAL-033`, below); one Informational was refuted outright (`F-SEN-013`).

### The four Criticals — all reproduced live, with value actually moving

| ID | Cert. | Claim | What Phase 8 executed |
| --- | --- | --- | --- |
| [`F-ENG-030`](../findings/F-ENG-030.md) | 99% | `NestedSafeChecker` rates any `execTransaction`-shaped call `secure` while ignoring `value` | `{"verdict":"secure"}` for 1000 ETH to a codeless EOA, then executed on a real Safe 1.5.0 proxy: balance `1000e18` → `0`. First attempt |
| [`F-ENG-031`](../findings/F-ENG-031.md) | 99% | The gas-refund leg is never vetted on any transaction an affirming checker approves | Both legs paid out: **0.503 tokens** (ERC-20) and **100.0003 ETH** (native, attacker relaying at 100 gwei, `baseGas` unbounded). `RefundChecker` never reached, at position 9 |
| [`F-ENG-033`](../findings/F-ENG-033.md) | 99% | `AddressPoisoningChecker` affirms `secure` from event history on an attacker-chosen `to` | Attacker forged a `Transfer` from their own EOA on their own non-token contract; engine logged _"genuine prior interaction found"_ → `secure`; **1000 ETH drained** |
| [`F-VAL-001`](../findings/F-VAL-001.md) | 97% | DKG encryption key `q` has no proof of possession; a peer's complete FROST signing share is recoverable while the group finalises normally | Driven against real `FROSTCoordinator`/`FROSTParticipantMap` bytecode: duplicate-`q` commit, `n-1` complaints from a plaintiff never marked `COMPROMISED`, impostor's own `keyGenConfirm`, group finalises with the impostor holding a slot. **5/5 fresh seeds. The contracts block nothing** |

### The green test that hides the bug

`scripts/run_validator_reorg_nonce_test.sh` exits 0 and prints `SUCCESS`. It does not vindicate the reorg path — it **exhibits [`F-VAL-005`](../findings/F-VAL-005.md)** (High, 91 → **99 %**) inside the passing run:

- **The harness never restarts validator A**, though its header comment and SUCCESS message both say it does. One `starting validator service` line, no `kill`. _No suite in `scripts/` restarts a validator._
- It uncles the `KeyGenSecretShared` block (9), below the epoch-1 group's `KeyGen` block (10) — exactly `F-VAL-005`'s trigger — then asserts only on the **genesis** group. The affected group is never checked.
- Both validators logged `failed to advance key generation, skipping to next epoch :: "The participant's commitment is incorrect."`, and validator A's epoch-1 commitment differs before (`0343738943…`) and after (`03308eece3…`) the reorg: the `keygen_secrets` row was deleted and resampled.
- **Epoch 1 was lost network-wide while the suite reported SUCCESS.**
- The stale commitment's re-inclusion came from the validator's own `resubmitting stale transaction` path, not from anvil semantics (`anvil_reorg` drops reorged transactions permanently).

Pointing that suite's assertions at the epoch-1 group is the cheapest high-value change in this report: it converts a misleading green into a failing test that pins a High finding.

### Quantified losses measured on chain

**1000 ETH drained twice** (`F-ENG-030`, `F-ENG-033`), **100.0003 ETH and 0.503 tokens** paid out as refunds (`F-ENG-031`), sentinel **−4,000 fee tokens** with 2,000 slashed (`F-SEN-001`, `F-CORE-002`), **4,500 left unclaimed** (`F-SEN-002`), and the fee cap bypassed **~28,700×** (`F-CORE-060`: tip 1 → 11,527 → 201,207 wei, max fee 4,239 gwei against a real base fee of 772 wei).

### A Critical fell — the phase working as intended

[`F-VAL-033`](../findings/F-VAL-033.md) moved **Critical → High**, 85 → **72 %**. The un-burn is real and was reproduced. But across two well-formed live runs the restore-across-reorg drove the validator into a **permanent genesis self-halt before any nonce could be reused**. Impact is **self-inflicted denial of service, not key leakage** — it was Critical on the strength of "nonce reuse leaks the FROST key", and the system never gets there.

---

## 2. Fix these first

Read §3 before taking any remediation option: about 25 proposed options were judged unsound. `cargo test --lib` in every PoC README must be `cargo test -p <crate> --bins`.

**0. Fix the harness, today.** Make `scripts/run_validator_reorg_nonce_test.sh` assert on the epoch-1 group its own reorg affects, and correct its header comment and SUCCESS message, which both claim a restart of validator A that the script does not perform. Closes the coverage illusion behind [`F-VAL-005`](../findings/F-VAL-005.md).

**1. Ship the zero-risk documentation halves.** [`F-ENG-001`](../findings/F-ENG-001.md), [`F-ENG-003`](../findings/F-ENG-003.md), [`F-ENG-004`](../findings/F-ENG-004.md) are Charter-citation corrections in `engine/rule.rs` doc comments, separable from the behavioural halves. [`F-CORE-038`](../findings/F-CORE-038.md), [`F-CORE-065`](../findings/F-CORE-065.md) opt 4, [`F-CORE-064`](../findings/F-CORE-064.md) opt 1 and the M3 doc drift are the same shape.

**2. Engine deadlines and fan-out bound — a precondition, not a follow-up.** [`F-ENG-005`](../findings/F-ENG-005.md) (timeouts) and [`F-ENG-009`](../findings/F-ENG-009.md) (fan-out bound) **must land before** `F-ENG-044`'s conjunctive fix, which _increases_ per-request RPC fan-out. Fixing [`F-ENG-032`](../findings/F-ENG-032.md) also makes `RefundChecker` issue a second set of `eth_getLogs` per relayed transaction.

**3. The combinator and the three engine Criticals, in one change set.** [`F-ENG-044`](../findings/F-ENG-044.md) (98 %, executed) together with [`F-ENG-030`](../findings/F-ENG-030.md), [`F-ENG-031`](../findings/F-ENG-031.md), [`F-ENG-033`](../findings/F-ENG-033.md). The verdict is a function of checker registration order — `[Secure, denial]` returns `Secure`, and the production chain rates a blocklisted `to` as `secure`. **Fixing the three per-checker without `F-ENG-044` leaves the next over-broad affirmer exploitable**; fixing the combinator alone leaves three live vectors. Take `F-ENG-031` **option 1 or 4, never option 2** — abstain, do not deny. Option 1 makes `RefundChecker` unreachable, so `F-ENG-032`'s fix must not be dropped as "no longer needed".

**4. `F-VAL-001` and `F-VAL-002` together — both changes.** The KDF ([`F-VAL-002`](../findings/F-VAL-002.md) opt 1) closes links 4–5 and makes the attack noisy; the proof of possession ([`F-VAL-001`](../findings/F-VAL-001.md) opt 2) closes links 1–3. **The KDF fix alone does not close the Critical.** `F-VAL-003` opt 3 substitutes for neither.

**5. Validator crash-consistency, in dependency order.** [`F-VAL-005`](../findings/F-VAL-005.md) **first**; only then the [`F-VAL-061`](../findings/F-VAL-061.md) opt 2 / [`F-VAL-004`](../findings/F-VAL-004.md) opt 1 retries, or the "idempotent" retry resamples into a deleted row. [`F-VAL-033`](../findings/F-VAL-033.md) opt 1 or opt 3 **before** [`F-VAL-038`](../findings/F-VAL-038.md) opt 3, or the separate-pool change invalidates `F-VAL-033`'s benign case. Do not take `F-VAL-033` opt 2.

**6. `F-CORE-067` is canonical for duplicate actions.** [`F-CORE-067`](../findings/F-CORE-067.md) (98 %) — [`F-VAL-065`](../findings/F-VAL-065.md) and [`F-SEN-006`](../findings/F-SEN-006.md) cannot fix it from inside their own crates. Fix it at `tx/storage.rs` with an idempotency key. Do not take its opt 3 or [`F-CORE-062`](../findings/F-CORE-062.md) opt 3.

**7. Core reorg and indexing.** [`F-CORE-001`](../findings/F-CORE-001.md) (99 %; option 2 cannot work alone — the retained snapshot window _is_ exactly `max_reorg_depth`), [`F-CORE-002`](../findings/F-CORE-002.md) (99 %; three HTTP 429s at the shipped default budget of 3 strip the integrity check), then [`F-CORE-031`](../findings/F-CORE-031.md) — but **not** its option 1 (§3).

**8. Sentinel bond safety.** [`F-SEN-001`](../findings/F-SEN-001.md) (98 %) and [`F-SEN-002`](../findings/F-SEN-002.md) (98 %) lose money on ordinary restarts and on a merely-slower engine; [`F-SEN-015`](../findings/F-SEN-015.md) (97 %) shares their root. The `F-SEN-001` loss is **unconditional** — the warp-ordering control test passed, so there is no race to win. `F-SEN-001` opt 3 and `F-SEN-015` opt 3 are the unsound `F-CORE-031` opt 1 in disguise.

**9. Observability last, but not never.** [`F-XC-010`](../findings/F-XC-010.md) (97 %, executed: the metrics scrape is _completely empty_) is why `F-ENG-032` — a checker dead since it was written — and `F-XC-005` are invisible in production. Every silent failure mode in this report stays silent until this lands.

**10. Dependency hygiene, correctly prioritised.** Upgrade `h2` — the **only** advisory reachable from a network-facing surface, and only on the engine's check API, which A3 gates ([`F-XC-011`](../findings/F-XC-011.md)). Upgrade `ruint` and `crossbeam-epoch` as hygiene. **Do not prioritise the 7.5 HIGH `quinn-proto`** — it is not compiled and not reachable. Add the CI advisory gate [`F-XC-007`](../findings/F-XC-007.md) asks for.

---

## 3. Do not ship these "fixes"

Roughly **25 proposed remediation options were judged unsound**, several of them the option multiple findings independently converged on. A fix that makes things worse is more urgent than a finding.

### The two that would have done real damage

**[`F-CORE-031`](../findings/F-CORE-031.md) option 1 — "commit the resume".** The fix that [`F-SEN-001`](../findings/F-SEN-001.md) opt 3, [`F-SEN-015`](../findings/F-SEN-015.md) opt 3 **and** [`F-CORE-002`](../findings/F-CORE-002.md) all point at. It commits a snapshot at `latest` while the status is `BlockEvents`, so a crash resumes at `latest + 1` and **loses that block's logs permanently**. Four findings converging on a change that trades a lost effect for lost logs; all four are redirected in their QA sections. `F-CORE-031` option 3 ("anchor at `uncle-2`") rests on a false premise — actions have no at-least-once contract — and would _worsen_ `F-CORE-067`.

**[`F-ENG-031`](../findings/F-ENG-031.md) option 2 — deny an unvettable refund leg.** Denying would deny honest relayed traffic. The correct behaviour is to **abstain, not deny**: a fix that turns a missed-detection bug into a wrong-vote bug is worse than the bug.

### The rest, by owner

**Core and sentinel.** `F-CORE-067` opt 3 and `F-CORE-062` opt 3 both read `submitted_at IS NULL` as "never submitted", when it also means "rejected as underpriced" (proved in the `F-CORE-060` PoC, part 3) — they would delete or release rows sitting in a mempool. `F-SEN-015` opt 2 is not implementable as written (a SQLite write inside the pure, non-`async` `apply_transition`). `F-CORE-060` opt 2 cannot bound an absolute fee and creates a second ratchet loop. `F-CORE-001` opt 2 ("walk back") has nothing to walk back to. **`F-CORE-004` opt 2 and `F-SEN-013` opt 2 directly contradict `F-CORE-002` opt 1** — same code path, opposite policies.

**Engine.** `F-ENG-044` opt 3 encodes a hand-maintained ordering table — the reasoning that already failed for `EscapeHatchChecker`. `F-ENG-002` opt 3 and `F-ENG-041` opt 2 use transaction history as a "plausibly required" proxy, denying the first-time honest user. `F-ENG-032` opt 3 uses `debug_assert!`, compiled out of the release binary (`F-XC-001`: there is no `[profile.release]` section at all). `F-ENG-037` opt 3's `approved >= total` clause silently reverses `cow.rs:348-350`'s deliberate policy, in the denying direction.

**Validator.** Nine judged unsound, notably: `F-VAL-003` opt 3, whose own text claims it makes `F-VAL-001` impossible — it does not, the pad harvest supplies the valid ciphertexts; `F-VAL-005` opt 4 (a commitment-hash key makes the resample invisible, not impossible); `F-VAL-066` opt 4 (a post-write read races the delete it is meant to catch); `F-VAL-033` opt 2 (a high-water mark rejects legitimate lower offsets, reintroducing `F-VAL-030`'s harm); `F-VAL-004` opt 2 (a genesis deadline reaches `Halted`, worse than the stall); the `F-VAL-030`/`F-VAL-061` "reorder the commands" halves (the driver spawns concurrently).

**Cross-cutting.** `F-XC-052` opt 1 as written would make one refund look like five — only the `chain_id` half is correct. `F-XC-001` opt 3 would put the epoch-rollover path behind a validator crash; an `if`-guarded `error!` is the right shape. `F-XC-009` opt 2 must be dropped with the refuted `0.0.0.0` item, keeping only its startup-`warn!` clause. `F-XC-003` opt 1 is a detector, not a fix — the fix is `deny_unknown_fields` on `core::driver::Config`. `F-XC-007` item 2 is refuted, not merely mis-emphasised (§6).

### Ordering hazards, restated

- **`F-ENG-005` / `F-ENG-009` are preconditions for `F-ENG-044`** (fan-out).
- **The `F-VAL-061` opt 2 / `F-VAL-004` opt 1 retries must wait for `F-VAL-005`.**
- **`F-VAL-038` opt 3 must not precede `F-VAL-033` opt 1 or opt 3.**
- `F-ENG-031` opt 1 makes `RefundChecker` unreachable — `F-ENG-032`'s fix is still needed.
- Fixing `F-CORE-031` does **not** fix `F-VAL-030` or `F-VAL-061` (their triggers are deterministic _failure_, not lost delivery) and would make `F-VAL-061` **harder to see**, by generating `Resume::Noop`s that read as successes.

---

## 4. All 108 findings

Severity is the final severity; certainty is the finding header's number. Markers: **V** = Phase 5 `## Verification` (executed, 39), **I** = Phase 7 `## Integration verification (V-INT)` against the repo's Anvil suites (9), **R** = Phase 8 `## Real-world validation` against deployed contracts (21).

| Final severity | Count |  | Status | Count |  | ID family | Count |
| --- | --: | --- | --- | --: | --- | --- | --: |
| **Critical** | **4** |  | Verified (Phase 5) | 39 |  | `F-CORE-*` | 31 |
| **High** | **20** |  | QA-done | 41 |  | `F-ENG-*` | 24 |
| **Medium** | **32** |  | Critiqued | 32 |  | `F-VAL-*` | 24 |
| **Low** | **41** |  | Draft (Critic-promoted) | 3 |  | `F-SEN-*` | 15 |
| **Informational** | **11** |  | (of which Refuted-as-filed: 1) |  |  | `F-XC-*` | 14 |
| **Total** | **108** |  | **Total** | **108** |  | **Total** | **108** |

| ID | Title | Sev | Cert. | V·I·R | Status |
| --- | --- | --- | --: | :-: | --- |
| [`F-ENG-030`](../findings/F-ENG-030.md) | `NestedSafeChecker` rates any `execTransaction`-shaped call `secure` while ignoring `value`, so a full native-currency drain is affirmed | Critical | 99% | V·R | Verified |
| [`F-ENG-031`](../findings/F-ENG-031.md) | The gas-refund leg is never vetted on any transaction an affirming checker approves, so an unbounded native-currency refund drain is rated `secure` | Critical | 99% | V·R | Verified |
| [`F-ENG-033`](../findings/F-ENG-033.md) | `AddressPoisoningChecker` affirms `secure` from event history on an attacker-chosen `to`, and never inspects `transaction.value` | Critical | 99% | V·R | Verified |
| [`F-VAL-001`](../findings/F-VAL-001.md) | DKG encryption key `q` has no proof of possession: a participant that republishes a peer's `q` recovers that peer's complete FROST signing share while the group … | Critical | 97% | V·R | Verified |
| [`F-CORE-001`](../findings/F-CORE-001.md) | Persisted indexer state is bound to block numbers only, so a reorg during downtime is invisible and silently defeats `max_reorg_depth` | High | 99% | V·I·R | Confirmed |
| [`F-CORE-002`](../findings/F-CORE-002.md) | `use_client_filtering`'s log-completeness check disables itself after three failures, and the failures that exhaust it are the incomplete responses it exists to detect | High | 99% | V·R | Verified |
| [`F-ENG-002`](../findings/F-ENG-002.md) | `RuleId::R4_5ExcessiveApproval` claims `setApprovalForAll` is an unconditional immediate failure "per § 2.5"; the Charter makes operator approval-for-all conditional … | High | 99% | V·R | Verified |
| [`F-ENG-044`](../findings/F-ENG-044.md) | The engine's first-non-abstain-wins combinator cannot implement Charter §3.7, so one over-broad affirmer overrides every rule that never ran | High | 99% | V·R | Verified |
| [`F-VAL-005`](../findings/F-VAL-005.md) | A reorg across the key-generation block deletes the DKG secrets the store promises never to overwrite … | High | 99% | V·I·R | Confirmed |
| [`F-CORE-060`](../findings/F-CORE-060.md) | Underpriced-rejection fee ratchet is unbounded, runs every block, and bypasses `priority_fee_cap_percentage` | High | 98% | V·R | Verified |
| [`F-SEN-001`](../findings/F-SEN-001.md) | Replay after a restart or reorg discards the sentinel's own `Committed`, so it never reveals and its bond is slashed | High | 98% | V·R | Verified |
| [`F-SEN-002`](../findings/F-SEN-002.md) | Commitments seen before the engine verdict are discarded, so early finalisation fires with `self_revealed == false` and the bond and reward are never claimed | High | 98% | V·R | Verified |
| [`F-VAL-061`](../findings/F-VAL-061.md) | A failed effect is silently converted to `Resume::Noop` with no retry path, permanently stranding state written in anticipation of it | High | 98% | V·I·R | Confirmed |
| [`F-ENG-034`](../findings/F-ENG-034.md) | `EscapeHatchChecker` affirms the announcement shape for **any** `to` and runs ahead of the blocklist, so an R-4.6 target is rated `secure` | High | 97% | V | Verified |
| [`F-SEN-015`](../findings/F-SEN-015.md) | A replayed engine check re-decides an already-committed vote: the second verdict overwrites the reason the commitment was built from … | High | 97% | V·R | Verified |
| [`F-VAL-030`](../findings/F-VAL-030.md) | A lost or failed `NonceTree` effect leaves a phantom chunk reservation that is counted as capacity and never retried | High | 97% | V·I·R | Confirmed |
| [`F-ENG-037`](../findings/F-ENG-037.md) | The CoW TWAP approval tolerance is sized by an attacker-chosen `n`, so a near-unlimited relayer approval is rated `secure` | High | 96% | V | Verified |
| [`F-ENG-036`](../findings/F-ENG-036.md) | R-4.5 is implemented as an exact `U256::MAX` comparison, so `approve(X, 2^256-2)` evades it — and is then affirmed `secure` by the address-poisoning history bypass | High | 94% | V | Verified |
| [`F-ENG-035`](../findings/F-ENG-035.md) | The blocklist is applied only to the top-level `to`, so R-4.6 misses token recipients, approval spenders, batch sub-calls and the refund receiver … | High | 93% | V | Verified |
| [`F-VAL-004`](../findings/F-VAL-004.md) | A single failed or lost `KeyGenSetup` effect during genesis stalls the validator forever: the genesis rollover state has no deadline, no timeout arm and no retry | High | 93% | V·I·R | Verified |
| [`F-VAL-032`](../findings/F-VAL-032.md) | A `Sign` event whose sequence has no linked nonce chunk permanently discards the signing session | High | 93% | V·I·R | Verified |
| [`F-VAL-066`](../findings/F-VAL-066.md) | `ReconcileGroupSecrets` deletes from a retention set computed before the block's logs, and runs concurrently with the store writes those logs cause | High | 92% | V·I | Verified |
| [`F-VAL-033`](../findings/F-VAL-033.md) | Restoring the validator database after a reorg reuses a burned signing nonce for a second message; nothing records that a nonce was consumed | High | 72% | V·I·R | Verified |
| [`F-VAL-039`](../findings/F-VAL-039.md) | The nonce top-up threshold gives ~100 sequences of headroom against a permissionless, group-wide sequence counter … | High | 58% | — | QA-done (C-promoted) |
| [`F-ENG-032`](../findings/F-ENG-032.md) | `RefundChecker` is dead: its synthetic refund transfer carries `chainId = 0`, so the delegated address-poisoning check always abstains | Medium | 99% | V·R | Verified |
| [`F-CORE-067`](../findings/F-CORE-067.md) | `Command::Action` has no replay contract and the queueing path has no de-duplication, so every rollback replay enqueues duplicate onchain transactions | Medium | 98% | V·I·R | Verified |
| [`F-VAL-002`](../findings/F-VAL-002.md) | The ECDH share pad is an unhashed x-coordinate used in both directions of every pair, so each pad encrypts two shares and one complaint response exposes both | Medium | 93% | V | Verified |
| [`F-XC-005`](../findings/F-XC-005.md) | The engine sample config pairs a 50,000-block single-call lookback with a public RPC and no range cap, which silently disables the address-poisoning check | Medium | 92% | R | QA-done |
| [`F-SEN-005`](../findings/F-SEN-005.md) | `WaitingForDisputeResolution` never expires and the sentinel never calls the permissionless `timeoutArbitration` … | Medium | 86% | — | Critiqued |
| [`F-CORE-030`](../findings/F-CORE-030.md) | `Driver::run` discards its outcome, so every unrecoverable error exits the process with status 0 and the only other failure channel (`/health`) is liveness-only | Medium | 85% | — | Critiqued |
| [`F-ENG-001`](../findings/F-ENG-001.md) | `RuleId::R4_1SettingsChange`'s stated meaning is far wider than Charter R-4.1's allowed exception, and the base checker implements the doc comment rather than the Charter | Medium | 85% | — | QA-done |
| [`F-CORE-034`](../findings/F-CORE-034.md) | Every watcher error is retried at a fixed 100 ms forever, with one warning line per attempt: a rate-limited or deterministically-failing node becomes a self-sustaining … | Medium | 80% | — | Critiqued |
| [`F-ENG-039`](../findings/F-ENG-039.md) | `BaseChecker`'s Article IV Part A allow-lists are materially wider than the Charter's R-4.1/R-4.2 exceptions … | Medium | 80% | — | QA-done |
| [`F-VAL-003`](../findings/F-VAL-003.md) | A DKG complaint compels a plaintext share reveal with no check that the plaintiff ever received a share, no per-plaintiff bound and no deadline in the sharing round | Medium | 80% | — | QA-done |
| [`F-CORE-031`](../findings/F-CORE-031.md) | Effects are spawned only after the snapshot that records them as pending, so every rollback that lands on the spawning block reverts the resume and never re-runs the … | Medium | 78% | — | Critiqued |
| [`F-CORE-035`](../findings/F-CORE-035.md) | The driver classifies _every_ RPC error as intermittent and swallows it forever, so a permanently failing node silently stops all onchain action while the service … | Medium | 78% | — | Critiqued |
| [`F-CORE-066`](../findings/F-CORE-066.md) | `tx::Config` accepts values that silently disable or destabilise the queue: `max_in_flight_transactions = 0`, `blocks_before_resubmit = 0` … | Medium | 78% | — | Critiqued |
| [`F-ENG-042`](../findings/F-ENG-042.md) | An address-poisoning denial is issued from an evidence set bounded by recency and by provider completeness, so a genuine payee can be denied under R-4.3/R-4.4 | Medium | 78% | — | QA-done |
| [`F-CORE-004`](../findings/F-CORE-004.md) | The event watcher has no terminal error state: deterministic, content-dependent failures keep the indexer on the same block forever … | Medium | 75% | — | Critiqued |
| [`F-ENG-003`](../findings/F-ENG-003.md) | `RuleId::R4_2DelegatecallIntegrity` restates a storage-effect rule as a target allow-list, and the allow-list admits migrations that change Safe storage the Charter does … | Medium | 75% | — | QA-done |
| [`F-CORE-064`](../findings/F-CORE-064.md) | `expires_at` is silently void once a nonce is allocated, contradicting the queue's documented contract | Medium | 72% | — | Critiqued |
| [`F-SEN-003`](../findings/F-SEN-003.md) | A warp replay delivers no `NewBlock`, so reveals in the replayed range are discarded, `finalize` takes the timeout branch, and a frozen request's bond is never claimed | Medium | 72% | — | Critiqued |
| [`F-VAL-063`](../findings/F-VAL-063.md) | Consensus-critical configuration is unvalidated, has no onchain anchor, and its defaults are the unsafe ones | Medium | 72% | — | QA-done |
| [`F-CORE-003`](../findings/F-CORE-003.md) | A lagging RPC backend that answers `null` for a block it has not imported is treated as a reorg, producing a spurious uncle, a state rollback and a full replay | Medium | 70% | — | Critiqued |
| [`F-CORE-012`](../findings/F-CORE-012.md) | `use_client_filtering`'s bloom-equality completeness check is blind to the loss of any log whose (address, topics) shape another log in the same block repeats … | Medium | 70% | — | Draft (C-promoted) |
| [`F-CORE-033`](../findings/F-CORE-033.md) | Effect concurrency is unbounded: one backfill page can spawn a task per matching log at once, with no cap, no queue and no backpressure | Medium | 70% | — | Critiqued |
| [`F-VAL-065`](../findings/F-VAL-065.md) | Two actions are queued with no expiry and none is deduplicated, so restart and reorg replay produce duplicate onchain transactions … | Medium | 70% | — | QA-done |
| [`F-VAL-064`](../findings/F-VAL-064.md) | The shipped deployment cannot detect a halted validator: fatal exits return code 0, `/health` is unreachable by default, and the container runs as root | Medium | 68% | — | QA-done |
| [`F-SEN-004`](../findings/F-SEN-004.md) | The sentinel bonds on every proposal with no cap on concurrent engine checks, outstanding bonds or reveal throughput … | Medium | 62% | — | Critiqued |
| [`F-CORE-011`](../findings/F-CORE-011.md) | The shared provider is built with no timeout, retry or rate-limit layer, so a stalled RPC connection stalls indexing indefinitely with no error, no metric and `/health` … | Medium | 60% | — | Draft (C-promoted) |
| [`F-CORE-062`](../findings/F-CORE-062.md) | An allocated nonce is never released and allocation is floored at `MAX(nonce)+1`, so one bad nonce wedges the queue permanently with no error, metric or recovery path | Medium | 60% | — | Critiqued |
| [`F-CORE-061`](../findings/F-CORE-061.md) | `is_transaction_underpriced` only matches replacement rejections, so a first-submission fee rejection retries at an unchanged fee forever and blocks every later nonce | Medium | 58% | — | Critiqued |
| [`F-CORE-063`](../findings/F-CORE-063.md) | Execution is inferred from the account nonce alone and invalidated only by a block-number regression, so a transaction can be marked executed, pruned and silently lost | Medium | 55% | — | Critiqued |
| [`F-VAL-060`](../findings/F-VAL-060.md) | Coordinator and Consensus events are dispatched without checking the emitting contract address | Medium | 50% | — | QA-done |
| [`F-VAL-067`](../findings/F-VAL-067.md) | The Rust DKG-abort test counts complaints cumulatively while the contract's equivalent counter is decremented by every response … | Medium | 48% | — | QA-done (C-promoted) |
| [`F-XC-050`](../findings/F-XC-050.md) | No DKG event handler checks group membership, so one injected `KeyGenConfirmed` closes the confirmation round early and silently finalises genesis with no key share | Medium | 48% | — | QA-done |
| [`F-XC-011`](../findings/F-XC-011.md) | Four RUSTSEC advisories and eleven warnings are live in `Cargo.lock`; exactly one is reachable from a network-facing surface, and it is not the one with the highest CVSS | Low | 95% | V | Verified |
| [`F-VAL-062`](../findings/F-VAL-062.md) | Secret-bearing effects and resumes derive `Debug` and are printed at `warn`, unlike every other secret type in the crate | Low | 88% | V | Verified (leak refuted) |
| [`F-XC-002`](../findings/F-XC-002.md) | Secret-bearing types reach log statements through derived `Debug`; the redaction policy is inconsistent and nothing enforces it | Low | 88% | V | Verified (leak refuted) |
| [`F-CORE-036`](../findings/F-CORE-036.md) | The runtime requires `Debug` on every service `Effect` and `Resume` and prints them at `trace` in five places … | Low | 85% | V | Verified (secret leg refuted) |
| [`F-SEN-006`](../findings/F-SEN-006.md) | Emitted actions are not idempotent under replay, so every restart and reorg enqueues duplicate `approve`/`commit`/`reveal`/`finalize`/`claim` transactions that revert | Low | 85% | — | Critiqued |
| [`F-SEN-012`](../findings/F-SEN-012.md) | The engine client makes exactly one attempt per proposal, so any transient failure inside a window that still has blocks left is a permanent abstention | Low | 85% | — | Critiqued |
| [`F-XC-004`](../findings/F-XC-004.md) | All three runtime images run as root, pin no base-image digest, and silently discard the build's provenance argument | Low | 85% | — | QA-done |
| [`F-XC-052`](../findings/F-XC-052.md) | `decode_multi_send` synthesises sub-transactions with `chain_id`, `nonce` and every refund field zeroed — the identical construction that made `RefundChecker` dead code | Low | 85% | — | QA-done |
| [`F-SEN-009`](../findings/F-SEN-009.md) | The engine timeout is derived from an unvalidated config value instead of the oracle's real commit window … | Low | 82% | — | Critiqued |
| [`F-SEN-011`](../findings/F-SEN-011.md) | A restart orphans any in-flight engine check whose proposal is older than the rollback anchor: the request is never re-checked and silently expires without a vote | Low | 82% | — | Critiqued |
| [`F-ENG-005`](../findings/F-ENG-005.md) | The engine has no deadline anywhere: `x-request-timeout` is parsed and discarded, there is no server timeout or concurrency limit … | Low | 80% | — | QA-done |
| [`F-ENG-006`](../findings/F-ENG-006.md) | `decode_target_effects` recurses through MultiSend with no depth limit; the only thing keeping attacker-chosen depth away from it is undocumented, untested checker … | Low | 80% | — | QA-done |
| [`F-ENG-007`](../findings/F-ENG-007.md) | Shutdown drops the serve future instead of draining it, so every in-flight security check is aborted mid-request and the sentinel loses those votes on every deploy | Low | 80% | — | QA-done |
| [`F-SEN-007`](../findings/F-SEN-007.md) | No balance, allowance, registration or chain pre-check: a sentinel that cannot possibly commit still pays for an `approve` and a reverting `commit` on every single … | Low | 80% | — | Critiqued |
| [`F-XC-008`](../findings/F-XC-008.md) | Both outbound HTTP clients are built with library defaults: proxy environment honoured, redirects followed, and the CoW client has no timeout | Low | 80% | V | QA-done |
| [`F-XC-010`](../findings/F-XC-010.md) | The sentinel engine exports no metrics of its own: it serves a Prometheus endpoint that says nothing about checkers, verdicts or their failures … | Low | 80% | V | QA-done |
| [`F-CORE-009`](../findings/F-CORE-009.md) | The block-watcher configuration accepts values with no range validation: `block_time = 0` with empty retry delays is a delay-free RPC poll loop, `max_reorg_depth` is an … | Low | 78% | — | Critiqued |
| [`F-ENG-038`](../findings/F-ENG-038.md) | CoW shape recognisers accept batches their paired decoders reject, turning a dangling relayer approval from `insecure` into `abstain` | Low | 78% | — | QA-done |
| [`F-XC-006`](../findings/F-XC-006.md) | Nothing binds a deployment to a chain: no config field, no persisted column, and the legacy configuration had one | Low | 78% | — | QA-done |
| [`F-CORE-005`](../findings/F-CORE-005.md) | `max_reorg_depth = 0` documents "fail loudly on any reorg" but silently disables the uncled-block recovery path, turning the case it exists for into an infinite retry … | Low | 75% | — | Critiqued |
| [`F-ENG-009`](../findings/F-ENG-009.md) | `EngineConfig` performs no validation: the lookback and max-range pair silently sets the per-request `eth_getLogs` fan-out, with no bound, no derived-value check and no … | Low | 75% | — | QA-done |
| [`F-ENG-043`](../findings/F-ENG-043.md) | The CoW order lookup has no client timeout and puts an unbounded, unvalidated attacker-controlled `orderUid` into the request URL | Low | 74% | — | QA-done |
| [`F-XC-009`](../findings/F-XC-009.md) | Sample configs demonstrate dangerous values: a well-known private key as the signer placeholder, `0.0.0.0` binds for the unauthenticated listeners … | Low | 72% | — | QA-done |
| [`F-CORE-008`](../findings/F-CORE-008.md) | Block polling is scheduled by comparing chain timestamps against the host wall clock, so host clock skew silently and permanently delays indexing … | Low | 70% | — | Critiqued |
| [`F-CORE-040`](../findings/F-CORE-040.md) | The driver's inner `select!` restarts the watcher's in-flight RPC request on every effect resume, so a wide effect fan-out is paid for in abandoned `eth_getLogs` calls | Low | 65% | — | Critiqued |
| [`F-CORE-037`](../findings/F-CORE-037.md) | Snapshots are an unversioned JSON dump of the service state with no migration path and no recovery from a decode failure: an upgrade that changes a state type bricks … | Low | 62% | — | Critiqued |
| [`F-CORE-007`](../findings/F-CORE-007.md) | A node that keeps disagreeing with itself during startup puts `BlockWatcher::initialize` in an unbounded, undelayed RPC loop that is invisible at the default log level … | Low | 60% | — | Critiqued |
| [`F-XC-003`](../findings/F-XC-003.md) | `deny_unknown_fields` is combined with `#[serde(flatten)]` in the validator and sentinel configs, and no test proves a mistyped key is rejected | Low | 58% | V | QA-done |
| [`F-CORE-006`](../findings/F-CORE-006.md) | The event watcher matches on the cross product of watched addresses and watched topics, so any watched address can emit any watched event and the decoded value carries … | Low | 55% | — | Critiqued |
| [`F-CORE-039`](../findings/F-CORE-039.md) | Graceful shutdown is bounded only by the RPC's own patience: the shutdown branch is unreachable while an input is being processed … | Low | 55% | — | Critiqued |
| [`F-CORE-065`](../findings/F-CORE-065.md) | No chain-id or deployment binding on the `transactions` table, and `Provider::chain_id` is cached at connect so an endpoint chain change is undetectable | Low | 55% | — | Critiqued |
| [`F-VAL-034`](../findings/F-VAL-034.md) | `handle_nonces` applies a nonce resume to whatever session holds the message, without checking the signature id | Low | 55% | V | Verified (outcome benign) |
| [`F-VAL-038`](../findings/F-VAL-038.md) | Nonce chunk generation saturates every core and then holds the shared SQLite writer for 1025 statements, competing with the driver's own snapshot commits | Low | 55% | V | QA-done |
| [`F-SEN-008`](../findings/F-SEN-008.md) | Hard-coded gas limits and an unconditional non-zero `approve` assume a plain ERC-20; a proxied, hooked or non-zero-to-non-zero-reverting fee token breaks every commit | Low | 52% | — | Critiqued |
| [`F-VAL-040`](../findings/F-VAL-040.md) | `last_signer` is overwritten by every accepted nonce reveal and the contract does not deduplicate reveals … | Low | 50% | — | QA-done (C-promoted) |
| [`F-CORE-010`](../findings/F-CORE-010.md) | The `-32001` recovery commits the block watcher's rewind before the event watcher accepts it … | Low | 45% | — | Draft (C-promoted) |
| [`F-CORE-032`](../findings/F-CORE-032.md) | A failed effect task is logged and skipped, so a panicking effect silently removes a resume the state machine is waiting for … | Low | 45% | — | Critiqued |
| [`F-VAL-031`](../findings/F-VAL-031.md) | A dead nonce-generation worker thread is never detected, logged, or restarted | Low | 42% | — | QA-done |
| [`F-XC-051`](../findings/F-XC-051.md) | `verify_commitment` deliberately delegates the DKG commitment's only structural validation to a contract that is not in the event path, and accepts identity coefficients | Low | 42% | — | QA-done |
| [`F-VAL-036`](../findings/F-VAL-036.md) | `NonceState::observe` accepts a non-monotonic sequence and rewinds `next_sequence`, inflating the measured nonce capacity | Low | 40% | — | QA-done |
| [`F-VAL-035`](../findings/F-VAL-035.md) | Secret nonce material is copied into unzeroised JSON strings, abandoned chunks are never pruned … | Low | 35% | V | Verified (leg c refuted) |
| [`F-SEN-013`](../findings/F-SEN-013.md) | A single undecodable `Revealed.reason` from any active sentinel would stall every other sentinel's indexer permanently … | Informational | 98% | V | **Refuted as filed** |
| [`F-SEN-014`](../findings/F-SEN-014.md) | Every participating sentinel submits `finalize` for every request, so all but one revert | Informational | 88% | — | Critiqued |
| [`F-CORE-038`](../findings/F-CORE-038.md) | `kdf::derive_key`'s multi-part `info` is a plain concatenation, but the doc comment implies otherwise: a public API whose only safe use is undocumented | Informational | 85% | — | Critiqued |
| [`F-ENG-004`](../findings/F-ENG-004.md) | Two Charter citations in `RuleId` are wrong: R-4.3 attributes a verbatim quote to § 2.4 Notes, which does not contain it … | Informational | 85% | — | QA-done |
| [`F-ENG-040`](../findings/F-ENG-040.md) | Every MultiSend denial is reported as R-4.2, even when the failing sub-call is a settings-change violation | Informational | 85% | — | QA-done |
| [`F-ENG-041`](../findings/F-ENG-041.md) | A first-time recipient with no established history only ever abstains, so a novel-address drain is never denied | Informational | 85% | — | QA-done |
| [`F-SEN-010`](../findings/F-SEN-010.md) | The sample config ships zero addresses that parse and start cleanly, and the pending "sensible default" decision keeps the zero-address failure mode alive | Informational | 85% | — | Critiqued |
| [`F-XC-007`](../findings/F-XC-007.md) | Dependency surface is wider than the code needs and no advisory gate exists in CI | Informational | 84% | V | QA-done |
| [`F-ENG-008`](../findings/F-ENG-008.md) | `openapi.yaml`, the declared authoritative interface contract, documents only `200` while the engine provably returns `400`s … | Informational | 80% | — | QA-done |
| [`F-XC-001`](../findings/F-XC-001.md) | No release profile: overflow checks and debug assertions are off in every shipped binary | Informational | 66% | V | QA-done |
| [`F-VAL-037`](../findings/F-VAL-037.md) | Merkle trees pad with `B256::ZERO` and have no leaf/internal domain separation, so `B256::ZERO` is a provable leaf of most trees - safe today only by accident of what … | Informational | 60% | — | QA-done |

`F-SEN-013`'s claim was **refuted by execution** and it is retained as Informational so the refutation stays visible (§6).

### Findings scoped to unmerged branches (not counted above)

These describe defects in **open, unmerged PRs** and are not defects on `main`. They are excluded from every count above; see [`IN-FLIGHT.md`](IN-FLIGHT.md).

| ID | Title | Severity | Certainty | Markers | Status |
| --- | --- | --- | --- | --- | --- |
| [`F-CORE-068`](../findings/F-CORE-068.md) | A batch's execution status is unobservable, so a whole-batch revert or a mid-batch `CallFailed` silently drops up to a full batch of actions | High (forward-looking) | 85% | — | Filed (unmerged) |
| [`F-CORE-069`](../findings/F-CORE-069.md) | The two-nonce delegation reservation writes a permanent nonce gap into durable state on the epic's own acknowledged failure path, and the only alarm fires exactly once | High (forward-looking) | 80% | — | Filed (unmerged) |
| [`F-VAL-068`](../findings/F-VAL-068.md) | Scheduled secret pruning delays but does not prevent reorg-driven DKG secret loss, and adds a restart window in which no nonce generator runs | High (forward-looking) | 85% for D1–D4 | — | Filed (unmerged) |
| [`F-SEN-016`](../findings/F-SEN-016.md) | Sentinel deadline and optimistic-transition PRs: #914 alone drops a valid peer commit, and #915 discards effect results on warp rollback and re-queues actions after a reorg | High (forward-looking) | 85% for D1 | — | Filed (unmerged) |

---

## 5. Findings by severity

Trail key: reviewer basis · Critic verdict · PoC · final certainty · phases executed.

### Critical (4)

#### [`F-ENG-030`](../findings/F-ENG-030.md) — 99 % · `NestedSafeChecker` affirms any `execTransaction`-shaped call while ignoring `value`

`nested.rs:42-47`. Returns `Secure` for any `Operation::Call` to an address other than the Safe whose calldata starts with the `execTransaction` selector and ABI-decodes. It never inspects `value`, `gas_price`, `gas_token`, `refund_receiver` or the identity of `to` — which need not even be a contract. At position 5 of 10 it also suppresses the five checkers behind it. **Trigger:** one `POST /v1/security-check` with an attacker-chosen `to` and the Safe's full balance in `value`. **Fix:** require `value.is_zero && gas_price.is_zero` before affirming, or make the checker abstain-only — a nested `execTransaction` is a reason not to deny, not evidence of security. **Trail:** `E1`+E2×6 · C-ENG-B Confirmed · `poc/F-ENG-030/` · 99 % · Phases 5, 8.

#### [`F-ENG-031`](../findings/F-ENG-031.md) — 99 % · The gas-refund leg is never vetted on any affirmed transaction

`refund.rs:97-103`. Four of the six checkers that can return `Secure` — `NestedSafeChecker`, `CowChecker`, `StakingChecker`, `AddressPoisoningChecker` — reach it without reading `gas_price`, `base_gas`, `gas_token` or `refund_receiver`. `RefundChecker` is deny-only, runs 9th behind all four, and abstains for the largest-impact case (native refund, `gas_token == 0`); it is also dead for the ERC-20 case (`F-ENG-032`). **Trigger:** a single mainnet `claim` to the canonical rewards distributor with the Safe as `account` and a hostile native refund leg — deterministic, no RPC, no CoW API. **Fix:** a chain-wide pre-gate refusing to affirm any `gas_price != 0` transaction no checker can positively vet, or run every denier before considering any affirmation. **Never option 2** (deny). **Trail:** `E1`+E2×7 · C-ENG-B Confirmed · `poc/F-ENG-031/` · 99 % · Phases 5, 8.

#### [`F-ENG-033`](../findings/F-ENG-033.md) — 99 % · `AddressPoisoningChecker` affirms from attacker-supplied history and ignores `value`

`address_poisoning.rs:116-139, :192-222, :321-333`. Two weaknesses in one affirmation: `value` is never read, so a `Call` carrying `transfer(<a paid-before address>, 1)` with `value = <entire balance>` is rated `secure`; and the evidence source is the attacker-chosen `to`, so a forged `Transfer` on the attacker's own non-token contract counts as a genuine prior interaction. **Fix:** require `tx.value.is_zero` in `decode_target`; downgrade `ExactMatch` from `Secure` to `Abstain`; require independent standing (code-size/deployment-age probe, allow-list) before an address's logs count as evidence. **Trail:** `E1`+E2×5 · C-ENG-B Confirmed · `poc/F-ENG-033/` · 99 % · Phases 5, 8.

#### [`F-VAL-001`](../findings/F-VAL-001.md) — 97 % · DKG encryption key `q` has no proof of possession

`frost/keygen.rs:79-100`, `frost/ecdh.rs:110-121`. Nothing binds `q` to its publisher: the Rust validates only that it decodes to a non-identity point, the coordinator only that `q != 0` (`FROSTCoordinator.sol:377`), and the proof of knowledge covers the polynomial commitment vector `c`, not `q`. The pad is the plain unhashed ECDH x-coordinate and therefore symmetric, so a registered participant `M` publishing `q_M := q_A` makes every peer's pad to `M` identical to its pad to `A`, and `M` harvests those pads through the complaint mechanism. **Trigger:** one registered validator, inside the `< 1/3` fault bound for every `n >= 4`. **Fix:** a ceremony-bound KDF (`F-VAL-002` opt 1) **and** a Schnorr proof of possession over `(gid, participant, q)` (opt 2) — the KDF alone leaves a liveness variant. Reusing `C[0]` as the ECDH key (opt 3) makes the existing PoK do double duty. **Trail:** `E1`+E2×11, I×1 · C-VAL-A Confirmed · `poc/F-VAL-001/` · 97 % · Phases 5, 8 (real bytecode).

### High (20)

#### [`F-CORE-001`](../findings/F-CORE-001.md) — 99 % · A reorg during downtime silently defeats `max_reorg_depth`

`index/blocks.rs:244-289`. Nothing in the persisted state identifies the chain it came from: `snapshots` stores `(block_number, state)` and no hash, and `BlockWatcher::initialize` re-anchors on whatever the RPC calls `latest`. While running, a reorg replacing the `safe` anchor is fatal; after a stop/start the same reorg produces no error and the oldest retained snapshot — roughly `head - max_reorg_depth` — is accepted as the anchor whether or not its block is canonical. **A/B on one chain, depth-11 reorg:** running → `ERROR ExceededMaxReorgDepth(5)` and exit; across a restart → alive, **0 WARN / 0 ERROR in 2,731 lines**, resuming on orphaned block numbers. **Fix:** persist the anchor's identity (`block_hash`, plus `chain_id` and an address digest). Option 2 ("walk back") cannot work alone — the retained window is exactly the fatal depth. **Trail:** `E1`+E2×7 · C-CORE-A Confirmed · `poc/F-CORE-001/` · 99 % · Phases 5, 7, 8.

#### [`F-CORE-002`](../findings/F-CORE-002.md) — 99 % · The log-completeness check disables itself on the failures it exists to detect

`index/events.rs:362-398`. `use_client_filtering` is the handbook's remedy for providers returning partial `eth_getLogs`. Its bloom-equality check runs only while `retries < block_single_query_retry_count` (default 3); every failure — including the `IncompleteLogs` error the check itself raises — increments that counter, after which the watcher falls back permanently for that block to a node-filtered query with **no completeness check at all**. **Measured A/B:** 3 × HTTP 429 then one empty `eth_getLogs` → accepted silently, logs lost, sentinel **−4,000** with 2,000 slashed; the control with the budget intact rejected the same answer (_"incomplete logs served for block, bloom filter mismatch"_) and finished **+500**. **Fix:** never drop the check while it is enabled; bloom-check the fallback; separate transport-failure and integrity budgets. **Trail:** `E1`+E2×7 · C-CORE-A Confirmed · `poc/F-CORE-002/` · 99 % · Phases 5, 8.

#### [`F-ENG-002`](../findings/F-ENG-002.md) — 99 % · R-4.5 denies standard NFT-marketplace approvals

`engine/rule.rs:28-33`. The Charter makes ERC-721/1155 operator approval-for-all unlimited only "unless plausibly required for the stated interaction"; its immediate-failure branch names only the max-`uint256` ERC-20 case. The doc comment erases that line and the base checker implements the doc comment. **Trigger:** `setApprovalForAll(<marketplace conduit>, true)` — the required first transaction for listing an NFT from a Safe — denied `insecure R-4.5`, live, from a Safe that really owns the NFT. **No configuration can exempt it.** **Fix:** abstain on `OperatorApproval` and fix the doc comment; not opt 3 (history as a proxy). **Trail:** `E1`+E2×6 · C-ENG-A Confirmed · `poc/F-ENG-002/` · 99 % · Phases 5, 8.

#### [`F-ENG-044`](../findings/F-ENG-044.md) — 99 % · First-non-abstain-wins cannot implement Charter §3.7

`engine/mod.rs:57-72`. The engine returns the verdict of the first non-abstaining checker; everything registered after it never runs. Charter §3.7 requires the conjunction, and the type's own doc ("All configured checks consider the transaction secure") is false for every `Secure` the engine has ever returned except one from the last checker. **Executed:** `[Secure, denial]` returns `Secure`; on production wiring with an operator-populated blocklist the same `to` returns `insecure R-4.6` with plain calldata and **`secure` when prefixed with `announceTransaction` (`0x7b328c10`)**, `BlocklistChecker` never running. Severity kept High deliberately — its Critical impacts are carried by the separately-filed instances. **Fix:** make affirmation conjunctive, or split the verdict type so no checker can say "secure overall". Not opt 3 (a hand-maintained ordering table). **Trail:** `E1`+E2×5 · drafted by C-ENG-B, self-assessed 99 % · `poc/F-ENG-044/` · 99 % · Phases 5, 8.

#### [`F-VAL-005`](../findings/F-VAL-005.md) — 99 % · A reorg across the key-generation block deletes DKG secrets the store promises never to overwrite

`state/preprocess.rs:127-165`, `secrets/store.rs:98-124`. `store_keygen_secrets` documents that existing secrets are never overwritten, so a reorged-and-re-included commitment stays consistent with the shares the validator can still produce. `ReconcileGroupSecrets` deletes them anyway on the rollback path; the validator resamples and can no longer produce shares matching its own onchain commitment. **Reproduced end to end on the epoch-1 group:** group `0x6765b9e6…` resamples after the reorg and **both** validators fail with `IncorrectCommitment` / `next_epoch: "1"` — a 2-of-2 group, so network-wide epoch-1 loss is confirmed live. Exhibited by a passing suite (§1). **Fix:** never delete keygen secrets on the reconciliation path (expire on a block clock), or compute the retention set from the _safe_ block. Not opt 4. **Trail:** `E1`+E2×11 · C-VAL-A Confirmed · `poc/F-VAL-005-066/` · 99 % · Phases 5, 7, 8.

#### [`F-CORE-060`](../findings/F-CORE-060.md) — 98 % · Unbounded per-block fee ratchet bypasses `priority_fee_cap_percentage`

`tx/fees.rs:52-56`. A transaction the node keeps rejecting as an underpriced _replacement_ has both fee fields multiplied by 1.1 every block, compounding, with no ceiling; the configured cap is applied only to the fresh estimate and is silently overridden. The only brake is the signer's balance. **Measured:** tip 1 → 11,527 → 201,207 wei; max fee 4,239 gwei against a real base fee of 772 wei — the cap bypassed **~28,700×**. **Limit:** it does not self-start on a healthy node — Anvil accepts the code's own 10 % bump, so the ratchet needs a stale fee floor (restored database, or a foreign transaction at the nonce). **Fix:** an absolute configured ceiling; opt 2 alone cannot bound an absolute fee. **Trail:** `E1`+E2×11 · C-CORE-B Confirmed · `poc/F-CORE-060/` · 98 % · Phases 5, 8.

#### [`F-SEN-001`](../findings/F-SEN-001.md) — 98 % · Replay discards the sentinel's own `Committed`, so it never reveals and is slashed

`sentinel/service.rs:307-319, 413-417`. `self_committed` is set only by observing our own `Committed` while in `CollectingCommitments`. Every restart and every reorg within `max_reorg_depth` replays the range and re-spawns the engine check, so the replayed `Committed(self)` arrives in `WaitingForEngineCheck` and is discarded with a `warn`; at `commit_deadline + 1` the entry is dropped with **no `Reveal`** while the commitment is live onchain. **Cost measured:** 2,000 slashed to the funds receiver, 2,000 left locked — **−4,000 fee tokens**. The loss is unconditional: the warp-ordering control test passed, so there is no race to win. **Fix:** make `self_committed` derivable in every phase, or reconcile against `getCommitment` before dropping. **Not opt 3** (§3). **Trail:** `E1`+E2×7 · C-SEN Confirmed · `poc/F-SEN-001/` · 98 % · Phases 5, 8.

#### [`F-SEN-002`](../findings/F-SEN-002.md) — 98 % · Early finalisation with `self_revealed == false` abandons bond and reward

`sentinel/service.rs:307-319, 372-384, 626-633`. `committed_count` starts at 0 when `commit_vote` runs and counts only later `Committed` logs, so any peer commitment landing before our engine answers is invisible; `revealed_count >= committed_count` then fires early and `finalize` returns without `Finalize` or `Claim`. No attacker and no restart needed — only a slower engine. **Cost measured: 4,500 (bond + reward) left unclaimed on a real `SentinelOracle`.** **Fix:** never drop a bonded entry silently; stop early-finalising on a local tally; tally commitments in every pre-commit phase. **Trail:** `E1`+E2×5 · C-SEN Confirmed · `poc/F-SEN-002/` · 98 % · Phases 5, 8.

#### [`F-VAL-061`](../findings/F-VAL-061.md) — 98 % · Every effect failure becomes `Resume::Noop`, stranding state written in anticipation

`service/effect.rs:243-256`. One failure policy: forget it happened. That is safe only for effects whose state is written after the resume — but `Effect::NonceTree` and `Effect::KeyGenSetup` both write a placeholder first (a `None` chunk reservation; `KeyGenCommitment::Participating{secrets: None}`), and nothing re-issues them. **Observed live and unforced, inside a _passing_ suite:** `failed to perform effect NonceTree … "nonce generator is unavailable"` → `Resume::Noop`, with **zero** later `NonceTree` spawns. **Fix:** a per-effect failure policy, self-healing state, and an `effects_total` result label so a stranded reservation is alertable. The retry (opt 2) **must wait for `F-VAL-005`**. **Trail:** `E1`+E2×16 · C-VAL-B Confirmed · `poc/F-VAL-030-032-061/` · 98 % · Phases 5, 7, 8.

#### [`F-ENG-034`](../findings/F-ENG-034.md) — 97 % · `EscapeHatchChecker` affirms for any `to` and runs ahead of the blocklist

`escape_hatch.rs:52-61`. Affirms on `operation == Call`, zero `value` and `gas_price`, and an `announceTransaction`/`cancelAnnouncement` selector prefix — no constraint on `to`, no ABI decode of the arguments. The Guard's own rule is narrower (`_isAutoAllowed` requires `to == address(this)`), so a blocklisted R-4.6 target is rated `secure`. **Fix:** move it after `BlocklistChecker`, constrain `to` to a registered SafenetGuard, or return `Abstain`. **Trail:** `E1`+E2×5 · C-ENG-B Confirmed · `poc/F-ENG-034/` · 97 % · Phase 5.

#### [`F-SEN-015`](../findings/F-SEN-015.md) — 97 % · A replayed engine check re-decides an already-committed vote

`sentinel/service.rs:150-194, 198-244`. `reveal` recomputes `keccak256(abi.encodePacked(approve, salt, sentinel, requestId, reason))` and reverts `InvalidReveal` on any difference; `approve` and `reason` both come from a live HTTP call that a replay repeats, and the second verdict overwrites the reason the commitment was built from. **Executed:** the duplicate commit reverted `AlreadyCommitted` and the **reveal reverted `InvalidReveal 0x9ea6d127`**, bond slashed. At the default `max_reorg_depth` the re-decision still fires, but `F-SEN-001` wins the race for the same 4,000. **Fix:** never re-decide a request already committed onchain, or persist `(request_id, approve, reason)` when it is produced. **Not opt 2** (unimplementable) **or opt 3** (§3). **Trail:** `E1`+E2×7 · drafted by C-SEN, self-assessed 97 % · `poc/F-SEN-015/` · 97 % · Phases 5, 8.

#### [`F-VAL-030`](../findings/F-VAL-030.md) — 97 % · A lost `NonceTree` effect leaves a phantom chunk reservation counted as capacity

`state/preprocess.rs:85-103, 234-247`. The reservation is durable and counts as 1024 usable nonces; the effect that fills it is not durable and is never re-issued. `available` keeps returning `>= 1024`, so `handle_nonce_topup` never fires again and nothing repairs it — silent, self-inflicted exclusion from consensus. **Live:** the stranded reservation reproduced (no chunk beyond chunk 0 was ever linked); the downstream 1024-sequence sign refusal is not reachable in a local harness. **Fix:** re-emit `Effect::NonceTree` for any reservation still `None`, and exclude `None` reservations from `available`. **Trail:** `E1`+E2×11 · C-VAL-B Confirmed · `poc/F-VAL-030-032-061/` · 97 % · Phases 5, 7, 8.

#### [`F-ENG-037`](../findings/F-ENG-037.md) — 96 % · CoW TWAP approval tolerance is sized by an attacker-chosen `n`

`cow.rs:544-554`. `max_approval_for_twap_total(total, n) = total + (n - 1)` takes both `total` and `n` from the attacker's own `staticInput`: `partSellAmount = 0` with `n = U256::MAX` makes the ceiling `U256::MAX - 1`, so `approve(GPv2VaultRelayer, 2^256 - 2)` is affirmed. **Fix:** bound the headroom independently of `n` and reject degenerate orders (`partSellAmount > 0`, `n > 0`). Opt 3 reverses `cow.rs:348-350`'s deliberate policy. **Trail:** `E1`+E2×4 · C-ENG-B Confirmed · `poc/F-ENG-037/` · 96 % · Phase 5.

#### [`F-ENG-036`](../findings/F-ENG-036.md) — 94 % · R-4.5 implemented as an exact `U256::MAX` comparison

`excessive_approval.rs:19-33`. Denies only a bit-for-bit `U256::MAX` approval; `2^256 - 2`, `2^255` or `type(uint128).max` abstain — and `AddressPoisoningChecker`, four positions later, then affirms them `secure` on prior history. The Charter's test is "materially exceeds what is plausibly needed", and it says explicitly that an approval can be functionally unlimited without being max `uint256`. **Fix:** a policy that can express "functionally unlimited"; stop `AddressPoisoningChecker` affirming `approve` at all; add `increaseAllowance` to the selector chain. **Trail:** `E1`+E2×5 · C-ENG-B Confirmed · `poc/F-ENG-036/` · 94 % · Phase 5.

#### [`F-ENG-035`](../findings/F-ENG-035.md) — 93 % · The blocklist checks only the top-level `to`

`blocklist.rs:24-32`. Invisible to it: ERC-20 `transfer`/`transferFrom` recipients, `approve` spenders and `setApprovalForAll` operators, every MultiSend sub-call destination, the inner `to` of a nested `execTransaction`, and `gas_token`/`refund_receiver`. A blocklisted address with prior history is then affirmed `secure`. **Fix:** check every address the transaction reaches, and give the blocklist precedence over affirmations. **Trail:** `E1`+E2×4 · C-ENG-B Confirmed · `poc/F-ENG-033/`, `poc/F-ENG-035/` · 93 % · Phase 5.

#### [`F-VAL-004`](../findings/F-VAL-004.md) — 93 % · One failed `KeyGenSetup` effect stalls genesis forever

`state/keygen.rs:41-54, 992-1109`. Genesis deliberately runs without a deadline, and every recovery arm in `handle_key_gen_timeouts` is gated on a deadline being present, so it is a complete no-op while `next_epoch == EpochId::Genesis`. The effect is emitted once, spawned once, never retried, and its failure is swallowed to `Resume::Noop`. **Live:** a validator restarted inside the genesis window never finalises genesis, reports "permanently halted", and no retry arm fires — network bootstrap blocked. **Fix:** re-issue the effect whenever the state is `Participating { secrets: None }`. **Not opt 2** (a genesis deadline reaches `Halted`). The retry must follow `F-VAL-005`. **Trail:** `E1`+E2×9, I×1 · C-VAL-A Confirmed · `poc/F-VAL-004/` · 93 % · Phases 5, 7, 8.

#### [`F-VAL-032`](../findings/F-VAL-032.md) — 93 % · A `Sign` with no linked nonce chunk permanently discards the signing session

`state/sign.rs:30-35, 106-114`. `handle_sign` removes the session before knowing it can serve the request; the `(None, Some(WaitingForRequest { .. }))` arm logs a warning and never puts it back. The validator then misses any restart of that ceremony and the fallback attestation; for a `Packet::EpochRollover` there is no re-proposal path at all. Reached by this validator's own stranded chunk (`F-VAL-030`, precondition reproduced live) or by a third party burning sequences; the discard itself needs sequence ≥ 1024, not reachable locally. **Fix:** re-insert the session, or resolve the nonce before removing it. **Trail:** `E1`+E2×6 · C-VAL-B Confirmed · `poc/F-VAL-030-032-061/` · 93 % · Phases 5, 7, 8.

#### [`F-VAL-066`](../findings/F-VAL-066.md) — 92 % · `ReconcileGroupSecrets` deletes from a set computed before the block's logs

`service/effect.rs:202-238`. The retention set is computed inside the `NewBlock` transition — before any of that block's logs are applied — and becomes two unconditional `DELETE … WHERE group_id NOT IN (…)` statements. The block's log transitions and the effects they spawn run concurrently over the same pool, and nothing re-checks the set at execution time. Phase 7 observed the concurrent structure directly at block 14 (the retention set demonstrably excludes the group that block's logs introduce); the harmful inversion was **not** observed — in that run the delete committed ~17 ms before the insert, the benign order. **Fix:** compute the set at execution time, serialise all `SecretStore` mutations, and never issue an unqualified delete. Not opt 4. **Trail:** `E1`+E2×11 · C-VAL-B **Plausible** · `poc/F-VAL-005-066/` · 92 % · Phases 5, 7.

#### [`F-VAL-033`](../findings/F-VAL-033.md) — 72 % · Restoring the database across a reorg un-burns a signing nonce

`secrets/store.rs:198-218`. The only thing preventing FROST nonce reuse is that `take_nonce` deletes the row — a property of the current database file, not of history; the handbook instructs operators to back that file up, twice, with no caveat. A restore alone is safe (state and secrets rewind together); a restore spanning a reorg rebinds a consumed sequence to a different message. **Severity Critical → High, 85 → 72 %:** the un-burn is real and reproduced, but in two live runs the restore drove the validator into a permanent genesis self-halt **before any reuse** — self-inflicted DoS, not key leakage. **Fix:** an append-only `nonces_consumed` row written in the same transaction as the `DELETE` (opt 1), or message-matched replay (opt 3). **Never opt 2**; both must precede `F-VAL-038` opt 3. **Trail:** `E1`+E2×7 · C-VAL-B **Plausible** · `poc/F-VAL-033/` · 72 % · Phases 5, 7, 8.

#### [`F-VAL-039`](../findings/F-VAL-039.md) — 58 % · ~100 sequences of headroom against a permissionless group-wide counter

`state/preprocess.rs:15-17, 85-103`. Top-up triggers at `available < 100` and one reservation restores 1024, but the attacker need not exhaust a chunk — only keep the group's sequence ahead of the validators' _linked_ chunk for one `preprocess` round trip. Nothing rate-limits `Coordinator.sign`: at most 1024 calls from cold, typically ~100 near a chunk boundary. **Fix:** keep a linked chunk in reserve, scale the threshold with observed demand, and give `Action::Preprocess` an expiry and priority. **Trail:** E2×7 · drafted by C-VAL-B, Confirmed 58 % · `poc/F-VAL-030-032-061/` · 58 % · not executed.

### Medium (32)

| ID | Cert. | Claim |
| --- | --: | --- |
| [`F-ENG-032`](../findings/F-ENG-032.md) | 99% | `RefundChecker` is dead: its synthetic refund transfer carries `chainId = 0` … |
| [`F-CORE-067`](../findings/F-CORE-067.md) | 98% | `Command::Action` has no replay contract and the queueing path has no de-duplication … |
| [`F-VAL-002`](../findings/F-VAL-002.md) | 93% | The ECDH share pad is an unhashed x-coordinate used in both directions of every pair … |
| [`F-XC-005`](../findings/F-XC-005.md) | 92% | The engine sample config pairs a 50,000-block single-call lookback with a public RPC and no range cap, which … |
| [`F-SEN-005`](../findings/F-SEN-005.md) | 86% | `WaitingForDisputeResolution` never expires and the sentinel never calls the permissionless `timeoutArbitration` … |
| [`F-CORE-030`](../findings/F-CORE-030.md) | 85% | `Driver::run` discards its outcome, so every unrecoverable error exits the process with status 0 and the only … |
| [`F-ENG-001`](../findings/F-ENG-001.md) | 85% | `RuleId::R4_1SettingsChange`'s stated meaning is far wider than Charter R-4.1's allowed exception … |
| [`F-CORE-034`](../findings/F-CORE-034.md) | 80% | Every watcher error is retried at a fixed 100 ms forever, with one warning line per attempt … |
| [`F-ENG-039`](../findings/F-ENG-039.md) | 80% | `BaseChecker`'s Article IV Part A allow-lists are materially wider than the Charter's R-4.1/R-4.2 exceptions … |
| [`F-VAL-003`](../findings/F-VAL-003.md) | 80% | A DKG complaint compels a plaintext share reveal with no check that the plaintiff ever received a share, no … |
| [`F-CORE-031`](../findings/F-CORE-031.md) | 78% | Effects are spawned only after the snapshot that records them as pending … |
| [`F-CORE-035`](../findings/F-CORE-035.md) | 78% | The driver classifies _every_ RPC error as intermittent and swallows it forever … |
| [`F-CORE-066`](../findings/F-CORE-066.md) | 78% | `tx::Config` accepts values that silently disable or destabilise the queue … |
| [`F-ENG-042`](../findings/F-ENG-042.md) | 78% | An address-poisoning denial is issued from an evidence set bounded by recency and by provider completeness … |
| [`F-CORE-004`](../findings/F-CORE-004.md) | 75% | The event watcher has no terminal error state: deterministic, content-dependent failures keep the indexer on the … |
| [`F-ENG-003`](../findings/F-ENG-003.md) | 75% | `RuleId::R4_2DelegatecallIntegrity` restates a storage-effect rule as a target allow-list … |
| [`F-CORE-064`](../findings/F-CORE-064.md) | 72% | `expires_at` is silently void once a nonce is allocated, contradicting the queue's documented contract |
| [`F-SEN-003`](../findings/F-SEN-003.md) | 72% | A warp replay delivers no `NewBlock`, so reveals in the replayed range are discarded, `finalize` takes the … |
| [`F-VAL-063`](../findings/F-VAL-063.md) | 72% | Consensus-critical configuration is unvalidated, has no onchain anchor, and its defaults are the unsafe ones |
| [`F-CORE-003`](../findings/F-CORE-003.md) | 70% | A lagging RPC backend that answers `null` for a block it has not imported is treated as a reorg, producing a … |
| [`F-CORE-012`](../findings/F-CORE-012.md) | 70% | `use_client_filtering`'s bloom-equality completeness check is blind to the loss of any log whose (address, topics) … |
| [`F-CORE-033`](../findings/F-CORE-033.md) | 70% | Effect concurrency is unbounded: one backfill page can spawn a task per matching log at once, with no cap, no … |
| [`F-VAL-065`](../findings/F-VAL-065.md) | 70% | Two actions are queued with no expiry and none is deduplicated … |
| [`F-VAL-064`](../findings/F-VAL-064.md) | 68% | The shipped deployment cannot detect a halted validator … |
| [`F-SEN-004`](../findings/F-SEN-004.md) | 62% | The sentinel bonds on every proposal with no cap on concurrent engine checks, outstanding bonds or reveal … |
| [`F-CORE-011`](../findings/F-CORE-011.md) | 60% | The shared provider is built with no timeout, retry or rate-limit layer … |
| [`F-CORE-062`](../findings/F-CORE-062.md) | 60% | An allocated nonce is never released and allocation is floored at `MAX(nonce)+1` … |
| [`F-CORE-061`](../findings/F-CORE-061.md) | 58% | `is_transaction_underpriced` only matches replacement rejections … |
| [`F-CORE-063`](../findings/F-CORE-063.md) | 55% | Execution is inferred from the account nonce alone and invalidated only by a block-number regression … |
| [`F-VAL-060`](../findings/F-VAL-060.md) | 50% | Coordinator and Consensus events are dispatched without checking the emitting contract address |
| [`F-VAL-067`](../findings/F-VAL-067.md) | 48% | The Rust DKG-abort test counts complaints cumulatively while the contract's equivalent counter is decremented by … |
| [`F-XC-050`](../findings/F-XC-050.md) | 48% | No DKG event handler checks group membership, so one injected `KeyGenConfirmed` closes the confirmation round … |

### Low (41)

| ID | Cert. | Claim |
| --- | --: | --- |
| [`F-XC-011`](../findings/F-XC-011.md) | 95% | Four RUSTSEC advisories and eleven warnings are live in `Cargo.lock` … |
| [`F-VAL-062`](../findings/F-VAL-062.md) | 88% | Secret-bearing effects and resumes derive `Debug` and are printed at `warn`, unlike every other secret type in the … |
| [`F-XC-002`](../findings/F-XC-002.md) | 88% | Secret-bearing types reach log statements through derived `Debug` … |
| [`F-CORE-036`](../findings/F-CORE-036.md) | 85% | The runtime requires `Debug` on every service `Effect` and `Resume` and prints them at `trace` in five places … |
| [`F-SEN-006`](../findings/F-SEN-006.md) | 85% | Emitted actions are not idempotent under replay, so every restart and reorg enqueues duplicate … |
| [`F-SEN-012`](../findings/F-SEN-012.md) | 85% | The engine client makes exactly one attempt per proposal … |
| [`F-XC-004`](../findings/F-XC-004.md) | 85% | All three runtime images run as root, pin no base-image digest … |
| [`F-XC-052`](../findings/F-XC-052.md) | 85% | `decode_multi_send` synthesises sub-transactions with `chain_id`, `nonce` and every refund field zeroed … |
| [`F-SEN-009`](../findings/F-SEN-009.md) | 82% | The engine timeout is derived from an unvalidated config value instead of the oracle's real commit window … |
| [`F-SEN-011`](../findings/F-SEN-011.md) | 82% | A restart orphans any in-flight engine check whose proposal is older than the rollback anchor … |
| [`F-ENG-005`](../findings/F-ENG-005.md) | 80% | The engine has no deadline anywhere: `x-request-timeout` is parsed and discarded, there is no server timeout or … |
| [`F-ENG-006`](../findings/F-ENG-006.md) | 80% | `decode_target_effects` recurses through MultiSend with no depth limit … |
| [`F-ENG-007`](../findings/F-ENG-007.md) | 80% | Shutdown drops the serve future instead of draining it, so every in-flight security check is aborted mid-request … |
| [`F-SEN-007`](../findings/F-SEN-007.md) | 80% | No balance, allowance, registration or chain pre-check: a sentinel that cannot possibly commit still pays for an … |
| [`F-XC-008`](../findings/F-XC-008.md) | 80% | Both outbound HTTP clients are built with library defaults: proxy environment honoured, redirects followed … |
| [`F-XC-010`](../findings/F-XC-010.md) | 80% | The sentinel engine exports no metrics of its own: it serves a Prometheus endpoint that says nothing about … |
| [`F-CORE-009`](../findings/F-CORE-009.md) | 78% | The block-watcher configuration accepts values with no range validation … |
| [`F-ENG-038`](../findings/F-ENG-038.md) | 78% | CoW shape recognisers accept batches their paired decoders reject, turning a dangling relayer approval from … |
| [`F-XC-006`](../findings/F-XC-006.md) | 78% | Nothing binds a deployment to a chain: no config field, no persisted column, and the legacy configuration had one |
| [`F-CORE-005`](../findings/F-CORE-005.md) | 75% | `max_reorg_depth = 0` documents "fail loudly on any reorg" but silently disables the uncled-block recovery path … |
| [`F-ENG-009`](../findings/F-ENG-009.md) | 75% | `EngineConfig` performs no validation: the lookback and max-range pair silently sets the per-request `eth_getLogs` … |
| [`F-ENG-043`](../findings/F-ENG-043.md) | 74% | The CoW order lookup has no client timeout and puts an unbounded, unvalidated attacker-controlled `orderUid` into … |
| [`F-XC-009`](../findings/F-XC-009.md) | 72% | Sample configs demonstrate dangerous values: a well-known private key as the signer placeholder, `0.0.0.0` binds … |
| [`F-CORE-008`](../findings/F-CORE-008.md) | 70% | Block polling is scheduled by comparing chain timestamps against the host wall clock … |
| [`F-CORE-040`](../findings/F-CORE-040.md) | 65% | The driver's inner `select!` restarts the watcher's in-flight RPC request on every effect resume … |
| [`F-CORE-037`](../findings/F-CORE-037.md) | 62% | Snapshots are an unversioned JSON dump of the service state with no migration path and no recovery from a decode … |
| [`F-CORE-007`](../findings/F-CORE-007.md) | 60% | A node that keeps disagreeing with itself during startup puts `BlockWatcher::initialize` in an unbounded … |
| [`F-XC-003`](../findings/F-XC-003.md) | 58% | `deny_unknown_fields` is combined with `#[serde(flatten)]` in the validator and sentinel configs … |
| [`F-CORE-006`](../findings/F-CORE-006.md) | 55% | The event watcher matches on the cross product of watched addresses and watched topics … |
| [`F-CORE-039`](../findings/F-CORE-039.md) | 55% | Graceful shutdown is bounded only by the RPC's own patience … |
| [`F-CORE-065`](../findings/F-CORE-065.md) | 55% | No chain-id or deployment binding on the `transactions` table … |
| [`F-VAL-034`](../findings/F-VAL-034.md) | 55% | `handle_nonces` applies a nonce resume to whatever session holds the message, without checking the signature id |
| [`F-VAL-038`](../findings/F-VAL-038.md) | 55% | Nonce chunk generation saturates every core and then holds the shared SQLite writer for 1025 statements, competing … |
| [`F-SEN-008`](../findings/F-SEN-008.md) | 52% | Hard-coded gas limits and an unconditional non-zero `approve` assume a plain ERC-20 … |
| [`F-VAL-040`](../findings/F-VAL-040.md) | 50% | `last_signer` is overwritten by every accepted nonce reveal and the contract does not deduplicate reveals … |
| [`F-CORE-010`](../findings/F-CORE-010.md) | 45% | The `-32001` recovery commits the block watcher's rewind before the event watcher accepts it … |
| [`F-CORE-032`](../findings/F-CORE-032.md) | 45% | A failed effect task is logged and skipped, so a panicking effect silently removes a resume the state machine is … |
| [`F-VAL-031`](../findings/F-VAL-031.md) | 42% | A dead nonce-generation worker thread is never detected, logged, or restarted |
| [`F-XC-051`](../findings/F-XC-051.md) | 42% | `verify_commitment` deliberately delegates the DKG commitment's only structural validation to a contract that is … |
| [`F-VAL-036`](../findings/F-VAL-036.md) | 40% | `NonceState::observe` accepts a non-monotonic sequence and rewinds `next_sequence`, inflating the measured nonce … |
| [`F-VAL-035`](../findings/F-VAL-035.md) | 35% | Secret nonce material is copied into unzeroised JSON strings, abandoned chunks are never pruned … |

### Informational (11)

| ID | Cert. | Claim |
| --- | --: | --- |
| [`F-SEN-013`](../findings/F-SEN-013.md) | 98% | A single undecodable `Revealed.reason` from any active sentinel would stall every other sentinel's indexer … |
| [`F-SEN-014`](../findings/F-SEN-014.md) | 88% | Every participating sentinel submits `finalize` for every request, so all but one revert |
| [`F-CORE-038`](../findings/F-CORE-038.md) | 85% | `kdf::derive_key`'s multi-part `info` is a plain concatenation, but the doc comment implies otherwise … |
| [`F-ENG-004`](../findings/F-ENG-004.md) | 85% | Two Charter citations in `RuleId` are wrong: R-4.3 attributes a verbatim quote to § 2.4 Notes, which does not … |
| [`F-ENG-040`](../findings/F-ENG-040.md) | 85% | Every MultiSend denial is reported as R-4.2, even when the failing sub-call is a settings-change violation |
| [`F-ENG-041`](../findings/F-ENG-041.md) | 85% | A first-time recipient with no established history only ever abstains, so a novel-address drain is never denied |
| [`F-SEN-010`](../findings/F-SEN-010.md) | 85% | The sample config ships zero addresses that parse and start cleanly … |
| [`F-XC-007`](../findings/F-XC-007.md) | 84% | Dependency surface is wider than the code needs and no advisory gate exists in CI |
| [`F-ENG-008`](../findings/F-ENG-008.md) | 80% | `openapi.yaml`, the declared authoritative interface contract, documents only `200` while the engine provably … |
| [`F-XC-001`](../findings/F-XC-001.md) | 66% | No release profile: overflow checks and debug assertions are off in every shipped binary |
| [`F-VAL-037`](../findings/F-VAL-037.md) | 60% | Merkle trees pad with `B256::ZERO` and have no leaf/internal domain separation … |

---

## 6. What the audit got wrong, and what testing did not establish

Phase 5 ran under an explicit rule: a PoC that fails to compile, or passes when the finding says it should fail, is evidence **against** the finding. Five claims moved down; **no finding was refuted by any passing integration suite**.

### Refuted or reduced by execution

| Claim | Outcome |
| --- | --- |
| [`F-SEN-013`](../findings/F-SEN-013.md) basis 8 — one undecodable `Revealed.reason` stalls every indexer | **False.** `alloy-sol-types` 1.6.0 decodes invalid UTF-8 lossily (`detokenize` is `from_utf8_lossy`); byte `0x80` yields `reason: "\u{fffd}"`. Conditional severity resolves to **Informational**; status **Refuted-as-filed**, 98 % confidence in the refutation. Does **not** close [`F-CORE-004`](../findings/F-CORE-004.md) — only this trigger for it is gone |
| The secret-leak cluster — `frost-core` 3.0.0's derived `Debug` prints secret scalars | **It redacts:** `signing_share: SigningShare("<redacted>")`. QA's two apparent hits were a false positive in the PoC's own needle (`KeyShare::dummy()` gives the identifier the same bytes as the scalar). Leak leg refuted, hygiene leg confirmed: [`F-VAL-062`](../findings/F-VAL-062.md) 60 → 88 % (Medium → Informational/Low), [`F-XC-002`](../findings/F-XC-002.md) 74 → 88 % (Medium → Low), [`F-CORE-036`](../findings/F-CORE-036.md) 50 → 85 % |
| [`F-VAL-035`](../findings/F-VAL-035.md) leg (c) — the cascade depends on an unasserted SQLite pragma | **Refuted.** `sqlx-sqlite` sets `foreign_keys = 1` itself and the cascade was observed firing. 45 → **35 %**, below the 40 % bar; retained so the refutation is visible. Legs (a) and (b) unaffected |
| [`F-XC-007`](../findings/F-XC-007.md) item 2 — unused SQL drivers widen the attack surface | **Refuted.** `sqlx-mysql`/`sqlx-postgres` contribute **0 symbols** to all three release binaries. Worth doing as lockfile hygiene, not attack-surface reduction. The process claim (no CI advisory gate) stands: 84 → 92 % |
| The ABI memory-exhaustion worry | **Dead.** alloy's `vec_try_with_capacity` is fallible: across 2^20…2^64 every case errored with dVSZ = 0 kB and dRSS ≤ 192 kB. No finding filed; the `I`-class legs in `F-XC-051`, `F-VAL-001` and `F-VAL-003` close reassuringly |

[`F-VAL-033`](../findings/F-VAL-033.md) is the sixth downward move and the largest: **Critical → High**, 85 → 72 %, because the live restore self-halts the validator before any nonce can be reused (§1, §5).

Two Manager readings were also overturned by agents told to verify rather than trust: `ruint` 1.18.0's advisory covers **8 shift methods only**, with **zero calls** to any of them in `crates/` (the one `U256` shift is inside `#[cfg(test)]`); and `h2` reaches the engine's axum API (which answered a raw HTTP/2 preface with a 55-byte SETTINGS frame despite axum's `http2` feature being off) but **not** the metrics endpoint, which replied with 0 bytes.

### Hallucinated (`H`) claims: 3

Across roughly 1,000 citations in 107 files, each caught by a Critic or QA agent re-opening the source. None collapsed its finding.

| Where | Claim | Why it is `H` |
| --- | --- | --- |
| [`F-ENG-038`](../findings/F-ENG-038.md), closing paragraph | A combination sub-claim about MultiSend handling | Contradicted by `multi_send.rs:143` |
| [`F-VAL-064`](../findings/F-VAL-064.md) | "there is no `.dockerignore` anywhere in the repository" | **Four exist**, as per-Dockerfile `Dockerfile.dockerignore` files. The concern survives in narrower form: the allow-list re-admits `/crates/**` wholesale |
| [`F-XC-052`](../findings/F-XC-052.md), basis row 7 | Cited as `E2` | It cites another audit finding, so it is `I` |

### What live testing did _not_ establish

1. **[`F-CORE-060`](../findings/F-CORE-060.md) does not self-start on a healthy node.** The ratchet needs a **stale fee floor** — a restored database, or a foreign transaction at the nonce. The balance-brake sub-claim is not testable locally.
2. **[`F-VAL-030`](../findings/F-VAL-030.md) and [`F-VAL-032`](../findings/F-VAL-032.md): mechanism live-verified, consequence not locally testable.** The 1024-sequence sign refusal and a `Sign` at sequence ≥ 1024 need ~1024 signs of griefing. Not "reproduced", and not "unproven".
3. **[`F-CORE-067`](../findings/F-CORE-067.md) was reproduced by a restart, not a reorg.** `anvil_reorg` yields empty blocks and no log replay. Duplicate `approve`+`commit` at **nonces 2 and 3 — two onchain transactions, not one replacement** — across 3 runs. The restart path is proven; the reorg path is argued.
4. **[`F-XC-005`](../findings/F-XC-005.md) masking [`F-ENG-033`](../findings/F-ENG-033.md) is not a safe state.** Back to back on one chain behind a proxy mimicking a 10,000-block cap: shipped sample values → the engine **abstains** (_"range 15014 exceeds limit of 10000"_), `F-XC-005` reproduces; set `address_poisoning_max_block_range = 10000`, the remedy the sample file itself documents, and `F-ENG-033` returns `secure`. Neither severity moves. The masking switches off the engine's **only lookalike denial** while `F-ENG-030`, `F-ENG-031` and `F-ENG-044` still affirm drains **without touching the RPC at all**. `F-XC-005` rose 76 → 92 %.

### One cross-cutting caveat, with its limits

Phase 5 measured the shipped SQLite settings: **`busy_timeout = 5000`**, WAL **not** enabled (`journal_mode = delete`). That raises the bar for findings whose trigger is a transient SQLite _error_ — it refutes none of them. Two limits: it does **not** protect [`F-VAL-066`](../findings/F-VAL-066.md), which is an _ordering_ hazard (a blocked writer waits and then commits, which is the shape the finding needs); and for [`F-VAL-004`](../findings/F-VAL-004.md) the bar is lower still, because Phase 7 observed an effect failure **unforced**. Effect failures are not hypothetical in this system.

### What the audit established negatively

- **The workspace builds, tests and lints clean.** 266 tests pass, 0 failed, 0 ignored; `clippy -D warnings` clean; the per-crate test census (core 97, validator 35, sentinel 37, engine 97) is verified, not asserted.
- **No attacker-reachable panic exists.** R10's census and C-XC's independent Rust-aware re-derivation across all 83 files matched site for site: 0 `panic!`/`unreachable!`/`todo!`/`unimplemented!`/`unsafe`; 6 `unwrap`, 15 `expect`, 1 `assert!`, 2 `debug_assert`, 25 casts. All 21 sites re-checked in context; `cow.rs:130` is the one latent site.
- **`validator/src/consensus/hashing.rs` matches the Solidity exactly** — domain separator, type hashes, encoding and `0x1901` framing, verified twice; and all 18 event topic0s and 18 selectors in `bindings.rs` verified against the Solidity with a self-tested Keccak.
- **M5, M8, M1, M4, M7, M10 and CORE-H10 are refuted with citations**, as are a mid-run coordinator address change (`Consensus._COORDINATOR` is `immutable`) and duplicate live nonces in the transaction queue.
- **Secrets at rest are better protected than first written:** `SigningKey`/`SecretKey` are `ZeroizeOnDrop` and the `to_bytes` copies are zeroized (`signer.rs:60-63`, `:88-91`); the residual is that those wipes are not unwind-safe.
- **Zero unread files**, all 67 seeded leads dispositioned, 0 findings refuted outright by a Critic.

---

## 7. Scope, method and safety

**Assumptions** — all 15 signed off at Gate 0. Only the FALSE or changed ones need explanation.

| ID | State | Note |
| --- | --- | --- |
| **A8** | **FALSE** | The `sentinel-test-vectors` corpus is unavailable, so `just test-integration-sentinel-engine` could not run **even with a full toolchain and a working Anvil**. Engine checkers are validated only by in-crate PoCs, never against their intended oracle. **The audit's only hard blocker** |
| **A9** | **FALSE for phases 0–4; partly TRUE in Phase 5; satisfied but for one version gap by Phase 7** | Phase 0 measured cargo/rustc/rustup/forge/anvil/cast/just all absent and 3.8 GiB RAM — the reason the review was read-only. Later: cargo/rustc 1.98.1, `just` 1.40.0, 11 GB RAM, then Foundry. **Version gap: A9 specifies Foundry 1.5.1; the suites ran on 1.8.1** — a possible confounder for any anvil-dependent result, and the direct cause of `run_sentinel_integration_test.sh` being unrunnable |
| **A15** | **TRUE (mid-run)** | The Charter text was supplied as the **one operator-authorised exception** to the no-network rule: a read-only `git clone --depth 1` of the public `github.com/safe-research/safenet-charter` at `44a1e53` into the session scratchpad. Nothing written into the repository, no RPC contacted. Citations take the form `safenet-charter@44a1e53:…`. With A15 TRUE, `F-ENG-001`–`004`, `F-ENG-039` and `F-ENG-044` could reach Confirmed |
| A1–A7, A10–A14 | TRUE | Trusted operator and plaintext secrets at rest by design (A1); adversarial chain data within the <1/3 bound and attacker-controlled Safe payloads (A2); the engine API reachable only by its co-deployed sentinel (A3 — load-bearing for `F-XC-011`'s severity); a **malicious** RPC out of scope, stale/rate-limited/incomplete `eth_getLogs` in scope (A4); reorgs to `max_reorg_depth` handled (A5); crypto libraries trusted (A6); `contracts/src` audited and authoritative (A7); Gnosis ~5 s blocks (A10); scope exactly PROMPT.md §4 (A11); known `TODO`s reported tagged (A12); **no branches, commits, PRs or drift** (A13, A14) |

**Safety boundary.** Every scenario in phases 7 and 8 ran on **local Anvil only: chain 31337, `127.0.0.1`**, with the endpoint printed in each log. No testnet or mainnet endpoint was contacted. This matters because the repository points at live chains: **all three `*.sample.toml` configs ship `rpc = "https://rpc.gnosischain.com"`**, and **`scripts/run_sentinel_engine_integration_test.sh:11` defaults to public Ethereum mainnet**. Every config used was copied and rewritten to loopback; that script was never run. This is also the operational core of [`F-XC-009`](../findings/F-XC-009.md) and [`F-XC-005`](../findings/F-XC-005.md) — the shipped defaults are the unsafe ones. Anyone reproducing this work should do the same.

**Roster.** Recon → 10 reviewers (R1–R10) → 9 Critics (C-CORE-A/B, C-VAL-A/B, C-SEN, C-ENG-A/B, C-XC, Coverage) → 4 QA (QA-VAL, QA-ENG, QA-CORE-SEN, QA-XC) → Documentation → 4 verification agents (V-ENG, V-CORE-SEN, V-VAL, V-XC) → V-INT → RW-VAL, with a Manager gate between phases. Nothing outside `rust-audit/` was written or left modified; Phase 5's PoCs were appended into tracked source files and reverted per file, with the tree verified clean at Gate 5 (three of four crates are binary-only, so no PoC can live in `tests/` until a `lib.rs` exists — and every PoC README's `cargo test --lib` must be `--bins`).

**Detail deliberately not reproduced here:** the environment, inventory, test census and dependency baseline are in [`../state/baseline.md`](../state/baseline.md); the per-file coverage matrix, the weakest-evidence files and the honest limitations (self-reported reads, two consciously uncovered seams — SEN-H9 under A3 and SEN-H15 under A1 — and the two findings that under-class their own counter-evidence) are in [`../state/coverage.md`](../state/coverage.md) and §8.2 of the long-form report; the 22 toolchain-blocked questions and what each one settled are in [`../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md`](../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md).

---

## 8. Still open

- **A8 — the `sentinel-test-vectors` corpus is the only hard blocker.** Until QA may clone it and run `just test-integration-sentinel-engine <path>`, no engine checker finding is validated against its _intended_ oracle.
- **The reorg-nonce harness asserts on the wrong group.** `scripts/run_validator_reorg_nonce_test.sh` uncles block 9 and asserts on genesis instead of the epoch-1 group its own reorg destroys; its header comment and SUCCESS message claim a restart it never performs. Fix both.
- **The sentinel harness needs three named Foundry 1.8.1 fixes**, all proven green on a scratchpad copy: (1) `cast wallet new --json` now emits an envelope, so `jq -r '.[0].address'` becomes `.data[N]` — `cast block` and `cast receipt --json` need the same; (2) bare contract names must become `<file>.s.sol:<Contract>`; (3) `--root <dir>` no longer resolves a _relative_ script path, so the `.sol` path must be absolute. `scripts/` is reference-only under PROMPT.md §4, so this stays an observation — but until it is applied the sentinel suite's verdict is **unknown**.
- **No suite restarts a service, and `anvil_reorg` drops reorged transactions permanently**, so restart- and replay-triggered findings are untested by construction. A reorg fixture that re-includes transactions would close `F-CORE-067`'s reorg trigger.
- **Question 20** (the eight hard-coded MultiSend deployment addresses, their wire-format tags and `allows_delegate_calls` flags) and **question 21** (CoW and Safe contract semantics behind `F-ENG-031`, `F-ENG-037`, `F-ENG-038`) need external material, not tools. So does R9's standing warning: `base.rs`'s allow-listed addresses were **never verified against live deployments**, and a wrong or squatted address in any list is a silent R-4.1/R-4.2 bypass.
- **Two operational traps to fix in the runbook:** the engine binds its Prometheus/health listener **before** `Provider::connect`, so a health probe can see a live process that will never serve the API; and a provider that merely range-caps passes startup and then fails **every request forever**. Both are invisible in production because of `F-XC-010`.
- **Pin the Foundry version**, and **make the PoCs permanent** — 31 directories, ~90 tests, 24 executed and reproduced — by adding a thin `src/lib.rs` to the three binary-only crates.

---

_Compressed from the 108 finding files, `state/coverage.md`, `state/baseline.md` and `state/STATE.md` without changing any verdict, certainty, severity or claim._
