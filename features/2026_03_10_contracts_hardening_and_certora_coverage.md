# Feature Proposal: Contracts Hardening & Certora Coverage Expansion
Component: `contracts`

---

## Overview

Improve the Solidity contracts by expanding Certora formal verification coverage, adding security-critical inline documentation for the assembly-heavy `SafeTransaction.sol` and `FROST.sol` libraries, and addressing potential areas where test coverage can be strengthened.

**Phases:**

1. **Phase 1** — Inline documentation for assembly-heavy libraries
2. **Phase 2** — Expanded Certora specs for edge cases
3. **Phase 3** — Solidity test coverage improvements

---

## Architecture Decision

No architectural changes. This is a hardening initiative focused on documentation, verification, and test coverage for the existing contract suite. The contracts themselves should not change behavior.

### Alternatives Considered

- **Slither / Mythril static analysis**: These tools can complement Certora but focus on different vulnerability classes. They can be added in a separate initiative alongside CI integration.
- **Fuzzing with Foundry**: Foundry's built-in fuzzer is already used in some tests. Expanding fuzz test campaigns for the FROST math libraries would be valuable but is orthogonal to formal verification.

---

## Tech Specs

### Inline Documentation Improvements

#### `SafeTransaction.sol` (assembly-heavy EIP-712 hashing)

The `hash()` function at line 78-115 uses inline assembly with `mcopy` for gas-efficient EIP-712 hashing. While comments exist, they could be expanded:

| Location | What to document |
|---|---|
| Line 84 | `assembly ("memory-safe")` — document why the memory-safe annotation is correct (no allocation, only reads free memory pointer for scratch space) |
| Line 93 | `mcopy(add(ptr, 0x20), self, 0x40)` — document the struct memory layout assumption (chainId at offset 0, safe at offset 0x20) |
| Line 104 | `mcopy(add(ptr, 0x20), add(self, 0x40), 0x140)` — document that this copies 10 words starting from the `to` field |
| Line 106 | The `data` pointer replacement — document that `data` in memory is a pointer to a dynamic bytes array, and EIP-712 requires `keccak256(data)` instead |
| Line 113 | `keccak256(add(ptr, 0x1e), 0x42)` — document the `0x1901` prefix positioning: `0x1e = 0x20 - 2` to include the 2-byte prefix before the 32-byte domain separator |

#### `FROST.sol` and `Secp256k1.sol`

These libraries implement elliptic curve operations. Key areas needing documentation:

| Location | What to document |
|---|---|
| `Secp256k1.sol` — point validation | Document the on-curve check and why the point at infinity must be rejected |
| `FROST.sol` — signature verification | Document the Schnorr verification equation `z*G = R + c*Y` and how it maps to the RFC 9591 specification |
| `FROST.sol` — binding factor computation | Document the hash-to-scalar construction and reference the ciphersuite-specific hash functions |
| `FROST.sol` — `_expandMessageXmd` (lines 366-429) | 64 lines of dense assembly implementing RFC 9380 hash expansion with no inline comments. Each assembly block (memory layout calculations, SHA-256 loop, XOR operations) needs line-by-line documentation |
| `FROST.sol` — `bindingFactors` (lines 123-148) | Modifies input array in-place in assembly for optimization. This is intentional but dangerous and undocumented — add a warning comment explaining this is safe because the buffer is scratch space |
| `Secp256k1.sol` — `mulmuladd` (lines 139-160) | Abuses the `ecrecover` precompile for elliptic curve multiplication. The mathematical trick that makes this work should be explained in detail |

#### `FROSTCoordinator.sol`

| Location | What to document |
|---|---|
| KeyGen complaint flow | Document the three possible outcomes of a complaint: valid complaint (accused is dishonest), invalid complaint (plaintiff is dishonest), too many complaints (group is tainted) |
| Sequence number allocation | Document how the sequence counter prevents nonce reuse across signing ceremonies |
| Nonce commitment chunks | Document the Merkle root commitment scheme for pre-computed nonces and the chunk size choice (1024 = 2^10) |
| Group status transitions | The group has 5+ states (UNINITIALIZED, COMMITTING, SHARING, CONFIRMING, COMPROMISED, FINALIZED) but transitions are not explicitly asserted in state mutation code. Add comments at each state transition documenting the expected prior state |
| Lazy epoch rollover | Add `@dev` notes to all state-changing functions in `Consensus.sol` that may trigger lazy epoch rollover |

### Certora Spec Coverage

The existing Certora harness covers `Staking.sol`. Areas for expansion:

| Contract | Property to verify |
|---|---|
| `Consensus.sol` | Epoch monotonicity — staged epoch must always be > active epoch |
| `Consensus.sol` | Attestation uniqueness — a transaction hash cannot have two different attestations in the same epoch |
| `Consensus.sol` | Rollover correctness — `EpochStaged` can only be emitted after `EpochProposed` |
| `FROSTCoordinator.sol` | Group state machine — group cannot go backwards in its lifecycle (committed -> shared -> confirmed -> ready) |
| `FROSTCoordinator.sol` | Sequence monotonicity — signing ceremony sequence numbers must strictly increase |
| `Staking.sol` | Withdrawal timing — funds cannot be withdrawn before the unlock period |

### Security Observations

| Location | Issue | Severity | Action |
|---|---|---|---|
| `Consensus.sol` — `onSignCompleted()` | Uses `bytes4 selector = bytes4(context)` followed by `context[4:]` without verifying `context.length >= 4`. If `context` is shorter, the slice could be unsafe. | Medium | Add `require(context.length >= 4, InvalidContext())` |
| `FROST.sol:377` | Uses `assert(len < 0x8000)` in `_expandMessageXmd()` for input validation. `assert` consumes all remaining gas on failure, unlike `require`. | Low | Replace with `require(len < 0x8000, InvalidLength())` with a proper custom error |
| `Secp256k1.sol:261` | `assert(success)` in `_divmod()` — if the modexp precompile fails, reverts with no error context | Low | Replace with `require(success, ModularInversionFailed())` with a descriptive error |
| `FROSTSignatureId.sol:38` | Sequence encoded as `seq + 1` to distinguish from zero. If `seq == uint64.max`, this wraps to 0, breaking the invariant. | Low | Add a comment documenting this edge case or a check that `seq < type(uint64).max` |
| `Consensus.sol` — lazy epoch rollover | Epoch rollover happens lazily inside state-changing functions (documented in comments). Not a bug, but callers may not realize their transaction triggers a rollover. | Info | Add `@dev NOTE: This function may trigger lazy epoch rollover` to all affected functions |

### Solidity Test Improvements

The `Consensus.t.sol` test file is notably thin (~100 lines) compared to `FROSTCoordinator.t.sol` (~499 lines). Core transaction attestation functions (`proposeTransaction`, `attestTransaction`, callback dispatch) are entirely untested.

| Test file | Gap |
|---|---|
| `Consensus.t.sol` | **Critical**: `proposeTransaction()`, `attestTransaction()`, `proposeBasicTransaction()` have zero test coverage. The `onSignCompleted()` callback dispatch (selector-based routing) is untested. Error cases for `AlreadyAttested()`, `InvalidRollover()`, `UnknownSignatureSelector()` are not exercised. |
| `Consensus.t.sol` | Test epoch rollover with edge cases: rollover at epoch 0 (genesis), rollover when staged epoch already exists, double-staging |
| `FROSTCoordinator.t.sol` | Test `keyGenComplain()` and `keyGenComplaintResponse()` end-to-end: valid complaint leading to group taint, invalid complaint rejection, threshold-based group compromise |
| `FROSTCoordinator.t.sol` | Test callback execution via `keyGenConfirmWithCallback()` and `signShareWithCallback()` |
| `Staking.t.sol` | Test with maximum number of validators to verify gas limits. Test withdrawal queue with pathological insertion patterns (O(n) traversal risk). |
| `SafeTransaction.t.sol` | Test with empty calldata, maximum-length calldata, and all-zero transaction fields |
| `FROSTNonceCommitmentSet.t.sol` | Only ~70 lines — expand with edge cases for commitment set manipulation |

### Hardcoded Addresses in Validator

The validator's `service/checks.ts` contains hardcoded MultiSend contract addresses for versions 1.3.0, 1.4.1, and 1.5.0. While these are canonical Safe deployment addresses, they should be:

1. Documented with references to their deployment sources
2. Organized in a named constant map with version labels
3. Potentially made configurable for testnet deployments where Safe contracts may be at different addresses

---

## Implementation Phases

### Phase 1 — Inline Documentation for Assembly-Heavy Libraries (independent PR)

**Scope:** Add detailed inline comments to `SafeTransaction.sol`, `FROST.sol`, `Secp256k1.sol`, and `FROSTCoordinator.sol`.

**Files touched:**
- `contracts/src/libraries/SafeTransaction.sol` — expand assembly comments
- `contracts/src/libraries/FROST.sol` — document verification equations
- `contracts/src/libraries/Secp256k1.sol` — document point validation
- `contracts/src/FROSTCoordinator.sol` — document complaint flow, sequence numbers, nonce chunks

**No behavioral changes.** Only comments are added.

---

### Phase 2 — Certora Spec Expansion (independent PR)

**Scope:** Add new Certora specifications for `Consensus.sol` and `FROSTCoordinator.sol` properties.

**Files touched:**
- `certora/specs/Consensus.spec` — new
- `certora/specs/FROSTCoordinator.spec` — new
- `certora/harnesses/ConsensusHarness.sol` — new (if needed)
- `certora/harnesses/FROSTCoordinatorHarness.sol` — new (if needed)
- `certora/conf/` — new configuration files

---

### Phase 3 — Solidity Test Coverage Expansion (independent PR)

**Scope:** Add edge-case tests for existing contracts.

**Files touched:**
- `contracts/test/Consensus.t.sol` — add edge-case tests
- `contracts/test/FROSTCoordinator.t.sol` — add complaint flow tests
- `contracts/test/Staking.t.sol` — add gas limit tests
- `contracts/test/libraries/SafeTransaction.t.sol` — add edge-case tests
- `validator/src/service/checks.ts` — document and organize hardcoded addresses

---

## Open Questions / Assumptions

1. **Certora license**: Certora's prover requires a license. Confirm that the CI/CD pipeline has access to Certora Cloud for running the expanded specs.
2. **Assembly documentation depth**: How much detail should the assembly comments have? The current level assumes readers understand EVM memory layout. Should comments also explain EVM memory basics, or is that out of scope?
3. **FROST.sol spec complexity**: Formal verification of elliptic curve operations is non-trivial. Should the Certora specs focus on state machine properties only (which are more tractable) and defer EC math verification to Foundry fuzz tests?
4. **MultiSend addresses**: Should the hardcoded MultiSend addresses be moved to a configuration file or environment variable, or is the current approach (hardcoded with comments) acceptable given that these are canonical, immutable deployments?
