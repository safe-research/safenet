# Safenet Developer Guide

Safenet is a decentralized Safe transaction security network that uses FROST (Flexible Round-Optimized Schnorr Threshold) signatures to validate Safe transactions before on-chain execution.

## Architecture

The repository is a hybrid monorepo with:

- `contracts/` — Solidity 0.8.30 smart contracts built with Foundry. Core contracts: `FROSTCoordinator.sol`, `Consensus.sol`, `Staking.sol`. Has no JavaScript of its own; its commands are `forge` invocations exposed via the root [Justfile](./Justfile).
- NPM packages (each with its own `package.json`/lockfile, no longer npm workspace members):
  - `examples/` — Scripts for interacting with the Safenet protocol on public testnets.
  - `explorer/` — React 19 + TypeScript + Vite frontend for inspecting network state.
- Rust crates:
  - `crates/core/` — Shared code used by all Safenet offchain services
  - `crates/sentinel/` — Rust sentinel service that watches the `SentinelOracle` and `Consensus` contracts, asks its configured sentinel engine to assess proposed transactions, and puts up bonds for its votes onchain
  - `crates/sentinel-engine/` — Keyless HTTP transaction-verification service that owns the sentinel's checks; it reads chain state but holds no bond and makes no onchain writes
  - `crates/validator/` — Rust port of the validator service that participates in FROST DKG and signing rounds and submits epoch rollovers and transaction attestations onchain

Additionally, formal verification specs live in `certora/`. Integration and devnet scripts are in `scripts/`.

## Documentation

For detailed architecture and technical documentation, refer to the local [docs](./docs/) folder. Additional documentation on the Safe smart account is available on the [Safe Foundation docs](https://docs.safefoundation.org). The MCP server related to this documentation can be found at <https://docs.safefoundation.org/mcp>.

## Coding Guidelines

Code SHOULD focus on security and maintainability. Existing code and components SHOULD be reused. New components SHOULD be written in a way that they can be reused. Refer to existing code to determine coding style and which implementation to choose. Do not re-invent the wheel and follow existing paradigms.

You MUST format, lint and test before committing.

- For JavaScript/Typescript code, run `just fix`, `just check`, and `npm --prefix <package> run test` (only `explorer/` has a `test` script) respectively
- For Rust code, run `cargo fmt --all`, `cargo clippy --package <package>`, and `cargo test --package <package>` respectively

## Testing Guidelines

New code SHOULD generally be tested. Design tests that do not require a high amount of churn with refactors (such as testing general behaviours and not implementation details). Your goal is not 100% test coverage (except for Solidity code). When modifying code the equivalent test MUST also be updated as required.

## Development Commands

### Project Setup

The steps for project setup are documented in the root [README.md](./README.md#project-setup).

Always use `npm ci` instead of `npm install` / `npm i`. `npm ci` installs exactly what is in the package's lockfile and never modifies it, keeping the lock file stable. Since `examples/` and `explorer/` each have their own `package.json`/lockfile, run it in each directory (e.g. `npm ci --prefix explorer`), not once at the repo root.

Make sure you have the correct tool versions (NodeJS 24, NPM 11, Foundry 1.5.1, [Just](https://github.com/casey/just)). Use `just foundryup` to set up the correct Foundry version.

### Just Commands

All repo-wide commands are exposed as recipes in the root [Justfile](./Justfile) — run `just --list` for the full set. `contracts/` has no `package.json` of its own; its recipes shell out to `forge` directly. Package-specific commands not exposed as a recipe can still be run directly, e.g. `npm --prefix explorer run <script>`.

Biome (the formatter/linter `just check`/`just fix` run) is a devDependency of `examples/` and `explorer/` individually, not a separate root package — `just check`/`just fix` run it once per directory, each scoped to that package's own `biome.json` (which `extends` the shared rules in the root [`biome.json`](./biome.json) without inheriting its `includes`), so it never scans outside that package. Both `npm ci --prefix examples` and `npm ci --prefix explorer` are required before either recipe works, even if you're only touching contracts/Rust code.

### Integration Tests

Integration tests start a local Anvil chain, deploy contracts, and run the validator and/or sentinel services:

```sh
just test-integration-sentinel            # ./scripts/run_sentinel_integration_test.sh (Rust sentinel)
just test-integration-validator           # ./scripts/run_validator_integration_test.sh (two Rust validator instances, against an AlwaysApproveOracle-backed happy path, running in CI)
```

These scripts require:

- **Anvil**, **Forge**, **cast** — part of the Foundry toolchain (`foundryup` to install)
- **jq** — for parsing `cast`/deployment output
- **cargo** — for running the Rust services

### Local devnet

```sh
just devnet                  # ./scripts/run_devnet.sh (Podman required)
```

Runs the Rust validator, sentinel, and sentinel-engine services against a local Anvil chain — two validators (`alice`, `bob`) and two sentinel/engine pairs (`carol`, `dave`) vote on a freshly deployed `SentinelOracle`. Each instance is configured via a generated TOML file (`--config-file`), not environment variables. `just devnet --build` builds the validator, sentinel, sentinel-engine, and contracts images. This is separate from `test-integration-sentinel`/`test-integration-validator` above, which exercise the Rust services directly rather than through Podman.

## Code Quality Tools

Run `just check` before committing. Run `just fix` to auto-correct formatting issues.

## Git Branch Naming Convention

Branch names must follow the pattern `pr/<description>` where:

- `<description>` is kebab-case and meaningfully describes the specific change being made

### Good examples

- `pr/fix-staking-withdrawal-overflow`
- `pr/update-validator-setup-guide`

### Bad examples

- `dev`
- `wip`
- `my-branch`
- `feat/wip`
- `fix/stuff`

## Implementation Choices

### Contracts

#### Use Libraries over inheritance

To simplify reviews, code should be split into functional pieces. Solidity libraries should be used for this purpose and preferred over inheritance. Libraries should define a state struct named T and expose methods that alter this struct. An example of this pattern can be found in [FROSTParticipantMap](./contracts/src/libraries/FROSTParticipantMap.sol).

### Sentinel Engine Checks

1. **Check pattern** — each check implements the `Checker` trait (see `checkers/base.rs`'s `Checker` impl for the reference shape: `name()` + `async fn check(&self, transaction, context) -> Verdict`), lives under `crates/sentinel-engine/src/checkers/`, and is registered both in that module's exports (`checkers/mod.rs`) and wherever the engine composes its checker chain (`main.rs`'s `SentinelEngine::new` call). A checker with no opinion must return `Verdict::Abstain` so the next checker in the chain gets a turn — it must never invent a `secure`/`insecure` verdict just to answer. `context: &CheckContext` (`engine/mod.rs`) carries caller-supplied hints outside the transaction itself (currently just `block`, the caller's declared current block, from the request's required `block` field — see `docs/sentinel-engine.md`); most checks ignore it and should just take-and-drop the parameter.
2. **Rule citations** — `RuleId` (`crates/sentinel-engine/src/engine/rule.rs`) is grown incrementally: add a new variant only in the same change that implements the check giving it meaning, doc-commented with the Charter article/rule it corresponds to, `code()` following the `R-<major>.<minor>` pattern. A spec's `rule` field must exactly match an existing `code()` — unrecognized codes are rejected at deserialization.
3. **RPC / chain safety** — the engine is configured with exactly one RPC endpoint for exactly one chain (see [`docs/sentinel-engine.md`](./docs/sentinel-engine.md)). Any check that relies on RPC-derived state (following the existing `AddressPoisoningChecker` pattern, using the engine's injected `Provider` rather than a second client) must verify `transaction.chain_id` against the provider's actual chain itself (e.g. `provider.chain_id()`) and abstain on mismatch — don't assume this is validated elsewhere in the pipeline, because it isn't: `AddressPoisoningChecker` does this itself, but nothing enforces the pattern for a new RPC-backed check, so copy it explicitly rather than assuming a shared guarantee.
4. **No unit tests for checkers** — a checker's behavior is verified by the [sentinel-test-vectors](https://github.com/safe-research/sentinel-test-vectors) corpus (`just test-integration-sentinel-engine <path-to-checkout> [group]`), not by `#[cfg(test)]` unit tests in the checker's own file. Add or update a spec there instead of writing Rust tests when changing check logic.
