// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";

/**
 * @title FROST Signature ID
 * @notice A FROST coordinator unique signature identifier.
 */
library FROSTSignatureId {
    // ============================================================
    // TYPES
    // ============================================================

    type T is bytes32;

    // ============================================================
    // ERRORS
    // ============================================================

    /**
     * @notice Thrown when a signature ID is invalid.
     */
    error InvalidSignatureId();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Computes the deterministic signature ID for a given configuration.
     * @param gid The group ID.
     * @param seq The sequence number.
     * @return result The computed signature ID.
     * @dev We encode `sequence + 1` in the signature ID. This allows us to tell
     *      whether an ID belongs to a group or a signature by non-zero LSBs.
     */
    function create(FROSTGroupId.T gid, uint64 seq) internal pure returns (T result) {
        return T.wrap(FROSTGroupId.T.unwrap(gid) | bytes32(uint256(seq + 1)));
    }

    /**
     * @notice Returns a signature ID's group ID.
     * @param self The signature ID.
     * @return result The group ID.
     */
    function group(T self) internal pure returns (FROSTGroupId.T result) {
        return FROSTGroupId.mask(T.unwrap(self));
    }

    /**
     * @notice Returns a signature ID's sequence.
     * @param self The signature ID.
     * @return result The sequence number.
     */
    function sequence(T self) internal pure returns (uint64 result) {
        unchecked {
            return uint64(uint256(T.unwrap(self)) - 1);
        }
    }

    /**
     * @notice Checks if a signature ID is zero.
     * @param self The signature ID to check.
     * @return result True if the signature ID is zero, false otherwise.
     */
    function isZero(T self) internal pure returns (bool result) {
        return T.unwrap(self) == bytes32(0);
    }

    /**
     * @notice Requires that a signature ID is valid.
     * @param self The signature ID to validate.
     */
    function requireValid(T self) internal pure {
        require((uint256(T.unwrap(self)) << 192) != 0, InvalidSignatureId());
    }
}
