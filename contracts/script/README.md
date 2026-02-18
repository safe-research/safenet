# Deployment

All the commands have to be run from the `contracts` directory. Make sure to fill the `.env` file with the correct values before running the commands.

## Staking

### Chain

#### Anvil (Foundry)

If you are using anvil, recommended to use `-b`, i.e. block time for interval mining with value 1. This helps to increase the time automatically, rather than mining on transaction (which is required to increase the timestamp so we could accept validators once delay is reached after proposal).

### Signing

All the commands in this section require signing, so `--broadcast` flag is used to broadcast the transaction to the network. The default private key (from Anvil) is set in `.env copy` file, so make sure to rename the `.env copy` file to `.env`.

If you prefer to use a different way of signing, you can check the [Foundry forge script documentation](https://www.getfoundry.sh/reference/forge/script#forge-script) for more details.

### (Optional) ERC20 Test Token Deployment

An ERC20 Token needs to be specified in the `.env` file at `SAFE_TOKEN` for deploying the staking contract. If you already have a ERC20 Token (or SAFE Token), then this step can be skipped.

The deployed contract address can be taken from the Logs of forge script command output (Ex: `ERC20 deployed at: 0x...`).

#### Command

```
forge script ./script/DeployERC20.s.sol:ERC20Script --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
```

where
- RPC URL is the localhost
- `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266` is the default address (PK already set in `.env copy`)

Note: `--sender` here is specified because forge uses default sender `0x1804c8AB1F12E6bbf3894d4083f33e07309d1f38` otherwise.

Note: Replace the `RPC_URL` and `DEPLOYER_ADDRESS` in the above line for any custom deployment.

### Staking Contract Deployment

This deploys the staking contract.

Note: Make sure you have filled the `.env` file with the correct values. Staking contract requires these 4 values:
- `STAKING_INITIAL_OWNER`
- `SAFE_TOKEN`
- `STAKING_INITIAL_WITHDRAWAL_DELAY`
- `STAKING_CONFIG_TIME_DELAY`

The deployed contract address is stored in `deployments.json`.

Tip: For easier Testing, both delays can be kept to a minimum. Always remember to keep the withdraw delay <= config delay.

#### Command

```
forge script ./script/DeployStaking.s.sol:StakingScript --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
```

### (Optional) Propose and Accept Validators

This step is optional if the staking contract is already deployed with some validators.

Note: Make sure you have filled the `.env` file with the correct values. Validator proposal requires these two values:
- `ADD_VALIDATORS`: Comma separated addresses
- `IS_REGISTRATION`: Comma separated bool values

Validator acceptance requires an extra parameter on top of the previous values. You can either add it in `.env`, or within the command (recommended):
- `EXECUTABLE_AT`: Unix timestamp (which you can get from event logs)

#### Command

##### Propose

```
forge script ./script/ProposeAndAcceptValidators.s.sol:ProposeAndAcceptValidatorsScript --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
```

##### Extract EXECUTABLE_AT timestamp

```
forge script ./script/util/ExtractExecutableAt.s.sol:ExtractExecutableAtScript --rpc-url http://127.0.0.1:8545
```

You will get the `executableAt` Unix Timestamp from this command.

##### Accept

```
EXECUTABLE_AT=EXECUTABLE_AT_UNIX_TIMESTAMP forge script ./script/ProposeAndAcceptValidators.s.sol:ProposeAndAcceptValidatorsScript --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
```

Note: Replace the `EXECUTABLE_AT_UNIX_TIMESTAMP` with the output of the previous command to accept the validators.

### Stake SAFE Token

This step stakes the SAFE Token (or the selected ERC20 Token) into the staking contract for a particular validator. It also checks if there is enough allowance, if not, initiates a transaction to do the same.

Note: Make sure you have filled the `.env` file with the correct values.
Staking SAFE Tokens require 2 values:
- STAKE_VALIDATOR: Validator address for which you want to stake
- STAKE_AMOUNT: The amount to stake

#### Command

```
forge script ./script/StakeSafe.s.sol:StakeSafeScript --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
```

### Withdraw SAFE Token

This step withdraws the SAFE Token (or the selected ERC20 Token) from the staking contract for a particular validator. There are two steps for withdrawal:
- Initiate Withdraw: This starts the delay for withdrawal, and the tokens can only be withdrawn after the delay is passed.
- Claim Withdraw: This claims the withdrawn tokens after the delay is passed.

Note: Make sure you have filled the `.env` file with the correct values.
Withdrawing SAFE Tokens require 2 values:
- WITHDRAW_VALIDATOR: Validator address for which you want to withdraw
- WITHDRAW_AMOUNT: The amount to withdraw

Note: If you set the `WITHDRAW_AMOUNT` to 0, it will try to claim the next withdrawable amount for the validator. This is why we explicitly set `WITHDRAW_AMOUNT=0` in the Claim Withdraw command, so that it claims the next withdrawable amount, if the delay is passed.

#### Command

##### Initiate Withdraw

```
forge script ./script/StakeWithdraw.s.sol:StakeWithdrawScript --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
```

##### Claim Withdraw

```
WITHDRAW_AMOUNT=0 forge script ./script/StakeWithdraw.s.sol:StakeWithdrawScript --rpc-url http://127.0.0.1:8545 --broadcast --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
```
