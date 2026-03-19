# Safenet Configurations

## Beta Network

### Genesis Validators

Genesis Group Id: `0xf3d78298339ca6b6f2885e1157089bc86ac55520544a83840000000000000000`

- Validator 1:
    - `0x3D58a5475c1336b0A755c3aBd298CeB9b7BB9CDe`
- Validator 2:
    - `0x7B0A8EFA45dE81F11F2846EC28259B62155a2b37`
- Validator 3:
    - `0xb0E735D4a3b70195420E0ae933689A55750CFcd2`
- Validator 4:
    - `0xCc00DE0eA14c08669b26DcBFE365dBD9890B04D9`
- Validator 5:
    - `0xD8997c2a94052C4FB79B53b3e255c1F07c99305B`
- Validator 6:
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