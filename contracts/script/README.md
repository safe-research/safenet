# Deployment

All the commands specified in `package.json` currently only simulates the transaction. For signing and broadcasting the transaction, you should use the `--rpc-url` and `--broadcast` flag along with the wallet configuration in forge. Make sure to fill the `.env` file with the correct values before running the commands.

## Staking

### Chain

#### Anvil (Foundry)

If you are using anvil, recommended to use `-b`, i.e. block time for interval mining with value 1. This helps to increase the time automatically, rather than mining on transaction (which is required to increase the timestamp so we could accept validators once delay is reached after proposal).

### Signing

All the commands in this section require signing, so `--broadcast` flag is used to broadcast the transaction to the network. The default private key (from Anvil) is set in `.env.sample` file, so make sure to rename the `.env.sample` file to `.env`.

If you prefer to use a different way of signing, you can check the [Foundry forge script documentation](https://www.getfoundry.sh/reference/forge/script#forge-script) for more details.

### (Optional) ERC20 Test Token Deployment

An ERC20 Token needs to be specified in the `.env` file at `SAFE_TOKEN` for deploying the staking contract. If you already have a ERC20 Token (or SAFE Token), then this step can be skipped.

The deployed contract address can be taken from the Logs of forge script command output (Ex: `ERC20 deployed at: 0x...`).

#### Command

```
npm run cmd:erc20 -w @safenet/contracts
```

For broadcasting and specifying rpc url along with sender, you can use the following command:

```
npm run cmd:erc20 -w @safenet/contracts -- --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --account anvil-1
```

where
- RPC URL is the localhost (anvil local node) url, change it accordingly if you are using a different chain or network.
- `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266` is the default anvil address
- `anvil-1` is the default anvil account alias for the address `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266` used for signing the transaction with forge keystore feature. You can use any other method as specified in the [forge documentation](https://www.getfoundry.sh/forge/scripting#providing-a-private-key).

Note: `--sender` here is specified because forge uses default sender `0x1804c8AB1F12E6bbf3894d4083f33e07309d1f38` otherwise, even when `--account` is specified.

### Staking Contract Deployment

This deploys the staking contract.

Note: Make sure you have filled the `.env` file with the correct values. Staking contract requires these 4 values:
- `STAKING_INITIAL_OWNER`
- `SAFE_TOKEN`
- `STAKING_INITIAL_WITHDRAWAL_DELAY`
- `STAKING_CONFIG_TIME_DELAY`

Tip: For easier Testing, both delays can be kept to a minimum. Always remember to keep the withdraw delay <= config delay.

#### Command

```
npm run cmd:staking -w @safenet/contracts -- --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --account anvil-1
```

### (Optional) Propose and Accept Validators

This step is optional if the staking contract is already deployed with some validators.

Note: Make sure you have filled the `.env` file with the correct values. Validator proposal requires these two values:
- `ADD_VALIDATORS`: Comma separated addresses
- `IS_REGISTRATION`: Comma separated bool values

#### Command

##### Propose

```
npm run cmd:proposeValidators -w @safenet/contracts -- --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --account anvil-1
```

##### Accept

```
npm run cmd:acceptValidators -w @safenet/contracts -- --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --account anvil-1
```

You could also set an environment variable `EXECUTABLE_AT` with the timestamp value to execute the validator changes at a specific time (after the delay is passed). If not set, it will try to read the event from the propose script output.

### Stake SAFE Token

This step stakes the SAFE Token (or the selected ERC20 Token) into the staking contract for a particular validator. It also checks if there is enough allowance, if not, initiates a transaction to do the same.

Note: Make sure you have filled the `.env` file with the correct values.
Staking SAFE Tokens require 2 values:
- `STAKE_VALIDATOR`: Validator address for which you want to stake
- `STAKE_AMOUNT`: The amount to stake

#### Command

```
npm run cmd:stakeSafe -w @safenet/contracts -- --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --account anvil-1
```

### Withdraw SAFE Token

This step withdraws the SAFE Token (or the selected ERC20 Token) from the staking contract for a particular validator. There are two steps for withdrawal:
- Initiate Withdraw: This starts the delay for withdrawal, and the tokens can only be withdrawn after the delay is passed.
- Claim Withdraw: This claims the withdrawn tokens after the delay is passed.

Note: Make sure you have filled the `.env` file with the correct values.
Withdrawing SAFE Tokens require 2 values:
- `WITHDRAW_VALIDATOR`: Validator address for which you want to withdraw
- `WITHDRAW_AMOUNT`: The amount to withdraw

#### Command

##### Initiate Withdraw

```
npm run cmd:initiateWithdraw -w @safenet/contracts -- --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --account anvil-1
```

##### Claim Withdraw

```
npm run cmd:claimWithdraw -w @safenet/contracts -- --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --account anvil-1
```
