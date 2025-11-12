// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Hashes} from "@oz/utils/cryptography/Hashes.sol";
import {Secp256k1} from "@/lib/Secp256k1.sol";

/// @title FROST Nonce Commitment Set
/// @notice A set of nonce commitments for FROST signature ceremonies.
library FROSTNonceCommitmentSet {
    using Secp256k1 for Secp256k1.Point;

    struct T {
        mapping(uint256 index => Commitments) commitments;
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
    function commit(T storage self, uint256 index, bytes32 commitment, uint32 sequence)
        internal
        returns (uint32 chunk)
    {
        Commitments storage commitments = self.commitments[index];
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
        uint256 index,
        Secp256k1.Point memory d,
        Secp256k1.Point memory e,
        uint32 sequence,
        bytes32[] calldata proof
    ) internal view {
        (uint32 chunk, uint256 offset) = _sequence(sequence);
        (bytes32 commitment, uint256 startOffset) = _root(self.commitments[index].chunks[chunk]);
        require(offset >= startOffset, NotIncluded());

        // `commitment` represents an **ordered** Merkle tree of `_CHUNKSZ`
        // depth, where the `n`-th leaf represents the nonces for offset `n`.
        bytes32 digest = hash(d, e);
        require(proof.length == _CHUNKSZ, NotIncluded());
        for (uint256 i = 0; i < _CHUNKSZ; i++) {
            bytes32 p = proof[i];
            (bytes32 left, bytes32 right) = (offset >> i) & 1 == 0 ? (digest, p) : (p, digest);
            digest = Hashes.efficientKeccak256(left, right);
        }
        require(digest & _ROOTMASK == commitment, NotIncluded());
    }

    /// @notice Returns the hash of a pair of commited nonces. This hash is a
    ///         leaf in a commitment chunk's Merkle tree.
    function hash(Secp256k1.Point memory d, Secp256k1.Point memory e) internal pure returns (bytes32 digest) {
        d.requireNonZero();
        e.requireNonZero();
        assembly ("memory-safe") {
            let ptr := mload(0x40)
            mcopy(ptr, d, 0x40)
            mcopy(add(ptr, 0x40), e, 0x40)
            digest := keccak256(ptr, 0x80)
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
