# Safenet Beta Validator Handbook

This document provides a brief guide to operating a Safenet Beta validator.

## Introduction

Safenet Beta is a decentralized Safe transaction security network where validators coordinate to generate cryptographic attestations for Safe transactions. Validators are run by independent parties to maintain decentralization and prevent a single entity from producing invalid attestations that could compromise Safenet’s security guarantees.

In the initial beta release, Safenet validators communicate entirely onchain. This simplifies validator operations: only a stable RPC node connection is required, and the system does not need to be exposed to the public internet.

For more information on Safenet, consult the [technical overview](./overview.md) as well as the [general public docs](https://docs.safefoundation.org/safenet).

## Requirements

### System

The Safenet Beta validator node is distributed as an OCI container image that can run on any OCI-compatible runtime (Docker, Podman, etc.). The validator is lean and can run on a single core (with average CPU usage under 5%) and less than 500 MB of RAM (1 GB is recommended to handle spikes). Currently, only Linux x86-64 images are distributed.

Additionally, the validator node stores intermediate state in a SQLite database file. This file contains critical runtime information and should be backed up. Loss of this data would prevent the validator from correctly participating in consensus for the duration of an epoch.

### Infrastructure

#### Ethereum RPC

To run a validator, you need a reliable Ethereum RPC node that can accommodate a peak of 100.000 requests per day. The following table shows the approximate breakdown by RPC method (exact ratios depend on Safenet activity):

| eth_getBlockByNumber | eth_sendRawTransaction | eth_maxPriorityFeePerGas | eth_getLogs | eth_getTransactionCount |
| -------------------- | ---------------------- | ------------------------ | ----------- | ----------------------- |
| 42%                  | 8%                     | 11%                      | 11%         | 28%                     |

##### `eth_getLogs` Reliability

Unfortunately, some RPC providers are unreliable with `eth_getLogs` requests: if the logs are queried too soon after a block is observed then an empty array will be returned even if there logs in that block. This seems to affect RPC providers that use older versions of Nethermind before 1.36.

The integrity of logs are critical for proper validator operation. In order to work around these RPC issues, the validators have a built-in mechanism to check log query integrity at the cost of additional bandwidth. If you have reason to believe your RPC may not reliably return all logs, then enable the following configuration:

```sh
BLOCK_ALL_LOGS_QUERY_RETRY_COUNT=1
BLOCK_SINGLE_QUERY_RETRY_COUNT=1
```

#### Logging and Metrics

The validator node writes JSON-formatted logs to standard output. It also exposes Prometheus metrics on `:3555` by default, which can be scraped over HTTP. Note that, by default, the metrics service binds to `localhost`, which can be inconvenient if running from a container. In that case, use `METRICS_HOST=0.0.0.0` to expose metrics to outside of the container. 

### Secrets

#### `secp256k1` Validator Key

Each validator must be provisioned with a `secp256k1` private key. This key is used to authenticate the validator onchain for participation in Safenet Beta. It must be funded with sufficient gas for the EVM transactions required for onchain consensus-related communication.

> [!TIP]
> The validator currently requires the private key at startup and does not support any KMS systems. Do not use this key for anything else, especially security-related tasks. Use it only for validating, and fund it only with the amount needed for gas. In the future, we plan to support KMS systems for more secure setups.

##### Gas Costs

The exact amount varies by chain, but you can expect the account to consume roughly 1.000.000.000 gas per day under peak load. The actual cost of that gas depends on network congestion.

Safenet Beta’s onchain components are planned for deployment on Gnosis Chain. Over the past six months (Aug 25, 2025 – Feb 25, 2026), the average base fee per gas was approximately 0.042 Gwei, translating to just under $0.05 per day in gas costs. However, daily average base fee per gas reached as high as 3.4 Gwei. Based on these figures, validators should expect to need roughly $10 in tokens to cover gas costs over the six-month Beta period. It is recommended to overfund the validator to account for base gas fee variability.

#### Consensus Secrets

While participating in consensus, validators generate short-term secrets required to attest Safe transactions and participate correctly. Specifically, it generates:

- Secret coefficients used for distributed key generation once per epoch
- A secret signing share used for attesting Safe transactions once per epoch
- Secret nonce pairs that are used in signing ceremonies producing Safe transaction attestations once per signing sequence chunk (i.e. once every 1024 signatures)

Loss of these secrets would prevent the validator from participating in consensus until new ones are computed, and it would forgo protocol rewards during that period. Ensure this information survives restarts. In the current implementation, the validator stores these secrets in an SQLite database on disk. Operators must ensure the file persists across restarts and is backed up in case of failure.

> [!IMPORTANT]
> The secrets are stored in plaintext and are not encrypted in the SQLite database. **Treat the validator database file as containing secret keys**, and apply sufficient restrictions to prevent unauthorized access. **Never share this file with anyone, including the Safenet team for debugging**.

## Running

Configure the validator. See the [configuration documentation](./configuration.md) for reference.

```sh
cp validator/.env.sample validator/.env
$EDITOR validator/.env
```

Use the provided OCI image to run the validator. For example, with `docker` and assuming that the `STORAGE_FILE` was configured to be in the `/var/lib/safenet/validator/data` directory:

```sh
docker run --name safenet-validator \
    --env-file validator/.env \
    --volume validator/data:/var/lib/safenet/validator/data \
    ghcr.io/safe-research/safenet-validator:main
```

## Debugging

There are a few things you can do to verify your validator is running as expected:

- Check the logs. For example, if running with `docker`:
  ```sh
  docker logs --follow safenet-validator
  ```
- Check the validator EVM account on a block explorer. There should be recent transactions to the `Consensus` and `FROSTCoordinator` contracts.

### Common Problems

- Ethereum node RPC issues:
  -  Rate limits. While the validator implements exponential backoff for some RPC requests, rate limits can still prevent full participation in Safenet Beta.
  -  Missing logs. Some RPC providers do not reliably return all logs for `eth_getLogs` requests. This issue can be mitigated with the appropriate configuration (see [`eth_getLogs` Reliability](#eth_getlogs-reliability)).
- Insufficient funds on the validator account to submit onchain transactions. Logs will show that `actions` could not be submitted because of insufficient gas.
