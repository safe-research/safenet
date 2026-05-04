# Porting Notes

This file tracks the status of porting `validator/` from TypeScript to Rust in `validator-rust/`.

> **Agents: keep this file up to date.** Whenever a module is ported, a decision is made, or the porting scope changes, update the relevant section here. This document is the single source of truth for port status; stale entries cause confusion for future agents picking up this work.

## Port Goals

The Rust port aims to reach consensus "happy path" compatibility with the TypeScript validator — enough to attest a transaction. Error conditions (timeouts, complaint flows, RPC retry logic) may be handled with less robustness than the TypeScript implementation. The port is primarily for language evaluation purposes.

## Current Rust State

Implemented modules in `validator-rust/src/`:

| Module                | File                    | Status  | Notes                                                                                                                                                                                                                       |
| --------------------- | ----------------------- | ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| CLI & startup         | `main.rs`               | Done    | `argh` CLI, tracing init, TOML config load, driver invocation                                                                                                                                                               |
| Config                | `config.rs`             | Done    | `serde` + `toml` deserialization, optional fields, `#[serde(deny_unknown_fields)]`; sub-modules `config::chain`, `config::addresses`, `config::provider`                                                                    |
| Chain                 | `config/chain.rs`       | Done    | `Chain` enum (Gnosis/Sepolia/Anvil); `Chain::new(chain_id)?` fails for unknown chains; `blocks_per_epoch()` returns 1440/600/60; `id()` for EIP-1559 chain ID field                                                         |
| Addresses             | `config/addresses.rs`   | Done    | `Addresses::load(&provider, consensus_address)` queries coordinator address; bundles both for use by watcher and action handler                                                                                             |
| Contract bindings     | `bindings.rs`           | Done    | All consensus + coordinator events with `Debug`; `getCoordinator()`, `getActiveEpoch()`, `keyGenAndCommit()`, `keyGenSecretShare()`, `keyGenConfirm()` calls; `KeyGenCommitment` includes encryption pubkey `q`             |
| Actions               | `actions.rs`            | Partial | `Action::KeyGenAndCommit`, `Action::KeyGenSecretShare`, `Action::KeyGenConfirm`; background worker builds and **signs** EIP-1559 transactions but does not yet broadcast them (missing `send_raw_transaction` call)         |
| State                 | `state/mod.rs`          | Done    | `Phase` has `WaitingForGenesis`, `CollectingCommitments`, `CollectingShares`, `GenesisComplete { key_package }`, `WaitingForRollover`; all keygen phases 0–3 implemented; both commitment and share maps keyed by `Address` |
| State storage         | `state/storage.rs`      | Done    | SQLite-backed JSON persistence; `open`/`save`/`load_latest`; keeps last `state_history` entries by block number; defaults to `:memory:`                                                                                     |
| Driver                | `driver.rs`             | Done    | Loads chain + addresses; derives own address from private key; cold-starts or restores state; spawns action handler worker; passes provider + addresses to watcher                                                          |
| Watcher               | `watcher.rs`            | Partial | Block + log subscription; history replay via `start_block` (`eth_getLogs` range then monotonic updates); block monotonicity check; log sort by index; dispatches to `Driver`; no reorg handling                             |
| Participant utilities | `frost/participants.rs` | Done    | `calc_participants_root`, `calc_genesis_group_id` (tested); `generate_participant_proof`; `identifier(address)` via FROST `hid`                                                                                             |
| DKG rounds 1 & 2      | `frost/keygen.rs`       | Done    | `generate_round1`: ECDH keypair + `dkg::part1` → `KeyGenCommitment`; `generate_round2`: `dkg::part2` + verifying share via `PublicKeyPackage::from_dkg_commitments` + ECDH-encrypted signing shares → `KeyGenSecretShare`   |
| VSS math utilities    | `frost/math.rs`         | Done    | `g()`, `eval_poly()`, `eval_commitment()`, `create_verification_share()`, `ecdh()`, `create_signing_share()`, `verify_key()`                                                                                                |
| ECDH encryption       | `frost/secret.rs`       | Done    | `EncryptionKey::generate`, `encrypt`, `decrypt` using k256 point multiplication + U256 XOR; tested against TypeScript spec                                                                                                  |
| ABI marshalling       | `frost/marshal.rs`      | Done    | `solidity_commitment()`, `frost_commitment()`, `solidity_secret_share()`, `solidity_point()`, `abi_point_to_affine()`, `abi_point_to_projective()`                                                                          |

Not yet ported (TypeScript → Rust):

| TypeScript Domain            | TS Path                                                | Status            |
| ---------------------------- | ------------------------------------------------------ | ----------------- |
| ABI point marshalling        | (new — no TS equivalent)                               | Not started       |
| ECDH share encryption        | `validator/src/frost/secret.ts`                        | Done              |
| Consensus protocol clients   | `validator/src/consensus/protocol/`                    | Not started       |
| On-chain action queue        | `validator/src/consensus/protocol/onchain.ts`          | Not started       |
| Packet verification engine   | `validator/src/consensus/verify/`                      | Not started       |
| Key generation state machine | `validator/src/machine/keygen/`                        | Done              |
| Signing state machine        | `validator/src/machine/signing/`                       | Not started       |
| Consensus state machine      | `validator/src/machine/consensus/`                     | Not started       |
| Transition watcher           | `validator/src/machine/transitions/`                   | Not started       |
| SQLite storage               | `validator/src/consensus/storage/`, `machine/storage/` | Done (simplified) |
| Safe transaction checks      | `validator/src/service/checks.ts`                      | Not started       |

## Deliberate Porting Decisions

The Rust port does not need to preserve the TypeScript validator's configuration interface exactly. Prefer idiomatic Rust and strong types over env-var compatibility.

- Config uses snake_case TOML keys, not the TypeScript screaming-case env vars.
- Config deserializes directly with `serde`; avoid intermediate raw config types unless there is a concrete need.
- Do not add custom config error types unless a caller genuinely needs them; `toml::de::Error` is fine for now.
- Use Alloy primitives directly where possible: `Address`, `B256`.
- Address checksum enforcement is intentionally not custom-config behavior. Use Alloy's default `Address` deserialization.
- `rpc_url` is `url::Url`, not `String`.
- Metrics config was intentionally omitted. Do not port metrics just to match TypeScript.
- SQLite schema, storage layout, and operational plumbing may differ from TypeScript if the Rust design is cleaner. The Rust storage uses a single `validator_state` table with `block_number` as the primary key and the entire `ValidatorState` serialized as a JSON column. The last `state_history` entries are retained; older rows are pruned on each write. This differs from the TypeScript design (separate tables per state type) but is simpler and sufficient for evaluation.
- Error conditions (timeouts, complaint flows, RPC retries) can be handled with less robustness than TypeScript — the watcher, for example, handles far fewer edge cases; this is acceptable.
- Do not try to architect complex and robust solutions. Prefer "do only what is needed" for evaluation purposes.
- Alloy has two distinct `Log` types that are **not** interchangeable and cause confusing type errors:
  - `alloy::rpc::types::Log` — the RPC response type returned by `provider.get_logs()`. Has metadata fields (block number, tx hash, etc.). Call `.into_inner()` to unwrap it.
  - `alloy::primitives::Log` — the bare log type expected by `SolEventInterface::decode_log()`.

  To decode an RPC log into a generated event enum, consume the RPC log and unwrap it first:

  ```rust
  use alloy::sol_types::SolEventInterface;

  fn decode_consensus_log(log: Log) -> Result<Consensus::ConsensusEvents> {
      Ok(Consensus::ConsensusEvents::decode_log(&log.into_inner())
          .context("failed to decode consensus log")?
          .data)
  }
  ```

  Import `SolEventInterface` (not `SolEvent`) from `alloy::sol_types` to get the `decode_log` method on the generated `{Contract}Events` enum.

- Use `frost-secp256k1` for all FROST cryptography. Avoid writing any custom cryptographic code — the crate implements all required primitives (DKG, signing rounds, hash-to-scalar, polynomial evaluation, VSS, proof-of-knowledge). It re-exports `rand_core`; use `OsRng` from it without adding a separate `rand_core` dependency. It does not re-export `k256` at the top level. The crate does not natively speak the ABI `Point { uint256 x; uint256 y }` format used by the FROSTCoordinator contract, so marshalling helpers are required:
  - **From contract → FROST**: parse the two `U256` fields as big-endian 32-byte arrays, concatenate as `04 || x || y` (uncompressed SEC1), and call `k256::AffinePoint::from_encoded_point` (re-exported through `frost_secp256k1`).
  - **From FROST → contract**: call `.to_encoded_point(false)` (uncompressed), then split the 65-byte result into the `04` prefix (discarded) and the 32-byte `x` / `y` fields, which map to the ABI `uint256` words.

- To derive standard traits (e.g. `Debug`) on types generated by the `sol!` macro, place `#[derive(...)]` directly on the struct or contract definition inside the macro block. For example:

  ```rust
  sol! {
      #[derive(Debug)]
      struct Point { uint256 x; uint256 y; }

      #[sol(rpc)]
      #[derive(Debug)]
      contract Consensus { ... }
  }
  ```

  Annotating a contract derives the trait on the generated `{Contract}Events` enum and all other contract-level generated types.

- Always use `keyGenAndCommit` (not `keyGenCommit`) — both paths do the same thing and using one reduces code paths.

- `ConsensusConfig` is stored inside `ValidatorState` and serialized alongside it. It holds all consensus-relevant parameters (`own_address`, `coordinator_address`, `participants`, `genesis_salt`, `blocks_per_epoch`) so the state machine has access to them without threading config through every event handler. It is populated once at cold start in `driver.rs`.

- `blocks_per_epoch` is not optional in `ConsensusConfig`. It defaults to the chain's value via `Chain::blocks_per_epoch()` when not overridden in the TOML config. The chain is detected at cold start via `provider.get_chain_id()`.

- `own_address` is derived from `private_key` via `alloy::signers::local::PrivateKeySigner::from_bytes`. The alloy `signer-local` feature must be enabled.

- `round1::SecretPackage` and `round2::SecretPackage` are stored using the crate's built-in `serialize()` method (the `serialization` feature is on by default). They live in their respective `Phase` variants and are persisted automatically by the existing storage layer.

- Commitment and share maps in `Phase::CollectingCommitments` and `Phase::CollectingShares` are keyed by `Address` (not `Identifier`) to match the participant key used in on-chain events. Conversion to `Identifier`-keyed maps happens at the call sites for `generate_round2` and `generate_round3` in `frost/keygen.rs`.

- The watcher history replay (`start_block` parameter) fetches a range of logs with `eth_getLogs`, groups by block number, and emits one `Update` per block in order — empty `events` for blocks with no matching logs. The watcher subscribes to new blocks before fetching history so no blocks are missed between the two.

## Current Config Shape

Required fields:

```toml
rpc_url = "http://127.0.0.1:8545"
private_key = "0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe"
consensus_address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"

[[participants]]
address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
```

Optional fields:

```toml
storage_file = "validator.sqlite"
staker_address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
genesis_salt = "0x0000000000000000000000000000000000000000000000000000000000000000"
blocks_per_epoch = 1440
block_time_override = 5
state_history = 5
```

- `storage_file` deserializes as `PathBuf`; defaults to `:memory:` (in-process SQLite, no file) when omitted.
- `state_history` is a `NonZeroU64`; defaults to `5` when omitted.
- `blocks_per_epoch` overrides the chain default when provided; otherwise `Chain::blocks_per_epoch()` is used.

## TypeScript Source Map

Important TypeScript entry points:

- `validator/src/validator.ts` — original process entry point and env config wiring.
- `validator/src/service/service.ts` — service composition; creates clients, SQLite storage, protocol, state machine, watcher.
- `validator/src/types/schemas.ts` — original env-var schema. Use as a reference for supported knobs, not as an exact Rust API contract.
- `validator/src/types/interfaces.ts` — original `ProtocolConfig` and participant shape.
- `validator/src/types/chains.ts` — supported TS chains: Gnosis, Sepolia, Anvil.

Core domains:

- `validator/src/frost/` — FROST math, hashes, secret sharing, VSS.
- `validator/src/consensus/` — protocol clients, on-chain queue/tx logic, packet verification, Merkle hashing.
- `validator/src/machine/` — validator state machine, keygen, signing, consensus rollover, transition handling, storage.
- `validator/src/watcher/` — block/event watching with reorg handling and exponential backoff.
- `validator/src/service/checks.ts` — Safe transaction checks.

## Accuracy Priorities

The consensus implementation must be accurate. Configuration, database layout, logging, and process ergonomics may diverge from TypeScript, but consensus behavior should be ported with tests and careful comparison.

High-risk areas:

- FROST primitives and serialization.
- Key generation state transitions and complaint handling.
- Signing nonce/preprocess/share logic.
- Packet hashing for Safe transaction and epoch rollover verification.
- Merkle tree construction and proof semantics.
- On-chain protocol action ordering, deduplication, resubmission, and tx state handling.
- Epoch rollover participant selection and timing.

When porting consensus code, prefer porting or reproducing the TypeScript tests next to the Rust implementation. Use the TS tests as behavioral specs, but do not mechanically copy TypeScript structure when Rust types can make invalid states unrepresentable.

## Completed Mission: Genesis Key Generation ✓

The genesis key generation ceremony happy path is fully implemented. The state machine drives a validator from cold start through all four DKG phases and leaves it in `Phase::GenesisComplete { key_package }` holding a usable FROST signing share.

### Genesis Keygen Happy-Path State Machine

```
waiting_for_genesis
  → on KeyGen event where gid matches computed genesis group ID      ← DONE
collecting_commitments
  → each participant submits keyGenAndCommit on-chain               ← DONE
  → on all KeyGenCommitted events received: compute group key, publish secret shares
collecting_shares
  → each participant submits keyGenSecretShare on-chain             ← DONE
  → on all KeyGenSecretShared events received: compute signing share, confirm
genesis_complete
  → each participant submits keyGenConfirm on-chain                 ← DONE
```

### Phase 0 — Genesis Trigger ✓

The genesis group ID is computed in `frost::participants::calc_genesis_group_id`. On receiving a matching `KeyGen` coordinator event, `on_keygen` runs DKG round 1, transitions to `Phase::CollectingCommitments`, and emits `Action::KeyGenAndCommit`.

### Phase 1 — Commitment ✓

`on_keygen_committed` accumulates `KeyGenCommitted` events (keyed by `Address`). Once all participants have committed, it runs DKG round 2 via `keygen::generate_round2`, transitions to `Phase::CollectingShares` (carrying the encryption key and all commitments forward), and emits `Action::KeyGenSecretShare`.

### Phase 2 — Secret Shares ✓

`on_keygen_secret_shared` accumulates `KeyGenSecretShared` events (keyed by `Address`), skipping any where `event.shared == false`. Once all shares are received, it runs DKG round 3 via `keygen::generate_round3` (decrypts each peer's share with ECDH, calls `dkg::part3`, returns a `KeyPackage`), transitions to `Phase::GenesisComplete { key_package }`, and emits `Action::KeyGenConfirm`.

### Phase 3 — Confirm ✓

`Action::KeyGenConfirm` encodes a `keyGenConfirm(gid)` call on the coordinator. The validator does not wait for `KeyGenConfirmed` events from other participants; it transitions to `GenesisComplete` as soon as its own signing share is ready.

### Known gaps (acceptable for evaluation)

- `handle_action` in `actions.rs` signs EIP-1559 transactions with `sign_transaction_sync` but does not broadcast them — `send_raw_transaction` is not yet called.
- Proof-of-knowledge verification (`R == g^μ - g^{a₀}^c`) is skipped (happy path only).
- `KeyGenConfirmed` events from peers are not tracked; the validator does not wait for unanimous confirmation before considering genesis done.

## Suggested Next Steps

The keygen goal has been reached. If the port is extended further, the natural next areas are:

1. Fix `actions.rs`: call `provider.send_raw_transaction(...)` after `sign_transaction_sync` to actually broadcast transactions.
2. Implement signing: `validator/src/machine/signing/` — nonce preprocessing (`preprocess` call), signing round 1 & 2, `keyGenConfirm`-style confirm flow.
3. Epoch rollover: `validator/src/machine/consensus/` and `transitions/` — handle `EpochStaged`/`EpochProposed` events and transition out of `WaitingForRollover`.

## Verification Commands

The repository root is a Cargo workspace that includes `validator-rust`. All `cargo` commands work from there:

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
