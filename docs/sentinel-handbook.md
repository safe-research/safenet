# Safenet Testnet Sentinel Handbook

This document provides a brief guide to operating a Safenet Testnet sentinel.

## Introduction

Sentinels watch the `SentinelOracle` and `Consensus` contracts for proposed transactions, run their own checks against each proposal (e.g. blocklists, address-poisoning heuristics, and optionally a remote check), and commit/reveal a bond-backed approve-or-deny vote onchain. Once enough sentinels have revealed, the request resolves and bonds/fees are settled. Sentinels are run by independent parties, the same way validators are, to maintain decentralization and prevent a single entity from controlling which transactions get approved.

Like validators, sentinels communicate entirely onchain: only a stable RPC node connection is required, and the system does not need to be exposed to the public internet.

For more information on Safenet, consult the [technical overview](./overview.md) as well as the [general public docs](https://docs.safefoundation.org/safenet). See the [validator handbook](./validator-handbook.md) for the validator side of the protocol.

## Requirements

### Infrastructure

#### Ethereum RPC

To run a sentinel, you need a reliable Ethereum RPC node, able to keep up with `eth_getLogs`/`eth_getBlockByNumber` polling plus the `commit`/`reveal`/`finalize`/`claim` transactions described under [Running](#running) below.

##### `eth_getLogs` Reliability

Unfortunately, some RPC providers are unreliable with `eth_getLogs` requests: if the logs are queried too soon after a block is observed then an empty array will be returned even if there are logs in that block. This seems to affect RPC providers that use older versions of Nethermind before 1.36.

The integrity of logs are critical for proper sentinel operation. In order to work around these RPC issues, the sentinels have a built-in mechanism to check log query integrity at the cost of additional bandwidth. If you have reason to believe your RPC may not reliably return all logs, then enable the following configuration in the `[index]` table of your [configuration file](../crates/sentinel/sentinel.sample.toml):

```toml
[index]
use_client_filtering = true
```

#### Logging and Metrics

The sentinel node writes JSON-formatted logs to standard output. It also exposes Prometheus metrics over HTTP, bound by default to an ephemeral port on `localhost` (inconvenient if running from a container). Set `metrics_address` in the `[observability]` table of your [configuration file](../crates/sentinel/sentinel.sample.toml) to bind elsewhere, e.g.:

```toml
[observability]
metrics_address = "0.0.0.0:3555"
```

### Secrets

#### `secp256k1` Sentinel Key

Each sentinel must be provisioned with a `secp256k1` private key. This key is used to authenticate the sentinel onchain for participation in Safenet Testnet. It must be funded with sufficient gas for the EVM transactions required for onchain commit/reveal communication, and with enough of the fee token to put up bonds on the requests it votes on.

> [!TIP]
> The sentinel currently requires the private key at startup and does not support any KMS systems. Do not use this key for anything else, especially security-related tasks. Use it only for running the sentinel, and fund it only with the amount needed for gas and bonds. In the future, we plan to support KMS systems for more secure setups.

##### Gas Costs

The exact amount varies by chain and by how many requests a sentinel votes on, since gas is spent on `commit`/`reveal`/`finalize`/`claim` calls (plus an ERC-20 `approve` for the bond token) rather than on a fixed per-epoch schedule like the validator's. The actual cost of that gas depends on network congestion.

> [!TIP]
> On Gnosis Chain, the base fee is very low relative to the priority fee, so the priority fee makes up the bulk of gas costs. If your RPC occasionally returns an inflated `eth_maxPriorityFeePerGas` estimate, you can cap how much of the total fee cap can be a tip using the `[transactions]` table of your [configuration file](../crates/sentinel/sentinel.sample.toml). For example, setting `priority_fee_cap_percentage = 95` ensures the tip never exceeds 95% of `maxFeePerGas`, protecting against runaway estimates while still allowing normal inclusion.

## Running

Configure the sentinel by writing a TOML configuration file — see [`crates/sentinel/src/config.rs`](../crates/sentinel/src/config.rs) for the full schema, and copy [`sentinel.sample.toml`](../crates/sentinel/sentinel.sample.toml) as a worked example to start from.

```sh
cp crates/sentinel/sentinel.sample.toml sentinel.toml
$EDITOR sentinel.toml
```

Use the provided OCI image to run the sentinel, passing the configuration file's path via `--config-file`. The image's `ENTRYPOINT` is the `sentinel` binary itself, so this flag is appended directly as the container command. For example, with `docker` and assuming `database` in `sentinel.toml` points at a file under `/var/lib/safenet/sentinel/data`:

```sh
docker run --name safenet-sentinel \
    --volume "$(pwd)/sentinel.toml:/usr/src/app/sentinel.toml" \
    --volume sentinel-data:/var/lib/safenet/sentinel/data \
    ghcr.io/safe-research/safenet-sentinel:main \
    --config-file=sentinel.toml
```

## Debugging

There are a few things you can do to verify your sentinel is running as expected:

- Check the logs. For example, if running with `docker`:
  ```sh
  docker logs --follow safenet-sentinel
  ```
- Check the sentinel EVM account on a block explorer. There should be recent transactions to the `SentinelOracle` contract (`commit`/`reveal`/`finalize`/`claim`) and, when bonding, an `approve` call to the fee token.

### Common Problems

- Ethereum node RPC issues:
  -  Rate limits. While the sentinel implements exponential backoff for some RPC requests, rate limits can still prevent full participation in Safenet Testnet.
  -  Missing logs. Some RPC providers do not reliably return all logs for `eth_getLogs` requests. This issue can be mitigated with the appropriate configuration (see [`eth_getLogs` Reliability](#eth_getlogs-reliability)).
- Insufficient funds on the sentinel account to submit onchain transactions. Logs will show that `actions` could not be submitted because of insufficient gas, or that a bond commitment failed because of insufficient fee-token balance/allowance.
