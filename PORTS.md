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
