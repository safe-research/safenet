// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {MerkleProof} from "@oz/utils/cryptography/MerkleProof.sol";
import {FROST} from "@/lib/FROST.sol";
import {Secp256k1} from "@/lib/Secp256k1.sol";

/// @title FROST Nonce Commitment Set
/// @notice A set of nonce commitments for FROST signature ceremonies.
library FROSTNonceCommitmentSet {
    using Secp256k1 for Secp256k1.Point;

    struct T {
        mapping(FROST.Identifier => Commitments) commitments;
    }

    struct Commitments {
        uint32 next;
        mapping(uint32 chunk => Root) chunks;
    }

    type Root is bytes32;

    error NotIncluded();

    uint256 private constant _CHUNKSZ = 10;
    uint256 private constant _OFFSETMASK = 0x3ff;
    bytes32 private constant _ROOTMASK = 0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffc00;

    /// @notice Commits to the next chunk of nonces, given the current signature
    ///         sequence for a group. This prevents participants commiting to
    ///         nonces _after_ a signing ceremony has already begun.
    function commit(T storage self, FROST.Identifier identifier, bytes32 commitment, uint32 sequence)
        internal
        returns (uint32 chunk)
    {
        Commitments storage commitments = self.commitments[identifier];
        uint256 offset;
        (chunk, offset) = _sequence(sequence);
        uint32 next = commitments.next;
        if (next > chunk) {
            chunk = next;
        }
        commitments.next = chunk + 1;
        commitments.chunks[chunk] = _root(commitment, offset);
    }

    /// @notice Verifies that the specified commitment is part of the set.
    function verify(
        T storage self,
        FROST.Identifier identifier,
        Secp256k1.Point memory d,
        Secp256k1.Point memory e,
        uint32 sequence,
        bytes32[] calldata proof
    ) internal view {
        d.requireNonZero();
        e.requireNonZero();

        (uint32 chunk, uint256 offset) = _sequence(sequence);
        (bytes32 commitment, uint256 startOffset) = _root(self.commitments[identifier].chunks[chunk]);
        require(offset >= startOffset, NotIncluded());

        require(proof.length == _CHUNKSZ, NotIncluded());
        bytes32 digest = MerkleProof.processProofCalldata(proof, _hash(offset, d, e));
        require(digest & _ROOTMASK == commitment, NotIncluded());
    }

    function _hash(uint256 offset, Secp256k1.Point memory d, Secp256k1.Point memory e)
        private
        pure
        returns (bytes32 digest)
    {
        assembly ("memory-safe") {
            let ptr := mload(0x40)
            mstore(ptr, offset)
            mcopy(add(ptr, 0x20), d, 0x40)
            mcopy(add(ptr, 0x60), e, 0x40)
            digest := keccak256(ptr, 0xa0)
        }
    }

    function _sequence(uint32 sequence) private pure returns (uint32 chunk, uint256 offset) {
        chunk = sequence >> _CHUNKSZ;
        offset = uint256(sequence) & _OFFSETMASK;
    }

    function _root(bytes32 commitment, uint256 offset) private pure returns (Root root) {
        return Root.wrap(bytes32(uint256(commitment & _ROOTMASK) | offset));
    }

    function _root(Root root) private pure returns (bytes32 commitment, uint256 offset) {
        commitment = Root.unwrap(root) & _ROOTMASK;
        offset = uint256(Root.unwrap(root)) & _OFFSETMASK;
    }
}
