// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {IOracle} from "@/interfaces/IOracle.sol";

/**
 * @title Always Approve Oracle
 * @notice A proof-of-concept oracle that immediately approves every request.
 * @dev Emits OracleResult with approved=true synchronously inside postRequest. Useful for integration
 *      testing and demonstrating the end-to-end oracle flow without manual interaction.
 */
contract AlwaysApproveOracle is IOracle {
    /**
     * @inheritdoc IOracle
     */
    function postRequest(bytes32 requestId) external {
        emit OracleResult(requestId, msg.sender, "", true);
    }
}
