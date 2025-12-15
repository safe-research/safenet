// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {MerkleProof} from "@oz/utils/cryptography/MerkleProof.sol";
import {FROST} from "@/libraries/FROST.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

/**
 * @title FROST Nonce Commitment Set
 * @notice A set of nonce commitments for FROST signature ceremonies.
 */
library FROSTNonceCommitmentSet {
    using Secp256k1 for Secp256k1.Point;

    // ============================================================
    // STRUCTS
    // ============================================================

    /**
     * @notice The main storage struct for tracking nonce commitments.
     * @param commitments Mapping from participant identifier to their commitments.
     */
    struct T {
        mapping(FROST.Identifier => Commitments) commitments;
    }

    /**
     * @notice Commitments storage for a single participant.
     * @param next The next chunk index to use.
     * @param chunks Mapping from chunk index to commitment root.
     */
    struct Commitments {
        uint64 next;
        mapping(uint64 chunk => Root) chunks;
    }

    // ============================================================
    // TYPES
    // ============================================================

    type Root is bytes32;

    // ============================================================
    // ERRORS
    // ============================================================

    /**
     * @notice Thrown when a commitment is not included in the set.
     */
    error NotIncluded();

    // ============================================================
    // CONSTANTS
    // ============================================================

    uint256 private constant _CHUNKSZ = 10;
    uint256 private constant _OFFSETMASK = 0x3ff;
    bytes32 private constant _ROOTMASK = 0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffc00;

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Commits to the next chunk of nonces, given the current signature
     *         sequence for a group. This prevents participants commiting to
     *         nonces _after_ a signing ceremony has already begun.
     * @param self The storage struct.
     * @param identifier The participant's FROST identifier.
     * @param commitment The commitment merkle root.
     * @param sequence The current signature sequence.
     * @return chunk The chunk index for this commitment.
     */
    function commit(T storage self, FROST.Identifier identifier, bytes32 commitment, uint64 sequence)
        internal
        returns (uint64 chunk)
    {
        Commitments storage commitments = self.commitments[identifier];
        uint256 offset;
        (chunk, offset) = _sequence(sequence);
        uint64 next = commitments.next;
        if (next > chunk) {
            chunk = next;
        }
        commitments.next = chunk + 1;
        commitments.chunks[chunk] = _root(commitment, offset);
    }

    /**
     * @notice Verifies that the specified commitment is part of the set.
     * @param self The storage struct.
     * @param identifier The participant's FROST identifier.
     * @param d The first nonce commitment point.
     * @param e The second nonce commitment point.
     * @param sequence The signature sequence.
     * @param proof The Merkle proof for inclusion.
     */
    function verify(
        T storage self,
        FROST.Identifier identifier,
        Secp256k1.Point memory d,
        Secp256k1.Point memory e,
        uint64 sequence,
        bytes32[] calldata proof
    ) internal view {
        d.requireNonZero();
        e.requireNonZero();

        (uint64 chunk, uint256 offset) = _sequence(sequence);
        (bytes32 commitment, uint256 startOffset) = _root(self.commitments[identifier].chunks[chunk]);
        require(offset >= startOffset, NotIncluded());

        require(proof.length == _CHUNKSZ, NotIncluded());
        bytes32 digest = MerkleProof.processProofCalldata(proof, _hash(offset, d, e));
        require(digest & _ROOTMASK == commitment, NotIncluded());
    }

    // ============================================================
    // PRIVATE FUNCTIONS
    // ============================================================

    /**
     * @notice Computes the leaf hash for a nonce commitment.
     * @param offset The offset within the chunk.
     * @param d The first nonce commitment point.
     * @param e The second nonce commitment point.
     * @return digest The computed leaf hash.
     */
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

    /**
     * @notice Extracts chunk and offset from a sequence number.
     * @param sequence The signature sequence number.
     * @return chunk The chunk index.
     * @return offset The offset within the chunk.
     */
    function _sequence(uint64 sequence) private pure returns (uint64 chunk, uint256 offset) {
        chunk = sequence >> _CHUNKSZ;
        offset = uint256(sequence) & _OFFSETMASK;
    }

    /**
     * @notice Creates a Root from a commitment and offset.
     * @param commitment The commitment hash.
     * @param offset The offset to encode.
     * @return root The encoded Root.
     */
    function _root(bytes32 commitment, uint256 offset) private pure returns (Root root) {
        return Root.wrap(bytes32(uint256(commitment & _ROOTMASK) | offset));
    }

    /**
     * @notice Extracts commitment and offset from a Root.
     * @param root The Root to decode.
     * @return commitment The commitment hash.
     * @return offset The encoded offset.
     */
    function _root(Root root) private pure returns (bytes32 commitment, uint256 offset) {
        commitment = Root.unwrap(root) & _ROOTMASK;
        offset = uint256(Root.unwrap(root)) & _OFFSETMASK;
    }
}
