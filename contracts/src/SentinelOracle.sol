// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {IERC20} from "@oz/token/ERC20/IERC20.sol";
import {SafeERC20} from "@oz/token/ERC20/utils/SafeERC20.sol";
import {IOracle} from "@/interfaces/IOracle.sol";
import {BondConfig} from "@/libraries/BondConfig.sol";
import {SentinelMap} from "@/libraries/SentinelMap.sol";
import {SentinelOracleCommitments} from "@/libraries/SentinelOracleCommitments.sol";
import {SentinelOracleRequest} from "@/libraries/SentinelOracleRequest.sol";

contract SentinelOracle is IOracle {
    using BondConfig for BondConfig.T;
    using SentinelMap for SentinelMap.T;
    using SentinelOracleCommitments for SentinelOracleCommitments.T;
    using SentinelOracleRequest for SentinelOracleRequest.T;
    using SafeERC20 for IERC20;

    // ============================================================
    // EVENTS
    // ============================================================

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
    uint256 public immutable GOVERNANCE_DELAY;

    // ============================================================
    // STORAGE
    // ============================================================

    // forge-lint: disable-next-line(mixed-case-variable)
    BondConfig.T private $bondConfig;

    // forge-lint: disable-next-line(mixed-case-variable)
    SentinelMap.T private $sentinelMap;

    // forge-lint: disable-next-line(mixed-case-variable)
    SentinelOracleRequest.T private $requests;

    // forge-lint: disable-next-line(mixed-case-variable)
    SentinelOracleCommitments.T private $commitments;

    // ============================================================
    // ERRORS
    // ============================================================

    error NotArbitrator();
    error InvalidAddress();
    error ZeroFee();
    error SentinelNotActive();
    error ZeroBond();

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
    ) {
        require(arbitrator != address(0), InvalidAddress());
        require(feeToken != address(0), InvalidAddress());
        require(requestFee > 0, ZeroFee());
        ARBITRATOR = arbitrator;
        FEE_TOKEN = IERC20(feeToken);
        REQUEST_FEE = requestFee;
        VOTING_WINDOW = votingWindow;
        GOVERNANCE_DELAY = governanceDelay;
        $bondConfig.init(initialMultiplier);
    }

    // ============================================================
    // IOracle IMPLEMENTATION
    // ============================================================

    function postRequest(bytes32 requestId) external override(IOracle) {
        uint256 fee = REQUEST_FEE;
        uint256 bondTarget = fee * $bondConfig.bondMultiplier;
        uint256 deadline = block.number + VOTING_WINDOW;
        $requests.create(requestId, msg.sender, fee, bondTarget, deadline);
        FEE_TOKEN.safeTransferFrom(msg.sender, address(this), fee);
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
        (SentinelOracleRequest.State newState, address proposer, uint256 refundFee) = $requests.finalize(requestId);

        if (newState == SentinelOracleRequest.State.FROZEN) return;

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
        SentinelOracleRequest.State state = $requests.requireResolved(requestId);
        SentinelOracleCommitments.Commitment memory c = $commitments.markClaimed(requestId, msg.sender);
        uint256 feeReward = $requests.calcFeeReward(requestId, c.approved, c.bondAmount, c.position, state);
        FEE_TOKEN.safeTransfer(msg.sender, c.bondAmount + feeReward);
        emit Claimed(requestId, msg.sender, c.bondAmount, feeReward);
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
        return $bondConfig.bondMultiplier;
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
        returns (SentinelOracleCommitments.Commitment memory)
    {
        return $commitments.commitments[requestId][sentinel];
    }

    // ============================================================
    // INTERNAL HELPERS
    // ============================================================

    function _commit(bytes32 requestId, bool approve, uint256 bondAmount) internal {
        require(bondAmount > 0, ZeroBond());
        require($sentinelMap.isActive(msg.sender), SentinelNotActive());
        $commitments.checkNotCommitted(requestId, msg.sender);
        (uint256 effectiveBond, uint256 position) = $requests.applyCommit(requestId, approve, bondAmount);
        $commitments.add(requestId, msg.sender, approve, effectiveBond, position);
        FEE_TOKEN.safeTransferFrom(msg.sender, address(this), effectiveBond);
    }
}
