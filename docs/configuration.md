# Safenet Configurations

## Aegis Network

### Validators

Genesis Group Id: `0x0e758f1c2a12eb5d93f5328ff8b6896ae6e0bbc0f8c6d3320000000000000000`

- Core Contributors (_Genesis_):
  - `0xCc00DE0eA14c08669b26DcBFE365dBD9890B04D9`
- Safe Labs (_Genesis_):
  - `0xF6EA21D702983c443f58A267265912FE03D2FF0b`
- Gnosis:
  - `0x3D58a5475c1336b0A755c3aBd298CeB9b7BB9CDe`
- Greenfield:
  - `0x7B0A8EFA45dE81F11F2846EC28259B62155a2b37`

### Network Configuration

- Chain Id:
  - `100`
- Coordinator
  - `0xC6B34dA4c99C043A093214D58423a8bE39585B78`
  - [Gnosisscan](https://gnosisscan.io/address/0xC6B34dA4c99C043A093214D58423a8bE39585B78)
- Consensus
  - `0x1Ec44a7cFBdaE3d339F22F50d13B5661AF5D19a8`
  - [Gnosisscan](https://gnosisscan.io/address/0x1Ec44a7cFBdaE3d339F22F50d13B5661AF5D19a8)
- Sentinel Oracle
  - `0xc7E93E88fCA4F4DaB9eD393157afDf4F0Da3b1e7`
  - [Gnosisscan](https://gnosisscan.io/address/0xc7E93E88fCA4F4DaB9eD393157afDf4F0Da3b1e7)
- Safenet 7702 Executor
  - `0x4DFe88ADf2D7C3FBd2e334Ce820B9aDFa541215c`
  - [Gnosisscan](https://gnosisscan.io/address/0x4DFe88ADf2D7C3FBd2e334Ce820B9aDFa541215c)
- Genesis Salt
  - `0x0000000000000000000000000000000000000000000000000000000000000000`
- Blocks per epoch
  - `1440` (~2 hours)
- Key generation timeout
  - `120` (~10 minutes)
- Signing timeout
  - `6` (~30 seconds)
- Oracle timeout
  - `24` (~2 minutes)

### Sentinel Oracle Configuration

- Fee token
  - USDC.e `0x2a22f9c3b484c3629090FeED35F17Ff8F88f76F0`
  - [Gnosisscan](https://gnosisscan.io/address/0x2a22f9c3b484c3629090FeED35F17Ff8F88f76F0)
- Request fee
  - `0.40` USDC.e
- Bond multiplier
  - `2000` (800 USDC.e bond per vote)
- Slashing multiplier
  - `1` (0.40 USDC.e slashed per losing vote)
- DAO fee share
  - `0%`
- Commit window
  - `6` blocks (~30 seconds)
- Reveal window
  - `6` blocks (~30 seconds)
- Governance delay
  - `51840` blocks (~3 days)
- Arbitration timeout
  - `483840` blocks (~28 days)
- Governance and protocol funds receiver
  - `0x9D02bed59170cdE96782Ea9a9b17F9bCF4916127`
- Arbitrator
  - `0xc89Ae48382edc81287AcEf0881E88DEacAD58348`
- Charter
  - `charter.safenet-gov.eth`

### Validator Configuration

The settings for a validator's [configuration file](../crates/validator/validator.sample.toml), with placeholders for the operator-specific values. The timing parameters above are the validator's built-in defaults, so they can be omitted:

```toml
rpc = "..."
signer = "..."
database = "..."

[validator]
staker = "..."
consensus = "0x1Ec44a7cFBdaE3d339F22F50d13B5661AF5D19a8"
oracles = ["0xc7E93E88fCA4F4DaB9eD393157afDf4F0Da3b1e7"]
genesis_salt = "0x0000000000000000000000000000000000000000000000000000000000000000"

[[validator.participants]]
address = "0xCc00DE0eA14c08669b26DcBFE365dBD9890B04D9"

[[validator.participants]]
address = "0xF6EA21D702983c443f58A267265912FE03D2FF0b"

[[validator.participants]]
address = "0x3D58a5475c1336b0A755c3aBd298CeB9b7BB9CDe"
active_from = 1

[[validator.participants]]
address = "0x7B0A8EFA45dE81F11F2846EC28259B62155a2b37"
active_from = 1
```

### Sentinel Configuration

The settings for a sentinel's [configuration file](../crates/sentinel/sentinel.sample.toml), with placeholders for the operator-specific values:

```toml
rpc = "..."
signer = "..."
database = "..."
oracle = "0xc7E93E88fCA4F4DaB9eD393157afDf4F0Da3b1e7"
consensus = "0x1Ec44a7cFBdaE3d339F22F50d13B5661AF5D19a8"

[sentinel]
fee_token = "0x2a22f9c3b484c3629090FeED35F17Ff8F88f76F0"
voting_window = 6
engine = "..."
```

## Staking

The `staker` setting in the `[validator]` table of the validator's [configuration file](../crates/validator/validator.sample.toml) is of particular importance: it specifies which account is responsible for putting up the validator stake **on Ethereum Mainnet**. This allows a separate account (such as a Safe multisig) to be used to manage the large validator stake, instead of the same private key that is used by the validator for participating in consensus on Gnosis Chain. On startup, the validator registers this account onchain with `Consensus.setValidatorStaker` if it is not already set. Validators earn a commission on delegated stake which will only be earned if `staker` is set and the minimum stake has been put up by the `staker` account. This value must be set to the Ethereum Mainnet account that will put up the validator stake on the Safenet staking contract ([Etherscan](https://etherscan.io/address/0x115E78f160e1E3eF163B05C84562Fa16fA338509)).

More information can be found on the [Safenet rewards documentation](https://docs.safefoundation.org/safenet/staking/rewards)

The `staker` account will receive all validator rewards including commission, unless another beneficiary has been set.

### Configuring a Separate Commission Beneficiary

By default, the `staker` account will receive all validator rewards including commission **on Ethereum Mainnet**. In order to have commission be distributed to another beneficiary **on Ethereum Mainnet**, the `staker` account must set a delegate on the [DelegateRegistry](https://etherscan.io/address/0x469788fE6E9E9681C6ebF3bF78e7Fd26Fc015446) with `id = keccak256(toHex("Safenet Beta validator commission beneficiary"))`.

```
from: staker
to: 0x469788fE6E9E9681C6ebF3bF78e7Fd26Fc015446 // DelegateRegistry
function: setDelegate
    id: 0x45c518fef2d01542b884830ef4eaae3137aebc8a3df6e4c4b73c585f85e709b0  // keccak256(toHex("Safenet Beta validator commission beneficiary"))
    delegate: 0x...  // beneficiary address **on Ethereum Mainnet**
```

Please inform the Safe team in case you intend to do this so we can make sure everything is accounted properly.

Once executed, the `staker` account will only receive rewards on its own stake. `beneficiary` will only receive the commission.
