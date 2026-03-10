# Feature Proposal: Validator Test Coverage & Code Quality Improvements
Component: `validator`

---

## Overview

Improve the validator workspace by addressing missing test coverage for critical modules (especially cryptographic and consensus code), fixing code smells and leftover TODOs, adding inline documentation where the logic is non-obvious, and performing targeted refactoring. This is a code quality initiative split into parallelizable phases.

**Phases:**

1. **Phase 1** — FROST cryptography test coverage and documentation
2. **Phase 2** — Gas estimation TODOs and onchain protocol cleanup
3. **Phase 3** — Storage, state machine, and watcher test coverage
4. **Phase 4** — Code smells and minor refactoring

---

## Architecture Decision

No architectural changes are introduced. All work is additive (tests, comments, naming fixes) or minor refactoring within existing modules. The goal is to improve the maintainability and security confidence of existing code without changing runtime behavior.

### Alternatives Considered

- **A single large PR**: Rejected because the scope spans many modules. Smaller, focused PRs are easier to review and less risky.
- **Extracting shared packages** (e.g., SafeTx hashing shared between explorer and validator): Deferred to a separate feature spec (`2026_03_10_cross_workspace_deduplication.md`) since it involves cross-workspace changes.

---

## Tech Specs

### Missing Test Coverage

The following modules have no dedicated test files and contain non-trivial logic:

#### Critical (cryptographic / consensus — highest priority)

| Module | Why it matters |
|---|---|
| `frost/math.ts` | Core elliptic curve operations (`evalPoly`, `evalCommitment`, `createSigningShare`, `createVerificationShare`). Incorrect math breaks the entire FROST protocol. |
| `frost/vss.ts` | Verifiable Secret Sharing: coefficient generation, commitment verification, proof-of-knowledge. A bug here can compromise key generation security. |
| `frost/secret.ts` | ECDH encryption for secret share transmission. XOR-based encryption is correct only under specific assumptions that should be validated by tests. |
| `consensus/signing/shares.ts` | Signature share creation and verification. Untested signature logic can produce invalid attestations. |
| `consensus/signing/verify.ts` | Schnorr signature verification. This is the final trust boundary for attestations. |
| `consensus/verify/rollover/hashing.ts` | Epoch rollover hash computation. Incorrect hashing would cause validators to disagree on rollover messages. |
| `consensus/verify/safeTx/hashing.ts` | SafeTx packet hash computation. This must match the Solidity implementation exactly. |

#### Important (state machine, storage, infrastructure)

| Module | Why it matters |
|---|---|
| `machine/keygen/group.ts` | Group size calculations (`calcMinimumParticipants`, `calcTreshold`) — the core BFT parameter derivation. |
| `machine/keygen/timeouts.ts` | Timeout logic drives liveness guarantees. |
| `machine/state/diff.ts` | State diffing for consensus transitions. |
| `machine/transitions/watcher.ts` | Watcher transition orchestration. |
| `watcher/backoff.ts` | Rate-limit backoff logic. |
| `consensus/storage/inmemory.ts` | In-memory storage backend (used in tests and potentially devnet). |
| `utils/math.ts` | Utility math functions. |
| `utils/errors.ts` | Error formatting for viem `BaseError`. |

### TODOs and Code Smells

| Location | Issue | Action |
|---|---|---|
| `consensus/verify/rollover/handler.ts:9` | `// TODO: verify epoch` — epoch verification is **not implemented** | Implement epoch verification or document why it is intentionally skipped for beta |
| `consensus/protocol/onchain.ts:303` | `// TODO: the gas amount per share has not been estimated` | Benchmark gas costs on a test chain and replace hardcoded estimates |
| `consensus/protocol/onchain.ts:329,339,362,481,491,501` | Six more `// TODO: this has not been estimated yet` gas values | Same as above — estimate and set correct values |
| `consensus/storage/sqlite.ts:158` | `// TODO: We can cache all our prepared SQL statements` | Implement prepared statement caching for performance |
| `consensus/storage/sqlite.ts:175` | `// TODO: Computing the participant ID from inputs does not seem like the responsibility of the client` | Refactor so participant ID resolution is the caller's responsibility |
| `consensus/storage/sqlite.ts:564` | `// TODO: feels like a code-smell that we return an input parameter` | Refactor return type to avoid echoing input |
| `machine/keygen/group.ts:45` | `calcTreshold` — typo in exported function name | Rename to `calcThreshold` across all call sites |
| `frost/math.ts:17,19,21,25,29` | Parameter names use a Unicode character `ĺ` (ĺhs) instead of ASCII `l` (lhs) | Rename to `lhs` for clarity and consistency |
| `consensus/storage/inmemory.ts:212` | Typo in error message: `"Verificatrion share"` should be `"Verification share"` | Fix the typo |
| `consensus/storage/sqlite.ts:200-229` | SQL column names passed via string concatenation (`UPDATE groups SET ${column} = ?`). While only called with hardcoded strings internally, this is a code smell. | Refactor to use explicit per-column methods or a validated column-name enum |
| `consensus/signing/nonces.ts:62` | `createNonceTree` has default `size = SEQUENCE_CHUNK_SIZE` that is never overridden anywhere in the codebase | Remove the parameter or document why it exists for extensibility |

### Inline Comments Needed

| Location | What to document |
|---|---|
| `frost/secret.ts:3-5` | The ECDH XOR encryption: explain why XOR is safe here (shared secret used exactly once, same length as plaintext), and reference the overview doc section on public-channel DKG |
| `frost/vss.ts:54-59` | `verifyCommitments` — explain that only `commitment[0]` is verified via proof-of-knowledge; other commitments are implicitly verified during secret share validation in round 2 |
| `frost/math.ts:79-97` | `evalPoly` — document that this uses Horner's method for polynomial evaluation and operates in the scalar field mod N |
| `frost/math.ts:99-111` | `evalCommitment` — document this is the elliptic curve equivalent of polynomial evaluation, used for public verification shares |
| `consensus/merkle.ts:16` | The sorted-pair Merkle tree construction: explain why `a < b` sorting is used (matches OpenZeppelin's `MerkleProof.sol` and the Solidity contract) |
| `consensus/signing/nonces.ts` | Nonce lifecycle: document why nonces are deleted after use and the reorg implications (reference the overview doc section on nonces and reorgs) |
| `service/checks.ts:51-52` | `multiSendCheck150` and `multiSendCheckCallOnly150` appear to be identical to `multiSendCheck` and `multiSendCheckCallOnly` — the only difference is `{ toZeroIsSelf: true }`. Document why 1.5.0 contracts interpret `to=address(0)` as self |
| `frost/hashes.ts:9-22` | Six different FROST hash functions with different domain separation tags (DSTs). Missing documentation on why each function is needed and which FROST RFC section each corresponds to |
| `frost/secret.ts:3-5` | ECDH uses only the x-coordinate of the shared point for XOR. Document timing assumptions about the `@noble/curves` `multiply()` operation (whether it is constant-time) |
| `machine/state/diff.ts:6-55` | State diff application logic applies diffs in a specific order (e.g., `removeEpochGroups` before new diffs). Document why ordering matters and what invariants it preserves |
| `consensus/signing/shares.ts:7-16` | Complex modular arithmetic for signature share creation. Add reference to FROST RFC 9591 section that specifies this formula |

---

## Implementation Phases

### Phase 1 — FROST Cryptography Tests and Documentation (independent PR)

**Scope:** Add unit tests for `frost/math.ts`, `frost/vss.ts`, `frost/secret.ts` and add inline comments documenting non-obvious cryptographic operations.

**Files touched:**
- `validator/src/frost/math.test.ts` — new
- `validator/src/frost/vss.test.ts` — new
- `validator/src/frost/secret.test.ts` — new
- `validator/src/frost/math.ts` — add inline comments, fix Unicode parameter names
- `validator/src/frost/vss.ts` — add inline comments
- `validator/src/frost/secret.ts` — add inline comments

**Test cases:**
- `evalPoly` — verify polynomial evaluation at known points, zero coefficient, degree-0
- `evalCommitment` — verify against independently computed elliptic curve points
- `createSigningShare` / `createVerificationShare` — verify share aggregation matches known values
- `verifyKey` — positive and negative cases
- `createCoefficients` / `createCommitments` — output shape and determinism properties
- `verifyCommitments` — valid and invalid proofs of knowledge
- `ecdh` — symmetric encryption/decryption roundtrip, verify two parties derive same shared secret

---

### Phase 2 — Gas Estimation and Onchain Protocol Cleanup (independent PR)

**Scope:** Replace all hardcoded gas TODOs with benchmarked values, fix the `calcTreshold` typo, and address the epoch verification TODO in the rollover handler.

**Files touched:**
- `validator/src/consensus/protocol/onchain.ts` — update gas estimates (7 locations)
- `validator/src/consensus/verify/rollover/handler.ts` — implement epoch verification or add documented justification
- `validator/src/machine/keygen/group.ts` — rename `calcTreshold` to `calcThreshold`
- `validator/src/machine/keygen/trigger.ts` — update import
- `validator/src/machine/keygen/group.ts` — add inline documentation for BFT parameter derivation

---

### Phase 3 — Storage, State Machine, and Watcher Tests (independent PR)

**Scope:** Add tests for untested state machine, storage, and watcher modules.

**Files touched:**
- `validator/src/machine/keygen/group.test.ts` — new
- `validator/src/machine/state/diff.test.ts` — new
- `validator/src/watcher/backoff.test.ts` — new
- `validator/src/consensus/storage/inmemory.test.ts` — new (or extend sqlite.test.ts patterns)
- `validator/src/utils/math.test.ts` — new
- `validator/src/utils/errors.test.ts` — new

**Test cases:**
- `calcMinimumParticipants` — boundary cases (2, 3, 4, large N), verify > 2/3 invariant
- `calcThreshold` — verify > 1/2 invariant
- `calcGenesisGroup` — deterministic group ID generation
- `Backoff` — rate limit classification, reset, throttle timing
- `formatError` — BaseError unwrapping, non-BaseError passthrough

---

### Phase 4 — SQLite Storage Refactoring and Inline Documentation (independent PR)

**Scope:** Address the three SQLite TODOs, add inline documentation for the merkle tree and signing modules.

**Files touched:**
- `validator/src/consensus/storage/sqlite.ts` — implement prepared statement caching, refactor participant ID resolution, clean up return type
- `validator/src/consensus/merkle.ts` — add inline comments
- `validator/src/consensus/signing/nonces.ts` — add inline comments on nonce lifecycle
- `validator/src/service/checks.ts` — add inline comments for MultiSend version differences

---

## Open Questions / Assumptions

1. **Epoch verification TODO**: The `EpochRolloverHandler.hashAndVerify` has a `// TODO: verify epoch`. Is this intentionally deferred for beta, or is it a security gap? If intentionally deferred, it should be documented. If it's a gap, it needs implementation before launch.
2. **Gas estimation approach**: Should gas estimates be derived from on-chain benchmarks (e.g., Foundry gas reports) or from testnet observation? The Foundry approach is more reproducible.
3. **In-memory storage tests**: The in-memory storage backend implements the same interface as SQLite. Should tests be structured as interface-level tests that run against both backends (test parameterization)?
4. **Unicode parameter names**: The `ĺhs` parameters in `frost/math.ts` use Unicode (accented l). Confirm these should be plain ASCII `lhs` — the current names may cause issues in some editors and CI environments.
5. **Property-based / fuzz testing**: Cryptographic operations (Lagrange coefficients, polynomial evaluation, signature shares) have mathematical properties that lend themselves well to property-based testing (e.g., Lagrange coefficients summing to expected values). Should this initiative include property-based tests (e.g., via `fast-check`), or is that a separate effort?
6. **SQL column concatenation**: The `setGroupColumn` pattern in `sqlite.ts` uses string concatenation for column names. While currently called only with hardcoded strings, should this be refactored to explicit per-column methods for defense-in-depth, or is the current approach acceptable given the internal-only usage?
