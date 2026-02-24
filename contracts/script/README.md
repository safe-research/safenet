# Deployment

All the commands specified in `package.json` currently only simulates the transaction. For signing and broadcasting the transaction, you should use the `--rpc-url` and `--broadcast` flag along with the wallet configuration in forge. Make sure to fill the `.env` file with the correct values before running the commands.

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
npm run cmd:deploy:testing-erc20 -w @safenet/contracts
```

For broadcasting and specifying rpc url along with sender, you can use the following command:

```
npm run cmd:deploy:testing-erc20 -w @safenet/contracts -- --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --account sender-keystore-account
```

where
- RPC URL specified here is the localhost (anvil local node) url, change it accordingly based on the chain or network.
- `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266` is the sender address used for signing the transaction.
- `sender-keystore-account` is the keystore account alias for the address `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266` used for signing the transaction with forge keystore feature. You can use any other method as specified in the [forge documentation](https://www.getfoundry.sh/forge/scripting#providing-a-private-key). Replace `sender-keystore-account` with the alias of the account you want to use for signing.

Note: `--sender` here is specified because forge uses default sender `0x1804c8AB1F12E6bbf3894d4083f33e07309d1f38` otherwise, even when `--account` is specified.

Example ETH Mainnet Command will look something like this:

```
npm run cmd:deploy:testing-erc20 -w @safenet/contracts -- --rpc-url https://eth.drpc.org --broadcast --sender SENDER_ADDRESS --account mainnet-account
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
npm run cmd:deploy:staking -w @safenet/contracts -- --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --account sender-keystore-account
```

##### Safe Tx Builder based deployment

```
npm run cmd:deploy:staking-with-tx-builder -w @safenet/contracts
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
npm run cmd:proposeValidators -w @safenet/contracts -- --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --account sender-keystore-account
```

##### Accept

```
npm run cmd:acceptValidators -w @safenet/contracts -- --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --account sender-keystore-account
```

You could also set an environment variable `EXECUTABLE_AT` with the timestamp value to execute the validator changes at a specific time (after the delay is passed). If not set, it will try to read the event from the propose script output.

### Stake SAFE Token

This step stakes the SAFE Token (or the selected ERC20 Token) into the staking contract for a particular validator. It also checks if there is enough allowance, if not, initiates a transaction to do the same.

Note: Make sure you have filled the `.env` file with the correct values.
Staking SAFE Tokens require 3 values:
- `STAKE_VALIDATOR`: Validator address for which you want to stake
- `STAKE_AMOUNT`: The amount to stake
- `SAFE_TOKEN`: or the selected ERC20 Token should have enough balance in the sender's account and also approved for the staking contract.

Note: If you want to explicitly provide a staking contract address, you can set the `STAKING_ADDRESS` environment variable in the `.env` file. Else it will calculate the staking contract address based on the staking constructor arguments and the factory address.

#### Command

```
npm run cmd:stakeSafe -w @safenet/contracts -- --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --account sender-keystore-account
```

### Withdraw SAFE Token

This step withdraws the SAFE Token (or the selected ERC20 Token) from the staking contract for a particular validator. There are two steps for withdrawal:
- Initiate Withdraw: This starts the delay for withdrawal, and the tokens can only be withdrawn after the delay is passed.
- Claim Withdraw: This claims the withdrawn tokens after the delay is passed.

Note: Make sure you have filled the `.env` file with the correct values.
Withdrawing SAFE Tokens require 2 values:
- `WITHDRAW_VALIDATOR`: Validator address for which you want to withdraw
- `WITHDRAW_AMOUNT`: The amount to withdraw

Note: If you want to explicitly provide a staking contract address, you can set the `STAKING_ADDRESS` environment variable in the `.env` file. Else it will calculate the staking contract address based on the staking constructor arguments and the factory address.

#### Command

##### Initiate Withdraw

```
npm run cmd:initiateWithdraw -w @safenet/contracts -- --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --account sender-keystore-account
```

##### Claim Withdraw

```
npm run cmd:claimWithdraw -w @safenet/contracts -- --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --account sender-keystore-account
```
