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

To use the pre-approved transaction path instead, set the dynamic data to an empty byte sequence (length = 0, no following bytes).

**Replay protection.** The Safe nonce is embedded in `safeTxHash`, making every attested message unique. No separate spent-signature registry is needed.

**Epoch state.** Only the two most recent epochs are retained: `$currentEpoch` and `$previousEpoch`. On each `updateEpoch` call the current epoch shifts to previous and the new epoch becomes current. The sliding window allows in-flight transactions to complete across epoch boundaries while preventing unbounded storage growth. `updateEpoch` is permissionless — any party holding the FROST-signed rollover message can advance the epoch.

**Pre-approved transactions.** Safe owners can register any Safe transaction for time-delayed execution by calling `allowTransaction`. Registration requires signatures from `max(threshold - 1, 1)` Safe owners over `SafeTransaction.hash` — the same hash Safe owners sign for normal `execTransaction` — and does not require a Safenet attestation, making it available even when Safenet is unavailable. `allowTransaction` accepts a `SafeTransaction.T` struct covering all Safe transaction parameters (`safe`, `to`, `value`, `data`, `operation`, `safeTxGas`, `baseGas`, `gasPrice`, `gasToken`, `refundReceiver`, `nonce`) along with a `chainId` used to domain-separate the hash.

After the configured delay, the Safe owners execute the pre-registered transaction by passing empty bytes as the cosigner's dynamic signature data; the cosigner approves it via the empty-signature path in `isValidSignature`. Registrations are not deleted on use because `isValidSignature` is `view`; replay is prevented by Safe's nonce advancing after execution.

The registered hash is nonce-bound to the Safe's nonce at registration time. If other transactions advance the nonce before execution, the registration becomes stale and must be repeated. To invalidate a pending registration, `threshold` owners can execute a dummy transaction to advance the Safe nonce.

### Pros

- Uses Safe's standard EIP-1271 contract-signature mechanism — no custom trailer encoding required.
- Replay protection is inherited from the Safe nonce — no additional registry.
- Only two epoch keys are retained, bounding storage growth regardless of how many rollovers occur.
- `updateEpoch` is permissionless, improving liveness during validator set changes.
- Fully self-contained: no cross-chain calls at execution time.
- Pre-approved transactions require no Safenet attestation, providing a liveness guarantee if Safenet is unavailable.
- `allowTransaction` uses `SafeTransaction.hash` — the same hash owners sign for normal execution — so no additional signing infrastructure is needed on the client side.
- Registration requires `max(threshold - 1, 1)` owner signatures, matching the number of human signatures needed to execute the transaction, preventing unilateral registration by a single owner.
- Any Safe transaction can be pre-approved, not just cosigner removal — `removeOwner` is the typical escape hatch but the mechanism is general.

### Cons

- Only the two most recent epoch keys are valid. An in-flight transaction attested under an older epoch will be rejected once two subsequent rollovers have occurred.
- Pre-approved transaction UX requires owners to coordinate `max(threshold - 1, 1)` signatures, register the transaction hash, and wait out the full delay, which may be operationally burdensome in time-sensitive situations.
- The registered hash is nonce-bound; if the Safe nonce advances before execution (e.g. due to another transaction), the registration must be repeated. Though, this is by design.
- Invalidating a pending registration requires `threshold` owners to execute a dummy transaction without the cosigner's approval, which is only possible if the human owners can independently reach the Safe threshold.
