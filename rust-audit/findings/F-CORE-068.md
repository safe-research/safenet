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
