# Formal Verification

Certora/CVL formal-verification specifications for the Safenet contracts. This directory currently
covers two suites: the pre-existing **Staking** specs, and the **SafenetGuard** suite documented below.

## Installation

From the root of the repository:

```sh
python3 -m venv venv
source venv/bin/activate
pip install -r certora/requirements.txt
```

Additional requirements:

- **`solc` 0.8.30** on `PATH` (e.g. via `solc-select`). On non-x86 hosts (aarch64) the Solidity binaries
  run under `qemu-user-static`.
- **A JRE** (Java) must be installed: the local CVL type-checker needs it, and `certoraRun` aborts with a
  "build failed" error without it.
- A `CERTORAKEY` for the cloud prover.

## Running

From a `venv`-activated shell:

```sh
# A single concern:
CERTORAKEY=... certoraRun certora/conf/SafenetGuardCommon.conf --wait_for_results all

# Cheap local check (compile + CVL type-check only, no cloud run):
certoraRun certora/conf/SafenetGuardCommon.conf --compilation_steps_only
```

There is **no CI job** for Certora: run the specs manually and read the report; the goal is *verified
properties*, not a green exit code.

Two sanity-check artifacts are **advisory and expected** (they do not gate the exit code): ignore them.
`invariant_not_trivial_postcondition` fires for every parameterised storage invariant (methods that don't
touch the slot preserve it trivially), and `rule_not_vacuous` can fire on the `@withrevert` /
reverting-summary rules. Sanity checks run at the tool default (`rule_sanity: basic`): an `advanced` run was
tried on `SafenetGuardCheckTx.conf` and flagged **all** of that spec's rules as assert-tautologies, a
systematic false positive for the `@withrevert` + reverting-summary style (it flags rules that are
demonstrably not tautologies, e.g. the authorization-completeness and fail-closed rules), so it yields no
actionable signal and roughly doubles prover time. `advanced` was not run on the other specs.

---

# SafenetGuard suite

Formal verification of `contracts/src/guard/SafenetGuard.sol` and its libraries (`EpochRollover`,
`TransactionAnnouncement`, `AttestationTrailer`, `SignatureExtension`, `ConsensusMessages`,
`SafeTransaction`). **The suite makes no changes to the contracts**: the harnesses expose internals and
mirrors around the deployed code, never the reverse.

This base introduces the harness and the shared spec that every concern spec builds on; the concern
specs (epoch forest, announcements, `checkTransaction`, message binding) are added on top and each is
documented here as it lands.

Last verified green: **2026-08-17** against `main` (`fd01aaa`) with **certora-cli 8.6.4**. Every conf
documented below reports *"No errors found by Prover!"* (`exit_code=0`).

## Layout

| File | Role |
| --- | --- |
| `harnesses/SafenetGuardHarness.sol` | Base harness: packed-window accessors, the `isAutoAllowed` mirror, trailer decoders, genesis-pair getters, raw forest membership. |
| `specs/SafenetGuardCommon.spec` | Shared `methods` block, cryptography/hashing summaries, and the one-state invariants. Imported by the concern specs. |
| `specs/SafenetGuardEpoch.spec` | Epoch-forest rules and the genesis invariant. |
| `specs/SafenetGuardAnnouncements.spec` | Announcement lifecycle and hash field-separation rules. |
| `specs/SafenetGuardCheckTx.spec` | `checkTransaction` authorization rules. |
| `conf/SafenetGuard*.conf` | One conf per spec. |

## Property ledger

### Invariants: `SafenetGuardCommon.spec` (+ genesis, in `SafenetGuardEpoch.spec`)

- `announcementWindowCoherent`: a live announcement has `activeUntil >= activeFrom`.
- `announcementWindowWidthFixed`: `activeFrom != 0 => activeUntil == activeFrom + window`.
- `timingBoundsWithinUint64`: window timestamps stay within `uint64` (no packing overflow).
- `zeroKeyNeverTrusted`: the zero point is never a trusted `(key, epoch)`.
- `announcementSentinelCoherent`: a cleared announcement slot reads back `(0, 0)`, never `(0, x != 0)`.
- `genesisPairAlwaysKnown`: `(initialGroupKey, initialEpoch)` is trusted in every reachable state (the base case for "trust chains back to genesis").

### Epoch forest: `SafenetGuardEpoch.spec`

- `epochForestAppendOnly`: a trusted pair is never removed.
- `onlyUpdateEpochAddsPair`: only `updateEpoch` extends the forest.
- `updateEpochRecordsChild` / `updateEpochRecordsOnlyChild`: a rollover records exactly the named child.
- `updateEpochRequiresKnownParent` / `updateEpochRequiresAdvancingEpoch`: the two revert preconditions.
- `updateEpochRequiresVerifyingProof`: a rollover reverts without a verifying FROST proof (the control-flow twin of `failedAttestationNeverConsumesAnnouncement`).
- `updateEpochSucceedsFromKnownParent`: completeness, those are the *only* gates (never reverts on a valid call).
- `updateEpochIdempotent`: re-submitting a known pair is a no-op (no revert, no state change).
- `updateEpochOutcomeIndependentOfSender`: permissionless, revert outcome and state effect don't depend on `msg.sender`.
- `immutablesNeverChange`: the configured delay/window/domain never change.

### Announcements: `SafenetGuardAnnouncements.spec`

- `announcementsCallerIsolation`: a call only ever touches `$announcements[msg.sender][*]`.
- `onlyAnnounceCreatesEntry` / `onlyCancelOrConsumeClearsEntry`: write provenance.
- `announceTouchesOnlyItsOwnSlot` / `cancelTouchesOnlyItsOwnSlot`: slot locality (other `(safe, hash)` untouched).
- `announcementWindowsFrozenOutsideApi`: only the announcement API may change any window (frozen elsewhere).
- `announceCreatesWindow` / `announceRevertsWhilePending`: create semantics; renewal only once expired.
- `cancelClearsWindow` / `cancelRevertsIfAbsent`: cancel semantics.
- `announceSucceedsWhenAbsentOrExpired` / `cancelSucceedsWhenPresent`: liveness (announce/cancel always available).
- `announcementHashSeparates{To,Value,Data,Operation,SafeTxGas,BaseGas,GasPrice,GasToken,RefundReceiver}`: the announcement hash binds every parameter.

### `checkTransaction` authorization: `SafenetGuardCheckTx.spec`

- `checkTransactionRevertsWithoutAuthorization`: reverts unless auto-allowed / valid attestation / matured announcement.
- `autoAllowedNeverReverts` / `autoAllowedChangesNoState`: the auto-allow (announce/cancel self-call) path succeeds and mutates no state.
- `guardRejectsNativeValue`: no payable entry point.
- `attestationPathRequiresKnownEpoch`: the trust check precedes verification.
- `malformedTrailerFailsClosed` / `untrustedTrailerNeverFallsThrough`: a recognised trailer never silently downgrades to the announcement path.
- `trustedAttestationAlwaysAuthorizes`: liveness of the attestation path.
- `maturedAnnouncementAlwaysAuthorizes`: liveness of the escape hatch.
- `checkTransactionConsumesAnnouncement` / `consumeTouchesOnlyItsOwnSlot`: single-use consume; slot locality.
- `attestationDoesNotConsumeAnnouncement` / `failedAttestationNeverConsumesAnnouncement`: a trailer never consumes an announcement.
- `checkTransactionNeverExtendsForest`: `checkTransaction` never mutates the epoch forest.
- `checkAfterExecutionNoOp`: `checkAfterExecution` never reverts and changes no storage.

## Assumptions & scope

- **Cryptography is not modelled in CVL.** `FROST.verify` is summarised, but its *verdict* stays symbolic
  (`frostVerifyModel`) so the guard's fail-closed control flow is inside the verified boundary;
  `Secp256k1.requireNonZero` is modelled to reject only the zero point (recovering `zeroKeyNeverTrusted`).
  The real `requireNonZero` also reverts `NotOnCurve` for off-curve points; the model does not (a safety
  superset), so R-01's "only gates" liveness holds only modulo this on-curve check. Cryptographic soundness
  (which signatures/keys actually verify) is out of scope and covered by Foundry.
- **Hashing is opaque.** `ConsensusMessages.{domain,epochRollover,transactionProposal}` are `NONDET` in the
  concern specs; the byte values are irrelevant to the guard's control flow (`FROST.verify` is itself
  summarised). `_.nonce()` is summarised to a ghost `safeNonce`.
- **`_isAutoAllowed` is `private`.** The harness `isAutoAllowed` mirror re-expresses that gate so specs can
  call it `envfree`; it is pinned to the real gate *behaviourally* by four load-bearing rules, never a
  direct call: `autoAllowedNeverReverts` (not too permissive) and `checkTransactionRevertsWithoutAuthorization`
  (not too restrictive) cover the no-trailer input space; `attestationPathRequiresKnownEpoch` closes the
  well-formed-trailer region (it *asserts* pre-state key membership) and `malformedTrailerFailsClosed` closes
  the malformed-trailer sliver the former prunes. Together they sandwich the mirror onto the contract's decision.
- **`hashing_length_bound = 3200`** in every conf (Safe calldata is bounded to 3200 bytes).
- **Loops run in pessimistic mode** (`optimistic_loop: false` in every conf), so no unsound loop assumption
  is made. `loop_iter = 3` is therefore a *sound, asserted* bound: a loop needing more than three
  iterations would fail the run's unwinding condition.

## Foundry cross-checks

Properties that CVL abstracts are pinned by Foundry tests:

- `SafenetGuardTest.test_announcementHash_separatesSameLengthData`: the announcement hash binds `data` by
  content, not merely by length (the CVL family separates `data` only by length, so the same-length case
  is pinned here).
- `AttestationTrailerTest.testFuzz_parseTotalAndFailClosed`: the trailer parser is total and fail-closed
  (`hasTrailer` never reverts; a recognised trailer either decodes a full 256-byte payload or reverts,
  never reading out of bounds), the property `malformedTrailerFailsClosed` relies on.
