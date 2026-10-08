# PoC — F-CORE-067

**Never compiled, never run.** The audit host has no Rust toolchain (`rust-audit/state/baseline.md` §1: `cargo`, `rustc`, `rustup` all report exit 127). Every identifier below was checked by hand against commit `2893917`, but expect mechanical fixes (import paths, a `.clone`, a type annotation) on first build.

## What it shows

One action, emitted once by a pure transition, becomes **two queued transactions at two different nonces** — i.e. two onchain transactions — after the rollback replay that `BlockWatcher::initialize` performs on every restart (`crates/core/src/index/blocks.rs:261-266`).

## Where the code goes

`safenet-core`'s `tx::storage` module is **private** (`crates/core/src/tx/mod.rs:11` is `mod storage;`), so `TransactionStorage` cannot be reached from an integration test under `crates/core/tests/`. Both tests must be pasted into the existing `#[cfg(test)] mod tests` block at the bottom of `crates/core/src/tx/storage.rs`, immediately before its closing `}`. They reuse that module's `storage`, `tx` and `Status` helpers.

```
# from the repo root
$EDITOR crates/core/src/tx/storage.rs      # paste poc_tx_storage.rs before the final }
cargo test -p safenet-core --lib tx::storage::tests::poc_f_core_067
git checkout -- crates/core/src/tx/storage.rs
```

## Fixtures — spelled out

Nothing is attacker-supplied here; the trigger is an ordinary restart. The fixtures are:

| Fixture | Value | Why this value |
| --- | --- | --- |
| Action calldata | `0xdeadbeef` (part 1), `0x42` (part 2) | Arbitrary. What matters is that the two enqueued rows are **byte-identical**, which is what a pure transition re-emitting from an identical snapshot produces. |
| `expires_at` | `None` | The case `prune` can never clear (`tx/storage.rs:259-262`). The sentinel queues `Finalize` and `Claim` this way (`crates/sentinel/src/service.rs:521-526`, `:566-571`); the validator queues two actions this way (F-VAL-065). |
| Account nonce | `7` (part 1), `0` (part 2) | Any value; the point is that the second row gets `nonce + 1`, not `nonce`. |
| Block sequence | `New{1}`, `Logs{1..=1, []}`, `New{2}`, `Logs{2..=2,[0x42]}`, **`Uncle{2}`**, `New{2}`, `Logs{2..=2,[0x42]}` | `Uncle{2}` is exactly `Uncle { number: indexed.safe + 1 }` with `{safe: 1, latest: 2}` — the _synthetic_ uncle that `blocks.rs:261-266` emits on **every** restart retaining more than one snapshot. A real reorg produces the identical sequence. |

## Reading the result

- **Part 1 `poc_f_core_067_enqueue_is_not_idempotent`**
  - **Passes** → `enqueue` gained an idempotency key or a unique constraint, and the second identical insert was collapsed. The finding is fixed.
  - **Fails** → the finding reproduces. The panic message names both nonces; on this checkout expect `first=7 second=Some(AllocatedTransaction { nonce: 8, .. })` with calldata identical to `first`. **Two nonces means two onchain transactions, not a replacement.**

- **Part 2 `poc_f_core_067_restart_replay_enqueues_a_duplicate_action`**
  - **Passes** → the rollback rolled the queue back with the state, or the queueing path de-duplicated. Fixed.
  - **Fails at the second assertion with `queued_after_replay == 2`** → the finding reproduces end to end: `SnapshotStore::reorg` deleted the snapshot (`state/storage.rs:129-133`) but left the `transactions` row, and the pure transition re-emitted the same action, which `enqueue` inserted beside it.
  - **Fails at the FIRST assertion (`queued_before_restart != 1`)** → the test itself is wrong, not the code. Fix the harness before reading anything into it.

## What this PoC deliberately does not do

It does not drive a real `Driver`, because `Driver::new` requires a live `Provider`. The step it stands in for is `driver.rs:266-284` — every returned `Command::Action` is encoded and passed to `TransactionQueue::queue`, which is `self.storage.enqueue(transactions)` with nothing in between (`tx/mod.rs:132-135`). The `drain` helper in part 2 is that path, minus the provider. Anyone who wants the full path can add an `Asserter`-backed `TransactionQueue` (see `crates/core/src/tx/mod.rs`'s `queue` test helper) in place of the bare `TransactionStorage`; the assertion is unchanged.
