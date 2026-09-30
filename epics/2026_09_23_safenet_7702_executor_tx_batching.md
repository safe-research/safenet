# Plan: Safenet 7702 executor transaction batching

Component: `crates/core` (Cargo package `safenet-core`), mainly the `tx` module (transaction queue, its SQLite storage, config and signer); the `Validator7702Account` contract in `contracts/`, which becomes `Safenet7702Executor` behind a new `ISafenet7702Executor` interface; the `[transactions]` config of the `validator` and `sentinel` services; and a new 7702 integration test in `scripts/`.

---

## Overview

`contracts/src/Validator7702Account.sol` is an EIP-7702 delegation target that batches calls for a service EOA, but nothing offchain uses it yet. Each action a service emits is submitted as its own EIP-1559 transaction and uses one nonce. So a block that produces several actions, such as a `Logs` update covering several signing rounds, becomes several transactions sent one after another. Each pays its own 21,000 intrinsic gas and uses up part of the queue's `max_in_flight_transactions` budget.

This epic lets the transaction queue send a service's queued transactions as `ISafenet7702Executor.execute(Call[])` self-calls. Batches are formed when a nonce is allocated, not when transactions are enqueued, and at most one batch is in flight at a time. Delegation is handled on demand by attaching an EIP-7702 authorization to whichever batch needs it, rather than by a startup `SetCode` transaction.

This design supersedes the `feat/batex_*` branches. Two of their phases are kept (the contract and most of the config); everything from their Phase 3 onward is replaced:

1. **Contract** — take `fix/batex_1` unchanged (rename to `Safenet7702Executor`, calldata-pointer gas optimization, the best-effort `InsufficientGas` guard), then add a `value` to each `Call` and an `ISafenet7702Executor` interface. The services depend on this interface, not on the concrete contract.
2. **Config** — take `feat/batex_2`'s TOML keys, but replace `max_in_flight_transactions` with a `SubmissionMode` enum, either `Direct { max_in_flight_transactions }` or `Batched { executor, max_batch_gas }`, deserialized through a private flat `RawConfig`, so that mixing keys from both modes is a config error. Configuring an executor limits the queue to one transaction in flight.
3. **Storage** — split the transaction storage into the queued `transactions` and an `allocated_nonces` table holding one row per nonce the queue uses: one for each onchain transaction, and one for each authorization it carries. Several queued transactions can be allocated to one nonce, and the nonce's row stores the onchain transaction built for them. If a transaction's nonce is consumed without its authorization's nonce, the storage recovers by turning the authorization's row into a cancellation transaction.
4. **Unsigned transactions** — replace `TxEip1559` inside the `tx` module with a queue-owned `UnsignedTransaction`. The `Signer` then builds and signs either a `TxEip1559` or a `TxEip7702` (with a self-signed authorization at `nonce + 1`).
5. **Batching** — replace the nonce cache with an `eth_getProof`-backed account cache (`nonce` and `code_hash`). Add the batch encoder and the EIP-7702 delegation code hash helper. Then wire them into `TransactionQueue::submit_pending`: when an executor is configured, allocate the longest prefix of queued transactions that fits in `max_batch_gas` as one batch. Attach an authorization whenever the account's code hash differs from the one wanted: the executor's delegation designator when an executor is configured, and empty code when none is (an authorization to `address(0)`, which removes a leftover delegation). While the account is delegated, keep one transaction in flight. Histogram metrics show how many calls batches carry and how much gas they use.
6. **Integration test** — a 7702 integration test in which validators delegate on demand and serve three signing requests through `execute` self-calls only.
7. **Startup check** — `TransactionQueue::new` refuses to start with a configured executor that has no code.
8. **Cleanup** — remove this specification and the temporary test-network migration script.

---

## Architecture Decision

Batching and delegation live entirely inside `crates/core/src/tx`. The services' action encoders, state machines and the `Driver`'s command dispatch do not change: they keep producing one `(Transaction, Option<u64>)` per action through `ActionEncoder`, and `TransactionQueue::queue` stores them as they are. The queue decides how they get onchain when it allocates nonces. Both the validator and the sentinel get the feature from the same code, and each action's `gas` estimate becomes its per-call `gasLimit` in the batch.

```text
 ActionEncoder::encode_action
        | (Transaction, Option<u64>)            unchanged public API
        v
 TransactionQueue::queue -> TransactionStorage::enqueue            one row per transaction, as today
        v
 TransactionQueue::submit_pending (while in flight < limit; limit is 1 with an executor or a delegated account)
        |-- allocation := Single without an executor, or
        |                 Batch { build: executor::batch(account, .., max_batch_gas) } with one
        |-- wanted := Authorization { address: executor or address(0) }
        |   authorization := account.code_hash != wanted.code_hash()   (address(0) wants empty code)
        v
 storage.next_transaction(status, allocation, authorization)
        |   the oldest unexpired unallocated transactions; `build` takes a prefix and builds the batch
        |   allocated_nonces row N (the onchain transaction), and row N + 1 (delegate) when authorized
        |   transactions.nonce = N for the prefix
        v
 AllocatedTransaction { nonce, transaction, authorization, fees } -> build -> UnsignedTransaction
        v
 Signer::sign_transaction -> TxEip1559 | TxEip7702 { authorization_list: [sign(auth @ nonce + 1)] }
```

### Batch at allocation, with one batch in flight

EIP-7702 recommends that mempools accept at most one pending transaction from an account with a delegation, and clients do this by default:

- Geth rejects a second in-flight transaction with `ErrInflightTxLimitReached` (`--txpool.maxinflightdelegatedslots`, default 1).
- Reth does the same through `max_inflight_delegated_slot_limit` (default 1).
- Nethermind rejects any transaction from a delegated account whose nonce is not the current one (`NotCurrentNonceForDelegation`).

The same limit applies before delegation lands. While an authorization is pending, Geth (`validateAuth`) and Nethermind (`DelegatorHasPendingTx`) allow only one in-flight transaction for its authority. And even if the operator's own node allowed more, other nodes would not propagate the extra transactions. So once an executor is configured, the signer account has one transaction in flight at every stage, and the queue enforces this itself (see Config). Anything more would be rejected at submission and retried every block, and nonces would be allocated to batches that cannot enter the mempool.

With one transaction in flight, batching at enqueue would turn every `queue()` call into its own onchain transaction. The validator calls `queue()` once per effect resume (every `RevealNonceCommitments` and `UseNonce` resumes separately), so three concurrent signing rounds would take six blocks instead of the roughly two that today's 16-deep pipeline needs. Batching at allocation solves this without touching the `Driver`: everything queued while a batch is pending is allocated together as the next batch once that batch executes.

A batch's contents are fixed when its nonce is allocated. Resubmission rebroadcasts the same batch with bumped fees and never adds transactions to it. This is deliberate: if a later submission of the same nonce could carry a different set of transactions, and several versions reached mempools, the queue could not tell which transactions actually executed onchain.

### The storage stays a nonce store

The storage does not know about calldata encoding or batch gas. It is a queue of transactions plus a store of allocated nonces. Each transaction row of `allocated_nonces` holds the onchain transaction built for that nonce (the queued transaction itself without an executor, or the `execute` batch with one) and all of its submission state. Which queued transactions it carries follows from `transactions.nonce`.

The queue decides what to allocate through an `Allocation`: `Single` takes the oldest queued transaction as it is, and `Batch` passes the oldest queued transactions to a pure `build` function, which returns how long a prefix it took and the batch it built from it. The storage allocates the nonce, stores the transaction, and links the prefix to it, all in one SQLite transaction, and tracks submission and execution per nonce, as it does per transaction today.

The row stores what was actually sent, so it does not depend on the configuration. After a restart with a different `executor`, or none, a nonce allocated earlier is still resubmitted exactly as it was built, whether it is a one-call batch or a plain transaction.

### The authorization rides on the transaction that needs it

Under EIP-7702 the authorization list is processed before the transaction's call runs. So a batch that carries its own authorization always runs against delegated code, whatever happened before it. The queue attaches an authorization to a batch whenever the signer account's code hash (from the account cache, at the block the nonce is allocated against) is not `keccak256(0xef0100 ‖ executor)`. Once the delegation is observed onchain, later batches stop carrying one.

Compared with the `feat/batex_*` design (a standalone `SetCode` transaction enqueued at startup and gated by nonce ordering), this:

- needs no startup step and no idempotency bookkeeping across restarts;
- repairs itself: if the account is ever undelegated or re-delegated elsewhere, the next batch re-delegates it;
- makes every batch that carries an authorization correct on its own, rather than correct only because some earlier transaction landed first.

The cost is the authorization itself: the `PER_EMPTY_ACCOUNT_COST` of 25,000 gas plus a second nonce. With one batch in flight, only the first batch after the executor is configured normally carries one. The account nonce and code hash come from the same `eth_getProof` response, so the authorization decision needs no extra request and cannot fail independently of nonce allocation.

### Removing the executor undelegates the account

Removing `executor` from the config does not remove the account's delegation onchain. A delegated account is still limited to one pending transaction by the mempool, so the configured `max_in_flight_transactions` could not be honored. The queue therefore undelegates the account itself, using the same rule as delegation. EIP-7702 treats an authorization to `address(0)` as a reset that leaves the account with empty code, so `Authorization { address: Address::ZERO }.code_hash()` is `KECCAK_EMPTY`. The authorization the queue wants is to the executor when one is configured, and to `address(0)` otherwise. It is attached whenever the account's code hash differs:

- executor configured, account undelegated or delegated elsewhere: authorize the executor;
- no executor, account delegated to anything, including an executor from an earlier configuration or a delegation made outside the queue: authorize `address(0)`;
- otherwise: no authorization.

Without an executor, the undelegating authorization rides on the next plain transaction, which becomes a type-4 transaction and is not wrapped in `execute`. Batches allocated before the restart keep their lower nonces, so they execute while the delegation they need is still in place, and the undelegation lands after them.

Until the account's code hash is observed to be empty, the queue keeps one transaction in flight even without an executor, because the mempool allows no more for a delegated account or for one with a pending authorization. This is the one in-flight rule the queue applies itself rather than through the config. The config's limit of 1 covers a configured executor, but it cannot know whether an account is still delegated from an earlier configuration. Once the delegation is gone, `max_in_flight_transactions` applies again. A non-existent account's `eth_getProof` code hash of zero is treated as `KECCAK_EMPTY` when the account status is fetched, so a fresh account is never undelegated needlessly.

### Authorizations have their own nonce row

A self-sponsored authorization must name `nonce + 1`, because the sender's nonce is incremented before the authorization list is processed, and applying it increments the nonce again. So a transaction at `N` with an authorization leaves the account at `N + 2`. The storage gives the authorization's nonce a row of its own:

- `allocated_nonces` has exactly one row for every nonce the queue uses, and is the single source of truth for them. The next free nonce is `MAX(status.nonce, MAX(nonce) + 1)`, with no special case.
- A row with a `request` is a transaction. A row with a `delegate` is an authorization, carried by the transaction at `nonce - 1`. Both are allocated together, in one SQLite transaction.
- An authorization row has no `expires_at` or `submitted_at`. `mark_executed` marks it executed like any other row once the account moves past its nonce.

### Recovering from a consumed transaction nonce with an unused authorization nonce

If the account's onchain nonce is `N + 1` and the row at `N + 1` is an authorization, then nonce `N` was consumed but the authorization's `N + 1` was not. This happens if the EOA was used outside the queue and replaced the transaction at `N`, or if the transaction at `N` executed but its authorization was skipped as invalid. Every nonce the queue allocates from then on sits behind a permanent gap at `N + 1`.

The storage recovers by giving the authorization's row a `request`, which turns it into a cancellation: a transaction row with no queued transactions, sent as `Transaction::default()` (a call to `address(0)` with no value and no data, 21,000 gas). Its `delegate` stays set, so the transaction at `N` still loads with its authorization. Because the cancellation is allocated but never submitted, the existing `stale_submissions` query picks it up and `resubmit_stale` broadcasts it in the same pass. This does not depend on the in-flight budget, which the stuck transactions above the gap may already exhaust.

This is reorg-safe. If a reorg means the original transaction and its authorization do execute after all, nonce `N + 1` is consumed by the authorization. The account nonce then moves past it, and `mark_executed` marks the row executed like any other row the account has moved past.

### Every call goes through the executor

When an executor is configured, even a single queued transaction is wrapped in a one-call `execute` batch. This keeps one code path and gives the executor a single place for extra accounting or custom onchain logic later. It also means an operator can see from the chain alone that batching is active. There are no exceptions: `Call` carries a `value`, so value-bearing transactions are batched like any other.

### Batches preserve order

A batch is the longest prefix of the queued transactions, in the order they were enqueued, that fits in `max_batch_gas`. Order matters: the sentinel emits `ApproveToken` before `Commit`, batches execute in nonce order, and calls within a batch execute in array order. Taking a prefix, rather than packing whatever fits, keeps that order across batches.

Expiry does not split batches. `next_transaction` only considers transactions that have not expired at the block the nonce is allocated against, so a batch never contains an expired transaction. After allocation, a batch is in the mempool and must execute as a whole, just as a single transaction does today once it is submitted.

### Alternatives Considered

- **Batch at enqueue** (this epic's first revision, and `feat/batex_*`). Each `queue()` call would be split into batches and stored as its own onchain transactions. Together with the mempool's one-in-flight limit for delegated accounts, every `queue()` call would cost a block, and the validator issues one per effect resume. Rejected in favor of batching at allocation.
- **A separate `max_in_flight_authorizations` limit** next to `max_in_flight_transactions`. This only covers the pending-authorization rule. Once the account is delegated, the mempool still limits it to one transaction in flight. Rejected: the queue enforces one in flight whenever an executor is configured.
- **Coalesce effect resumes in the `Driver`** (drain already-completed resumes before calling `queue()`). Timing-dependent, and unnecessary once transactions coalesce while a batch is in flight. Not pursued.
- **Let a resubmission absorb newly queued transactions.** This would get them onchain sooner, but different submissions of one nonce could then carry different transactions, and the queue could not tell which ones executed. Rejected.
- **Standalone startup `SetCode` transaction** (the `feat/batex_4` design). It needs a startup enqueue, restart idempotency, a `json_extract`-based two-nonce span in the allocation query, and the guarantee that no batch runs before delegation rests entirely on nonce ordering. Rejected in favor of per-transaction authorizations.
- **Infer the authorization span from JSON** (`feat/batex_4`). This makes the allocation query depend on the most recently allocated row's JSON. Rejected: the authorization's nonce has its own row.
- **A nullable `authorization` column on the transaction's row that reserves `nonce + 1`.** The next free nonce becomes `MAX(nonce + 1 + (authorization IS NOT NULL))`, an implicit rule every nonce query has to know. Recovery must insert a row into the reserved gap. Rejected in favor of a row per nonce.
- **A separate `authorizations` table.** SQL cannot enforce that a nonce is unique across two tables, and the next free nonce needs a `UNION`. Rejected.
- **Return a nonce's queued transactions and build the onchain transaction on every submission.** The storage would not duplicate the queued requests in the batch calldata. But the row would no longer say what was sent: whether one queued transaction went out as is or as a one-call batch would need a stored flag, or would follow from the current config, which can change across a restart. Rejected: the nonce row stores the built transaction.
- **`queued(block, limit)` and `allocate(status, ids, transaction, authorization)`** (this epic's previous revision). Queued row ids leak out of the storage, and the queue has to hand them back consistently with the batch it built. Rejected in favor of one `next_transaction` that takes an `Allocation`.
- **Leave the delegation in place when the executor is removed.** The account stays delegated, and the mempool keeps capping it at one pending transaction, so `max_in_flight_transactions` would be silently ignored. It would also leave the account's behavior depending on an executor the operator no longer configures. Rejected: the queue undelegates the account.
- **Unbatched single transactions** (send a batch of one as the original transaction). This saves the executor overhead for lone actions. Rejected per the section above.
- **`eth_getCode` next to `eth_getTransactionCount`** instead of `eth_getProof`. `eth_getCode` is universally supported, but this takes two requests and two failure modes for what `eth_getProof` returns in one. Rejected. The RPC node must support `eth_getProof` for recent blocks (see Assumptions).
- **A `#[serde(flatten)]` optional executor struct.** Verified: serde's flattened `Option` turns any error in the inner struct into `None`. So `max_batch_gas = 3000000` without `executor` parses as "batching disabled", and so does a malformed `executor = 1`. `flatten` also does not combine reliably with `deny_unknown_fields`. Rejected in favor of `try_from` a flat `RawConfig` (see Tech Specs).

---

## Tech Specs

### Contract: `Safenet7702Executor` and `ISafenet7702Executor`

Phase 1a is `fix/batex_1` rebased onto `main`: rename `Validator7702Account` to `Safenet7702Executor` (source, test, deploy script, plus the `contracts-deploy-safenet-7702-executor` recipe), bind each `Call` to one calldata pointer, add the `InsufficientGas(index)` guard (`gasleft() * 63 / 64 >= call.gasLimit`), and pin `execute`'s gas cost for a fixed four-call batch. The branch's own epic file (`epics/2026_09_09_…`) is not ported. The over-simplification in the gas check (it ignores the `CALL` base cost) is an accepted tradeoff. The offchain batch gas formula covers it.

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

`tx::Config` moves to a new `crates/core/src/tx/config.rs` module, re-exported with a new `SubmissionMode` as `pub use self::config::{Config, SubmissionMode}` from `tx`. `max_in_flight_transactions` moves into the `SubmissionMode` it applies to:

```rust
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(try_from = "RawConfig")]
pub struct Config {
    pub mode: SubmissionMode,
    pub blocks_before_resubmit: u64,
    pub priority_fee_cap_percentage: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SubmissionMode {
    /// Each queued transaction is submitted as its own transaction.
    Direct { max_in_flight_transactions: usize },
    /// Queued transactions are batched into self-calls to an
    /// `ISafenet7702Executor`, with one transaction in flight at a time.
    Batched { executor: Address, max_batch_gas: u64 },
}

impl SubmissionMode {
    /// `max_in_flight_transactions` for `Direct`, 1 for `Batched`.
    pub fn max_in_flight_transactions(&self) -> usize;
}
```

`Config` gets its `Deserialize` through `#[serde(try_from = "RawConfig")]`. `RawConfig` is private and mirrors the flat TOML table: `#[serde(default, deny_unknown_fields)]`, the existing fields with `max_in_flight_transactions` as an `Option<usize>`, plus `executor: Option<Address>` and `max_batch_gas: Option<u64>`. Its `Default` is derived from `Config::default()` so the defaults live in one place. `TryFrom<RawConfig> for Config` applies the grouping rules:

- no `executor` and no `max_batch_gas`: `Direct`, with `max_in_flight_transactions` taking its value or the default of 16;
- `max_batch_gas` without `executor` is an error: "`max_batch_gas` requires `executor`";
- `max_in_flight_transactions` with `executor` is an error: "`max_in_flight_transactions` cannot be combined with `executor`";
- `executor` alone or with `max_batch_gas`: `Batched { executor, max_batch_gas: gas.unwrap_or(2_000_000) }`.

A `Batched` mode's in-flight limit of 1 means a configured executor needs no special case: `submit_pending` already respects `max_in_flight_transactions()`. The only in-flight rule the queue applies itself is for an account that is still delegated when no executor is configured (see the Architecture Decision). Rejecting an explicit value, rather than silently ignoring it, follows the same rule as `max_batch_gas` without `executor`. Because `RawConfig` is a plain flat struct with no `flatten`, `deny_unknown_fields` keeps working, and a malformed `executor` value is an ordinary type error instead of silently disabling batching.

The TOML keys are the same as in `feat/batex_2`: `executor` and `max_batch_gas` in the existing `[transactions]` table, documented in both sample TOMLs next to a newly documented `max_in_flight_transactions`. The sample comments are reworded for this design. `feat/batex_2`'s text describes a startup delegation transaction and calls `max_batch_gas` "ignored unless `executor` is set", and neither is true here:

```toml
# Optional: the `ISafenet7702Executor` this service's signer account delegates
# to via EIP-7702. Every transaction is then sent as a self-call to the
# executor's `execute`, batching all actions queued while the previous
# transaction is pending. The first transaction carries the EIP-7702
# authorization that delegates the account. EIP-7702 mempools accept only one
# pending transaction from a delegated account, so setting `executor` limits
# the service to one transaction in flight and cannot be combined with
# `max_in_flight_transactions`. Omit to submit one transaction per action; if
# the account is still delegated, for example from an earlier `executor`, its
# next transaction removes the delegation, and until then only one transaction
# is in flight.
# executor = "0x0000000000000000000000000000000000000000"

# Optional: the maximum gas a single batched transaction may consume. Queued
# actions that do not fit wait for the next batch, and an action that does not
# fit on its own is sent as a batch of one. Requires `executor`.
# max_batch_gas = 2000000
```

The new `config` module has its own unit tests, and the services' config tests are not extended:

- `executor` with and without `max_batch_gas` (`Batched`, default gas);
- neither key (`Direct`, in-flight limit as configured or defaulted);
- `max_batch_gas` alone (rejected);
- `max_in_flight_transactions` with `executor` (rejected);
- a malformed `executor` (rejected);
- an unknown key (still rejected).

### Storage schema

`crates/core/src/tx/storage.rs`, final shape (introduced in full in Phase 3a):

```sql
CREATE TABLE IF NOT EXISTS allocated_nonces (
    nonce        INTEGER PRIMARY KEY,
    request      TEXT    DEFAULT NULL,  -- transaction rows: the onchain transaction, plus the fees of its last submission
    delegate     TEXT    DEFAULT NULL,  -- authorization rows: the delegate
    submitted_at INTEGER DEFAULT NULL,  -- transaction rows
    executed_at  INTEGER DEFAULT NULL,  -- every row: the account has moved past this nonce
    CHECK (request IS NOT NULL OR delegate IS NOT NULL)
);
CREATE TABLE IF NOT EXISTS transactions (
    id         INTEGER PRIMARY KEY,
    request    TEXT    NOT NULL,
    expires_at INTEGER DEFAULT NULL,
    nonce      INTEGER DEFAULT NULL REFERENCES allocated_nonces (nonce)
);
```

The table is not called `nonces`: the validator's secret store already has a `nonces` table for its FROST nonces, in the same database.

A `transactions` row is a queued transaction, exactly as enqueued. An `allocated_nonces` row is a nonce the queue has used:

- `request` set: the transaction sent at this nonce, with the fees of its last submission (absent until the first submission), as an `AllocatedTransaction` without its nonce. It carries the `transactions` rows that reference it: one sent as is, the calls of a batch, or none for a cancellation.
- `delegate` set: the transaction at `nonce - 1` carries an authorization to `delegate`, which uses up this nonce.
- both set: the authorization was never used, and the row was recovered as a cancellation (see the Architecture Decision).

Storage methods:

- **`enqueue`:** unchanged. It still takes `(Transaction, Option<u64>)` tuples. Nothing about a queued transaction is decided at enqueue any more.
- **`next_transaction(status, allocation, authorization)`:** in one SQLite transaction, selects the oldest queued transactions with no nonce that have not expired at `status.block` (one for `Allocation::Single`, up to `limit` for `Allocation::Batch`), inserts the transaction row at the next free nonce (`MAX(status.nonce, MAX(nonce) + 1)`) holding the transaction to send (the queued one, or the batch `build` returns), inserts the authorization row at the nonce after it when `authorization` is set, and sets `transactions.nonce` for the allocated prefix. Returns `None`, allocating nothing, when nothing is queued. In Phase 3a it keeps today's signature and allocates one transaction.
- **`count_in_flight`:** transaction rows with `executed_at IS NULL`.
- **`count_outstanding(block)`:** rows not yet executed, including authorization rows, plus unallocated transactions not yet expired. It only gates whether `update_block_status` fetches the account status, and an authorization row left behind by a skipped authorization must keep that happening, so the recovery can run.
- **`record_submission`, `stale_submissions`:** today's queries, on transaction rows.
- **`mark_executed`, `unmark_executed`:** today's queries, on every row. `mark_executed` marks rows whose nonce is below the account nonce.
- **`recover_authorization_gap(status)`:** sets the `request` of an authorization row at `status.nonce` to `Transaction::default()` (`WHERE nonce = ? AND request IS NULL`), turning it into a cancellation. It can only apply once.
- **`prune(safe)`:** deletes rows executed at or below `safe` together with their transactions (the transactions first), and unallocated transactions that expired at or before `safe`.

Loading an `AllocatedTransaction` reads the row's `request` and, from Phase 3b, joins the authorization row at `nonce + 1` (`LEFT JOIN allocated_nonces a ON a.nonce = n.nonce + 1 AND a.delegate IS NOT NULL`). sqlx enables `PRAGMA foreign_keys` by default, so the reference is enforced.

**Migration.** Safenet has not been released and does not run in production, so there is no in-app migration. `TransactionStorage::new` only changes its `CREATE TABLE IF NOT EXISTS` definitions, and a recreated database gets the new schema. It adds no schema probes, no `ALTER TABLE` calls and no migration execution.

For the test network, whose existing database has only the old `transactions` table, Phase 3a adds a temporary `migrations/2026_09_23_safenet_7702_executor_tx_batching.sql`. An operator applies it manually with `sqlite3 <database> < migrations/…sql` while the service is stopped. The application never discovers or runs it. It follows the earlier scheduled-pruning migration (removed in `80951a0`): `.bail on`, a single `BEGIN … COMMIT`, and header comments that give its scope, how to check whether it has already been applied, and how to apply it. The check is whether `transactions` still has a `submitted_at` column, not whether `allocated_nonces` exists: a service started before the migration creates an empty `allocated_nonces` itself (and leaves the old `transactions` as it is), which is also why the script creates it with `IF NOT EXISTS`. It:

1. creates `allocated_nonces` and copies `(nonce, request, submitted_at, executed_at)` into it for rows with a non-null nonce, `request` with its fees;
2. rebuilds `transactions` as `(id, request, expires_at, nonce)` with the new reference: create a new table, copy the rows (removing the fee fields from `request` with `json_remove`), drop the old table and rename the new one.

The script is checked by hand against a copy of a pre-change database before Phase 3a merges. It has no automated test and is removed in Phase 8.

### Types

`crates/core/src/tx/types.rs`:

```rust
/// An EIP-7702 delegation to authorize alongside a transaction.
pub struct Authorization {
    /// The delegate the signer account authorizes, or `Address::ZERO` to
    /// remove the account's delegation.
    pub address: Address,
}

impl Authorization {
    /// The code hash of the account once the authorization is applied:
    /// `keccak256(0xef0100 ‖ address)`, or `KECCAK_EMPTY` for `Address::ZERO`.
    pub fn code_hash(&self) -> B256;
}

pub struct AllocatedTransaction {
    pub nonce: u64,
    pub transaction: Transaction,             // the onchain transaction from the nonce row
    pub authorization: Option<Authorization>, // from the authorization row at `nonce + 1`
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

`crates/core/src/tx/storage.rs`:

```rust
/// How `next_transaction` allocates a nonce.
pub enum Allocation<'a> {
    /// The oldest queued transaction, sent as is.
    Single,
    /// A batch of the oldest queued transactions: `build` is given up to
    /// `limit` of them and returns how many it took, at least one, and the
    /// batch built from them.
    Batch {
        limit: usize,
        build: &'a dyn Fn(&[Transaction]) -> (usize, Transaction),
    },
}
```

The storage fails with an error, allocating nothing, if `build` returns a count of zero or more than the transactions it passed.

The public `TransactionQueue::queue` and `ActionEncoder` signatures keep their tuples, and so does `TransactionStorage::enqueue`: batching and authorization are decided at allocation.

`AllocatedTransaction::build` adds EIP-7702's `PER_EMPTY_ACCOUNT_COST` of 25,000 gas to the stored transaction's gas when an authorization is set, so the stored transaction and `max_batch_gas` exclude it. Until Phase 4a, `build` returns a `TxEip1559` and panics with `todo!()` for `Some(authorization)`. Nothing sets one before Phase 5d. From Phase 4a it returns an `UnsignedTransaction`, and the `todo!()` moves to the `UnsignedTransaction` → `TxEip1559` conversion that `submit_transaction` uses until Phase 4b. Phase 4b removes it.

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

It is fetched with `eth_getProof(signer, [], block)`, at the same block as today's `eth_getTransactionCount` (the latest block status's number, or `latest` before the first status), so `mark_executed` and `unmark_executed` stay consistent with the block they record. A code hash of zero, which some clients return for an account that does not exist, is normalized to `KECCAK_EMPTY`. `AccountStatus::is_delegated()` is `code_hash != KECCAK_EMPTY`: an EOA's only possible code is a delegation designator. It is one request, as today, and is invalidated on the same block status changes. Existing callers use `account().await?.nonce`. The queue tests' `U64` nonce responses are replaced by a small `account_proof(nonce, code_hash)` helper, so the test churn is mechanical.

This makes `eth_getProof` for recent blocks a requirement on the RPC node, whether or not an executor is configured. Phase 5a documents it in both handbooks, and replaces `eth_getTransactionCount` with `eth_getProof` in the validator handbook's RPC method table. Of the public Gnosis Chain RPCs listed on chainlist that responded, all but one (`gnosis.oat.farm`, which disables the method) serve `eth_getProof` for the latest and recent block numbers. Reth nodes need a non-zero `--rpc.eth-proof-window`, because the requested block can trail the node's tip.

### Batch encoding

New module `crates/core/src/tx/executor.rs`, with a `sol!` transcription of `ISafenet7702Executor` (following the repo's existing inline `sol!` bindings) and one pure function, the `build` of `Allocation::Batch`:

```rust
/// Builds an `ISafenet7702Executor.execute` self-call sent to `account` from
/// the longest prefix of `transactions` whose batch gas stays within
/// `max_batch_gas`, and at least the first transaction. Returns the number of
/// transactions taken and the batch.
pub fn batch(
    account: Address,
    transactions: &[Transaction],
    max_batch_gas: u64,
) -> (usize, Transaction);
```

`account` is the signer's own address, not the executor's. Sending `execute` calldata to the executor implementation instead would silently discard every action, so the parameter is named to make the two hard to confuse.

Rules:

1. Take transactions in order while the batch gas stays within `max_batch_gas`. Stop at the first one that does not fit. Transactions after it are not considered, so order is preserved.
2. Every batch goes through `execute`, including a batch of one.
3. A first transaction whose own gas exceeds `max_batch_gas` becomes a one-call batch (over the limit) and is never dropped. This is normal operation and logs at `debug`, not `warn`. It also means `max_batch_gas = 0` sends every transaction as its own one-call batch, for operators who want the executor (for onchain accounting, say) without batching.
4. A batch is `Transaction { to: account, value: 0, data: executeCall { calls }.abi_encode(), gas }`, where each call is `Call { to, value, gasLimit: gas, data }` from the original transaction. A transaction's `value` travels in its `Call` and is paid from the account's own balance.

`submit_pending` sets the `Allocation::Batch` `limit` to `max_batch_gas / 5_000 + 1` queued transactions. Every call adds at least the formula's 5,000 per-call overhead, so no batch within the limit can hold more, and the bound caps the query without cutting a batch short.

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
- several under the limit (one batch, all taken);
- a stop at the gas limit (the prefix that fits, the rest left);
- no packing past a transaction that does not fit, even if a later one would;
- an oversized first transaction (a batch of one);
- `max_batch_gas = 0` (a batch of one);
- a value-bearing transaction (value carried in its `Call`, value-transfer gas added);
- decoding every batch back through `executeCall::abi_decode` to assert the exact calls.

### Wiring

```rust
async fn submit_pending(&mut self, block: u64) -> Result<(), Error> {
    let account = self.account().await?;
    let limit = match account.is_delegated() {
        // Mempools accept one pending transaction from a delegated account.
        true => 1,
        false => self.config.mode.max_in_flight_transactions(),
    };
    let in_flight = self.storage.count_in_flight().await?;
    for _ in in_flight..limit {
        let account = self.account().await?;
        let status = Status { nonce: account.nonce, block };
        let account_address = self.signer.address();
        let build;
        let (allocation, wanted) = match self.config.mode {
            SubmissionMode::Direct { .. } => (Allocation::Single, Authorization { address: Address::ZERO }),
            SubmissionMode::Batched { executor, max_batch_gas } => {
                build = move |queued: &[Transaction]| executor::batch(account_address, queued, max_batch_gas);
                let limit = usize::try_from(max_batch_gas / 5_000 + 1).unwrap_or(usize::MAX);
                (Allocation::Batch { limit, build: &build }, Authorization { address: executor })
            }
        };
        let authorization = (account.code_hash != wanted.code_hash()).then_some(wanted);
        let Some(transaction) = self.storage.next_transaction(status, allocation, authorization).await? else {
            break;
        };
        self.submit_transaction(transaction, block).await?;
    }
    Ok(())
}
```

`queue()` itself goes back to only enqueueing and calling `submit_pending`, as today. The 25,000 authorization gas is added by `build`, on top of the batch's gas and not counted against `max_batch_gas`. The account status is already required for the nonce, so the authorization decision adds no request and no failure mode. The account status is cached per block, so within one `submit_pending` pass the code hash does not change, and at most one transaction is allocated while the account is delegated.

`update_block_status` calls `storage.recover_authorization_gap(status)` right after `mark_executed`, before `resubmit_stale`.

`submit_pending` logs at `debug` for each batch (call count and gas), and at `info` when it attaches an authorization, naming whether it delegates or undelegates the account.

### Metrics

`crates/core/src/metrics.rs` gains two histograms, following the existing `metrics::histogram!` pattern in `crates/sentinel/src/metrics.rs`. Both are recorded by the `build` closure that `submit_pending` passes in `Allocation::Batch`, from the prefix length and the batch `executor::batch` returns. Resubmissions do not build, so they are not counted again, and only batches are recorded:

- `safenet_core_transaction_batch_size` — the number of calls in each batch. This is the measure of whether batching pays off at all: a distribution stuck at 1 means the executor adds overhead without coalescing anything.
- `safenet_core_transaction_batch_gas` — the gas limit of each batch, excluding the authorization gas. Values near `max_batch_gas` mean the limit is what splits batches.

### Integration coverage

New script `scripts/run_validator_7702_integration_test.sh`, Justfile recipe `test-integration-validator-7702`, and a matrix entry in `.github/workflows/integration.yml`. `scripts/lib/shared_test_scripts.sh` gains two things:

- a `deploy_safenet_7702_executor` helper, using `DeploySafenet7702ExecutorScript`;
- an optional executor argument on `print_validator_config_base`, which emits `executor` in the `[transactions]` table.

The test:

1. Deploys the contracts and the executor, and starts two validators with `executor` configured.
2. Triggers genesis keygen and waits until both validators are delegated (`cast code` equals `0xef0100 ‖ executor`) and their startup/preprocess transactions have been mined.
3. Records the current block.
4. Proposes three transactions (distinct Safe nonces) from the Anvil deployer account in one transaction: `cast send <deployer> 'execute((address,uint256,uint256,bytes)[])' … --auth <executor>`. Asserts that the three `TransactionProposed` logs share a block.
5. Waits for all three `TransactionAttested` events.
6. Asserts that every transaction either validator sent between the recorded block and the last attestation is addressed to the validator itself and calls `execute`.
7. Restarts one validator without `executor`, proposes one more transaction, waits for its `TransactionAttested`, and asserts that the restarted validator's `cast code` is empty.

It does not assert how many transactions the validators used. Nonce reveals and signature shares reach the queue one effect resume at a time, so how many coalesce depends on timing.

---

## Implementation Phases

Each phase is a separate PR, targeting fewer than 300 changed lines and fewer than ten files. This specification is its own plan-only PR.

Parallel tracks:

- **Contracts** (1a → 1b → 1c) are independent of all Rust work until Phase 6.
- **Config** (2a → 2b) is independent and only needs to land before 5d.
- **Storage** (3a → 3b → 3c) then **unsigned transactions** (4a → 4b) form the main sequential track.
- **Account cache** (5a) and **batch encoder** (5b) are independent of the storage track and of each other. 5b's binding must match 1c's interface.
- **Code hash** (5c) needs only the `Authorization` type from 3b.
- **Wiring** (5d) joins every track. Tests (5e) and metrics (5f) follow it and are independent of each other, as is the integration test (6).
- **Startup check** (7) only needs 2b, but is scheduled last as a short follow-up.

Nothing is observable to operators until 5d, except the in-flight limit of 1 from 2b. Before then, setting `executor` parses but does not batch, which the sample TOML comments added in Phase 2b must not contradict: they describe the behavior once it ships, and 5d is where it becomes true.

### Phase 1a — Rename, gas-optimize and guard the executor contract

Port `fix/batex_1` onto `main` unchanged in substance, without its epic file. Keep the rename as its own commit so the diff stays readable.

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

Add `SubmissionMode`, replace `Config::max_in_flight_transactions` with `Config::mode`, add the private `RawConfig` and its `TryFrom` (including the in-flight limit of 1 and the rejected combinations), and switch `Config` to `#[serde(try_from = "RawConfig")]`. Re-export `SubmissionMode`. Add the sample TOML keys with the comments above. Add the config module's unit tests with the cases above. Config only: nothing reads `executor` yet.

**Files:** `crates/core/src/tx/config.rs`, `crates/core/src/tx/mod.rs`, `crates/validator/src/config.rs` (the existing in-flight assertion), `crates/validator/validator.sample.toml`, `crates/sentinel/sentinel.sample.toml`.

### Phase 3a — `allocated_nonces` table and final schema

Introduce the full final schema (`allocated_nonces` with its unused `delegate` column, and `transactions.nonce` referencing it) in one PR, so that the test network needs only one manual migration. Move submission and execution state and every query about it onto `allocated_nonces`. `next_transaction` keeps its signature and still allocates one queued transaction per nonce, copying its request into the new nonce row. `prune` deletes a nonce row together with its transactions. Add the temporary manual migration script for the test network. Behavior-preserving: the existing storage and queue tests pass unchanged. A storage test covers pruning an executed transaction together with its nonce row, which the foreign key makes order-sensitive.

**Files:** `crates/core/src/tx/storage.rs`, `migrations/2026_09_23_safenet_7702_executor_tx_batching.sql` (new).

### Phase 3b — Allocate batches and authorizations

Add `types::Authorization`, `AllocatedTransaction::authorization` and `storage::Allocation`. `next_transaction` takes an `Allocation` and an `Option<Authorization>`: a batch stores the transaction `build` returns and allocates its prefix, an authorization gets its own row at `N + 1`, and `AllocatedTransaction::authorization` is loaded from that row. `submit_pending` passes `Allocation::Single` and no authorization. `build` panics with `todo!()` for `Some(authorization)`.

Storage tests cover:

- a batch of several transactions allocated to one nonce, which records its submission and is marked executed as one, and stores the transaction `build` returned;
- `build` is given only unexpired, unallocated transactions in order, at most `limit`, and a count of zero or more than it was given fails without allocating anything;
- a nonce with an authorization takes `N`, the authorization row takes `N + 1`, and the next allocation takes `N + 2`;
- allocation without authorizations is unchanged;
- a nonce consumed outside the queue still wins over the next free nonce;
- `AllocatedTransaction::authorization` round-trips, and a transaction without one has none;
- pruning removes a nonce row, its authorization row and all of its transactions.

**Files:** `crates/core/src/tx/types.rs`, `crates/core/src/tx/storage.rs`, `crates/core/src/tx/mod.rs`.

### Phase 3c — Recover an unused authorization nonce

Add `recover_authorization_gap`, called from `update_block_status` right after `mark_executed`.

Storage tests cover:

- the cancellation is created exactly once, loads as `Transaction::default()` with no queued transactions, and is returned by `stale_submissions`, while the transaction before it still loads with its authorization;
- nothing is created while the authorization's transaction is still in flight, or once the account is past `N + 1`;
- the cancellation is marked executed when the nonce moves past it, both when it lands and when a reorg lets the original authorization land instead.

A queue test covers recovery with the in-flight budget already full.

**Files:** `crates/core/src/tx/storage.rs`, `crates/core/src/tx/mod.rs`.

### Phase 4a — `UnsignedTransaction`

Add `types::UnsignedTransaction`. `AllocatedTransaction::build` returns it, and `submit_transaction` reads the submission fees from it. Until 4b, a temporary conversion to `TxEip1559` feeds the unchanged `Signer` and holds the `todo!()` for authorizations. No behavior change.

**Files:** `crates/core/src/tx/types.rs`, `crates/core/src/tx/mod.rs`.

### Phase 4b — Sign `UnsignedTransaction`

Change `Signer::sign_transaction` to take `UnsignedTransaction`, building `TxEip1559` or `TxEip7702` with the self-signed authorization at `nonce.checked_add(1)`. Remove the temporary conversion and its `todo!()`. Signer tests cover both transaction types.

**Files:** `crates/core/src/tx/signer.rs`, `crates/core/src/tx/mod.rs`.

### Phase 5a — Account cache via `eth_getProof`

Replace `nonce_cache`/`nonce()` with `account_cache`/`account()` returning `AccountStatus` (with the zero code hash normalization and `is_delegated`), fetched at the same block as today, and update the queue tests to use the `account_proof` helper. Document the `eth_getProof` requirement in both handbooks, including the Reth proof-window note, and update the validator handbook's RPC method table. No behavior change beyond the RPC method. Independent of Phases 3 and 4.

**Files:** `crates/core/src/tx/mod.rs` (plus `types.rs` if `AccountStatus` lives there), `docs/validator-handbook.md`, `docs/sentinel-handbook.md`.

### Phase 5b — Batch encoder

Add `tx/executor.rs` with the `ISafenet7702Executor` binding, `batch`, the gas formula and its unit tests. Nothing calls it yet. Confirm the formula's constants against the gas snapshot from Phases 1a and 1b, and record the numbers in the PR description.

**Files:** `crates/core/src/tx/executor.rs` (new), `crates/core/src/tx/mod.rs` (module declaration).

### Phase 5c — Delegation code hash

Add `Authorization::code_hash`, using alloy's EIP-7702 delegation designator constant, with `KECCAK_EMPTY` for `Address::ZERO`. Test it against a known delegated account's code hash, and for `Address::ZERO`. Can be done in parallel with 5a and 5b once 3b has landed.

**Files:** `crates/core/src/tx/types.rs`.

### Phase 5d — Batch and authorize at allocation

Wire `Allocation::Batch` with `executor::batch`, the authorization decision (delegating and undelegating) and the delegated-account in-flight limit into `TransactionQueue::submit_pending` as specified above, and add the 25,000 authorization gas in `build`. Add a batching tip to the `[transactions]` sections of both handbooks, covering the one-in-flight limit and how removing `executor` undelegates the account.

Queue tests cover:

- an unconfigured queue with an undelegated account behaves exactly as before;
- with an executor, an undelegated account gets a type-4 transaction carrying an authorization at `n + 1`, and the next allocation skips a nonce;
- an account already delegated to the executor gets a plain type-2 self-call;
- an account delegated elsewhere is re-delegated;
- without an executor, a delegated account's next transaction is sent unwrapped as a type-4 transaction authorizing `address(0)`, only one transaction is in flight until the code hash is observed empty, and `max_in_flight_transactions` applies again after that.

**Files:** `crates/core/src/tx/mod.rs`, `docs/validator-handbook.md`, `docs/sentinel-handbook.md`.

### Phase 5e — Batching queue tests

Tests in `safenet_core::tx::tests` only:

- transactions queued across several `queue()` calls while a batch is in flight are not submitted until it executes, and then go out as exactly one `eth_sendRawTransaction`, addressed to the signer's own account, whose calldata decodes to the queued calls in order;
- resubmitting a stale batch rebroadcasts the same calls, even when more transactions have been queued since;
- a queue whose total gas exceeds `max_batch_gas` is submitted as successive batches, one per executed nonce, each within the limit;
- a transaction that expires while waiting behind an in-flight batch is left out of the next one.

**Files:** `crates/core/src/tx/mod.rs`.

### Phase 5f — Batching metrics

Add the `safenet_core_transaction_batch_size` and `safenet_core_transaction_batch_gas` histograms and record them in `submit_pending` as specified above.

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

- The configured RPC supports `eth_getProof` for the latest block and blocks shortly before it. Geth, Nethermind, Erigon and Anvil do by default, and Reth does with a non-zero `--rpc.eth-proof-window`. On Gnosis Chain, the public RPCs listed on chainlist that responded all do, except `gnosis.oat.farm`. Operators must pick an RPC that supports it, whether or not they configure an executor.
- An EOA's only possible code is an EIP-7702 delegation designator, so a non-empty code hash always means the account is delegated.
- Mempools accept one pending transaction from an account that is delegated or has a pending authorization (see the Architecture Decision). Operators who run a node with a raised limit gain nothing, because other nodes would not propagate the extra transactions.
- Anvil under Foundry 1.5.1 runs a Prague-or-later hardfork by default. If not, Phase 6 passes `--hardfork prague` through `start_anvil`. Anvil does not enforce the one-in-flight mempool limit, so the integration test does not exercise it. Phases 5d and 5e cover it through the in-flight limit of 1.
- Foundry 1.5.1's `cast send --auth` can delegate and call in one transaction for the integration test's batched proposal. If not, a small Forge script using `vm.signAndAttachDelegation` does the same.
- The signer account is dedicated to the service. Using it outside the queue is handled (the Phase 3c recovery) but not supported: an action whose nonce was taken by an outside transaction is lost, exactly as it is today.
- `max_batch_gas` (default 2,000,000) stays well below the block gas limit of every target chain, so the 25,000 authorization gas added on top of it never matters for inclusion.
