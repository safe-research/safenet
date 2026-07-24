# Safenet Guards

`SafenetGuard` is the canonical Safe Guard for Safenet. `SafenetGuardA` and `SafenetGuardB` are earlier explorations, retained for comparison and slated for removal once `SafenetGuard` is adopted.

---

## SafenetGuard

**File:** `SafenetGuard.sol`

SafenetGuard is a Safe *transaction* guard, assembled from focused, independently-auditable libraries so that each concern has a single, isolated audit surface. It implements Safe's `BaseTransactionGuard`, and its interface `ISafenetGuard` extends `ITransactionGuard`, so the guard hooks are part of the published ABI. Every owner-signed transaction is gated behind a FROST threshold-signature attestation, except when a matured announcement authorises execution through the nonce-free escape hatch.

**Scope.** This is a *transaction* guard: it gates only owner-signed `execTransaction` calls. Safe module executions do not invoke transaction-guard hooks, so an enabled module can move assets without an attestation or announcement. Deployments must prohibit modules or treat each enabled module as an explicit bypass of this guard's policy.

### What it reuses

- **`EpochRollover`** (`../libraries/EpochRollover.sol`) — the trusted epoch state and FROST-verified rollover, shared with the rest of Safenet rather than reimplemented in the guard.
- **`TransactionAnnouncement`** (`../libraries/TransactionAnnouncement.sol`) — the escape-hatch announcement type and nonce-free hashing (`AnnouncedTransaction`, `hash`) plus the time-windowed state (`announce` / `cancel` / `consume`).
- **`AttestationTrailer`** (`../libraries/AttestationTrailer.sol`) — recognising and decoding the inline attestation trailer from Safe's `signatures` bytes.

The structural self-call gate for the escape-hatch functions (target is the guard, zero value, `CALL` not `DELEGATECALL`, 4-byte selector) is kept inline in the guard for explicitness.

### Design

**Epoch forest.** Epoch state is delegated to `EpochRollover`, which tracks a *forest* of trusted `(group key, epoch)` pairs: any trusted pair may sign a rollover to any strictly-greater epoch, an epoch may hold more than one key (reorg branches), and every pair is kept forever. There is no single "active" epoch — `updateEpoch` names the exact `(parentKey, parentEpoch)` to roll over from, and membership is queried with `isKnownEpoch(groupKey, epoch)`.

**Attestation via signature extension.** Because the forest is keyed by key coordinates (there is no reverse "key for epoch N" lookup) and an epoch may hold several keys, the inline attestation carries the key explicitly. It is appended to Safe's `signatures` bytes as a *signature extension* — a fixed 192-byte payload followed by a 32-byte type hash:

```
[safe owner signatures] [192-byte abi.encode(epoch, Secp256k1.Point groupKey, FROST.Signature)] [32-byte TYPE_HASH]
```

Anchoring the extension at the end leaves Safe's front-to-back signature parser untouched, and the terminal type hash makes detection independent of signature suffixes — only a blob whose last word equals `TYPE_HASH` is treated as an attestation, so a valid Safe signature ending in an unrelated value (even the number 192) is never mis-parsed. The type hash doubles as an extensible format tag: it embeds the version, so a future format uses a different type hash (which this guard reads as "no trailer"). This is the first canonical guard the Safe project publishes to use signature extension, and the type-hash separator is intended as a reusable convention for future extensions.

`checkTransaction` outcomes: no type hash → falls through to the announcement path; a type hash on a too-short blob → reverts `MalformedAttestationTrailer`; a recognised trailer → the `(groupKey, epoch)` pair must be trusted (else `UntrustedAttestationKey`) and the FROST signature is verified. A recognised trailer never silently falls through. Forest membership already implies a non-zero key, so no separate non-zero check is needed.

**Nonce-free escape hatch.** Announcements are keyed by a **nonce-free** hash covering every `execTransaction` parameter *except* the Safe nonce (`getAnnouncementHash`). Owners call `announceTransaction(AnnouncedTransaction)` with the **full transaction parameters** (not a bare hash), so signers see exactly what they authorise and the guard derives the announcement hash on-chain — guaranteeing it matches what `checkTransaction` reconstructs and removing a class of silent off-chain hash-mismatch bugs. Because the hash excludes the nonce, a queued announcement survives unrelated nonce advances: owners can keep transacting via attestation while an announcement matures, and after the fixed delay any matching transaction executes without an attestation at whatever nonce is current. Announcements are single-use (consumed on execution) and can be revoked immediately, with no delay, via `cancelAnnouncement(hash)`. Both `announceTransaction` and `cancelAnnouncement` are auto-allowed self-calls, so the escape hatch never requires Safenet. A normally-attested transaction whose parameters happen to match a pending announcement takes the attestation path and does not consume the announcement.

**Bounded validity window.** Each announcement is executable only within `[activeFrom, activeUntil]` (both bounds inclusive), where `activeFrom = now + delay` and `activeUntil = activeFrom + window` (both durations fixed at construction). Bounding the tail prevents a "set and forget" announcement from remaining executable indefinitely — a critical transaction that was queued but not used cannot be triggered by a malicious party long afterward. An announcement that expires unused is inert; it can be renewed in place (`announceTransaction` overwrites an expired entry with a fresh full window), while a pending or still-active entry cannot be overwritten. Both timestamps are packed into a single storage slot (two `uint128`); `announce` rejects durations that would overflow `uint128` (`WindowOverflow`).

**Consumption tracks the authorization path, not execution success.** The announcement is consumed in the pre-execution hook along the escape-hatch path, so it records which authorization path was taken rather than whether the inner call succeeded. If the announced parameters set a non-zero `safeTxGas`/`gasPrice`, Safe may catch an inner-call failure and return `false` while the announcement stays consumed; with all-zero gas params the whole call reverts and the consumption rolls back. This is an accepted trade-off (a full lock/finalize/restore state machine across both hooks was considered and deferred).

**Single announcement event.** Announcing (including renewal) emits `TransactionAnnounced(safe, announcementHash, activeFrom, activeUntil)`. The full announced parameters are always recoverable from the `announceTransaction` calldata, so the event itself stays minimal.

**Fixed delay and window.** Both the embargo delay (`allowTransactionDelay`) and the validity window (`allowTransactionWindow`) are fixed at construction (immutable).

### Integration — attestation trailer format (v1)

Relayers that append the inline attestation must build the exact trailer the guard recognises:

```
[safe owner signatures]
[192-byte payload]     = abi.encode(uint64 epoch, Secp256k1.Point groupKey, FROST.Signature signature)   // 6 × 32-byte words
[32-byte TYPE_HASH]    = keccak256("SafenetGuard.AttestationTrailer.v1")
```

- **TYPE_HASH** = `keccak256("SafenetGuard.AttestationTrailer.v1")`
  = `0x7574ada57823dfda76df60551fc6a8662abe3441dc7b19194fb2cc08b312e436`

Total trailer overhead is exactly **224 bytes** (192 payload + 32 type hash) appended after the Safe owner signatures. Decoding (`AttestationTrailer.decode`): a blob whose last 32 bytes are not `TYPE_HASH` is *no trailer* (falls through to the announcement path); the type hash on a blob shorter than 224 bytes reverts `MalformedAttestationTrailer`.

### Design decisions

Deliberate choices with their rationale, recorded so reviewers and auditors can distinguish "intended" from "oversight." They are not defects.

**Epoch trust model**

- **Forest of `(groupKey, epoch)` pairs, kept forever, never pruned.** There is no single "active" epoch; multiple keys per epoch (reorg branches) are allowed. *Rationale:* the FROST per-participant secret shares are destroyed/rotated after an epoch, so a historical group key can never be reconstituted; keeping old pairs valid indefinitely is therefore not a practical risk. Storage grows monotonically, but each added pair requires a valid FROST signature.
- **A recorded historical key may attest future transactions and sign new rollover branches.** Accepted as a direct consequence of the above (shares no longer exist to abuse). It cannot replay past transactions, which the Safe nonce binds.
- **`updateEpoch` is permissionless** — the FROST signature is the authorization; the caller names the explicit parent pair; re-submitting a known pair is a no-op.
- **`rolloverBlock` is not checked against local `block.number`** — it is a Gnosis Chain block number, meaningless on the guard's chain, folded into the signed message only.

**Consensus binding**

- **Consensus is Gnosis-only (chain id 100); the guard keeps a local copy of the epoch forest** (cross-chain calls are infeasible). The EIP-712 domain separator is immutable from constructor args; misconfiguration is unrecoverable (redeploy).

**Attestation (owner transactions)**

- **Inline signature-extension trailer carrying `epoch + groupKey + signature` explicitly** (the forest has no epoch→key reverse lookup). The non-standard encoding (wallets must be Safenet-aware) and the larger trailer are accepted trade-offs.
- **Type-hash trailer framing** (`keccak256("SafenetGuard.AttestationTrailer.v1")`, not length-only) so a valid Safe-signature suffix cannot be mis-parsed; a recognised trailer never falls through to the announcement path.
- **Replay/ordering come from the Safe nonce** bound into the verified hash; there is no spent-signature registry.

**Escape hatch (announcements)**

- **`announceTransaction` takes the full parameter struct** (not a bare hash) — signers see what they authorize and the guard derives the hash on-chain (so it cannot diverge from `checkTransaction`).
- **Nonce-free announcement hash** (excludes the Safe nonce and the Safe address; scoped by storage key) — keeps the hatch usable while unrelated transactions advance the Safe nonce.
- **Bounded, inclusive `[activeFrom, activeUntil]` window**, both durations immutable; packed into one slot (two `uint128`), with `WindowOverflow`/constructor bounds preventing absurd values.
- **Single-use; expired entries are renewable in place**; pending/active ones cannot be overwritten; `cancelAnnouncement` is immediate.
- **Consumption tracks the authorization path, not execution success** (a caught inner-call failure with non-zero `safeTxGas`/`gasPrice` still consumes the announcement). A lock/finalize/restore state machine was considered and deferred.
- **A relayer holding both a valid attestation and a matured announcement can choose the path**; the attestation path takes precedence and does not consume the announcement.
- **A single `TransactionAnnounced` event** — the full parameters are recoverable from the announce calldata, so the event carries only the hash and window.

**Structure & scope**

- **Module-transaction guarding is intentionally not integrated** — deferred pending product requirements. Enabled Safe modules bypass this guard; deployers must prohibit modules or treat each as an explicit bypass.
- **The structural self-call gate is inlined** in the guard for explicitness, rather than extracted into a separate library.
- **Library-composed design** (`EpochRollover`, `TransactionAnnouncement`, `AttestationTrailer`): state/mechanism in libraries, FROST verification and domain events in the guard. `EpochRollover` epoch events are mirrored on `ISafenetGuard` for a single canonical integration ABI (they appear twice in the generated ABI; harmless — same topic).

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

**Module nonce.** Module transactions carry no Safe nonce, so the guard maintains its own sequence counter in `$moduleNonces`, keyed by `keccak256(abi.encode(safe, module, to, value, keccak256(data), operation))` (`data` is pre-hashed to avoid a large memory copy on every call site). The key is intentionally scoped to the full tx-param tuple rather than just `(safe, module)`: a per-module counter would serialise all operations from a module onto a single sequence, causing a deadlock if two different tx-param combinations are attested (say at nonce=0 and nonce=1) but the second arrives for execution first — the guard would reconstruct the wrong hash and reject it. With per-tx-param counters, distinct operations have independent tracks and can execute in any order. The counter is packed with the module address as `(uint256(uint160(module)) << 96) | nonce` and embedded as the `nonce` field in the module tx hash, binding both the module identity and the sequence position into the hash. The counter increments on both successful execution and cancellation. A cancelled signing ceremony's hash is permanently orphaned — `submitModuleAttestation` always uses the current nonce, so FROST verification against the new hash rejects any signature produced for the old hash.

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
