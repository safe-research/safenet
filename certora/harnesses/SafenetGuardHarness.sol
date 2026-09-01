// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {SafenetGuard} from "@/guard/SafenetGuard.sol";
import {AttestationTrailer} from "@/libraries/AttestationTrailer.sol";
import {FROST} from "@/libraries/FROST.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";
import {TransactionAnnouncement} from "@/libraries/TransactionAnnouncement.sol";
import {Enum} from "@safe/interfaces/Enum.sol";

/// @dev Certora harness for {SafenetGuard}: forwards the constructor and exposes accessors/predicates the
///      specs need but the contract keeps private.
contract SafenetGuardHarness is SafenetGuard {
    // Genesis `(key, epoch)` captured from the constructor args (the base contract records it but exposes
    // no getter), so a spec can assert the seeded pair stays trusted: the base case for trust-to-genesis.
    uint64 private immutable _genesisEpoch;
    uint256 private immutable _genesisKeyX;
    uint256 private immutable _genesisKeyY;

    constructor(
        uint256 consensusChainId,
        address consensusAddress,
        uint64 initialEpoch,
        Secp256k1.Point memory initialGroupKey,
        uint256 allowTransactionDelay,
        uint256 allowTransactionWindow
    )
        SafenetGuard(
            consensusChainId,
            consensusAddress,
            initialEpoch,
            initialGroupKey,
            allowTransactionDelay,
            allowTransactionWindow
        )
    {
        _genesisEpoch = initialEpoch;
        _genesisKeyX = initialGroupKey.x;
        _genesisKeyY = initialGroupKey.y;
    }

    function genesisEpoch() external view returns (uint64) {
        return _genesisEpoch;
    }

    function genesisKeyX() external view returns (uint256) {
        return _genesisKeyX;
    }

    function genesisKeyY() external view returns (uint256) {
        return _genesisKeyY;
    }

    // ----------------------------------------------------------------------------------------------
    // Storage accessors: expose the packed announcement window fields individually, so specs can
    // reason about `activeFrom` / `activeUntil` without CVL tuple destructuring.
    // ----------------------------------------------------------------------------------------------

    function announcementActiveFrom(address safe, bytes32 announcementHash) external view returns (uint256 activeFrom) {
        (activeFrom,) = this.getAnnouncementWindow(safe, announcementHash);
    }

    function announcementActiveUntil(address safe, bytes32 announcementHash)
        external
        view
        returns (uint256 activeUntil)
    {
        (, activeUntil) = this.getAnnouncementWindow(safe, announcementHash);
    }

    // ----------------------------------------------------------------------------------------------
    // checkTransaction decision helpers.
    // ----------------------------------------------------------------------------------------------

    /// @dev Mirror of `SafenetGuard._isAutoAllowed`, re-expressed so specs can call it `envfree` (the real
    ///      gate is `private`, so it cannot be reached from the harness). It is pinned to the real gate
    ///      behaviourally, not by a direct call, by three load-bearing rules that between them cover both
    ///      drift directions across the whole input space:
    ///        - `autoAllowedNeverReverts`: not too permissive (a mirror-true, no-trailer/no-announcement
    ///          call always succeeds, which is only possible if the contract actually auto-allowed it);
    ///        - `checkTransactionRevertsWithoutAuthorization`: not too restrictive in the no-trailer region
    ///          (a mirror-false, no-authorization call reverts);
    ///        - `attestationPathRequiresKnownEpoch`: closes the trailer-bearing region that R-CHK-1
    ///          excludes: it *asserts* rather than assumes pre-state key membership, so a contract that
    ///          auto-allowed a trailer-bearing call would return with an untrusted key and fail that assert.
    ///      So the mirror cannot silently drift from the contract's decision.
    function isAutoAllowed(address to, uint256 value, bytes calldata data, Enum.Operation operation)
        external
        view
        returns (bool)
    {
        if (to != address(this) || value != 0 || operation != Enum.Operation.Call || data.length < 4) {
            return false;
        }
        // forge-lint: disable-next-line(unsafe-typecast)
        bytes4 selector = bytes4(data);
        return selector == SafenetGuard.announceTransaction.selector
            || selector == SafenetGuard.cancelAnnouncement.selector;
    }

    function hasTrailer(bytes calldata signatures) external pure returns (bool) {
        return AttestationTrailer.hasTrailer(signatures);
    }

    /// @dev The announcement hash `checkTransaction` computes from the same Safe-transaction parameters
    ///      (nonce excluded), so a spec can locate the announcement slot a call would consume.
    function announcementHashOf(
        address to,
        uint256 value,
        bytes calldata data,
        Enum.Operation operation,
        uint256 safeTxGas,
        uint256 baseGas,
        uint256 gasPrice,
        address gasToken,
        address refundReceiver
    ) external pure returns (bytes32) {
        return TransactionAnnouncement.hash(
            TransactionAnnouncement.AnnouncedTransaction({
                to: to,
                value: value,
                data: data,
                operation: operation,
                safeTxGas: safeTxGas,
                baseGas: baseGas,
                gasPrice: gasPrice,
                gasToken: gasToken,
                refundReceiver: refundReceiver
            })
        );
    }

    // Trailer field decoders: each returns one field of `AttestationTrailer.decode` and reverts on a
    // malformed trailer, exactly as `checkTransaction` does.
    function trailerEpoch(bytes calldata signatures) external pure returns (uint64 epoch) {
        (epoch,,,,) = AttestationTrailer.decode(signatures);
    }

    function trailerOracle(bytes calldata signatures) external pure returns (address oracle) {
        (, oracle,,,) = AttestationTrailer.decode(signatures);
    }

    function trailerOracleDataHash(bytes calldata signatures) external pure returns (bytes32 oracleDataHash) {
        (,, oracleDataHash,,) = AttestationTrailer.decode(signatures);
    }

    function trailerGroupKey(bytes calldata signatures) external pure returns (Secp256k1.Point memory groupKey) {
        (,,, groupKey,) = AttestationTrailer.decode(signatures);
    }

    function trailerSignature(bytes calldata signatures) external pure returns (FROST.Signature memory signature) {
        (,,,, signature) = AttestationTrailer.decode(signatures);
    }

    /// @dev Forest membership by raw coordinates, so a spec can query the zero point `(0, 0)` without
    ///      building a `Secp256k1.Point` struct literal in an invariant parameter (INV-4, F-08).
    function isKnownEpochRaw(uint256 x, uint256 y, uint64 epoch) external view returns (bool) {
        return this.isKnownEpoch(Secp256k1.Point({x: x, y: y}), epoch);
    }
}
