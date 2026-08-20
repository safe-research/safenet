# Safenet

This is a work-in-progress. Don't use it yet!

## Project Organisation

- [Contracts](./contracts) Safenet contracts (Solidity & Foundry)
- [Examples](./examples) Interact with Safenet on test networks (Typescript & npm)
- [Explorer](./explorer) Explorer web interface (Typescript & npm)
- [Services core crate](./crates/core) Shared logic between the offchain Safenet services (Rust)
- [Sentinel](./crates/sentinel) Service that watches proposals and puts up bonds for its onchain votes (Rust)
- [Sentinel engine](./crates/sentinel-engine) Keyless transaction-verification API used by sentinels ([operator guide](./docs/sentinel-engine.md)) (Rust)
- [Validator](./crates/validator) Validator service that participates in FROST signing rounds and epoch rollovers

## Developing

### Requirements

Developing on the project requires a few tools:

- NodeJS v24 (LTS)
- NPM v11
- Foundry v1.5.1
- Rust + Cargo (Stable)
- [Just](https://github.com/casey/just), the command runner used to invoke every command below

### Foundry Setup

Stable Foundry has a known [formatting bug](https://github.com/foundry-rs/foundry/issues/13362) that affects this repository. With `foundryup` installed, the correct Foundry version can be set up with `just foundryup`.

### Project Setup

Clone the repository and all its submodules with:

```sh
git clone --recurse-submodules https://github.com/safe-research/safenet
```

In order to update the submodules, or fetch them if the repository was cloned without a `--recurse-submodules` flag:

```sh
git submodule update --init --recursive
```

`contracts/` has no JavaScript of its own (see the [Justfile](./Justfile)), but `examples/` and
`explorer/` are each an independent NPM package with its own lockfile. Install their dependencies
with:

```sh
just deps
```

Each of `examples/`/`explorer/` also has its own [Biome](https://biomejs.dev/) devDependency and
`biome.json` (extending the shared rules in the root [`biome.json`](./biome.json), but scoped to
that package's own files only) — `just check`/`just fix` run it once per directory, so both
`npm ci` commands above are needed before either recipe works.

### Run tests

Unit tests for all projects:

```
just test
```

Integration tests:

```sh
just test-integration-sentinel
just test-integration-validator
```

Verbose logging for tests can be enabled by setting `SAFENET_TEST_VERBOSE=1`.

### Rust Services

The offchain services of the Safenet protocol (`crates/validator`, `crates/sentinel`,
`crates/sentinel-engine`) are implemented in Rust. Use the standard Cargo commands to
build/test/lint/format/etc. them:

```sh
cargo build
cargo test
cargo clippy
cargo fmt
```

## Planning Epics

When developing larger epics spanning over multiple PRs with an agent (for example a complex feature or big refactor), generate a plan to help guide the agent by outlining the separate phases in the development. It is recommended to use the `/plan-epic` feature from <https://github.com/safe-research/agents> for this.
