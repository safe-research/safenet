# Plan: Safenet 7702 executor transaction batching

Component: `crates/core` (Cargo package `safenet-core`), mainly the `tx` module (transaction queue, its SQLite storage, config and signer); the `Validator7702Account` contract in `contracts/`, which becomes `Safenet7702Executor` behind a new `ISafenet7702Executor` interface; the `[transactions]` config of the `validator` and `sentinel` services; and a new 7702 integration test in `scripts/`.

---

## Overview

`contracts/src/Validator7702Account.sol` is an EIP-7702 delegation target that batches calls for a service EOA, but nothing offchain uses it yet. Each action a service emits is submitted as its own EIP-1559 transaction and uses one nonce. So a block that produces several actions, such as a `Logs` update covering several signing rounds, becomes several transactions sent one after another. Each pays its own 21,000 intrinsic gas and uses up part of the queue's `max_in_flight_transactions` budget.

This epic lets the transaction queue send a service's actions as `ISafenet7702Executor.execute(Call[])` self-calls. Delegation is handled on demand by attaching an EIP-7702 authorization to whichever transaction needs it, rather than by a startup `SetCode` transaction.

This design supersedes the `feat/batex_*` branches. Two of their phases are kept as-is (the contract and most of the config); everything from their Phase 3 onward is replaced:

1. **Contract** — take `fix/batex_1` unchanged (rename to `Safenet7702Executor`, calldata-pointer gas optimization, the best-effort `InsufficientGas` guard), then add a `value` to each `Call` and an `ISafenet7702Executor` interface. The services depend on this interface, not on the concrete contract.
2. **Config** — take `fix/batex_2`'s TOML surface unchanged, but group `executor` and `max_batch_gas` into one `Option<ExecutorConfig>`, deserialized through a private flat `RawConfig`, so that `max_batch_gas` without `executor` is a config error.
3. **Storage** — track authorizations in the transaction storage: a new `nonces` table that owns nonce allocation, a new `authorizations` table, and a `transactions.authorization_id` column. Transactions are enqueued as a new `EnqueueTransaction` struct. If a transaction's nonce is consumed without its authorization's nonce, the storage recovers with a cancellation transaction.
4. **Unsigned transactions** — replace `TxEip1559` inside the `tx` module with a queue-owned `UnsignedTransaction`. The `Signer` then builds and signs either a `TxEip1559` or a `TxEip7702` (with a self-signed authorization at `nonce + 1`).
5. **Batching** — replace the nonce cache with an `eth_getProof`-backed account cache (`nonce` and `code_hash`). Add the batch encoder and the EIP-7702 delegation code hash helper. Then wire them into `TransactionQueue::queue`: batch when an executor is configured, and attach an authorization when the account's code hash is not the executor's delegation designator. Histogram metrics show how well the batching rules are coalescing.
6. **Integration test** — a 7702 integration test that emits three `TransactionProposed` events in a single block and asserts each validator's nonce goes up by exactly two (one batched nonce reveal, one batched signature share).
7. **Startup check** — `TransactionQueue::new` refuses to start with a configured executor that has no code.
8. **Cleanup** — remove this specification and the temporary test-network migration script.

---

## Architecture Decision

Batching and delegation live entirely inside `crates/core/src/tx`. The services' action encoders, state machines and the `Driver`'s command dispatch do not change: they keep producing one `(Transaction, Option<u64>)` per action through `ActionEncoder`, and the queue decides how those get onchain. Both the validator and the sentinel get the feature from the same code, and each action's `gas` estimate becomes its per-call `gasLimit` in the batch.

```text
 ActionEncoder::encode_action
        | (Transaction, Option<u64>)            unchanged public API
        v
 TransactionQueue::queue
        |-- executor configured?
        |     no  -> one EnqueueTransaction per action, no authorization
        |     yes -> executor::batch(account, txs, max_batch_gas)
        |              every call goes through `execute`, even a batch of one
        |            authorization := account.code_hash != Authorization::code_hash(executor)
        v
 TransactionStorage::enqueue(EnqueueTransaction { transaction, authorization, expires_at })
        |   transactions ──authorization_id──> authorizations
        v
 next_transaction: allocates N to the transaction, N + 1 to its authorization (nonces table)
        v
 AllocatedTransaction { nonce, transaction, authorization, fees } -> build -> UnsignedTransaction
        v
 Signer::sign_transaction -> TxEip1559 | TxEip7702 { authorization_list: [sign(auth @ nonce + 1)] }
```

### The authorization rides on the transaction that needs it

Under EIP-7702 the authorization list is processed before the transaction's call runs. So a batch that carries its own authorization always runs against delegated code, whatever happened before it. The queue attaches an authorization to a batch whenever the signer account's current code hash (from the account cache) is not `keccak256(0xef0100 ‖ executor)`. Once the delegation is observed onchain, later batches stop carrying one.

Compared with the `feat/batex_*` design (a standalone `SetCode` transaction enqueued at startup and gated by nonce ordering), this:

- needs no startup step and no idempotency bookkeeping across restarts;
- repairs itself: if the account is ever undelegated or re-delegated elsewhere, the next batch re-delegates it;
- makes every batch that carries an authorization correct on its own, rather than correct only because some earlier transaction landed first.

The cost: until the delegation is observed, every batch enqueued pays for an authorization (the `PER_EMPTY_ACCOUNT_COST` of 25,000 gas plus a second nonce). In practice that is the batches queued in the first block or two after the executor is first configured.

### Authorizations own a nonce too

A self-sponsored authorization must name `nonce + 1`, because the sender's nonce is incremented before the authorization list is processed, and applying it increments the nonce again. So a transaction at `N` with an authorization leaves the account at `N + 2`. The storage models this directly instead of inferring it from a JSON flag:

- `nonces` is the single source of truth for allocated nonces. Each row points at a `transaction_id`, an `authorization_id`, or both.
- Allocation gives `N` to the transaction and `N + 1` to its authorization in one SQLite transaction, and the next free nonce is simply `MAX(status.nonce, MAX(nonces.nonce) + 1)`.
- An authorization has no `expires_at`, `submitted_at` or `executed_at` of its own. It is always part of exactly one transaction, and those facts are found by `JOIN`ing to it.

### Recovering from a consumed transaction nonce with an unused authorization nonce

If the account's onchain nonce equals a nonce held only by an authorization (`transaction_id IS NULL`), then the owning transaction's nonce `N` was consumed but the authorization's `N + 1` was not. That can only happen if the EOA was used outside the queue and replaced the transaction at `N`. Every nonce the queue allocated above `N + 1` is now stuck behind a permanent gap.

The storage recovers by inserting a cancellation transaction (`Transaction::default()`: a call to `address(0)` with no value and no data, 21,000 gas) and attaching it to that same nonce row, which then has both a `transaction_id` and an `authorization_id`. Because it is allocated but never submitted, the existing `stale_submissions` query picks it up and `resubmit_stale` broadcasts it in the same pass. This does not depend on the in-flight budget, which may already be exhausted by the stuck transactions above the gap.

This is reorg-safe. If a reorg means the original transaction and its authorization do execute after all, nonce `N + 1` is consumed by the authorization. The account nonce then moves past it, and `mark_executed` marks the cancellation executed like any other transaction whose nonce was passed.

### Every call goes through the executor

When an executor is configured, even a single queued transaction is wrapped in a one-call `execute` batch. This keeps one code path and gives the executor a single place for extra accounting or custom onchain logic later. It also means an operator can see from the chain alone that batching is active. There are no exceptions: `Call` carries a `value`, so value-bearing transactions are batched like any other.

### Batch boundaries preserve order and expiry

Carried over from `feat/batex_*`. Batching walks transactions in the order the driver produced them. It starts a new batch when the accumulated gas would exceed `max_batch_gas` or when `expires_at` changes. Order matters: the sentinel emits `ApproveToken` before `Commit`, batches execute in nonce order, and calls within a batch execute in array order. Splitting on any `expires_at` change means a batch never drops a still-valid action because another action in it expired.

These rules are deliberately simple and conservative, and they may split more than necessary. For example, the actions for distinct signing requests can carry different expiries even when they could safely share a batch. The batching metrics (see Tech Specs) measure how often that happens, so that smarter grouping, such as batching per independent "group" of actions whose relative onchain order does not matter, can be judged on data in a future epic. This epic does not attempt it.

### Alternatives Considered

- **Standalone startup `SetCode` transaction** (the `feat/batex_4` design). It needs a startup enqueue, restart idempotency, a `json_extract`-based two-nonce span in the allocation query, and the guarantee that no batch runs before delegation rests entirely on nonce ordering. Rejected in favor of per-transaction authorizations.
- **Keep `nonce` on `transactions` and infer the authorization span from JSON.** This is what `feat/batex_4` does. It has no row to attach a recovery transaction to, and it makes the allocation query depend on the most recently allocated row's JSON. Rejected: a `nonces` table represents "one transaction, two nonces" and "one nonce, two owners" directly.
- **Barrier while an authorization is in flight** (only the first batch carries it, later ones wait). This saves authorization gas while the first one is pending, but brings back the ordering dependency and stalls the pipeline. Rejected.
- **Decide the authorization at nonce allocation instead of at enqueue.** This would always use the freshest account state. Rejected: the storage schema stores the authorization on the enqueued row, and the enqueue-time decision is cheap because the account cache is already warm for the block.
- **Unbatched single transactions** (send a batch of one as the original transaction). This saves the executor overhead for lone actions. Rejected per the section above.
- **`#[serde(flatten)] Option<ExecutorConfig>`.** Verified: serde's flattened `Option` turns any error in the inner struct into `None`. So `max_batch_gas = 3000000` without `executor` parses as "batching disabled", and so does a malformed `executor = 1`. `flatten` also does not combine reliably with `deny_unknown_fields`. Rejected in favor of `try_from` a flat `RawConfig` (see Tech Specs).

---

## Tech Specs

### Contract: `Safenet7702Executor` and `ISafenet7702Executor`

Phase 1a is `fix/batex_1` rebased onto `main`: rename `Validator7702Account` to `Safenet7702Executor` (source, test, deploy script, plus the `contracts-deploy-safenet-7702-executor` recipe), bind each `Call` to one calldata pointer, add the `InsufficientGas(index)` guard (`gasleft() * 63 / 64 >= call.gasLimit`), and pin `execute`'s gas cost for a fixed four-call batch. The over-simplification in the gas check (it ignores the `CALL` base cost) is an accepted tradeoff. The offchain batch gas formula covers it.

Phase 1b adds a `value` to `Call` and forwards it: `call.to.call{gas: call.gasLimit, value: call.value}(call.data)`. `execute` stays non-payable. The batch transaction itself sends no value, so a value-bearing call spends the delegated account's own balance. A call whose value exceeds that balance fails like any other call: it emits `CallFailed` and the batch carries on. This is a logic change, so the pinned gas snapshot is updated deliberately. Tests cover value forwarded to the target and an unaffordable value failing without reverting the batch.

Phase 1c adds `contracts/src/interfaces/ISafenet7702Executor.sol`. `Safenet7702Executor` implements it, and the Rust side binds to it, not to the concrete contract:

```solidity
interface ISafenet7702Executor {
    struct Call {
        address to;
        uint256 value;
        uint256 gasLimit;
        bytes data;
    }

    function execute(Call[] calldata calls) external;
}
```

The interface's natspec is the contract the Safenet services rely on, so any executor that honors it can be configured:

- `execute` must only accept `msg.sender == address(this)`, meaning the delegating EOA calling itself.
- Calls run in array order, each forwarded exactly its `value` and `gasLimit`.
- A failing call must not revert the batch.
- An underfunded call must revert the whole batch rather than be silently truncated.
- The executor must be usable as an EIP-7702 delegation target: no constructor-initialized storage and no initializer.

`OnlySelf`, `InsufficientGas` and the `CallFailed` event stay on the implementation. The errors are how this executor fulfils the contract, not part of it. `CallFailed` exists only to help debugging, and no service depends on it. `Call` moves to the interface, so tests refer to `ISafenet7702Executor.Call`. Adding the interface is not a logic change, so the gas snapshot as updated in 1b must not move.

### Config

`tx::Config` and a new `ExecutorConfig` move to a new `crates/core/src/tx/config.rs` module, re-exported as `pub use self::config::{Config, ExecutorConfig}` from `tx`. `Config` gains one grouped field:

```rust
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(try_from = "RawConfig")]
pub struct Config {
    pub max_in_flight_transactions: usize,
    pub blocks_before_resubmit: u64,
    pub priority_fee_cap_percentage: Option<f64>,
    /// EIP-7702 batching through an `ISafenet7702Executor`. `None` submits
    /// one transaction per queued transaction.
    pub executor: Option<ExecutorConfig>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExecutorConfig {
    /// The `ISafenet7702Executor` the signer account delegates to.
    pub address: Address,
    /// The maximum gas a single batch may consume (default 2_000_000). A
    /// transaction that does not fit on its own gets a one-call batch, so `0`
    /// sends every transaction through the executor without batching.
    pub max_batch_gas: u64,
}
```

`Config` gets its `Deserialize` through `#[serde(try_from = "RawConfig")]`. `RawConfig` is private and mirrors the flat TOML table: `#[serde(default, deny_unknown_fields)]`, the three existing fields, plus `executor: Option<Address>` and `max_batch_gas: Option<u64>`. Its `Default` is derived from `Config::default()` so the defaults live in one place. `TryFrom<RawConfig> for Config` applies the grouping rules:

- `(None, None)` becomes `None`;
- `(None, Some(_))` is an error, "`max_batch_gas` requires `executor`";
- `(Some(address), gas)` becomes `Some(ExecutorConfig { address, max_batch_gas: gas.unwrap_or(2_000_000) })`.

Because `RawConfig` is a plain flat struct with no `flatten`, `deny_unknown_fields` keeps working, and a malformed `executor` value is an ordinary type error instead of silently disabling batching.

The TOML surface is identical to `fix/batex_2`: `executor` and `max_batch_gas` keys in the existing `[transactions]` table, documented in both sample TOMLs. The new `config` module has no test module of its own. The cases are covered by extending the existing config tests in `crates/validator/src/config.rs` and `crates/sentinel/src/config.rs`:

- both keys set;
- `executor` only (default gas);
- neither key (disabled);
- `max_batch_gas` alone (rejected);
- a malformed `executor` (rejected);
- an unknown key in `[transactions]` (still rejected).

### Storage schema

`crates/core/src/tx/storage.rs`, final shape (introduced in full in Phase 3b):

```sql
CREATE TABLE authorizations (
    id      INTEGER PRIMARY KEY,
    address TEXT    NOT NULL                 -- the delegate
);
CREATE TABLE transactions (
    id               INTEGER PRIMARY KEY,
    request          TEXT    NOT NULL,
    authorization_id INTEGER DEFAULT NULL UNIQUE REFERENCES authorizations (id),
    expires_at       INTEGER DEFAULT NULL,
    submitted_at     INTEGER DEFAULT NULL,
    executed_at      INTEGER DEFAULT NULL
);
CREATE TABLE nonces (
    nonce            INTEGER PRIMARY KEY,
    transaction_id   INTEGER DEFAULT NULL UNIQUE REFERENCES transactions (id),
    authorization_id INTEGER DEFAULT NULL UNIQUE REFERENCES authorizations (id),
    CHECK (transaction_id IS NOT NULL OR authorization_id IS NOT NULL)
);
```

Queries move from `transactions.nonce` to `JOIN nonces`:

- **Unallocated:** a transaction with no `nonces` row.
- **In flight:** a transaction with a `nonces` row and `executed_at IS NULL`.
- **`next_transaction`:** picks the oldest unexpired unallocated transaction, inserts `(N, transaction_id, NULL)`, and, if the transaction has an `authorization_id`, also inserts `(N + 1, NULL, authorization_id)`. All of this happens in one SQLite transaction.
- **`record_submission`:** resolves the transaction via `nonces.transaction_id`.
- **`mark_executed`:** marks transactions whose own nonce is below the account nonce. An authorization's nonce plays no part: the transaction executed as soon as its own nonce was consumed.
- **`prune`:** removes a nonce row only once neither its transaction nor its authorization's owning transaction survives. It clears a surviving row's `authorization_id` before deleting the authorization (the cancellation case), and deletes an authorization together with its transaction.

sqlx enables `PRAGMA foreign_keys` by default, so these references are enforced.

**Migration.** Safenet has not been released and does not run in production, so there is no in-app migration. `TransactionStorage::new` only changes its `CREATE TABLE IF NOT EXISTS` definitions, and a recreated database gets the new schema. It adds no schema probes, no `ALTER TABLE` calls and no migration execution.

For the test network, whose existing database has a `transactions.nonce` column, Phase 3b adds a temporary `migrations/2026_09_23_safenet_7702_executor_tx_batching.sql`. An operator applies it manually with `sqlite3 <database> < migrations/…sql` while the service is stopped. The application never discovers or runs it. It follows the earlier scheduled-pruning migration (removed in `80951a0`): `.bail on`, a single `BEGIN … COMMIT`, and header comments that give its scope, how to check whether it has already been applied (`nonce` present in `pragma_table_info('transactions')`), and how to apply it. It:

1. creates `authorizations` and `nonces`;
2. copies `(nonce, id)` into `nonces` for rows with a non-null nonce;
3. runs `ALTER TABLE transactions ADD COLUMN authorization_id …` and `DROP COLUMN nonce`.

The script is checked by hand against a copy of a pre-change database before Phase 3b merges. It has no automated test and is removed in Phase 8.

### Types

`crates/core/src/tx/types.rs`:

```rust
/// An EIP-7702 delegation to authorize alongside a transaction.
pub struct Authorization {
    /// The delegate the signer account authorizes.
    pub address: Address,
}

impl Authorization {
    /// The code hash of an account delegated to `address`:
    /// `keccak256(0xef0100 ‖ address)`.
    pub fn code_hash(&self) -> B256;
}

/// A transaction to enqueue.
pub struct EnqueueTransaction {
    pub transaction: Transaction,
    pub authorization: Option<Authorization>,
    pub expires_at: Option<u64>,
}

pub struct AllocatedTransaction {
    pub nonce: u64,
    pub transaction: Transaction,
    pub authorization: Option<Authorization>, // populated for non-null authorization_id
    pub max_fee_per_gas: Option<u128>,
    pub max_priority_fee_per_gas: Option<u128>,
}

/// An unsigned transaction, as built by the queue for signing.
pub struct UnsignedTransaction {
    pub chain_id: u64,
    pub nonce: u64,
    pub gas_limit: u64,
    pub max_fee_per_gas: u128,
    pub max_priority_fee_per_gas: u128,
    pub to: Address,
    pub value: U256,
    pub input: Bytes,
    pub authorization: Option<Authorization>,
}
```

`EnqueueTransaction` replaces the `(Transaction, Option<u64>)` tuple at the storage boundary (`TransactionStorage::enqueue`). The public `TransactionQueue::queue` and `ActionEncoder` signatures keep their tuples: which authorization to attach is decided inside the queue, and changing them would touch every action encoder in both services for no benefit.

Until Phase 4a, `AllocatedTransaction::build` returns a `TxEip1559` and panics with `todo!()` for `Some(authorization)`. Nothing sets one before Phase 5d. From Phase 4a it returns an `UnsignedTransaction`, and the `todo!()` moves to the `UnsignedTransaction` → `TxEip1559` conversion that `submit_transaction` uses until Phase 4b. Phase 4b removes it.

### Signer

`Signer::sign_transaction(&self, tx: UnsignedTransaction)` builds a `TxEip1559` when `authorization` is `None`. Otherwise it builds a `TxEip7702` whose single `authorization_list` entry is the signer's signature over `alloy::eips::eip7702::Authorization { chain_id, address, nonce: tx.nonce.checked_add(1)? }`; overflow is a `SigningError`. Tests assert that both shapes round-trip through `decode_2718_exact`, recover to the signer, and (for 7702) carry an authorization that recovers to the signer, with nonce `n + 1` and the transaction's chain ID.

### Account cache

`TransactionQueue::nonce_cache: Option<u64>` becomes `account_cache: Option<AccountStatus>`:

```rust
pub struct AccountStatus {
    pub nonce: u64,
    pub code_hash: B256,
}
```

It is fetched with `eth_getProof(signer, [], latest)`, one request just like today's `eth_getTransactionCount`, and invalidated on the same block status changes. Existing callers use `account().await?.nonce`. The queue tests' `U64` nonce responses are replaced by a small `account_proof(nonce, code_hash)` helper, so the test churn is mechanical.

### Batch encoding

New module `crates/core/src/tx/executor.rs`, with a `sol!` transcription of `ISafenet7702Executor` (following the repo's existing inline `sol!` bindings) and one pure function:

```rust
/// Groups `transactions` into `ISafenet7702Executor.execute` self-calls sent to
/// `account`, so that no batch exceeds `max_batch_gas`.
pub fn batch(
    account: Address,
    transactions: impl IntoIterator<Item = (Transaction, Option<u64>)>,
    max_batch_gas: u64,
) -> Vec<(Transaction, Option<u64>)>;
```

`account` is the signer's own address, not the executor's. Sending `execute` calldata to the executor implementation instead would silently discard every action, so the parameter is named to make the two hard to confuse.

Rules:

1. Start a new batch when `expires_at` differs from the current batch's, or when adding the transaction would push the batch gas above `max_batch_gas`.
2. Every batch goes through `execute`, including a batch of one.
3. A transaction whose own gas exceeds `max_batch_gas` becomes a one-call batch (over the limit) and is never dropped. This is normal operation and logs at `debug`, not `warn`. It also means `max_batch_gas = 0` sends every transaction as its own one-call batch, for operators who want the executor (for onchain accounting, say) without batching.
4. A batch is `Transaction { to: account, value: 0, data: executeCall { calls }.abi_encode(), gas }`, where each call is `Call { to, value, gasLimit: gas, data }` from the original transaction. A transaction's `value` travels in its `Call` and is paid from the account's own balance.

The batch gas is computed from the encoded calldata, so it needs no RPC request. The formula is carried over from `feat/batex_*`:

```text
gas = 26_000                                  // intrinsic + array decode
    + 16 * abi_encode(execute(calls)).len()   // conservative calldata cost
    + Σ (call.gas + call.gas / 63 + 5_000)    // callee gas, 63/64 headroom, per-call overhead
    + Σ (call.value != 0 ? 34_000 : 0)        // CALL value transfer + possible account creation
```

The `call.gas / 63` term is exactly what the `InsufficientGas` guard checks, so it must not be dropped. The `5_000` covers the executor's per-call overhead plus the `CALL` base cost the onchain check ignores. A value-bearing `CALL` also pays 9,000 for the value transfer and, if the target account is empty, 25,000 to create it. The EVM deducts both before the 63/64 rule applies, and the onchain guard does not model them, so the formula budgets them on top of the call's own `gasLimit`. Phase 5b confirms these constants against the gas snapshot pinned in Phase 1a and updated in 1b.

Unit tests cover:

- one transaction (still batched);
- several under the limit (one batch);
- a split at the gas limit;
- a split on an `expires_at` change;
- order preserved across splits;
- an oversized transaction (its own batch);
- `max_batch_gas = 0` (one batch per transaction);
- a value-bearing transaction (value carried in its `Call`, value-transfer gas added);
- decoding every batch back through `executeCall::abi_decode` to assert the exact calls.

### Wiring

```rust
pub async fn queue(&mut self, transactions: impl IntoIterator<Item = (Transaction, Option<u64>)>) -> Result<(), Error> {
    let transactions = match &self.config.executor {
        None => /* EnqueueTransaction { authorization: None, .. } for each */,
        Some(executor) => {
            let authorization = self.authorization(executor).await;
            executor::batch(self.signer.address(), transactions, executor.max_batch_gas)
                .into_iter()
                .map(|(transaction, expires_at)| /* each batch carries `authorization`,
                                                    gas += 25_000 when set */)
                .collect()
        }
    };
    self.storage.enqueue(transactions).await?;
    ...
}
```

`authorization(executor)` returns `None` when `account().code_hash == Authorization { address: executor.address }.code_hash()`, and `Some` otherwise. If the account status cannot be fetched, it returns `Some` and logs a warning, rather than failing `queue()` before the transactions are stored. The driver treats a `queue()` RPC error as intermittent and would otherwise drop those transactions. Attaching an authorization that turns out to be redundant is always safe; it just costs gas. The 25,000 is EIP-7702's `PER_EMPTY_ACCOUNT_COST`, added on top of the batch's computed gas and not counted against `max_batch_gas`.

`queue()` logs at `debug` for each batch (call count and gas), and at `info` when it attaches an authorization.

### Metrics

`crates/core/src/metrics.rs` gains two histograms, following the existing `metrics::histogram!` pattern in `crates/sentinel/src/metrics.rs`. Both are recorded in `TransactionQueue::queue` at enqueue time, so resubmissions are not counted again, and only when an executor is configured:

- `safenet_core_transaction_batch_size` — the number of calls in each batch, recorded once per batch.
- `safenet_core_transaction_batches_per_queue` — for each list of transactions handed to one `queue()` call, the number of batches it turned into, recorded once per call. A value above 1 when the list's total gas fits within `max_batch_gas` means the order- and expiry-preserving rules split it. That is the signal for judging future group-based batching.

### Integration coverage

New script `scripts/run_validator_7702_integration_test.sh`, Justfile recipe `test-integration-validator-7702`, and a matrix entry in `.github/workflows/integration.yml`. `scripts/lib/shared_test_scripts.sh` gains two things:

- a `deploy_safenet_7702_executor` helper, using `DeploySafenet7702ExecutorScript`;
- an optional executor argument on `print_validator_config_base`, which emits `executor` in the `[transactions]` table.

The test:

1. Deploys the contracts and the executor, and starts two validators with `executor` configured.
2. Triggers genesis keygen and waits until both validators are delegated (`cast code` equals `0xef0100 ‖ executor`) and their startup/preprocess transactions have been mined.
3. Records each validator's `cast nonce`.
4. Proposes three transactions (distinct Safe nonces) from the Anvil deployer account in one transaction: `cast send <deployer> 'execute((address,uint256,uint256,bytes)[])' … --auth <executor>`. Asserts that the three `TransactionProposed` logs share a block.
5. Waits for all three `TransactionAttested` events.
6. Asserts each validator's nonce advanced by exactly 2: one batch of three nonce reveals and one batch of three signature shares. Attestation happens through the `signShareWithCallback` callback, so there is no separate attestation transaction on the happy path.

---

## Implementation Phases

Each phase is a separate PR, targeting fewer than 300 changed lines and fewer than ten files. This specification is its own plan-only PR.

Parallel tracks:

- **Contracts** (1a → 1b → 1c) are independent of all Rust work until Phase 6.
- **Config** (2a → 2b) is independent and only needs to land before 5d.
- **Storage** (3a → 3b → 3c → 3d) then **unsigned transactions** (4a → 4b) form the main sequential track.
- **Account cache** (5a) and **batch encoder** (5b) are independent of the storage track and of each other. 5b's binding must match 1c's interface.
- **Code hash** (5c) needs only the `Authorization` type from 3c.
- **Wiring** (5d) joins every track. Tests (5e) and metrics (5f) follow it and are independent of each other, as is the integration test (6).
- **Startup check** (7) only needs 2b, but is scheduled last as a short follow-up.

Nothing is observable to operators until 5d. Before then, setting `executor` parses but has no effect, which the sample TOML comments added in Phase 2b must not contradict: they describe the behavior once it ships, and 5d is where it becomes true.

### Phase 1a — Rename, gas-optimize and guard the executor contract

Port `fix/batex_1` onto `main` unchanged in substance. Keep the rename as its own commit so the diff stays readable.

**Files:** `contracts/src/Safenet7702Executor.sol`, `contracts/test/Safenet7702Executor.t.sol`, `contracts/script/DeploySafenet7702Executor.s.sol`, `Justfile`.

### Phase 1b — Add `value` to `Call`

Add `value` to `Call` and forward it in `execute`, as specified above. Update the natspec and the pinned gas snapshot, and add tests for value forwarding and for an unaffordable value failing without reverting the batch.

**Files:** `contracts/src/Safenet7702Executor.sol`, `contracts/test/Safenet7702Executor.t.sol`.

### Phase 1c — Add `ISafenet7702Executor`

Add the interface with the natspec contract above, move `Call` into it (`CallFailed` stays on the implementation), make `Safenet7702Executor` implement it, and update the tests to reference `ISafenet7702Executor.Call`. The pinned gas snapshot must not move.

**Files:** `contracts/src/interfaces/ISafenet7702Executor.sol` (new), `contracts/src/Safenet7702Executor.sol`, `contracts/test/Safenet7702Executor.t.sol`.

### Phase 2a — Move `tx::Config` into its own module

Pure move: `Config` and its `Default` go from `tx/mod.rs` to a new `tx/config.rs`, re-exported as `pub use self::config::Config`. No behavior or serde change.

**Files:** `crates/core/src/tx/config.rs` (new), `crates/core/src/tx/mod.rs`.

### Phase 2b — `executor` / `max_batch_gas` config

Add `ExecutorConfig`, `Config::executor`, the private `RawConfig` and its `TryFrom`, and switch `Config` to `#[serde(try_from = "RawConfig")]`. Re-export `ExecutorConfig`. Add the sample TOML keys exactly as in `fix/batex_2`. Extend both services' config tests with the cases above. Config only, no behavior.

**Files:** `crates/core/src/tx/config.rs`, `crates/core/src/tx/mod.rs`, `crates/validator/src/config.rs`, `crates/validator/validator.sample.toml`, `crates/sentinel/src/config.rs`, `crates/sentinel/sentinel.sample.toml`.

### Phase 3a — `EnqueueTransaction`

Pure refactor: introduce `types::EnqueueTransaction { transaction, expires_at }` and make `TransactionStorage::enqueue` take it. `TransactionQueue::queue` maps its tuples. The `authorization` field arrives in 3c, together with the schema that can store it. Storage tests are updated mechanically.

**Files:** `crates/core/src/tx/types.rs`, `crates/core/src/tx/storage.rs`, `crates/core/src/tx/mod.rs`.

### Phase 3b — `nonces` table and final schema

Introduce the full final schema (`nonces`, `authorizations`, `transactions.authorization_id`) in one PR, so that the test network needs only one manual migration. Move every nonce query onto `nonces`. `authorizations` and `authorization_id` exist but stay unused. Add the temporary manual migration script for the test network. Behavior-preserving: the existing storage and queue tests pass unchanged.

**Files:** `crates/core/src/tx/storage.rs`, `migrations/2026_09_23_safenet_7702_executor_tx_batching.sql` (new).

### Phase 3c — Enqueue and allocate authorizations

Add `types::Authorization` and `EnqueueTransaction::authorization`, and have `enqueue` insert `authorizations` rows. `next_transaction` allocates `N + 1` to the authorization, and `AllocatedTransaction::authorization` is populated from the join. `prune` handles authorizations. `build` panics with `todo!()` for `Some(authorization)`.

Storage tests cover:

- a transaction with an authorization takes `N` and the next transaction takes `N + 2`;
- allocation without authorizations is unchanged;
- a nonce consumed outside the queue still wins over the reservation;
- `AllocatedTransaction::authorization` round-trips;
- pruning removes the transaction, authorization and both nonce rows.

**Files:** `crates/core/src/tx/types.rs`, `crates/core/src/tx/storage.rs`, `crates/core/src/tx/mod.rs` (the tuple mapping sets `authorization: None`).

### Phase 3d — Recover an unused authorization nonce

Add a storage method, called from `update_block_status` right after `mark_executed`. If the account nonce is held by an authorization-only `nonces` row, it inserts a cancellation `Transaction::default()` and attaches it to that row.

Storage tests cover:

- the cancellation is created exactly once and returned by `stale_submissions`;
- it is marked executed when the nonce moves past it, both when it lands and when a reorg lets the original authorization land instead;
- pruning with a pending cancellation keeps its nonce row.

A queue test covers recovery with the in-flight budget already full.

**Files:** `crates/core/src/tx/storage.rs`, `crates/core/src/tx/mod.rs`.

### Phase 4a — `UnsignedTransaction`

Add `types::UnsignedTransaction`. `AllocatedTransaction::build` returns it, and `submit_transaction` reads the submission fees from it. Until 4b, a temporary conversion to `TxEip1559` feeds the unchanged `Signer` and holds the `todo!()` for authorizations. No behavior change.

**Files:** `crates/core/src/tx/types.rs`, `crates/core/src/tx/mod.rs`.

### Phase 4b — Sign `UnsignedTransaction`

Change `Signer::sign_transaction` to take `UnsignedTransaction`, building `TxEip1559` or `TxEip7702` with the self-signed authorization at `nonce.checked_add(1)`. Remove the temporary conversion and its `todo!()`. Signer tests cover both transaction types.

**Files:** `crates/core/src/tx/signer.rs`, `crates/core/src/tx/mod.rs`.

### Phase 5a — Account cache via `eth_getProof`

Replace `nonce_cache`/`nonce()` with `account_cache`/`account()` returning `AccountStatus`, and update the queue tests to use the `account_proof` helper. No behavior change beyond the RPC method. Independent of Phases 3 and 4.

**Files:** `crates/core/src/tx/mod.rs` (plus `types.rs` if `AccountStatus` lives there).

### Phase 5b — Batch encoder

Add `tx/executor.rs` with the `ISafenet7702Executor` binding, `batch`, the gas formula and its unit tests. Nothing calls it yet. Confirm the formula's constants against the gas snapshot from Phases 1a and 1b, and record the numbers in the PR description.

**Files:** `crates/core/src/tx/executor.rs` (new), `crates/core/src/tx/mod.rs` (module declaration).

### Phase 5c — Delegation code hash

Add `Authorization::code_hash`, using alloy's EIP-7702 delegation designator constant. Test it against a known delegated account's code hash. Can be done in parallel with 5a and 5b once 3c has landed.

**Files:** `crates/core/src/tx/types.rs`.

### Phase 5d — Batch and authorize on enqueue

Wire `executor::batch` and the authorization decision into `TransactionQueue::queue` as specified above, including the fallback on RPC failure and the 25,000 authorization gas. Add a batching tip to the `[transactions]` sections of both handbooks.

Queue tests cover:

- an unconfigured queue behaves exactly as before;
- an undelegated account gets a type-4 transaction carrying an authorization at `n + 1`, and the next allocation skips a nonce;
- an account already delegated to the executor gets a plain type-2 self-call;
- an account delegated elsewhere is re-delegated;
- an account status fetch failure still enqueues, with an authorization.

**Files:** `crates/core/src/tx/mod.rs`, `docs/validator-handbook.md`, `docs/sentinel-handbook.md`.

### Phase 5e — Batching queue tests

Tests in `safenet_core::tx::tests` only:

- with batching enabled and a delegated account, queuing several transactions in one `queue()` call produces exactly one `eth_sendRawTransaction`, addressed to the signer's own account, whose calldata decodes to the queued calls in order;
- a set of transactions whose total gas exceeds `max_batch_gas` is split into the expected number of submitted batches, each within the limit.

**Files:** `crates/core/src/tx/mod.rs`.

### Phase 5f — Batching metrics

Add the `safenet_core_transaction_batch_size` and `safenet_core_transaction_batches_per_queue` histograms and record them in `queue()` as specified above.

**Files:** `crates/core/src/metrics.rs`, `crates/core/src/tx/mod.rs`.

### Phase 6 — 7702 integration test

Add the script, the shared helpers, the Justfile recipe and the CI matrix entry described above.

**Files:** `scripts/run_validator_7702_integration_test.sh` (new), `scripts/lib/shared_test_scripts.sh`, `Justfile`, `.github/workflows/integration.yml`, and the `AGENTS.md` integration test list.

### Phase 7 — Check the executor's code on startup

When an executor is configured, `TransactionQueue::new` fetches `eth_getCode(executor)` at `latest` and fails with a new `Error::ExecutorWithoutCode(Address)` if the code is empty. Without this check, a mistyped `executor` means every `execute` self-call runs against empty code and succeeds as a no-op, silently dropping actions. An RPC failure here also fails startup, as the driver's other startup RPC requests already do.

Queue tests cover a configured executor with no code (startup fails) and with code (startup succeeds). Tests that build a queue with an executor configured (from 5d, 5e and 5f) push a code response first, through their shared constructor helper. Unconfigured queues make no extra request.

**Files:** `crates/core/src/tx/mod.rs`.

### Phase 8 — Remove the epic

Delete this specification and the temporary test-network migration script in their own `[7702 End]` PR. Also delete the superseded `feat/batex_*` and `fix/batex_*` remote branches, after confirming with their authors.

**Files:** `epics/2026_09_23_safenet_7702_executor_tx_batching.md`, `migrations/2026_09_23_safenet_7702_executor_tx_batching.sql`.

---

## Open Questions and Assumptions

### Open questions

None at the moment.

### Assumptions

- The configured RPC supports `eth_getProof` at `latest`. Geth, Nethermind, Erigon, Reth and Anvil all do, including on Gnosis Chain.
- Anvil under Foundry 1.5.1 runs a Prague-or-later hardfork by default. If not, Phase 6 passes `--hardfork prague` through `start_anvil`.
- Foundry 1.5.1's `cast send --auth` can delegate and call in one transaction for the integration test's batched proposal. If not, a small Forge script using `vm.signAndAttachDelegation` does the same.
- The integration test's nonce-delta window can be chosen to exclude `Preprocess` (nonce commitment) and genesis keygen transactions. Three signing requests do not exhaust a 1,024-signature nonce chunk, so no `Preprocess` should fall inside the window.
- The signer account is dedicated to the service. Using it outside the queue is handled (the Phase 3d recovery) but not supported: an action whose nonce was taken by an outside transaction is lost, exactly as it is today.
- `max_batch_gas` (default 2,000,000) stays well below the block gas limit of every target chain, so the 25,000 authorization gas added on top of it never matters for inclusion.
