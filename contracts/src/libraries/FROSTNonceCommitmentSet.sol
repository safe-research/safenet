// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {MerkleProof} from "@oz/utils/cryptography/MerkleProof.sol";
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
     * @custom:param commitments Mapping from participant address to their commitments.
     */
    struct T {
        mapping(address participant => Commitments) commitments;
    }

    /**
     * @notice Commitments storage for a single participant.
     * @custom:param next The next chunk index to use.
     * @custom:param chunks Mapping from chunk index to commitment root.
     * @custom:param nonces Mapping from sequence number to the status of the nonce for that sequence.
     */
    struct Commitments {
        uint64 next;
        mapping(uint64 chunk => Root) chunks;
        mapping(uint64 sequence => SequenceStatus) nonces;
    }

    // ============================================================
    // TYPES
    // ============================================================

    type Root is bytes32;

    /**
     * @notice Tracks the lifecycle state of a participant's nonce for a given signing sequence.
     * @dev A nonce slot starts as `None`, transitions to `Revealed` when the participant calls
     *      `signRevealNonces`, or to `Burned` when the participant calls `signDecline`. These
     *      transitions are mutually exclusive: a revealed nonce cannot be burned and a burned slot
     *      cannot be revealed.
     */
    enum SequenceStatus {
        None,
        Revealed,
        Burned
    }

    // ============================================================
    // ERRORS
    // ============================================================

    /**
     * @notice Thrown when a commitment is not included in the set.
     */
    error NotIncluded();

    /**
     * @notice Thrown when attempting to burn a nonce slot that has already been revealed.
     */
    error NoncesAlreadyRevealed();

    /**
     * @notice Thrown when attempting to reveal or burn a nonce slot that has already been burned.
     */
    error NoncesAlreadyBurned();

    // ============================================================
    // CONSTANTS
    // ============================================================

    /**
     * @dev The size of a nonce chunk, expressed as a power of 2. A chunk size of 10 means each Merkle tree committed
     *      to by a participant contains 2^10 = 1024 nonce commitments. This value balances the gas cost of on-chain
     *      commitments against the off-chain computational overhead for participants. Larger chunks reduce the
     *      frequency of on-chain transactions but require more work upfront.
     */
    uint256 private constant _CHUNKSZ = 10;

    /**
     * @dev A bitmask used to extract the 10-bit offset from a packed sequence number. The value 0x3ff is `2^10 - 1`,
     *      which is `...001111111111` in binary. Applying this mask with a bitwise AND operation isolates the lower 10
     *      bits, which represent the nonce's index within a chunk of 1024 nonces.
     */
    uint256 private constant _OFFSETMASK = 0x3ff;

    /**
     * @dev A bitmask used to extract the Merkle root from a packed `bytes32` value. This mask has its lower 10 bits
     *      set to 0 and the upper 246 bits set to 1. It is used to zero out the bits where the offset is stored,
     *      leaving only the Merkle root part of the packed value. This is part of the gas-saving strategy to pack the
     *      root and offset into a single storage slot.
     */
    bytes32 private constant _ROOTMASK = 0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffc00;

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Commits to the next chunk of nonces, given the current signature sequence for a group. This prevents
     *         participants committing to nonces _after_ a signing ceremony has already begun.
     * @param self The storage struct.
     * @param participant The participant's address.
     * @param commitment The commitment merkle root.
     * @param sequence The current signature sequence.
     * @return chunk The chunk index for this commitment.
     */
    function commit(T storage self, address participant, bytes32 commitment, uint64 sequence)
        internal
        returns (uint64 chunk)
    {
        Commitments storage commitments = self.commitments[participant];
        uint256 offset;
        (chunk, offset) = _sequence(sequence);
        uint64 next = commitments.next;
        if (next > chunk) {
            chunk = next;
            offset = 0;
        }
        commitments.next = chunk + 1;
        commitments.chunks[chunk] = _root(commitment, offset);
    }

    /**
     * @notice Verifies that the specified commitment is part of the set and marks the nonce as revealed.
     * @param self The storage struct.
     * @param participant The participant's address.
     * @param d The first nonce commitment point.
     * @param e The second nonce commitment point.
     * @param sequence The signature sequence.
     * @param proof The Merkle proof for inclusion.
     * @dev Transitions the nonce slot for `(participant, sequence)` from `None` to `Revealed`. Reverts
     *      with `NoncesAlreadyBurned` if the participant has already called `signDecline` for this
     *      sequence (i.e. the slot is `Burned`).
     */
    function verify(
        T storage self,
        address participant,
        Secp256k1.Point memory d,
        Secp256k1.Point memory e,
        uint64 sequence,
        bytes32[] calldata proof
    ) internal {
        require(self.commitments[participant].nonces[sequence] != SequenceStatus.Burned, NoncesAlreadyBurned());

        d.requireNonZero();
        e.requireNonZero();

        (uint64 chunk, uint256 offset) = _sequence(sequence);
        (bytes32 commitment, uint256 startOffset) = _root(self.commitments[participant].chunks[chunk]);
        require(offset >= startOffset, NotIncluded());

        require(proof.length == _CHUNKSZ, NotIncluded());
        bytes32 digest = MerkleProof.processProofCalldata(proof, _hash(offset, d, e));
        require(digest & _ROOTMASK == commitment, NotIncluded());

        self.commitments[participant].nonces[sequence] = SequenceStatus.Revealed;
    }

    /**
     * @notice Burns a nonce slot for a participant, marking it as declined for this sequence.
     * @param self The storage struct.
     * @param participant The participant's address.
     * @param sequence The signature sequence.
     * @dev Transitions the nonce slot from `None` to `Burned`. Reverts with `NoncesAlreadyRevealed`
     *      if nonces were already revealed (participant already called `signRevealNonces`), or with
     *      `NoncesAlreadyBurned` if they already declined.
     */
    function burn(T storage self, address participant, uint64 sequence) internal {
        SequenceStatus status = self.commitments[participant].nonces[sequence];
        require(status != SequenceStatus.Revealed, NoncesAlreadyRevealed());
        require(status != SequenceStatus.Burned, NoncesAlreadyBurned());
        self.commitments[participant].nonces[sequence] = SequenceStatus.Burned;
    }

    /**
     * @notice Returns whether a participant has revealed their nonce for a given sequence.
     * @param self The storage struct.
     * @param participant The participant's address.
     * @param sequence The signature sequence.
     * @return True if the nonce has been revealed.
     */
    function isRevealed(T storage self, address participant, uint64 sequence) internal view returns (bool) {
        return self.commitments[participant].nonces[sequence] == SequenceStatus.Revealed;
    }

    /**
     * @notice Returns whether a participant has burned (declined) their nonce for a given sequence.
     * @param self The storage struct.
     * @param participant The participant's address.
     * @param sequence The signature sequence.
     * @return True if the nonce has been burned.
     */
    function isBurned(T storage self, address participant, uint64 sequence) internal view returns (bool) {
        return self.commitments[participant].nonces[sequence] == SequenceStatus.Burned;
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
     * @dev This function implements a gas-saving packing strategy. It combines a 32-byte Merkle root and a 10-bit
     *      offset into a single `bytes32` storage slot. The offset is stored in the 10 least significant bits, and the
     *      Merkle root occupies the remaining 246 bits. This reduces storage costs but implies that only a prefix of
     *      the Merkle root is stored, slightly increasing the theoretical collision probability (though it remains
     *      negligible in practice).
     */
    function _root(bytes32 commitment, uint256 offset) private pure returns (Root root) {
        return Root.wrap(bytes32(uint256(commitment & _ROOTMASK) | offset));
    }

    /**
     * @notice Extracts commitment and offset from a Root.
     * @param root The Root to decode.
     * @return commitment The commitment hash.
     * @return offset The encoded offset.
     * @dev This function unpacks a `bytes32` value into a Merkle root and an offset, reversing the packing performed
     *      by the `_root` function. It uses bitmasks to separate the 246-bit root from the 10-bit offset.
     */
    function _root(Root root) private pure returns (bytes32 commitment, uint256 offset) {
        commitment = Root.unwrap(root) & _ROOTMASK;
        offset = uint256(Root.unwrap(root)) & _OFFSETMASK;
    }
}
