// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {IOracle} from "@/interfaces/IOracle.sol";
import {SentinelOracleRequest} from "@/libraries/SentinelOracleRequests.sol";
import {SentinelOracleCommitment} from "@/libraries/SentinelOracleCommitments.sol";

interface ISentinelOracle is IOracle {
    // ============================================================
    // EVENTS
    // ============================================================

    event DisputeResolved(bytes32 indexed requestId, address winner, address loser, uint256 slashed);
    event Claimed(bytes32 indexed requestId, address indexed sentinel, uint256 bondReturn, uint256 feeReward);

    // ============================================================
    // VOTING
    // ============================================================

    function commitApprove(bytes32 requestId, uint256 bondAmount) external;

    function commitDeny(bytes32 requestId, uint256 bondAmount) external;

    // ============================================================
    // FINALISATION
    // ============================================================

    function finalize(bytes32 requestId) external;

    function claim(bytes32 requestId) external;

    // ============================================================
    // GOVERNANCE
    // ============================================================

    function addSentinel(address sentinel) external;

    function removeSentinel(address sentinel) external;

    function scheduleBondMultiplier(uint256 newValue) external;

    function applyBondMultiplier() external;

    // ============================================================
    // VIEW FUNCTIONS
    // ============================================================

    function CONSENSUS() external view returns (address);

    function sentinelActiveAt(address sentinel) external view returns (uint256);

    function bondMultiplier() external view returns (uint256);

    function pendingBondMultiplier() external view returns (uint256);

    function pendingBondMultiplierActiveAt() external view returns (uint256);

    function getRequest(bytes32 requestId) external view returns (SentinelOracleRequest.Request memory);

    function getCommitment(bytes32 requestId, address sentinel)
        external
        view
        returns (SentinelOracleCommitment.Commitment memory);
}
