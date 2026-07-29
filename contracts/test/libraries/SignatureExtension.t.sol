// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {SignatureExtension} from "@/libraries/SignatureExtension.sol";

/**
 * @title SignatureExtensionTest
 * @notice Unit tests for the generic `SignatureExtension` envelope: terminal-type-hash recognition and
 *         length-prefixed payload extraction across payload sizes, plus the malformed/overrun reverts.
 *         Both functions take `calldata`, so they are exercised through external `this.call*` wrappers
 *         (which also lets `vm.expectRevert` observe the internal revert at a lower call depth).
 */
contract SignatureExtensionTest is Test {
    bytes32 internal constant TYPE_HASH = keccak256("Test.Extension.v1");
    bytes32 internal constant OTHER_TYPE_HASH = keccak256("Test.Other.v1");

    function callHas(bytes calldata signatures, bytes32 typeHash) external pure returns (bool) {
        return SignatureExtension.has(signatures, typeHash);
    }

    function callPayload(bytes calldata signatures, bytes32 typeHash) external pure returns (bytes memory) {
        return SignatureExtension.payload(signatures, typeHash);
    }

    /// @dev Build a well-formed envelope: [ownerSigs][payload][uint256 payload.length][typeHash].
    function _envelope(bytes memory ownerSigs, bytes memory payload_, bytes32 typeHash)
        internal
        pure
        returns (bytes memory)
    {
        return bytes.concat(ownerSigs, payload_, abi.encode(payload_.length), typeHash);
    }

    // ----------------------------------------------------------------
    // has
    // ----------------------------------------------------------------

    function test_has_trueForMatchingTypeHash() public view {
        bytes memory sigs = _envelope(hex"aabbcc", hex"1122334455", TYPE_HASH);
        assertTrue(this.callHas(sigs, TYPE_HASH));
    }

    function test_has_falseForDifferentTypeHash() public view {
        bytes memory sigs = _envelope(hex"aabbcc", hex"1122334455", TYPE_HASH);
        assertFalse(this.callHas(sigs, OTHER_TYPE_HASH));
    }

    function test_has_falseWhenShorterThanWord() public view {
        assertFalse(this.callHas(hex"deadbeef", TYPE_HASH));
        assertFalse(this.callHas("", TYPE_HASH));
    }

    // ----------------------------------------------------------------
    // payload
    // ----------------------------------------------------------------

    function test_payload_roundTripsAcrossSizes() public view {
        // Empty payload, a 5-byte (non-word-multiple) payload, and a full 32-byte word.
        bytes memory empty = this.callPayload(_envelope(hex"aabb", "", TYPE_HASH), TYPE_HASH);
        assertEq(empty.length, 0);

        bytes memory five = hex"0102030405";
        assertEq(this.callPayload(_envelope(hex"aabb", five, TYPE_HASH), TYPE_HASH), five);

        bytes memory word = hex"1111111111111111111111111111111111111111111111111111111111111111";
        assertEq(this.callPayload(_envelope(hex"", word, TYPE_HASH), TYPE_HASH), word);
    }

    function test_payload_ignoresOwnerSignatureLength() public view {
        bytes memory p = hex"cafebabe";
        assertEq(this.callPayload(_envelope(hex"", p, TYPE_HASH), TYPE_HASH), p);
        assertEq(this.callPayload(_envelope(hex"00112233445566778899", p, TYPE_HASH), TYPE_HASH), p);
    }

    function test_payload_revertsWhenTooShort() public {
        // Only a type-hash word (32 bytes): shorter than the 64-byte minimum envelope.
        vm.expectRevert(SignatureExtension.MalformedSignatureExtension.selector);
        this.callPayload(bytes.concat(TYPE_HASH), TYPE_HASH);
    }

    function test_payload_revertsOnTypeHashMismatch() public {
        bytes memory sigs = _envelope(hex"aabb", hex"1122", TYPE_HASH);
        vm.expectRevert(SignatureExtension.MalformedSignatureExtension.selector);
        this.callPayload(sigs, OTHER_TYPE_HASH);
    }

    function test_payload_revertsWhenLengthOverrunsFront() public {
        // Well-formed framing, but payloadLength claims more bytes than precede the length word.
        bytes memory sigs = bytes.concat(hex"1122", abi.encode(uint256(64)), TYPE_HASH);
        vm.expectRevert(SignatureExtension.MalformedSignatureExtension.selector);
        this.callPayload(sigs, TYPE_HASH);
    }
}
