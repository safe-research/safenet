# Porting Notes

This file tracks the status of porting `validator/` from TypeScript to Rust in `validator-rust/`.

> **Agents: keep this file up to date.** Whenever a module is ported, a decision is made, or the porting scope changes, update the relevant section here. This document is the single source of truth for port status; stale entries cause confusion for future agents picking up this work.

## Port Goals

The Rust port aims to reach consensus "happy path" compatibility with the TypeScript validator — enough to attest a transaction. Error conditions (timeouts, complaint flows, RPC retry logic) may be handled with less robustness than the TypeScript implementation. The port is primarily for language evaluation purposes.

## Current Rust State

Implemented modules in `validator-rust/src/`:

| Module            | File                   | Status  | Notes                                                                                                                                          |
| ----------------- | ---------------------- | ------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| CLI & startup     | `main.rs`              | Done    | `argh` CLI, tracing init, TOML config load, driver invocation                                                                                  |
| Config            | `config.rs`            | Done    | `serde` + `toml` deserialization, optional fields, `#[serde(deny_unknown_fields)]`; uses `PathBuf` and `NonZeroU64` for strong types           |
| Contract bindings | `bindings.rs`          | Partial | All consensus + coordinator events with `Debug`; `getCoordinator()` and `getActiveEpoch()` calls; keygen tx functions not yet added            |
| Actions           | `actions.rs`           | Partial | `Action` enum (empty); `Handler` struct that processes actions produced by state transitions                                                   |
| State             | `state/mod.rs`         | Partial | `Phase` enum (`WaitingForGenesis` / `WaitingForRollover`); `ValidatorState` with serde derives; handles events and returns `Vec<Action>`       |
| State storage     | `state/storage.rs`     | Done    | SQLite-backed JSON persistence; `open`/`save`/`load_latest`; keeps last `state_history` entries by block number; defaults to `:memory:`        |
| Driver            | `driver.rs`            | Partial | Async init: opens storage, restores state or queries `getActiveEpoch()` for cold start; persists state after each block; dispatches actions    |
| Watcher           | `watcher.rs`           | Partial | Block + log subscription; decodes logs via `SolEventInterface`; block monotonicity check; log sort by index; dispatches to `Driver`; no reorg  |

Not yet ported (TypeScript → Rust):

| TypeScript Domain            | TS Path                                                | Status      |
| ---------------------------- | ------------------------------------------------------ | ----------- |
| FROST primitives             | `validator/src/frost/`                                 | Not started |
| Consensus protocol clients   | `validator/src/consensus/protocol/`                    | Not started |
| On-chain action queue        | `validator/src/consensus/protocol/onchain.ts`          | Not started |
| Packet verification engine   | `validator/src/consensus/verify/`                      | Not started |
| Merkle tree / proof          | `validator/src/consensus/merkle.ts`                    | Not started |
| Key generation state machine | `validator/src/machine/keygen/`                        | Not started |
| Signing state machine        | `validator/src/machine/signing/`                       | Not started |
| Consensus state machine      | `validator/src/machine/consensus/`                     | Not started |
| Transition watcher           | `validator/src/machine/transitions/`                   | Not started |
| SQLite storage               | `validator/src/consensus/storage/`, `machine/storage/` | Done (simplified) |
| Safe transaction checks      | `validator/src/service/checks.ts`                      | Not started |
| Participant utilities        | `validator/src/utils/participants.ts`                  | Not started |

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

- Use `frost-secp256k1` for all FROST cryptography. Avoid writing any custom cryptographic code — the crate implements all required primitives (DKG, signing rounds, hash-to-scalar, polynomial evaluation, VSS, proof-of-knowledge). It re-exports `k256` internally; do not add a separate `k256` dependency. The crate does not natively speak the ABI `Point { uint256 x; uint256 y }` format used by the FROSTCoordinator contract, so marshalling helpers are required:
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

## Current Mission: Genesis Key Generation

The immediate goal is to implement the happy-path genesis key generation ceremony. Complaint handling, timeouts, and keygen restarts can all be skipped. The flow ends when all participants have confirmed on-chain and are ready to preprocess nonces for signing.

### Genesis Keygen Happy-Path State Machine

```
waiting_for_genesis
  → on KeyGen event where gid matches computed genesis group ID
collecting_commitments
  → each participant submits keyGenAndCommit (or keyGenCommit) on-chain
  → on all KeyGenCommitted events received: compute group key, publish secret shares
collecting_shares
  → each participant submits keyGenSecretShare on-chain
  → on all KeyGenSecretShared events received: compute signing share, confirm
collecting_confirmations
  → each participant submits keyGenConfirm on-chain
  → on all KeyGenConfirmed events received: genesis complete
```

After genesis completes, participants register nonce preprocessings (`preprocess` contract call) to enable future signing.

### Phase 0 — Genesis Trigger

Triggered by a `KeyGen` coordinator event whose `gid` matches the locally computed genesis group ID:

```
genesisGroupId = keccak256(participantsRoot || count || threshold || context)
participantsRoot = merkleRoot(sortedPaddedAddresses)
threshold = floor(n/2) + 1
context = keccak256("genesis" || genesisSalt)   // or zeroHash if no salt
```

Participants are sorted by their FROST participant ID `hid(address)` (not by address directly). TS source: `validator/src/machine/keygen/group.ts`, `genesis.ts`.

### Phase 1 — Commitment

Each participant:

1. Samples a random secret polynomial of degree `threshold-1`: coefficients `[a₀, …, a_{t-1}]`, each derived via `hdkg(random32)`.
2. Computes public commitments `Cᵢ = [g^{a₀}, …, g^{a_{t-1}}]`.
3. Generates a Schnorr proof-of-knowledge of `a₀`:
   - `k = hpok(random32)`, `R = g^k`
   - `c = hdkg(participantId || g^{a₀} || R)`
   - `μ = k + a₀·c (mod N)`
4. Generates an ECDH encryption keypair: `sk_enc = hdkg(random32)`, `pk_enc = g^{sk_enc}`.
5. Computes a Merkle proof-of-attestation-participation (`poap`) showing they are in the participant list.
6. Calls `keyGenAndCommit` (first participant to start the group) or `keyGenCommit` (subsequent participants) on the Coordinator contract.

On receiving all `KeyGenCommitted` events:

- Verify each peer's proof-of-knowledge: `R == g^μ - g^{a₀}^c` where `c` is recomputed locally.
- Compute group public key: `Y = Σ g^{a₀_k}` (sum of first commitments from all participants).
- Compute own verification share: `y_i = Σₖ evalCommitment(Cₖ, i)`.
- Evaluate polynomial for each peer `j`: `f_i(j) = evalPoly([a₀,…,a_{t-1}], j)`.
- Encrypt each share: `enc_j = f_i(j) XOR ecdh(pk_enc_j, sk_enc_i)` where `ecdh = (pk_enc_j^{sk_enc_i}).x`.
- Submit `keyGenSecretShare` on-chain with `(y_i, [enc_j, …])`.

TS source: `validator/src/machine/keygen/trigger.ts`, `committed.ts`, `validator/src/consensus/keyGen/client.ts`.

### Phase 2 — Secret Shares

On each `KeyGenSecretShared` event from peer `k`:

- Decrypt: `f_k(i) = enc_i XOR ecdh(pk_enc_i, sk_enc_k)`.
- Verify: `g^{f_k(i)} == evalCommitment(Cₖ, i)`.

Once all shares are received:

- Compute signing share: `x_i = Σₖ f_k(i)` (sum of all received shares).
- Sanity check: `g^{x_i} == y_i`.
- Submit `keyGenConfirm` on-chain.

Skip invalid shares / complaint logic entirely for the happy path. TS source: `validator/src/machine/keygen/secretShares.ts`.

### Phase 3 — Confirmations

On each `KeyGenConfirmed` event, track who has confirmed. Once all participants have confirmed, genesis is complete. TS source: `validator/src/machine/keygen/confirmed.ts`.

### Cryptographic Primitives

Use `frost-secp256k1` directly — do not reimplement hashes, polynomial math, VSS, or proof-of-knowledge. The crate's DKG module (`frost_secp256k1::keys::dkg`) covers the full keygen ceremony. The hash functions (`hid`, `hdkg`, `hpok`, `h2`) are internal to the crate and invoked automatically through its API.

The only custom code needed is:

- **ABI marshalling** — converting between `frost-secp256k1` points and the ABI `Point { uint256 x; uint256 y }` (see Deliberate Porting Decisions).
- **ECDH share encryption** — encrypting/decrypting secret shares with `f_k(i) XOR (peer_pk^{our_sk}).x`. This uses point multiplication available via the re-exported `k256` types, but is not crypto we invented — it directly follows the TypeScript in `validator/src/frost/secret.ts`.
- **Merkle tree** — Safenet-specific; leaves are `keccak256(address)` on addresses sorted by FROST participant ID. The proof (`poap`) is passed to the contract. TS source: `validator/src/consensus/merkle.ts`.

### Contract Calls to Add to `bindings.rs`

The following Coordinator functions from `validator/src/types/abis.ts` are needed for keygen:

```solidity
function keyGenAndCommit(bytes32 participants, uint16 count, uint16 threshold, bytes32 context, bytes32[] poap, ((uint256 x, uint256 y) q, (uint256 x, uint256 y)[] c, (uint256 x, uint256 y) r, uint256 mu) commitment) external
function keyGenCommit(bytes32 gid, bytes32[] poap, ((uint256 x, uint256 y) q, (uint256 x, uint256 y)[] c, (uint256 x, uint256 y) r, uint256 mu) commitment) external
function keyGenSecretShare(bytes32 gid, ((uint256 x, uint256 y) y, uint256[] f) share) external
function keyGenConfirm(bytes32 gid) external
```

### Implementation Order

1. **ABI marshalling** (`src/frost/mod.rs` or `src/frost/marshal.rs`) — helpers to convert `frost-secp256k1` points to/from the ABI `Point { uint256 x; uint256 y }`. Test against known values.
2. **ECDH share encryption** (`src/frost/secret.rs`) — `encrypt(msg, sender_sk, receiver_pk)` and `decrypt` using `(receiver_pk^{sender_sk}).x` as the one-time pad. Port directly from `validator/src/frost/secret.ts`.
3. **Merkle tree** (`src/frost/merkle.rs`) — leaf hashing, tree construction, proof generation. Port with test vectors from `validator/src/consensus/merkle.ts`.
4. **Participant utilities** (`src/frost/participants.rs`) — derive participant ID via `frost-secp256k1` hash-to-scalar, sort by ID, compute group ID.
5. **Contract bindings** — add the four keygen functions to `bindings.rs`.
6. **Keygen state machine** (`src/state/mod.rs`) — extend `Phase` with keygen sub-phases; store `frost-secp256k1` DKG round state in `ValidatorState` (in-memory; persisted automatically by the existing storage layer).
7. **Actions** — add `Action` variants for each on-chain submission; implement `Handler::handle` to submit them via the provider.

## Suggested Next Steps

1. Add `frost-secp256k1` to `Cargo.toml`. No separate `k256` dependency is needed.
2. Implement ABI marshalling helpers and test against known values.
3. Implement ECDH share encryption (`src/frost/secret.rs`) following `validator/src/frost/secret.ts`.
4. Implement Merkle tree (`src/frost/merkle.rs`) with test vectors from the TypeScript implementation.
5. Implement participant utilities (`src/frost/participants.rs`): participant ID derivation, address sorting, group ID computation.
6. Add the four keygen contract function bindings to `bindings.rs`.
7. Extend `ValidatorState` with keygen phase tracking; DKG round state is stored in-memory and will be persisted automatically by the existing storage layer on each block.
8. Add `Action` variants for on-chain keygen submissions and implement the handler.

## Verification Commands

The repository root is a Cargo workspace that includes `validator-rust`. All `cargo` commands work from there:

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
