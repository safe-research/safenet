# SafenetGuard

**File:** `SafenetGuard.sol`

SafenetGuard is a Safe *transaction* guard, assembled from focused, independently-auditable libraries so that each concern has a single, isolated audit surface. It implements Safe's `BaseTransactionGuard`, and its interface `ISafenetGuard` extends `ITransactionGuard`, so the guard hooks are part of the published ABI. Every owner-signed transaction is gated behind a FROST threshold-signature attestation, except when a matured announcement authorises execution through the nonce-free escape hatch.

**Scope.** This is a *transaction* guard: it gates only owner-signed `execTransaction` calls. Safe module executions do not invoke transaction-guard hooks, so an enabled module can move assets without an attestation or announcement. Deployments must prohibit modules or treat each enabled module as an explicit bypass of this guard's policy.

## What it reuses

- **`EpochRollover`** (`../libraries/EpochRollover.sol`) — the trusted epoch state and FROST-verified rollover, shared with the rest of Safenet rather than reimplemented in the guard.
- **`TransactionAnnouncement`** (`../libraries/TransactionAnnouncement.sol`) — the escape-hatch announcement type and nonce-free hashing (`AnnouncedTransaction`, `hash`) plus the time-windowed state (`announce` / `cancel` / `consume`).
- **`AttestationTrailer`** (`../libraries/AttestationTrailer.sol`) — recognising and decoding the inline attestation trailer from Safe's `signatures` bytes.

The structural self-call gate for the escape-hatch functions (target is the guard, zero value, `CALL` not `DELEGATECALL`, 4-byte selector) is kept inline in the guard for explicitness.

## Design

**Epoch forest.** Epoch state is delegated to `EpochRollover`, which tracks a *forest* of trusted `(group key, epoch)` pairs: any trusted pair may sign a rollover to any strictly-greater epoch, an epoch may hold more than one key (reorg branches), and every pair is kept forever. There is no single "active" epoch — `updateEpoch` names the exact `(parentKey, parentEpoch)` to roll over from, and membership is queried with `isKnownEpoch(groupKey, epoch)`.

**Attestation via signature extension.** Because the forest is keyed by key coordinates (there is no reverse "key for epoch N" lookup) and an epoch may hold several keys, the inline attestation carries the key explicitly. It is appended to Safe's `signatures` bytes as a *signature extension* ([`SignatureExtension`](../libraries/SignatureExtension.sol)) — a 192-byte payload, a `uint256` payload-length word, and a terminal 32-byte type hash:

```
[safe owner signatures] [192-byte abi.encode(epoch, Secp256k1.Point groupKey, FROST.Signature)] [uint256 payloadLength] [32-byte TYPE_HASH]
```

Anchoring the extension at the end leaves Safe's front-to-back signature parser untouched, and the terminal *signature type hash* — so called to distinguish it from the EIP-712 message type hashes used elsewhere in Safenet — makes detection independent of signature suffixes: only a blob whose last word equals `TYPE_HASH` is treated as an attestation, so a valid Safe signature ending in an unrelated value (even the number 192) is never mis-parsed. The signature type hash doubles as an extensible format tag: it embeds the version, so a future format uses a different one (which this guard reads as "no trailer"). This is the first canonical guard the Safe project publishes to use signature extension, and the type-hash separator is intended as a reusable convention for future extensions.

`checkTransaction` outcomes: no signature type hash → falls through to the announcement path; a signature type hash on a malformed envelope, or a payload that isn't the 192-byte attestation, reverts (fail-closed — `MalformedSignatureExtension` or `MalformedAttestationTrailer` respectively); a recognised trailer → the `(groupKey, epoch)` pair must be trusted (else `UntrustedAttestationKey`) and the FROST signature is verified. A recognised trailer never silently falls through. Forest membership already implies a non-zero key, so no separate non-zero check is needed.

**Nonce-free escape hatch.** Announcements are keyed by a **nonce-free** hash covering every `execTransaction` parameter *except* the Safe nonce (`getAnnouncementHash`). Owners call `announceTransaction(AnnouncedTransaction)` with the **full transaction parameters** (not a bare hash), so signers see exactly what they authorise and the guard derives the announcement hash on-chain — guaranteeing it matches what `checkTransaction` reconstructs and removing a class of silent off-chain hash-mismatch bugs. Because the hash excludes the nonce, a queued announcement survives unrelated nonce advances: owners can keep transacting via attestation while an announcement matures, and after the fixed delay any matching transaction executes without an attestation at whatever nonce is current. Announcements are single-use (consumed on execution) and can be revoked immediately, with no delay, via `cancelAnnouncement(hash)`. Both `announceTransaction` and `cancelAnnouncement` are auto-allowed self-calls, so the escape hatch never requires Safenet. A normally-attested transaction whose parameters happen to match a pending announcement takes the attestation path and does not consume the announcement.

**Bounded validity window.** Each announcement is executable only within `[activeFrom, activeUntil]` (both bounds inclusive), where `activeFrom = now + delay` and `activeUntil = activeFrom + window` (both durations fixed at construction). Bounding the tail prevents a "set and forget" announcement from remaining executable indefinitely — a critical transaction that was queued but not used cannot be triggered by a malicious party long afterward. An announcement that expires unused is inert; it can be renewed in place (`announceTransaction` overwrites an expired entry with a fresh full window), while a pending or still-active entry cannot be overwritten. Both timestamps are packed into a single storage slot (two `uint128`); `announce` rejects durations that would overflow `uint128` (`WindowOverflow`).

**Consumption tracks the authorization path, not execution success.** The announcement is consumed in the pre-execution hook along the escape-hatch path, so it records which authorization path was taken rather than whether the inner call succeeded. If the announced parameters set a non-zero `safeTxGas`/`gasPrice`, Safe may catch an inner-call failure and return `false` while the announcement stays consumed; with all-zero gas params the whole call reverts and the consumption rolls back. This is an accepted trade-off (a full lock/finalize/restore state machine across both hooks was considered and deferred).

**Single announcement event.** Announcing (including renewal) emits `TransactionAnnounced(safe, announcementHash, activeFrom, activeUntil)`. The full announced parameters are always recoverable from the `announceTransaction` calldata, so the event itself stays minimal.

**Fixed delay and window.** Both the embargo delay (`allowTransactionDelay`) and the validity window (`allowTransactionWindow`) are fixed at construction (immutable).

## Integration — attestation trailer format

The attestation is a [`SignatureExtension`](../libraries/SignatureExtension.sol) (the canonical envelope spec lives in that library's NatSpec). A relayer builds it as:

```
[safe owner signatures]
[192-byte payload]     = abi.encode(uint64 epoch, Secp256k1.Point groupKey, FROST.Signature signature)   // 6 × 32-byte words
[uint256 payloadLength] = 192
[32-byte TYPE_HASH]    = keccak256("SafenetGuard.AttestationTrailer.v1")
```

- **TYPE_HASH** = `keccak256("SafenetGuard.AttestationTrailer.v1")`
  = `0x7574ada57823dfda76df60551fc6a8662abe3441dc7b19194fb2cc08b312e436`

Total trailer overhead is exactly **256 bytes** (192 payload + 32 length + 32 type hash) appended after the Safe owner signatures. Decoding (`AttestationTrailer.decode`): a blob whose last 32 bytes are not `TYPE_HASH` is *no trailer* (falls through to the announcement path); a recognised type hash over a malformed envelope reverts `MalformedSignatureExtension`, and a payload that isn't 192 bytes reverts `MalformedAttestationTrailer`.

## Design decisions

The accepted design decisions and their rationale are documented in [DESIGN.md](./DESIGN.md).
