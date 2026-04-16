# Safenet Configurations

## Beta Network

### Genesis Validators

Genesis Group Id: `0xf3d78298339ca6b6f2885e1157089bc86ac55520544a83840000000000000000`

- Gnosis:
    - `0x3D58a5475c1336b0A755c3aBd298CeB9b7BB9CDe`
- Greenfield:
    - `0x7B0A8EFA45dE81F11F2846EC28259B62155a2b37`
- Rockaway:
    - `0xb0E735D4a3b70195420E0ae933689A55750CFcd2`
- Core Contributors:
    - `0xCc00DE0eA14c08669b26DcBFE365dBD9890B04D9`
- Blockchain Capital:
    - `0xD8997c2a94052C4FB79B53b3e255c1F07c99305B`
- Safe Labs:
    - `0xF6EA21D702983c443f58A267265912FE03D2FF0b`

### Network Configuration
- Chain Id:
    - `100`
- Coordinator
    - `0xaE27021CEB45316f1efe69D8E362aC07ED3Bd7E4`
    - [Gnosisscan](https://gnosisscan.io/address/0xaE27021CEB45316f1efe69D8E362aC07ED3Bd7E4)
- Consensus
    - `0x223624cBF099e5a8f8cD5aF22aFa424a1d1acEE9`
    - [Gnosisscan](https://gnosisscan.io/address/0x223624cBF099e5a8f8cD5aF22aFa424a1d1acEE9)
- Genesis Salt
    - `0x5afe000000000000000000000000000000000000000000000000000000000000`
- Blocks per epoch
    - `1440` (~2 hours)
- Signing timeout
    - `6` (~30 seconds)

### Environment Variables

```bash
LOG_LEVEL=notice
STORAGE_FILE=# i.e. /var/lib/safenet/validator/data/storage.db
STORAGE_BACKUP=# i.e. '/var/lib/safenet/validator/data/storage.%%s.db'
CHAIN_ID=100
BLOCKS_PER_EPOCH=1440
SIGNING_TIMEOUT=6
CONSENSUS_ADDRESS=0x223624cBF099e5a8f8cD5aF22aFa424a1d1acEE9
COORDINATOR_ADDRESS=0xaE27021CEB45316f1efe69D8E362aC07ED3Bd7E4
PARTICIPANTS='[{"address":"0x3D58a5475c1336b0A755c3aBd298CeB9b7BB9CDe"},{"address":"0x7B0A8EFA45dE81F11F2846EC28259B62155a2b37"},{"address":"0xb0E735D4a3b70195420E0ae933689A55750CFcd2"},{"address":"0xCc00DE0eA14c08669b26DcBFE365dBD9890B04D9"},{"address":"0xD8997c2a94052C4FB79B53b3e255c1F07c99305B"},{"address":"0xF6EA21D702983c443f58A267265912FE03D2FF0b"}]'
GENESIS_SALT=0x5afe000000000000000000000000000000000000000000000000000000000000
RPC_URL=# Gnosis Chain RPC
STAKER_ADDRESS=# Address that manages the stake on Ethereum Mainnet for the validator
PRIVATE_KEY=# EOA that is funded on Gnosis Chain to interact with the consensus contract
```

See validator [.env.sample](../validator/.env.sample) for reference and additional configuration options.

#### Staking

The `STAKER_ADDRESS` environment variable is of particular importance: it specifies which account is responsible for putting up the validator stake **on Ethereum Mainnet**. This allows a separate account (such as a Safe multisig) to be used to manage the large validator stake, instead of the same private key that is used by the validator for participating in consensus on Gnosis Chain. Validators earn a commission on delegated stake which will only be earned if the `STAKER_ADDRESS` is set and the minimum stake has been put up by the `STAKER_ADDRESS`. This value must be set to the Ethereum Mainnet account that will put up the validator stake on the Safenet staking contract ([Etherscan](https://etherscan.io/address/0x115E78f160e1E3eF163B05C84562Fa16fA338509)).

More information can be found on the [Safenet rewards documentation](https://docs.safefoundation.org/safenet/staking/rewards)

The `STAKER_ADDRESS` will receive all validator rewards including commission, unless another beneficiary has been set.

##### Configuring a Separate Commission Beneficiary

Per default, the `STAKER_ADDRESS` will receive all validator rewards including commission **on Ethereum Mainnet**. In order to have commission be distributed to another beneficiary **on Ethereum Mainnet**, validators must set a delegate on the [DelegateRegistry](https://etherscan.io/address/0x469788fE6E9E9681C6ebF3bF78e7Fd26Fc015446) with `id = keccak256(toHex("Safenet Beta validator commission beneficiary"))`

```
to: 0x469788fE6E9E9681C6ebF3bF78e7Fd26Fc015446
function: setDelegate
    id: 0x45c518fef2d01542b884830ef4eaae3137aebc8a3df6e4c4b73c585f85e709b0  // keccak256(toHex("Safenet Beta validator commission beneficiary"))
    delegate: 0x...  // beneficiary address **on Ethereum Mainnet**
```

Please inform the Safe team in case you intend to do this so we can make sure everything is accounted properly. 

Once executed, `STAKER_ADDRESS` will only receive rewards on its own stake. `beneficiary` will only receive the commission.

