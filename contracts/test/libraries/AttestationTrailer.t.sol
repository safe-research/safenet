// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {AttestationTrailer} from "@/libraries/AttestationTrailer.sol";
import {FROST} from "@/libraries/FROST.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

/**
 * @title AttestationTrailerTest
 * @notice Unit tests for the `AttestationTrailer` wire-format library. `hasTrailer` is pure recognition;
 *         `decode` extracts the payload and reverts `MalformedAttestationTrailer` on a too-short blob.
 *         Both take `calldata`, so they are exercised through external `this.call*` wrappers (which also
 *         lets `vm.expectRevert` observe the internal revert at a lower call depth).
 */
contract AttestationTrailerTest is Test {
    uint64 internal constant EPOCH = 7;

    function callHasTrailer(bytes calldata signatures) external pure returns (bool) {
        return AttestationTrailer.hasTrailer(signatures);
    }

    function callDecode(bytes calldata signatures)
        external
        pure
        returns (uint64 epoch, Secp256k1.Point memory groupKey, FROST.Signature memory signature)
    {
        return AttestationTrailer.decode(signatures);
    }

    function _key() internal pure returns (Secp256k1.Point memory) {
        return Secp256k1.Point({x: 111, y: 222});
    }

    function _sig() internal pure returns (FROST.Signature memory) {
        return FROST.Signature({r: Secp256k1.Point({x: 333, y: 444}), z: 555});
    }

    // A full, well-formed trailer: [owner sigs][192-byte payload][32-byte TYPE_HASH].
    function _trailer(bytes memory ownerSigs) internal pure returns (bytes memory) {
        bytes memory payload = abi.encode(EPOCH, _key(), _sig());
        return bytes.concat(ownerSigs, payload, AttestationTrailer.TYPE_HASH);
    }

    function test_hasTrailer_trueForWellFormed() public view {
        assertTrue(this.callHasTrailer(_trailer(hex"aabbcc")));
    }

    function test_hasTrailer_falseWhenNoTypeHash() public view {
        // A plausible Safe signature blob ending in an unrelated word (even the number 192).
        bytes memory sigs = bytes.concat(hex"aabbcc", bytes32(uint256(192)));
        assertFalse(this.callHasTrailer(sigs));
    }

    function test_hasTrailer_falseWhenShorterThanWord() public view {
        assertFalse(this.callHasTrailer(hex"deadbeef"));
        assertFalse(this.callHasTrailer(""));
    }

    function test_decode_roundTrips() public view {
        (uint64 epoch, Secp256k1.Point memory k, FROST.Signature memory s) = this.callDecode(_trailer(hex"aabbcc"));
        assertEq(epoch, EPOCH);
        assertEq(k.x, 111);
        assertEq(k.y, 222);
        assertEq(s.r.x, 333);
        assertEq(s.r.y, 444);
        assertEq(s.z, 555);
    }

    function test_decode_ignoresOwnerSignatureLength() public view {
        // Payload is sliced from the tail, so any owner-signature prefix decodes to the same values.
        (uint64 epoch,,) = this.callDecode(_trailer(hex""));
        assertEq(epoch, EPOCH);
        (uint64 epoch2,,) = this.callDecode(_trailer(hex"0102030405060708090a0b0c0d0e0f"));
        assertEq(epoch2, EPOCH);
    }

    function test_decode_revertsWhenTooShort() public {
        // Type hash present but the blob is shorter than a full 224-byte trailer.
        vm.expectRevert(AttestationTrailer.MalformedAttestationTrailer.selector);
        this.callDecode(bytes.concat(AttestationTrailer.TYPE_HASH));
    }
}
