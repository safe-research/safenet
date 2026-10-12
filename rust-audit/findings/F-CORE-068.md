# F-CORE-068 (UNMERGED) A batch's execution status is unobservable, so a whole-batch revert or a mid-batch `CallFailed` silently drops up to a full batch of actions

| Field | Value |
| --- | --- |
| Status | Filed (forward-looking, unmerged code) |
| Crate and module | core, `tx/mod.rs`, `tx/storage.rs`, `tx/executor.rs` (planned); contracts, `Safenet7702Executor.sol` |
| Location | **UNMERGED — applies to branch `origin/feat/batex_4` (PR #904) and to the planned Phases 6–7 of the epic; not present on `main` (`49d7e39`).** `contracts/src/Safenet7702Executor.sol:107-123` (guard at `:115`, swallow at `:118-120`) on `origin/feat/batex_4`; queue side `crates/core/src/tx/storage.rs:280-288` (`mark_executed`) and `:301-330` (`prune`), unchanged from `main`. Specified in `epics/2026_09_09_safenet_7702_executor_tx_batching.md` §"Batch building and gas accounting", §"Swallowed inner failures". |
| Severity | High / Medium (forward-looking) |
| Certainty | 85% (static, against unmerged code; the batching wiring it depends on is Phases 6–7 and is not yet on any pushed branch) |
| Assumptions involved | A1, A4 |
| Tags | unmerged, batching, eip-7702, observability, liveness |

## Claim

`TransactionQueue` decides that a transaction executed purely because the account nonce moved past its nonce (`F-CORE-063`); it never reads a receipt, a status, or a log. Under batching that inference becomes a per-**batch** inference over a transaction that carries up to `max_batch_gas / per-call gas` — on the epic's own default numbers, six to eight — protocol actions. Three distinct outcomes are indistinguishable to the queue, because all three advance the nonce:

1. **The batch succeeded and every call took effect.**
2. **A call reverted.** `execute` swallows it into a `CallFailed(index, result)` event and continues (`Safenet7702Executor.sol:118-120`). The transaction succeeds. Nothing offchain watches the signer's own EOA for logs, as the epic states outright.
3. **The batch reverted as a whole** — `InsufficientGas(i)` from the new Phase 1 guard (`:115`), or `OnlySelf()` if the delegation is not in place. **Every** call in the batch is rolled back, including calls that had already succeeded at lower indices.

In cases 2 and 3 `mark_executed` sets `executed_at` for the batch row, `prune` deletes it once the block is reorg-safe, and the actions are gone: not retried, not reported, not recoverable. The state machine believes all of them were performed.

This is not merely `F-CORE-063` restated. `F-CORE-063`'s blast radius is one action per silent failure. Batching raises it to a whole batch per silent failure, and case 3 is a _new_ failure mode that did not exist before Phase 1: previously an under-gassed standalone transaction failed alone. The epic's own framing — "it converts a silent action-dropping failure into a loud one" — is true onchain (the transaction reverts) and false offchain (the queue cannot see a revert), so the guard makes the drop _bigger_ from the service's point of view while making it louder from a block explorer's.

Order dependence makes case 2 worse than a raw count suggests. The sentinel emits `ApproveToken` before `Commit` in one driver update; the epic's batcher deliberately preserves that order within a batch. If the `ApproveToken` call fails (`F-SEN-008`'s non-zero-allowance ERC-20 case), `execute` does **not** stop — the `Commit` still runs, reverts for want of allowance, is swallowed as a second `CallFailed`, and the batch reports success. The batch is a self-call to the service's own EOA, so even an operator watching the oracle's events sees nothing; the only trace is a `CallFailed` on an address nobody indexes.

## Basis

- `execute` swallows call failures by design and documents it as such (`Safenet7702Executor.sol` `@custom:warning`, "executes batches best-effort (failures are swallowed and only logged via {CallFailed})").
- The new guard reverts the whole batch: `require(gasleft() * 63 / 64 >= call.gasLimit, InsufficientGas(i))` at `:115`, evaluated _inside_ the loop, so calls at indices `< i` are rolled back with it.
- `mark_executed` is a pure nonce comparison and is untouched by the batex stack (`storage.rs:280-288`): `WHERE nonce IS NOT NULL AND nonce < ? AND executed_at IS NULL`.
- `prune` deletes executed rows at or below the safe block (`storage.rs:301-315`), so the record of what was in the batch is destroyed within `max_reorg_depth` blocks (default 5, ~25 s on Gnosis).
- The epic acknowledges the observability gap and puts it out of scope: "Surfacing them (the services do not currently watch their own EOA for logs) is deliberately out of scope; flagging it as a known observability gap."

## Trigger

Any batch containing an action that reverts (a duplicate under `F-VAL-065`/`F-SEN-006`, a commit without bond under `F-SEN-007`, an allowance-rejecting approve under `F-SEN-008`), or any batch whose transaction gas limit is below the `InsufficientGas` threshold for some call — which the epic's own open questions concede is an unmeasured formula ("the `26_000` base and `5_000` per-call overhead in the batch gas formula are estimates").

## Remediation options

- Before marking a batch executed, read its receipt: `status == 0` means every call in it must be re-queued, and a `CallFailed` log means that index must be re-queued. This is one `eth_getTransactionReceipt` per executed batch and it resolves `F-CORE-063` for batched and unbatched transactions alike.
- Failing that, decode the batch row's `calls` on `executed_at` and emit one `warn` per call so the action is at least recoverable from logs before `prune` destroys the row.
- Add a counter metric for `CallFailed` observed on the signer's own address, so a silently dropping deployment is visible without a receipt read.

## Trail

Filed by FWD from `origin/feat/batex_4` + the epic at `epics/2026_09_09_safenet_7702_executor_tx_batching.md`. No PoC: the batching wiring (Phases 6–7) is not on any pushed branch, so this is static-only and must be re-validated when Phase 7 lands.

## Reconciliation (run 2)

**Final: Unmerged, forward-looking — unchanged.** The Batched Execution stack (PR #904) is not in the tree at `fe9e84c` (`crates/core/src/tx/` has no `executor.rs`; `types.rs` has no `authorization` field), so run 2 could not and did not assess it; this is not a miss. Re-validate when the stack merges (`report/IN-FLIGHT.md` round 1). No run-2 counterpart (`state/run2/reconciliation/core.md` §2).

## Run 3 re-validation (rv-core-b, 2026-10-08)

| Field | Value |
| --- | --- |
| Audit commit | `361aaf3` |
| Verdict | Changed |
| Changed by | The `feat/batex_4` code this row was written against never merged. The merged 7702 stack (#987, #900, #989 to #1006, #1023; queue side `ce01715` #995, `acacac1` #1001, executor `contracts/src/Safenet7702Executor.sol`) carries the mechanism: execution of a batch row is still inferred from the account nonce alone. Of the three outcomes, the whole-batch revert is settled as 7702 review low L4 (`settled.md` section 1: "a reverted batch loses all its actions, with no receipt check"), and the `OnlySelf` route is unreachable for the service's own batches. What remains is the swallowed mid-batch `CallFailed`. |
| Severity, certainty | Proposed Low (was High / Medium, forward-looking), Confirmed, 90 (E1 for the queue side, Foundry for the executor side). Recommend folding it into `F-CORE-063` at reconciliation, as a remediation note for batched rows. |
| Evidence | E1 `poc/F-CORE-068/run3/` (new queue tests; `output-forge-executor.txt` for the executor, reference only) |

Re-based anchors (old `feat/batex_4` locations, merged locations at `361aaf3`, verbatim quote).

- Executor swallow, `Safenet7702Executor.sol:118-120` on `feat/batex_4` to `contracts/src/Safenet7702Executor.sol:106-109` (reference only):

```solidity
            (bool success, bytes memory result) = call.to.call{gas: call.gasLimit, value: call.value}(call.data);
            if (!success) {
                emit CallFailed(i, result);
            }
```

- Whole-batch guard, `:115` to `contracts/src/Safenet7702Executor.sol:104`: `require(gasleft() * 63 / 64 >= call.gasLimit, InsufficientGas(i));` (the L4 case).
- `mark_executed`, `storage.rs:280-288` to `crates/core/src/tx/storage.rs:281-292`: `UPDATE allocated_nonces SET executed_at = ? WHERE nonce < ? AND executed_at IS NULL`, one row per batch.
- `prune`, `storage.rs:301-330` to `crates/core/src/tx/storage.rs:326-344`: deletes the queued `transactions` rows of an executed nonce, then the nonce row, so a batch's calls go together.
- The interface contract the services rely on (`git show a9f7eef^:epics/2026_09_23_safenet_7702_executor_tx_batching.md`, lines 171 and 175): "A failing call must not revert the batch." and "`CallFailed` exists only to help debugging, and no service depends on it."
- `OnlySelf`: the merged design attaches the authorization to the batch that needs it, processed before the call (epic line 76; `crates/core/src/tx/mod.rs:248-258`), and refuses an executor without code at startup (`crates/core/src/tx/mod.rs:130-135`, #1005). A first-party self-call therefore always runs against delegated code, and `OnlySelf` (`contracts/src/Safenet7702Executor.sol:97`) only rejects other callers.

What was run (worktree at `361aaf3`, `poc/F-CORE-068/run3/tx_mod_tests_run3.rs` appended to `crates/core/src/tx/mod.rs`'s `mod tests`, reverted, `git status --short` empty):

- `cargo test -p safenet-core --lib rv3_ -- --nocapture --test-threads=1`.
- `rv3_f_core_068_batch_marked_executed_by_nonce_and_pruned_with_its_calls`: ok. A three-call batch at nonce 0 carrying the authorization is marked executed at block 11 when `eth_getProof` reads nonce 2 and delegated code; the only RPC issued is `eth_getProof` (no receipt, status or log request); at safe 11 the batch row, its authorization row and all three queued transactions are deleted. The queue cannot distinguish success, a swallowed `CallFailed`, or an `InsufficientGas` revert: all three leave the same account state.
- `rv3_f_core_068_direct_reverted_transaction_is_treated_the_same`: ok. In direct mode a mined but reverted standalone transaction is marked executed and pruned the same way.
- `forge test --match-path test/Safenet7702Executor.t.sol -vv` (forge 1.5.1, existing tests): 10 passed, among them `test_Execute_ContinuesPastFailure_EmitsCallFailed` and `test_Execute_InsufficientGas_RollsBackEarlierCalls`.

What changed, and why Changed rather than Settled:

- Case 3 (whole-batch revert, every call lost, including earlier successful ones) is exactly L4, and the trigger "a batch gas limit below the guard" rests on the batch gas constants, which is L5 (no worst-case test). Both are settled review lows.
- Case 2 (a call reverts inside a batch that succeeds) is not in L4's text, so the match is not exact. Its loss is one action per failing call, and the calls before and after still run; that is the same loss as a reverting standalone transaction in direct mode, which `F-CORE-063` (via `F2-CORE-061`'s "related consequence") already carries. The one batching-specific aspect is observability: the batch transaction reports status 1, so a receipt-status check, the remediation proposed for `F-CORE-063` and for L4, would still miss it; only the `CallFailed` log on the service's own address shows it.
- The order-dependence example (a failing `ApproveToken` followed by a `Commit` that reverts for want of allowance) behaves the same in direct mode, where the two are consecutive transactions. No service action carries a native value (`value: U256::ZERO` throughout `crates/validator/src/service/action.rs` and `crates/sentinel/src/service.rs:865-930`), so the "unaffordable value is swallowed" path of the executor has no Safenet trigger.
- An action whose own `gas` is under-estimated runs out of gas inside its call and is swallowed as a `CallFailed`; in direct mode the same action reverts the standalone transaction. Batched calls get the action's whole `gas` as their call budget (no intrinsic cost deducted), so batching does not make this more likely.

Notes: the residual is Low (an error-handling and observability gap with the same per-action loss as direct mode). If the head reads L4 broadly ("no receipt check" covering everything a batch receipt would show), the remainder is fully `F-CORE-063`'s and this row can be marked Settled; the `F-CORE-063` run-3 section already records the batched-row remediation note, so nothing is lost either way. The `F-VAL-065` and `F-SEN-006` duplicates that revert become `CallFailed` entries in batched mode (see `F-CORE-067`, run-3 section).
