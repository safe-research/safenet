// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

/// @title FROST Group ID
/// @notice A FROST coordinator unique group identifier.
library FROSTGroupId {
    type T is bytes32;

    error InvalidGroupId();

    /// @notice Computes the deterministic group ID for a given configuration.
    function create(bytes32 participants, uint64 count, uint64 threshold, bytes32 context)
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

    /// @notice Masks a `bytes32` to a group ID value.
    function mask(bytes32 raw) internal pure returns (T result) {
        return T.wrap(raw & 0xffffffffffffffffffffffffffffffffffffffffffffffff0000000000000000);
    }

    /// @notice Compares two group IDs.
    function eq(T a, T b) internal pure returns (bool result) {
        return T.unwrap(a) == T.unwrap(b);
    }

    /// @notice Requires that a group ID is valid.
    function requireValid(T self) internal pure {
        require((uint256(T.unwrap(self)) << 192) == 0, InvalidGroupId());
    }
}
