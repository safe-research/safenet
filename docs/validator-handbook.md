# Safenet Validator Handbook

This document contains a brief guide on operating a Safenet validator.

## Introduction

Safenet is a decentralized Safe transaction security network, where validators coordinate to generate cryptographic attestations to Safe transactions. Validators are run by independent parties, in order to ensure a high level of decentralization of the network, and preventing a single entity from producing invalid attestations that would compromise the security protections offered by Safenet.

In the initial beta release, Safenet validators communicate entirely onchain. This **greatly** simplifies the logistics of running a validator, only a stable RPC node connection is needed, and it does not require exposing the system to the public internet.

For more general information on Safenet, consult the [technical overview](./overview.md).

## Requirements

### System

The Safenet validator node is distributed as an OCI container image that can run on any OCI-compatible runtime (Docker, Podman, etc.). The validator itself is fairly lean and can run on a single core (with average CPU usage at less than 5%) and less than 500 MB of RAM (although 1 GB is recommended in order to deal with spikes). Currently, we only distribute Linux x86-64 images.

Additionally, the validator node stores intermediate state in a SQLite database file. This file contains critical runtime information and should be backed up. Loss of this data would prevent from the validator from correctly participating with consensus for the duration of an epoch.

### Infrastructure

#### Ethereum RPC

In order to run a validator, you need a reliable Ethereum RPC node that can accommodate a peak of 100.000 requests per day. The following table shows the approximate breakdown of each RPC method that is used (note that exact ratios depends on how busy Safenet is):

| eth_getBlockByNumber | eth_sendRawTransaction | eth_maxPriorityFeePerGas | eth_getLogs | eth_getTransactionCount |
| -------------------- | ---------------------- | ------------------------ | ----------- | ----------------------- |
| 33%                  | 16%                    | 7%                       | 11%         | 33%                     |

#### Logging and Metrics

The validator node writes JSON-formatted logs to standard output. Additionally, it exposes Prometheus metrics to `:3555` by default which can be scraped over HTTP.

### Private Keys

#### `secp256k1` Keys

Each validator must be provisioned a `secp256k1` private key. This key is used to authenticate the validator onchain for participating in Safenet. This key must be funded with sufficient gas money for the executing EVM transactions required for onchain consensus-related communication. The exact amount of funds required varies widely from chain to chain, but you can expect the account to consume roughly 1.000.000.000 gas per day under peak load.

> [!TIP]
> The validator currently requires the private key to be provided at startup, and currently does not support any KMS systems. As such, it is recommended that this private key not be used for anything else, especially nothing security related. It should only be used for validating and have a small amount of funds required as gas money for executing transactions on chain. In the future, we plan to support KMS systems enabling more secure setups.

#### Consensus Secrets

While participating in consensus, validators generate short term secrets that are required for attesting Safe transactions and correctly participating in consensus. Specifically, it generates:

- Secret coefficients used for distributed key generation once per epoch
- A secret signing share used for attesting Safe transactions once per epoch
- Secret nonce pairs that are used in signing ceremonies producing Safe transaction attestations once per signing sequence chunk (i.e. once every 1024 signatures)

The loss of these secrets would cause the validator to not be able to participate in consensus until new ones are computed, foregoing protocol rewards during that period. As such, it is very important to ensure that this information isn't lost and survives validator restarts. In the current implementation, the validator stores these secrets in an SQLite database on disk. Validator operators must ensure that the file is stored in a way such that it is not lost across restarts, and properly backed up in case of failure.

> [!TIP]
> The secrets are currently stored in plaintext and not encrypted in the SQLite database. **This means that the validator database file must be treated as containing secret keys**, ensuring the sufficient restrictions are applied in order to prevent unwanted access. **Never share this file with anyone, including the Safenet team for debugging**.

## Running

Configure the validator:

```shell
cp validator/.env.sample validator/.env
$EDITOR validator/.env
```

Use the provided OCI image to run the validator. For example, with `docker` and assuming that the `STORAGE_FILE` was configured to be in the `/var/lib/safenet/validator/data` directory:

```shell
docker run --name safenet-validator \
    --env-file validator/.env \
    --volume validator/data:/var/lib/safenet/validator/data \
    ghcr.io/safe-research/safenet-validator:main
```

## Debugging

There are a few things you can do to check your validator is running as expected:

- Check the logs, for example if running with `docker` you can check them with:
  ```shell
  docker logs --follow safenet-validator
  ```
- Check the validator EVM account on a block explorer, there should be recent transactions to the `Consensus` and `FROSTCoordinator` contracts

### Common Problems

- Ethereum node RPC issues such as rate limits being hit, while the validator does implement exponential backoff for some of its RPC requests, RPC rate limits can cause the validator to not fully participate in Safenet.
- Insufficient funds on the validator account to submit transactions for communicating onchain, there will be logs that `actions` could not be submitted because of insufficient gas.
