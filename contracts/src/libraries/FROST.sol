// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Secp256k1} from "@/libraries/Secp256k1.sol";

/// @title FROST
/// @notice Implementation of FROST(secp256k1, SHA-256) cyphersuite.
library FROST {
    using Secp256k1 for Secp256k1.Point;

    type Identifier is uint256;

    struct Commitment {
        Identifier identifier;
        Secp256k1.Point d;
        Secp256k1.Point e;
    }

    struct Signature {
        Secp256k1.Point r;
        uint256 z;
    }

    struct SignatureShare {
        Secp256k1.Point r;
        uint256 z;
        uint256 l;
    }

    error InvalidIdentifier();
    error UnorderedCommitments();
    error InvalidScalar();

    /// @notice Contruct a FROST participant identifier.
    function newIdentifier(uint256 value) internal pure returns (Identifier identifier) {
        identifier = Identifier.wrap(value);
        requireValidIdentifier(identifier);
    }

    /// @notice Requires that an identifier is valid.
    function requireValidIdentifier(Identifier identifier) internal pure {
        require(Identifier.unwrap(identifier) != 0, InvalidIdentifier());
    }

    /// @notice Generate a random nonce from some randomness and a secret key.
    function nonce(bytes32 random, uint256 secret) internal view returns (uint256 n) {
        // The RFC-9591 `nonce_generate` function.
        // <https://datatracker.ietf.org/doc/html/rfc9591#section-4.1>

        return _h3(abi.encodePacked(random, secret));
    }

    /// @notice Compute the binding factors for a message and group commitments.
    function bindingFactors(Secp256k1.Point memory y, Commitment[] memory commitments, bytes32 message)
        internal
        view
        returns (uint256[] memory rho)
    {
        // The RFC-9591 `compute_binding_factors` function.
        // <https://datatracker.ietf.org/doc/html/rfc9591#section-4.4>

        (uint8 yv, bytes32 yx) = y.serialize();
        bytes32 msgHash = _h4(abi.encode(message));
        bytes32 commitmentsHash = _h5(_encodeCommitments(commitments));
        bytes memory rhoInput = abi.encodePacked(yv, yx, msgHash, commitmentsHash, uint256(0));
        uint256 rhoIndexPtr;
        assembly ("memory-safe") {
            rhoIndexPtr := add(rhoInput, 0x81)
        }

        rho = new uint256[](commitments.length);
        for (uint256 i = 0; i < commitments.length; i++) {
            Identifier identifier = commitments[i].identifier;
            assembly ("memory-safe") {
                mstore(rhoIndexPtr, identifier)
            }
            rho[i] = _h1(rhoInput);
        }
    }

    /// @notice Compute the challenge for a message from a group commitment and
    ///         group public key.
    function challenge(Secp256k1.Point memory r, Secp256k1.Point memory y, bytes32 message)
        internal
        view
        returns (uint256 c)
    {
        // The RFC-9591 `compute_challenge` function.
        // <https://datatracker.ietf.org/doc/html/rfc9591#section-4.6>

        (uint8 rv, bytes32 rx) = r.serialize();
        (uint8 yv, bytes32 yx) = y.serialize();
        return _h2(abi.encodePacked(rv, rx, yv, yx, message));
    }

    /// @notice Verifies a FROST signature.
    function verify(Secp256k1.Point memory y, Signature memory signature, bytes32 message) internal view {
        require(signature.z < Secp256k1.N, InvalidScalar());
        uint256 c = challenge(signature.r, y, message);
        Secp256k1.mulmuladd(signature.z, c, y, signature.r);
    }

    /// @notice Verifies a FROST signature share.
    function verifyShare(
        Secp256k1.Point memory group,
        Secp256k1.Point memory r,
        Secp256k1.Point memory participant,
        SignatureShare memory share,
        bytes32 message
    ) internal view {
        require(share.z < Secp256k1.N, InvalidScalar());
        uint256 c = challenge(r, group, message);
        uint256 cl = mulmod(c, share.l, Secp256k1.N);
        Secp256k1.mulmuladd(share.z, cl, participant, share.r);
    }

    /// @notice Generate a KeyGen challenge for the proof of knowledge.
    function keyGenChallenge(Identifier identifier, Secp256k1.Point memory phi, Secp256k1.Point memory r)
        internal
        view
        returns (uint256 c)
    {
        // The official FROST implementation KeyGen `challenge` function.
        // <https://github.com/ZcashFoundation/frost/blob/3ffc19d8f473d5bc4e07ed41bc884bdb42d6c29f/frost-core/src/keys/dkg.rs#L413-L430>
        // <https://github.com/ZcashFoundation/frost/blob/3ffc19d8f473d5bc4e07ed41bc884bdb42d6c29f/frost-secp256k1/src/lib.rs#L222-L224>

        (uint8 phiv, bytes32 phix) = phi.serialize();
        (uint8 rv, bytes32 rx) = r.serialize();
        return _hdkg(abi.encodePacked(identifier, phiv, phix, rv, rx));
    }

    function _encodeCommitments(Commitment[] memory commitments) private pure returns (bytes memory result) {
        // The RFC-9591 `encode_group_commitment_list` function.
        // <https://datatracker.ietf.org/doc/html/rfc9591#section-4.3>

        result = new bytes(commitments.length * 98);

        uint256 identifier = 0;
        for (uint256 i = 0; i < commitments.length; i++) {
            Commitment memory commitment = commitments[i];

            require(Identifier.unwrap(commitment.identifier) > identifier, UnorderedCommitments());
            identifier = Identifier.unwrap(commitment.identifier);

            (uint8 dv, bytes32 dx) = commitment.d.serialize();
            (uint8 ev, bytes32 ex) = commitment.e.serialize();
            assembly ("memory-safe") {
                let ptr := add(add(result, 0x20), mul(i, 98))
                mstore(ptr, identifier)
                mstore8(add(ptr, 0x20), dv)
                mstore(add(ptr, 0x21), dx)
                mstore8(add(ptr, 0x41), ev)
                mstore(add(ptr, 0x42), ex)
            }
        }
    }

    function _h1(bytes memory input) private view returns (uint256 result) {
        return _hashToField(input, "FROST-secp256k1-SHA256-v1rho\x00\x00\x00\x1c");
    }

    function _h2(bytes memory input) private view returns (uint256 result) {
        return _hashToField(input, "FROST-secp256k1-SHA256-v1chal\x00\x00\x1d");
    }

    function _h3(bytes memory input) private view returns (uint256 result) {
        return _hashToField(input, "FROST-secp256k1-SHA256-v1nonce\x00\x1e");
    }

    function _h4(bytes memory input) private view returns (bytes32 result) {
        return _hash(input, "FROST-secp256k1-SHA256-v1msg\x00\x00\x00\x1c");
    }

    function _h5(bytes memory input) private view returns (bytes32 result) {
        return _hash(input, "FROST-secp256k1-SHA256-v1com\x00\x00\x00\x1c");
    }

    function _hdkg(bytes memory input) private view returns (uint256 result) {
        return _hashToField(input, "FROST-secp256k1-SHA256-v1dkg\x00\x00\x00\x1c");
    }

    function _hashToField(bytes memory message, bytes32 dst) private view returns (uint256 e) {
        // The RFC-9380 `hash_to_field` function with:
        // - F: the finite field of order Secp256k1.N
        // - p: Secp256k1.N
        // - m: 1
        // - L: 48
        // <https://datatracker.ietf.org/doc/html/rfc9380#section-5.2>

        bytes memory uniform = _expandMessageXmd(message, dst, 48);
        uint256 n = Secp256k1.N;
        assembly ("memory-safe") {
            e := mulmod(mload(add(uniform, 0x20)), 0x100000000000000000000000000000000, n)
            e := addmod(e, shr(128, mload(add(uniform, 0x40))), n)
        }
    }

    function _expandMessageXmd(bytes memory message, bytes32 dst, uint256 len)
        private
        view
        returns (bytes memory uniform)
    {
        // The RFC-9380 `expand_message_xmd` function with:
        // - H: SHA-256
        // - b_in_bytes: 32
        // - s_in_bytes: 64
        // <https://datatracker.ietf.org/doc/html/rfc9380#section-5.3.1>

        assert(len < 0x8000);
        assembly ("memory-safe") {
            function _sha256(inputPtr, inputLen, outputPtr) {
                if iszero(
                    and(eq(returndatasize(), 0x20), staticcall(gas(), 0x2, inputPtr, inputLen, outputPtr, 0x20))
                ) {
                    revert(0x00, 0x00)
                }
            }

            uniform := mload(0x40)
            mstore(0x40, add(uniform, and(add(0x3f, len), 0xffe0)))
            mstore(uniform, len)

            let prime := mload(0x40)
            let ptr := prime

            mstore(ptr, 0)
            ptr := add(ptr, 0x20)
            mstore(ptr, 0)
            ptr := add(ptr, 0x20)

            mcopy(ptr, add(message, 0x20), mload(message))
            ptr := add(ptr, mload(message))
            mstore(ptr, shl(240, len))
            ptr := add(ptr, 0x03)

            let bPtr := sub(ptr, 0x21)
            let iPtr := sub(ptr, 0x01)

            mstore(ptr, dst)
            let dstLen := byte(31, dst)
            ptr := add(ptr, dstLen)
            mstore8(ptr, dstLen)
            ptr := add(ptr, 0x01)

            let bLen := sub(ptr, bPtr)

            _sha256(prime, sub(ptr, prime), bPtr)
            let b0 := mload(bPtr)
            mstore8(iPtr, 1)
            _sha256(bPtr, bLen, add(uniform, 0x20))
            for { let i := 2 } gt(len, 0x20) {
                i := add(i, 1)
                len := sub(len, 32)
            } {
                let uPtr := add(uniform, shl(5, i))
                mstore(bPtr, xor(b0, mload(sub(uPtr, 0x20))))
                mstore8(iPtr, i)
                _sha256(bPtr, bLen, uPtr)
            }
        }
    }

    function _hash(bytes memory input, bytes32 dst) private view returns (bytes32 digest) {
        assembly ("memory-safe") {
            let dstLen := byte(31, dst)
            let dstOffset := sub(32, dstLen)
            let len := mload(input)
            mstore(input, shr(shl(3, dstOffset), dst))
            if iszero(
                and(
                    eq(returndatasize(), 0x20),
                    staticcall(gas(), 0x2, add(input, dstOffset), add(len, dstLen), 0x00, 0x20)
                )
            ) { revert(0x00, 0x00) }
            digest := mload(0x00)
            mstore(input, len)
        }
    }
}
