// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {IERC20} from "@oz/token/ERC20/IERC20.sol";
import {SafeERC20} from "@oz/token/ERC20/utils/SafeERC20.sol";
import {IOracle} from "@/interfaces/IOracle.sol";
import {ICheckerOracle} from "@/interfaces/ICheckerOracle.sol";

/**
 * @title Checker Oracle (WardensGame)
 * @notice Competitive bonded transaction checker oracle. A permissioned set of checker nodes
 *         races to post Approve or Deny bonds within a time-boxed voting window. The winning
 *         side's checkers share the request fee proportionally by a capital-weighted speed score.
 * @dev Implements IOracle so it can be used as a drop-in replacement for SimpleOracle with
 *      Consensus. No changes to Consensus or FROSTCoordinator are required.
 *
 *      Phase 1 covers: postRequest, commitApprove, commitDeny, finalize, claim, checker
 *      management (addChecker / removeChecker), and bond multiplier governance.
 *      Phase 2 will add: triggerArbitration, resolveDispute, slashing waterfall.
 */
contract CheckerOracle is IOracle, ICheckerOracle {
    using SafeERC20 for IERC20;

    // ============================================================
    // IMMUTABLES
    // ============================================================

    /**
     * @notice Duration of the voting window in blocks (~1 minute on Gnosis Chain at 5 s/block).
     */
    uint256 public immutable VOTING_WINDOW;

    /**
     * @notice Minimum block delay applied to checker additions and bond-multiplier updates.
     */
    uint256 public immutable GOVERNANCE_DELAY;

    /**
     * @notice ERC-20 token used for both request fees and checker bonds.
     */
    IERC20 public immutable FEE_TOKEN;

    /**
     * @notice Foundation address authorised to manage checkers and update the bond multiplier.
     */
    address public immutable ARBITRATOR;

    /**
     * @notice Fixed fee pulled from the proposer on every postRequest call.
     */
    uint256 public immutable REQUEST_FEE;

    // ============================================================
    // STORAGE
    // ============================================================

    /**
     * @notice Current bond multiplier. approveBondTarget = REQUEST_FEE × bondMultiplier.
     */
    uint256 public bondMultiplier;

    /**
     * @notice Staged bond multiplier awaiting activation (0 when none pending).
     */
    uint256 public pendingBondMultiplier;

    /**
     * @notice Block number at which pendingBondMultiplier becomes active (0 when none pending).
     */
    uint256 public pendingBondMultiplierActiveAt;

    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(address checker => uint256 activeAt) private $checkerActiveAt;

    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(bytes32 requestId => Request) private $requests;

    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(bytes32 requestId => mapping(address checker => Commitment)) private $commitments;

    /**
     * @notice Ordered list of all checkers who committed bonds for a request (both sides).
     */
    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(bytes32 requestId => address[]) private $checkerOrder;

    // ============================================================
    // ERRORS
    // ============================================================

    error NotArbitrator();
    error InvalidAddress();
    error ZeroFee();
    error InvalidMultiplier();
    error RequestAlreadyExists();
    error RequestNotFound();
    error RequestNotPending();
    error RequestNotResolved();
    error VotingWindowOpen();
    error VotingWindowClosed();
    error CheckerNotActive();
    error CheckerAlreadyScheduled();
    error CheckerNotScheduled();
    error AlreadyCommitted();
    error ThresholdAlreadyReached();
    error ZeroBond();
    error NoPendingMultiplier();
    error MultiplierNotReady();
    error NothingToClaim();
    error AlreadyClaimed();

    // ============================================================
    // MODIFIERS
    // ============================================================

    // forge-lint: disable-start(unwrapped-modifier-logic)

    /**
     * @notice Restricts functions to be callable only by the arbitrator.
     */
    modifier onlyArbitrator() {
        require(msg.sender == ARBITRATOR, NotArbitrator());
        _;
    }

    // forge-lint: disable-end(unwrapped-modifier-logic)

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    /**
     * @param arbitrator       Foundation address authorised to manage checkers and governance.
     * @param feeToken         ERC-20 token for fees and bonds.
     * @param requestFee       Fixed fee pulled from the proposer per request.
     * @param votingWindow     Voting window duration in blocks.
     * @param governanceDelay  Block delay applied to checker additions and multiplier updates.
     * @param initialMultiplier Initial bond multiplier (approveBondTarget = requestFee × multiplier).
     */
    constructor(
        address arbitrator,
        address feeToken,
        uint256 requestFee,
        uint256 votingWindow,
        uint256 governanceDelay,
        uint256 initialMultiplier
    ) {
        require(arbitrator != address(0), InvalidAddress());
        require(feeToken != address(0), InvalidAddress());
        require(requestFee > 0, ZeroFee());
        require(initialMultiplier > 0, InvalidMultiplier());
        ARBITRATOR = arbitrator;
        FEE_TOKEN = IERC20(feeToken);
        REQUEST_FEE = requestFee;
        VOTING_WINDOW = votingWindow;
        GOVERNANCE_DELAY = governanceDelay;
        bondMultiplier = initialMultiplier;
    }

    // ============================================================
    // IOracle IMPLEMENTATION
    // ============================================================

    /**
     * @inheritdoc IOracle
     * @dev Pulls REQUEST_FEE from msg.sender (the Consensus contract). The proposer must have
     *      pre-approved this contract for at least REQUEST_FEE of FEE_TOKEN before Consensus
     *      calls proposeOracleTransaction.
     * @dev Currently this contract pulls from and pushes refunds back to the Consensus contract
     *      address; the actual fee payer is the Safe owner interacting with Consensus. A future
     *      iteration may adopt a pull model where the fee payer interacts directly with this contract.
     */
    function postRequest(bytes32 requestId) external override(IOracle) {
        require($requests[requestId].proposer == address(0), RequestAlreadyExists());

        uint256 fee = REQUEST_FEE;
        uint256 bondTarget = fee * bondMultiplier;
        uint256 deadline = block.number + VOTING_WINDOW;

        $requests[requestId] = Request({
            proposer: msg.sender,
            fee: fee,
            approveBondTarget: bondTarget,
            deadline: deadline,
            state: State.PENDING,
            totalApproveBond: 0,
            totalDenyBond: 0,
            approveCheckerCount: 0,
            denyCheckerCount: 0,
            checkerCount: 0,
            totalScore: 0,
            approvedOutcome: false,
            arbitrated: false
        });

        FEE_TOKEN.safeTransferFrom(msg.sender, address(this), fee);

        emit NewRequest(requestId, msg.sender, fee, bondTarget, deadline);
    }

    // ============================================================
    // ICheckerOracle IMPLEMENTATION — VOTING
    // ============================================================

    /**
     * @inheritdoc ICheckerOracle
     */
    function commitApprove(bytes32 requestId, uint256 bondAmount) external override(ICheckerOracle) {
        _commit(requestId, true, bondAmount);
    }

    /**
     * @inheritdoc ICheckerOracle
     */
    function commitDeny(bytes32 requestId, uint256 bondAmount) external override(ICheckerOracle) {
        _commit(requestId, false, bondAmount);
    }

    // ============================================================
    // ICheckerOracle IMPLEMENTATION — FINALISATION
    // ============================================================

    /**
     * @inheritdoc ICheckerOracle
     */
    function finalize(bytes32 requestId) external override(ICheckerOracle) {
        Request storage req = $requests[requestId];
        require(req.proposer != address(0), RequestNotFound());
        require(req.state == State.PENDING, RequestNotPending());
        require(block.number > req.deadline, VotingWindowOpen());

        bool approveMet = req.totalApproveBond >= req.approveBondTarget;
        bool denyMet = req.totalDenyBond >= req.approveBondTarget;

        if (approveMet && denyMet) {
            // Conflict detected — freeze for Phase 2 arbitration. No OracleResult yet.
            req.state = State.FROZEN;
            return;
        }

        req.state = State.RESOLVED;

        if (approveMet) {
            req.approvedOutcome = true;
            req.checkerCount = req.approveCheckerCount;
            req.totalScore = _computeTotalScore(requestId, true, req.approveCheckerCount);
            emit Resolved(requestId, true, ResolveReason.UNANIMOUS_APPROVE);
            emit OracleResult(requestId, req.proposer, abi.encode(ResolveReason.UNANIMOUS_APPROVE), true);
        } else if (denyMet) {
            req.approvedOutcome = false;
            req.checkerCount = req.denyCheckerCount;
            req.totalScore = _computeTotalScore(requestId, false, req.denyCheckerCount);
            emit Resolved(requestId, false, ResolveReason.UNANIMOUS_DENY);
            emit OracleResult(requestId, req.proposer, abi.encode(ResolveReason.UNANIMOUS_DENY), false);
        } else {
            // Timeout / undercapitalised — refund fee to proposer, reject by default.
            req.approvedOutcome = false;
            uint256 fee = req.fee;
            req.fee = 0;
            FEE_TOKEN.safeTransfer(req.proposer, fee);
            emit Resolved(requestId, false, ResolveReason.TIMEOUT);
            emit OracleResult(requestId, req.proposer, abi.encode(ResolveReason.TIMEOUT), false);
        }
    }

    /**
     * @inheritdoc ICheckerOracle
     */
    function claim(bytes32 requestId) external override(ICheckerOracle) {
        Request storage req = $requests[requestId];
        require(req.state == State.RESOLVED, RequestNotResolved());

        Commitment storage c = $commitments[requestId][msg.sender];
        require(c.bondAmount > 0, NothingToClaim());
        require(!c.claimed, AlreadyClaimed());
        c.claimed = true;

        // checkerCount is set only on unanimous outcomes; zero means the request timed out.
        bool isTimeout = req.checkerCount == 0;

        // On timeout all bonds are returned (no side was definitively wrong).
        // On a unanimous outcome the losing side forfeits their bond.
        if (!isTimeout && c.approved != req.approvedOutcome) {
            emit Claimed(requestId, msg.sender, 0, 0);
            return;
        }

        uint256 bondReturn = c.bondAmount;
        uint256 feeReward = 0;

        if (!isTimeout) {
            uint256 score = c.bondAmount * (req.checkerCount + 1 - c.position);
            feeReward = req.fee * score / req.totalScore;
        }

        FEE_TOKEN.safeTransfer(msg.sender, bondReturn + feeReward);
        emit Claimed(requestId, msg.sender, bondReturn, feeReward);
    }

    // ============================================================
    // ICheckerOracle IMPLEMENTATION — CHECKER MANAGEMENT
    // ============================================================

    /**
     * @inheritdoc ICheckerOracle
     */
    function addChecker(address checker) external override(ICheckerOracle) onlyArbitrator {
        require(checker != address(0), InvalidAddress());
        require($checkerActiveAt[checker] == 0, CheckerAlreadyScheduled());

        uint256 activeAt = block.number + GOVERNANCE_DELAY;
        $checkerActiveAt[checker] = activeAt;
        emit CheckerScheduled(checker, activeAt);
    }

    /**
     * @inheritdoc ICheckerOracle
     */
    function removeChecker(address checker) external override(ICheckerOracle) onlyArbitrator {
        require($checkerActiveAt[checker] != 0, CheckerNotScheduled());

        delete $checkerActiveAt[checker];
        emit CheckerRemoved(checker);
    }

    // ============================================================
    // ICheckerOracle IMPLEMENTATION — BOND MULTIPLIER GOVERNANCE
    // ============================================================

    /**
     * @inheritdoc ICheckerOracle
     */
    function scheduleBondMultiplier(uint256 newValue) external override(ICheckerOracle) onlyArbitrator {
        require(newValue > 0, InvalidMultiplier());

        uint256 activeAt = block.number + GOVERNANCE_DELAY;
        pendingBondMultiplier = newValue;
        pendingBondMultiplierActiveAt = activeAt;
        emit BondMultiplierScheduled(newValue, activeAt);
    }

    /**
     * @inheritdoc ICheckerOracle
     */
    function applyBondMultiplier() external override(ICheckerOracle) {
        require(pendingBondMultiplierActiveAt != 0, NoPendingMultiplier());
        require(block.number >= pendingBondMultiplierActiveAt, MultiplierNotReady());

        uint256 newValue = pendingBondMultiplier;
        bondMultiplier = newValue;
        pendingBondMultiplier = 0;
        pendingBondMultiplierActiveAt = 0;
        emit BondMultiplierApplied(newValue);
    }

    // ============================================================
    // ICheckerOracle IMPLEMENTATION — VIEW FUNCTIONS
    // ============================================================

    /**
     * @inheritdoc ICheckerOracle
     */
    function checkerActiveAt(address checker) external view override(ICheckerOracle) returns (uint256) {
        return $checkerActiveAt[checker];
    }

    /**
     * @inheritdoc ICheckerOracle
     */
    function getRequest(bytes32 requestId) external view override(ICheckerOracle) returns (Request memory) {
        return $requests[requestId];
    }

    /**
     * @inheritdoc ICheckerOracle
     */
    function getCommitment(bytes32 requestId, address checker)
        external
        view
        override(ICheckerOracle)
        returns (Commitment memory)
    {
        return $commitments[requestId][checker];
    }

    // ============================================================
    // INTERNAL HELPERS
    // ============================================================

    /**
     * @dev Core commitment logic shared by commitApprove and commitDeny.
     *      Pulls the effective bond (min of bondAmount and the remaining threshold gap)
     *      from the caller and records the commitment.
     */
    function _commit(bytes32 requestId, bool approve, uint256 bondAmount) internal {
        require(bondAmount > 0, ZeroBond());
        require(_isActiveChecker(msg.sender), CheckerNotActive());

        Request storage req = $requests[requestId];
        require(req.proposer != address(0), RequestNotFound());
        require(req.state == State.PENDING, RequestNotPending());
        require(block.number <= req.deadline, VotingWindowClosed());
        require($commitments[requestId][msg.sender].bondAmount == 0, AlreadyCommitted());

        uint256 totalBond = approve ? req.totalApproveBond : req.totalDenyBond;
        require(totalBond < req.approveBondTarget, ThresholdAlreadyReached());

        uint256 remaining = req.approveBondTarget - totalBond;
        uint256 effectiveBond = bondAmount < remaining ? bondAmount : remaining;

        uint256 position;
        if (approve) {
            req.approveCheckerCount += 1;
            position = req.approveCheckerCount;
            req.totalApproveBond += effectiveBond;
        } else {
            req.denyCheckerCount += 1;
            position = req.denyCheckerCount;
            req.totalDenyBond += effectiveBond;
        }

        $commitments[requestId][msg.sender] =
            Commitment({approved: approve, bondAmount: effectiveBond, position: position, claimed: false});
        $checkerOrder[requestId].push(msg.sender);

        FEE_TOKEN.safeTransferFrom(msg.sender, address(this), effectiveBond);

        emit Committed(requestId, msg.sender, approve, effectiveBond, position);
    }

    /**
     * @dev Returns true if the checker is in the permissioned set and their activation
     *      block has been reached.
     */
    function _isActiveChecker(address checker) internal view returns (bool) {
        uint256 activeAt = $checkerActiveAt[checker];
        return activeAt != 0 && block.number >= activeAt;
    }

    /**
     * @dev Iterates over all commitments for the request and computes the total score for
     *      the given side. Used once during finalize() to cache Request.totalScore.
     *
     *      Score_i    = effectiveBond_i × (winnerCount + 1 - position_i)
     *      TotalScore = Σ Score_i
     */
    function _computeTotalScore(bytes32 requestId, bool approve, uint256 winnerCount)
        internal
        view
        returns (uint256 totalScore)
    {
        address[] storage order = $checkerOrder[requestId];
        uint256 len = order.length;
        for (uint256 i = 0; i < len; i++) {
            Commitment storage c = $commitments[requestId][order[i]];
            if (c.approved == approve) {
                totalScore += c.bondAmount * (winnerCount + 1 - c.position);
            }
        }
    }
}
