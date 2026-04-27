# Safenet Guards

This directory contains a collection of Safe Guard implementations for Safenet. Each variant explores a different design trade-off in how FROST threshold-signature attestations are delivered, verified, and stored on-chain. The goal is to evaluate approaches across dimensions such as gas cost, tooling compatibility, epoch management, and escape-hatch ergonomics before settling on a production design.

---

## SafenetGuardA

**File:** `SafenetGuardA.sol`

SafenetGuardA is the baseline design. It implements Safe's `BaseTransactionGuard` and gates every transaction behind a FROST threshold-signature attestation produced by the Safenet validator network.

### How it works

**Inline attestation.** The FROST signature travels as a trailer appended to Safe's `signatures` bytes:

```
[safe owner signatures] [abi.encode(epoch, FROST.Signature)] [uint256 length]
```

`checkTransaction` reads the last 32 bytes as a length field, slices backwards to extract the attestation, and verifies it atomically during transaction execution. Because the trailer is anchored at the end, Safe's own signature parser (which reads from the front) is unaffected.

**Replay protection.** The Safe nonce is embedded in `safeTxHash`, making every attested message unique. No separate spent-signature registry is needed.

**Epoch state.** Group public keys are stored in a `mapping(uint64 => Secp256k1.Point)` keyed by epoch number. `updateEpoch` is permissionless — any party holding the FROST-signed rollover message can advance the epoch. All historic keys remain in the mapping so in-flight transactions can complete across epoch boundaries.

**Escape hatch.** Owners can register a specific transaction for time-delayed execution via `allowTransaction`. After a configurable delay, the transaction may execute without a Safenet attestation. The guard auto-allows calls to `allowTransaction` and `cancelAllowTransaction` on itself, requiring only the Safe's own threshold signature.

### Pros

- Inline attestation requires no separate on-chain registration for the happy path.
- Replay protection is inherited from the Safe nonce — no additional registry.
- `updateEpoch` is permissionless, improving liveness during validator set changes.
- Fully self-contained: no cross-chain calls at execution time.
- Escape hatch provides a liveness guarantee if Safenet is unavailable.

### Cons

- Non-standard `signatures` encoding: wallets and dApps must be Safenet-aware to append the attestation trailer; standard Safe UIs will not work without modification.
- All historic epoch keys remain valid for new transactions. A compromised past key can sign attestations for future transactions (though not replay past ones, due to nonce binding).
- The `_epochGroupKeys` mapping grows with every epoch rotation and is never pruned.
- Escape-hatch UX requires owners to proactively register a transaction hash and wait out the full delay, which may be operationally burdensome in time-sensitive situations.
