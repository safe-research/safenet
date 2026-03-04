# CLAUDE.md — Safenet Developer Guide

Safenet is a decentralized Safe transaction security network that uses FROST (Flexible Round-Optimized Schnorr Threshold) signatures to validate Safe transactions before on-chain execution.

## Architecture

The repository is an npm monorepo with three workspaces:

- `contracts/` — Solidity 0.8.30 smart contracts built with Foundry. Core contracts: `FROSTCoordinator.sol`, `Consensus.sol`, `Staking.sol`.
- `explorer/` — React 19 + TypeScript + Vite frontend for inspecting network state.
- `validator/` — Node.js + TypeScript validator service that participates in FROST signing rounds.

Formal verification specs live in `certora/`. Integration and devnet scripts are in `scripts/`.

Technical documentation for the system is in `docs/`, where we can find an overview, a glossary, and detailed validator information.

## Documentation

For detailed architecture and setup information, refer to the local [docs](./docs/) folder. Additional documentation, including information about Safe, is available on the [Safe Foundation docs](https://docs.safefoundation.org). The MCP server related to this documentation can be found at https://docs.safefoundation.org/mcp.

## Development Commands

### Project Setup

The steps for project setup are documented in the root [README.md](./README.md#project-setup).

### Commands

All commands are specified in the root [package.json](./package.json). Workspace specific commands can be found in the `package.json` of each corresponding workspace. 

To run a command in a specific workspace use `--workspace` or `-w`.


### Integration Tests

Integration tests start a local Anvil chain, deploy contracts, and run the validator against both storage backends:

```sh
npm run test:integration        # Run all integration tests
```

The script (`./scripts/run_integration_test.sh`) requires:
- **Anvil** — part of the Foundry toolchain (`foundryup` to install)
- **Forge** — for contract deployment
- **Node.js** — validator service

To run against a specific storage backend:

```sh
SAFENET_TEST_STORAGE=inmemory npm run test:integration
SAFENET_TEST_STORAGE=sqlite   npm run test:integration
```

### Local devnet

```sh
npm run devnet                  # ./scripts/run_devnet.sh (Podman required)
```

## Code Quality Tools

- **Biome 2.0.6** — linter and formatter for TypeScript/JavaScript. Config: `biome.json` at root and per workspace. Line width: 120 characters.
- **Forge** — Solidity formatter (`forge fmt`) and linter (`forge lint --deny notes`).
- **TypeScript** — strict mode, version 5.8.3. Each workspace has its own `tsconfig.json`.
- **Vitest** — unit test runner for TypeScript workspaces.
- **Certora** — formal verification. Python environment via `certora/requirements.txt`.

Run `npm run check` before committing. Run `npm run fix` to auto-correct formatting issues.

## Git Branch Naming Convention

Branch names must follow the pattern `<prefix>/<description>` where:

- `<prefix>` is one of: `feat`, `fix`, `docs`, `refactor`, `chore`, `test`, `claude`
- `<description>` is kebab-case and meaningfully describes the specific change being made

### Good examples

```
feat/add-frost-key-rotation
fix/staking-withdrawal-overflow
docs/update-validator-setup-guide
refactor/simplify-consensus-state-machine
chore/bump-viem-to-v3
claude/add-claude-md-branch-rules
```

### Bad examples

```
dev
wip
my-branch
feat/wip
fix/stuff
```

Always use a name that makes the purpose of the branch immediately clear to anyone reading it.
