// Announcement (escape-hatch) lifecycle rules: R-ANN-1..3, the write-provenance / slot-locality frame
// conditions (F-02), and the announcement-hash field-separation rules (F-05). The single-use consume
// property (R-ANN-4) and consume slot-locality live in the checkTransaction spec, since consumption
// happens in the guard's pre-execution hook.
//
// Imports the shared methods/summaries/invariants from SafenetGuardCommon.spec.

import "SafenetGuardCommon.spec";

// ------------------------------------------------------------------------------------------------------
// R-ANN-1: caller isolation. An announcement is keyed by the caller, so a call from one Safe can never
// create, overwrite, or clear another Safe's announcement.
// ------------------------------------------------------------------------------------------------------

rule announcementsCallerIsolation(env e, method f, calldataarg args, address safe, bytes32 announcementHash)
    filtered { f -> !f.isView && !f.isPure }
{
    require safe != e.msg.sender;

    uint256 activeFromBefore = announcementActiveFrom(safe, announcementHash);
    uint256 activeUntilBefore = announcementActiveUntil(safe, announcementHash);

    f(e, args);

    assert announcementActiveFrom(safe, announcementHash) == activeFromBefore
        && announcementActiveUntil(safe, announcementHash) == activeUntilBefore,
        "a call must not change another Safe's announcement";
}

// ------------------------------------------------------------------------------------------------------
// F-02 / M-4: write provenance for the announcement mapping: only the Safe's own announceTransaction
// creates an entry, and only cancelAnnouncement or checkTransaction (consume) clears one.
// ------------------------------------------------------------------------------------------------------

rule onlyAnnounceCreatesEntry(env e, method f, calldataarg args, address safe, bytes32 announcementHash)
    filtered { f -> !f.isView && !f.isPure }
{
    require announcementActiveFrom(safe, announcementHash) == 0;

    f(e, args);

    assert announcementActiveFrom(safe, announcementHash) != 0 =>
        (f.selector == sig:announceTransaction(TransactionAnnouncement.AnnouncedTransaction).selector
            && safe == e.msg.sender),
        "only the Safe's own announceTransaction creates an entry";
}

rule onlyCancelOrConsumeClearsEntry(env e, method f, calldataarg args, address safe, bytes32 announcementHash)
    filtered { f -> !f.isView && !f.isPure }
{
    require announcementActiveFrom(safe, announcementHash) != 0;

    f(e, args);

    assert announcementActiveFrom(safe, announcementHash) == 0 =>
        (f.selector == sig:cancelAnnouncement(bytes32).selector
            || f.selector == sig:checkTransaction(address, uint256, bytes, Enum.Operation, uint256, uint256,
                uint256, address, address, bytes, address).selector),
        "only cancelAnnouncement or checkTransaction (consume) clears an entry";
}

// ------------------------------------------------------------------------------------------------------
// F-02 / M-5: writes are slot-local. announce/cancel touch only the caller's own (msg.sender, hash) slot,
// so they cannot disturb a second live announcement; the "relayer holding two hatches" case R-ANN-1 does
// not reach (it excludes the same Safe).
// ------------------------------------------------------------------------------------------------------

rule announceTouchesOnlyItsOwnSlot(env e, TransactionAnnouncement.AnnouncedTransaction announcement,
    address safe, bytes32 other)
{
    bytes32 h = getAnnouncementHash(announcement);
    require safe != e.msg.sender || other != h;

    uint256 fromBefore = announcementActiveFrom(safe, other);
    uint256 untilBefore = announcementActiveUntil(safe, other);

    announceTransaction(e, announcement);

    assert announcementActiveFrom(safe, other) == fromBefore
        && announcementActiveUntil(safe, other) == untilBefore,
        "announce writes only its own (caller, hash) slot";
}

rule cancelTouchesOnlyItsOwnSlot(env e, bytes32 announcementHash, address safe, bytes32 other) {
    require safe != e.msg.sender || other != announcementHash;

    uint256 fromBefore = announcementActiveFrom(safe, other);
    uint256 untilBefore = announcementActiveUntil(safe, other);

    cancelAnnouncement(e, announcementHash);

    assert announcementActiveFrom(safe, other) == fromBefore
        && announcementActiveUntil(safe, other) == untilBefore,
        "cancel clears only its own (caller, hash) slot";
}

// ------------------------------------------------------------------------------------------------------
// F-02 / M-5b: announcement windows are frozen outside the announcement API. Only announceTransaction,
// cancelAnnouncement, and checkTransaction (consume) touch `$announcements`; every other method leaves
// every `(safe, hash)` window byte-identical. Closes the width-preserving-shift gap the write-provenance
// rules (0<->nonzero only) and caller-isolation (safe != msg.sender only) leave open.
// ------------------------------------------------------------------------------------------------------

rule announcementWindowsFrozenOutsideApi(env e, method f, calldataarg args, address safe, bytes32 announcementHash)
    filtered {
        f -> !f.isView && !f.isPure
            && f.selector != sig:announceTransaction(TransactionAnnouncement.AnnouncedTransaction).selector
            && f.selector != sig:cancelAnnouncement(bytes32).selector
            && f.selector != sig:checkTransaction(address, uint256, bytes, Enum.Operation, uint256, uint256,
                uint256, address, address, bytes, address).selector
    }
{
    uint256 fromBefore = announcementActiveFrom(safe, announcementHash);
    uint256 untilBefore = announcementActiveUntil(safe, announcementHash);

    f(e, args);

    assert announcementActiveFrom(safe, announcementHash) == fromBefore
        && announcementActiveUntil(safe, announcementHash) == untilBefore,
        "only the announcement API may change any announcement window";
}

// ------------------------------------------------------------------------------------------------------
// R-ANN-2: announceTransaction records the exact window `[now + delay, now + delay + window]`, and
// reverts while a pending or still-active announcement of the same parameters already exists.
// ------------------------------------------------------------------------------------------------------

rule announceCreatesWindow(env e, TransactionAnnouncement.AnnouncedTransaction announcement) {
    bytes32 announcementHash = getAnnouncementHash(announcement);

    announceTransaction(e, announcement);

    assert announcementActiveFrom(e.msg.sender, announcementHash) == e.block.timestamp + getAllowTxDelay(),
        "activeFrom must be now + delay";
    assert announcementActiveUntil(e.msg.sender, announcementHash)
        == announcementActiveFrom(e.msg.sender, announcementHash) + getAllowTxWindow(),
        "activeUntil must be activeFrom + window";
}

rule announceRevertsWhilePending(env e, TransactionAnnouncement.AnnouncedTransaction announcement) {
    require e.msg.value == 0; // non-payable
    bytes32 announcementHash = getAnnouncementHash(announcement);

    // A pending or still-active announcement exists for these exact parameters.
    require announcementActiveFrom(e.msg.sender, announcementHash) != 0
        && e.block.timestamp <= announcementActiveUntil(e.msg.sender, announcementHash);

    announceTransaction@withrevert(e, announcement);

    assert lastReverted, "announce must revert while a pending/active announcement exists";
}

// ------------------------------------------------------------------------------------------------------
// R-ANN-3: cancelAnnouncement removes the entry immediately (window reads back empty), and reverts when
// there is nothing to cancel.
// ------------------------------------------------------------------------------------------------------

rule cancelClearsWindow(env e, bytes32 announcementHash) {
    cancelAnnouncement(e, announcementHash);

    assert announcementActiveFrom(e.msg.sender, announcementHash) == 0
        && announcementActiveUntil(e.msg.sender, announcementHash) == 0,
        "a cancelled announcement has no window";
}

rule cancelRevertsIfAbsent(env e, bytes32 announcementHash) {
    require e.msg.value == 0; // non-payable
    require announcementActiveFrom(e.msg.sender, announcementHash) == 0;

    cancelAnnouncement@withrevert(e, announcementHash);

    assert lastReverted, "cancel must revert when no announcement exists";
}

// ------------------------------------------------------------------------------------------------------
// F-11 / M-11: the success direction of the announcement lifecycle (documented but previously pruned by
// non-@withrevert calls). announce succeeds when absent or expired (renewal in place); cancel always
// succeeds when an entry exists (unblockable revocation). Together with the revert-direction rules above
// this gives the full iff on both revert conditions.
// ------------------------------------------------------------------------------------------------------

rule announceSucceedsWhenAbsentOrExpired(env e, TransactionAnnouncement.AnnouncedTransaction announcement) {
    requireInvariant timingBoundsWithinUint64();      // delay/window non-zero and uint64-bounded
    require e.msg.value == 0;
    require e.block.timestamp <= max_uint64;          // realistic-timestamp assumption, stated explicitly

    bytes32 announcementHash = getAnnouncementHash(announcement);
    require announcementActiveFrom(e.msg.sender, announcementHash) == 0
        || e.block.timestamp > announcementActiveUntil(e.msg.sender, announcementHash); // absent or expired

    announceTransaction@withrevert(e, announcement);

    assert !lastReverted, "announce succeeds when absent or expired, renewal in place, no WindowOverflow";
}

rule cancelSucceedsWhenPresent(env e, bytes32 announcementHash) {
    require e.msg.value == 0;
    require announcementActiveFrom(e.msg.sender, announcementHash) != 0; // pending, active, or expired

    cancelAnnouncement@withrevert(e, announcementHash);

    assert !lastReverted, "cancel always succeeds when an entry exists: revocation is unblockable";
}

// ------------------------------------------------------------------------------------------------------
// F-05 / M-8: the announcement hash separates every field: no two transactions differing in a single
// field share a hash. The security-critical cases are `operation` (a CALL announcement must never
// authorise a DELEGATECALL) and `data` (an announcement must not authorise different calldata to the same
// target, the most severe field-drop). One rule per field. The `data` rule (R-02) forces a difference
// via differing lengths, which catches the dropped-`dataHash` class without needing `bytes` equality.
// ------------------------------------------------------------------------------------------------------

rule announcementHashSeparatesOperation(address to, uint256 value, bytes data, Enum.Operation opA,
    Enum.Operation opB, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken,
    address refundReceiver)
{
    require opA != opB;
    assert announcementHashOf(to, value, data, opA, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver)
        != announcementHashOf(to, value, data, opB, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver),
        "CALL and DELEGATECALL announcements are never interchangeable";
}

rule announcementHashSeparatesTo(address toA, address toB, uint256 value, bytes data,
    Enum.Operation operation, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken,
    address refundReceiver)
{
    require toA != toB;
    assert announcementHashOf(toA, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver)
        != announcementHashOf(toB, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver),
        "the announcement hash separates the target";
}

rule announcementHashSeparatesValue(address to, uint256 valueA, uint256 valueB, bytes data,
    Enum.Operation operation, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken,
    address refundReceiver)
{
    require valueA != valueB;
    assert announcementHashOf(to, valueA, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver)
        != announcementHashOf(to, valueB, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver),
        "the announcement hash separates the value";
}

rule announcementHashSeparatesSafeTxGas(address to, uint256 value, bytes data, Enum.Operation operation,
    uint256 gasA, uint256 gasB, uint256 baseGas, uint256 gasPrice, address gasToken, address refundReceiver)
{
    require gasA != gasB;
    assert announcementHashOf(to, value, data, operation, gasA, baseGas, gasPrice, gasToken, refundReceiver)
        != announcementHashOf(to, value, data, operation, gasB, baseGas, gasPrice, gasToken, refundReceiver),
        "the announcement hash separates safeTxGas";
}

rule announcementHashSeparatesBaseGas(address to, uint256 value, bytes data, Enum.Operation operation,
    uint256 safeTxGas, uint256 baseA, uint256 baseB, uint256 gasPrice, address gasToken, address refundReceiver)
{
    require baseA != baseB;
    assert announcementHashOf(to, value, data, operation, safeTxGas, baseA, gasPrice, gasToken, refundReceiver)
        != announcementHashOf(to, value, data, operation, safeTxGas, baseB, gasPrice, gasToken, refundReceiver),
        "the announcement hash separates baseGas";
}

rule announcementHashSeparatesGasPrice(address to, uint256 value, bytes data, Enum.Operation operation,
    uint256 safeTxGas, uint256 baseGas, uint256 priceA, uint256 priceB, address gasToken, address refundReceiver)
{
    require priceA != priceB;
    assert announcementHashOf(to, value, data, operation, safeTxGas, baseGas, priceA, gasToken, refundReceiver)
        != announcementHashOf(to, value, data, operation, safeTxGas, baseGas, priceB, gasToken, refundReceiver),
        "the announcement hash separates gasPrice";
}

rule announcementHashSeparatesGasToken(address to, uint256 value, bytes data, Enum.Operation operation,
    uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address tokenA, address tokenB, address refundReceiver)
{
    require tokenA != tokenB;
    assert announcementHashOf(to, value, data, operation, safeTxGas, baseGas, gasPrice, tokenA, refundReceiver)
        != announcementHashOf(to, value, data, operation, safeTxGas, baseGas, gasPrice, tokenB, refundReceiver),
        "the announcement hash separates gasToken";
}

rule announcementHashSeparatesRefundReceiver(address to, uint256 value, bytes data, Enum.Operation operation,
    uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken, address receiverA, address receiverB)
{
    require receiverA != receiverB;
    assert announcementHashOf(to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, receiverA)
        != announcementHashOf(to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, receiverB),
        "the announcement hash separates refundReceiver";
}

/// @dev R-02: the hash separates `data`. Differing lengths force `dataA != dataB` without comparing the
///      bytes, which is enough to catch a dropped `dataHash` in the encoding. CVL cannot compare `bytes`
///      content, so the same-length/different-content case (which catches a length-for-hash substitution)
///      is pinned in Foundry by `test_announcementHash_separatesSameLengthData` (S-02).
rule announcementHashSeparatesData(address to, uint256 value, bytes dataA, bytes dataB,
    Enum.Operation operation, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken,
    address refundReceiver)
{
    require dataA.length != dataB.length;
    assert announcementHashOf(to, value, dataA, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver)
        != announcementHashOf(to, value, dataB, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver),
        "the announcement hash separates the call data";
}
