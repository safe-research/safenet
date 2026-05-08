# Port Reference

This file captures the Safenet protocol scope implemented in the Rust validator port (`validator-rust/`). It serves as the target specification for any subsequent port (e.g. Go).

## Scope

The port implements the genesis key generation happy path: a validator can cold-start, join a DKG ceremony, and end up with a usable FROST signing share. Error paths (timeouts, complaint flows, reorg handling, RPC retries) are not implemented.

## Config

Required fields:

```toml
rpc_url = "http://127.0.0.1:8545"
private_key = "0x..."
consensus_address = "0x..."

[[participants]]
address = "0x..."
```

Optional fields:

```toml
storage_file = "validator.sqlite"   # defaults to in-memory SQLite
staker_address = "0x..."
genesis_salt = "0x000...0"
blocks_per_epoch = 1440             # overrides chain default
block_time_override = 5
state_history = 5                   # how many state snapshots to retain
```

Chains supported: Gnosis (1440 blocks/epoch), Sepolia (600), Anvil (60). Chain is detected from the RPC at startup.

The validator's own address is derived from `private_key`. `consensus_address` is used to query the coordinator contract address at startup. All consensus parameters (`own_address`, `coordinator_address`, `participants`, `genesis_salt`, `blocks_per_epoch`) are bundled into a `ConsensusConfig` and stored inside the persisted state.

## Implemented Components

### Watcher

Subscribes to new blocks and logs from the consensus contract. On startup, replays history from a configurable `start_block` via `eth_getLogs`, then switches to live subscription. Logs are sorted by index within each block. Block monotonicity is enforced. No reorg handling.

### State Machine

State machine phases:

- `WaitingForGenesis` — idle until a `KeyGen` event matching the computed genesis group ID arrives.
- `CollectingCommitments` — accumulates `KeyGenCommitted` events from all participants.
- `CollectingShares` — accumulates `KeyGenSecretShared` events from all participants.
- `GenesisComplete { key_package }` — holds the validator's FROST signing share.
- `WaitingForRollover` — post-genesis, not yet implemented beyond the state variant.

State is persisted to SQLite as JSON snapshots keyed by block number. The last `state_history` entries are retained.

### Genesis Keygen State Machine (Happy Path)

**Phase 0 — Genesis trigger**

On a `KeyGen` coordinator event whose group ID matches `calc_genesis_group_id(participants, genesis_salt)`: run DKG round 1, transition to `CollectingCommitments`, emit `KeyGenAndCommit` action.

**Phase 1 — Commitment**

Accumulate `KeyGenCommitted` events keyed by participant address. Once all participants have committed: run DKG round 2, transition to `CollectingShares`, emit `KeyGenSecretShare` action.

**Phase 2 — Secret shares**

Accumulate `KeyGenSecretShared` events keyed by participant address (skip any where `shared == false`). Once all shares are received: run DKG round 3 (ECDH-decrypt each peer's share, call `dkg::part3`), transition to `GenesisComplete { key_package }`, emit `KeyGenConfirm` action.

**Phase 3 — Confirm**

`KeyGenConfirm` encodes a `keyGenConfirm(gid)` call on the coordinator contract. The validator transitions to `GenesisComplete` as soon as its own signing share is ready, without waiting for `KeyGenConfirmed` events from peers.

### FROST / DKG

- DKG rounds 1 and 2 implemented using `frost-secp256k1`.
- ECDH key agreement for encrypting secret shares between participants (k256 point multiplication + XOR).
- ABI marshalling between the contract's `Point { uint256 x; uint256 y }` format and SEC1-encoded curve points.
- Participant utilities: `calc_participants_root`, `calc_genesis_group_id`, `generate_participant_proof`, `identifier(address)`.

### Actions / On-Chain Calls

Three action types: `KeyGenAndCommit`, `KeyGenSecretShare`, `KeyGenConfirm`. The action handler builds and signs EIP-1559 transactions but **does not broadcast them** — `send_raw_transaction` is not called.

### Contract Bindings

All consensus and coordinator events with debug support. Calls: `getCoordinator()`, `getActiveEpoch()`, `keyGenAndCommit()`, `keyGenSecretShare()`, `keyGenConfirm()`. `KeyGenCommitment` includes the ECDH encryption public key `q`.

## Known Gaps

- Transactions are signed but not broadcast.
- Proof-of-knowledge verification is skipped.
- `KeyGenConfirmed` events from peers are not tracked.
- Signing state machine not implemented.
- Epoch rollover not implemented.
- Safe transaction checks not implemented.
- Consensus packet verification not implemented.

## Go Port Plan

The Go port (`validator-go/`) targets the same scope as the Rust port: genesis keygen happy path, ending in a usable FROST signing share. The plan is structured as a dependency graph of small, independently-reviewable PRs. Phases A and F are sequential; B/C/D/E can overlap once A is done.

### Phase A — Foundation (sequential)

**A1. Module skeleton + config loading.** ✅ `validator-go/` Go module, CLI entry point that takes a config file path, TOML config parsing with required + optional fields. Address fields parsed via go-ethereum's `common.Address`. No business logic; running the binary loads and validates a config.

**A2. Chain detection + addresses.** ✅ Chain type with Gnosis/Sepolia/Anvil and their `BlocksPerEpoch`. Connect to RPC, query `eth_chainId`, resolve to chain. Query the consensus contract for the coordinator address. Bundle both into an addresses struct.

**A3. Contract bindings.** ✅ Generate Go bindings (e.g. via `abigen`) for the `Consensus` and `FROSTCoordinator` contracts. Verify event decoding and the read calls used by the watcher and actions: `getCoordinator`, `getActiveEpoch`, plus the three keygen calls.

### Phase B — Crypto primitives (parallelizable after A3)

Each B step is self-contained with unit tests against fixed vectors (port the Rust/TS tests where they exist).

**B1. Participant utilities.** ✅ `IdentifierFromAddress` (FROST `hid` hash-to-scalar), `CalcParticipantsRoot`, `CalcGenesisGroupId`, `GenerateParticipantProof`.

**B2. ECDH encryption.** ✅ Keypair generation, encrypt/decrypt of 32-byte shares via secp256k1 point multiplication + XOR. Round-trip tests and cross-implementation vectors.

**B3. ABI Point marshalling.** ✅ Convert between the contract `Point { uint256 x; uint256 y }` ABI shape and SEC1 curve points. Round-trip tests.

**B4. FROST math helpers.** ✅ `EvalPoly`, `EvalCommitment`, `CreateVerificationShare`, `CreateSigningShare`, `VerifyKey`. Tests against TS vectors.

### Phase C — DKG rounds (sequential, depends on B)

Each C step uses the chosen Go FROST library (or a vendored implementation) plus the B primitives.

**C1. DKG round 1.** `GenerateRound1`: create ECDH keypair, run FROST DKG part 1, build the on-chain `KeyGenCommitment` payload (incl. encryption pubkey `q`).

**C2. DKG round 2.** `GenerateRound2`: run FROST DKG part 2, derive verifying shares from collected commitments, ECDH-encrypt per-participant signing shares into `KeyGenSecretShare` payloads.

**C3. DKG round 3.** `GenerateRound3`: ECDH-decrypt peers' shares, run FROST DKG part 3, return a `KeyPackage`.

### Phase D — State & storage (parallelizable after A1)

**D1. State types.** Phase sum type covering `WaitingForGenesis`, `CollectingCommitments`, `CollectingShares`, `GenesisComplete`, `WaitingForRollover`. `ConsensusConfig` struct holding `OwnAddress`, `CoordinatorAddress`, `Participants`, `GenesisSalt`, `BlocksPerEpoch`. JSON serialization round-trip tests for every variant.

**D2. SQLite storage.** `validator_state` table keyed by `block_number` with a JSON state column. `Open`, `Save`, `LoadLatest`. Pruning to keep the last `state_history` rows. Default to in-memory database. Tests covering save → load and pruning behaviour.

### Phase E — I/O (parallelizable after A3)

**E1. Watcher.** Block + consensus-log subscription, history replay through `eth_getLogs` from a given start block, log sort by index, block monotonicity enforcement. Emits one update per block to a callback. Subscription is established before history replay so no blocks are missed.

**E2. Action handler with broadcast.** Builds, signs, **and broadcasts** EIP-1559 transactions for `KeyGenAndCommit`, `KeyGenSecretShare`, `KeyGenConfirm`. Closes the gap left in the Rust port (`send_raw_transaction` is wired up here from the start).

### Phase F — Integration (sequential)

**F1. State machine handlers.** Pure functions taking `(state, event)` and returning `(new state, actions)`. One handler per event the keygen flow consumes: `KeyGen`, `KeyGenCommitted`, `KeyGenSecretShared`. Unit-tested with synthetic event sequences and assertions on emitted actions and resulting phase. Depends on C3 + D1 + B1.

**F2. Driver wiring.** Cold-start path: derive `OwnAddress` from the private key, load addresses (A2), build `ConsensusConfig`, initialise or restore state via storage (D2), spawn the action-handler worker (E2), and start the watcher (E1) feeding into the state machine handlers (F1).

**F3. Integration verification.** Run the Go validator against `scripts/run_integration_test.sh` (or an equivalent harness) and confirm the genesis ceremony completes with `Phase = GenesisComplete` for every participant. No new code beyond test harness adjustments; the goal is to validate end-to-end behaviour matches the TS and Rust validators.
