// Epoch-forest rules: R-EPOCH-1..3 plus the write-provenance frame conditions (F-02). The trusted
// `(group key, epoch)` forest is seeded at construction and only ever extended, only by `updateEpoch`,
// and a rollover records exactly the one named child off an already-trusted parent.
//
// Note on scope: FROST's arithmetic is summarised but its verdict is symbolic (`frostVerifyModel` in
// SafenetGuardCommon.spec): it can fail, which is why the liveness/idempotence rules `require
// frostAccepts`. These rules prove the *structural* trust chain (parent must be known, the epoch must
// advance, exactly the child is recorded); the cryptographic soundness of the rollover proof is covered
// by Foundry tests.
//
// Imports the shared methods/summaries/invariants from SafenetGuardCommon.spec.

import "SafenetGuardCommon.spec";

methods {
    // Genesis `(key, epoch)` captured by the harness from the constructor args.
    function genesisEpoch() external returns (uint64) envfree;
    function genesisKeyX() external returns (uint256) envfree;
    function genesisKeyY() external returns (uint256) envfree;
}

// ------------------------------------------------------------------------------------------------------
// Genesis trust (base case for "trust chains back to genesis"): the `(initialGroupKey, initialEpoch)` pair
// seeded at construction is trusted in every reachable state. The base case runs the constructor (which
// records it, rejecting a zero key); the append-only forest (R-EPOCH-1) preserves it thereafter. Every
// other trusted pair descends from this one via `updateEpoch` off an already-trusted parent (R-EPOCH-3).
// ------------------------------------------------------------------------------------------------------

invariant genesisPairAlwaysKnown()
    isKnownEpochRaw(genesisKeyX(), genesisKeyY(), genesisEpoch());

// ------------------------------------------------------------------------------------------------------
// R-EPOCH-1: the forest is append-only, a trusted `(key, epoch)` pair is never removed.
// ------------------------------------------------------------------------------------------------------

rule epochForestAppendOnly(env e, method f, calldataarg args, Secp256k1.Point groupKey, uint64 epoch)
    filtered { f -> !f.isView && !f.isPure }
{
    require isKnownEpoch(groupKey, epoch);

    f(e, args);

    assert isKnownEpoch(groupKey, epoch), "a trusted (key, epoch) pair is never removed";
}

// ------------------------------------------------------------------------------------------------------
// F-02 / M-3: write provenance: only `updateEpoch` ever extends the trusted forest. Together with
// R-EPOCH-3's per-transition gating this is what makes the forest's trust chain sound.
// ------------------------------------------------------------------------------------------------------

rule onlyUpdateEpochAddsPair(env e, method f, calldataarg args, Secp256k1.Point groupKey, uint64 epoch)
    filtered { f -> !f.isView && !f.isPure }
{
    require !isKnownEpoch(groupKey, epoch);

    f(e, args);

    assert isKnownEpoch(groupKey, epoch) =>
        f.selector == sig:updateEpoch(
            Secp256k1.Point, uint64, uint64, uint64, Secp256k1.Point, FROST.Signature).selector,
        "only updateEpoch ever extends the trusted forest";
}

// ------------------------------------------------------------------------------------------------------
// R-EPOCH-2: a successful rollover records the proposed child `(newGroupKey, proposedEpoch)`.
// ------------------------------------------------------------------------------------------------------

rule updateEpochRecordsChild(
    env e,
    Secp256k1.Point parentKey,
    uint64 parentEpoch,
    uint64 proposedEpoch,
    uint64 rolloverBlock,
    Secp256k1.Point newGroupKey,
    FROST.Signature signature
) {
    updateEpoch(e, parentKey, parentEpoch, proposedEpoch, rolloverBlock, newGroupKey, signature);

    assert isKnownEpoch(newGroupKey, proposedEpoch), "a successful rollover records the child (key, epoch)";
}

// ------------------------------------------------------------------------------------------------------
// F-02 / M-15: a rollover records ONLY the named child: membership is exact on the `(x, y, epoch)`
// triple, never a family (e.g. it must not trust the negation `(x, p - y)` at the same epoch).
// ------------------------------------------------------------------------------------------------------

rule updateEpochRecordsOnlyChild(
    env e,
    Secp256k1.Point parentKey,
    uint64 parentEpoch,
    uint64 proposedEpoch,
    uint64 rolloverBlock,
    Secp256k1.Point newGroupKey,
    FROST.Signature signature,
    Secp256k1.Point otherKey,
    uint64 otherEpoch
) {
    require otherKey.x != newGroupKey.x || otherKey.y != newGroupKey.y || otherEpoch != proposedEpoch;

    bool knownBefore = isKnownEpoch(otherKey, otherEpoch);

    updateEpoch(e, parentKey, parentEpoch, proposedEpoch, rolloverBlock, newGroupKey, signature);

    assert isKnownEpoch(otherKey, otherEpoch) == knownBefore,
        "a rollover records only the named (newGroupKey, proposedEpoch) pair";
}

// ------------------------------------------------------------------------------------------------------
// R-EPOCH-3: a rollover is only accepted off an already-trusted parent at a strictly-advancing epoch.
// (The FROST proof over the rollover message is assumed valid; see the scope note above.)
// ------------------------------------------------------------------------------------------------------

rule updateEpochRequiresKnownParent(
    env e,
    Secp256k1.Point parentKey,
    uint64 parentEpoch,
    uint64 proposedEpoch,
    uint64 rolloverBlock,
    Secp256k1.Point newGroupKey,
    FROST.Signature signature
) {
    require e.msg.value == 0; // non-payable
    require !isKnownEpoch(parentKey, parentEpoch);

    updateEpoch@withrevert(e, parentKey, parentEpoch, proposedEpoch, rolloverBlock, newGroupKey, signature);

    assert lastReverted, "updateEpoch must revert when the parent (key, epoch) is not trusted";
}

rule updateEpochRequiresAdvancingEpoch(
    env e,
    Secp256k1.Point parentKey,
    uint64 parentEpoch,
    uint64 proposedEpoch,
    uint64 rolloverBlock,
    Secp256k1.Point newGroupKey,
    FROST.Signature signature
) {
    require e.msg.value == 0; // non-payable
    require proposedEpoch <= parentEpoch;

    updateEpoch@withrevert(e, parentKey, parentEpoch, proposedEpoch, rolloverBlock, newGroupKey, signature);

    assert lastReverted, "updateEpoch must revert unless the proposed epoch strictly advances";
}

// ------------------------------------------------------------------------------------------------------
// R-EPOCH-4: a rollover reverts without a verifying FROST proof (the control-flow half of the rollover's
// crypto guarantee, twin of `failedAttestationNeverConsumesAnnouncement`). Kills a mutant that drops the
// `FROST.verify` call from `rollover`; the message-binding half is R-CHK-4 + Foundry (see Common's A-1).
// ------------------------------------------------------------------------------------------------------

rule updateEpochRequiresVerifyingProof(
    env e,
    Secp256k1.Point parentKey,
    uint64 parentEpoch,
    uint64 proposedEpoch,
    uint64 rolloverBlock,
    Secp256k1.Point newGroupKey,
    FROST.Signature signature
) {
    require e.msg.value == 0; // non-payable
    require !frostAccepts; // the rollover proof does NOT verify (F-06 model)

    updateEpoch@withrevert(e, parentKey, parentEpoch, proposedEpoch, rolloverBlock, newGroupKey, signature);

    assert lastReverted, "a rollover must revert when the FROST proof does not verify";
}

// ------------------------------------------------------------------------------------------------------
// F-21 / M-18: the guard's configuration immutables never change: pins the configuration story that INV-2
// is expressed in terms of, and would fail loudly if the delay/window/domain were ever made governable.
// ------------------------------------------------------------------------------------------------------

rule immutablesNeverChange(env e, method f, calldataarg args) filtered { f -> !f.isView && !f.isPure } {
    uint256 delayBefore = getAllowTxDelay();
    uint256 windowBefore = getAllowTxWindow();
    bytes32 domainBefore = getConsensusDomainSeparator();

    f(e, args);

    assert getAllowTxDelay() == delayBefore && getAllowTxWindow() == windowBefore
        && getConsensusDomainSeparator() == domainBefore,
        "the guard's configuration immutables never change";
}

// ------------------------------------------------------------------------------------------------------
// F-20 / M-17: re-submitting an already-known rollover is a no-op (no revert, no state change), so reorg
// replays and racing submitters are harmless. The `!lastReverted` half holds modulo the FROST/curve
// summaries: it needs `frostAccepts`, and non-zero-ness of the already-known key is derived from INV-4
// (via `requireInvariant`) rather than assumed by a bare `require` (R-05).
// ------------------------------------------------------------------------------------------------------

rule updateEpochIdempotent(
    env e,
    Secp256k1.Point parentKey,
    uint64 parentEpoch,
    uint64 proposedEpoch,
    uint64 rolloverBlock,
    Secp256k1.Point newGroupKey,
    FROST.Signature signature
) {
    require e.msg.value == 0;
    require frostAccepts;                               // the rollover proof verifies (F-06 model)
    require isKnownEpoch(parentKey, parentEpoch);
    require proposedEpoch > parentEpoch;
    require isKnownEpoch(newGroupKey, proposedEpoch);    // the child is already recorded
    // ... so `newGroupKey` is non-zero: INV-4 says the zero point is never trusted at any epoch.
    requireInvariant zeroKeyNeverTrusted(proposedEpoch);

    storage before = lastStorage;
    updateEpoch@withrevert(e, parentKey, parentEpoch, proposedEpoch, rolloverBlock, newGroupKey, signature);

    assert !lastReverted, "re-submitting an already-known pair does not revert";
    assert lastStorage == before, "re-submitting an already-known pair changes nothing";
}

// ------------------------------------------------------------------------------------------------------
// R-01: rollover liveness: the completeness counterpart to R-EPOCH-3. A rollover off an already-trusted
// parent, at a strictly-advancing epoch, with a valid (non-zero) new key and a verifying proof, MUST
// succeed: these are the only gates, modulo the on-curve check abstracted by A-2 (a non-zero but off-curve
// key reverts `NotOnCurve` on-chain but is admitted here).
// ------------------------------------------------------------------------------------------------------

rule updateEpochSucceedsFromKnownParent(
    env e,
    Secp256k1.Point parentKey,
    uint64 parentEpoch,
    uint64 proposedEpoch,
    uint64 rolloverBlock,
    Secp256k1.Point newGroupKey,
    FROST.Signature signature
) {
    require e.msg.value == 0;
    require frostAccepts;                                // the rollover proof verifies
    require newGroupKey.x != 0 || newGroupKey.y != 0;    // requireNonZeroModel rejects the zero point
    require isKnownEpoch(parentKey, parentEpoch);
    require proposedEpoch > parentEpoch;

    updateEpoch@withrevert(e, parentKey, parentEpoch, proposedEpoch, rolloverBlock, newGroupKey, signature);

    assert !lastReverted, "the two documented preconditions are the only gates on a rollover";
}

// ------------------------------------------------------------------------------------------------------
// `updateEpoch` is permissionless: its authorization is the FROST proof over the rollover message, not the
// caller. So both its revert outcome and its state effect are independent of `msg.sender` (and of the rest
// of the environment bar `msg.value`, which the non-payable check pins to zero). This complements
// `authorizationIgnoresMsgSender` (the checkTransaction counterpart) for the forest-mutating path.
// ------------------------------------------------------------------------------------------------------

rule updateEpochOutcomeIndependentOfSender(
    env eA,
    env eB,
    Secp256k1.Point parentKey,
    uint64 parentEpoch,
    uint64 proposedEpoch,
    uint64 rolloverBlock,
    Secp256k1.Point newGroupKey,
    FROST.Signature signature
) {
    require eA.msg.value == 0 && eB.msg.value == 0;

    storage init = lastStorage;

    updateEpoch@withrevert(eA, parentKey, parentEpoch, proposedEpoch, rolloverBlock, newGroupKey, signature);
    bool revertedA = lastReverted;
    storage afterA = lastStorage;

    updateEpoch@withrevert(eB, parentKey, parentEpoch, proposedEpoch, rolloverBlock, newGroupKey, signature) at init;

    assert revertedA == lastReverted, "updateEpoch's revert outcome does not depend on msg.sender";
    assert lastStorage == afterA, "updateEpoch's state effect does not depend on msg.sender";
}
