# Plan: Signature-Extension Standard

Component: `contracts/src/libraries/` (new `SignatureExtension` library), `contracts/src/libraries/AttestationTrailer.sol`, `contracts/src/guard/SafenetGuard.sol`, `examples/attest-safe-tx.ts`, guard docs.

---

## Overview

`SafenetGuard` is the first canonical Safe guard that authorises a transaction via a **signature extension** — data appended after the Safe owner signatures in the `signatures` blob, which Safe's front-to-back signature parser ignores and the guard reads from the tail. Today that mechanism lives entirely inside `AttestationTrailer` and is hard-coded to one shape: a **fixed 192-byte** payload (`abi.encode(uint64 epoch, Secp256k1.Point groupKey, FROST.Signature signature)`) followed by a 32-byte terminal type hash (`keccak256("SafenetGuard.AttestationTrailer.v1")`). Detection and slicing both assume that exact 224-byte size.

During review (PR #604) it was noted that if signature extension is going to be a reusable Safe convention, the format has to support **variable-length payloads** — a future extension will not always be exactly 192 bytes, and a fixed-size framing cannot be parsed generically. This epic promotes the trailer format from a guard-private detail into a small, reusable **signature-extension standard**: a generic envelope any Safe integration can append and any consumer can parse from the tail without knowing the payload size in advance, plus a type-hash convention that both identifies and versions each extension. `AttestationTrailer` becomes a thin, typed consumer of that standard rather than its owner.

The guard's own payload stays a fixed 192-byte `abi.encode(...)`; only its *framing* changes (it gains a length word, keeping the same type hash). The point of the epic is the **format and the shared library**, so the next extension — whatever it is — reuses them instead of re-deriving tail-parsing logic.

This is a Solidity, tests, and examples change only. It does not touch the FROST verification, the epoch forest, or the announcement escape hatch — only how the attestation payload is framed within `signatures` and decoded.

---

## Current state

`AttestationTrailer` (post-#631–#635) exposes two internal functions over `bytes calldata signatures`:

- `hasTrailer(signatures) -> bool` — non-reverting; true iff the last 32 bytes equal `TYPE_HASH`.
- `decode(signatures) -> (epoch, groupKey, signature)` — reverts `MalformedAttestationTrailer` if shorter than 224 bytes; slices the payload at the fixed offset `length - 224` and `abi.decode`s it.

Layout: `[owner signatures][192-byte payload][32-byte TYPE_HASH]`. The 192 is baked into `_PAYLOAD_LENGTH`/`_TRAILER_LENGTH`. Detection is tail-anchored (terminal type hash) so a Safe signature that happens to end in an unrelated word is never mis-parsed, and a future format that uses a different type hash reads as "no trailer".

The tail-anchoring and type-hash-versioning are already the right primitives; the only thing missing for a standard is that **the payload length is implicit** (fixed) rather than carried in the envelope.

---

## Design decision — length-prefixed terminal envelope

Introduce a generic `SignatureExtension` library defining a single envelope:

```
[safe owner signatures]                      (parsed by Safe from the front; ignored by the extension)
[payload: N bytes]                           (extension-specific, arbitrary length)
[uint256 payloadLength = N]                  (32 bytes)
[bytes32 TYPE_HASH]                          (32 bytes; identifies the extension type AND its version)
```

- **Detection** is unchanged in spirit: read the last 32 bytes; if they equal a recognised `TYPE_HASH`, an extension of that type is present.
- **Extraction** reads `payloadLength` from the second-to-last word, then slices the `payloadLength` bytes preceding it. This makes the payload **any size** while keeping the parse fully tail-anchored and O(1) — no scanning, no front/back ambiguity.
- **Type-hash registry convention.** Each extension type is `keccak256("<Owner>.<Name>.v<N>")` (the guard keeps `SafenetGuard.AttestationTrailer.v1`). The terminal word therefore both selects the extension and pins its version; a future, post-release format change bumps to a new type hash that older consumers treat as "no extension" (forward-compatible).

Overhead is **64 bytes** (a 32-byte length + the 32-byte type hash), 32 more than v1's type-hash-only framing — the cost of self-description.

`SignatureExtension` owns recognition, bounds-checking, and slicing; it is payload-agnostic. `AttestationTrailer` is rewritten as a thin wrapper: it recognises its own type hash, calls `SignatureExtension.payload(signatures, typeHash)` to get the payload bytes, asserts the payload is its expected fixed size, and `abi.decode`s it into `(epoch, groupKey, signature)`. The guard is otherwise untouched.

Proposed `SignatureExtension` surface (to be refined in Phase 1):

```solidity
// True iff `signatures` ends with `typeHash`. Non-reverting — the consumer uses this to decide
// whether an extension of its type is present before committing to extract it.
function has(bytes calldata signatures, bytes32 typeHash) internal pure returns (bool);

// Extract the payload of a `typeHash` extension. Takes `typeHash` so it is self-verifying and safe
// to call standalone: reverts MalformedSignatureExtension if the terminal word is not `typeHash`,
// if the blob is too short to hold [payloadLength][typeHash], or if payloadLength runs past the
// front of the blob.
function payload(bytes calldata signatures, bytes32 typeHash) internal pure returns (bytes calldata);
```

Passing `typeHash` to `payload` (rather than trusting the caller to have run `has` first) removes a footgun: a direct call that skipped the presence check can never read a length word from an unrelated extension type — it reverts instead.

---

## Why two layers — `SignatureExtension` and `AttestationTrailer`

Once `SignatureExtension` exists, the guard *could* consume it directly (hold its own type hash, `abi.decode` the payload inline) and `AttestationTrailer` would not be functionally required. We deliberately keep both, because they own different concerns:

- **`SignatureExtension` is the generic transport** — bytes in, bytes out. It detects the terminal type hash, bounds-checks, and slices the payload, knowing nothing about epochs, keys, or signatures. This is the reusable standard any Safe integration parses through.
- **`AttestationTrailer` is the guard's typed codec** — it owns the only guard-specific pieces: the `SafenetGuard.AttestationTrailer.v1` type hash, the payload schema (`abi.encode(uint64 epoch, Secp256k1.Point groupKey, FROST.Signature)`, fixed 192 bytes), the fixed-size assertion, and the `MalformedAttestationTrailer` revert. It turns raw payload bytes into a typed `(epoch, groupKey, signature)`.

Reasons to keep the typed codec as its own (thin) library rather than inlining it into `checkTransaction`:

1. **Codecs merit encapsulation more than predicates do.** `GuardAutoAllow` was inlined because it was a structural boolean check (target / value / operation / selector) that reads clearly in place. `AttestationTrailer` is a *codec* — byte-slicing + `abi.decode` + size validation + a custom error — exactly the wire-format logic that is safest when named, isolated, and unit-tested on its own.
2. **The hot authorisation path stays readable.** `checkTransaction` is security-critical; consuming a typed `decode(...)` keeps it at the level of "trailer present? decode, then verify," instead of interleaving raw tail-parsing with the FROST/forest logic.
3. **Layering that models the standard.** Transport (generic) vs. typed message (specific) is a natural split, and `AttestationTrailer` is the reference worked example of building a typed extension on `SignatureExtension` — the point of publishing the format as a standard.
4. **Independent audit and test surfaces.** `SignatureExtension` is tested for envelope correctness at arbitrary payload sizes; `AttestationTrailer` is tested for the guard's specific schema. Neither suite has to reason about the other's concern.

There is no runtime cost to the split: under `via_ir` internal libraries are inlined, so this is purely code organisation.

## Alternatives considered

- **A — Keep fixed-length-per-type (status quo, extended).** Each type hash implies a documented, fixed layout; no length word. Smallest (32-byte overhead) and simplest, but every extension must be a compile-time-fixed size, and a generic parser cannot locate a variable-length payload. **Rejected:** it does not meet the "must support variable length" requirement that motivated the epic; a "standard" that only works for fixed sizes just re-states the guard-specific status quo.
- **B — Length-prefixed terminal envelope `[payload][uint256 len][bytes32 typeHash]` (chosen).** Variable length, tail-anchored, O(1) parse, +32 bytes. Small, deterministic, and the length is a plain word so bounds-checking is trivial.
- **C — Front/header-based framing** (`[typeHash][len][payload]` before or interleaved with owner sigs). **Rejected:** detection must happen from the *end* (Safe consumes owner signatures front-to-back and any prefix would corrupt that), so the discriminator has to be terminal. A front header cannot be found without first knowing the owner-signature length, which the guard does not have.
- **D — Self-describing ABI envelope** (`abi.encode(bytes32 typeHash, bytes payload)` appended). **Rejected:** the discriminator is **not terminal** — the encoding is `[offset][typeHash][length][payload]`, so the type hash sits near the *front* of the blob, not the tail. Detection must happen from the end (Safe consumes owner signatures front-to-back), so a non-terminal discriminator can't be found without first knowing the extension's own length, and the dynamic `bytes` adds an offset word on top of the length word. Reading two fixed trailing words is simpler and strictly less overhead.

The choice (B) keeps everything that already works about v1 (terminal discriminator, type-hash versioning, Safe-parser-safety) and adds exactly one word to remove the fixed-size limitation.

---

## Security considerations

- **The length word is untrusted.** `payload()` must bound-check: the blob must be at least 64 bytes (`[len][typeHash]`), and `payloadLength` must not exceed `length - 64`. Any violation reverts (fail-closed) — never returns out-of-bounds or wrapped slices. This mirrors the guard's current `MalformedAttestationTrailer` and must be equally strict.
- **Detection stays fail-closed for a recognised type.** As in v1, a recognised terminal type hash on a malformed (too-short / length-overrun) blob reverts rather than silently falling through to another authorisation path.
- **Safe's front parser is unaffected.** The envelope is a suffix; Safe reads the owner signatures from the front and ignores trailing bytes, exactly as with v1.
- **Accidental collision** (a genuine owner-signature blob ending in a registry type hash) remains a `keccak256` preimage event — infeasible; unchanged from v1.
- **No new trust in the payload.** For the guard, the extracted payload is still FROST-verified against the trusted `(groupKey, epoch)` forest; framing changes do not widen what is trusted.

---

## Not a breaking change

`SafenetGuard` is **not yet deployed to production** and nothing consumes the current trailer format on-chain, so redefining the framing is **not a breaking change**. The format is finalized as the length-prefixed envelope *before any release*, so the type hash stays `SafenetGuard.AttestationTrailer.v1` — there is no version bump and no dual-support of the old fixed-length framing. Relayers, the example script, and tests move to the new framing in the same stack. Because the guard is not yet deployed and no external tooling consumes the current trailer, this needs no migration tooling and no dual-format support. The type-hash-registry convention still governs *future* changes: a genuinely post-release format change would bump to a new type hash, which older consumers read as "no extension".

---

## Plan (PR stack)

Each phase is its own PR, stacked linearly; disjoint file sets where possible.

- **Phase 0 — this epic.** Intent, alternatives, chosen format, and plan, for reviewer context and fine-tuning before code.
- **Phase 1 — `SignatureExtension` library.** New `contracts/src/libraries/SignatureExtension.sol` (generic `has` / `payload`, strict bounds, `MalformedSignatureExtension`), with a focused unit-test suite (`test/libraries/SignatureExtension.t.sol`): present/absent detection, exact round-trip at several payload sizes (including 0 and non-multiple-of-32), and the malformed/overrun revert cases. No consumer changes yet — the library stands alone and is audit-reviewable in isolation.
- **Phase 2 — migrate `AttestationTrailer` to the length-prefixed envelope.** Rebuild `hasTrailer`/`decode` on top of `SignatureExtension`; keep the existing `SafenetGuard.AttestationTrailer.v1` type hash (the format is unreleased, so redefining it is not breaking); write the length word on the produced side and read it on the consumed side; assert the guard's payload is its fixed 192 bytes. Update `SafenetGuard.checkTransaction` only where the trailer API shifts (should be minimal). Update `SafenetGuard.t.sol`'s `_buildInlineAttestation` helper to emit the length-prefixed envelope, and the trailer/decoding tests.
- **Phase 3 — relayer + docs.** Update `examples/attest-safe-tx.ts` to build the length-prefixed envelope (`... [payload][uint256 len][typeHash]`), and update the trailer-format sections of `src/guard/README.md` / `DESIGN.md` (and add a short signature-extension spec — either a dedicated doc or a section in the library NatSpec) documenting the envelope and the type-hash registry convention.

---

## Open questions (to fine-tune before Phase 1)

1. **Library scope.** Should `SignatureExtension` also expose the owner-signature boundary (`payloadStart`) for consumers that need to separate owner sigs from the extension, or stay payload-only (the guard does not need the boundary — Safe parses the front independently)? Leaning payload-only for minimalism; the boundary is trivially derivable as `signatures.length - payloadLength - 64`, which we would document rather than add a function for unless a third-party consumer needs it.
2. **Multiple extensions.** Do we need to support *stacking* more than one extension on a single `signatures` blob (nested envelopes)? Not required by any current consumer; proposed to explicitly declare single-extension for now and leave nesting to a future version bump.
3. **Spec home.** Dedicated `contracts/src/libraries/SignatureExtension` NatSpec as the canonical spec, a section in `DESIGN.md`, or a standalone `docs/signature-extension.md`? 
4. **Naming.** `SignatureExtension` vs `SafeSignatureExtension` vs `SigExt`; `has`/`payload` vs `isPresent`/`extract`.
5. **Should the guard keep asserting a fixed 192-byte payload**, or decode leniently and let `abi.decode` enforce the shape? Leaning explicit length assertion for a clear revert.
