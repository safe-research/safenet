// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

/**
 * @title SignatureExtension
 * @notice A reusable format for appending typed, variable-length data after a Safe's owner signatures.
 * @dev A *signature extension* is a self-describing envelope appended to Safe's `signatures` bytes.
 *      Safe's own signature parser reads owner signatures front-to-back and ignores trailing bytes, so a
 *      consumer (e.g. a transaction guard) can carry extra authorisation data in the tail and recover it
 *      here without disturbing Safe's parse.
 *
 *      Envelope layout:
 *
 *          [safe owner signatures] [payload: N bytes] [uint256 payloadLength = N] [bytes32 typeHash]
 *
 *      - The **type hash** is the terminal 32-byte word. It both identifies the extension type and pins
 *        its version, by the registry convention `typeHash = keccak256("<Owner>.<Name>.v<N>")` (e.g.
 *        `SafenetGuard.AttestationTrailer.v1`). Anchoring the discriminator at the end makes detection
 *        independent of any owner-signature suffix, and a consumer of a different type simply does not
 *        recognise it. A future, post-release format change bumps to a new type hash, which older
 *        consumers read as "no extension".
 *      - The **length word** precedes the type hash and gives the payload size, so the payload may be any
 *        length and is sliceable from the tail in O(1). Total envelope overhead is a fixed 64 bytes.
 *      - The owner-signature/payload boundary is not exposed: the guard does not need it (Safe parses the
 *        front independently), and a consumer that does can derive it as
 *        `signatures.length - payloadLength - {ENVELOPE_OVERHEAD}`.
 *
 *      This library is the transport only — payload-agnostic. A typed consumer holds its own `typeHash`
 *      and decodes the returned payload bytes into its own schema. At most one extension per blob;
 *      nesting is not supported (a future need would be a new format/version).
 */
library SignatureExtension {
    // ============================================================
    // CONSTANTS
    // ============================================================

    /**
     * @dev Fixed envelope overhead: the `uint256 payloadLength` word (32 bytes) plus the terminal
     *      `bytes32 typeHash` word (32 bytes).
     */
    uint256 private constant _ENVELOPE_OVERHEAD = 64;

    // ============================================================
    // ERRORS
    // ============================================================

    /**
     * @notice Thrown when a blob carries the expected type hash but is not a well-formed envelope — too
     *         short to hold `[payloadLength][typeHash]`, or a payload length that runs past the front.
     */
    error MalformedSignatureExtension();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Returns whether `signatures` carries an extension of type `typeHash` (ends with it).
     * @dev Non-reverting recognition: a blob shorter than a word, or not ending in `typeHash`, is simply
     *      "no extension of this type", so the consumer can fall through to other authorisation paths.
     *      This is detection only; {payload} performs the full well-formedness validation.
     * @param signatures The Safe `signatures` blob, optionally suffixed with an extension.
     * @param typeHash The extension type to look for.
     * @return present True if the blob ends with `typeHash`.
     */
    function has(bytes calldata signatures, bytes32 typeHash) internal pure returns (bool present) {
        return signatures.length >= 32 && bytes32(signatures[signatures.length - 32:]) == typeHash;
    }

    /**
     * @notice Extracts the payload of a `typeHash` extension from `signatures`.
     * @dev Self-verifying, so it is safe to call standalone without a preceding {has}: reverts
     *      `MalformedSignatureExtension` if the blob is too short to hold `[payloadLength][typeHash]`, if
     *      the terminal word is not `typeHash`, or if the encoded payload length runs past the front of
     *      the blob. The returned slice is a `calldata` view (no copy).
     * @param signatures The Safe `signatures` blob ending in a `typeHash` extension.
     * @param typeHash The expected extension type; the terminal word must equal it.
     * @return payloadData The extension payload (`payloadLength` bytes preceding the length word).
     */
    function payload(bytes calldata signatures, bytes32 typeHash) internal pure returns (bytes calldata payloadData) {
        require(
            signatures.length >= _ENVELOPE_OVERHEAD && bytes32(signatures[signatures.length - 32:]) == typeHash,
            MalformedSignatureExtension()
        );
        uint256 end = signatures.length - _ENVELOPE_OVERHEAD; // index just past the payload
        uint256 payloadLength = uint256(bytes32(signatures[end:end + 32]));
        require(payloadLength <= end, MalformedSignatureExtension());
        return signatures[end - payloadLength:end];
    }
}
