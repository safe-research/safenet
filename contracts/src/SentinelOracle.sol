// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {IERC20} from "@oz/token/ERC20/IERC20.sol";
import {SafeERC20} from "@oz/token/ERC20/utils/SafeERC20.sol";
import {IOracle} from "@/interfaces/IOracle.sol";
import {BondConfig} from "@/libraries/BondConfig.sol";
import {DelayedAddress} from "@/libraries/DelayedAddress.sol";
import {DelayedUint256} from "@/libraries/DelayedUint256.sol";
import {SentinelMap} from "@/libraries/SentinelMap.sol";
import {SentinelOracleCommitment, SentinelOracleCommitmentMap} from "@/libraries/SentinelOracleCommitments.sol";
import {SentinelOracleRequest, SentinelOracleRequestMap} from "@/libraries/SentinelOracleRequests.sol";

contract SentinelOracle is IOracle {
    using BondConfig for BondConfig.T;
    using DelayedAddress for DelayedAddress.T;
    using DelayedUint256 for DelayedUint256.T;
    using SentinelMap for SentinelMap.T;
    using SentinelOracleCommitment for SentinelOracleCommitment.Commitment;
    using SentinelOracleCommitmentMap for SentinelOracleCommitmentMap.T;
    using SentinelOracleRequest for SentinelOracleRequest.Request;
    using SentinelOracleRequestMap for SentinelOracleRequestMap.T;
    using SafeERC20 for IERC20;

    // ============================================================
    // EVENTS
    // ============================================================

    // `context` is the arbitrator's rationale for this ruling (free-form text, or e.g. an IPFS
    // CID pointing to a longer writeup).
    event DisputeResolved(
        bytes32 indexed requestId, SentinelOracleRequest.State outcome, uint256 slashed, string context
    );
    event Claimed(bytes32 indexed requestId, address indexed sentinel, uint256 bondReturn, uint256 feeReward);

    // ============================================================
    // IMMUTABLES
    // ============================================================

    address public immutable ARBITRATOR;
    address public immutable GOVERNANCE;
    // The trusted contract (typically Consensus) allowed to call `postRequest` -- distinct from a
    // request's `sponsor`, the address that funds a given request's fee and is refunded on timeout.
    address public immutable PROPOSER;
    IERC20 public immutable FEE_TOKEN;
    uint256 public immutable COMMIT_WINDOW;
    uint256 public immutable REVEAL_WINDOW;
    uint256 public immutable GOVERNANCE_DELAY;
    uint256 public immutable ARBITRATION_TIMEOUT;

    // ============================================================
    // CONSTANTS
    // ============================================================

    uint256 public constant FEE_SHARE_DENOMINATOR = 100_000;

    // ============================================================
    // STORAGE
    // ============================================================

    // forge-lint: disable-next-line(mixed-case-variable)
    BondConfig.T private $bondConfig;

    // forge-lint: disable-next-line(mixed-case-variable)
    DelayedAddress.T private $protocolFundsReceiverConfig;

    // forge-lint: disable-next-line(mixed-case-variable)
    DelayedUint256.T private $feeConfig;

    // forge-lint: disable-next-line(mixed-case-variable)
    DelayedUint256.T private $daoFeeShareConfig;

    // forge-lint: disable-next-line(mixed-case-variable)
    SentinelMap.T private $sentinelMap;

    // forge-lint: disable-next-line(mixed-case-variable)
    SentinelOracleRequestMap.T private $requests;

    // forge-lint: disable-next-line(mixed-case-variable)
    SentinelOracleCommitmentMap.T private $commitments;

    // The ENS name (e.g. `safenet-charter.safe.eth`) of the Charter this Oracle
    // trusts -- set once at construction and never updated (there is no setter),
    // so despite living in storage it is as fixed as an immutable would be. A
    // human-readable domain, rather than its namehash, is stored so it can be
    // read directly on-chain (by a validator, the explorer, or a block
    // explorer's "read contract" UI) without an off-chain ENS reverse lookup.
    // forge-lint: disable-next-line(mixed-case-variable)
    string private $charterEns;

    // ============================================================
    // ERRORS
    // ============================================================

    error NotArbitrator();
    error NotGovernance();
    error NotProposer();
    error InvalidAddress();
    error ZeroFee();
    error ZeroWindow();
    error SentinelNotActive();
    error InvalidFeeShare();

    // ============================================================
    // MODIFIERS
    // ============================================================

    // forge-lint: disable-start(unwrapped-modifier-logic)

    modifier onlyArbitrator() {
        require(msg.sender == ARBITRATOR, NotArbitrator());
        _;
    }

    modifier onlyGovernance() {
        require(msg.sender == GOVERNANCE, NotGovernance());
        _;
    }

    // forge-lint: disable-end(unwrapped-modifier-logic)

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    struct ConstructorParams {
        // Operators: the three addresses the SafeDAO/arbitrator ecosystem cares about.
        // `arbitrator` rules on frozen disputes; `governance` administers every other dynamic
        // parameter; `proposer` is the trusted contract (typically Consensus) allowed to call
        // `postRequest`. `protocolFundsReceiver` is grouped here too since it is itself an
        // address, even though -- unlike the other three -- it is governed/delay-gated rather
        // than immutable; this is only its *initial* value.
        address arbitrator;
        address governance;
        address protocolFundsReceiver;
        address proposer;
        // Fee config: everything that determines the size of a request's fee/bond/slash and how
        // it is split.
        address feeToken;
        uint256 requestFee;
        uint256 initialBondMultiplier;
        uint256 initialSlashingMultiplier;
        uint256 initialDaoFeeShare;
        // Timeouts: every block-denominated window/delay in the contract.
        uint256 commitWindow;
        uint256 revealWindow;
        uint256 governanceDelay;
        uint256 arbitrationTimeout;
        // Charter reference (see `$charterEns` below).
        string initialCharterEns;
    }

    constructor(ConstructorParams memory params) {
        require(params.arbitrator != address(0), InvalidAddress());
        require(params.governance != address(0), InvalidAddress());
        require(params.protocolFundsReceiver != address(0), InvalidAddress());
        require(params.proposer != address(0), InvalidAddress());
        require(params.feeToken != address(0), InvalidAddress());
        require(params.requestFee > 0, ZeroFee());
        require(params.commitWindow > 0, ZeroWindow());
        require(params.revealWindow > 0, ZeroWindow());
        require(params.initialDaoFeeShare <= FEE_SHARE_DENOMINATOR, InvalidFeeShare());
        require(params.arbitrationTimeout > 0, ZeroWindow());
        ARBITRATOR = params.arbitrator;
        GOVERNANCE = params.governance;
        PROPOSER = params.proposer;
        FEE_TOKEN = IERC20(params.feeToken);
        COMMIT_WINDOW = params.commitWindow;
        REVEAL_WINDOW = params.revealWindow;
        GOVERNANCE_DELAY = params.governanceDelay;
        ARBITRATION_TIMEOUT = params.arbitrationTimeout;
        $charterEns = params.initialCharterEns;
        $bondConfig.init(params.initialBondMultiplier, params.initialSlashingMultiplier);
        $protocolFundsReceiverConfig.init(params.protocolFundsReceiver);
        $feeConfig.init(params.requestFee);
        $daoFeeShareConfig.init(params.initialDaoFeeShare);
    }

    // ============================================================
    // IOracle IMPLEMENTATION
    // ============================================================

    function postRequest(bytes32 requestId, address sponsor, bytes calldata) external override(IOracle) {
        require(msg.sender == PROPOSER, NotProposer());
        uint256 currentFee = $feeConfig.applyPending();
        (uint256 currentBondMultiplier, uint256 currentSlashingMultiplier) = $bondConfig.applyPending();
        uint256 bondTarget = currentFee * currentBondMultiplier;
        uint256 slashAmount = currentFee * currentSlashingMultiplier;
        uint256 currentDaoFeeShare = $daoFeeShareConfig.applyPending();
        uint256 commitDeadline = block.number + COMMIT_WINDOW;
        uint256 revealDeadline = commitDeadline + REVEAL_WINDOW;
        $requests.create(
            requestId, sponsor, currentFee, bondTarget, currentDaoFeeShare, slashAmount, commitDeadline, revealDeadline
        );
        FEE_TOKEN.safeTransferFrom(sponsor, address(this), currentFee);
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

    function reveal(bytes32 requestId, bool approve, bytes32 salt, string calldata reason) external {
        SentinelOracleRequest.Request storage req = $requests.get(requestId);
        $commitments.reveal(requestId, msg.sender, approve, salt, reason);
        req.applyReveal(approve);
    }

    function hashCommitment(address sentinel, bytes32 requestId, bool approve, bytes32 salt, string calldata reason)
        external
        pure
        returns (bytes32)
    {
        return SentinelOracleCommitment.computeHash(sentinel, requestId, approve, salt, reason);
    }

    // ============================================================
    // FINALISATION
    // ============================================================

    function finalize(bytes32 requestId) external {
        SentinelOracleRequest.Request storage req = $requests.get(requestId);
        address sponsor = req.sponsor;
        (SentinelOracleRequest.State newState, uint256 refundFee, uint256 unrevealedBond) =
            req.finalize(ARBITRATION_TIMEOUT);

        address fundsReceiver = $protocolFundsReceiverConfig.applyPending();
        if (unrevealedBond > 0) {
            FEE_TOKEN.safeTransfer(fundsReceiver, unrevealedBond);
        }

        if (newState == SentinelOracleRequest.State.FROZEN) {
            return;
        }

        if (newState == SentinelOracleRequest.State.TIMED_OUT) {
            FEE_TOKEN.safeTransfer(sponsor, refundFee);
            return;
        }

        uint256 daoCut = req.fee * req.daoFeeShare / FEE_SHARE_DENOMINATOR;
        req.fee -= daoCut;
        if (daoCut > 0) {
            FEE_TOKEN.safeTransfer(fundsReceiver, daoCut);
        }

        emit OracleResult(requestId, sponsor, "", newState == SentinelOracleRequest.State.RESOLVED_APPROVED);
    }

    function claim(bytes32 requestId) external {
        SentinelOracleRequest.Request storage req = $requests.get(requestId);
        req.requireResolved();
        SentinelOracleCommitment.Commitment storage commitment = $commitments.get(requestId, msg.sender);
        SentinelOracleCommitment.Vote vote = commitment.vote;
        commitment.markClaimed();
        uint256 feeReward = req.calcFeeReward(vote);
        uint256 bondReturn = commitment.bondAmount - req.slashAmountFor(vote);
        uint256 totalClaim = bondReturn + feeReward;
        if (totalClaim > 0) {
            FEE_TOKEN.safeTransfer(msg.sender, totalClaim);
        }
        emit Claimed(requestId, msg.sender, bondReturn, feeReward);
    }

    // ============================================================
    // ARBITRATION
    // ============================================================

    function resolveDispute(bytes32 requestId, bool approveWins, string calldata context) external onlyArbitrator {
        SentinelOracleRequest.Request storage req = $requests.get(requestId);
        address sponsor = req.sponsor;
        uint256 slashed = req.resolveDispute(approveWins);
        SentinelOracleRequest.State outcome = req.state;
        uint256 refundFee = req.fee;
        uint256 daoCut = refundFee * req.daoFeeShare / FEE_SHARE_DENOMINATOR;
        req.fee -= daoCut;
        address fundsReceiver = $protocolFundsReceiverConfig.applyPending();
        FEE_TOKEN.safeTransfer(sponsor, refundFee);
        FEE_TOKEN.safeTransfer(fundsReceiver, slashed - refundFee + daoCut);
        emit DisputeResolved(requestId, outcome, slashed, context);
    }

    // Permissionless: a `FROZEN` request that outlives `ARBITRATION_TIMEOUT` should not wait on the
    // arbitrator forever. Reuses the `TIMED_OUT` machinery, so every committed bond returns in
    // full via `claim()` -- identical to the no-reveal timeout path.
    function timeoutArbitration(bytes32 requestId) external {
        SentinelOracleRequest.Request storage req = $requests.get(requestId);
        address sponsor = req.sponsor;
        uint256 refundFee = req.timeoutArbitration();
        FEE_TOKEN.safeTransfer(sponsor, refundFee);
    }

    // ============================================================
    // GOVERNANCE
    // ============================================================

    function addSentinel(address sentinel) external onlyGovernance {
        $sentinelMap.add(sentinel, GOVERNANCE_DELAY);
    }

    function removeSentinel(address sentinel) external onlyGovernance {
        $sentinelMap.remove(sentinel);
    }

    function scheduleBondConfig(uint256 newBondMultiplier, uint256 newSlashingMultiplier) external onlyGovernance {
        $bondConfig.schedule(newBondMultiplier, newSlashingMultiplier, GOVERNANCE_DELAY);
    }

    function applyBondConfig() external {
        $bondConfig.applyPending();
    }

    function scheduleProtocolFundsReceiver(address newValue) external onlyGovernance {
        require(newValue != address(0), InvalidAddress());
        $protocolFundsReceiverConfig.schedule(newValue, GOVERNANCE_DELAY);
    }

    function applyProtocolFundsReceiver() external {
        $protocolFundsReceiverConfig.applyPending();
    }

    function scheduleFee(uint256 newValue) external onlyGovernance {
        require(newValue > 0, ZeroFee());
        $feeConfig.schedule(newValue, GOVERNANCE_DELAY);
    }

    function applyFee() external {
        $feeConfig.applyPending();
    }

    function scheduleDaoFeeShare(uint256 newValue) external onlyGovernance {
        require(newValue <= FEE_SHARE_DENOMINATOR, InvalidFeeShare());
        $daoFeeShareConfig.schedule(newValue, GOVERNANCE_DELAY);
    }

    function applyDaoFeeShare() external {
        $daoFeeShareConfig.applyPending();
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
        return $bondConfig.pendingActiveAt;
    }

    function slashingMultiplier() external view returns (uint256) {
        return $bondConfig.currentSlashingMultiplier();
    }

    function pendingSlashingMultiplier() external view returns (uint256) {
        return $bondConfig.pendingSlashingMultiplier;
    }

    function protocolFundsReceiver() external view returns (address) {
        return $protocolFundsReceiverConfig.current();
    }

    function pendingProtocolFundsReceiver() external view returns (address) {
        return $protocolFundsReceiverConfig.pendingValue;
    }

    function pendingProtocolFundsReceiverActiveAt() external view returns (uint256) {
        return $protocolFundsReceiverConfig.pendingActiveAt;
    }

    function fee() external view returns (uint256) {
        return $feeConfig.current();
    }

    function pendingFee() external view returns (uint256) {
        return $feeConfig.pendingValue;
    }

    function pendingFeeActiveAt() external view returns (uint256) {
        return $feeConfig.pendingActiveAt;
    }

    function daoFeeShare() external view returns (uint256) {
        return $daoFeeShareConfig.current();
    }

    function pendingDaoFeeShare() external view returns (uint256) {
        return $daoFeeShareConfig.pendingValue;
    }

    function pendingDaoFeeShareActiveAt() external view returns (uint256) {
        return $daoFeeShareConfig.pendingActiveAt;
    }

    function charterEns() external view returns (string memory) {
        return $charterEns;
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
