// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

/**
 * @title FROST Group ID
 * @notice A FROST coordinator unique group identifier.
 */
library FROSTGroupId {
    // ============================================================
    // TYPES
    // ============================================================

    type T is bytes32;

    // ============================================================
    // ERRORS
    // ============================================================

    /**
     * @notice Thrown when a group ID is invalid.
     */
    error InvalidGroupId();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Computes the deterministic group ID for a given configuration.
     * @param participants The participant merkle root.
     * @param count The number of participants.
     * @param threshold The threshold for the group.
     * @param context The context data for the group.
     * @return result The computed group ID.
     */
    function create(bytes32 participants, uint16 count, uint16 threshold, bytes32 context)
        internal
        pure
        returns (T result)
    {
        bytes32 digest;
        assembly ("memory-safe") {
            let ptr := mload(0x40)
            mstore(ptr, participants)
            mstore(add(ptr, 0x20), count)
            mstore(add(ptr, 0x40), threshold)
            mstore(add(ptr, 0x60), context)
            digest := keccak256(ptr, 0x80)
        }
        return mask(digest);
    }

    /**
     * @notice Masks a `bytes32` to a group ID value.
     * @param raw The raw bytes32 value to mask.
     * @return result The masked group ID.
     */
    function mask(bytes32 raw) internal pure returns (T result) {
        return T.wrap(raw & 0xffffffffffffffffffffffffffffffffffffffffffffffff0000000000000000);
    }

    /**
     * @notice Compares two group IDs.
     * @param a The first group ID.
     * @param b The second group ID.
     * @return result True if the group IDs are equal, false otherwise.
     */
    function eq(T a, T b) internal pure returns (bool result) {
        return T.unwrap(a) == T.unwrap(b);
    }

    /**
     * @notice Checks if a group ID is zero.
     * @param self The group ID to check.
     * @return result True if the group ID is zero, false otherwise.
     */
    function isZero(T self) internal pure returns (bool result) {
        return T.unwrap(self) == bytes32(0);
    }

    /**
     * @notice Requires that a group ID is valid.
     * @param self The group ID to validate.
     */
    function requireValid(T self) internal pure {
        require((uint256(T.unwrap(self)) << 192) == 0, InvalidGroupId());
    }
}
