// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {IERC20} from "@oz/token/ERC20/IERC20.sol";
import {SafeERC20} from "@oz/token/ERC20/utils/SafeERC20.sol";
import {IOracle} from "@/interfaces/IOracle.sol";
import {BondMultiplierGovernance} from "@/BondMultiplierGovernance.sol";

contract CheckerOracle is IOracle, BondMultiplierGovernance {
    using SafeERC20 for IERC20;

    // ============================================================
    // ENUMS
    // ============================================================

    enum State {
        PENDING,
        FROZEN,
        RESOLVED_APPROVED,
        RESOLVED_DENIED,
        TIMED_OUT
    }

    enum ResolveReason {
        UNANIMOUS_APPROVE,
        UNANIMOUS_DENY,
        TIMEOUT,
        ARBITRATION
    }

    // ============================================================
    // STRUCTS
    // ============================================================

    struct Request {
        address proposer;
        uint256 fee;
        uint256 approveBondTarget;
        uint256 deadline;
        State state;
        uint256 totalApproveBond;
        uint256 totalDenyBond;
        uint256 approveSentinelCount;
        uint256 denySentinelCount;
        uint256 approveTotalScore;
        uint256 denyTotalScore;
        bool arbitrated;
    }

    struct Commitment {
        bool approved;
        uint256 bondAmount;
        uint256 position;
        bool claimed;
    }

    // ============================================================
    // EVENTS
    // ============================================================

    event NewRequest(
        bytes32 indexed requestId, address indexed proposer, uint256 fee, uint256 approveBondTarget, uint256 deadline
    );
    event Committed(
        bytes32 indexed requestId, address indexed sentinel, bool approved, uint256 bondAmount, uint256 position
    );
    event Resolved(bytes32 indexed requestId, bool approved, ResolveReason reason);
    event ArbitrationTriggered(bytes32 indexed requestId);
    event DisputeResolved(bytes32 indexed requestId, address winner, address loser, uint256 slashed);
    event Claimed(bytes32 indexed requestId, address indexed sentinel, uint256 bondReturn, uint256 feeReward);

    // ============================================================
    // IMMUTABLES
    // ============================================================

    address public immutable ARBITRATOR;
    IERC20 public immutable FEE_TOKEN;
    uint256 public immutable REQUEST_FEE;
    uint256 public immutable VOTING_WINDOW;

    // ============================================================
    // STORAGE
    // ============================================================

    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(bytes32 requestId => Request) private $requests;

    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(bytes32 requestId => mapping(address sentinel => Commitment)) private $commitments;

    // ============================================================
    // ERRORS
    // ============================================================

    error NotArbitrator();
    error ZeroFee();
    error RequestAlreadyExists();
    error RequestNotFound();
    error RequestNotPending();
    error RequestNotResolved();
    error VotingWindowOpen();
    error VotingWindowClosed();
    error AlreadyCommitted();
    error ThresholdAlreadyReached();
    error ZeroBond();
    error NothingToClaim();
    error AlreadyClaimed();

    // ============================================================
    // MODIFIERS
    // ============================================================

    // forge-lint: disable-start(unwrapped-modifier-logic)

    modifier onlyArbitrator() {
        require(msg.sender == ARBITRATOR, NotArbitrator());
        _;
    }

    // forge-lint: disable-end(unwrapped-modifier-logic)

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    constructor(
        address arbitrator,
        address feeToken,
        uint256 requestFee,
        uint256 votingWindow,
        uint256 governanceDelay,
        uint256 initialMultiplier
    ) BondMultiplierGovernance(governanceDelay, initialMultiplier) {
        require(arbitrator != address(0), InvalidAddress());
        require(feeToken != address(0), InvalidAddress());
        require(requestFee > 0, ZeroFee());
        ARBITRATOR = arbitrator;
        FEE_TOKEN = IERC20(feeToken);
        REQUEST_FEE = requestFee;
        VOTING_WINDOW = votingWindow;
    }

    // ============================================================
    // IOracle IMPLEMENTATION
    // ============================================================

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
            approveSentinelCount: 0,
            denySentinelCount: 0,
            approveTotalScore: 0,
            denyTotalScore: 0,
            arbitrated: false
        });

        FEE_TOKEN.safeTransferFrom(msg.sender, address(this), fee);

        emit NewRequest(requestId, msg.sender, fee, bondTarget, deadline);
    }

    // ============================================================
    // VOTING
    // ============================================================

    function commitApprove(bytes32 requestId, uint256 bondAmount) external {
        _commit(requestId, true, bondAmount);
    }

    function commitDeny(bytes32 requestId, uint256 bondAmount) external {
        _commit(requestId, false, bondAmount);
    }

    // ============================================================
    // FINALISATION
    // ============================================================

    function finalize(bytes32 requestId) external {
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

        if (approveMet) {
            req.state = State.RESOLVED_APPROVED;
            emit Resolved(requestId, true, ResolveReason.UNANIMOUS_APPROVE);
            emit OracleResult(requestId, req.proposer, abi.encode(ResolveReason.UNANIMOUS_APPROVE), true);
        } else if (denyMet) {
            req.state = State.RESOLVED_DENIED;
            emit Resolved(requestId, false, ResolveReason.UNANIMOUS_DENY);
            emit OracleResult(requestId, req.proposer, abi.encode(ResolveReason.UNANIMOUS_DENY), false);
        } else {
            // Timeout / undercapitalised — refund fee to proposer, reject by default.
            req.state = State.TIMED_OUT;
            uint256 fee = req.fee;
            req.fee = 0;
            FEE_TOKEN.safeTransfer(req.proposer, fee);
            emit Resolved(requestId, false, ResolveReason.TIMEOUT);
            emit OracleResult(requestId, req.proposer, abi.encode(ResolveReason.TIMEOUT), false);
        }
    }

    function claim(bytes32 requestId) external {
        Request storage req = $requests[requestId];
        State state = req.state;
        require(
            state == State.RESOLVED_APPROVED || state == State.RESOLVED_DENIED || state == State.TIMED_OUT,
            RequestNotResolved()
        );

        Commitment storage c = $commitments[requestId][msg.sender];
        require(c.bondAmount > 0, NothingToClaim());
        require(!c.claimed, AlreadyClaimed());
        c.claimed = true;

        bool isTimeout = state == State.TIMED_OUT;

        // Bonds are only slashed through Phase 2 arbitration; losers simply earn no fee reward.
        uint256 bondReturn = c.bondAmount;
        uint256 feeReward = 0;

        bool isWinner = !isTimeout && c.approved == (state == State.RESOLVED_APPROVED);
        if (isWinner) {
            uint256 score = c.bondAmount / c.position;
            uint256 totalScore = state == State.RESOLVED_APPROVED ? req.approveTotalScore : req.denyTotalScore;
            feeReward = req.fee * score / totalScore;
        }

        FEE_TOKEN.safeTransfer(msg.sender, bondReturn + feeReward);
        emit Claimed(requestId, msg.sender, bondReturn, feeReward);
    }

    // ============================================================
    // GOVERNANCE
    // ============================================================

    function addSentinel(address sentinel) external onlyArbitrator {
        _addSentinel(sentinel);
    }

    function removeSentinel(address sentinel) external onlyArbitrator {
        _removeSentinel(sentinel);
    }

    function scheduleBondMultiplier(uint256 newValue) external onlyArbitrator {
        _scheduleBondMultiplier(newValue);
    }

    // ============================================================
    // VIEW FUNCTIONS
    // ============================================================

    function getRequest(bytes32 requestId) external view returns (Request memory) {
        return $requests[requestId];
    }

    function getCommitment(bytes32 requestId, address sentinel) external view returns (Commitment memory) {
        return $commitments[requestId][sentinel];
    }

    // ============================================================
    // INTERNAL HELPERS
    // ============================================================

    function _commit(bytes32 requestId, bool approve, uint256 bondAmount) internal {
        require(bondAmount > 0, ZeroBond());
        require(_isActiveSentinel(msg.sender), SentinelNotActive());

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
            req.approveSentinelCount += 1;
            position = req.approveSentinelCount;
            req.totalApproveBond += effectiveBond;
            req.approveTotalScore += effectiveBond / position;
        } else {
            req.denySentinelCount += 1;
            position = req.denySentinelCount;
            req.totalDenyBond += effectiveBond;
            req.denyTotalScore += effectiveBond / position;
        }

        $commitments[requestId][msg.sender] =
            Commitment({approved: approve, bondAmount: effectiveBond, position: position, claimed: false});

        FEE_TOKEN.safeTransferFrom(msg.sender, address(this), effectiveBond);

        emit Committed(requestId, msg.sender, approve, effectiveBond, position);
    }
}
