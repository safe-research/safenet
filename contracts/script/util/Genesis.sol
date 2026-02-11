// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";

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
    function participantsRoot(address[] memory participants) internal pure returns (bytes32 result) {
        uint256 depth = 0;
        for (uint256 l = participants.length; l > 1; l = (l + 1) / 2) {
            depth++;
        }

        // forge-lint: disable-next-line(incorrect-shift)
        bytes32[] memory nodes = new bytes32[](1 << depth);
        for (uint256 i = 0; i < participants.length; i++) {
            nodes[i] = keccak256(abi.encode(i + 1, participants[i]));
        }

        for (uint256 w = nodes.length; w > 1; w /= 2) {
            for (uint256 i = 0; i < w; i += 2) {
                (bytes32 a, bytes32 b) = (nodes[i], nodes[i + 1]);
                (bytes32 left, bytes32 right) = a < b ? (a, b) : (b, a);
                nodes[i / 2] = keccak256(abi.encode(left, right));
            }
        }

        return nodes[0];
    }

    /**
     * @notice Computes the genesis group ID for a set of participants and a salt.
     * @param participants The participant addresses.
     * @param salt The genesis salt.
     * @return result The FROST coordinator genesis group ID.
     */
    function groupId(address[] memory participants, bytes32 salt) internal pure returns (FROSTGroupId.T result) {
        bytes32 proot = participantsRoot(participants);
        uint16 count = uint16(participants.length);
        uint16 threshold = (count / 2) + 1;
        bytes32 context = salt == bytes32(0) ? bytes32(0) : keccak256(abi.encodePacked("genesis", salt));
        return FROSTGroupId.create(proot, count, threshold, context);
    }
}
