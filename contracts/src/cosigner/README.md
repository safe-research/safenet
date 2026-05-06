# Safenet Cosigner

This directory contains an EIP-1271 contract signer implementation for Safenet. Unlike the guard variants, which intercept every transaction as a Safe Guard module, the cosigner participates as a Safe owner and gates transactions through Safe's standard EIP-1271 signature verification flow.

---

## SafenetCosigner

**File:** `SafenetCosigner.sol`

SafenetCosigner is an EIP-1271 contract signer that gates Safe transactions behind FROST threshold-signature attestation produced by the Safenet validator network. It is deployed once and added as an owner of a Safe alongside the human key holders.

### How it works

**EIP-1271 integration.** Safe calls `isValidSignature(safeTxHash, contractSignature)` on any owner that is a contract. The cosigner implements this interface and returns `0x1626ba7e` when it approves the transaction. The FROST attestation travels as dynamic contract signature data inside Safe's existing `signatures` bytes — no custom trailer encoding is required.

**Signature construction.** Safe requires `signatures` entries sorted ascending by owner address. For the cosigner entry, include a contract signature slot (`v = 0`) and append the FROST attestation as dynamic data:

```
Static slot (65 bytes):
  r (bytes32) = address(cosigner) left-padded with zeros
  s (bytes32) = byte offset of the dynamic data = (total static entries) * 65
  v (uint8)   = 0x00

Dynamic data (appended after all static entries):
  uint256                             : byte length of the encoded attestation
  abi.encode(uint64, FROST.Signature) : epoch and FROST signature
```

To use the escape hatch instead, set the dynamic data to an empty byte sequence (length = 0, no following bytes).

**Replay protection.** The Safe nonce is embedded in `safeTxHash`, making every attested message unique. No separate spent-signature registry is needed.

**Epoch state.** Only the two most recent epochs are retained: `$currentEpoch` and `$previousEpoch`. On each `updateEpoch` call the current epoch shifts to previous and the new epoch becomes current. The sliding window allows in-flight transactions to complete across epoch boundaries while preventing unbounded storage growth. `updateEpoch` is permissionless — any party holding the FROST-signed rollover message can advance the epoch.

**Escape hatch.** Owners can register a specific transaction for time-delayed execution via `allowTransaction`. The call must go through the Safe's own `execTransaction`, so the Safe's owner-signature threshold is required to register an allowance — including the cosigner's approval if the human owners alone cannot reach it. After a configurable delay, passing empty bytes as the dynamic contract signature data causes the cosigner to approve via the matured allowance. Allowances are not deleted on use because `isValidSignature` is `view`; replay is prevented by Safe's nonce advancing after execution.

### Pros

- Uses Safe's standard EIP-1271 contract-signature mechanism — no custom trailer encoding required.
- Replay protection is inherited from the Safe nonce — no additional registry.
- Only two epoch keys are retained, bounding storage growth regardless of how many rollovers occur.
- `updateEpoch` is permissionless, improving liveness during validator set changes.
- Fully self-contained: no cross-chain calls at execution time.
- Escape hatch provides a liveness guarantee if Safenet is unavailable.

### Cons

- Only the two most recent epoch keys are valid. An in-flight transaction attested under an older epoch will be rejected once two subsequent rollovers have occurred.
- Registering an escape-hatch allowance requires the cosigner's approval at registration time, so it must be done proactively while Safenet is still available.
- Cancelling an allowance goes through `execTransaction` and requires the Safe's owner-signature threshold. If the remaining human owners can reach that threshold without the cosigner, cancellation is still possible even when Safenet is unavailable.
- Escape-hatch UX requires owners to proactively register a transaction hash and wait out the full delay, which may be operationally burdensome in time-sensitive situations.
