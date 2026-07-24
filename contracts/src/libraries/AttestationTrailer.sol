// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROST} from "@/libraries/FROST.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

/**
 * @title AttestationTrailer
 * @notice Recognises and decodes a Safenet attestation appended to Safe's `signatures` bytes.
 * @dev Layout: `[safe owner signatures][192-byte abi.encode(epoch, groupKey, signature)][32-byte TYPE_HASH]`.
 *      Anchoring at the end leaves Safe's front-to-back signature parser untouched, and the terminal type
 *      hash makes detection independent of signature suffixes — a blob not ending in `TYPE_HASH` is simply
 *      "no trailer". The type hash embeds the version, so a future format uses a different type hash (and
 *      this guard treats it as absent). The library owns recognition, sizing, and the typed decode.
 */
library AttestationTrailer {
    // ============================================================
    // CONSTANTS
    // ============================================================

    /**
     * @notice Type hash that must terminate a v1 attestation trailer; doubles as the version tag.
     */
    bytes32 internal constant TYPE_HASH = keccak256("SafenetGuard.AttestationTrailer.v1");

    /**
     * @dev Payload is `abi.encode(uint64, Secp256k1.Point, FROST.Signature)` = 6 words = 192 bytes.
     */
    uint256 private constant _PAYLOAD_LENGTH = 192;

    /**
     * @dev Total trailer overhead: the payload followed by the 32-byte type hash word. Derived from
     *      `_PAYLOAD_LENGTH` so the two constants cannot drift if the payload layout changes.
     */
    uint256 private constant _TRAILER_LENGTH = _PAYLOAD_LENGTH + 32;

    // ============================================================
    // ERRORS
    // ============================================================

    /**
     * @notice Thrown when the type hash is present but the blob is too short to hold a v1 trailer.
     */
    error MalformedAttestationTrailer();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Recognises and decodes an attestation trailer from a `signatures` blob.
     * @dev Returns `present == false` when the last word is not `TYPE_HASH` (so the consumer falls through
     *      to other authorisation paths). Reverts `MalformedAttestationTrailer` when the type hash is
     *      present but the blob is shorter than a full trailer — a recognised trailer never silently
     *      falls through.
     * @return present True if a trailer was recognised and decoded.
     * @return epoch The attested epoch.
     * @return groupKey The attesting FROST group key.
     * @return signature The FROST signature.
     */
    function decode(bytes calldata signatures)
        internal
        pure
        returns (bool present, uint64 epoch, Secp256k1.Point memory groupKey, FROST.Signature memory signature)
    {
        if (signatures.length < 32 || bytes32(signatures[signatures.length - 32:]) != TYPE_HASH) {
            return (false, 0, groupKey, signature);
        }
        require(signatures.length >= _TRAILER_LENGTH, MalformedAttestationTrailer());

        uint256 payloadStart = signatures.length - _TRAILER_LENGTH;
        (epoch, groupKey, signature) = abi.decode(
            signatures[payloadStart:payloadStart + _PAYLOAD_LENGTH], (uint64, Secp256k1.Point, FROST.Signature)
        );
        present = true;
    }
}
