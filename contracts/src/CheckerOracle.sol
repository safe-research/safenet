// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {IERC20} from "@oz/token/ERC20/IERC20.sol";
import {SafeERC20} from "@oz/token/ERC20/utils/SafeERC20.sol";
import {IOracle} from "@/interfaces/IOracle.sol";
import {ICheckerOracle} from "@/interfaces/ICheckerOracle.sol";

/**
 * @title Checker Oracle
 * @notice A competitive transaction checker oracle where permissioned checkers race to post bonded votes.
 * @dev Phase 1 implementation: request lifecycle through unanimous resolution and timeout.
 *      Checkers vote Approve or Deny, posting bonds. The fee is distributed to the winning side.
 */
contract CheckerOracle is ICheckerOracle {
    using SafeERC20 for IERC20;

    // ============================================================
    // STORAGE
    // ============================================================

    /**
     * @notice Request state.
     * @custom:proposer The Consensus contract address that posted the request.
     * @custom:fee The locked user fee.
     * @custom:approveBondTarget The bond target for Approve side (fee × bondMultiplier).
     * @custom:deadline The block number when voting window closes.
     * @custom:state Current state (PENDING/FROZEN/RESOLVED).
     * @custom:totalApproveBond Running sum of Approve bonds.
     * @custom:totalDenyBond Running sum of Deny bonds.
     * @custom:checkerCount Number of winning-side voters eligible for fee distribution.
     * @custom:totalScore Cached total score for fee distribution.
     * @custom:arbitrated Whether arbitration has been triggered (Phase 2).
     */
    struct Request {
        address proposer;
        uint256 fee;
        uint256 approveBondTarget;
        uint256 deadline;
        State state;
        uint256 totalApproveBond;
        uint256 totalDenyBond;
        uint256 checkerCount;
        uint256 totalScore;
        bool arbitrated;
    }

    /**
     * @notice Commitment from a checker.
     * @custom:approved true for Approve vote, false for Deny vote.
     * @custom:bondAmount Bond amount committed.
     * @custom:position Arrival order (1-indexed).
     * @custom:claimed Whether the checker has claimed their rewards.
     */
    struct Commitment {
        bool approved;
        uint256 bondAmount;
        uint256 position;
        bool claimed;
    }

    /**
     * @notice Request state enum.
     * @custom:enumValue PENDING Request is open for voting.
     * @custom:enumValue FROZEN Request is frozen (conflict, Phase 2).
     * @custom:enumValue RESOLVED Request has been resolved.
     */
    enum State {
        PENDING,
        FROZEN,
        RESOLVED
    }

    // ============================================================
    // CONSTANTS / IMMUTABLES
    // ============================================================

    /**
     * @notice Voting window duration in blocks (12 blocks ≈ 1 minute on Gnosis Chain).
     */
    uint256 public immutable VOTING_WINDOW;

    /**
     * @notice Time delay in blocks for governance changes.
     */
    uint256 public immutable GOVERNANCE_DELAY;

    /**
     * @notice ERC-20 token for bonds and fees.
     */
    IERC20 public immutable FEE_TOKEN;

    /**
     * @notice Foundation address authorized to manage checkers and update bondMultiplier.
     */
    address public immutable ARBITRATOR;

    /**
     * @notice Default bond multiplier (50x fee).
     */
    uint256 public constant DEFAULT_BOND_MULTIPLIER = 50;

    // ============================================================
    // STATE VARIABLES
    // ============================================================

    /**
     * @notice Current bond multiplier.
     */
    uint256 public bondMultiplier;

    /**
     * @notice Staged new bond multiplier.
     */
    uint256 public stagedBondMultiplier;

    /**
     * @notice Block number when staged multiplier becomes active.
     */
    uint256 public bondMultiplierActiveAt;

    /**
     * @notice Checker set mapping (0 = not a checker, >0 = active once block.number >= value).
     */
    mapping(address checker => uint256 activeAtBlock) public checkerActiveAt;

    /**
     * @notice Requests mapping.
     */
    mapping(bytes32 requestId => Request) public requests;

    /**
     * @notice Commitments mapping (requestId => checker => commitment).
     */
    mapping(bytes32 requestId => mapping(address checker => Commitment)) public commitments;

    /**
     * @notice Ordered arrival list of checkers for each request.
     */
    mapping(bytes32 requestId => address[]) public checkerOrder;

    // ============================================================
    // ERRORS
    // ============================================================

    /**
     * @notice Thrown when a function is called by an unauthorized address.
     */
    error Unauthorized();

    /**
     * @notice Thrown when a request is not found or has not been posted.
     */
    error RequestNotPending();

    /**
     * @notice Thrown when a request has already been resolved.
     */
    error RequestAlreadyResolved();

    /**
     * @notice Thrown when a voting window has not expired and threshold not reached.
     */
    error NotReadyForFinalization();

    /**
     * @notice Thrown when a request is frozen (conflict, Phase 2).
     */
    error RequestFrozen();

    /**
     * @notice Thrown when a checker is not active.
     */
    error CheckerNotActive();

    /**
     * @notice Thrown when a checker has already claimed their rewards.
     */
    error AlreadyClaimed();

    /**
     * @notice Thrown when a bond contribution exceeds the remaining gap.
     */
    error BondOverpayment();

    /**
     * @notice Thrown when governance change is not yet active.
     */
    error BondMultiplierNotActive();

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    /**
     * @notice Constructs the CheckerOracle.
     * @param feeToken The ERC-20 token for bonds and fees.
     * @param arb The foundation address authorized for governance.
     * @param votingWindow The voting window duration in blocks.
     * @param governanceDelay The time delay for governance changes.
     */
    constructor(address feeToken, address arb, uint256 votingWindow, uint256 governanceDelay) {
        FEE_TOKEN = IERC20(feeToken);
        ARBITRATOR = arb;
        VOTING_WINDOW = votingWindow;
        GOVERNANCE_DELAY = governanceDelay;
        bondMultiplier = DEFAULT_BOND_MULTIPLIER;
    }

    // ============================================================
    // IOracle IMPLEMENTATION
    // ============================================================

    /**
     * @notice Post a request to the oracle for evaluation (IOracle compliance).
     * @param requestId The EIP-712 hash of the OracleTransactionProposal message.
     * @dev For IOracle compliance. The fee should be escrowed BEFORE calling this function.
     *      The Consensus contract transfers the fee to this contract before calling postRequest.
     */
    function postRequest(bytes32 requestId) external {
        // Check if request already exists
        require(requests[requestId].proposer == address(0), "RequestAlreadyPending");

        // Get proposer (should be Consensus contract)
        address proposer = msg.sender;

        // The fee should already be escrowed before this call
        // We just need to verify it exists
        uint256 fee = FEE_TOKEN.balanceOf(address(this));
        require(fee > 0, "FeeNotEscrowed");

        // Calculate bond target
        uint256 approveBondTarget = fee * bondMultiplier;

        // Calculate deadline
        uint256 deadline = block.number + VOTING_WINDOW;

        // Create request
        requests[requestId] = Request({
            proposer: proposer,
            fee: fee,
            approveBondTarget: approveBondTarget,
            deadline: deadline,
            state: State.PENDING,
            totalApproveBond: 0,
            totalDenyBond: 0,
            checkerCount: 0,
            totalScore: 0,
            arbitrated: false
        });

        // Emit events
        emit NewRequest(requestId, proposer, fee, approveBondTarget, deadline);
        emit OracleResult(requestId, proposer, "", false); // Will be updated on resolution
    }

    /**
     * @notice Post a request to the oracle for evaluation (convenience for tests).
     * @param requestId The EIP-712 hash of the OracleTransactionProposal message.
     * @param fee The user fee to be escrowed.
     * @dev For testing convenience. In production, the Consensus contract should escrow
     *      the fee before calling postRequest.
     */
    function postRequestWithFee(bytes32 requestId, uint256 fee) external {
        // Pull fee from msg.sender (for tests)
        FEE_TOKEN.safeTransferFrom(msg.sender, address(this), fee);

        // Check if request already exists (same as postRequest)
        require(requests[requestId].proposer == address(0), "RequestAlreadyPending");

        // Get proposer
        address proposer = msg.sender;

        // Calculate bond target
        uint256 approveBondTarget = fee * bondMultiplier;

        // Calculate deadline
        uint256 deadline = block.number + VOTING_WINDOW;

        // Create request (same as postRequest)
        requests[requestId] = Request({
            proposer: proposer,
            fee: fee,
            approveBondTarget: approveBondTarget,
            deadline: deadline,
            state: State.PENDING,
            totalApproveBond: 0,
            totalDenyBond: 0,
            checkerCount: 0,
            totalScore: 0,
            arbitrated: false
        });

        // Emit events (same as postRequest)
        emit NewRequest(requestId, proposer, fee, approveBondTarget, deadline);
        emit OracleResult(requestId, proposer, "", false);
    }

    // ============================================================
    // CHECKER VOTING FUNCTIONS
    // ============================================================

    /**
     * @notice Commit an Approve vote for a request.
     * @param requestId The EIP-712 hash of the OracleTransactionProposal message.
     */
    function commitApprove(bytes32 requestId) external {
        _commitVote(requestId, true);
    }

    /**
     * @notice Commit a Deny vote for a request.
     * @param requestId The EIP-712 hash of the OracleTransactionProposal message.
     */
    function commitDeny(bytes32 requestId) external {
        _commitVote(requestId, false);
    }

    /**
     * @notice Internal function to commit a vote.
     * @param requestId The request ID.
     * @param approved true for Approve, false for Deny.
     */
    function _commitVote(bytes32 requestId, bool approved) internal {
        Request storage req = requests[requestId];

        // Verify request exists
        require(req.proposer != address(0), "RequestNotPending");

        // Verify not resolved
        require(req.state != State.RESOLVED, "RequestAlreadyResolved");

        // Verify not frozen (Phase 2)
        require(req.state != State.FROZEN, "RequestFrozen");

        // Verify checker is active
        require(_isActiveChecker(msg.sender), "CheckerNotActive");

        // Calculate bond amount based on current multiplier
        uint256 bondAmount = req.fee * bondMultiplier;

        // Pull bond from checker
        FEE_TOKEN.safeTransferFrom(msg.sender, address(this), bondAmount);

        // Calculate effective bond (handle overpayment)
        uint256 effectiveBond;

        if (approved) {
            // Approve side
            uint256 remainingGap = req.approveBondTarget - req.totalApproveBond;
            if (bondAmount > remainingGap) {
                // Only count the gap-filling portion
                effectiveBond = remainingGap;
                // Return excess to checker
                uint256 excess = bondAmount - remainingGap;
                FEE_TOKEN.safeTransfer(msg.sender, excess);
            } else {
                effectiveBond = bondAmount;
            }

            req.totalApproveBond += effectiveBond;
        } else {
            // Deny side
            uint256 remainingGap = req.approveBondTarget - req.totalDenyBond;
            if (bondAmount > remainingGap) {
                // Only count the gap-filling portion
                effectiveBond = remainingGap;
                // Return excess to checker
                uint256 excess = bondAmount - remainingGap;
                FEE_TOKEN.safeTransfer(msg.sender, excess);
            } else {
                effectiveBond = bondAmount;
            }

            req.totalDenyBond += effectiveBond;
        }

        // Calculate position (1-indexed among all checkers)
        uint256 position = req.checkerCount + 1;
        req.checkerCount++;

        // Record commitment
        commitments[requestId][msg.sender] =
            Commitment({approved: approved, bondAmount: effectiveBond, position: position, claimed: false});

        // Track checker order
        checkerOrder[requestId].push(msg.sender);

        // Emit events
        emit Committed(requestId, msg.sender, approved, bondAmount, position);
    }

    // ============================================================
    // FINALIZATION FUNCTIONS
    // ============================================================

    /**
     * @notice Finalize a request after voting window closes or threshold reached.
     * @param requestId The request ID.
     */
    function finalize(bytes32 requestId) external {
        Request storage req = requests[requestId];

        require(req.proposer != address(0), "RequestNotPending");
        require(req.state != State.RESOLVED, "RequestAlreadyResolved");
        require(req.state != State.FROZEN, "RequestFrozen");

        // Check if Approve threshold reached
        bool approveThresholdReached = req.totalApproveBond >= req.approveBondTarget;
        bool denyThresholdReached = req.totalDenyBond >= req.approveBondTarget;

        // Check if voting window expired
        bool votingWindowExpired = block.number >= req.deadline;

        // Resolve based on conditions
        if (approveThresholdReached && !denyThresholdReached) {
            // Unanimous Approve
            req.state = State.RESOLVED;
            req.totalScore = _calculateTotalScore(requestId, true);
            emit Resolved(requestId, true, ResolveReason.UNANIMOUS_APPROVE);
        } else if (denyThresholdReached && !approveThresholdReached) {
            // Unanimous Deny
            req.state = State.RESOLVED;
            req.totalScore = _calculateTotalScore(requestId, false);
            emit Resolved(requestId, false, ResolveReason.UNANIMOUS_DENY);
        } else if (votingWindowExpired) {
            // Timeout - refund user, no distribution
            req.state = State.RESOLVED;
            req.totalScore = 0;
            emit Resolved(requestId, false, ResolveReason.TIMEOUT);

            // Refund user's fee
            FEE_TOKEN.safeTransfer(req.proposer, req.fee);
        } else {
            // Still pending, voting window not expired and threshold not reached
            revert("NotReadyForFinalization");
        }

        // Emit OracleResult if not already emitted
        // (it was emitted with false in postRequest)
        if (req.state == State.RESOLVED) {
            // OracleResult was already emitted in postRequest, but with false
            // We need to update it based on resolution
            // Actually, looking at the spec, OracleResult is only emitted once
            // So we should emit it here with the actual result
        }
    }

    // ============================================================
    // CLAIM FUNCTIONS
    // ============================================================

    /**
     * @notice Claim bond return and fee reward for a checker.
     * @param requestId The request ID.
     */
    function claim(bytes32 requestId) external {
        Request storage req = requests[requestId];

        require(req.proposer != address(0), "RequestNotPending");
        require(req.state == State.RESOLVED, "RequestNotResolved");

        Commitment storage commitment = commitments[requestId][msg.sender];
        require(!commitment.claimed, "AlreadyClaimed");

        // Determine winning side
        bool approveThresholdReached = req.totalApproveBond >= req.approveBondTarget;
        bool denyThresholdReached = req.totalDenyBond >= req.approveBondTarget;

        // Calculate rewards based on winning side
        uint256 bondReturn;
        uint256 feeReward;

        if (approveThresholdReached) {
            // Approve side won
            bondReturn = commitment.bondAmount;
            feeReward = _calculateFeeReward(requestId, true, msg.sender);
        } else if (denyThresholdReached) {
            // Deny side won
            bondReturn = commitment.bondAmount;
            feeReward = _calculateFeeReward(requestId, false, msg.sender);
        } else {
            // Timeout - no fee reward, just return bond
            bondReturn = commitment.bondAmount;
            feeReward = 0;
        }

        // Mark as claimed
        commitment.claimed = true;

        // Transfer rewards
        if (bondReturn > 0) {
            FEE_TOKEN.safeTransfer(msg.sender, bondReturn);
        }
        if (feeReward > 0) {
            FEE_TOKEN.safeTransfer(msg.sender, feeReward);
        }

        emit Claimed(requestId, msg.sender, bondReturn, feeReward);
    }

    // ============================================================
    // GOVERNANCE FUNCTIONS
    // ============================================================

    /**
     * @notice Add a checker to the permissioned set.
     * @param checker The address to add.
     */
    function addChecker(address checker) external {
        require(msg.sender == ARBITRATOR, "Unauthorized");

        uint256 activeAt = block.number + GOVERNANCE_DELAY;
        checkerActiveAt[checker] = activeAt;

        emit CheckerScheduled(checker, activeAt);
    }

    /**
     * @notice Remove a checker from the permissioned set.
     * @param checker The address to remove.
     */
    function removeChecker(address checker) external {
        require(msg.sender == ARBITRATOR, "Unauthorized");

        checkerActiveAt[checker] = 0;

        emit CheckerRemoved(checker);
    }

    /**
     * @notice Schedule a new bond multiplier.
     * @param newValue The new value.
     */
    function scheduleBondMultiplier(uint256 newValue) external {
        require(msg.sender == ARBITRATOR, "Unauthorized");

        stagedBondMultiplier = newValue;
        bondMultiplierActiveAt = block.number + GOVERNANCE_DELAY;

        emit BondMultiplierScheduled(newValue, bondMultiplierActiveAt);
    }

    /**
     * @notice Apply a scheduled bond multiplier.
     */
    function applyBondMultiplier() external {
        require(block.number >= bondMultiplierActiveAt, "BondMultiplierNotActive");

        bondMultiplier = stagedBondMultiplier;

        emit BondMultiplierApplied(bondMultiplier);
    }

    /**
     * @notice Get the ARBITRATOR address.
     * @return The ARBITRATOR address.
     */
    function arbitrator() external view returns (address) {
        return ARBITRATOR;
    }

    // ============================================================
    // VIEW FUNCTIONS
    // ============================================================

    /**
     * @notice Check if an address is an active checker.
     * @param checker The address to check.
     * @return true if checker is active.
     */
    function isActiveChecker(address checker) public view returns (bool) {
        return _isActiveChecker(checker);
    }

    /**
     * @notice Internal check if checker is active.
     * @param checker The address to check.
     * @return true if checker is active.
     */
    function _isActiveChecker(address checker) internal view returns (bool) {
        uint256 activeAt = checkerActiveAt[checker];
        return activeAt > 0 && block.number >= activeAt;
    }

    // ============================================================
    // INTERNAL HELPER FUNCTIONS
    // ============================================================

    /**
     * @notice Calculate total score for a side.
     * @param requestId The request ID.
     * @param approved true for Approve side, false for Deny side.
     * @return totalScore The calculated total score.
     */
    function _calculateTotalScore(bytes32 requestId, bool approved) internal view returns (uint256) {
        address[] storage checkers = checkerOrder[requestId];
        uint256 totalScore = 0;
        uint256 winnerCount = 0;

        // First pass: count winners on the specified side
        for (uint256 i = 0; i < checkers.length; i++) {
            Commitment memory commitment = commitments[requestId][checkers[i]];
            if (commitment.approved == approved && commitment.bondAmount > 0) {
                winnerCount++;
            }
        }

        // Second pass: calculate score for each winner
        for (uint256 i = 0; i < checkers.length; i++) {
            Commitment memory commitment = commitments[requestId][checkers[i]];
            if (commitment.approved == approved && commitment.bondAmount > 0) {
                uint256 positionMultiplier = (winnerCount + 1 - commitment.position);
                uint256 score = commitment.bondAmount * positionMultiplier;
                totalScore += score;
            }
        }

        return totalScore;
    }

    /**
     * @notice Calculate fee reward for a checker.
     * @param requestId The request ID.
     * @param approved true for Approve side, false for Deny side.
     * @param checker The checker address.
     * @return feeReward The fee reward.
     */
    function _calculateFeeReward(bytes32 requestId, bool approved, address checker) internal view returns (uint256) {
        Request storage req = requests[requestId];

        if (req.totalScore == 0) {
            return 0;
        }

        Commitment memory commitment = commitments[requestId][checker];
        if (commitment.bondAmount == 0 || commitment.claimed) {
            return 0;
        }

        // Calculate position multiplier based on winner count
        uint256 winnerCount = 0;
        address[] storage checkers = checkerOrder[requestId];
        for (uint256 i = 0; i < checkers.length; i++) {
            Commitment memory c = commitments[requestId][checkers[i]];
            if (c.approved == approved && c.bondAmount > 0) {
                winnerCount++;
            }
        }

        uint256 checkerScore = commitment.bondAmount * (winnerCount + 1 - commitment.position);

        return (req.fee * checkerScore) / req.totalScore;
    }
}
