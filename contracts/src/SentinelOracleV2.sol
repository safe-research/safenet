// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {IERC20} from "@oz/token/ERC20/IERC20.sol";
import {SafeERC20} from "@oz/token/ERC20/utils/SafeERC20.sol";
import {IOracle} from "@/interfaces/IOracle.sol";
import {BondConfig} from "@/libraries/BondConfig.sol";
import {SentinelMap} from "@/libraries/SentinelMap.sol";
import {SentinelOracleCommitment, SentinelOracleCommitmentMap} from "@/libraries/SentinelOracleCommitmentsV2.sol";
import {SentinelOracleRequest, SentinelOracleRequestMap} from "@/libraries/SentinelOracleRequestsV2.sol";

// TODO(A4): rename to SentinelOracle.sol once SentinelOracle.t.sol is rewritten against this
// interface and the V1 contract/libraries are removed.
contract SentinelOracleV2 is IOracle {
    using BondConfig for BondConfig.T;
    using SentinelMap for SentinelMap.T;
    using SentinelOracleCommitment for SentinelOracleCommitment.Commitment;
    using SentinelOracleCommitmentMap for SentinelOracleCommitmentMap.T;
    using SentinelOracleRequest for SentinelOracleRequest.Request;
    using SentinelOracleRequestMap for SentinelOracleRequestMap.T;
    using SafeERC20 for IERC20;

    // ============================================================
    // EVENTS
    // ============================================================

    event DisputeResolved(bytes32 indexed requestId, SentinelOracleRequest.State outcome, uint256 slashed);
    event Claimed(bytes32 indexed requestId, address indexed sentinel, uint256 bondReturn, uint256 feeReward);

    // ============================================================
    // IMMUTABLES
    // ============================================================

    address public immutable ARBITRATOR;
    address public immutable CONSENSUS;
    IERC20 public immutable FEE_TOKEN;
    uint256 public immutable REQUEST_FEE;
    uint256 public immutable COMMIT_WINDOW;
    uint256 public immutable REVEAL_WINDOW;
    uint256 public immutable GOVERNANCE_DELAY;

    // ============================================================
    // STORAGE
    // ============================================================

    // forge-lint: disable-next-line(mixed-case-variable)
    BondConfig.T private $bondConfig;

    // forge-lint: disable-next-line(mixed-case-variable)
    SentinelMap.T private $sentinelMap;

    // forge-lint: disable-next-line(mixed-case-variable)
    SentinelOracleRequestMap.T private $requests;

    // forge-lint: disable-next-line(mixed-case-variable)
    SentinelOracleCommitmentMap.T private $commitments;

    // ============================================================
    // ERRORS
    // ============================================================

    error NotArbitrator();
    error NotConsensus();
    error InvalidAddress();
    error ZeroFee();
    error ZeroWindow();
    error SentinelNotActive();

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
        address consensus,
        address feeToken,
        uint256 requestFee,
        uint256 commitWindow,
        uint256 revealWindow,
        uint256 governanceDelay,
        uint256 initialMultiplier
    ) {
        require(arbitrator != address(0), InvalidAddress());
        require(consensus != address(0), InvalidAddress());
        require(feeToken != address(0), InvalidAddress());
        require(requestFee > 0, ZeroFee());
        require(commitWindow > 0, ZeroWindow());
        require(revealWindow > 0, ZeroWindow());
        ARBITRATOR = arbitrator;
        CONSENSUS = consensus;
        FEE_TOKEN = IERC20(feeToken);
        REQUEST_FEE = requestFee;
        COMMIT_WINDOW = commitWindow;
        REVEAL_WINDOW = revealWindow;
        GOVERNANCE_DELAY = governanceDelay;
        $bondConfig.init(initialMultiplier);
    }

    // ============================================================
    // IOracle IMPLEMENTATION
    // ============================================================

    function postRequest(bytes32 requestId, address proposer, bytes calldata) external override(IOracle) {
        require(msg.sender == CONSENSUS, NotConsensus());
        uint256 fee = REQUEST_FEE;
        uint256 bondTarget = fee * $bondConfig.currentMultiplier();
        uint256 commitDeadline = block.number + COMMIT_WINDOW;
        uint256 revealDeadline = commitDeadline + REVEAL_WINDOW;
        $requests.create(requestId, proposer, fee, bondTarget, commitDeadline, revealDeadline);
        FEE_TOKEN.safeTransferFrom(proposer, address(this), fee);
    }

    // ============================================================
    // VOTING
    // ============================================================

    function commit(bytes32 requestId, bytes32 commitHash) external {
        require($sentinelMap.isActive(msg.sender), SentinelNotActive());
        SentinelOracleRequest.Request storage req = $requests.get(requestId);
        uint256 bondAmount = req.applyCommit();
        $commitments.add(requestId, msg.sender, commitHash, bondAmount);
        FEE_TOKEN.safeTransferFrom(msg.sender, address(this), bondAmount);
    }

    function reveal(bytes32 requestId, bool approve, bytes32 salt) external {
        SentinelOracleRequest.Request storage req = $requests.get(requestId);
        $commitments.reveal(requestId, msg.sender, approve, salt);
        req.applyReveal(approve);
    }

    function hashCommitment(address sentinel, bytes32 requestId, bool approve, bytes32 salt)
        external
        pure
        returns (bytes32)
    {
        return SentinelOracleCommitment.computeHash(sentinel, requestId, approve, salt);
    }

    // ============================================================
    // FINALISATION
    // ============================================================

    function finalize(bytes32 requestId) external {
        SentinelOracleRequest.Request storage req = $requests.get(requestId);
        address proposer = req.proposer;
        (SentinelOracleRequest.State newState, uint256 refundFee, uint256 unrevealedBond) = req.finalize();

        if (unrevealedBond > 0) {
            FEE_TOKEN.safeTransfer(ARBITRATOR, unrevealedBond);
        }

        if (newState == SentinelOracleRequest.State.FROZEN) {
            return;
        }

        if (newState == SentinelOracleRequest.State.RESOLVED_APPROVED) {
            emit OracleResult(
                requestId, proposer, abi.encode(SentinelOracleRequest.ResolveReason.UNANIMOUS_APPROVE), true
            );
        } else if (newState == SentinelOracleRequest.State.RESOLVED_DENIED) {
            emit OracleResult(
                requestId, proposer, abi.encode(SentinelOracleRequest.ResolveReason.UNANIMOUS_DENY), false
            );
        } else {
            FEE_TOKEN.safeTransfer(proposer, refundFee);
            emit OracleResult(requestId, proposer, abi.encode(SentinelOracleRequest.ResolveReason.TIMEOUT), false);
        }
    }

    function claim(bytes32 requestId) external {
        SentinelOracleRequest.Request storage req = $requests.get(requestId);
        req.requireResolved();
        SentinelOracleCommitment.Commitment memory c = $commitments.get(requestId, msg.sender).markClaimed();
        bool approved = c.vote == SentinelOracleCommitment.Vote.APPROVED;
        uint256 feeReward = req.calcFeeReward(approved);
        uint256 bondReturn = req.isBondSlashed(approved) ? 0 : c.bondAmount;
        uint256 totalClaim = bondReturn + feeReward;
        if (totalClaim > 0) {
            FEE_TOKEN.safeTransfer(msg.sender, totalClaim);
        }
        emit Claimed(requestId, msg.sender, bondReturn, feeReward);
    }

    // ============================================================
    // ARBITRATION
    // ============================================================

    function resolveDispute(bytes32 requestId, bool approveWins) external onlyArbitrator {
        SentinelOracleRequest.Request storage req = $requests.get(requestId);
        address proposer = req.proposer;
        uint256 slashed = req.resolveDispute(approveWins);
        SentinelOracleRequest.State outcome = req.state;
        uint256 refundFee = req.fee;
        FEE_TOKEN.safeTransfer(proposer, refundFee);
        FEE_TOKEN.safeTransfer(ARBITRATOR, slashed - refundFee);
        emit DisputeResolved(requestId, outcome, slashed);
        emit OracleResult(requestId, proposer, abi.encode(SentinelOracleRequest.ResolveReason.ARBITRATION), approveWins);
    }

    // ============================================================
    // GOVERNANCE
    // ============================================================

    function addSentinel(address sentinel) external onlyArbitrator {
        $sentinelMap.add(sentinel, GOVERNANCE_DELAY);
    }

    function removeSentinel(address sentinel) external onlyArbitrator {
        $sentinelMap.remove(sentinel);
    }

    function scheduleBondMultiplier(uint256 newValue) external onlyArbitrator {
        $bondConfig.schedule(newValue, GOVERNANCE_DELAY);
    }

    function applyBondMultiplier() external {
        $bondConfig.applyPending();
    }

    // ============================================================
    // VIEW FUNCTIONS
    // ============================================================

    function sentinelActiveAt(address sentinel) external view returns (uint256) {
        return $sentinelMap.getActiveAt(sentinel);
    }

    function bondMultiplier() external view returns (uint256) {
        return $bondConfig.currentMultiplier();
    }

    function pendingBondMultiplier() external view returns (uint256) {
        return $bondConfig.pendingBondMultiplier;
    }

    function pendingBondMultiplierActiveAt() external view returns (uint256) {
        return $bondConfig.pendingBondMultiplierActiveAt;
    }

    function getRequest(bytes32 requestId) external view returns (SentinelOracleRequest.Request memory) {
        return $requests.requests[requestId];
    }

    function getCommitment(bytes32 requestId, address sentinel)
        external
        view
        returns (SentinelOracleCommitment.Commitment memory)
    {
        return $commitments.commitments[requestId][sentinel];
    }
}
