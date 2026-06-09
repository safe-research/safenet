# Safenet Developer Guide

Safenet is a decentralized Safe transaction security network that uses FROST (Flexible Round-Optimized Schnorr Threshold) signatures to validate Safe transactions before on-chain execution.

## Architecture

The repository is a hybrid monorepo with:

- Three NPM workspaces:
  - `contracts/` — Solidity 0.8.30 smart contracts built with Foundry. Core contracts: `FROSTCoordinator.sol`, `Consensus.sol`, `Staking.sol`.
  - `explorer/` — React 19 + TypeScript + Vite frontend for inspecting network state.
  - `validator/` — Node.js + TypeScript validator service that participates in FROST signing rounds.
- Rust crates:
  - `crates/core/` — Shared code used by all Safenet offchain services

Additionally, formal verification specs live in `certora/`. Integration and devnet scripts are in `scripts/`.

## Documentation

For detailed architecture and technical documentation, refer to the local [docs](./docs/) folder. Additional documentation on the Safe smart account is available on the [Safe Foundation docs](https://docs.safefoundation.org). The MCP server related to this documentation can be found at <https://docs.safefoundation.org/mcp>.

## Coding Guidelines

Code SHOULD focus on security and maintainability. Existing code and components SHOULD be reused. New components SHOULD be written in a way that they can be reused. If in doubt, refer to existing code to determine coding style and which implementation to choose. Do not re-invent the wheel and follow existing paradigms.

You MUST format, lint and test before committing.

- For JavaScript/Typescript code, run `npm run fix --workspace <package>`, `npm run check --workspace <package>`, and `npm test --workspace <package>` respectively
- For Rust code, run `cargo fmt --all`, `cargo clippy --package <package>`, and `cargo test --package <package>` respectively

## Testing Guidelines

New code SHOULD generally be tested. Design tests that do not require a high amount of churn with refactors (such as testing general behaviours and not implementation details). Your goal is not 100% test coverage (except for Solidity code). When modifying code the equivalent test MUST also be updated as required.

## Development Commands

### Project Setup

The steps for project setup are documented in the root [README.md](./README.md#project-setup).

Always use `npm ci` instead of `npm install` / `npm i`. `npm ci` installs exactly what is in `package-lock.json` and never modifies it, keeping the lock file stable.

Make sure you have the correct tool versions (NodeJS 24, NPM 11, Foundry 1.5.1). Use `npm run foundryup` to set up the correct Foundry version.

### NPM Commands

All commands are specified in the root [package.json](./package.json). Workspace specific commands can be found in the `package.json` of each corresponding workspace.

To run a command in a specific workspace use `--workspace` or `-w`.

### Integration Tests

Integration tests start a local Anvil chain, deploy contracts, and run the validator:

```sh
npm run test:integration        # Run all integration tests
```

The script (`./scripts/run_integration_test.sh`) requires:

- **Anvil** — part of the Foundry toolchain (`foundryup` to install)
- **Forge** — for contract deployment
- **Node.js** — validator service

### Local devnet

```sh
npm run devnet                  # ./scripts/run_devnet.sh (Podman required)
```

## Code Quality Tools

Run `npm run check` before committing. Run `npm run fix` to auto-correct formatting issues.

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
