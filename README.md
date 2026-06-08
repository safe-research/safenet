# Safenet

This is a work-in-progress. Don't use it yet!

## Project Organisation

- [Contracts](./contracts) Safenet contracts (Solidity & Foundry)
- [Explorer](./explorer) Explorer web interface (Typescript & npm)
- [Validator](./validator) Validator service (Typescript & npm)

## Developing

### Requirements

Developing on the project requires a few tools:

- NodeJS v24 (LTS)
- NPM v11
- Foundry v1.5.1

For the ongoing Rust port, the `stable` channel of the Rust compiler and Cargo package manager are required.

### Foundry Setup

Stable Foundry has a known [formatting bug](https://github.com/foundry-rs/foundry/issues/13362) that affects this repository. With `foundryup` installed, the correct Foundry version can be set up with `npm run foundryup`.

### Project Setup

Clone the repository and all its submodules with:

```sh
git clone --recurse-submodules https://github.com/safe-research/safenet
```

In order to update the submodules, or fetch them if the repository was cloned without a `--recurse-submodules` flag:

```sh
git submodule update --init --recursive
```

Safenet repository is currently an NPM workspace project. Install dependencies with:

```sh
npm ci
```

### Run tests

Unit tests for all projects:

```
npm test
```

Integration test:

```sh
npm run test:integration
```

Verbose logging for tests can be enabled by setting `SAFENET_TEST_VERBOSE=1`.

### Rust Port

Currently, the offchain services of the Safenet protocol are being ported to Rust. Use the standard Cargo commands to build/test/lint/format/etc. the Rust port:

```sh
cargo build
cargo test
cargo clippy
cargo fmt
```
