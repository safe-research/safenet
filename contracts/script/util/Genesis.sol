// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

/**
 * @title Safenet Genesis
 * @notice Library with utilities for computing Safenet genesis parameters.
 */
library Genesis {
    /**
     * @notice Computes the Merkle root for a set of participants. The participant FROST identifiers are assumed to
     *         be sequential starting at 1.
     * @param participants The participant addresses.
     * @return result The Merkle root hash.
     */
    function participantsMerkleRoot(address[] memory participants) internal pure returns (bytes32 result) {
        if (participants.length == 0) {
            return bytes32(0);
        }

        bytes32[] memory nodes = new bytes32[](participants.length);
        for (uint256 i = 0; i < participants.length; i++) {
            nodes[i] = keccak256(abi.encode(i + 1, participants[i]));
        }
        for (uint256 l = participants.length; l > 1; l = (l + 1) / 2) {
            for (uint256 i = 0; i < l; i += 2) {
                bytes32 a = nodes[i];
                bytes32 b = i + 1 < l ? nodes[i + 1] : bytes32(0);
                (bytes32 left, bytes32 right) = a < b ? (a, b) : (b, a);
                nodes[i / 2] = keccak256(abi.encode(left, right));
            }
        }

        return nodes[0];
    }

    /**
     * @notice Computes the genesis group parameters for a set of participants and a salt.
     * @param participants The participant addresses.
     * @param salt The genesis salt.
     * @return participantsRoot The Merkle root of the participants.
     * @return count The number of participants.
     * @return threshold The threshold of participants required for an attestation.
     * @return context The FROST coordinator group context.
     */
    function groupParameters(address[] memory participants, bytes32 salt)
        internal
        pure
        returns (bytes32 participantsRoot, uint16 count, uint16 threshold, bytes32 context)
    {
        participantsRoot = participantsMerkleRoot(participants);
        count = uint16(participants.length);
        threshold = (count / 2) + 1;
        context = salt == bytes32(0) ? bytes32(0) : keccak256(abi.encodePacked("genesis", salt));
    }
}
