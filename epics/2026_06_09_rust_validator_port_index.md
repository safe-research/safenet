# Code Index: experimental Rust validator port (`project/ports`)

> Companion reference to [`2026_06_09_safenet_core_crate.md`](./2026_06_09_safenet_core_crate.md).
> **Not** part of the epic plan PR — this is a research artifact cataloguing reusable snippets from
> the **partially-implemented, hacky, experimental** Rust port that lives on the `project/ports`
> branch (crate `validator-rust/`). Read files on-branch with
> `git show project/ports:validator-rust/src/<file>`.

## Status & caveats

The port is a spike: it implements the genesis-DKG happy path only, has **no reorg handling**
(asserts monotonic block numbers and errors otherwise), **no robust transaction management**
(fire-and-forget, no nonce store / resubmission), and uses `anyhow` + `rusqlite` rather than the
`thiserror` + `sqlx` chosen for `safenet-core`. Treat it as a **source of proven snippets and
type-level patterns**, not as an architecture to copy wholesale.

**The epic plan is the source of truth.** Pull snippets from here **only when in doubt** — never
follow this branch as a guide or mirror its structure. Where this spike and the plan disagree,
**follow the plan**.

## Module map

| File (`validator-rust/src/`)                        | LOC  | Purpose                                                                                                                                 | Reuse value for the epic                                                               |
| --------------------------------------------------- | ---- | --------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `bindings.rs`                                       | 197  | `sol!` block: `Point`/`Attestation`/`SafeTransaction` structs + `#[sol(rpc)] contract Consensus`/`Coordinator` with all events & calls. | **High** — shows `sol!` layout that generates the `*Events` enums used by the indexer. |
| `watcher.rs`                                        | 136  | Block + log indexer; `Update`/`Event` types; `decode_log` address dispatch.                                                             | **High (the `sol!` generics)** — feeds Phase C (indexer).                              |
| `config/provider.rs`                                | 20   | `Provider` type + `RetryBackoffLayer`-wired builder.                                                                                    | **High** — feeds PR A2 (`rpc.rs`).                                                     |
| `actions.rs`                                        | 160  | Action enum → ABI-encoded EIP-1559 tx; `Handler`/`Worker` mpsc submit loop.                                                             | **Medium** — tx primitives feed Phase E; nonce/resubmit logic is missing.              |
| `state/storage.rs`                                  | 60   | `rusqlite` block-keyed state snapshots: `save`/`load_latest` + prune.                                                                   | **High** — feeds Phase D (snapshot store); missing `rollback_to`.                      |
| `state/mod.rs`                                      | 224  | `ValidatorState` / `Phase` FSM; `on_block`/`on_*_event` → `Vec<Action>`.                                                                | Low (validator-specific) — shows event-consumption shape.                              |
| `driver.rs`                                         | 79   | Wires provider/signer/chain/addresses/storage/state + watcher loop.                                                                     | **Medium** — bootstrap/wiring reference for F2 e2e.                                    |
| `config/mod.rs`                                     | 82   | TOML `ValidatorConfig` (+ tests); `state_history: NonZeroU64`.                                                                          | Low — config is a consumer concern.                                                    |
| `config/chain.rs`                                   | 42   | `Chain` enum (Gnosis/Sepolia/Anvil); `blocks_per_epoch`.                                                                                | Low.                                                                                   |
| `config/addresses.rs`                               | 22   | Resolves coordinator from consensus via `getCoordinator()`.                                                                             | Low.                                                                                   |
| `main.rs`                                           | 50   | `argh` CLI, `tracing_subscriber::fmt().with_env_filter(...)`, TOML load.                                                                | **Medium** — minimal logging init reference for PR B1.                                 |
| `frost/{mod,keygen,participants,secret,marshal}.rs` | ~709 | FROST DKG rounds, participant identifiers/proofs, ECDH secret encryption, serde marshalling of `frost-core` types.                      | Out of epic scope (no FROST pillar), catalogued for completeness.                      |

Dependencies (`validator-rust/Cargo.toml`): `alloy = "2"` (feat `signer-local`), `rusqlite`
(bundled), `tracing`/`tracing-subscriber` (env-filter, fmt), `tokio` (full), `tokio-stream`,
`anyhow`, `serde`/`serde_json`, `toml`, `url`, `argh`, `frost-secp256k1`/`frost-core`, `k256`,
`rand`. Note: **no `sqlx`, no `thiserror`, no metrics crate** (intentional divergences).

---

## 1. The `sol!` typed-event indexer (the key pattern)

This is the part the user called out as "done right". The trick is **not** a single
`Watcher<E: SolEventInterface>` generic spanning multiple contracts — it is **per-contract
`*Events` enums dispatched by `log.address()`**, with the rpc-log → primitive-log bridge made
explicit via `.into_inner()`.

`sol!` (`bindings.rs`) generates, for each `#[sol(rpc)] contract Consensus { event …; }`, an enum
`Consensus::ConsensusEvents` that implements `alloy::sol_types::SolEventInterface` (and likewise
`Coordinator::CoordinatorEvents`). The indexer wraps those:

```rust
// watcher.rs
use alloy::{rpc::types::{Filter, Log}, sol_types::SolEventInterface};

pub struct Update { pub block_number: u64, pub events: Vec<Event> }

pub enum Event {
    Consensus(Consensus::ConsensusEvents),
    Coordinator(Coordinator::CoordinatorEvents),
}

fn decode_log(addresses: &Addresses, log: Log) -> Option<Event> {
    let result = if log.address() == addresses.consensus {
        Consensus::ConsensusEvents::decode_log(&log.into_inner())   // <-- rpc Log -> Log<LogData>
            .context("failed to decode consensus log")
            .map(|log| Some(Event::Consensus(log.data)))            // <-- .data is the enum
    } else if log.address() == addresses.coordinator {
        Coordinator::CoordinatorEvents::decode_log(&log.into_inner())
            .context("failed to decode coordinator log")
            .map(|log| Some(Event::Coordinator(log.data)))
    } else {
        Ok(None)
    };
    match result {
        Ok(log) => log,
        Err(err) => { tracing::warn!(%err, "skipping unknown event"); None }
    }
}
```

**Type-level gotchas that this resolves** (the bits the predecessor struggled with):

1. `SolEventInterface::decode_log` takes `&alloy::primitives::Log` (`Log<LogData>`), **not** the
   `alloy::rpc::types::Log` returned by `provider.get_logs`. The bridge is
   **`rpc_log.into_inner()`**. Getting this wrong produces confusing trait/type-mismatch errors.
2. `SolEventInterface` **must be in scope** (`use alloy::sol_types::SolEventInterface`) for
   `decode_log` to resolve.
3. The generated method returns `Result<Log<ContractEvents>, _>`; the decoded enum is the **`.data`
   field**, not the return value itself.
4. For **multiple contracts**, one merged `E: SolEventInterface` does not exist — each contract has
   its own enum. **Dispatch by `log.address()`** and decode into the matching per-contract enum.

The rest of the watcher (subscribe → fetch → decode → emit) for reference:

```rust
let mut blocks = provider.watch_blocks().await?.into_stream();
let filter = Filter::new().address(vec![addresses.consensus, addresses.coordinator]);
// ... optional backfill: blocks.next() -> range get_logs(from..=to) -> bucket by block_number ...
loop {
  tokio::select! {
    blocks = blocks.next() => for block_hash in blocks.context("…")? {
      let block = provider.get_block_by_hash(block_hash).await?.context("missing block")?;
      // monotonic check (NO reorg handling — just errors):
      anyhow::ensure!(block.header.number == last + 1, "non-monotonic block number …");
      let filter = filter.clone().at_block_hash(block_hash);
      let mut logs = provider.get_logs(&filter).await?;
      logs.sort_unstable_by_key(|log| log.log_index);
      let events = logs.into_iter().filter_map(|l| decode_log(&addresses, l)).collect();
      on_update(Update { block_number: block.header.number, events });
    }
    signal = tokio::signal::ctrl_c() => { signal?; return Ok(()); }
  }
}
```

**How this maps to the epic (Phase C):** Generalise the hand-written `decode_log` into a small
crate-defined trait the consumer implements, so `safenet-core`'s `Watcher` stays generic and typed
without knowing the contracts:

```rust
// proposed crate API — consumer impls this once, using the snippet above as the body
pub trait DecodeLog: Sized {
    fn decode_log(address: Address, log: &alloy::rpc::types::Log) -> Option<Self>;
}
// Watcher<E: DecodeLog> emits Update<E> { block_number, events: Vec<E> }
```

For a single-contract watcher, the `E: SolEventInterface` form also works; the trait above covers
the general multi-contract case the reference demonstrates. **The reference (`project/ports`) is the
canonical example to copy the `decode_log` body and the `.into_inner()` bridge from.** Everything
else in Phase C (reorg detection, bloom skipping, range-warp pagination, `revalidate_last_block`)
is net-new — the spike has none of it.

---

## 2. Provider with `RetryBackoffLayer` (feeds PR A2)

```rust
// config/provider.rs
use alloy::{providers::{ProviderBuilder, RootProvider}, rpc::client::ClientBuilder,
            transports::layers::RetryBackoffLayer};

pub type Provider = RootProvider;

pub fn create(url: Url) -> Provider {
    let client = ClientBuilder::default()
        .layer(RetryBackoffLayer::new(10, 500, 500))   // max_retries, initial_backoff_ms, cups
        .http(url);
    ProviderBuilder::new()
        .disable_recommended_fillers()                 // fills manually in actions.rs
        .connect_client(client)
}
```

Answers Open Question #2 (RetryBackoffLayer params): the spike uses `(10, 500, 500)`. Note it
**disables** recommended fillers; the epic plan instead leans on built-in fee estimation
(`estimate_eip1559_fees`) in the tx manager — keep recommended fillers (or call the estimator
directly) per the plan rather than copying `disable_recommended_fillers()`.

---

## 3. Transaction submission primitives (feeds Phase E)

The spike has the low-level submit primitives but **none** of the robust management (no persistent
nonce/status store, no resubmission, no fee bump). Reuse the build/sign/send mechanics; keep the
TS-ported `TransactionStorage` + `TransactionManager` logic for the rest.

```rust
// actions.rs — build, sign, send an EIP-1559 tx
use alloy::{consensus::{SignableTransaction as _, TxEip1559}, eips::Encodable2718 as _,
            network::TxSignerSync as _, primitives::TxKind, sol_types::SolCall as _};

let data = Coordinator::keyGenConfirmCall { gid }.abi_encode();   // sol!-generated *Call
let (nonce, max_fee_per_gas, max_priority_fee_per_gas) = tokio::try_join!(
    self.provider.get_transaction_count(self.signer.address()),
    self.provider.get_gas_price(),
    self.provider.get_max_priority_fee_per_gas(),
)?;
let mut tx = TxEip1559 { chain_id, nonce, gas_limit, max_fee_per_gas,
                         max_priority_fee_per_gas, to: TxKind::Call(to), input: data.into(),
                         ..Default::default() };
let signature = self.signer.sign_transaction_sync(&mut tx)?;       // PrivateKeySigner
let raw_tx = tx.into_signed(signature).encoded_2718();
let tx_hash = self.provider.send_raw_transaction(&raw_tx).await?.watch().await?;
```

Notes for the epic:

- Signing snippet (`PrivateKeySigner` + `TxSignerSync::sign_transaction_sync` +
  `into_signed(..).encoded_2718()`) is the body for the `Account` trait's default impl (PR E1).
- The spike fetches fees via `get_gas_price` + `get_max_priority_fee_per_gas`; the plan prefers the
  single built-in **`provider.estimate_eip1559_fees()`** (cleaner; the user's instruction). Either
  yields the EIP-1559 pair.
- `*Call::abi_encode()` (e.g. `Coordinator::keyGenConfirmCall { … }.abi_encode()`) is how `sol!`
  generates typed calldata — useful wherever the crate builds transactions.

---

## 4. Block-keyed state snapshots (feeds Phase D)

The spike already has the **snapshot history + pruning** half of the reorg-aware store — but **no
`rollback_to`** and it is `rusqlite`/JSON-`TEXT`. The epic must port the schema to `sqlx` and add
rollback.

```rust
// state/storage.rs (rusqlite)
const DEFAULT_STATE_HISTORY: NonZeroU64 = NonZeroU64::new(5).unwrap();  // == maxReorgDepth
// schema:
//   CREATE TABLE IF NOT EXISTS validator_state (
//     block_number INTEGER PRIMARY KEY,
//     state_json   TEXT NOT NULL
//   );

pub fn save(&self, block_number: u64, state: &ValidatorState) -> Result<()> {
    let json = serde_json::to_string(state)?;
    self.conn.execute("INSERT INTO validator_state (block_number, state_json) VALUES (?1, ?2)",
                      params![i64::try_from(block_number)?, json])?;
    if let Some(cleanup) = block_number.checked_sub(self.history.get()) {   // prune old snapshots
        self.conn.execute("DELETE FROM validator_state WHERE block_number <= ?1",
                          params![i64::try_from(cleanup)?])?;
    }
    Ok(())
}

pub fn load_latest(&self) -> Result<Option<ValidatorState>> {
    // SELECT state_json FROM validator_state ORDER BY block_number DESC LIMIT 1
}
```

Maps to PR D2:

- `save` ≈ `commit(BlockRef, &S)`; `load_latest` ≈ `current()`; the `DELETE … <= block-history`
  is exactly `prune(finalized_below)`. Default history **5** answers Open Question #5.
- **Add** `rollback_to(block_number)`: `DELETE FROM … WHERE block_number > ?1` then return the new
  tip (`load_latest`). This is the net-new reorg half the spike lacks.
- Plan stores `state BLOB`; the spike uses `state_json TEXT` (human-debuggable). Either is fine —
  recommend keeping JSON `TEXT` to match the spike and ease debugging. Store `block_hash` too (the
  spike omits it) so reorgs can match by hash, not just number.

---

## 5. Orchestration wiring (`driver.rs`, feeds F2 e2e)

`Driver::on_update` is the analog of the TS `BlockchainWatcher` handler:

```rust
fn on_update(&mut self, update: watcher::Update) {
    let mut actions = self.state.on_block(update.block_number);
    for event in update.events {
        actions.extend(match event {
            watcher::Event::Consensus(e)   => self.state.on_consensus_event(e),
            watcher::Event::Coordinator(e) => self.state.on_coordinator_event(e),
        });
    }
    self.actions.handle(actions);
    if let Err(err) = self.storage.save(update.block_number, &self.state) { /* warn */ }
}
```

`run()` shows the bootstrap order: `provider::create` → `PrivateKeySigner::from_bytes` →
`Chain::load` → `Addresses::load` → `Storage::open` → restore via `load_latest()` or init from
on-chain `getActiveEpoch()` → compute `start_block = last_seen_block + 1` → `watcher::run(.., on_update)`.

---

## 6. Logging init (`main.rs`, feeds PR B1)

```rust
tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::try_new(&cli.log_level)?)
    .init();
```

Minimal — no JSON/pretty switch, no TTY detection. PR B1 should extend this (JSON-vs-pretty by TTY,
default `RUST_LOG`, structured fields) to match the TS validator's JSON-to-stdout behaviour.

---

## Divergence checklist (spike → `safenet-core`)

| Concern        | `project/ports` spike                        | `safenet-core` epic                                  |
| -------------- | -------------------------------------------- | ---------------------------------------------------- |
| Error type     | `anyhow`                                     | `thiserror` (library)                                |
| SQLite         | `rusqlite` (bundled)                         | `sqlx` (async)                                       |
| Reorgs         | none (monotonic assert)                      | detect + per-block-snapshot rollback (net-new)       |
| Tx management  | fire-and-forget, no store                    | nonce store + resubmit + fee bump (port from TS)     |
| Fee estimation | `get_gas_price`+`get_max_priority_fee`       | `estimate_eip1559_fees`                              |
| Metrics        | none                                         | Prometheus (`metrics-exporter-prometheus`)           |
| Indexer        | hand-written `Event` enum + address dispatch | same pattern, generalised behind a `DecodeLog` trait |
| Config         | TOML file + `argh`                           | consumer concern (out of scope)                      |
