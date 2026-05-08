// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

/**
 * @title Checker Oracle Interface
 * @notice Interface for the WardensGame competitive transaction checker oracle.
 * @dev Checker nodes race to post bonded Approve/Deny votes within a time-boxed window.
 *      The contract escrows the proposer's fee and distributes it to the winning side
 *      proportionally by a capital-weighted speed score after finalisation.
 */
interface ICheckerOracle {
    // ============================================================
    // ENUMS
    // ============================================================

    /**
     * @notice Lifecycle state of a request.
     */
    enum State {
        PENDING,
        FROZEN,
        RESOLVED
    }

    /**
     * @notice Reason a request was resolved.
     */
    enum ResolveReason {
        UNANIMOUS_APPROVE,
        UNANIMOUS_DENY,
        TIMEOUT,
        ARBITRATION
    }

    // ============================================================
    // STRUCTS
    // ============================================================

    /**
     * @notice On-chain record for a single oracle request.
     * @custom:param proposer           Address that called postRequest (typically the Consensus contract).
     * @custom:param fee                Fee locked at request time; disbursed or refunded at resolution.
     *                                  Set to zero at finalisation when the request times out.
     * @custom:param approveBondTarget  Aggregate bond target for each side: fee × bondMultiplier.
     * @custom:param deadline           Block number after which the voting window is closed.
     * @custom:param state              Current lifecycle state.
     * @custom:param totalApproveBond   Running aggregate of Approve bonds collected so far.
     * @custom:param totalDenyBond      Running aggregate of Deny bonds collected so far.
     * @custom:param approveCheckerCount Number of Approve-side commitments recorded.
     * @custom:param denyCheckerCount    Number of Deny-side commitments recorded.
     * @custom:param approveTotalScore  Running sum of Approve-side scores (bond / position); updated on commit.
     * @custom:param denyTotalScore     Running sum of Deny-side scores (bond / position); updated on commit.
     * @custom:param approvedOutcome    True if the winning resolution is Approve; set at finalisation.
     * @custom:param arbitrated         True once dispute resolution has been triggered (Phase 2).
     */
    struct Request {
        address proposer;
        uint256 fee;
        uint256 approveBondTarget;
        uint256 deadline;
        State state;
        uint256 totalApproveBond;
        uint256 totalDenyBond;
        uint256 approveCheckerCount;
        uint256 denyCheckerCount;
        uint256 approveTotalScore;
        uint256 denyTotalScore;
        bool approvedOutcome;
        bool arbitrated;
    }

    /**
     * @notice Bond commitment made by a checker for a specific request.
     * @custom:param approved    True = Approve vote, false = Deny vote.
     * @custom:param bondAmount  Effective bond amount locked; may be less than submitted if it fills
     *                          the remaining threshold gap.
     * @custom:param position    1-indexed arrival order among eligible voters on the same side.
     * @custom:param claimed     True once the checker has called claim().
     */
    struct Commitment {
        bool approved;
        uint256 bondAmount;
        uint256 position;
        bool claimed;
    }

    // ============================================================
    // EVENTS
    // ============================================================

    /**
     * @notice Emitted when a new oracle request is opened.
     * @param requestId       EIP-712 hash of the OracleTransactionProposal.
     * @param proposer        Address that called postRequest.
     * @param fee             Fee locked in escrow.
     * @param approveBondTarget Aggregate bond target for each side.
     * @param deadline        Block number at which the voting window closes.
     */
    event NewRequest(
        bytes32 indexed requestId, address indexed proposer, uint256 fee, uint256 approveBondTarget, uint256 deadline
    );

    /**
     * @notice Emitted when a checker is scheduled to become active.
     * @param checker     The checker address.
     * @param activeAtBlock Block number from which the checker will be considered active.
     */
    event CheckerScheduled(address indexed checker, uint256 activeAtBlock);

    /**
     * @notice Emitted when a checker is removed from the active set.
     * @param checker The checker address.
     */
    event CheckerRemoved(address indexed checker);

    /**
     * @notice Emitted when a new bond multiplier is staged.
     * @param newMultiplier The staged multiplier value.
     * @param activeAtBlock Block number from which the new multiplier will be active.
     */
    event BondMultiplierScheduled(uint256 newMultiplier, uint256 activeAtBlock);

    /**
     * @notice Emitted when the staged bond multiplier is applied.
     * @param newMultiplier The newly active multiplier value.
     */
    event BondMultiplierApplied(uint256 newMultiplier);

    /**
     * @notice Emitted when a checker posts a bond commitment.
     * @param requestId  The request being voted on.
     * @param checker    The committing checker.
     * @param approved   True for an Approve vote, false for a Deny vote.
     * @param bondAmount Effective bond amount locked.
     * @param position   Arrival position among eligible voters on the same side.
     */
    event Committed(
        bytes32 indexed requestId, address indexed checker, bool approved, uint256 bondAmount, uint256 position
    );

    /**
     * @notice Emitted when a request is resolved.
     * @param requestId The resolved request.
     * @param approved  True if the outcome is Approve.
     * @param reason    Why the request was resolved.
     */
    event Resolved(bytes32 indexed requestId, bool approved, ResolveReason reason);

    /**
     * @notice Emitted when arbitration is triggered on a conflicted request (Phase 2).
     */
    event ArbitrationTriggered(bytes32 indexed requestId);

    /**
     * @notice Emitted when a dispute is resolved by the arbitrator (Phase 2).
     */
    event DisputeResolved(bytes32 indexed requestId, address winner, address loser, uint256 slashed);

    /**
     * @notice Emitted when a checker claims their bond and fee reward.
     * @param requestId  The claimed request.
     * @param checker    The claiming checker.
     * @param bondReturn Bond amount returned.
     * @param feeReward  Proportional fee reward paid.
     */
    event Claimed(bytes32 indexed requestId, address indexed checker, uint256 bondReturn, uint256 feeReward);

    // ============================================================
    // EXTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Post a bond committing to Approve the request.
     * @param requestId  The EIP-712 hash of the OracleTransactionProposal.
     * @param bondAmount Amount of fee token to bond. The contract pulls at most the remaining
     *                   gap to the Approve threshold; any unneeded surplus is not collected.
     */
    function commitApprove(bytes32 requestId, uint256 bondAmount) external;

    /**
     * @notice Post a bond committing to Deny the request.
     * @param requestId  The EIP-712 hash of the OracleTransactionProposal.
     * @param bondAmount Amount of fee token to bond. The contract pulls at most the remaining
     *                   gap to the Deny threshold; any unneeded surplus is not collected.
     */
    function commitDeny(bytes32 requestId, uint256 bondAmount) external;

    /**
     * @notice Resolve the request after the voting window has closed.
     * @dev Callable by anyone. Determines the outcome from the collected bonds and emits OracleResult.
     *      If both thresholds are met (conflict), sets state to FROZEN for Phase 2 arbitration.
     * @param requestId The EIP-712 hash of the OracleTransactionProposal.
     */
    function finalize(bytes32 requestId) external;

    /**
     * @notice Claim bond return and proportional fee reward after resolution.
     * @param requestId The EIP-712 hash of the OracleTransactionProposal.
     */
    function claim(bytes32 requestId) external;

    /**
     * @notice Schedule a new checker to become active after GOVERNANCE_DELAY blocks.
     * @param checker The address to add to the permissioned checker set.
     */
    function addChecker(address checker) external;

    /**
     * @notice Immediately remove a checker from the active set.
     * @param checker The address to remove.
     */
    function removeChecker(address checker) external;

    /**
     * @notice Stage a new bond multiplier, to take effect after GOVERNANCE_DELAY blocks.
     * @param newValue The new multiplier value (must be > 0).
     */
    function scheduleBondMultiplier(uint256 newValue) external;

    /**
     * @notice Apply the staged bond multiplier once its activation block has been reached.
     */
    function applyBondMultiplier() external;

    /**
     * @notice Returns the block number from which a checker is considered active, or 0 if not scheduled.
     */
    function checkerActiveAt(address checker) external view returns (uint256);

    /**
     * @notice Returns the full Request record for a given requestId.
     */
    function getRequest(bytes32 requestId) external view returns (Request memory);

    /**
     * @notice Returns the Commitment record for a given requestId and checker address.
     */
    function getCommitment(bytes32 requestId, address checker) external view returns (Commitment memory);
}
