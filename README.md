# Safenet

This is a work-in-progress. Don't use it yet!

## Project Organisation

- [Contracts](./contracts) Consensus contracts (Solidity & Foundry)
- [Explorer](./explorer) Explorer web interface (Typescript & npm)
- [Validator](./validator) Validator service (Typescript & npm)

## Developing

### Project Setup

Clone the repository and all its submodules with:

```sh
git clone --recurse-submodules https://github.com/safe-research/safenet
```

Safenet repository currently an NPM workspace project. Install dependencies with:

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
