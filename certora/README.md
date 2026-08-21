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

One sanity-check artifact is **advisory and expected** (it does not gate the exit code): ignore it.
`invariant_not_trivial_postcondition` fires for every parameterised storage invariant (methods that don't
touch the slot preserve it trivially). Sanity checks run at the tool default (`rule_sanity: basic`).

---

# SafenetGuard suite

Formal verification of `contracts/src/guard/SafenetGuard.sol` and its libraries (`EpochRollover`,
`TransactionAnnouncement`, `AttestationTrailer`, `SignatureExtension`, `ConsensusMessages`,
`SafeTransaction`). **The suite makes no changes to the contracts**: the harnesses expose internals and
mirrors around the deployed code, never the reverse.

This base introduces the harness and the shared spec that every concern spec builds on; the concern
specs (epoch forest, announcements, `checkTransaction`, message binding) are added on top and each is
documented here as it lands.

Last verified green: **2026-08-17** against `main` (`fd01aaa`) with **certora-cli 8.6.4**:
`SafenetGuardCommon` reports *"No errors found by Prover!"* (`exit_code=0`) with its five state invariants.

## Layout

| File | Role |
| --- | --- |
| `harnesses/SafenetGuardHarness.sol` | Base harness: packed-window accessors, the `isAutoAllowed` mirror, trailer decoders, genesis-pair getters, raw forest membership. |
| `specs/SafenetGuardCommon.spec` | Shared `methods` block, cryptography/hashing summaries, and the one-state invariants. Imported by the concern specs. |
| `conf/SafenetGuardCommon.conf` | Conf for the shared spec. |

## Property ledger

### Invariants: `SafenetGuardCommon.spec`

- `announcementWindowCoherent`: a live announcement has `activeUntil >= activeFrom`.
- `announcementWindowWidthFixed`: `activeFrom != 0 => activeUntil == activeFrom + window`.
- `timingBoundsWithinUint64`: window timestamps stay within `uint64` (no packing overflow).
- `zeroKeyNeverTrusted`: the zero point is never a trusted `(key, epoch)`.
- `announcementSentinelCoherent`: a cleared announcement slot reads back `(0, 0)`, never `(0, x != 0)`.

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
- **`hashing_length_bound = 3200`** in every conf (Safe calldata is bounded to 3200 bytes).
- **Loops run in pessimistic mode** (`optimistic_loop: false` in every conf), so no unsound loop assumption
  is made. `loop_iter = 3` is therefore a *sound, asserted* bound: a loop needing more than three
  iterations would fail the run's unwinding condition.
