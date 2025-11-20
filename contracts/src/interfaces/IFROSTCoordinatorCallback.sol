// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";

/// @title FROST Coordinator Callback
/// @notice Callback interface for the FROST coordinator.
interface IFROSTCoordinatorCallback {
    /// @notice A key generation ceremony was completed.
    function onKeyGenCompleted(FROSTGroupId.T gid, bytes calldata context) external;

    /// @notice A signature ceremony was successfully completed.
    function onSignCompleted(FROSTSignatureId.T sid, bytes calldata context) external;
}
