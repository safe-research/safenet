# Examples

Scripts for interacting with the Safenet protocol on public testnets.

## Build a SafenetGuard Attestation for a Safe Transaction

`examples/attest-safe-tx.ts` fetches a completed FROST attestation from the Safenet network
and assembles the `signatures` blob that satisfies `SafenetGuard` — the owner signatures
followed by the inline attestation *trailer* — ready to submit via `execTransaction`.

Use this after a Safe transaction has been proposed to Safenet (via `proposeOracleTransaction`
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
| `ORACLE_ADDRESS` | Address of the oracle contract the transaction was proposed against |
| `RPC_URL` | RPC endpoint for Gnosis (reads the attestation from Consensus and the guard's epoch events) |
| `SAFE_TX_SERVICE_URL` | Safe Transaction Service base URL (e.g. `https://api.safe.global/tx-service/sep`) |
| `SAFE_TX_SERVICE_API_KEY` | API key for the Safe Transaction Service |
| `GUARD_FROM_BLOCK` | *(optional)* first block to scan for the guard's epoch events (default `0`) |

### Usage

```sh
npm run attest-safe-tx -w @safenet/examples -- <safeTxHash> <guardAddress>
```

| Argument | Description |
|----------|-------------|
| `safeTxHash` | The Safe transaction hash (`bytes32`) the attestation was produced for |
| `guardAddress` | The deployed `SafenetGuard` set on the Safe (its epoch events supply the group key) |

### What it does

1. Calls `getActiveEpoch()` and then `getOracleTransactionAttestationByHash(epoch, oracleAddress, safeTxHash)`
   on the Consensus contract on Gnosis Chain to fetch the FROST signature.
2. Resolves the attesting group key by scanning the guard's `EpochInitialized` /
   `EpochRolledOver` events for that epoch (Consensus exposes no group-key getter).
3. Fetches the transaction and its collected owner confirmations from the Safe TX Service
   (`GET /api/v2/multisig-transactions/{safeTxHash}/`).
4. Builds the 256-byte trailer — `abi.encode(uint64 epoch, address oracle, groupKey, FROST.Signature)`
   followed by `keccak256("SafenetGuard.AttestationTrailer.v1")` — appends it to the address-sorted
   owner signatures, and prints the combined `signatures` blob plus the `execTransaction`
   parameters for a relayer to submit.
