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

---

## SafenetGuardB

**File:** `SafenetGuardB.sol`

SafenetGuardB is an alternative design that separates attestation from execution and extends coverage to module transactions. It implements Safe's `BaseGuard` (both transaction guard and module guard) and gates every Safe transaction — whether owner-signed or module-initiated — behind a FROST threshold-signature attestation.

### How it works

**Pre-submission attestation.** Rather than embedding the FROST signature inside Safe's `signatures` bytes, validators or relayers call `submitAttestation` (regular transactions) or `submitModuleAttestation` (module transactions) on-chain before execution. Each call stores a `sigId = keccak256(r.x, r.y, z)` against the computed `safeTxHash`. When `checkTransaction` or `checkModuleTransaction` runs, it looks up the stored entry and deletes it on success — no signature parsing required.

**Epoch pair.** Only the two most recent epochs are retained: `$currentEpoch` and `$previousEpoch`. On every `updateEpoch` call, the current epoch shifts to the previous slot and the new epoch becomes current. Attestations are accepted for either epoch, allowing signing ceremonies that straddle a rollover boundary to complete after the epoch has advanced. A `$hasPreviousEpoch` flag guards against treating the zero-initialised previous slot as a valid epoch before any rollover has occurred — without it, a zero group key would be trivially forgeable.

**Module nonce.** Module transactions carry no Safe nonce, so the guard maintains its own sequence counter in `$moduleNonces`, keyed by `keccak256(abi.encode(safe, module, to, value, data, operation))`. The key is intentionally scoped to the full tx-param tuple rather than just `(safe, module)`: a per-module counter would serialise all operations from a module onto a single sequence, causing a deadlock if two different tx-param combinations are attested (say at nonce=0 and nonce=1) but the second arrives for execution first — the guard would reconstruct the wrong hash and reject it. With per-tx-param counters, distinct operations have independent tracks and can execute in any order. The counter is packed with the module address as `(uint256(uint160(module)) << 96) | nonce` and embedded as the `nonce` field in the module tx hash, binding both the module identity and the sequence position into the hash. The counter increments on both successful execution and cancellation. A cancelled signing ceremony's hash is permanently orphaned — `submitModuleAttestation` always uses the current nonce, so FROST verification against the new hash rejects any signature produced for the old hash.

**Escape hatch.** Owners can register a specific transaction for time-delayed execution via `allowTransaction`. After `_ALLOW_TX_DELAY` seconds the registered hash may execute without attestation via either `checkTransaction` or `checkModuleTransaction`. For module transactions the hash must be pre-computed with the current module nonce (readable via `getModuleNonce`). The guard auto-allows calls to `allowTransaction`, `cancelAllowTransaction`, and `cancelModuleAttestation` on itself, requiring only the Safe's own threshold signature.

### Pros

- Standard `signatures` bytes: wallets and dApps require no modification for the happy path.
- Pre-submitted attestations decouple validator/relayer work from execution timing.
- Full module guard coverage (`checkModuleTransaction`, `checkAfterModuleExecution`) — module-initiated transactions are subject to the same attestation requirement as owner-signed ones.
- Two-slot epoch storage bounds memory growth; older keys are discarded on each rollover.
- Per-tx-param module nonce tracks are independent, so different operations from the same module can execute in any order without deadlocking each other.
- No `$usedModuleSigs` registry needed — nonce advancement permanently invalidates old signatures.
- `cancelModuleAttestation` is auto-allowed and advances the nonce in one step, enabling a clean recovery path when a signing ceremony must be abandoned.

### Cons

- Every transaction requires an on-chain pre-submission step before `execTransaction` — two transactions instead of one for the happy path.
- Only two epochs are retained. If the epoch has advanced twice since an attestation was submitted, the corresponding group key is gone and the ceremony cannot be completed; a new signing round is needed.
- Module transaction replay protection relies entirely on the guard's own nonce counter rather than Safe's built-in nonce — additional operational care is required.
- A module escape-hatch allowance registered via `allowTransaction` is bound to the module nonce at registration time. If another module execution advances the nonce before the allowance is consumed, the allowance hash no longer matches and cannot be used.
