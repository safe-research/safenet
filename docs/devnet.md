# Safenet Devnet

This document covers how to spin up the local Safenet devnet and interact with it once it's
running: finding contract addresses, sending transactions, checking validator/sentinel status, and
pointing an explorer at it. The devnet is driven by
[`scripts/run_devnet.sh`](../scripts/run_devnet.sh) (see also [`AGENTS.md`](../AGENTS.md#local-devnet)
for a short summary).

## Prerequisites

- Full [project setup](../README.md#project-setup) (submodules, `npm ci`, `npm run foundryup`) —
  required even if you only plan to run `npm run devnet` without touching the contracts/validator/
  sentinel code yourself, since `npm run devnet -- --build` builds their Podman images from this
  checkout and will fail if submodules aren't initialized or Foundry isn't set up.
- [Podman](https://podman.io/docs/installation) — the devnet is orchestrated with `podman kube play`,
  not `docker-compose`, and there is currently no Docker equivalent.
- [`jq`](https://jqlang.org/), only if you use the broadcast-receipt method in
  [Manually recovering contract addresses](#manually-recovering-contract-addresses).

## Starting the devnet

```sh
npm run devnet                  # ./scripts/run_devnet.sh
```

The first run (or after contract/validator/sentinel code changes) needs `--build`, to build the
three Podman images the devnet depends on:

```sh
npm run devnet -- --build       # builds localhost/safenet-{contracts,validator,sentinel}
```

This brings up a single Podman pod named `safenet` containing:

- `node` — an Anvil chain, RPC exposed on the host at `http://localhost:8545`.
- `validator-alice`, `validator-bob` — two Rust validators (`crates/validator`) participating in
  FROST consensus.
- `sentinel-carol`, `sentinel-dave` — two Rust sentinels (`crates/sentinel`) that commit/reveal
  votes on oracle-checked transactions.

Startup deploys the core Safenet contracts (`Consensus`, `FROSTCoordinator`, `AlwaysApproveOracle`),
a fee ERC-20 token, and a `SentinelOracleV2` arbitrated by a dedicated arbitrator account, registers
and funds both sentinels against it, and (unless `--no-genesis` is passed) kicks off FROST key
generation for the genesis epoch. The script runs in the foreground and only returns once that's
done, so leave the terminal open (or run it in the background) while you interact with the network.

### Options

```
-h, --help                  Print the script's help message.
--build                     Build the contracts, validator and sentinel Podman images.
--port <PORT>               Alternate host port for the Ethereum RPC (default 8545).
--block-time <SECS>         Block time in seconds for the devnet (default 5).
--blocks-per-epoch <NUM>    Number of blocks per Safenet epoch (default 60).
--no-genesis                Do not kick off genesis key generation.
--fund-account <ADDRESS>    Fund an additional account with ETH and fee tokens.
```

For example, to run a faster-ticking devnet on an alternate port and fund your own wallet:

```sh
npm run devnet -- --port 9545 --block-time 1 --fund-account 0xYourAddress
```

## Network details

| | |
|---|---|
| RPC endpoint | `http://localhost:8545` (override with `--port`) |
| Chain ID | `31337` (Anvil default) |
| Block time | `5`s by default (override with `--block-time`) |
| Blocks per epoch | `60` by default (override with `--blocks-per-epoch`) |

The pod's containers are named `safenet-node`, `safenet-validator-alice`, `safenet-validator-bob`,
`safenet-sentinel-carol`, and `safenet-sentinel-dave`.

## Accounts

The devnet uses Anvil's standard test-mnemonic accounts. All of them are pre-funded with ETH and
unlocked by Anvil, so you can impersonate any of them with `cast --unlocked` without a private key.

| Role | Address | Private key |
|---|---|---|
| Deployer / sender | `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266` | `0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80` |
| Validator `alice` | `0x70997970C51812dc3A010C7d01b50e0d17dc79C8` | `0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d` |
| Validator `bob` | `0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC` | `0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a` |
| Sentinel `carol` | `0x90F79bf6EB2c4f870365E785982E1f101E93b906` | `0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6` |
| Sentinel `dave` | `0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65` | `0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a` |
| Arbitrator | `0x9965507D1a55bcC2695C58ba16FB37d819B0A4dc` | `0x8b3a350cf5c34c9194ca85829a2df0ec3153be0318b5e2d3348e872092edffba` |

## Setting up environment variables

Most of the commands below need the `Consensus`, `SentinelOracleV2`, fee token, and
`FROSTCoordinator` addresses. Rather than copying them by hand, pull them into your shell once per
devnet run:

```sh
CONFIG=$(podman exec safenet-sentinel-carol cat /config/sentinel.toml)
export CONSENSUS_ADDRESS=$(echo "$CONFIG" | grep '^consensus' | grep -oE '0x[0-9a-fA-F]{40}')
export ORACLE_ADDRESS=$(echo "$CONFIG" | grep '^oracle' | grep -oE '0x[0-9a-fA-F]{40}')
export FEE_TOKEN_ADDRESS=$(echo "$CONFIG" | grep '^fee_token' | grep -oE '0x[0-9a-fA-F]{40}')
export COORDINATOR_ADDRESS=$(cast call "$CONSENSUS_ADDRESS" "getCoordinator()(address)" --rpc-url http://localhost:8545)
```

This reads the same mounted config `safenet-sentinel-carol` was started with, then derives
`COORDINATOR_ADDRESS` via `Consensus.getCoordinator()` (see
[Manually recovering contract addresses](#manually-recovering-contract-addresses) for how this
works in more detail, and alternative ways to get at the same values). The rest of this document
uses `$CONSENSUS_ADDRESS`, `$ORACLE_ADDRESS`, and `$FEE_TOKEN_ADDRESS` directly — since these are
`export`ed (or, on fish, `set -gx`'d), every command run in the same shell picks them up
automatically, with no need to restate them. Re-run this after every `npm run devnet` restart, since
addresses are freshly deployed each time.

## Interacting with the devnet

### Reading contract state with `cast`

Run `cast` against the devnet RPC directly, or from inside the `safenet-node` container:

```sh
cast call $CONSENSUS_ADDRESS "getActiveEpoch()(uint64,bytes32)" --rpc-url http://localhost:8545
cast call $CONSENSUS_ADDRESS "getCoordinator()(address)" --rpc-url http://localhost:8545
```

`SentinelOracleV2` requests move through `PENDING → FROZEN → RESOLVED_APPROVED` / `RESOLVED_DENIED`
(or `TIMED_OUT`); inspect a request's state and sentinel commitments with:

```sh
cast call $ORACLE_ADDRESS "getRequest(bytes32)" <REQUEST_ID> --rpc-url http://localhost:8545
cast call $ORACLE_ADDRESS "getCommitment(bytes32,address)" <REQUEST_ID> <SENTINEL_ADDRESS> --rpc-url http://localhost:8545
cast call $ORACLE_ADDRESS "bondMultiplier()(uint256)" --rpc-url http://localhost:8545
```

### Proposing a Safe transaction

Use the same Forge scripts documented in [`contracts/script/README.md`](../contracts/script/README.md),
pointed at the devnet's RPC and one of the accounts above, e.g. to propose a plain Safe transaction
(`CONSENSUS_ADDRESS` is already exported from [above](#setting-up-environment-variables), so it
doesn't need to be restated here):

```sh
TX_CHAIN_ID=31337 \
TX_SAFE=<SAFE_ADDRESS> \
TX_TO=<TO_ADDRESS> \
TX_NONCE=0 \
TX_DATA=<CALLDATA> \
TX_OPERATION=0 \
npm run cmd:propose -w @safenet/contracts -- \
    --rpc-url http://127.0.0.1:8545 --unlocked --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --broadcast
```

Or, to propose a transaction that requires sentinel oracle approval. `SentinelOracleV2.postRequest`
pulls the request fee from the proposing account (`--sender`, i.e. `msg.sender` on
`proposeOracleTransaction`) via `transferFrom`, so that account must first `approve` the oracle to
spend the fee token:

```sh
cast send $FEE_TOKEN_ADDRESS "approve(address,uint256)" $ORACLE_ADDRESS 400000000000000000 \
    --rpc-url http://127.0.0.1:8545 --unlocked --from 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
```

`400000000000000000` (0.4 tokens, 18 decimals) matches the devnet's default `SENTINEL_REQUEST_FEE`
(`scripts/run_devnet.sh`); approve more if you plan to propose several oracle transactions. The
deployer account already holds the entire fee token supply (minted to it by `DeployERC20Script`), so
using it as `--sender` only requires this allowance, not additional funding — an account funded via
`--fund-account` instead already has both balance and would still need this approval.

```sh
TX_CHAIN_ID=31337 \
TX_SAFE=<SAFE_ADDRESS> \
TX_TO=<TO_ADDRESS> \
TX_NONCE=0 \
TX_DATA=<CALLDATA> \
TX_OPERATION=0 \
npm run cmd:propose:oracle -w @safenet/contracts -- \
    --rpc-url http://127.0.0.1:8545 --unlocked --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --broadcast
```

`TX_DATA` is the transaction calldata (`0x` for a plain ETH transfer), and `TX_OPERATION` is `0` for
a `Call` or `1` for a `DelegateCall`; both are optional and default to those values if omitted.

Both print the resulting `Safe Transaction Hash`. Note `--unlocked --sender <address>` is used
instead of `--account`/a private key, since every devnet account is already unlocked by Anvil.

Once a FROST signing round completes, `examples/attest-safe-tx.ts` (see
[`examples/README.md`](../examples/README.md)) can assemble the `SafenetGuard` signature blob —
though it also expects a Safe Transaction Service instance, which the devnet does not provide, so
this path may need adaptation to work fully offline.

### Checking attestation status

`Consensus` stores an attestation per `(epoch, Safe transaction hash)` (or, for oracle-checked
transactions, per `(epoch, oracle, Safe transaction hash)`) as a FROST `Signature` — a
`((uint256,uint256),uint256)` tuple of `(r.x, r.y), z`. A **zero** signature (all three fields `0`)
means the FROST group hasn't produced an attestation for that hash yet.

The simplest check doesn't require knowing the epoch — it looks at the active epoch first, falling
back to the previous one:

```sh
cast call $CONSENSUS_ADDRESS "getRecentTransactionAttestationByHash(bytes32)(uint64,((uint256,uint256),uint256))" \
    <SAFE_TX_HASH> --rpc-url http://localhost:8545
```

This returns the `epoch` the attestation was found in alongside the signature itself. To check a
specific epoch instead (e.g. `getActiveEpoch()` from [above](#reading-contract-state-with-cast)),
or to check the attestation for an oracle-checked transaction, use:

```sh
cast call $CONSENSUS_ADDRESS "getTransactionAttestationByHash(uint64,bytes32)(((uint256,uint256),uint256))" \
    <EPOCH> <SAFE_TX_HASH> --rpc-url http://localhost:8545

cast call $CONSENSUS_ADDRESS "getOracleTransactionAttestationByHash(uint64,address,bytes32)(((uint256,uint256),uint256))" \
    <EPOCH> $ORACLE_ADDRESS <SAFE_TX_HASH> --rpc-url http://localhost:8545
```

> [!TIP]
> For an oracle-checked transaction, the attestation only appears once `SentinelOracleV2` has
> resolved the request (see [Reading contract state with `cast`](#reading-contract-state-with-cast)
> for how to check a request's `getRequest`/`getCommitment` state) — validators won't attest before
> the oracle approves.

### Checking validator and sentinel status

The most direct way to check liveness is the container logs:

```sh
podman logs -f safenet-validator-alice
podman logs -f safenet-sentinel-carol
```

Beyond logs, check the validator/sentinel accounts on a block explorer pointed at the devnet (see
below) — there should be recent transactions to the `Consensus`/`FROSTCoordinator` contracts from
validators, and `commit`/`reveal` calls to `SentinelOracleV2` from sentinels.

### Tearing down

There is no teardown script; stop and remove the pod manually:

```sh
podman pod stop safenet && podman pod rm safenet
```

## Pointing an explorer at the devnet

### Safenet Explorer (this repo's protocol explorer)

The [`explorer/`](../explorer/) app in this repo inspects Safenet protocol state — proposals,
epochs, validator/sentinel status — rather than being a generic chain explorer. Run it locally and
point it at the devnet:

```sh
npm run dev -w explorer        # http://localhost:3000
```

By default it opens with an empty RPC/Consensus configuration and a **Settings** panel in the UI
where you can enter values directly — set:

- RPC endpoint: `http://localhost:8545`
- Consensus address: `$CONSENSUS_ADDRESS` (run `echo $CONSENSUS_ADDRESS` to get the value to paste)

Alternatively, pre-fill these as the app's defaults via env vars (see
[`explorer/.env.sample`](../explorer/.env.sample)):

```sh
cp explorer/.env.sample explorer/.env
echo "VITE_DEFAULT_RPC=http://localhost:8545" >> explorer/.env
echo "VITE_DEFAULT_CONSENSUS=$CONSENSUS_ADDRESS" >> explorer/.env
```

### Generic EVM block explorer

For a raw transaction/block explorer against the devnet's Anvil chain, there is no explorer bundled
or pre-wired in this repo — but since it's a plain EVM chain on `http://localhost:8545` (chain ID
`31337`), [Otterscan](https://github.com/otterscan/otterscan) works well: Anvil natively implements
Otterscan's required `ots_*` RPC namespace, so despite the env var's name, `ERIGON_URL` just points
Otterscan at any compatible backend, Anvil included:

```sh
podman run --rm -p 5100:80 -e ERIGON_URL=http://localhost:8545 docker.io/otterscan/otterscan:latest
```

Then open `http://localhost:5100` and browse the deployer/validator/sentinel/arbitrator addresses
from the [accounts table](#accounts) above.

## Tips and Tricks

### Manually recovering contract addresses

The [environment variable step](#setting-up-environment-variables) above already covers the common
case. A few other ways to get at the same values:

`DeployScript` prints its output to the terminal running `npm run devnet`, so the following are
visible directly in that output:

```
Genesis Group ID: 0x...
FROSTCoordinator: 0x...
Consensus: 0x...
AlwaysApproveOracle: 0x...
```

The fee ERC-20 token and `SentinelOracleV2` addresses are **not** printed (their deployment output
is silenced by the script). Besides reading them out of the mounted TOML config (as the
environment variable step does):

```sh
podman exec safenet-sentinel-carol cat /config/sentinel.toml   # oracle, consensus, fee_token
podman exec safenet-validator-alice cat /config/validator.toml # consensus, oracles
```

you can also read the deployment receipt straight from the contracts container:

```sh
podman exec safenet-node cat /contracts/build/broadcast/DeployERC20.s.sol/31337/run-latest.json \
    | jq -r '.returns.erc20.value'
podman exec safenet-node cat /contracts/build/broadcast/DeploySentinelOracleV2.s.sol/31337/run-latest.json \
    | jq -r '.returns.sentinelOracle.value'
```

### Funding an additional account

Pass `--fund-account <address>` to `run_devnet.sh` to additionally fund an arbitrary address with
10 ETH and 1000 fee tokens, useful for proposing transactions from your own wallet.

### Podman image short names

Podman (unlike Docker) doesn't assume `docker.io` for unqualified image names by default, and fails
with `short-name "..." did not resolve to an alias` unless `unqualified-search-registries` is
configured in `containers-registries.conf(5)`. Fully-qualify images (e.g.
`docker.io/otterscan/otterscan:latest`, as used [above](#generic-evm-block-explorer)) to avoid the
issue regardless of local Podman configuration.

### Accessing a headless/remote devnet

If the devnet runs on a remote or headless machine (e.g. a VM you only reach over SSH), forward the
relevant ports to your local machine instead of exposing them or reconfiguring anything:

```sh
ssh -N -L 8545:localhost:8545 -L 5100:localhost:5100 -L 3000:localhost:3000 user@remote-vm
```

Keep the local and remote port numbers matched, as shown above. Both the Safenet Explorer and
Otterscan are browser apps with no server-side proxy — `VITE_DEFAULT_RPC`/`ERIGON_URL` are read
and called by your **local browser**, so they must resolve on your machine, not the remote one.
Forwarding each port to the same number locally means the existing `http://localhost:8545` /
`http://localhost:5100` values keep working unmodified; you don't need to point either app at the
VM's hostname or IP.
