// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {AttestationTrailer} from "@/libraries/AttestationTrailer.sol";
import {SignatureExtension} from "@/libraries/SignatureExtension.sol";
import {FROST} from "@/libraries/FROST.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

/**
 * @title AttestationTrailerTest
 * @notice Unit tests for the typed attestation codec layered on `SignatureExtension`: recognition,
 *         round-trip decode of the fixed 256-byte payload (including the `oracleDataHash`), and the two
 *         reverts — a well-formed envelope whose payload is the wrong size (`MalformedAttestationTrailer`)
 *         and a malformed envelope (`MalformedSignatureExtension`, surfaced from the transport layer).
 *         Both functions take `calldata`, so they run through external `this.call*` wrappers.
 */
contract AttestationTrailerTest is Test {
    uint64 internal constant EPOCH = 7;
    address internal constant ORACLE = address(0x0AC1E);
    bytes32 internal constant ORACLE_DATA_HASH = keccak256("oracle-data");

    function callHasTrailer(bytes calldata signatures) external pure returns (bool) {
        return AttestationTrailer.hasTrailer(signatures);
    }

    function callDecode(bytes calldata signatures)
        external
        pure
        returns (
            uint64 epoch,
            address oracle,
            bytes32 oracleDataHash,
            Secp256k1.Point memory groupKey,
            FROST.Signature memory signature
        )
    {
        return AttestationTrailer.decode(signatures);
    }

    function _attestationPayload() internal pure returns (bytes memory) {
        // abi.encode(uint64, address, bytes32, Secp256k1.Point, FROST.Signature) = 256 bytes.
        return abi.encode(
            EPOCH,
            ORACLE,
            ORACLE_DATA_HASH,
            Secp256k1.Point({x: 111, y: 222}),
            FROST.Signature({r: Secp256k1.Point({x: 333, y: 444}), z: 555})
        );
    }

    /// @dev Well-formed trailer: [ownerSigs][payload][uint256 payload.length][TYPE_HASH].
    function _trailer(bytes memory ownerSigs, bytes memory payload) internal pure returns (bytes memory) {
        return bytes.concat(ownerSigs, payload, abi.encode(payload.length), AttestationTrailer.TYPE_HASH);
    }

    function test_hasTrailer_trueForWellFormed() public view {
        assertTrue(this.callHasTrailer(_trailer(hex"aabbcc", _attestationPayload())));
    }

    function test_hasTrailer_falseWhenAbsent() public view {
        assertFalse(this.callHasTrailer(bytes.concat(hex"aabbcc", bytes32(uint256(256)))));
    }

    function test_decode_roundTrips() public view {
        (uint64 epoch, address oracle, bytes32 oracleDataHash, Secp256k1.Point memory k, FROST.Signature memory s) =
            this.callDecode(_trailer(hex"aabbcc", _attestationPayload()));
        assertEq(epoch, EPOCH);
        assertEq(oracle, ORACLE);
        assertEq(oracleDataHash, ORACLE_DATA_HASH);
        assertEq(k.x, 111);
        assertEq(k.y, 222);
        assertEq(s.r.x, 333);
        assertEq(s.r.y, 444);
        assertEq(s.z, 555);
    }

    function test_decode_revertsOnWrongPayloadSize() public {
        // A well-formed envelope whose payload is not the fixed 256-byte attestation.
        vm.expectRevert(AttestationTrailer.MalformedAttestationTrailer.selector);
        this.callDecode(_trailer(hex"aabb", hex"1122334455667788"));
    }

    function test_decode_revertsOnMalformedEnvelope() public {
        // Ends in TYPE_HASH but has no valid [payloadLength][typeHash] envelope behind it.
        vm.expectRevert(SignatureExtension.MalformedSignatureExtension.selector);
        this.callDecode(bytes.concat(AttestationTrailer.TYPE_HASH));
    }
}
