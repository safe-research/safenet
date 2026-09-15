# Deployment

All the commands specified in the root [Justfile](../../Justfile) currently only simulate the transaction. For signing and broadcasting the transaction, you should use the `--rpc-url` and `--broadcast` flag along with the wallet configuration in forge. Make sure to fill the `.env` file with the correct values before running the commands.

## Setup

We use Foundry as a tool for the deployment & interaction scripts. Make sure to have Foundry installed and set up in your system. You can follow the instructions from the [Foundry Book](https://www.getfoundry.sh/introduction/installation) for installation.

Make sure to follow the setup instructions from the project [README](../../README.md#project-setup)

Also, please use the `.env.sample` file to create a `.env` file and fill the required values before running the commands.

Note: If you face an error like `vm.envUint: environment variable "ABC" not found`, make sure to set the required environment variables in the `.env` file.

For deployment commands, the choice of `FACTORY` needs to be specified in the `.env` file. It can have two values:

- `1`: Safe Singleton Factory (Default, recommended for mainnet or public testnets)
- `2`: Canonical Deterministic Deployment Factory

Note: If you set the `FACTORY` as `1`, make sure that the Safe Singleton Factory is deployed on the chain you are using. You can find the list of deployed factory addresses on different chains [here](https://github.com/safe-fndn/safe-singleton-factory).

## Staking

### Chain

#### Anvil (Foundry)

If you are using anvil as a blockchain for testing, recommended to use `-b`, i.e. block time for interval mining with value 1. This helps to increase the time automatically, rather than mining on transaction (which is required to increase the timestamp so we could accept validators once delay is reached after proposal).

```
anvil -b 1
```

The above command is only required if you want to test things out locally.

### Signing

All the commands in this section require signing, so `--broadcast` flag is used to broadcast the transaction to the network. We use the `--account` flag to specify the account alias from forge keystore, which is used for signing the transaction.

If you prefer to use a different way of signing, you can check the [Foundry signing documentation](https://www.getfoundry.sh/cast/sending-transactions#sending-transactions) for more details.

To setup a new wallet in forge, you can check the [Foundry wallet operation documentation](https://www.getfoundry.sh/cast/wallet-operations#wallet-operations).

### (Optional) ERC20 Test Token Deployment

An ERC20 Token needs to be specified in the `.env` file at `SAFE_TOKEN` for deploying the staking contract. If you already have a ERC20 Token (or SAFE Token), then this step can be skipped.

The deployed contract address can be taken from the Logs of forge script command output (Ex: `ERC20 deployed at: 0x...`).

#### Command

Dry Run:

```
just contracts-deploy-erc20
```

For broadcasting and specifying rpc url along with sender, you can use the following command:

```
just contracts-deploy-erc20 --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --account sender-keystore-account
```

where

- RPC URL specified here is the localhost (anvil local node) url, change it accordingly based on the chain or network.
- `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266` is the sender address used for signing the transaction.
- `sender-keystore-account` is the keystore account alias for the address `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266` used for signing the transaction with forge keystore feature. You can use any other method as specified in the [forge documentation](https://www.getfoundry.sh/forge/scripting#providing-a-private-key). Replace `sender-keystore-account` with the alias of the account you want to use for signing.

Note: `--sender` here is specified because forge uses default sender `0x1804c8AB1F12E6bbf3894d4083f33e07309d1f38` otherwise, even when `--account` is specified.

Example ETH Mainnet Command will look something like this:

```
just contracts-deploy-erc20 --rpc-url https://eth.drpc.org --broadcast --sender SENDER_ADDRESS --account mainnet-account
```

Here we are using `https://eth.drpc.org` as the RPC URL for mainnet, you can replace it with any other RPC provider. Also replace `SENDER_ADDRESS` with the address you want to use for deployment and `mainnet-account` with the keystore alias of the account in forge keystore which has the private key for the sender address.

### Staking Contract Deployment

This deploys the staking contract.

Note: Make sure you have filled the `.env` file with the correct values. Staking contract deployment requires these five values (if not provided explicitly, default will be taken):

- `STAKING_INITIAL_OWNER`
- `SAFE_TOKEN`
- `STAKING_INITIAL_WITHDRAWAL_DELAY`
- `STAKING_CONFIG_TIME_DELAY`
- `FACTORY`

Tip: For easier Testing, both delays can be kept to a minimum. Always remember to keep the withdraw delay <= config delay.

#### Command

##### EOA based deployment

```
just contracts-deploy-staking --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --account sender-keystore-account
```

##### Safe Tx Builder based deployment

```
just contracts-deploy-staking-tx-builder
```

This command provides the output as a JSON in the path: `contracts/build/staking-deployment.json` which can be used with the Safe Tx Builder tool to deploy the staking contract. A forge verify command is also provided as the output of the above command, which can be used to verify the deployment transaction in etherscan (requires the etherscan API key).

Note: An added environment variable `CHAIN_ID` can optionally be specified for the above command to specify the chain id of the network for which the deployment transaction is being built. If not set, it will take the default value as `1` (Ethereum Mainnet).

### (Optional) Propose and Accept Validators

This step is optional if the staking contract is already deployed with some validators.

Note: Make sure you have filled the `.env` file with the correct values. Validator proposal requires these two values:

- `ADD_VALIDATORS`: Comma separated addresses
- `IS_REGISTRATION`: Comma separated bool values

Note: If you want to explicitly provide a staking contract address, you can set the `STAKING_ADDRESS` environment variable in the `.env` file. Else it will calculate the staking contract address based on the staking constructor arguments and the factory address.

#### Command

##### Propose

```
just contracts-propose-validators --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --account sender-keystore-account
```

##### Accept

```
just contracts-accept-validators --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --account sender-keystore-account
```

You could also set an environment variable `EXECUTABLE_AT` with the timestamp value to execute the validator changes at a specific time (after the delay is passed). If not set, it will try to read the event from the propose script output.

### Stake SAFE Token

This step stakes the SAFE Token (or the selected ERC20 Token) into the staking contract for a particular validator. It also checks if there is enough allowance, if not, initiates a transaction to do the same.

Note: Make sure you have filled the `.env` file with the correct values. Staking SAFE Tokens require 3 values:

- `STAKE_VALIDATOR`: Validator address for which you want to stake
- `STAKE_AMOUNT`: The amount to stake
- `SAFE_TOKEN`: or the selected ERC20 Token should have enough balance in the sender's account and also approved for the staking contract.

Note: If you want to explicitly provide a staking contract address, you can set the `STAKING_ADDRESS` environment variable in the `.env` file. Else it will calculate the staking contract address based on the staking constructor arguments and the factory address.

#### Command

```
just contracts-stake-safe --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --account sender-keystore-account
```

### Withdraw SAFE Token

This step withdraws the SAFE Token (or the selected ERC20 Token) from the staking contract for a particular validator. There are two steps for withdrawal:

- Initiate Withdraw: This starts the delay for withdrawal, and the tokens can only be withdrawn after the delay is passed.
- Claim Withdraw: This claims the withdrawn tokens after the delay is passed.

Note: Make sure you have filled the `.env` file with the correct values. Withdrawing SAFE Tokens require 2 values:

- `WITHDRAW_VALIDATOR`: Validator address for which you want to withdraw
- `WITHDRAW_AMOUNT`: The amount to withdraw

Note: If you want to explicitly provide a staking contract address, you can set the `STAKING_ADDRESS` environment variable in the `.env` file. Else it will calculate the staking contract address based on the staking constructor arguments and the factory address.

#### Command

##### Initiate Withdraw

```
just contracts-initiate-withdraw --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --account sender-keystore-account
```

##### Claim Withdraw

```
just contracts-claim-withdraw --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --account sender-keystore-account
```

## Reality Veto Module

Deploys and operates [`RealityVetoModule`](../src/veto/README.md), the SafeDAO module that lets one address (the vetoer) make the Safe invalidate a pending SafeSnap proposal. Read [`src/veto/README.md`](../src/veto/README.md) first: it holds the threat model and the invariants this runbook assumes.

Enabling a module on the SafeDAO Safe is a governance transaction. Everything below the deployment step is executed by SafeDAO owners, not by whoever runs the script.

### Configuration

Set these in `contracts/.env`; `.env.sample` has the block. All three are Gnosis Chain addresses.

| Variable | What it is |
| --- | --- |
| `VETO_SAFE_ADDRESS` | The SafeDAO Safe that will enable this module, and that owns the Reality module. |
| `VETO_REALITY_MODULE_ADDRESS` | The SafeSnap Reality module proxy owned by that Safe. |
| `VETOER_ADDRESS` | The account permitted to veto. Still an open governance decision, so do not broadcast until it is settled. |

### Before deploying

Check that the Safe still owns the Reality module. A swapped or stale `VETO_REALITY_MODULE_ADDRESS` otherwise survives deployment, survives the governance vote, and only shows up as a `VetoFailed` at the first real veto:

```
cast call $VETO_REALITY_MODULE_ADDRESS "owner()(address)" --rpc-url $RPC_URL   # must equal VETO_SAFE_ADDRESS
cast call $VETO_REALITY_MODULE_ADDRESS "avatar()(address)" --rpc-url $RPC_URL  # must equal VETO_SAFE_ADDRESS
cast call $VETO_REALITY_MODULE_ADDRESS "target()(address)" --rpc-url $RPC_URL  # must equal VETO_SAFE_ADDRESS
```

The constructor checks addresses, not code or ownership, so this is the only place these are caught early.

### Deployment

Dry run:

```
just contracts-deploy-reality-veto
```

Broadcast:

```
just contracts-deploy-reality-veto --rpc-url $RPC_URL --broadcast --sender SENDER_ADDRESS --account gnosis-account
```

The deployed address is in the script output (`RealityVetoModule deployed at: 0x...`). Record it alongside the other deployment addresses and verify the contract on Gnosisscan. The module does nothing at all until the Safe enables it.

### Enabling the module

One transaction from the SafeDAO Safe, at the full owner threshold. In the Safe Tx Builder:

- To: the SafeDAO Safe itself (`$VETO_SAFE_ADDRESS`)
- Value: `0`
- Operation: `Call`
- Function: `enableModule(address module)`, with `module` set to the deployed `RealityVetoModule`

Raw calldata is `0x610b5925` followed by the module address left-padded to 32 bytes.

### After enabling

1. `cast call $VETO_SAFE_ADDRESS "isModuleEnabled(address)(bool)" $VETO_MODULE --rpc-url $RPC_URL` must return `true`.
2. `cast call $VETO_MODULE "getVetoer()(address)" --rpc-url $RPC_URL` must return the intended vetoer, and `SAFE()` and `REALITY_MODULE()` the addresses above.
3. Smoke-test a real veto on a fork, not on chain. Fork Gnosis at the current block (`anvil --fork-url $RPC_URL`), add a throwaway proposal through the Reality module, veto it from the vetoer with `vetoProposal(proposalId, txHashes)`, and check `questionIds(questionHash)` becomes `INVALIDATED` (`bytes32(uint256(1))`). A veto that silently does nothing is the failure this catches; nothing else does.

### Vetoing a proposal

From the vetoer, against the deployed module:

```
cast send $VETO_MODULE "vetoProposal(string,bytes32[])" "$PROPOSAL_ID" "[$TX_HASHES]" --rpc-url $RPC_URL
```

The arguments must be exactly those the proposal was added with. Different `txHashes`, or a veto sent before `addProposal` lands, produce a different question hash and revert `ProposalNotFound`; retry once the proposal is on chain.

Three things to know before pressing send:

- **A veto is permanent for that proposal identity** and cannot be undone by anyone. Governance can re-propose under a new `proposalId`, which restarts the timeout and cooldown.
- **A late veto can leave a multi-transaction proposal half-applied.** Execution is one call per transaction and each re-reads the invalidation flag, so a veto after index `k` blocks `k+1` onward and does not undo `0..k`. Check how far execution has progressed (`executedProposalTransactions`) before vetoing a multi-transaction proposal, and prepare the remediation transaction alongside the veto.
- **A repeat veto succeeds and re-emits.** `ProposalVetoed` can legitimately appear twice for one question hash.

### Monitoring

Watch `ProposalQuestionCreated` on the Reality module and `ProposalVetoed` / `VetoerChanged` on the veto module.

A proposal whose transactions target the veto module (`setVetoer`) or the Safe's module list (`disableModule`) neutralises the veto if it executes: the Reality module calls through the Safe, so such a proposal arrives at `setVetoer` with `msg.sender == the Safe` and passes the gate. The only defence is vetoing that proposal inside its own window, so monitoring must flag proposals by what their `txHashes` touch, not only by what they appear to be about.

### Rotating the vetoer

One governance transaction from the Safe:

- To: the deployed `RealityVetoModule`
- Value: `0`
- Operation: `Call`
- Function: `setVetoer(address newVetoer)`

The sitting vetoer cannot rotate or renounce itself, and `newVetoer` cannot be the zero address. Confirm with `getVetoer()`; the old vetoer loses access in the same transaction.

### Emergency revocation

`disableModule(address prevModule, address module)` from the Safe, on the Safe. Read the module list first, because `prevModule` is the entry preceding the veto module in the Safe's linked list:

```
cast call $VETO_SAFE_ADDRESS "getModulesPaginated(address,uint256)(address[],address)" 0x0000000000000000000000000000000000000001 100 --rpc-url $RPC_URL
```

`prevModule` is `0x...01` (the sentinel) when the veto module is at the head of the list, which it is unless another module was enabled after it. Disabling leaves the Reality module untouched and makes every subsequent `vetoProposal` revert `GS104`.

### Fallback if the module is disabled

The Safe can still invalidate a proposal without the module, by owner-signed `execTransaction` calling `markProposalAsInvalid(string,bytes32[])` on the Reality module directly. Same effect, at the full owner threshold, which is the latency the module exists to avoid.

### Ether sent to the module

The module has no `receive` and no `fallback`, so it cannot be paid by a normal transfer, but ether can still be forced in (a `selfdestruct` beneficiary, or a block reward). Such a balance is stuck: the module has no function that moves value. It is not Safe funds and needs no action.
