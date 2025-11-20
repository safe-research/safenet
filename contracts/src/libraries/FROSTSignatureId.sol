// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";

/// @title FROST Signature ID
/// @notice A FROST coordinator unique signature identifier.
library FROSTSignatureId {
    type T is bytes32;

    error InvalidSignatureId();

    /// @notice Computes the deterministic group ID for a given configuration.
    function create(FROSTGroupId.T gid, uint64 seq) internal pure returns (T result) {
        // We encode `sequence + 1` in the signature ID. This allows us to tell
        // whether an ID belongs to a group or a signature by non-zero LSBs.
        return T.wrap(FROSTGroupId.T.unwrap(gid) | bytes32(uint256(seq + 1)));
    }

    /// @notice Returns a signature ID's group ID.
    function group(T self) internal pure returns (FROSTGroupId.T result) {
        return FROSTGroupId.mask(T.unwrap(self));
    }

    /// @notice Returns a signature ID's sequence.
    function sequence(T self) internal pure returns (uint64 result) {
        unchecked {
            return uint64(uint256(T.unwrap(self)) - 1);
        }
    }

    /// @notice Requires that a signature ID is valid.
    function requireValid(T self) internal pure {
        require((uint256(T.unwrap(self)) << 192) != 0, InvalidSignatureId());
    }
}
