# Examples

Integration test examples for the Safenet protocol on public testnets.

## Sepolia Integration Test: Safenet Cosigner + Hypernative Guard

End-to-end setup and test for running a Safe on Sepolia with:

- **SafenetCosigner** as a co-owner (EIP-1271, threshold 2/2)
- **HypernativeGuard** as the transaction guard

Every Safe transaction requires two independent approvals: a FROST threshold signature
from the Safenet validator network (via the cosigner) and a keeper signature from
Hypernative. This is "Approach 1" from the Safenet on-chain enforcement design.

### Architecture

```
Safe (Sepolia, threshold 2)
  owners:
    [0] owner1          — EOA, signs with forge keystore
    [1] owner2          — EOA, recipient of the test transaction
    [2] SafenetCosigner — contract owner, validates FROST attestation from Safenet
  guard: HypernativeGuard — requires keeper signature on every transaction
```

**Signature layout** (387 bytes, passed as `signatures` to `execTransaction`):

```
[owner1 ECDSA: 65 bytes]          static slot, sorted ascending by address
[cosigner EIP-1271: 65 bytes]     static slot: r=cosigner address, s=130 (offset), v=0
[attestation length: 32 bytes]    = 128
[attestation: 128 bytes]          abi.encode(uint64 epoch, FROST.Signature{r:{x,y}, z})
[keeper ECDSA: 65 bytes]          appended after dynamic cosigner data
[context length: 32 bytes]        = 0 (no HypernativeGuard context)
```

### Prerequisites

#### Tools

```sh
# Foundry (forge, cast, anvil)
curl -L https://foundry.paradigm.xyz | bash
foundryup

# Node.js dependencies (from repo root)
npm ci
```

#### Build SafenetCosigner

The script reads the SafenetCosigner creation bytecode from the Foundry build
artifact. Run this from the repository root before the first test run and after
any changes to `contracts/src/cosigner/SafenetCosigner.sol`:

```sh
npm run build -w @safenet/contracts
```

#### Keystore accounts

Four accounts are required. Create them once and reuse across test runs:

```sh
cast wallet import owner1    --interactive   # Safe owner, signs Safe transactions
cast wallet import owner2    --interactive   # Recipient of the test transaction and one of the Safe owners
cast wallet import keeper    --interactive   # Hypernative keeper: signs guard checks, proposes to Safenet
cast wallet import deployer  --interactive   # Pays gas for all deployment transactions
```

`cast wallet import` stores each key in the default Foundry keystore at
`~/.foundry/keystores/`. The aliases (`owner1`, `keeper`, etc.) are used in all
`--account` flags below.

#### Funding

| Account  | Minimum ETH | Reason |
|----------|-------------|--------|
| deployer | 0.05        | Deploy SafenetCosigner, Safe proxy, HypernativeGuard |
| owner1   | 0.01        | Configure Safe (execTransaction), execute test transaction |
| keeper   | 0.02        | DisablePassThrough, propose to Safenet |

Faucets: <https://faucets.chain.link/sepolia>, <https://www.alchemy.com/faucets/ethereum-sepolia>

The keeper also calls `proposeBasicTransaction` on Gnosis Chain — fund the keeper
address with a small amount of xDAI:
<https://faucet.gnosischain.com/?chain=gnosis>

### Environment Configuration

Copy `examples/.env.sample` to `examples/.env` and fill in the required values:

```sh
cp examples/.env.sample examples/.env
# edit examples/.env
```

Key variable groups:

| Group | Variables | Notes |
|-------|-----------|-------|
| RPC | `SEPOLIA_RPC_URL`, `GNOSIS_RPC_URL` | Any public or private endpoint |
| Consensus | `CONSENSUS_ADDRESS` | Gnosis Chain address; pre-filled in `.env.sample` |
| Safe TX Service | `SAFE_TX_SERVICE_URL`, `SAFE_TX_SERVICE_API_KEY` | Used to record the test transaction |
| Keystore aliases | `OWNER1_ACCOUNT`, `OWNER2_ACCOUNT`, `KEEPER_ACCOUNT`, `DEPLOYER_ACCOUNT` | Must match the aliases from `cast wallet import` |
| Addresses | `OWNER1_ADDRESS`, `OWNER2_ADDRESS`, `KEEPER_ADDRESS`, `DEPLOYER_ADDRESS` | EOA addresses for each keystore |
| Optional overrides | `CONSENSUS_CHAIN_ID`, `ALLOW_TX_DELAY`, `COSIGNER_SALT`, `GUARD_SALT`, `SAFE_CREATION_SALT_NONCE` | All have safe defaults; see comments in `.env.sample` |

### Running the Integration Test

```sh
npm run test:integration:cosigner
```

This runs `examples/test-cosigner-hypernative.ts` which handles both deployment
setup (Phase 0) and the integration test (Phase 1) in a single command. All setup
steps are **idempotent** — they check whether each component is already deployed
or configured and skip any step that is already complete. It is safe to re-run
the script multiple times.

#### Phase 0 — Deploy and configure

| Step | What happens | Why |
|------|-------------|-----|
| 0.1 | Fetch active epoch and FROST group key from Gnosis Chain | `SafenetCosigner` is initialised at deploy time with the live epoch and key; CREATE2 address changes if the epoch changes, so the cosigner is always deployed already in sync |
| 0.2 | Deploy `SafenetCosigner` via Safe Singleton Factory (CREATE2) | Deterministic address from constructor args + salt; skipped if code already present at the computed address |
| 0.3 | Create Safe proxy via `SafeProxyFactory.createProxyWithNonce` | Owners = [owner1, owner2, SafenetCosigner], threshold 2; cosigner address is known from step 0.2 and included at creation time, avoiding a post-creation `addOwnerWithThreshold` call |
| 0.4 | Deploy `HypernativeGuard` via Safe Singleton Factory (CREATE2) | Deployed in **pass-through mode** so the setup transaction in 0.5 does not yet require a keeper signature |
| 0.5 | Safe configuration: `setGuard(guard)` via `execTransaction` | Signed by owner1 (pre-approved executor slot) and owner2 (ECDSA); no keeper signature needed because the guard is in pass-through mode and `checkTransaction` is not yet enforced |
| 0.6 | Activate enforcement: `HypernativeGuard.disablePassThroughMode()` | Called from the keeper account; from this point every `execTransaction` requires a keeper ECDSA signature |

After Phase 0 the Safe is fully configured:

```
owners:    [owner1, owner2, SafenetCosigner]
threshold: 2
guard:     HypernativeGuard (enforcement active)
```

#### Phase 1 — Integration test

| Step | What happens | Why |
|------|-------------|-----|
| 1 | Read Safe nonce from Sepolia | Needed to compute the Safe transaction hash; each nonce is used exactly once |
| 2 | Compute `safeTxHash` | Commits to all transaction parameters; both owners sign this hash and Safenet attests over it |
| 3 | Propose transaction to Safenet (Gnosis Chain) | Keeper calls `proposeBasicTransaction` on the Consensus contract; validators produce a FROST threshold signature over `safeTxHash` |
| 4 | Poll `getRecentTransactionAttestationByHash` every 5 s | Continues until a non-zero FROST signature is returned or `ATTESTATION_TIMEOUT_SECONDS` is reached |
| 5 | Sign `safeTxHash` with owner1 | Standard ECDSA Safe owner signature via `cast wallet sign` |
| 6 | Sign `safeTxHash` with keeper | `HypernativeGuard` requires the keeper's ECDSA signature appended to `signatures` |
| 7 | Assemble 387-byte `signatures` blob | Static slots ordered ascending by owner address (Safe requirement); cosigner EIP-1271 slot points to the dynamic attestation bytes; keeper sig appended after |
| 8 | Submit to Safe Transaction Service | Records the transaction for traceability in the Safe UI |
| 9.1–9.4 | Pre-flight checks | Verify keeper `KEEPER_ROLE`, epoch still valid on cosigner, FROST dry-run via `isValidSignature`, and `eth_call` simulation — catch any configuration error before committing gas |
| 10 | Broadcast `execTransaction` via `cast send` | On-chain execution; `SafenetCosigner.isValidSignature` validates the FROST attestation, `HypernativeGuard.checkTransaction` validates the keeper signature |
| 11 | Assert nonce +1 and receipt status = success | Confirms the transaction executed successfully; nonce increase proves `execTransaction` did not revert silently |

### Troubleshooting

**`Attestation timeout after 120s`**
The Safenet validator network did not produce a FROST signature in time. Check
that `GNOSIS_RPC_URL` is reachable, `CONSENSUS_ADDRESS` is correct, and the
keeper's proposal transaction was mined. Increase `ATTESTATION_TIMEOUT_SECONDS`
if the network is slow.

**`GS026` revert on `execTransaction`**
Safe signature validation failed. Common causes: wrong owner address order in the
static slots (must be ascending by address), incorrect dynamic data offset in the
cosigner static slot (must be `2 * 65 = 130`), or a nonce mismatch between the
proposed and executed transaction.

**`HypernativeGuard: keeper signature invalid`**
The keeper signed the wrong hash, or `KEEPER_ADDRESS` does not match the address
passed to the HypernativeGuard constructor. Verify both in `examples/.env`.

**`Could not read artifact at .../SafenetCosigner.json`**
The SafenetCosigner build artifact is missing. Run `npm run build -w @safenet/contracts`
from the repository root.
