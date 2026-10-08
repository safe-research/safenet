# In-flight impact assessment — unmerged PR stacks

This document covers two assessment rounds of **unmerged** work: first the Batched Execution stack (PRs #899–#904), then the open PRs #906–#917 (Scheduled Secret Pruning, sentinel deadlines, optimistic block transition, SEF veto epic). Nothing here is a finding against `main`.

**Status after run 2** (audit branch at `fe9e84c`, `origin/main` `8b6a75d` merged): the pruning stack (#906–#913), #914 and #917 are **merged** and no longer in flight — #914 is now a finding on `main`, [`F2-SEN-010`](../findings/F2-SEN-010.md). #915 and the batex stack remain unmerged. See the section [Status at `fe9e84c`](#status-at-fe9e84c-after-run-2) at the end; the rounds below are kept as written.

## Round 1 — Batched Execution stack (#899–#904)

**Assessor:** FWD. **Audit HEAD:** `a7f3915` (audit ran at `2893917`, re-validated against `origin/main` = `49d7e39`, which has not moved).

This document assesses **unmerged** code. Nothing here is a finding against `main`. No branch was merged, checked out, rebased or modified; everything below was read with `git show` / `git diff` against `origin/...` refs.

## The stack

| PR | Branch → base | Title | Assessed at |
| --- | --- | --- | --- |
| #899 | `feat/batex_0` → `main` | [Phase 0] Add Epic for Batched Execution | — |
| #900 | `fix/batex_1` → `feat/batex_0` | [Phase 1] Adjust 7702 batching contract | contract |
| #902 | `feat/batex_2` → `fix/batex_1` | [Phase 2] Adjust config | config |
| #903 | `feat/batex_3` → `feat/batex_2` | [Phase 3] Add authorisation signing support | signer/types |
| #904 | `feat/batex_4` → `feat/batex_3` | [Phase 4] Adjust nonce handling | storage/mod |

The epic (`epics/2026_09_09_safenet_7702_executor_tx_batching.md`, on `feat/batex_0`) plans **ten** phases. **The stack stops at Phase 4.** Phases 5 (enqueue the delegation on start), 6 (the batch builder `tx/executor.rs`), 7 (wire batching into `queue()`), 8–10 are **not on any pushed branch**.

The single most important consequence of that for this assessment: **on `feat/batex_4` nothing ever sets `Transaction::authorization`, so Phases 3 and 4 are behaviourally inert.** Every new code path in the diff — 7702 signing, the two-nonce reservation, `pending_delegation`, the authorization self-check — is dead code today. The risks below are therefore _forward-looking_: they are what the stack will do once Phase 5 arms it, and they are unreachable as the branches stand.

## Summary table

Effect key: **fix** / **worsen** / **unchanged** / **reshape** (impact changes character without a clear net improvement).

| Finding | Sev / Cert | Effect of the batex stack | Note |
| --- | --- | --- | --- |
| `F-CORE-060` (fee ratchet unbounded) | High / 98% | **unchanged**, with a new trigger | `fees.rs` is not in the diff; `fees::bump` is byte-identical. `submit_transaction`'s rearrangement (`bumped_fees` before `build`) is a pure refactor producing the same `Submission`. **But** `F-CORE-069`'s permanent nonce gap strands transactions in flight where the ratchet runs on them without bound, and manually repairing the gap releases the whole backlog at the ratcheted fee. |
| `F-CORE-061` (first-submission rejection retries forever) | Medium / 58% | **unchanged** | `is_transaction_underpriced` and its regexes are untouched; the generic branch still records no fee floor. Batching would reduce the _number_ of transactions exposed to it but raise the cost of each one wedging (a batch, not an action). |
| `F-CORE-062` (nonce never released, queue wedges) | High / 60% | **worsen** | Phase 4 keeps `MAX(chain, high_water + span)` — it still never allocates into a gap — and now _creates_ gaps deliberately. The epic's own acknowledged failure path (authorization does not apply) is a new, first-party way to reach exactly this defect. Filed as `F-CORE-069`. |
| `F-CORE-063` (execution inferred from account nonce) | Medium / 55% | **worsen** (blast radius) | `mark_executed` is untouched, still a pure nonce comparison. Under batching the inference covers a transaction carrying 6–8 actions, and Phase 1's `InsufficientGas` guard adds a **whole-batch revert** that is indistinguishable from success to the queue. Filed as `F-CORE-068`. The delegation transaction itself is also declared executed purely from a nonce advance. |
| `F-CORE-064` (`expires_at` void once a nonce is allocated) | Medium / 72% | **reshape** | The delegation is specified with `expires_at: None` _by design_, so the epic institutionalises the never-expiring row this finding is about — and makes it load-bearing ("never dropped or pruned while unexecuted"). Batches split on any `expires_at` change, so no batch inherits a wrong deadline, but a late-landing batch now lands _all_ of its stale actions at one nonce. |
| `F-CORE-065` (no chain-id / signer binding) | Low / 55% | **worsen** | The EIP-7702 authorization's `chain_id` comes from the same connect-time `Provider::chain_id()` cache (`signer.rs`, `types.rs::build`). The cache is now load-bearing for a _code delegation_, not just one transaction: a stale cached id produces an authorization the chain rejects, which is precisely `F-CORE-069`'s gap. The stored `request` JSON gains an `authorization` field naming an executor **address with no chain binding**, so a database moved between chains now carries a delegation target too. (No cross-chain _replay_ risk: the epic correctly uses the real chain id rather than `0`.) |
| `F-CORE-066` (`tx::Config` unvalidated) | Medium / 78% | **unchanged, surface extended** | Two new keys, neither validated: `max_batch_gas: u64` accepts `0` (and anything below the ~26 000 base, which silently disables batching via the "cannot fit in an empty batch" pass-through), and `executor: Option<Address>` accepts the **zero address** — which the sample TOML ships as its literal example value. Phase 5's planned `eth_getCode(executor)` check would catch that, but Phase 5 is not in this stack, so today the field is accepted with no check whatsoever. `deny_unknown_fields` and `#[serde(default)]` are preserved. |
| `F-CORE-067` (no idempotency key on `enqueue`) | Critic / 98% | **unchanged in kind, worsen in blast radius** | `enqueue` is still the identical unconditional `INSERT INTO transactions (request, expires_at)` with no idempotency key, no content hash, no `ON CONFLICT`, no unique constraint; the table DDL is unchanged apart from a new **`transactions_nonce_idx` index on `nonce` only** — not a uniqueness constraint and not an idempotency key. `SnapshotStore::reorg` still never touches `transactions`. The +186 lines in `storage.rs` are the nonce span, two delegation helpers, an index and ~120 lines of tests. Once Phase 7 lands, a replay produces **two duplicate batches** — the same defect multiplied by the batch size. Phase 5's `queue_delegation` idempotency also rests on `has_delegation`, a read-then-write with no constraint behind it, so the delegation is itself duplicable. |
| `F-VAL-063` (unvalidated consensus config, unsafe defaults) | Medium / 72% | **unchanged** | `validator.sample.toml` on `feat/batex_4` still ships `genesis_salt = "0x00…00"` uncommented and `oracles` commented out; `crates/validator/src/config.rs` gains only two assertions in an existing test. |
| `F-XC-009` (samples teach dangerous values) | Low / 72% | **unchanged, one new instance** | Both samples on `feat/batex_4` still carry `rpc = "https://rpc.gnosischain.com"` (live mainnet) and `signer = "0x00…01"` (the well-known key), and `metrics_address = "0.0.0.0:3555"`. The new block adds a **third** dangerous placeholder: `# executor = "0x0000000000000000000000000000000000000000"`. It is commented out, but under EIP-7702 the zero address is the _delegation-clearing_ target and a codeless address re-opens the epic's own "every batched action silently no-ops" mode. |
| `F-VAL-065` (validator actions not deduplicated) | Low / 70% | **unchanged, worsen downstream** | `action.rs`'s +12 lines are twelve `authorization: None` initialisers — a mechanical struct-field addition, no encoder logic changes, no expiry changes. `SetValidatorStaker` and `Preprocess` still carry `None` expiry. The finding's compounding note ("a duplicate that reverts … is recorded as executed and never retried") is exactly what `F-CORE-068` amplifies to batch scale. A duplicate `Sign` batched with other actions still burns a nonce sequence for the whole group. |
| `F-SEN-006` (sentinel actions not idempotent under replay) | Low / 85% | **unchanged, worsen downstream** | `service.rs`'s +5 lines are five `authorization: None` initialisers in `SentinelEncoder`. All five duplicate-producing paths are untouched. Batching makes each replay produce a duplicate _batch_; and because `execute` swallows the `AlreadyCommitted` / `AlreadyRevealed` reverts as `CallFailed`, the duplicates' reverts stop being visible even in principle. |
| `F-SEN-007` (no balance/allowance/registration pre-check) | Medium / 80% | **worsen** | Still no `eth_call` or estimation before broadcast. With batching, the per-request burn becomes a batch that _succeeds_ while every call in it reverts into a swallowed `CallFailed` on the sentinel's own EOA — an address nothing indexes. The finding's stated symptom ("no error visible beyond the absence of `Committed` events") gets strictly harder to diagnose. Note also that `execute` does **not** stop on a failed `ApproveToken`: the following `Commit` runs anyway and reverts for want of allowance. |

**Net: nothing in the batex stack fixes any existing finding.**

## New defects filed

Both are marked **UNMERGED** in their title and Location field, naming the branch and PR.

- **`F-CORE-068`** (High / Medium, 85%) — _A batch's execution status is unobservable, so a whole-batch revert or a mid-batch `CallFailed` silently drops up to a full batch of actions._ Three outcomes (success, swallowed `CallFailed`, whole-batch `InsufficientGas` revert) all advance the nonce and are therefore identical to `mark_executed`. `prune` then destroys the record within `max_reorg_depth` blocks. Phase 1's new gas guard is genuinely louder onchain and **no louder offchain**, while enlarging the unit of loss from one action to a whole batch. Applies to `origin/feat/batex_4` (PR #904) plus the planned Phases 6–7.
- **`F-CORE-069`** (High / Medium, 80%) — _The two-nonce delegation reservation writes a permanent nonce gap into durable state on the epic's own acknowledged failure path, and the only alarm fires exactly once._ The `error!` in `update_block_status` is immediately self-clearing, because the very next statement is `mark_executed`, which sets `executed_at` on the delegation and makes `pending_delegation()` return `None` from the next block on. One log line is the entire signal for a permanently wedged queue. Includes a secondary defect: `pending_delegation`'s query has no `ORDER BY` and no `WHERE nonce IS NOT NULL` despite promising the in-flight nonce, so with two unexecuted delegation rows it can return the `NULL`-nonce one and silently disable the self-check; all three of its tests create exactly one delegation row. Applies to `origin/feat/batex_4` (PR #904).

### Checked and found sound (no finding)

Recorded so the negative results are not re-derived:

- **Authorization replay.** `Authorization { chain_id: U256::from(tx.chain_id), address: delegate, nonce: tx.nonce + 1 }` uses the real chain id rather than the wildcard `0`, so it cannot be replayed on another chain. Within the chain the signed authorization is a standalone object that a mempool observer can lift into their own type-4 transaction, but it only applies when the authority's nonce is exactly `N + 1`, which is reachable only _after_ the service's own transaction at `N` has already applied it — and the delegate is the same either way. Not exploitable.
- **The batch gas formula against the new guard.** Worst case: every earlier call consumes its full reservation. The formula budgets `gas_j + gas_j/63 + 5_000` per call while the guard needs `gas_i * 64/63 = gas_i + gas_i/63`, so the remaining budget at index `i` still satisfies the guard with the `5_000` terms to spare. The formula is sound _provided_ the `5_000` and `26_000` constants cover real overhead — which the epic itself flags as unmeasured. An **under-estimated per-call `gas`** does **not** revert the batch; the callee OOGs inside its own reservation and is swallowed as a `CallFailed`, i.e. case 2 of `F-CORE-068`.
- **Nonce high-water lookahead vs. `prune`.** The Phase 4 query reads a single row (`ORDER BY nonce DESC LIMIT 1`) and relies on the max-nonce row being the most recently allocated. Pruning cannot break this: any row with a nonce above the delegation's is allocated later, so deleting an executed lower row cannot lower the high-water mark below a live reservation.
- **EOA-ness assumptions in the contracts.** `grep` over `contracts/src/` finds no `extcodesize`, `code.length`, `isContract` or `tx.origin == msg.sender` check, so delegating the validator's and sentinel's signer EOAs does not trip any "must be an EOA" gate in Safenet's own contracts.
- **`Safenet7702Executor`'s access control.** `require(msg.sender == address(this), OnlySelf())` is the only authorisation, which is correct for a 7702 self-call target; there is no ERC-1271, no 4337 entry point, no sponsored path, and the `receive()` is plain payable.
- **`submit_transaction`'s rearrangement.** Computing `bumped_fees` before `build` and recording the `Submission` from those fees produces values identical to reading them back off the built `TxEip1559`. Behaviour-preserving, as the epic claims.

## What to re-run when this merges

The audit's 21 end-to-end-validated PoCs were run on local Anvil. These are the ones the batex stack puts at risk, plus the new coverage it demands. Anvil must be on **Prague or later** for any 7702 path (`--hardfork prague`); the epic flags this as unconfirmed in its own Phase 8.

**Re-run unchanged, to confirm no regression (Phases 2–4 are inert, so all should still pass):**

1. `F-CORE-067`'s duplicate-action PoC — reorg replay produces two onchain transactions at two nonces. Re-run **twice**: once with `executor` unset (must be byte-identical to the `main` result), once with it set, where the expected new result is **two duplicate batches**.
2. `F-CORE-060`'s underpriced-ratchet PoC — confirm `fees::bump` still ratchets at 1.1× per block and that the `bumped_fees`/`build` split records the same fee floor in `record_submission`.
3. `F-CORE-061`'s first-submission-rejection PoC — confirm the regexes still miss it and the fee is still unchanged on retry.
4. `F-CORE-062`'s nonce-wedge PoC — then extend it: allocate a delegation, let the authorization fail, and confirm the queue wedges with exactly one `error` line (this is `F-CORE-069`).
5. `F-CORE-063`'s mark-executed-without-a-receipt PoC — extend to a batch: submit a batch whose calls all revert, confirm `executed_at` is set and the row is pruned (this is `F-CORE-068`).
6. `F-CORE-064`'s post-expiry-resubmission PoC — and add the delegation, which is `expires_at: None` by design.
7. `F-CORE-066`'s config PoC — extend with `max_batch_gas = 0` and `executor = "0x00…00"`.
8. `F-VAL-065` and `F-SEN-006` replay PoCs — re-run with `executor` set; the expected observable changes from N duplicate transactions to one duplicate batch whose reverts are `CallFailed` events on the service's own EOA rather than failed transactions.

**New coverage required before Phase 7 ships:**

9. Phase 5 landing: assert `queue_delegation` is idempotent across a restart **while the delegation is still queued but unallocated**, and across a restart **after it executed but before `prune`** — the second case enqueues a second delegation by design and must not produce two reservations.
10. `pending_delegation` with **two** unexecuted delegation rows (one allocated, one not) — currently order-undefined; add the test before the query is relied on.
11. Phase 6 landing: measure the `5_000` per-call and `26_000` base constants against real action calldata on a Prague Anvil, per the epic's own open question, and assert the `InsufficientGas` guard does not fire for a full `max_batch_gas` batch of real validator actions.
12. Phase 7 landing: a batch containing one reverting call — assert the remaining calls still take effect and that the service can tell (it currently cannot; that is `F-CORE-068`).

## Round 2 — open PRs #906–#917

**`main` had not moved** (still `49d7e39`), so every finding still stands there. None of the ten PRs claims to close an issue or cites an audit finding; the only cross-reference is #915 → #471 (the June draft _Optimize Block State Transition_, same design). No issue was closed or updated. Each branch was read with `git show`/`git diff` and built from a `git archive` extraction; no branch was merged or checked out, and `rust-audit/` and git state were verified byte-identical before and after.

| Work | PRs | Verdict |
| --- | --- | --- |
| Scheduled Secret Pruning | #906 → #907 → #908 → #909 → #910 → #912 → #913 | **`F-VAL-005` partially fixed**; `F-VAL-066` changed shape; everything else it touches unchanged. Four new defects: `F-VAL-068`. — **Since merged**; run 2's verdict on the merged code is in the status section below. |
| Sentinel deadlines | #914 | **Fixes nothing — worsens `F-SEN-002` if merged without #915** (`F-SEN-016` D1). — **Since merged alone** (`8b6a75d`): the D1 defect is live on `main` as [`F2-SEN-010`](../findings/F2-SEN-010.md), High 93. |
| Optimistic block transition | #915 (on #914) | **Fixes nothing.** `F-CORE-031` changed shape; the ordering behind `F-VAL-005` is unchanged. New defects: `F-SEN-016` D2–D4. — **Still unmerged.** |

### Scheduled Secret Pruning (#906–#913)

| Finding | Effect |
| --- | --- |
| [`F-VAL-005`](../findings/F-VAL-005.md) | **Partially fixed** — stored-block ordering (`store.rs:323-334`) closes the same-height replay; live Anvil showed identical epoch-1 commitments across the reorg. Residual: absence beyond `max_reorg_depth` still loses the ceremony. |
| [`F-VAL-066`](../findings/F-VAL-066.md) | **Changed shape** — unqualified `DELETE` gone; retention set still pre-logs; race survives at depth 0–1. |
| [`F-VAL-033`](../findings/F-VAL-033.md) | Unchanged |
| [`F-VAL-030`](../findings/F-VAL-030.md), [`F-VAL-061`](../findings/F-VAL-061.md), [`F-VAL-032`](../findings/F-VAL-032.md) | Unchanged; `F-VAL-061`'s restart window widened (D1) |
| [`F-VAL-035`](../findings/F-VAL-035.md) | Partially fixed (tests only) |
| [`F-VAL-034`](../findings/F-VAL-034.md), [`F-VAL-036`](../findings/F-VAL-036.md), [`F-VAL-038`](../findings/F-VAL-038.md), [`F-VAL-062`](../findings/F-VAL-062.md), [`F-XC-002`](../findings/F-XC-002.md) | Unchanged |
| [`F-CORE-031`](../findings/F-CORE-031.md), [`F-CORE-032`](../findings/F-CORE-032.md), [`F-CORE-033`](../findings/F-CORE-033.md), [`F-CORE-034`](../findings/F-CORE-034.md) | Unchanged |

Caveats: the reorg-nonce harness performs no validator restart and asserts only on the genesis group, so the epoch-1 evidence was read from the validator logs; no same-machine control run on `main` was made.

### Sentinel deadlines (#914) and optimistic block transition (#915)

Both apply cleanly onto `main`: `main`'s crates are byte-identical to their base `199629e`.

| Finding | Effect |
| --- | --- |
| [`F-SEN-005`](../findings/F-SEN-005.md) | Unchanged — waiting states still never expire; `event.deadline` still never read |
| [`F-SEN-001`](../findings/F-SEN-001.md) | Unchanged — **bond lost again live on the #915 tip** (A: 996000 vs B: 1001000; second commit reverted `AlreadyCommitted()`) |
| [`F-SEN-002`](../findings/F-SEN-002.md) | Unchanged on the tip; **worsened by #914 alone** |
| [`F-SEN-003`](../findings/F-SEN-003.md), [`F-SEN-015`](../findings/F-SEN-015.md), [`F-SEN-011`](../findings/F-SEN-011.md), [`F-SEN-009`](../findings/F-SEN-009.md), [`F-SEN-004`](../findings/F-SEN-004.md), [`F-SEN-006`](../findings/F-SEN-006.md) | Unchanged |
| [`F-CORE-031`](../findings/F-CORE-031.md) | **Changed shape** — block-transition effects now re-emit; log-originated effects still lost |
| [`F-VAL-005`](../findings/F-VAL-005.md) | Unchanged — adapted PoC ordering case reproduces on the tip |
| [`F-CORE-001`](../findings/F-CORE-001.md), [`F-CORE-067`](../findings/F-CORE-067.md), [`F-CORE-037`](../findings/F-CORE-037.md), [`F-CORE-003`](../findings/F-CORE-003.md) | Unchanged; #915 needs no snapshot migration |

### SEF veto epic (#917)

Plans a small Solidity Safe module letting one SEF address invalidate a SafeSnap Reality proposal. It repeats none of the Rust-service defect patterns: `to`, `value`, `operation` and selector are fixed in code, call success is required, and the question hash is computed onchain. Its gap: it never asks whether the SafeDAO Safe is Safenet-protected. If it is, a signed `enableModule` violates Charter R-4.1. (Engine-side analysis removed with the engine's scope.)

### New defects filed (round 2)

| Finding | Branch | Defects |
| --- | --- | --- |
| [`F-VAL-068`](../findings/F-VAL-068.md) | `origin/prune/end` | D1 restart leaves a group with no nonce generator · D2 deletion delayed, not prevented · D3 `F-VAL-066` race at depth 0–1 · D4 stored block has no hash and never decreases |
| [`F-SEN-016`](../findings/F-SEN-016.md) | #914, #915 | D1 #914 alone drops a valid peer commit · D2 warp rollback discards effect results (latent) · D3 early-transition actions re-queued after a reorg · D4 validator keygen deadlines one block earlier (unverified) |

### What to re-run when these merge

1. Pruning stack: `poc/F-VAL-005-066`, `poc/F-VAL-030-032-061`, `poc/F-VAL-033`, plus a reorg-nonce run that restarts a validator and asserts on the **epoch-1** group, with a same-machine control on `main`.
2. #914/#915: `poc/F-SEN-001`, `F-SEN-002`, `F-SEN-015`, `F-CORE-067`, `F-CORE-001`, the `poc/F-VAL-005-066` ordering case, and the `F-SEN-016` D1 probe — **before** #914 merges on its own.

## Status at `fe9e84c` (after run 2)

Written by the reconciliation after `origin/main` `8b6a75d` was merged into the audit branch. Branch positions are from `git branch -r` and `git rev-list --left-right --count origin/main...<branch>`; nothing was fetched or checked out.

| Work | PRs | Status at `fe9e84c` | Effect on findings |
| --- | --- | --- | --- |
| Scheduled Secret Pruning | #906–#913 | **Merged** (`main` commits `8b88f4a` #909, `c87c054` #910, `f0fdc40` #912, `80951a0` #913 and the earlier three). | [`F-VAL-005`](../findings/F-VAL-005.md) **fixed for the reproduced trigger** (reorg within `max_reorg_depth`, no restart; 13/13 store tests at `fe9e84c`); [`F-VAL-066`](../findings/F-VAL-066.md) changed shape — the reconcile-vs-prune race across a restart, executed 19/20 at the driver seam ([`F2-VAL-035`](../findings/F2-VAL-035.md)); [`F-VAL-068`](../findings/F-VAL-068.md) **superseded and split** (D1 → [`F2-VAL-031`](../findings/F2-VAL-031.md) Medium 80; D2 → `F-VAL-005`'s documented residual, `known`; D3 → `F2-VAL-035`; D4 → nearest [`F2-XC-006`](../findings/F2-XC-006.md)); new [`F2-VAL-034`](../findings/F2-VAL-034.md) (no schema-version check, Informational `known`); `F-VAL-061`'s restart window widened as predicted. Details: [`RECONCILIATION.md`](RECONCILIATION.md) §5.2 (`F-VAL-005` reply) and the [validator part](../state/run2/reconciliation/validator.md) §4. |
| Sentinel deadlines | #914 | **Merged alone** as `8b6a75d` (squash of `origin/fix/sentinel_deadlines` `dd2c54d`). | Fixes nothing of ours (R7Δ re-validated all nine sentinel findings; QA2-SEN-Δ re-ran twelve PoC tests). **Introduces [`F2-SEN-010`](../findings/F2-SEN-010.md), High 93** — the `F-SEN-016` D1 defect, now confirmed on `main` with Rust and forge execution; `F-SEN-016` D1 is superseded by it. `F-SEN-002`'s trigger set broadened; `F-SEN-003`'s restart variant narrowed by one block; `F-SEN-004`/`F2-SEN-005` narrowed (85 → 78), not fixed; `F-SEN-005` untouched. [`RECONCILIATION.md`](RECONCILIATION.md) §6. |
| Optimistic block transition | #915 | **Unmerged** — `origin/feat/optimistic_block_transition` at `b2aad06`, 2 ahead / 31 behind `main`; #914's `STOPGAP` note (`crates/sentinel/src/service.rs:390-402`) defers the compensation revert to it (safe-research/safenet#471). | `F-SEN-016` D2–D4 remain forward-looking; the warp arm at `crates/core/src/state/mod.rs:173-181` is the pre-#915 form. Round 3 below: unchanged head; resolves `F2-SEN-010`'s mechanism when rebased (executed). |
| Batched Execution | #899–#904 | **Unmerged** — `origin/feat/batex_4` at `d6edbb6`, 5 ahead / 31 behind `main` (`feat/batex_0`, `fix/batex_1`, `feat/batex_2`, `feat/batex_3` likewise); Phases 5–10 are still on no pushed branch. | `F-CORE-068`, `F-CORE-069` remain forward-looking; Round 1's re-run list stands. |
| SEF veto epic | #917 | **Merged** (`246d28e`, epic document only; no Rust change). | The Round 2 assessment stands as written; nothing to re-validate. |

**Counts after run 2.** In flight: two PR stacks (#915; #899–#904). Forward-looking findings: two whole files (`F-CORE-068`, `F-CORE-069`) and one partial (`F-SEN-016` D2–D4). Superseded: `F-VAL-068` (whole), `F-SEN-016` D1. Merged work that turned into a finding on `main`: `F2-SEN-010`. Merged work that fixed a finding: the pruning stack, for `F-VAL-005`'s reproduced trigger only.

**Re-run status.** Of the lists above: run 2 did not re-run run 1's Anvil PoCs (independence rule); its own equivalents were executed at `fe9e84c` — the validator store tests and `poc/F2-VAL-030`, `F2-VAL-031`, `poc/F2-XC-050/coverage-7.3` for the pruning stack; `poc/F2-SEN-001`…`010` re-runs (`*.rerun-fe9e84c.txt`) and the `F2-SEN-010` Rust + forge PoC for #914. Still not done: a reorg-nonce run that restarts a validator and asserts on the epoch-1 group with a same-machine control, and the multi-validator restart with a real group drop during the outage that would measure `F2-VAL-035`'s live rate ([`RECONCILIATION.md`](RECONCILIATION.md) §7).

## Round 3 — open branches after run 2

**Assessors:** IF-MISC and IF-SENBAT. **Audit HEAD:** `cfabcaa` (`crates/` equal to `origin/main` `8b6a75d`). Detailed files: [`misc-branches.md`](../state/run2/in-flight/misc-branches.md) (#915, batex, Reality Veto, six older work-in-progress branches) and [`senbat.md`](../state/run2/in-flight/senbat.md) (the MetaTransaction / Proposal / CallCoverage stack). Every branch was read with `git diff origin/main...<ref>`, `git show` and `git merge-tree`; nothing was fetched, checked out, merged or committed. One PoC was executed against a `git archive` extraction of a cherry-picked tree; the repository's tracked files were verified untouched. Nothing here is a finding against `main`.

| Work | PRs / branches | Verdict |
| --- | --- | --- |
| Optimistic block transition | #915 (`b2aad06`, unchanged since Round 2; 2 ahead / 31 behind, still on a pre-#914, pre-pruning base) | **Resolves the mechanism of [`F2-SEN-010`](../findings/F2-SEN-010.md)** once rebased — executed, 3/3. `F-SEN-002` untouched; `F-SEN-016` D2 latent, D3 present, D4 reduced to a timing note. |
| MetaTransaction / Proposal / CallCoverage | #947–#958 (`feat/senbat_1` … `senbat_7c`, base `bfecf1d`) | **Unrelated to every ledger row**: all 19 changed files are under `crates/sentinel-engine/` (out of scope) or the epic; `crates/core`, `crates/sentinel`, `crates/validator`, `Cargo.*` and `contracts/src` are byte-identical to the base. Needs a rebase (the base predates #914; the merge is conflict-free); `feat/senbat_7b_test`'s head (`2d44f74`) is not the commit `7b_checks`/`7c` build on (`3a9cc44`). |
| Batched Execution | #899–#904 (`07b02ed`, `413fbb0`, `125def1`, `8118b69`, `d6edbb6`; all 31 behind) | **Unchanged**; `F-CORE-068`, `F-CORE-069` forward-looking exactly as filed; Round 1's re-run list stands. |
| Reality Veto module | `pr/pin-reality-module-test-deps`, `pr/reality-veto-module-{contract,tests,deploy-and-runbook}` | **Contracts only**: no `crates/` file; three new files under `contracts/src` (`interfaces/IRealityModule.sol`, `veto/RealityVetoModule.sol`, `veto/README.md`); the services' bindings are inline `sol!` declarations, so no finding's ABI assumption is affected. Round 2's #917 note stands (the runbook still does not ask whether the SafeDAO Safe is Safenet-protected). |
| Older work-in-progress branches | `fix/issue_820_exceeding_reorgs`, `wip/nonce-gen-optimizations`, `obs/validator-metrics`, `ncs/4`, `wip/safe-tx-types-refactor`, `wip/open-ended-rules` (88–189 behind) | **All Stale-superseded** by merged work (#834; Nonces 1–7b; Metrics 1–4; Sentinel Engine 1a–1e and #829). Two carry a hunk the review dropped that partially implements a recommended fix — see below. |

### #915 — optimistic block transition

- **Position.** Head `b2aad06` as Round 2 recorded; its first commit `dd2c54d` is the unsquashed #914. A three-way merge with `main` conflicts in `handle_block_advance`, but the cherry-pick of `b2aad06` alone onto `main` is conflict-free (`git merge-tree --write-tree --merge-base=dd2c54d origin/main b2aad06` → tree `f73f6e9`). `main`'s `STOPGAP` note (`crates/sentinel/src/service.rs:398-402`) is the revert this branch performs.
- **`F2-SEN-010` — resolves the mechanism (E1).** The switch to `CollectingVotes` now runs after block `commitDeadline`'s logs (`crates/core/src/state/mod.rs:309-321` on the branch) with the inclusive comparison restored (`service.rs:418`), so a commit mined in the deadline block is tallied. `poc/F2-SEN-010/poc.rs` pasted into the cherry-picked tree fails only on its #914 precondition ("Reveal at `NewBlock(20)`", which #915 moves to `NewBlock(21)` by design); with that precondition adapted to #915's delivery order — two lines, scratch copy only — **3/3 pass** (`committed_count: 2` after `Committed(OTHER)@20`, no premature `Finalize`, entry retained); the copy's `sentinel` (47) and `safenet-core` (102) suites pass. Log: [`poc/F2-SEN-010/inflight-915-rerun.txt`](../poc/F2-SEN-010/inflight-915-rerun.txt). The adversarial form of `F2-SEN-010` (a sentinel committing in the deadline block and never revealing) loses its lever with the count exact.
- **What stays open.** [`F-SEN-002`](../findings/F-SEN-002.md) (commits before the verdict, `service.rs:372-375`) and run 1's A4 trigger are untouched — remediation convergence item 4 ("stop early-finalizing on local tallies") is still the only fix that closes both. `F-SEN-016` D2 (warp rollback drops resumes, `state/mod.rs:206-223`) is still latent (`applied: false` at startup); D3 (early-transition actions re-queued after a reorg, `:232-239`, `:246-253`) is still present and is closed by the `F-CORE-067` idempotency key, not by #915; D4 reduces to a timing note — keygen and signing deadlines are local (`contracts/src` has no `block.number` deadline for either), so the `block >= deadline` arms evaluate over the same log set one block earlier in wall-clock. [`F2-SEN-003`](../findings/F2-SEN-003.md) unrelated; [`F2-CORE-030`](../findings/F2-CORE-030.md)/`F-CORE-031` changed shape (the snapshot now precedes the pending transition, `:287`, so block-transition effects re-emit on restart; log-originated effects are still lost); `F-CORE-067` gains one more block of replay.
- **Interaction notes.** `F2-SEN-010` remediation option 1 (`poc/F2-SEN-010/remediation.patch`, counting a `Committed` seen in `CollectingVotes`) composes with #915 but becomes unreachable in live indexing and its comment's premise becomes false — apply one, or rewrite the comment. Remediation convergence item 5 (a synthetic `NewBlock` at the end of each warp page) must keep the new `applied` flag consistent with `state/mod.rs:298-303`, and its sentence "the validator needs the same `<`/`<=` check the sentinel got in #914" is moot once #915 lands, because every comparison returns to the inclusive form.

### Stale branches with a revivable partial fix

- **`fix/issue_820_exceeding_reorgs` (`33fcdcc`) → [`F2-CORE-008`](../findings/F2-CORE-008.md) / [`F-CORE-005`](../findings/F-CORE-005.md).** The pre-review draft of #834 (`40467c5`; `driver.rs` and `index/mod.rs` byte-identical to the squash). Its `revalidate_last_block` returned `ExceededMaxReorgDepth` when the block to invalidate was the anchor itself (`crates/core/src/index/blocks.rs:504-506`, test `:1166` at `max_reorg_depth = 0`); the review moved the anchor into a separate `SafeBlock` field and dropped that guard, which is exactly the depth-0 `-32001` spin the two findings describe (`main` `blocks.rs:494-501`). **Partially resolves, superseded design** — port the guard, do not merge the branch.
- **`wip/nonce-gen-optimizations` (`abf32d8`) → [`F2-VAL-030`](../findings/F2-VAL-030.md) consequence 2.** The prototype of the Nonces series. Its `link` clears every pending reservation before recording the onchain chunk (`crates/validator/src/state/preprocess.rs:50-51`) and `reserve_chunk` refuses while one is pending (`:38-39`); `main`'s `link` is a plain insert (`preprocess.rs:211-213`), which is the `expected_chunk` cascade. That is the second half of `F2-VAL-030` remediation option 2. **Partially resolves, superseded design**; consequence 1 (the phantom counted as capacity) is not addressed — the branch's `available()` counts pending reservations as `main`'s does.

### Consolidated table

| Branch / PR | Finding | Relation | Evidence | Action |
| --- | --- | --- | --- | --- |
| #915 | `F2-SEN-010` | Resolves (mechanism) | branch `service.rs:418`, `state/mod.rs:309-321`; adapted PoC 3/3 on `f73f6e9` | rebase (clean) and merge; then re-run `poc/F2-SEN-010` and `poc/F2-SEN-002` |
| #915 | `F-SEN-002` | Unrelated | `service.rs:372-375` on `main` unchanged | remediation item 4 |
| #915 | `F-SEN-016` D2 / D3 / D4 | latent / present / timing-only | `state/mod.rs:206-223`; `:232-239`, `:246-253`; `main` `keygen.rs:1010-1087`, `sign.rs:585-706` | D2: snapshot after resumes or re-apply on warp rollback; D3: `F-CORE-067` key plus a duplicate-action test; D4: note in the PR |
| #915 | `F2-SEN-003` | Unrelated | `finalize()` untouched | — |
| #915 | `F2-CORE-030` / `F-CORE-031`; `F-CORE-067` | Changes; one more block of replay | `state/mod.rs:287` | remediation items 1 and 6 |
| #915 | remediation item 5; `F2-SEN-010` option 1 | design interaction / composes | `state/mod.rs:298-303`; `remediation.patch` hunk `service.rs:304-312` | keep `applied` consistent; pick #915 or option 1 |
| #947–#958 | every ledger row | Unrelated | no in-scope hunk; `crates/sentinel/src/engine.rs:59-72` and the engine's `Verdict` unchanged | rebase onto `8b6a75d`; re-push `7b_test`; roll the engine out with no in-flight requests or land `F-SEN-015` option 1 first |
| #899–#904 | `F-CORE-068`, `F-CORE-069` | forward-looking, unchanged | `feat/batex_4` anchors as filed | Round 1 re-run list on merge; rebase over 31 commits |
| Reality Veto (4) | all | Unrelated | no `crates/` file; three new `contracts/src` files | none for the Rust audit; answer the Charter R-4.1 question in the runbook |
| `fix/issue_820_exceeding_reorgs` | `F2-CORE-008` / `F-CORE-005` | Partially resolves (superseded design) | `blocks.rs:504-506`, `:1166` | port the anchor-invalidation guard to `SafeBlock` |
| `wip/nonce-gen-optimizations` | `F2-VAL-030` (consequence 2); `F-VAL-031` | Partially resolves (superseded design); Changes (superseded) | `preprocess.rs:38-39`, `:50-51`; `service/nonce_generator.rs:88-95` | adopt `link`'s clear-pending semantics in option 2 |
| `obs/validator-metrics`, `ncs/4`, `wip/safe-tx-types-refactor`, `wip/open-ended-rules` | all | Stale-superseded | superseded by #870–#880, #745–#754, #712–#716 / #829 | close the branches |

### What to re-run when #915 merges

1. `poc/F2-SEN-010` with the setup's precondition adapted as in [`inflight-915-rerun.txt`](../poc/F2-SEN-010/inflight-915-rerun.txt) — the three tests must pass on the merged tree, as they do on the cherry-pick.
2. `poc/F2-SEN-002` and `poc/F-SEN-002` — `F-SEN-002` must reproduce unchanged (the undercount source #915 does not touch).
3. Round 2's #915 list: `poc/F-SEN-001`, `F-SEN-015`, `F-CORE-067`, `F-CORE-001`, `poc/F2-SEN-001`…`009` (`F2-SEN-005`'s FIFO knife-edge moves back by one block), the `poc/F-VAL-005-066` ordering case, and the four new core tests in `crates/core/src/state/mod.rs`.
4. A new D3 test — uncle the block below an applied pending block and count the emitted actions — and, if a mid-run `Warp` is ever introduced, the `F-SEN-016` D2 probe (`PROBE-WARP`).
5. The validator keygen and signing flow tests (`scripts/run_validator_*` on Anvil) once, to confirm D4's one-block-earlier exclusion actions are accepted by `FROSTCoordinator`.
