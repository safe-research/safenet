// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";

/**
 * @title FROST Coordinator Callback
 * @notice Callback interface for the FROST coordinator.
 */
interface IFROSTCoordinatorCallback {
    /**
     * @notice A key generation ceremony was completed.
     * @param gid The group ID of the completed key generation.
     * @param context The context data associated with the key generation.
     */
    function onKeyGenCompleted(FROSTGroupId.T gid, bytes calldata context) external;

    /**
     * @notice A signature ceremony was successfully completed.
     * @param sid The signature ID of the completed signing ceremony.
     * @param context The context data associated with the signature.
     */
    function onSignCompleted(FROSTSignatureId.T sid, bytes calldata context) external;
}
