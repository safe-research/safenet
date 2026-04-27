# Porting Notes

This file tracks decisions and useful context for porting `validator/` from TypeScript to Rust in `validator-rust/`.

## Current Rust State

- `validator-rust` is a Cargo workspace member from the repository root.
- The binary currently only parses CLI flags, initializes tracing, reads a TOML config file, and deserializes it.
- CLI:
  - `--log-level <filter>` defaults to `info` and is passed directly to `tracing_subscriber::EnvFilter`.
  - `--config-file <path>` defaults to `validator.toml` in the current working directory.
- Rust modules:
  - `src/main.rs`: CLI, tracing setup, config file read.
  - `src/config.rs`: direct `serde` TOML config deserialization.
  - `src/chain.rs`: typed chain enum.
- Dependencies currently include `alloy`, `argh`, `serde`, `toml`, `tracing`, `tracing-subscriber`, and `url` with `serde`.

## Deliberate Porting Decisions

The Rust port does not need to preserve the TypeScript validator's configuration interface exactly. Prefer idiomatic Rust and strong types over env-var compatibility.

- Config uses snake_case TOML keys, not the TypeScript screaming-case env vars.
- Config deserializes directly with `serde`; avoid intermediate raw config types unless there is a concrete need.
- Do not add custom config error types unless a caller genuinely needs them; `toml::de::Error` is fine for now.
- Use Alloy primitives directly where possible:
  - `Address`
  - `B256`
- Address checksum enforcement is intentionally not custom-config behavior. Use Alloy's default `Address` deserialization.
- `rpc_url` is `url::Url`, not `String`.
- Chain selection is typed as `Chain`, not `u64`.
- Metrics config was intentionally removed for now. Do not port metrics just to match TypeScript.
- SQLite schema, storage layout, and operational plumbing may differ from TypeScript if the Rust design is cleaner.

## Current Config Shape

Example TOML:

```toml
storage_file = "validator.sqlite"
rpc_url = "http://127.0.0.1:8545"
private_key = "0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe"
staker_address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
consensus_address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
coordinator_address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
chain = "gnosis"
genesis_salt = "0x0000000000000000000000000000000000000000000000000000000000000000"
blocks_per_epoch = 1440
skip_genesis = false

[[participants]]
address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
active_from = 0

[[participants]]
address = "0x6Adb3baB5730852eB53987EA89D8e8f16393C200"
active_from = 10
active_before = 20
```

Supported chain names today:

- `gnosis`
- `sepolia`
- `alloy`

Add methods on `Chain` when code needs chain IDs, Alloy chain definitions, default block timing, or RPC-specific behavior.

## TypeScript Source Map

Important TypeScript entry points:

- `validator/src/validator.ts`: original process entry point and env config wiring.
- `validator/src/service/service.ts`: service composition; creates clients, SQLite storage, protocol, state machine, watcher.
- `validator/src/types/schemas.ts`: original env-var schema. Use as a reference for supported knobs, not as an exact Rust API contract.
- `validator/src/types/interfaces.ts`: original `ProtocolConfig` and participant shape.
- `validator/src/types/chains.ts`: supported TS chains: Gnosis, Sepolia, Anvil.

Core domains:

- `validator/src/frost/`: FROST math, hashes, secret sharing, VSS.
- `validator/src/consensus/`: protocol clients, on-chain queue/tx logic, packet verification, Merkle hashing.
- `validator/src/machine/`: validator state machine, keygen, signing, consensus rollover, transition handling, storage.
- `validator/src/watcher/`: block/event watching.
- `validator/src/service/checks.ts`: Safe transaction checks.

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
- Block/event watcher reorg behavior.

When porting consensus code, prefer porting or reproducing the TypeScript tests next to the Rust implementation. Use the TS tests as behavioral specs, but do not mechanically copy TypeScript structure when Rust types can make invalid states unrepresentable.

## Suggested Next Steps

1. Add Rust domain modules with empty public types matching the main concepts: `frost`, `consensus`, `machine`, `storage`, `watcher`.
2. Port low-level deterministic utilities first:
   - FROST field/scalar helpers.
   - Hashing helpers.
   - Merkle tree utilities.
   - Participant selection.
3. Bring over tests for each low-level module before service wiring.
4. Add SQLite only when a storage trait boundary is clear. It does not need to use the TypeScript schema.
5. Add chain/RPC wiring after the protocol/state-machine boundaries are typed.

## Verification Commands

From the repository root:

```sh
cargo test
cargo check
cargo clippy --all-targets -- -D warnings
```

If working only in the Rust crate:

```sh
cd validator-rust
cargo test
```
