// checkTransaction authorization rules: the heart of the guard. `checkTransaction` permits execution
// via exactly one of three paths, in order: (1) an auto-allowed escape-hatch self-call; (2) an inline
// attestation trailer for a trusted (key, epoch); (3) a matured announcement of these exact parameters.
// Otherwise it reverts.
//
// Scope: FROST's elliptic-curve arithmetic is summarised, but its *verdict* is symbolic (a free ghost;
// see `frostVerifyModel` in SafenetGuardCommon.spec), so the guard's control-flow response to a failed
// verification is proven here (`failedAttestationNeverConsumesAnnouncement`, below). *Which* signatures
// verify remains an oracle, delegated to Foundry. The message-binding property (R-CHK-4) lives in
// SafenetGuardBinding.spec.
//
// Imports the shared methods/summaries/invariants/definition from SafenetGuardCommon.spec.

import "SafenetGuardCommon.spec";

// ------------------------------------------------------------------------------------------------------
// R-CHK-6: the post-execution hook is a genuine no-op: it never reverts and changes no state.
// ------------------------------------------------------------------------------------------------------

rule checkAfterExecutionNoOp(env e, bytes32 hash, bool success, address safe, bytes32 announcementHash) {
    require e.msg.value == 0; // non-payable: the Safe never forwards value to the guard hooks

    uint256 activeFromBefore = announcementActiveFrom(safe, announcementHash);

    checkAfterExecution@withrevert(e, hash, success);

    assert !lastReverted, "checkAfterExecution never reverts";
    assert announcementActiveFrom(safe, announcementHash) == activeFromBefore,
        "checkAfterExecution changes no state";
}

// ------------------------------------------------------------------------------------------------------
// R-CHK-2: an auto-allowed self-call is always permitted, regardless of signatures/nonce. This also ties
// the harness `isAutoAllowed` predicate to the contract's real auto-allow branch.
// ------------------------------------------------------------------------------------------------------

rule autoAllowedNeverReverts(
    env e,
    address to,
    uint256 value,
    bytes data,
    Enum.Operation operation,
    uint256 safeTxGas,
    uint256 baseGas,
    uint256 gasPrice,
    address gasToken,
    address refundReceiver,
    bytes signatures,
    address msgSender
) {
    require e.msg.value == 0; // non-payable: the Safe never forwards value to the guard hook
    require isAutoAllowed(to, value, data, operation);

    checkTransaction@withrevert(
        e, to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver, signatures, msgSender
    );

    assert !lastReverted, "an auto-allowed escape-hatch self-call is always permitted";
}

// ------------------------------------------------------------------------------------------------------
// R-CHK-1: authorization completeness: with no auto-allow, no trailer, and no matured announcement,
// checkTransaction must revert.
// ------------------------------------------------------------------------------------------------------

rule checkTransactionRevertsWithoutAuthorization(
    env e,
    address to,
    uint256 value,
    bytes data,
    Enum.Operation operation,
    uint256 safeTxGas,
    uint256 baseGas,
    uint256 gasPrice,
    address gasToken,
    address refundReceiver,
    bytes signatures,
    address msgSender
) {
    require e.msg.value == 0; // non-payable (proven by guardRejectsNativeValue)
    require !isAutoAllowed(to, value, data, operation);
    require !hasTrailer(signatures);

    bytes32 announcementHash =
        announcementHashOf(to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver);
    require !matured(e, e.msg.sender, announcementHash);

    checkTransaction@withrevert(
        e, to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver, signatures, msgSender
    );

    assert lastReverted, "an unauthorised transaction must revert";
}

// ------------------------------------------------------------------------------------------------------
// R-CHK-3 (F-01 fix): taking the attestation path implies the decoded (key, epoch) was trusted ON ENTRY.
// The membership snapshot is read BEFORE the call, so a record-then-report implementation cannot satisfy
// it. Paired with checkTransactionNeverExtendsForest (below), the entry gate is airtight.
//
// LOAD-BEARING for the `isAutoAllowed` mirror pinning (see the M-14 note above): this rule ASSERTS
// pre-state membership rather than assuming it, which is exactly what closes the trailer-bearing drift
// region. Do not weaken it to `require isKnownEpoch(...)`, that would make the assertion vacuous and
// silently unpin the mirror on the attestation path.
// ------------------------------------------------------------------------------------------------------

rule attestationPathRequiresKnownEpoch(
    env e,
    address to,
    uint256 value,
    bytes data,
    Enum.Operation operation,
    uint256 safeTxGas,
    uint256 baseGas,
    uint256 gasPrice,
    address gasToken,
    address refundReceiver,
    bytes signatures,
    address msgSender
) {
    require !isAutoAllowed(to, value, data, operation);
    require hasTrailer(signatures);

    bool knownBefore = isKnownEpoch(trailerGroupKey(signatures), trailerEpoch(signatures)); // pre-state

    checkTransaction(
        e, to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver, signatures, msgSender
    );

    assert knownBefore, "attested execution implies the (key, epoch) was already trusted on entry";
}

// ------------------------------------------------------------------------------------------------------
// F-02 / M-2: checkTransaction never extends (nor prunes) the epoch forest. Frame condition R-CHK-3
// rests on.
// ------------------------------------------------------------------------------------------------------

rule checkTransactionNeverExtendsForest(
    env e,
    address to,
    uint256 value,
    bytes data,
    Enum.Operation operation,
    uint256 safeTxGas,
    uint256 baseGas,
    uint256 gasPrice,
    address gasToken,
    address refundReceiver,
    bytes signatures,
    address msgSender,
    Secp256k1.Point anyKey,
    uint64 anyEpoch
) {
    bool before = isKnownEpoch(anyKey, anyEpoch);

    checkTransaction(
        e, to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver, signatures, msgSender
    );

    assert isKnownEpoch(anyKey, anyEpoch) == before, "checkTransaction never touches the epoch forest";
}

// ------------------------------------------------------------------------------------------------------
// R-CHK-5: the attestation path takes precedence and does not consume a matching (matured) announcement.
// The `matured` precondition (F-15) certifies the interesting co-existence case is reached.
// ------------------------------------------------------------------------------------------------------

rule attestationDoesNotConsumeAnnouncement(
    env e,
    address to,
    uint256 value,
    bytes data,
    Enum.Operation operation,
    uint256 safeTxGas,
    uint256 baseGas,
    uint256 gasPrice,
    address gasToken,
    address refundReceiver,
    bytes signatures,
    address msgSender
) {
    require !isAutoAllowed(to, value, data, operation);
    require hasTrailer(signatures);

    bytes32 announcementHash =
        announcementHashOf(to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver);
    require matured(e, e.msg.sender, announcementHash); // a consumable announcement co-exists
    uint256 activeFromBefore = announcementActiveFrom(e.msg.sender, announcementHash);
    uint256 activeUntilBefore = announcementActiveUntil(e.msg.sender, announcementHash);

    checkTransaction(
        e, to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver, signatures, msgSender
    );

    assert announcementActiveFrom(e.msg.sender, announcementHash) == activeFromBefore
        && announcementActiveUntil(e.msg.sender, announcementHash) == activeUntilBefore,
        "the attestation path leaves a matching announcement untouched";
}

// ------------------------------------------------------------------------------------------------------
// R-ANN-4: single-use escape hatch: consuming a matured announcement (no auto-allow, no trailer) deletes
// it, so it cannot be replayed.
// ------------------------------------------------------------------------------------------------------

rule checkTransactionConsumesAnnouncement(
    env e,
    address to,
    uint256 value,
    bytes data,
    Enum.Operation operation,
    uint256 safeTxGas,
    uint256 baseGas,
    uint256 gasPrice,
    address gasToken,
    address refundReceiver,
    bytes signatures,
    address msgSender
) {
    require !isAutoAllowed(to, value, data, operation);
    require !hasTrailer(signatures);

    bytes32 announcementHash =
        announcementHashOf(to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver);
    require matured(e, e.msg.sender, announcementHash);

    checkTransaction(
        e, to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver, signatures, msgSender
    );

    assert announcementActiveFrom(e.msg.sender, announcementHash) == 0,
        "a consumed announcement is deleted (single-use)";
}

// ------------------------------------------------------------------------------------------------------
// F-02 / M-5: consuming an announcement is slot-local: checkTransaction clears only the caller's own
// (msg.sender, hash) slot, never a second live announcement of the same Safe.
// ------------------------------------------------------------------------------------------------------

rule consumeTouchesOnlyItsOwnSlot(
    env e,
    address to,
    uint256 value,
    bytes data,
    Enum.Operation operation,
    uint256 safeTxGas,
    uint256 baseGas,
    uint256 gasPrice,
    address gasToken,
    address refundReceiver,
    bytes signatures,
    address msgSender,
    address safe,
    bytes32 other
) {
    require !isAutoAllowed(to, value, data, operation);
    require !hasTrailer(signatures);

    bytes32 announcementHash =
        announcementHashOf(to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver);
    require safe != e.msg.sender || other != announcementHash;

    uint256 fromBefore = announcementActiveFrom(safe, other);
    uint256 untilBefore = announcementActiveUntil(safe, other);

    checkTransaction(
        e, to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver, signatures, msgSender
    );

    assert announcementActiveFrom(safe, other) == fromBefore
        && announcementActiveUntil(safe, other) == untilBefore,
        "consuming an announcement clears only the caller's own target slot";
}

// ------------------------------------------------------------------------------------------------------
// F-03 / M-6: escape-hatch liveness. A matured announcement (no auto-allow, no trailer) MUST authorise
// execution: the anti-censorship guarantee. Because `matured` uses the spec's inclusive-window predicate,
// this also catches a window off-by-one in the code (`>=`→`>` etc.): at the boundary the rule would find
// `lastReverted`.
// ------------------------------------------------------------------------------------------------------

rule maturedAnnouncementAlwaysAuthorizes(
    env e,
    address to,
    uint256 value,
    bytes data,
    Enum.Operation operation,
    uint256 safeTxGas,
    uint256 baseGas,
    uint256 gasPrice,
    address gasToken,
    address refundReceiver,
    bytes signatures,
    address msgSender
) {
    require e.msg.value == 0;
    require !isAutoAllowed(to, value, data, operation);
    require !hasTrailer(signatures);

    bytes32 announcementHash =
        announcementHashOf(to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver);
    require matured(e, e.msg.sender, announcementHash);

    checkTransaction@withrevert(
        e, to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver, signatures, msgSender
    );

    assert !lastReverted, "a matured announcement must authorise execution (escape-hatch liveness)";
}

// ------------------------------------------------------------------------------------------------------
// R-01 / R-CHK-11: attestation-path liveness: the primary authorization path. A trusted (key, epoch)
// whose FROST verification succeeds MUST authorise execution: no parameter shape can make it revert. This
// is the counterpart to the escape-hatch liveness rule on the primary path, and closes the last liveness
// asymmetry. Malformed trailers are pruned (covered by malformedTrailerFailsClosed).
// ------------------------------------------------------------------------------------------------------

rule trustedAttestationAlwaysAuthorizes(
    env e,
    address to,
    uint256 value,
    bytes data,
    Enum.Operation operation,
    uint256 safeTxGas,
    uint256 baseGas,
    uint256 gasPrice,
    address gasToken,
    address refundReceiver,
    bytes signatures,
    address msgSender
) {
    require e.msg.value == 0;
    require safeNonce > 0; // the Safe pre-increments, so nonce() - 1 cannot underflow
    require !isAutoAllowed(to, value, data, operation);
    require hasTrailer(signatures);
    require isKnownEpoch(trailerGroupKey(signatures), trailerEpoch(signatures)); // trusted pair
    require frostAccepts;                                                        // verification succeeds

    checkTransaction@withrevert(
        e, to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver, signatures, msgSender
    );

    assert !lastReverted, "a trusted, verifying attestation must authorise execution";
}

// ------------------------------------------------------------------------------------------------------
// F-04 / M-7: fail-closed on a recognised trailer. A recognised-but-malformed trailer, or a well-formed
// trailer for an untrusted key, must revert, never silently fall through to the escape hatch (and never
// consume a matching announcement).
// ------------------------------------------------------------------------------------------------------

rule malformedTrailerFailsClosed(
    env e,
    address to,
    uint256 value,
    bytes data,
    Enum.Operation operation,
    uint256 safeTxGas,
    uint256 baseGas,
    uint256 gasPrice,
    address gasToken,
    address refundReceiver,
    bytes signatures,
    address msgSender
) {
    require e.msg.value == 0;
    require !isAutoAllowed(to, value, data, operation);
    require hasTrailer(signatures);

    // The blob is recognised (hasTrailer) but malformed: the same decode the guard runs reverts on it.
    trailerEpoch@withrevert(signatures);
    require lastReverted;

    checkTransaction@withrevert(
        e, to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver, signatures, msgSender
    );

    assert lastReverted, "a recognised but malformed trailer must revert, never fall through";
}

rule untrustedTrailerNeverFallsThrough(
    env e,
    address to,
    uint256 value,
    bytes data,
    Enum.Operation operation,
    uint256 safeTxGas,
    uint256 baseGas,
    uint256 gasPrice,
    address gasToken,
    address refundReceiver,
    bytes signatures,
    address msgSender
) {
    require e.msg.value == 0;
    require !isAutoAllowed(to, value, data, operation);
    require hasTrailer(signatures);
    require !isKnownEpoch(trailerGroupKey(signatures), trailerEpoch(signatures)); // well-formed but untrusted

    bytes32 announcementHash =
        announcementHashOf(to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver);
    require matured(e, e.msg.sender, announcementHash); // a consumable announcement sits right there
    uint256 fromBefore = announcementActiveFrom(e.msg.sender, announcementHash);

    checkTransaction@withrevert(
        e, to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver, signatures, msgSender
    );

    assert lastReverted, "an untrusted trailer reverts rather than downgrading to the escape hatch";
    assert announcementActiveFrom(e.msg.sender, announcementHash) == fromBefore, "and does not consume it";
}

// ------------------------------------------------------------------------------------------------------
// F-06 / M-9 (R-CHK-10): the guard honours the FROST verdict. A trusted-key attestation whose verification
// FAILS must revert, never fall through to the escape hatch and consume a matured announcement. This is
// the control-flow half of the crypto guarantee, made provable by the reverting `frostVerifyModel`.
// ------------------------------------------------------------------------------------------------------

rule failedAttestationNeverConsumesAnnouncement(
    env e,
    address to,
    uint256 value,
    bytes data,
    Enum.Operation operation,
    uint256 safeTxGas,
    uint256 baseGas,
    uint256 gasPrice,
    address gasToken,
    address refundReceiver,
    bytes signatures,
    address msgSender
) {
    require e.msg.value == 0;
    require safeNonce > 0; // reach the verifier past the nonce-1 underflow, so we test the verdict path
    require !isAutoAllowed(to, value, data, operation);
    require hasTrailer(signatures);
    require isKnownEpoch(trailerGroupKey(signatures), trailerEpoch(signatures)); // trusted key
    require !frostAccepts;                                                        // ... but verification fails

    bytes32 announcementHash =
        announcementHashOf(to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver);
    require matured(e, e.msg.sender, announcementHash); // a consumable announcement sits right there
    uint256 fromBefore = announcementActiveFrom(e.msg.sender, announcementHash);

    checkTransaction@withrevert(
        e, to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver, signatures, msgSender
    );

    assert lastReverted, "an invalid attestation reverts, it does not fall through to the escape hatch";
    assert announcementActiveFrom(e.msg.sender, announcementHash) == fromBefore, "and does not consume it";
}

// ------------------------------------------------------------------------------------------------------
// F-02 / M-16: the auto-allow path has no state effects: it returns before touching either mapping. The
// only checkTransaction path with no effect rule of its own.
// ------------------------------------------------------------------------------------------------------

rule autoAllowedChangesNoState(
    env e,
    address to,
    uint256 value,
    bytes data,
    Enum.Operation operation,
    uint256 safeTxGas,
    uint256 baseGas,
    uint256 gasPrice,
    address gasToken,
    address refundReceiver,
    bytes signatures,
    address msgSender,
    address safe,
    bytes32 announcementHash,
    Secp256k1.Point anyKey,
    uint64 anyEpoch
) {
    require e.msg.value == 0;
    require isAutoAllowed(to, value, data, operation);

    uint256 fromBefore = announcementActiveFrom(safe, announcementHash);
    bool epochBefore = isKnownEpoch(anyKey, anyEpoch);

    checkTransaction(
        e, to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver, signatures, msgSender
    );

    assert announcementActiveFrom(safe, announcementHash) == fromBefore
        && isKnownEpoch(anyKey, anyEpoch) == epochBefore,
        "the auto-allow path changes no announcement or epoch state";
}

// ------------------------------------------------------------------------------------------------------
// F-28 / M-19: the guard has no payable entry point: a native-value call to any state-changing method
// reverts. Turns the `msg.value == 0` assumptions on the liveness rules into a proven consequence.
// ------------------------------------------------------------------------------------------------------

rule guardRejectsNativeValue(env e, method f, calldataarg args) filtered { f -> !f.isView && !f.isPure } {
    require e.msg.value != 0;

    f@withrevert(e, args);

    assert lastReverted, "the guard has no payable entry point";
}

// ------------------------------------------------------------------------------------------------------
// Note (M-14): `SafenetGuard._isAutoAllowed` is `private`, so the harness `isAutoAllowed` mirror cannot be
// pinned by a direct call. It is instead pinned behaviourally by FOUR load-bearing rules covering both drift
// directions across the whole input space:
//   - `autoAllowedNeverReverts`: not too permissive (a mirror-true, no-trailer/no-announcement call always
//     succeeds, which by R-CHK-1 is only possible if the contract auto-allowed it);
//   - `checkTransactionRevertsWithoutAuthorization`: not too restrictive in the no-trailer region (a
//     mirror-false, no-authorization call reverts);
//   - `attestationPathRequiresKnownEpoch`: closes the well-formed-trailer region, asserting pre-state key
//     membership, so auto-allowing such a call would fail that assertion;
//   - `malformedTrailerFailsClosed`: closes the malformed-trailer sliver the previous rule prunes (its
//     non-`@withrevert` decode discards those paths), asserting such a call reverts.
// Together these sandwich the mirror onto the contract's real decision.
// ------------------------------------------------------------------------------------------------------
