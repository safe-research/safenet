# Plan: Safenet 7702 executor transaction batching

Component: `crates/core` (Cargo package `safenet-core`), specifically the `tx` module (transaction queue, its storage, config and signer) and the `driver` that wires it up; plus the `Validator7702Account` contract in `contracts/` and the startup paths of the `validator` and `sentinel` binaries.

---

## Overview

`contracts/src/Validator7702Account.sol` already exists as an EIP-7702 delegation target that batches calls for the validator EOA, but nothing offchain uses it. Every action a service emits is submitted as its own EIP-1559 transaction, consuming one nonce each. A block that produces several actions (a `Logs` update covering multiple signing rounds, for example) therefore turns into several sequential transactions, each paying its own 21,000 gas intrinsic cost and each competing for the queue's `max_in_flight_transactions` budget.

This epic makes the transaction queue able to submit a batch of calls as a single transaction through the delegated account:

- Rename the contract to `Safenet7702Executor` — it is used by more than the validator — as a pure, no-logic rename.
- Gas-optimize `execute` by binding each `Call` to a single calldata pointer instead of re-resolving `calls[i]` three times per iteration.
- Add two `[transactions]` config parameters: the executor address the service delegates to, and the maximum gas a single batch may consume.
- Teach the queue to carry an EIP-7702 `SetCode` transaction, so the delegation is enqueued on service start like any other transaction and inherits the queue's nonce management, fee bumping, resubmission and reorg handling.
- Group transactions into `Safenet7702Executor.execute` batches in `TransactionQueue::queue`, before they are enqueued, starting a new batch whenever the next call would exceed the configured max batch gas.

The work splits into two independent tracks that only converge at the integration test: a contracts track (the rename and gas optimization together) and a Rust track (config, delegation transaction support, startup enqueue, batch builder, then queue wiring). Documentation and integration-test coverage land at the end, followed by removal of this specification.

---

## Architecture Decision

Batching lives entirely inside `crates/core/src/tx`. Neither service's action encoder, state machine, nor the `Driver`'s command dispatch changes: they keep producing one `(Transaction, Option<u64>)` per action, and the queue decides how to get those onchain. This keeps the batching concern in one place, gives both the validator and the sentinel the feature for free, and leaves the existing `ActionEncoder` contract — and its per-action `gas` estimate, which becomes the batch's per-call `gasLimit` — untouched.

```text
 Service actions                      crates/core/src/tx
 ---------------                      ------------------
 ActionEncoder::encode_action
        |
        | Vec<(Transaction, Option<u64>)>
        v
 TransactionQueue::queue ----> executor::batch(sender, txs, max_batch_gas)
        |                            |
        |                            +-- runs of equal `expires_at`
        |                            +-- greedy fill up to max_batch_gas
        |                            +-- abi_encode execute(Call[]) to `sender`
        v
 TransactionStorage::enqueue  (one row per submitted transaction)
        |                     Driver::new enqueues the delegation here first,
        |                     so it holds the lowest nonce.
        v
 submit_pending / resubmit_stale
        |                     Unchanged, except that a delegation transaction
        |                     reserves two nonces and is signed as TxEip7702.
        v
 chain
```

### The delegation goes through the transaction queue

The EIP-7702 `SetCode` transaction is enqueued by `Driver::new` when an executor is configured, before any service action can be queued, exactly as the validator already queues its startup `SetValidatorStaker` action. It is therefore an ordinary queued transaction: it gets the lowest nonce, and it inherits nonce allocation, EIP-1559 fee bumping, stale resubmission, underpriced-rejection handling, execution marking and reorg invalidation for free. Reimplementing any part of that in a bespoke startup submission path would be strictly worse.

### Nonce ordering is the gate, so no confirmation read is needed

If the queue sends `execute(Call[])` to an EOA that has no delegated code, the call **succeeds as a no-op**: the calldata is ignored, the transaction is mined, the nonce advances, and every action in the batch is silently dropped. Nothing reverts and nothing logs. Making that state unreachable does not require reading the account's code, because the queue's own invariants already exclude it:

1. The delegation is enqueued first, so it is allocated the lowest nonce.
2. It is enqueued with `expires_at: None`, so it is never dropped or pruned while unexecuted, and it is resubmitted with bumped fees until it lands.
3. Nonces are allocated in ascending order and the chain executes them in that order, so no later transaction — batched or not — can execute before the delegation.

Any batch that executes therefore executes against delegated code. This holds across restarts, since the unexecuted delegation persists in the transaction storage.

### A self-sponsored `SetCode` transaction reserves two nonces

This is the one place the queue genuinely has to learn something new. Under EIP-7702 the sender's nonce is incremented before the authorization list is processed, so a self-signed authorization must specify `nonce + 1`, and applying it increments the account nonce again: a delegation transaction at nonce `N` leaves the account at `N + 2`.

The queue's storage allocates `MAX(status.nonce, MAX(nonce) + 1)`. Left alone it would hand `N + 1` to the next transaction — the nonce the authorization itself consumes. That transaction could never execute, and worse, `mark_executed` (which marks everything with `nonce < onchain_nonce`) would mark it executed anyway, silently dropping it.

The fix is to make nonce allocation aware of how many nonces a transaction consumes: a delegation transaction spans two, everything else spans one. This is a single change to the allocation expression, keeps the pipeline flowing (later transactions are still allocated and submitted while the delegation is in flight, and simply wait in the mempool for their turn), and leaves the rest of the queue's nonce accounting — including recovery from nonces consumed outside the queue — untouched.

If an authorization were ever skipped by the chain, the account would stop one nonce short of the reservation and every subsequent transaction would be stuck behind a permanent gap. That is a loud failure (nothing progresses, resubmissions repeat, block metrics flatten) rather than a silent one, and it is reachable only through a construction bug or a contract account at the signer address. The queue additionally logs at `error` when it marks a delegation executed and the observed nonce advanced by one instead of two, which needs no extra RPC request since it already has both numbers.

### Batch boundaries preserve order and expiry

Batching walks the transactions in the order the driver produced them and starts a new batch when either the accumulated gas would exceed `max_batch_gas` or `expires_at` changes. Order is preserved because it is load-bearing — the sentinel emits `ApproveToken` before `Commit`, and batches execute in nonce order while calls within a batch execute in array order.

Splitting on any `expires_at` change (rather than assigning a batch the minimum expiry of its members) means a batch never drops a still-valid action because a batch-mate's deadline passed.

### Alternatives Considered

- **Submit the delegation out of band at startup**, signing and broadcasting a `TxEip7702` directly through the provider and awaiting its receipt before starting the driver. Avoids teaching the queue about `SetCode` transactions and the two-nonce reservation, but gives up the queue's fee bumping, stale resubmission and underpriced-rejection handling, so it would need a bespoke retry loop for the one transaction the service cannot start without. Rejected.
- **Gate batching on an `eth_getCode(signer)` confirmation**, enabling it only once the `0xef0100 ‖ executor` designator is observed. An extra RPC request per start that buys nothing: nonce ordering already guarantees no batch executes before the delegation, and the signer's key is not used anywhere else, so the delegation cannot be changed behind the service's back. Rejected.
- **Skip the delegation when the account is already delegated**, via one `eth_getCode(signer)` at startup. Saves roughly 34,000 gas per restart at the cost of an RPC request and a branch; restarts are rare enough that the unconditional, self-healing enqueue is preferred. Noted as an available optimization, not planned.
- **Barrier instead of reservation**: refuse to allocate any nonce while a delegation is in flight. Equally correct and slightly simpler to reason about, but stalls the submission pipeline until the delegation lands, and its failure mode (batches silently no-op) is quieter than the reservation's (queue visibly stalls). Rejected.
- **Piggyback the authorization on the first batch** rather than sending a standalone `SetCode` transaction. The authorization applies before the call, so it would work and save one transaction, but it makes the first batch's correctness depend on the authorization applying and pushes 7702 concerns into the batch encoder. Rejected.
- **Batch with the minimum member expiry.** Yields larger batches, but the queue drops a transaction that has not been submitted by its expiry block, so a batch inheriting the earliest deadline can discard actions that are still valid. Rejected: silently losing actions is worse than a smaller batch.
- **Group by expiry across the whole input (stable partition).** Also yields larger batches, but reorders actions relative to each other across batches, breaking the sentinel's approve-then-commit ordering. Rejected.
- **Batch at submission time** in `submit_pending`, coalescing whatever is queued but un-nonced. Strictly more batching, since it also combines transactions queued across different driver updates. Rejected for this epic: it requires reserving one nonce for N storage rows and redefining what a reorg-invalidated batch means for its members, which is a large change to the most safety-critical part of the queue when a single driver update already emits the multiple actions this epic targets.
- **Batch at the `Driver` level, before `queue()`.** Equivalent placement but puts an executor-specific concern in the generic service driver and makes the queue's config the wrong home for `executor`/`max_batch_gas`. Rejected.
- **Add `value` to the executor's `Call` struct.** Would let batching carry value-bearing transactions. Not needed: every action either service encodes today sets `value: U256::ZERO`. Batching instead excludes non-zero-value transactions (see Tech Specs), which keeps the contract minimal.

---

## Tech Specs

### Contract: `Safenet7702Executor`

`contracts/src/Validator7702Account.sol` becomes `contracts/src/Safenet7702Executor.sol`, with `contract Validator7702Account` → `contract Safenet7702Executor` and the doc comments retitled from "Validator 7702 Account" to "Safenet 7702 Executor". `contracts/test/Validator7702Account.t.sol` and `contracts/script/DeployValidator7702Account.s.sol` are renamed to match (`Safenet7702Executor.t.sol`, `DeploySafenet7702Executor.s.sol`, `DeploySafenet7702ExecutorScript`). No other file in the repository references the contract by name. The rename PR carries no logic change.

The gas optimization rewrites the loop body to bind the calldata struct once:

```solidity
for (uint256 i = 0; i < calls.length; ++i) {
    Call calldata call = calls[i];
    (bool success, bytes memory result) = call.to.call{gas: call.gasLimit}(call.data);
    if (!success) {
        emit CallFailed(i, result);
    }
}
```

`calls[i]` currently re-computes the array element offset for each of the three field reads. The optimization PR adds a gas assertion to the existing test file that pins the observed `execute` cost for a fixed multi-call batch, so the improvement is measured rather than asserted, and future regressions are caught.

Phase 1 also adds a per-call gas guard, because `execute` as written does not enforce that a call receives the `gasLimit` it asks for:

```solidity
require(gasleft() * 63 / 64 >= call.gasLimit, InsufficientGas(i));
```

EIP-150 forwards at most 63/64 of the gas remaining when the call is made, and Solidity does not check the shortfall. A truncated call that runs out of gas is indistinguishable from one that reverted, so it was swallowed as a `CallFailed` and `execute` still returned success. Measured on the unguarded contract, with one call reserving 1,000,000 gas and really needing 500,000:

| transaction gas limit | `execute` succeeded | call took effect |
| --------------------- | ------------------- | ---------------- |
| 50,000 / 100,000      | no                  | no               |
| 144,687               | **yes**             | **no**           |
| 200,000 / 400,000     | **yes**             | **no**           |
| 700,000 and above     | yes                 | yes              |

So there was a wide band in which the transaction succeeded and the batched call was silently discarded. Worse, that band broke gas estimation: `eth_estimateGas` binary-searches for the _lowest_ gas limit at which the transaction does not revert, which is by construction the "callee ran out of gas, ~1/64 remained to emit `CallFailed` and return" point. It returned **144,687** for a batch needing ~1,050,000 — roughly 7× too low, and a limit at which nothing executes. The behavior was also non-monotonic, so estimation's core assumption did not hold at all.

With the guard, estimation on the same batch returns **1,016,665** — matching `gasLimit × 64/63` plus dispatch — and every gas limit below it fails loudly while every limit at or above it both succeeds and takes effect.

The check is deliberately **best-effort and approximate**. It encodes EIP-150's 63/64 rule but does _not_ model the `CALL`'s own base cost — cold account access, argument copy, memory expansion — which the EVM deducts before that rule applies. Those are gas _prices_, repriced by hardforks (cold account access was already repriced by EIP-2929), so hardcoding them would age badly in exchange for a bound that only needs to be roughly right. The 63/64 factor is a structural consensus rule rather than a price, and it fails safe: if it were ever relaxed so that calls receive all remaining gas, the check would merely become conservative, never unsafe. Sizing the gas limit to cover the EVM's base deductions on top of each `gasLimit` is the caller's responsibility, which the batch gas formula below discharges.

The residual window this leaves is measurable and narrow. At the estimate above, the callee receives roughly `gasLimit` minus the `CALL`'s base cost (~2,560 gas), so a call needing within ~0.26% of its full reservation can still be truncated silently: with the callee needing 999,000 of its 1,000,000 reservation, the transaction succeeded at the estimate and the call did not take effect. A call needing 500,000 of the same reservation behaved correctly. Chasing that last sliver onchain is exactly the exactness the check declines to attempt; the per-call overhead term in the batch gas formula covers it offchain.

The guard changes `execute`'s failure semantics, which is the reason it is worth stating here: an underfunded batch now reverts as a whole instead of applying a prefix and reporting success.

The guard costs roughly 491 gas per call, which is more than the calldata-pointer optimization saves (298 per call), so Phase 1 is a net gas _increase_ of about 193 gas per call. That is a deliberate trade: it converts a silent action-dropping failure into a loud one. The cost is this high only because `Safenet7702Executor` is compiled with neither the optimizer nor viaIR (`foundry.toml` restricts viaIR to four other contracts), so each sub-expression in the loop body carries real cost — the same reason the calldata-pointer binding saves as much as it does. For reference, the weaker `require(gasleft() >= call.gasLimit, ...)` costs only 65 gas per call but widens the residual window to ~1.6% of each reservation. Adding this contract to the viaIR set would likely make the guard close to free, but it changes the deployed bytecode and hence the contract's deterministic address, so it is left as a follow-up rather than bundled into the rename.

### Config

`tx::Config` (`crates/core/src/tx/mod.rs`), reachable as the `[transactions]` table of both services' config files:

```rust
pub struct Config {
    pub max_in_flight_transactions: usize,
    pub blocks_before_resubmit: u64,
    pub priority_fee_cap_percentage: Option<f64>,
    /// The `Safenet7702Executor` contract the signer account delegates to via
    /// EIP-7702, enabling call batching. `None` disables batching.
    pub executor: Option<Address>,
    /// The maximum gas a single batched transaction may consume. Ignored when
    /// `executor` is `None`.
    pub max_batch_gas: u64,
}
```

Defaults: `executor: None`, `max_batch_gas: 2_000_000`. `Config` keeps `#[serde(default, deny_unknown_fields)]`, so both fields are optional and omitting `executor` preserves today's behavior exactly.

Sample TOML additions to the existing `[transactions]` tables of `crates/validator/validator.sample.toml` and `crates/sentinel/sentinel.sample.toml`:

```toml
[transactions]
# Optional: the `Safenet7702Executor` this service's signer account delegates
# to via EIP-7702, so several onchain actions can be submitted as a single
# batched transaction. The service submits the delegation transaction on
# startup, and no action is executed before it lands. Omit to submit one
# transaction per action.
# executor = "0x0000000000000000000000000000000000000000"

# Optional: the maximum gas a single batched transaction may consume; actions
# are split across batches so that no batch exceeds it.
# max_batch_gas = 2000000
```

### Delegation transactions in the queue

`Transaction` (`crates/core/src/tx/types.rs`) gains one field:

```rust
pub struct Transaction {
    pub to: Address,
    pub value: U256,
    pub data: Bytes,
    pub gas: u64,
    /// The EIP-7702 delegation target to authorize, making this a `SetCode`
    /// transaction that consumes two nonces. Set by the queue for its own
    /// delegation transaction; action encoders must leave this `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorization: Option<Address>,
}
```

`#[serde(default)]` keeps transaction rows written by earlier versions loadable, and `skip_serializing_if` keeps the stored JSON unchanged for the overwhelmingly common case.

The authorization is a function of the transaction's own nonce, so it can only be built once a nonce is allocated, and it must be rebuilt identically on every resubmission (which it is, because the queue never reallocates a nonce). Signing therefore moves behind the signer:

```rust
// types.rs — no key material involved.
pub enum UnsignedTransaction {
    Eip1559(TxEip1559),
    /// A `SetCode` transaction whose authorization list is filled in at
    /// signing time from the transaction's own nonce.
    Eip7702 { tx: TxEip7702, delegate: Address },
}

impl AllocatedTransaction {
    /// The fees this transaction will be submitted with, bumped above any
    /// previous submission.
    pub fn bumped_fees(&self, estimate: Eip1559Estimation) -> Eip1559Estimation;

    pub fn build(self, chain_id: u64, fees: Eip1559Estimation) -> UnsignedTransaction;
}

// signer.rs
impl Signer {
    pub fn sign_transaction(
        &self,
        tx: UnsignedTransaction,
    ) -> Result<SignedTransaction, SigningError>;
}
```

For the `Eip7702` variant, `sign_transaction` builds `Authorization { chain_id, address: delegate, nonce: tx.nonce + 1 }`, signs it over `authorization.signature_hash()` with `SignerSync::sign_hash_sync`, sets it as the sole entry of `authorization_list`, and then signs the transaction as usual. The authorization's `chain_id` is the real chain id rather than `0`, so the delegation cannot be replayed on another chain. Both variants can be carried by alloy's `TypedTransaction` for the shared `SignableTransaction`/`Encodable2718` path; the exact envelope conversion is an implementation detail.

`submit_transaction` needs a small rearrangement: it currently derives its `Submission` record from the built `TxEip1559`, but `UnsignedTransaction` is an enum. It instead computes `bumped_fees` first, records the `Submission` from the allocated nonce and those fees, and then builds and signs. Behavior is identical.

Nonce allocation in `storage.rs` becomes span-aware:

```sql
UPDATE transactions
SET nonce = MAX(?, COALESCE((
        SELECT MAX(nonce + IIF(json_extract(request, '$.authorization') IS NULL, 1, 2))
        FROM transactions WHERE nonce IS NOT NULL
    ), 0))
...
```

and two small helpers are added for the startup path and the self-check:

```rust
/// The nonce of the outstanding delegation transaction, if one is in flight.
pub async fn pending_delegation(&self) -> Result<Option<u64>, Error>;

/// Whether a delegation transaction is queued or in flight, used to keep the
/// startup enqueue idempotent across restarts.
pub async fn has_delegation(&self) -> Result<bool, Error>;
```

`update_block_status` consults `pending_delegation` only while one is outstanding, and logs at `error` if the delegation is about to be marked executed with the account nonce having advanced by one rather than two.

The delegation transaction itself is `Transaction { to: signer.address(), value: 0, data: empty, gas: 60_000, authorization: Some(executor) }`. The empty-calldata self-call reaches the executor's `receive()` — the authorization applies before the call, so the code is already in place — and succeeds. The gas covers the 21,000 intrinsic cost plus `PER_AUTH_BASE_COST` (12,500), with headroom for the `PER_EMPTY_ACCOUNT_COST` case.

Queue API:

```rust
/// Queues the EIP-7702 delegation transaction pointing the signer account at
/// `executor`, unless one is already queued or in flight. It never expires,
/// and it takes the lowest nonce, so no other queued transaction can execute
/// before it.
pub async fn queue_delegation(&mut self, executor: Address) -> Result<(), Error>;
```

`Driver::new` calls it when `config.transactions.executor` is set, before returning, so the ordering relative to `queue_action` cannot be got wrong at the call site and neither binary's `main` changes. At that point `block_status` is still `None`, so `queue()` enqueues without attempting submission.

`Driver::new` also performs one startup sanity check when an executor is configured: `eth_getCode(executor)` must be non-empty. EIP-7702 happily delegates to an address with no code, and doing so re-opens the silent no-op failure mode, so a typo'd or not-yet-deployed executor address must fail loudly at startup rather than quietly discarding every action. This is one RPC request per start, on the executor address rather than the signer's.

### Batch building and gas accounting

New module `crates/core/src/tx/executor.rs`, named after the contract it speaks to, holding the `sol!` binding for the executor and one pure batching function:

```rust
sol! {
    struct Call { address to; uint256 gasLimit; bytes data; }
    function execute(Call[] calldata calls) external;
}

/// Groups `transactions` into batched `execute` calls, each sent by `sender`
/// to itself, so that no batch exceeds `max_batch_gas`.
pub fn batch(
    sender: Address,
    transactions: impl IntoIterator<Item = (Transaction, Option<u64>)>,
    max_batch_gas: u64,
) -> Vec<(Transaction, Option<u64>)>;
```

`sender` is the signer's own account: a batch is a self-call, so the function needs the address to set `to` on the transactions it builds. It is a parameter rather than being read from the queue so that the function stays pure and can be table-tested without a signer, provider or pool. The parameter is deliberately _not_ named after the executor — the executor address is only ever an authorization target, and sending `execute` calldata to it instead of to the signer's account would silently discard every action, so the two must not be confusable at the call site. The alternative shape, if that risk is judged too high, is for the function to return an intermediate `enum Batch { Single(Transaction), Batched { calls, gas } }` and let `queue()` be the only place that supplies an address; it removes the parameter at the cost of one extra type and a mapping step.

Rules:

1. A new batch starts when `expires_at` differs from the current batch's, or when adding the transaction would push the batch gas above `max_batch_gas`.
2. A batch of one call is emitted as the original unbatched `Transaction`, avoiding the executor's overhead entirely.
3. A transaction that cannot fit in an empty batch (its own gas already exceeds `max_batch_gas`) is emitted unbatched and logs a warning. It is never dropped and never silently truncated.
4. A transaction with a non-zero `value` is emitted unbatched, because the executor's `Call` carries no value. A warning is logged, since no action encoder produces one today.
5. A transaction carrying an `authorization` is emitted unbatched: the delegation transaction is never a batch member.
6. Batches are emitted with `to: sender`, `value: U256::ZERO`, `data: executeCall { calls }.abi_encode()`, `authorization: None`, and `gas` as computed below.

Batch gas is computed from the encoded calldata rather than estimated onchain, so batching adds no RPC round-trip:

```text
batch_gas = 26_000                                  // 21_000 intrinsic + array decode
          + 16 * abi_encode(execute(calls)).len()   // conservative calldata cost
          + Σ (call.gas + 5_000 + call.gas / 63)    // callee gas, per-call executor
                                                    // overhead, and 63/64 headroom
```

The `call.gas / 63` term covers EIP-150: a call only receives `min(gasLimit, 63/64 × remaining)`, so without headroom the last calls in a large batch would be under-gassed. Since Phase 1, under-gassing is not silent: `execute` refuses to make a call it cannot fully fund and reverts with `InsufficientGas(index)`, so a formula that under-estimates costs the batch loudly rather than dropping actions.

This makes the formula's per-call terms load-bearing against a known onchain floor. The guard requires `gasleft() × 63/64 ≥ call.gas`, i.e. `gasleft() ≥ call.gas × 64/63`, and the formula budgets `call.gas + call.gas / 63 + 5_000` per call, which satisfies it. The `/63` term must therefore not be dropped: it is exactly what the guard checks.

The `5_000` is what closes the guard's residual window, so it carries more weight than a safety margin. It has to cover both the executor's real per-call overhead (loop, guard, `CALL` base cost, and the `CallFailed` path) and the ~2,560 gas of `CALL` base cost that the onchain check deliberately ignores. Phase 6 should measure that overhead against real action calldata rather than assume it, since `execute` is compiled unoptimized and larger per-call calldata expands memory more than a cold account access alone. The `5_000` per-call overhead and the `26_000` base are to be confirmed against the gas assertion added in the contract optimization PR; if EIP-7623's calldata floor cost turns out to dominate for large batches, the calldata term is raised to `max(standard, floor)`.

Unit tests for `batch` cover: a single transaction (unbatched), several transactions under the limit (one batch), splitting at the gas limit, splitting on an `expires_at` change, order preservation across splits, an oversized transaction passed through unbatched, a non-zero-value transaction passed through unbatched, a delegation transaction passed through unbatched, and round-tripping the encoded calldata back through `executeCall::abi_decode` to assert the exact `to`/`gasLimit`/`data` of every call.

### Wiring batching into the queue

```rust
pub async fn queue(&mut self, transactions: ...) -> Result<(), Error> {
    let transactions = match self.config.executor {
        Some(_) => executor::batch(self.signer.address(), transactions, self.config.max_batch_gas),
        None => transactions.into_iter().collect(),
    };
    self.storage.enqueue(transactions).await?;
    ...
}
```

Note that `batch`'s `sender` is the signer's own address — the delegated EOA batches are sent to — not the executor implementation address, which only ever appears as an authorization target.

### Metrics and logging

`crates/core/src/metrics.rs` gains a histogram of calls per submitted batch (the `metrics` crate's `Histogram` is already used this way in `crates/sentinel/src/metrics.rs`), so operators can see whether batching is actually coalescing anything. `queue()` logs at `debug` when it emits a batch (call count and batch gas), and at `warn` for the pass-through cases in rules 3 and 4 above. `queue_delegation` logs at `info` when it enqueues a delegation and at `debug` when one is already outstanding.

### Integration coverage

`scripts/lib/shared_test_scripts.sh` gains a helper that deploys the executor via `DeploySafenet7702ExecutorScript` and returns its address, and `print_validator_config_base` gains an optional executor argument that emits the `[transactions]` table. `scripts/run_validator_integration_test.sh` runs one of its two validators with `executor` set and the other without, so a single CI run exercises both the batched and unbatched paths against real contracts, and asserts that the batched validator's mined transactions target its own EOA. This depends on Anvil running a Prague-or-later hardfork under Foundry 1.5.1 — to be confirmed in that phase, and made explicit with `--hardfork` if it is not the default.

---

## Implementation Phases

Each numbered phase is a separate PR. PRs have a single purpose, target fewer than 300 changed lines and fewer than ten files, and do not mix the rename with logic changes. This specification is its own plan-only PR containing no implementation.

The contracts track (Phase 1) and the Rust track (Phases 2–7) are independent and can be developed in parallel; they converge at Phase 8.

The Rust phases are deliberately ordered so that batching is wired **after** the delegation path works. Enabling batching first would leave a window in which an operator setting `executor` on `main` gets self-calls to an undelegated EOA, silently discarding actions.

### Phase 1 — Rename and gas-optimize the executor contract

Three small changes to the same three files, kept in one PR at the author's request:

- **Rename:** `contracts/src/Validator7702Account.sol` → `Safenet7702Executor.sol` with the contract, its natspec title and its doc comments retitled; the test and deploy script renamed to match (`Safenet7702Executor.t.sol`, `DeploySafenet7702Executor.s.sol`, `DeploySafenet7702ExecutorScript`). Add the missing `contracts-deploy-safenet-7702-executor` front door to the root `Justfile`.
- **Gas:** bind each `Call` to a single calldata pointer in the `execute` loop body, and add a gas assertion pinning `execute`'s cost for a fixed multi-call batch. Record the before/after numbers in the PR description.
- **Gas guard:** refuse to make a call the transaction cannot fund in full, reverting with `InsufficientGas(index)`, so an underfunded batch fails loudly instead of silently discarding calls and poisoning `eth_estimateGas` (see the contract section above for the measurements). Tests cover the guard firing at index 0, and firing mid-batch with the earlier call's effect rolled back.

Keep the rename as its own commit so its diff stays reviewable, since a whole-file rename plus an edit is otherwise hard to read.

Expected files: `contracts/src/Safenet7702Executor.sol`, `contracts/test/Safenet7702Executor.t.sol`, `contracts/script/DeploySafenet7702Executor.s.sol`, `Justfile`.

### Phase 2 — Add the `executor` and `max_batch_gas` config parameters

Config-only, no behavior: add both fields to `tx::Config` with their defaults, extend the existing `Config` deserialization tests in both services, and document the new keys in both sample TOMLs' existing `[transactions]` tables (both are asserted loadable by a `parses_sample_config` test).

Expected files: `crates/core/src/tx/mod.rs`, `crates/validator/src/config.rs`, `crates/validator/validator.sample.toml`, `crates/sentinel/src/config.rs`, `crates/sentinel/sentinel.sample.toml`.

### Phase 3 — Sign EIP-7702 `SetCode` transactions

Add `Transaction::authorization`, the `UnsignedTransaction` enum, `AllocatedTransaction::bumped_fees`, and authorization signing in `Signer`. Rearrange `submit_transaction` to record its `Submission` before building. Behaviorally inert: nothing sets `authorization` yet, so the existing queue tests must pass unchanged, and a new test asserts a delegation transaction signs into a type-4 transaction whose authorization recovers to the signer with nonce `n + 1`.

Expected files: `crates/core/src/tx/types.rs`, `crates/core/src/tx/signer.rs`, `crates/core/src/tx/mod.rs`.

### Phase 4 — Reserve two nonces for delegation transactions

Make nonce allocation span-aware in `storage.rs`, add `pending_delegation`/`has_delegation`, and add the `error`-level log in `update_block_status` for an authorization that did not apply. Also behaviorally inert while nothing sets `authorization`. Storage tests cover: allocation skips the reserved nonce after a delegation, allocation is unchanged without one, and a nonce consumed outside the queue still wins over the reservation. Depends on Phase 3.

Expected files: `crates/core/src/tx/storage.rs`, `crates/core/src/tx/mod.rs`.

### Phase 5 — Enqueue the delegation on service start

Add `TransactionQueue::queue_delegation` and call it from `Driver::new` when an executor is configured, together with the `eth_getCode(executor)` sanity check. This is the PR that makes a configured service actually delegate; batching is still off, so the only observable change is one extra transaction on startup. Queue tests cover: the delegation takes the lowest nonce, a following transaction is allocated `n + 2`, the enqueue is idempotent across a simulated restart with the delegation still in flight, and a configured executor with no code fails startup. Depends on Phases 3 and 4.

Expected files: `crates/core/src/tx/mod.rs`, `crates/core/src/driver.rs`.

### Phase 6 — Add the batch builder

Add `crates/core/src/tx/executor.rs` with the executor `sol!` binding, `batch`, and the gas formula, fully unit-tested as a pure function. No wiring: nothing calls it yet, so this PR cannot change runtime behavior. Independent of Phases 3–5 and can be developed in parallel with them.

Expected files: `crates/core/src/tx/executor.rs`, `crates/core/src/tx/mod.rs` (module declaration).

### Phase 7 — Wire batching into the transaction queue

Call `executor::batch` from `queue()` when an executor is configured, and add the batch-size metric. Queue-level tests assert that an unconfigured queue behaves exactly as before, and that a configured one submits a single transaction for several queued actions, addressed to the signer's own account, decoding back to the expected calls. Depends on Phases 2, 5 and 6.

Expected files: `crates/core/src/tx/mod.rs`, `crates/core/src/metrics.rs`.

### Phase 8 — Exercise the batched path in the validator integration test

Deploy the executor in the integration-test setup, configure one of the two validators to use it, and assert its transactions are self-directed batches. Confirm Anvil's hardfork supports EIP-7702 and pin it explicitly if needed. Depends on Phases 1 and 7.

Expected files: `scripts/lib/shared_test_scripts.sh`, `scripts/run_validator_integration_test.sh`.

### Phase 9 — Operator documentation

Document batching for operators: what `executor`/`max_batch_gas` do, the delegation transaction submitted on every start and its gas cost, the fact that no action executes before it lands, and how to read the batch-size metric. Optionally extend the devnet (`scripts/run_devnet.sh`, `docs/devnet.md`) to run with batching enabled. Depends on Phase 7.

Expected files: `docs/validator-handbook.md`, `docs/sentinel-handbook.md`, `docs/configuration.md`.

### Phase 10 — Remove this specification

Delete `epics/2026_09_09_safenet_7702_executor_tx_batching.md` once Phases 1–9 have landed.

---

## Open Questions and Assumptions

- **Redundant delegation per restart.** The startup enqueue is unconditional and idempotent only against a delegation still in storage, so a service that restarts after its delegation has executed and been pruned submits another one (roughly 34,000 gas, plus a one-transaction delay before the first action). This is deliberate: it is self-healing and needs no RPC read or persisted flag. A single `eth_getCode(signer)` short-circuit remains available if the cost ever matters.
- **Config key naming.** The plan uses `executor` and `max_batch_gas` under `[transactions]`, where the table already provides the namespace. `safenet_executor` was considered to echo the contract name; worth settling in Phase 2 before the key is public.
- **The executor code check.** Phase 5 adds one `eth_getCode(executor)` at startup, because delegating to a codeless address silently discards every batched action and a typo in a config file is a realistic way to get there. If that RPC request is unwanted too, the alternative is to accept the misconfiguration risk and rely on the integration test plus documentation.
- **Batch expiry policy.** The plan starts a new batch on any `expires_at` change so a batch can never drop a still-valid action. This is the safe choice but produces smaller batches when a single driver update mixes deadlines (for example a `None`-expiry `Preprocess` alongside a 6-block signing action). If measured batch sizes turn out to be disappointing, the alternative is minimum-expiry grouping with the drop risk accepted and documented.
- **Gas constants.** The `26_000` base and `5_000` per-call overhead in the batch gas formula are estimates. Phase 1's gas assertion should produce real numbers, and Phase 6 should adopt them. Similarly, the `max_batch_gas` default of `2_000_000` (roughly six to eight typical 250,000-gas actions) is a starting point, not a measured optimum.
- **EIP-7623 calldata floor.** For large batches the floor cost (`21_000 + 10 × tokens`) may exceed the standard calldata cost. The formula should be raised to `max(standard, floor)` if Phase 6's tests show the standard term under-estimating.
- **Swallowed inner failures.** `execute` is best-effort for calls that genuinely revert: a failing call emits `CallFailed` and the batch still succeeds, so the queue marks it executed. This matches today's behavior in the sense that a reverting standalone transaction also does not get retried by the queue — the state machine's timeouts drive retries either way — but it does mean a batch's `CallFailed` events are the only signal that an action did not take effect. Surfacing them (the services do not currently watch their own EOA for logs) is deliberately out of scope; flagging it as a known observability gap. Note that the _gas_ half of this problem is no longer swallowed: since Phase 1, a call the transaction cannot fund reverts the batch rather than emitting `CallFailed`, so "the batch was under-gassed" and "the callee reverted" are now distinguishable.
- **The gas guard's residual window.** Phase 1's check is intentionally approximate: it applies the 63/64 rule but not the `CALL`'s own base cost, so a call needing within roughly 2,560 gas of its full reservation can still be truncated silently. Hardcoding that cost onchain was rejected because hardforks reprice it. The window is therefore closed offchain by the batch gas formula's per-call overhead term, which makes that term correctness-relevant rather than a margin — see the Phase 6 note above. An action whose `gas` estimate is exactly its real cost, with no headroom of its own, is the case to watch.
- **Assumption: the signer's key is not used outside the service.** Nonce ordering is what keeps batches from executing before the delegation, and it is what makes the two-nonce reservation sound. A second process signing with the same key breaks both, as it already breaks the queue's existing nonce accounting.
- **Assumption: no value-bearing actions.** Every action both services encode today sets `value: U256::ZERO`. Batching passes non-zero-value transactions through unbatched rather than adding a `value` field to the executor's `Call`.
- **Assumption: Anvil supports EIP-7702 under Foundry 1.5.1.** The Solidity test already uses `vm.signAndAttachDelegation`, so the EVM supports it; whether the integration test's Anvil defaults to a Prague-or-later hardfork is to be confirmed in Phase 8.
- **Out of scope: submission-time batching.** Coalescing transactions queued across separate driver updates is the larger prize and is explicitly deferred (see the Architecture Decision). If Phase 8 shows typical batches contain only one or two calls, that is the follow-up epic.
