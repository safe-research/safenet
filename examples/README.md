# Examples

Scripts for interacting with the Safenet protocol on public testnets.

## Attach Safenet Attestation to a Safe Transaction

`examples/attest-safe-tx.ts` fetches a completed FROST attestation from the Safenet
network and posts it as a cosigner EIP-1271 signature to the Safe Transaction Service,
making it visible as a confirmation in the Safe UI.

Use this after a Safe transaction has been proposed to Safenet (via `proposeBasicTransaction`
on the Consensus contract) and the FROST signing round has completed.

### Prerequisites

```sh
# Node.js dependencies (from repo root)
npm ci
```

### Environment

Copy `examples/.env.sample` to `examples/.env` and fill in the required values:

```sh
cp examples/.env.sample examples/.env
# edit examples/.env
```

| Variable | Description |
|----------|-------------|
| `CONSENSUS_ADDRESS` | Address of the Safenet Consensus contract on Gnosis Chain |
| `RPC_URL` | RPC endpoint for the consensus chain (to read attestation) |
| `SAFE_TX_SERVICE_URL` | Safe Transaction Service base URL (e.g. `https://api.safe.global/tx-service/sep`) |
| `SAFE_TX_SERVICE_API_KEY` | API key for the Safe Transaction Service |

### Usage

```sh
npm run attest-safe-tx -w @safenet/examples -- <safeTxHash> <cosignerAddress>
```

| Argument | Description |
|----------|-------------|
| `safeTxHash` | The Safe transaction hash (`bytes32`) to attach the attestation to |
| `cosignerAddress` | The deployed `SafenetCosigner` contract address (must be an owner of the Safe) |

### What it does

1. Calls `getRecentTransactionAttestationByHash(safeTxHash)` on the Consensus contract on
   Gnosis Chain to fetch the epoch number and FROST signature.
2. Fetches the multisig transaction parameters from the Safe TX Service
   (`GET /api/v2/multisig-transactions/{safeTxHash}/`).
3. Encodes the attestation as a full EIP-1271 contract signature:
   - **Static slot** (65 bytes): `r` = cosigner address, `s` = 65 (offset to dynamic data), `v` = 0x00
   - **Dynamic data** (160 bytes): length = 128, data = `abi.encode(uint64 epoch, FROST.Signature)`
4. Posts the cosigner signature to the Safe TX Service
   (`POST /api/v2/safes/{safe}/multisig-transactions/`).
