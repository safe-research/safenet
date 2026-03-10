# Feature Proposal: FROST Cryptographic and Signing Test Coverage
Component: `validator`

---

## Overview

The validator's FROST cryptographic primitives — the security foundation of the entire Safenet protocol — currently have **zero unit test coverage**. This includes signature verification, signature share creation, VSS (Verifiable Secret Sharing) functions, elliptic curve math operations, ECDH encryption, and protocol-level hashing functions. Additionally, the signing ceremony helper functions (Lagrange coefficients, group challenges, binding factors, nonce operations) and Safe transaction hashing utilities are untested.

Recent commits have actively hardened security in other areas (MultiSend v1.5.0 self-call prevention in `9932a1c`, epoch rollover verification in `ccafe37`), and contract events were enriched to support better attestation tracking (`ce5e8d3`, `2268ffc`). However, the core cryptographic layer that these features depend on remains unverified by automated tests.

This feature adds comprehensive unit tests across three phases, each suitable for an independent PR:

1. **Phase 1**: FROST math primitives and VSS functions (`frost/math.ts`, `frost/vss.ts`, `frost/secret.ts`)
2. **Phase 2**: Signing ceremony functions (`consensus/signing/verify.ts`, `consensus/signing/shares.ts`, `consensus/signing/group.ts`, `consensus/signing/nonces.ts`)
3. **Phase 3**: Protocol hashing and ID computation (`consensus/verify/safeTx/hashing.ts`, `consensus/verify/rollover/hashing.ts`, `consensus/keyGen/utils.ts`)

---

## Architecture Decision

No new components or architecture changes are introduced. This feature exclusively adds co-located test files (`.test.ts`) following the existing validator test conventions. The tests use known test vectors and round-trip verification to ensure correctness without requiring changes to production code.

### Alternatives Considered

- **Integration-only testing**: FROST operations are already exercised end-to-end in integration tests (`scripts/run_integration_test.sh`), but integration tests are slow, coarse-grained, and don't isolate individual function correctness. Unit tests provide faster feedback and pinpoint failures to specific functions.
- **Snapshot/golden-file testing**: Using pre-computed test vectors from the FROST RFC. This is useful for cross-implementation compatibility but doesn't cover edge cases. We combine both approaches: RFC-aligned vectors where applicable plus targeted edge case tests.
- **Formal verification via Certora**: Certora currently covers only `Staking.sol`. Extending it to the TypeScript cryptographic layer is not feasible with the current tooling; unit tests are the appropriate verification method here.

---

## User Flow

Not applicable — this is an internal test coverage improvement with no user-facing changes.

---

## Tech Specs

### Phase 1: FROST Math Primitives and VSS

**New test files:**

- `validator/src/frost/math.test.ts`
  - `g()`: scalar multiplication against generator, identity element edge case, invalid scalar handling
  - `neg()`, `addmod()`, `submod()`, `mulmod()`, `divmod()`: modular arithmetic correctness (commutativity, associativity, inverse properties, overflow wrapping)
  - `toPoint()`, `pointFromBytes()`: round-trip serialization, invalid coordinate rejection
  - `scalarToBytes()`, `scalarFromBytes()`: round-trip serialization, boundary values (0, N-1)
  - `evalPoly()`: known polynomial evaluations, zero-x rejection (throws), single-coefficient (constant) case
  - `evalCommitment()`:
    - For `x != 0`: commitment evaluation consistency with `evalPoly` (i.e., `evalCommitment(g(coeffs), x) == g(evalPoly(coeffs, x))`)
    - For `x == 0`: returns `commitments[0]` directly without throwing (diverges from `evalPoly` behaviour — must be tested explicitly)
  - `createVerificationShare()`: aggregation across multiple commitment maps, empty map error
  - `createSigningShare()`: aggregation of secret shares, empty map error
  - `verifyKey()`: valid key pair verification, mismatched key rejection

- `validator/src/frost/vss.test.ts`
  - `createEncryptionKey()`: returns valid key pair (public key = g(secret key))
  - `createCoefficients()`: returns the correct number of coefficients equal to the threshold (no non-zero assertion — the code has no explicit guard; non-zero is probabilistically certain via SHA-256 derivation but not deterministically testable)
  - `createProofOfKnowledge()`: proof verifies against own commitments
  - `createCommitments()`: commitments match g(coefficient) for each coefficient
  - `verifyCommitments()`: valid proof passes, tampered proof fails, wrong ID fails

- `validator/src/frost/secret.test.ts`
  - `ecdh()`: XOR-based symmetric cipher using an EC shared secret — three test cases:
    1. **Round-trip**: `ecdh(ecdh(msg, alicePriv, bobPub), bobPriv, alicePub) === msg` (XOR is self-inverse when both sides derive the same shared secret x-coordinate)
    2. **Shared secret commutativity**: `ecdh(msg, alicePriv, bobPub) === ecdh(msg, bobPriv, alicePub)` (holds because `alicePriv * bobPub = bobPriv * alicePub` on the curve)
    3. **Different keys produce different ciphertext**: `ecdh(msg, alicePriv, bobPub) !== ecdh(msg, alicePriv, carolPub)` for distinct recipients

### Phase 2: Signing Ceremony Functions

**New test files:**

- `validator/src/consensus/signing/verify.test.ts`
  - `verifySignature()`: valid FROST group signature verifies, tampered message fails, tampered commitment fails, identity point (0,0) short-circuits to `false`
  - `verifySignatureShare()`:
    - Valid individual share verifies
    - Incorrect share fails
    - Explicitly validates that only the **x-coordinate** is compared (`sG.x === r.x`), consistent with the FROST spec — a share differing only in the y-coordinate would still pass

- `validator/src/consensus/signing/shares.test.ts`
  - `lagrangeChallenge()`: multiplication of Lagrange coefficient and challenge
  - `createSignatureShare()`: deterministic output given fixed inputs, algebraic correctness (`share = hidingNonce + bindingNonce * bindingFactor + lagrangeChallenge * privateKey`)

- `validator/src/consensus/signing/group.test.ts`
  - `groupChallenge()`: deterministic output for same inputs, different output for different messages
  - `lagrangeCoefficient()`: known values for small signer sets (e.g., signers [1,3] id=1), signer-not-in-set error

- `validator/src/consensus/signing/nonces.test.ts`
  - `generateNonce()`: deterministic with fixed randomness, invalid randomness length rejection
  - `generateNonceCommitments()`: commitments match g(nonce) for hiding and binding
  - `createNonceTree()`: correct tree size, valid Merkle root
  - `bindingPrefix()`: deterministic for same inputs, different output for different messages or signer sets
  - `bindingFactors()`: deterministic for same inputs, one factor per signer
  - `groupCommitmentShare()`: algebraic correctness (hiding + binding * factor)
  - `groupCommitementShares()` _(note: exported with this spelling)_: returns one share per signer, each matching the result of `groupCommitmentShare()` applied individually
  - `calculateGroupCommitment()`: sum of shares
  - `decodeSequence()`: chunk/offset calculation for known values (e.g., sequence 0 → chunk 0 / offset 0, sequence 1023 → chunk 0 / offset 1023, sequence 1024 → chunk 1 / offset 0)
  - `nonceCommitmentsWithProof()`: proof verifies against tree root

### Phase 3: Protocol Hashing and ID Computation

**New test files:**

- `validator/src/consensus/verify/safeTx/hashing.test.ts`
  - `safeTxHash()`: deterministic for same transaction, matches expected EIP-712 typed data hash
  - `safeTxStructHash()`: deterministic struct hash without domain
  - `safeTxProposalHash()`: deterministic for same proposal
  - `safeTxPacketHash()`: composes `safeTxHash` and `safeTxProposalHash` correctly

- `validator/src/consensus/verify/rollover/hashing.test.ts`
  - `epochRolloverHash()`: deterministic for same packet, different output for different epoch values or chain/consensus address, matches expected EIP-712 typed data hash

- `validator/src/consensus/keyGen/utils.test.ts`
  - `calcGroupId()`: deterministic for same inputs, last 8 bytes are zeroed (mask verification), different inputs produce different IDs

### Test data strategy

- Use deterministic scalar values (e.g., `1n`, `2n`, `42n`) for reproducible test cases
- Use `generateNonce(secret, fixedRandomness)` for deterministic nonce generation in tests
- Construct minimal FROST signing ceremonies (threshold=2, participants=3) for end-to-end signing round-trip tests within Phase 2

---

## Implementation Phases

### Phase 1: FROST Math Primitives and VSS Tests

**Files created:**
- `validator/src/frost/math.test.ts`
- `validator/src/frost/vss.test.ts`
- `validator/src/frost/secret.test.ts`

**Files read (not modified):**
- `validator/src/frost/math.ts`
- `validator/src/frost/vss.ts`
- `validator/src/frost/secret.ts`
- `validator/src/frost/hashes.ts` (already tested, referenced for test vector generation)
- `validator/src/frost/types.ts`

**Estimated scope:** ~300 lines of test code across 3 files.

### Phase 2: Signing Ceremony Function Tests

**Files created:**
- `validator/src/consensus/signing/verify.test.ts`
- `validator/src/consensus/signing/shares.test.ts`
- `validator/src/consensus/signing/group.test.ts`
- `validator/src/consensus/signing/nonces.test.ts`

**Files read (not modified):**
- `validator/src/consensus/signing/verify.ts`
- `validator/src/consensus/signing/shares.ts`
- `validator/src/consensus/signing/group.ts`
- `validator/src/consensus/signing/nonces.ts`
- `validator/src/frost/math.ts` (for constructing test data)

**Estimated scope:** ~400 lines of test code across 4 files. This is the most complex phase as it requires constructing valid FROST signing ceremonies.

**Note:** Phases 1 and 2 can be parallelized since Phase 2 tests only depend on the production code from Phase 1, not on Phase 1 tests.

### Phase 3: Protocol Hashing and ID Computation Tests

**Files created:**
- `validator/src/consensus/verify/safeTx/hashing.test.ts`
- `validator/src/consensus/verify/rollover/hashing.test.ts`
- `validator/src/consensus/keyGen/utils.test.ts`

**Files read (not modified):**
- `validator/src/consensus/verify/safeTx/hashing.ts`
- `validator/src/consensus/verify/safeTx/schemas.ts`
- `validator/src/consensus/verify/rollover/hashing.ts`
- `validator/src/consensus/verify/rollover/schemas.ts`
- `validator/src/consensus/keyGen/utils.ts`

**Estimated scope:** ~200 lines of test code across 3 files. Can be parallelized with Phases 1 and 2.

---

## Open Questions / Assumptions

- **FROST RFC test vectors**: The [FROST RFC (draft-irtf-cfrg-frost)](https://www.ietf.org/archive/id/draft-irtf-cfrg-frost-15.html) provides test vectors for secp256k1-SHA256. These should be used where applicable (especially for `h1`–`h5` hash functions, which are already tested in `hashes.test.ts`) to ensure cross-implementation compatibility. The existing `hashes.test.ts` can serve as a reference for the vector format.
- **Randomness in tests**: Functions like `createEncryptionKey()`, `createCoefficients()`, and `generateNonceCommitments()` use `randomBytes()` internally. Tests should verify structural properties (e.g., public key = g(secret key)) rather than exact output values, unless the randomness source is injectable.
- **Test execution time**: Elliptic curve operations are computationally expensive. Large nonce trees (`createNonceTree` default size of 1024) should use smaller sizes in tests to keep execution fast. The `size` parameter is already configurable.
- **No production code changes**: This feature is strictly additive (test files only). No refactoring of production code is planned, though some functions could benefit from dependency injection for testability (e.g., injectable randomness). Such refactoring is out of scope.
