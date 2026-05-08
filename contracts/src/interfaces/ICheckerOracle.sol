// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {IOracle} from "@/interfaces/IOracle.sol";

/**
 * @title Checker Oracle Interface
 * @notice Interface for the CheckerOracle contract implementing competitive transaction checking.
 */
interface ICheckerOracle is IOracle {
    // ============================================================
    // EVENTS
    // ============================================================

    /**
     * @notice Emitted when a new request is posted.
     * @param requestId The EIP-712 hash of the OracleTransactionProposal message.
     * @param proposer The address that posted the request (typically the Consensus contract).
     * @param fee The user fee being escrowed.
     * @param approveBondTarget The bond target for Approve side (fee × bondMultiplier).
     * @param deadline The block number when the voting window closes.
     */
    event NewRequest(
        bytes32 indexed requestId, address indexed proposer, uint256 fee, uint256 approveBondTarget, uint256 deadline
    );

    /**
     * @notice Emitted when a checker commits a vote.
     * @param requestId The EIP-712 hash of the OracleTransactionProposal message.
     * @param checker The address of the checker committing the vote.
     * @param approved true for Approve vote, false for Deny vote.
     * @param bondAmount The bond amount committed.
     * @param position The arrival position (1-indexed) among voters on this side.
     */
    event Committed(
        bytes32 indexed requestId, address indexed checker, bool approved, uint256 bondAmount, uint256 position
    );

    /**
     * @notice Emitted when a request is resolved.
     * @param requestId The EIP-712 hash of the OracleTransactionProposal message.
     * @param approved Whether the request is approved.
     * @param reason The reason for resolution.
     */
    event Resolved(bytes32 indexed requestId, bool approved, ResolveReason reason);

    /**
     * @notice Emitted when a checker claims their rewards.
     * @param requestId The EIP-712 hash of the OracleTransactionProposal message.
     * @param checker The address of the checker claiming.
     * @param bondReturn The bond amount returned.
     * @param feeReward The fee reward earned.
     */
    event Claimed(bytes32 indexed requestId, address indexed checker, uint256 bondReturn, uint256 feeReward);

    /**
     * @notice Emitted when a checker is scheduled to become active.
     * @param checker The address of the checker.
     * @param activeAtBlock The block number when the checker becomes active.
     */
    event CheckerScheduled(address indexed checker, uint256 activeAtBlock);

    /**
     * @notice Emitted when a checker is removed.
     * @param checker The address of the checker removed.
     */
    event CheckerRemoved(address indexed checker);

    /**
     * @notice Emitted when a new bond multiplier is scheduled.
     * @param newMultiplier The new bond multiplier value.
     * @param activeAtBlock The block number when it becomes active.
     */
    event BondMultiplierScheduled(uint256 newMultiplier, uint256 activeAtBlock);

    /**
     * @notice Emitted when a scheduled bond multiplier is applied.
     * @param newMultiplier The new bond multiplier value.
     */
    event BondMultiplierApplied(uint256 newMultiplier);

    // ============================================================
    // STRUCTS
    // ============================================================

    /**
     * @notice Reason for request resolution.
     * @custom:enumValue UNANIMOUS_APPROVE Request approved unanimously.
     * @custom:enumValue UNANIMOUS_DENY Request denied unanimously.
     * @custom:enumValue TIMEOUT Request timed out with neither threshold met.
     */
    enum ResolveReason {
        UNANIMOUS_APPROVE,
        UNANIMOUS_DENY,
        TIMEOUT
    }

    // ============================================================
    // EXTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Commit an Approve vote for a request.
     * @param requestId The EIP-712 hash of the OracleTransactionProposal message.
     * @dev Only active checkers can commit. The checker must have approved this contract to pull the bond amount.
     *      The bond is pulled via transferFrom at call time.
     */
    function commitApprove(bytes32 requestId) external;

    /**
     * @notice Commit a Deny vote for a request.
     * @param requestId The EIP-712 hash of the OracleTransactionProposal message.
     * @dev Only active checkers can commit. The checker must have approved this contract to pull the bond amount.
     *      The bond is pulled via transferFrom at call time.
     */
    function commitDeny(bytes32 requestId) external;

    /**
     * @notice Finalize a request after the voting window closes or when threshold is reached.
     * @param requestId The EIP-712 hash of the OracleTransactionProposal message.
     * @dev Can be called by anyone. Resolves the request and caches the total score for fee distribution.
     */
    function finalize(bytes32 requestId) external;

    /**
     * @notice Claim bond return and fee reward for a checker.
     * @param requestId The EIP-712 hash of the OracleTransactionProposal message.
     * @dev Can only be called by the checker who committed a vote.
     */
    function claim(bytes32 requestId) external;

    /**
     * @notice Add a checker to the permissioned set (time-delayed).
     * @param checker The address to add as a checker.
     * @dev Only ARBITRATOR can call. Checker becomes active after GOVERNANCE_DELAY blocks.
     */
    function addChecker(address checker) external;

    /**
     * @notice Remove a checker from the permissioned set (immediate).
     * @param checker The address to remove from the checker set.
     * @dev Only ARBITRATOR can call. Immediate effect.
     */
    function removeChecker(address checker) external;

    /**
     * @notice Schedule a new bond multiplier (time-delayed).
     * @param newValue The new bond multiplier value to set.
     * @dev Only ARBITRATOR can call. Not applied until after GOVERNANCE_DELAY blocks.
     */
    function scheduleBondMultiplier(uint256 newValue) external;

    /**
     * @notice Apply a scheduled bond multiplier (once activation block is reached).
     * @dev Anyone can call once the activation block has passed.
     */
    function applyBondMultiplier() external;

    // ============================================================
    // VIEW FUNCTIONS
    // ============================================================

    /**
     * @notice Check if an address is an active checker.
     * @param checker The address to check.
     * @return true if checker is active, false otherwise.
     */
    function isActiveChecker(address checker) external view returns (bool);

    /**
     * @notice Get the current bond multiplier.
     * @return The current bond multiplier value.
     */
    function bondMultiplier() external view returns (uint256);

    /**
     * @notice Get the ARBITRATOR address.
     * @return The ARBITRATOR address.
     */
    function arbitrator() external view returns (address);
}
