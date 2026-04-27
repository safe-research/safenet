# Porting Notes

This file tracks the status of porting `validator/` from TypeScript to Rust in `validator-rust/`.

> **Agents: keep this file up to date.** Whenever a module is ported, a decision is made, or the porting scope changes, update the relevant section here. This document is the single source of truth for port status; stale entries cause confusion for future agents picking up this work.

## Port Goals

The Rust port aims to reach consensus "happy path" compatibility with the TypeScript validator — enough to attest a transaction. Error conditions (timeouts, complaint flows, RPC retry logic) may be handled with less robustness than the TypeScript implementation. The port is primarily for language evaluation purposes.

## Current Rust State

Implemented modules in `validator-rust/src/`:

| Module            | File          | Status  | Notes                                                                              |
| ----------------- | ------------- | ------- | ---------------------------------------------------------------------------------- |
| CLI & startup     | `main.rs`     | Done    | `argh` CLI, tracing init, TOML config load, watcher invocation                     |
| Config            | `config.rs`   | Done    | `serde` + `toml` deserialization, optional fields, `#[serde(deny_unknown_fields)]` |
| Contract bindings | `bindings.rs` | Partial | `Consensus::getCoordinator()` only; no signing or keygen ABI                       |
| Watcher           | `watcher.rs`  | Partial | Block + log subscription loop; no reorg handling, no event parsing                 |

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
| SQLite storage               | `validator/src/consensus/storage/`, `machine/storage/` | Not started |
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
- SQLite schema, storage layout, and operational plumbing may differ from TypeScript if the Rust design is cleaner.
- Error conditions (timeouts, complaint flows, RPC retries) can be handled with less robustness than TypeScript — the watcher, for example, handles far fewer edge cases; this is acceptable.
- Do not try to architect complex and robust solutions. Prefer "do only what is needed" for evaluation purposes.

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
```

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

## Suggested Next Steps

1. Extend `bindings.rs` with the ABI for all contract calls needed in the happy-path signing flow.
2. Add domain modules with empty public types matching the main concepts: `frost`, `consensus`, `machine`, `storage`.
3. Port low-level deterministic utilities first:
   - FROST field/scalar helpers.
   - Hashing helpers.
   - Merkle tree utilities.
   - Participant selection.
4. Bring over tests for each low-level module before service wiring.
5. Add SQLite only when a storage trait boundary is clear. It does not need to use the TypeScript schema.
6. Add chain/RPC wiring after protocol/state-machine boundaries are typed.
7. Wire the state machine into `watcher.rs` to handle emitted events.

## Verification Commands

The repository root is a Cargo workspace that includes `validator-rust`. All `cargo` commands work from there:

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
