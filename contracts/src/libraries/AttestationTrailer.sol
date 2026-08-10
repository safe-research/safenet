// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROST} from "@/libraries/FROST.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";
import {SignatureExtension} from "@/libraries/SignatureExtension.sol";

/**
 * @title AttestationTrailer
 * @notice Recognises and decodes a Safenet attestation carried as a {SignatureExtension} on Safe's
 *         `signatures` bytes.
 * @dev The attestation is a typed signature extension: its payload is the fixed 224-byte
 *      `abi.encode(uint64 epoch, address oracle, Secp256k1.Point groupKey, FROST.Signature signature)`,
 *      tagged by the terminal `TYPE_HASH`. The `oracle` is the oracle contract the attestation was gated
 *      by; the guard reconstructs the consensus `OracleTransactionProposal` message from it. The envelope
 *      framing (length word + type hash, tail-anchored so Safe's front-to-back signature parser is
 *      untouched) is owned by {SignatureExtension}; this library owns only the guard-specific type hash
 *      and payload schema.
 */
library AttestationTrailer {
    // ============================================================
    // CONSTANTS
    // ============================================================

    /**
     * @notice Type hash identifying an attestation trailer; the terminal word of the signature extension.
     */
    bytes32 internal constant TYPE_HASH = keccak256("SafenetGuard.AttestationTrailer.v1");

    /**
     * @dev Payload is `abi.encode(uint64, address, Secp256k1.Point, FROST.Signature)` = 7 words = 224 bytes.
     */
    uint256 private constant _PAYLOAD_LENGTH = 224;

    // ============================================================
    // ERRORS
    // ============================================================

    /**
     * @notice Thrown when a recognised trailer's payload is not the expected 224-byte attestation.
     */
    error MalformedAttestationTrailer();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Returns whether `signatures` carries an attestation trailer (ends with `TYPE_HASH`).
     * @dev Non-reverting; the consumer falls through to other authorisation paths when absent.
     * @param signatures The Safe `signatures` blob, optionally suffixed with a trailer.
     * @return present True if the blob ends with `TYPE_HASH`.
     */
    function hasTrailer(bytes calldata signatures) internal pure returns (bool present) {
        return SignatureExtension.has(signatures, TYPE_HASH);
    }

    /**
     * @notice Decodes the attestation from a trailer-bearing `signatures` blob.
     * @dev Self-verifying via {SignatureExtension.payload}: reverts `MalformedSignatureExtension` if the
     *      envelope itself is malformed, and `MalformedAttestationTrailer` if the payload is not the fixed
     *      224-byte attestation — a recognised trailer never yields partial or wrongly-sized data.
     * @param signatures The Safe `signatures` blob ending in an attestation trailer.
     * @return epoch The attested epoch.
     * @return oracle The oracle contract the attestation was gated by.
     * @return groupKey The attesting FROST group key.
     * @return signature The FROST signature.
     */
    function decode(bytes calldata signatures)
        internal
        pure
        returns (uint64 epoch, address oracle, Secp256k1.Point memory groupKey, FROST.Signature memory signature)
    {
        bytes calldata payloadData = SignatureExtension.payload(signatures, TYPE_HASH);
        require(payloadData.length == _PAYLOAD_LENGTH, MalformedAttestationTrailer());
        (epoch, oracle, groupKey, signature) =
            abi.decode(payloadData, (uint64, address, Secp256k1.Point, FROST.Signature));
    }
}
