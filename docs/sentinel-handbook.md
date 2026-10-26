# Safenet Aegis Sentinel Handbook

This document provides a brief guide to operating a Safenet Aegis sentinel.

## Introduction

Sentinels watch the `SentinelOracle` and `Consensus` contracts for proposed transactions, ask their configured [sentinel engine](./sentinel-engine.md) to assess each proposal, and commit/reveal a bond-backed approve-or-deny vote onchain. Once every committed sentinel has revealed, or the reveal window closes, the request is finalized: a unanimous vote resolves it, a split vote escalates it to arbitration, and each sentinel then claims its bond and fee share. Sentinels are run by independent parties, the same way validators are, to maintain decentralization and prevent a single entity from controlling which transactions get approved.

Like validators, sentinels communicate with the protocol entirely onchain. They additionally call their engine over HTTP, but neither service needs to be exposed to the public internet: keep that connection on the same host, in the same pod, or on a private network.

For more information on Safenet, consult the [technical overview](./overview.md) as well as the [general public docs](https://docs.safefoundation.org/safenet). See the [validator handbook](./validator-handbook.md) for the validator side of the protocol.

## Requirements

### Infrastructure

#### Ethereum RPC

To run a sentinel, you need a reliable Ethereum RPC node, able to keep up with `eth_getLogs`/`eth_getBlockByNumber` polling plus the `commit`/`reveal`/`finalize`/`claim`/`timeoutArbitration` transactions described under [Running](#running) below.

##### `eth_getProof` Support

The sentinel reads its account's nonce and code with `eth_getProof` at the latest block it has observed, which can trail the RPC node's tip by a few blocks. Your RPC node must therefore serve `eth_getProof` for recent blocks. Most public Gnosis Chain RPCs do, but some disable the method entirely. Reth nodes need a non-zero `--rpc.eth-proof-window`, as the default only serves proofs for the tip.

##### `eth_getLogs` Reliability

Unfortunately, some RPC providers are unreliable with `eth_getLogs` requests: if the logs are queried too soon after a block is observed then an empty array will be returned even if there are logs in that block. This seems to affect RPC providers that use older versions of Nethermind before 1.36.

The integrity of logs are critical for proper sentinel operation. In order to work around these RPC issues, the sentinels have a built-in mechanism to check log query integrity at the cost of additional bandwidth. If you have reason to believe your RPC may not reliably return all logs, then enable the following configuration in the `[index]` table of your [configuration file](../crates/sentinel/sentinel.sample.toml):

```toml
[index]
use_client_filtering = true
```

#### Sentinel Engine

Every sentinel requires a reachable transaction-verification engine. You can run the repository's reference implementation or your own implementation of its OpenAPI contract; see the [sentinel engine operator guide](./sentinel-engine.md) for configuration, deployment, and the engine's keyless trust boundary.

#### Logging and Metrics

The sentinel node writes JSON-formatted logs to standard output. It also exposes Prometheus metrics over HTTP, bound by default to an ephemeral port on `localhost` (inconvenient if running from a container). Set `metrics_address` in the `[observability]` table of your [configuration file](../crates/sentinel/sentinel.sample.toml) to bind elsewhere, e.g.:

```toml
[observability]
metrics_address = "0.0.0.0:3555"
```

### Secrets

#### `secp256k1` Sentinel Key

Each sentinel must be provisioned with a `secp256k1` private key. This key is used to authenticate the sentinel onchain for participation in Safenet Aegis. It must be funded with sufficient gas for the EVM transactions required for onchain commit/reveal communication, and with enough of the fee token to put up bonds on the requests it votes on.

A bond stays locked from `commit` until the sentinel's `claim` for that request lands, so hold enough of the fee token for several bonds at once. With only one bond's worth, every bond that is still locked (for example during arbitration, or one stranded as described under [Bonds](#bonds)) stops the sentinel from voting on anything else.

> [!TIP]
>
> The sentinel currently requires the private key at startup and does not support any KMS systems. Do not use this key for anything else, especially security-related tasks. Use it only for running the sentinel, and fund it only with the amount needed for gas and bonds. In the future, we plan to support KMS systems for more secure setups.

##### Gas Costs

The exact amount varies by chain and by how many requests a sentinel votes on, since gas is spent on `commit`/`reveal`/`finalize`/`claim`/`timeoutArbitration` calls (plus an ERC-20 `approve` of the fee token for each bond) rather than on a fixed per-epoch schedule like the validator's. The actual cost of that gas depends on network congestion.

> [!TIP]
>
> On Gnosis Chain, the base fee is very low relative to the priority fee, so the priority fee makes up the bulk of gas costs. If your RPC's fee history occasionally yields an inflated priority fee estimate, you can cap how much of the total fee cap can be a tip using the `[transactions]` table of your [configuration file](../crates/sentinel/sentinel.sample.toml). For example, setting `priority_fee_cap_percentage = 95` ensures the tip never exceeds 95% of `maxFeePerGas`, protecting against runaway estimates while still allowing normal inclusion.

> [!TIP]
>
> To reduce gas costs and improve throughput, the sentinel can batch its transactions through an EIP-7702 executor implementing `ISafenet7702Executor`, such as [`Safenet7702Executor`](../contracts/src/Safenet7702Executor.sol). Set `executor` (and optionally `max_batch_gas`) in the `[transactions]` table: every transaction is then sent as a self-call to the executor, batching all actions queued while the previous transaction was pending, so many actions can get onchain in a single transaction. The sentinel's first transaction also delegates its account to the executor. EIP-7702 mempools accept only one pending transaction from a delegated account, so the sentinel keeps a single transaction in flight while its account is delegated. Removing `executor` does not undelegate the account immediately: the sentinel's next transaction removes the delegation, and until it executes, only one transaction is in flight.

## Running

Configure the sentinel by writing a TOML configuration file — see [`crates/sentinel/src/config.rs`](../crates/sentinel/src/config.rs) for the full schema, and copy [`sentinel.sample.toml`](../crates/sentinel/sentinel.sample.toml) as a worked example to start from. Its mandatory `sentinel.engine` URL is a base URL; the sentinel appends the versioned security-check path itself.

The `rpc`, `signer` and `database` settings may reference environment variables as `${NAME}` (use `$$` for a literal `$`), so that secrets such as the sentinel private key or an RPC API key can be injected at startup instead of being stored in the configuration file — for example `signer = "${SIGNER_PRIVATE_KEY}"`. Substitution applies only to these settings; a referenced variable that is not set fails startup.

```sh
cp crates/sentinel/sentinel.sample.toml sentinel.toml
$EDITOR sentinel.toml
```

Use the provided OCI image to run the sentinel, passing the configuration file's path via `--config-file`. The image's `ENTRYPOINT` is the `sentinel` binary itself, so this flag is appended directly as the container command. The example below assumes the reference engine is already running as `safenet-sentinel-engine` on the private `safenet-sentinel` network created in the [engine guide](./sentinel-engine.md#running-the-reference-engine), the sentinel config uses `engine = "http://safenet-sentinel-engine:5473"`, and `database` points at a file under `/var/lib/safenet/sentinel/data`:

```sh
docker run --name safenet-sentinel \
    --network safenet-sentinel \
    --volume "$(pwd)/sentinel.toml:/usr/src/app/sentinel.toml" \
    --volume sentinel-data:/var/lib/safenet/sentinel/data \
    ghcr.io/safe-research/safenet-sentinel:main \
    --config-file=sentinel.toml
```

## Bonds

Each `commit` puts up a bond in the fee token. The sentinel tracks every request it has committed to until its own `claim` for that request lands onchain, so that the bond (minus any slash) and its fee share are returned:

| Request state | Bond locked | Next step |
| --- | --- | --- |
| Waiting for the engine check, or for the `NewRequest` event | no, unless our own `commit` landed after a rollback (see below) | may be dropped (commit deadline passed, engine failure); never dropped once our own `commit` landed, but voting resumes with the recovered verdict, or `finalize` is sent at the reveal deadline |
| Collecting commitments, before our own `commit` landed | no | dropped at the commit deadline |
| Collecting commitments, after our own `commit` landed | yes | collecting votes at the commit deadline |
| Collecting votes | yes | once every commitment is revealed, or at the reveal deadline, `finalize` is sent, even if our own `reveal` never landed |
| Waiting for the outcome | yes | `OracleResult`, `RequestTimedOut` or `DisputeTriggered`; `claim` is sent for the first two |
| Waiting for dispute resolution | yes | arbitration ruling, or `timeoutArbitration` at the arbitration deadline; then `claim` is sent |
| Claiming | yes, until our own `Claimed` | our own `Claimed` event; an `error` is logged once if it hasn't arrived by the claim deadline |

A sentinel whose own `reveal` did not land still has `finalize` and `claim` sent: the contract only slashes the request's `slashAmount` from it, and the remainder of the bond can be claimed.

The sentinel only sends `commit` once the engine check is done, but a chain reorganization can roll back the sentinel's state to before the check while the `commit` it sent still lands. The sentinel then logs `own commitment landed while waiting for the engine check; recovering its verdict` at `warn` and uses the verdict it recorded when it committed, without asking the engine again, since a fresh verdict could differ from the one it committed to. Once that verdict is recovered (`recovered recorded engine verdict`), voting continues without a second `commit`. If there is no recorded verdict, or it can't be read, the sentinel logs an `error` (`no recorded engine verdict to recover` or `failed to read recorded engine verdict to recover`) followed by a `warn`. Both this case and a verdict still missing at the reveal deadline (logged as a `warn` only) need no manual recovery: the sentinel sends `finalize` once the reveal deadline has passed, then `claim` as usual. Without our `reveal`, only the request's `slashAmount` is slashed from our bond, and the remainder is returned.

`finalize` and `claim` are retried by the transaction queue until they are included in a block. If another sentinel finalizes the request first, our `finalize` reverts with `RequestNotPending`; this is expected and harmless.

### Claim Deadline

After sending `claim`, the sentinel waits `claim_window` blocks (in the `[sentinel]` table of the [configuration file](../crates/sentinel/sentinel.sample.toml), default 720, about one hour on Gnosis Chain) for its own `Claimed` event. If the event does not arrive in time, it logs the following `error`, with the affected request in its `request_id` field and the missed claim deadline in `deadline`:

```
claim deadline passed without our own claim landing; the bond must be recovered manually
```

The sentinel does not retry, because a `claim` that was included but reverted points to an unexpected onchain state or a bug, and sending it again would only spend gas. Treat this log as an alert: investigate it and [recover the bond manually](#recovering-a-stranded-bond). The sentinel logs the error only once, but keeps tracking the request until its own `Claimed` event arrives, whether from a `claim` still waiting in the transaction queue or from the manual recovery.

The following `warn` logs also point to a bond that may need attention, since they mean the sentinel's view of a request disagrees with the onchain events:

- `ignoring own commitment for an untracked request`
- `all commitments revealed without our own reveal`
- `own claim for a request that is not claiming`

### Recovering a Stranded Bond

A bond is stranded when the sentinel's account has a `Committed` event for a request but no matching `Claimed` event. To find and recover one:

1. If you are acting on the [claim deadline](#claim-deadline) `error` log, take the request ID from its `request_id` field and continue with step 2. Otherwise, for example to check for bonds stranded by an earlier version, list the sentinel's `Committed` and `Claimed` events on the `SentinelOracle` (both have `sentinel` as an indexed topic, so they can be filtered on a block explorer or with `cast logs`), and note every `requestId` that has a `Committed` but no `Claimed`.
2. For each such request, call `getCommitment(requestId, sentinel)`. A `bondAmount` above zero with `claimed = false` confirms the bond is still locked.
3. Call `getRequest(requestId)` and look at `progress.state`:
   - `PENDING` (1): the request was never finalized. Once the reveal deadline (`terms.revealDeadline`) has passed, call `finalize(requestId)` from any account.
   - `FROZEN` (2): the request is in arbitration. Wait for the arbitrator's ruling, or once `progress.arbitrationDeadline` has passed, call `timeoutArbitration(requestId)` from any account.
   - `RESOLVED_APPROVED` (3), `RESOLVED_DENIED` (4) or `TIMED_OUT` (5): the request can be claimed.
4. Call `claim(requestId)` **from the sentinel's own account** (signed with the sentinel key), since the contract pays out the bond of `msg.sender`. For example:
   ```sh
   cast send --rpc-url "$RPC_URL" --private-key "$SIGNER_PRIVATE_KEY" \
       "$ORACLE" "claim(bytes32)" "$REQUEST_ID"
   ```
   This uses the same account nonce as the running sentinel, so send it while the sentinel has no transaction in flight, or stop the sentinel first.

If `claim` reverts, decode the error: `AlreadyClaimed` means the bond was already returned, `RequestNotResolved` means step 3 is not done yet, and `NothingToClaim` means the account never committed to that request.

## Debugging

There are a few things you can do to verify your sentinel is running as expected:

- Check the logs. For example, if running with `docker`:
  ```sh
  docker logs --follow safenet-sentinel
  docker logs --follow safenet-sentinel-engine
  ```
- Check the sentinel EVM account on a block explorer. There should be recent transactions to the `SentinelOracle` contract (`commit`/`reveal`/`finalize`/`claim`/`timeoutArbitration`) and an `approve` call to the fee token before each `commit`. With `executor` set, these appear as internal calls of transactions the sentinel sends to its own account.

### Common Problems

- Ethereum node RPC issues:
  - Rate limits. While the sentinel retries failed RPC requests, rate limits can still prevent full participation in Safenet Aegis.
  - Missing logs. Some RPC providers do not reliably return all logs for `eth_getLogs` requests. This issue can be mitigated with the appropriate configuration (see [`eth_getLogs` Reliability](#eth_getlogs-reliability)).
- An unreachable engine or a request that exceeds the sentinel's timeout makes the sentinel abstain instead of guessing. Check both services' logs and connectivity over the configured `sentinel.engine` URL.
- Insufficient funds on the sentinel account to submit onchain transactions. Logs will show transaction submissions failing (`submission failed, will retry without bumping fees`, with the RPC's insufficient-funds error). An account without enough of the fee token instead has its `commit` transactions revert onchain, which shows on a block explorer.
- A bond that was never claimed. Logs will show `claim deadline passed without our own claim landing; the bond must be recovered manually`, and the fee token balance stays lower than expected. See [Recovering a Stranded Bond](#recovering-a-stranded-bond).
