// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {IERC20} from "@oz/token/ERC20/IERC20.sol";
import {SafeERC20} from "@oz/token/ERC20/utils/SafeERC20.sol";
import {SafeCast} from "@oz/utils/math/SafeCast.sol";
import {IOracle} from "@/interfaces/IOracle.sol";
import {BondConfig} from "@/libraries/BondConfig.sol";
import {DelayedAddress} from "@/libraries/DelayedAddress.sol";
import {DelayedUint96} from "@/libraries/DelayedUint96.sol";
import {SentinelMap} from "@/libraries/SentinelMap.sol";
import {SentinelOracleCommitment, SentinelOracleCommitmentMap} from "@/libraries/SentinelOracleCommitments.sol";
import {SentinelOracleRequest, SentinelOracleRequestMap} from "@/libraries/SentinelOracleRequests.sol";

contract SentinelOracle is IOracle {
    using BondConfig for BondConfig.T;
    using DelayedAddress for DelayedAddress.T;
    using DelayedUint96 for DelayedUint96.T;
    using SentinelMap for SentinelMap.T;
    using SentinelOracleCommitment for SentinelOracleCommitment.Commitment;
    using SentinelOracleCommitmentMap for SentinelOracleCommitmentMap.T;
    using SentinelOracleRequest for SentinelOracleRequest.Request;
    using SentinelOracleRequestMap for SentinelOracleRequestMap.T;
    using SafeERC20 for IERC20;
    using SafeCast for uint256;

    // ============================================================
    // EVENTS
    // ============================================================

    // `context` is the arbitrator's rationale for this ruling (free-form text, or e.g. an IPFS
    // CID pointing to a longer writeup).
    event DisputeResolved(
        bytes32 indexed requestId, SentinelOracleRequest.State outcome, uint128 slashed, string context
    );
    event Claimed(bytes32 indexed requestId, address indexed sentinel, uint96 bondReturn, uint96 feeReward);

    event FeeScheduled(uint96 newValue, uint64 activeAtBlock);
    event DaoFeeShareScheduled(uint24 newValue, uint64 activeAtBlock);
    event ProtocolFundsReceiverScheduled(address newValue, uint64 activeAtBlock);

    // ============================================================
    // IMMUTABLES
    // ============================================================

    address public immutable ARBITRATOR;
    address public immutable GOVERNANCE;
    // The trusted contract (typically Consensus) allowed to call `postRequest` -- distinct from a
    // request's `sponsor`, the address that funds a given request's fee and is refunded on timeout.
    address public immutable PROPOSER;
    IERC20 public immutable FEE_TOKEN;
    // `uint32` blocks is vastly more than any realistic window/delay/timeout needs (billions of
    // blocks -- centuries even at a fast chain's block time), matching the same convention already
    // used for `BondConfig`'s multipliers.
    uint32 public immutable COMMIT_WINDOW;
    uint32 public immutable REVEAL_WINDOW;
    uint32 public immutable GOVERNANCE_DELAY;
    uint32 public immutable ARBITRATION_TIMEOUT;

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
    DelayedUint96.T private $feeConfig;

    // forge-lint: disable-next-line(mixed-case-variable)
    DelayedUint96.T private $daoFeeShareConfig;

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
    error AmountOutOfRange();
    error DeadlineOutOfRange();

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
        // it is split. Sized to match each value's own governed-storage width (see `$feeConfig`,
        // `$bondConfig`, `$daoFeeShareConfig` below) so an oversized value fails at the ABI/
        // calldata boundary instead of being silently accepted and caught later.
        address feeToken;
        uint96 requestFee;
        uint32 initialBondMultiplier;
        uint32 initialSlashingMultiplier;
        uint24 initialDaoFeeShare;
        // Timeouts: every block-denominated window/delay in the contract.
        uint32 commitWindow;
        uint32 revealWindow;
        uint32 governanceDelay;
        uint32 arbitrationTimeout;
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
        uint96 currentFee = $feeConfig.applyPending();
        (uint32 currentBondMultiplier, uint32 currentSlashingMultiplier) = $bondConfig.applyPending();
        // Widen `currentFee` to uint256 *before* multiplying -- computing directly in `uint96`
        // (the wider of {uint96, uint32}) risks an overflow revert for a legitimate large
        // fee/multiplier combination, even though the checked result below fits comfortably.
        uint96 bondTarget = (uint256(currentFee) * currentBondMultiplier).toUint96();
        uint96 slashAmount = (uint256(currentFee) * currentSlashingMultiplier).toUint96();
        // Dao fee share type is checked when setting it, therefore can be always assumed to fit uint24
        uint24 currentDaoFeeShare = uint24($daoFeeShareConfig.applyPending());
        uint256 commitDeadlineWide = block.number + COMMIT_WINDOW;
        uint256 revealDeadlineWide = commitDeadlineWide + REVEAL_WINDOW;
        uint64 commitDeadline = commitDeadlineWide.toUint64();
        uint64 revealDeadline = revealDeadlineWide.toUint64();
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
        uint96 bondAmount = req.applyCommit();
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
        (SentinelOracleRequest.State newState, uint96 refundFee, uint128 unrevealedBond) =
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

        // Read req.fee once -- it's used twice below (the multiply and the narrowing subtraction),
        // and this is the only path in `finalize()` that reads it, so caching it locally avoids a
        // second, redundant read of the same storage slot.
        uint96 feeAmount = req.fee;
        // Widen feeAmount to uint256 *before* multiplying -- see the identical reasoning in
        // `postRequest` above. `daoCut` is always <= feeAmount (daoFeeShare <= FEE_SHARE_DENOMINATOR),
        // so the narrowing cast back to the uint96 field below never truncates.
        uint256 daoCut = uint256(feeAmount) * req.daoFeeShare / FEE_SHARE_DENOMINATOR;
        // forge-lint: disable-next-line(unsafe-typecast)
        req.fee = uint96(feeAmount - daoCut);
        if (daoCut > 0) {
            FEE_TOKEN.safeTransfer(fundsReceiver, daoCut);
        }

        emit OracleResult(requestId, sponsor, "", newState == SentinelOracleRequest.State.RESOLVED_APPROVED);
    }

    function claim(bytes32 requestId) external {
        SentinelOracleRequest.Request storage req = $requests.get(requestId);
        // Resolved once here and threaded into `calcFeeReward`/`slashAmountFor` below, rather than
        // each of those independently re-deriving it (`requireResolved`/`self.state`) -- otherwise
        // this function would read `req.state`'s storage slot three separate times for a value
        // that never changes across the call.
        SentinelOracleRequest.State state = req.requireResolved();
        SentinelOracleCommitment.Commitment storage commitment = $commitments.get(requestId, msg.sender);
        SentinelOracleCommitment.Vote vote = commitment.vote;
        commitment.markClaimed();
        uint96 feeReward = req.calcFeeReward(state, vote);
        uint96 bondReturn = commitment.bondAmount - req.slashAmountFor(state, vote);
        // Widen before adding: `bondReturn` and `feeReward` can each be near `uint96`'s max, and
        // their sum could exceed it even though it fits easily in `uint256`.
        uint256 totalClaim = uint256(bondReturn) + feeReward;
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
        uint128 slashed = req.resolveDispute(approveWins);
        SentinelOracleRequest.State outcome = req.state;
        // Deliberately uint256, not uint96 (unlike finalize()/timeoutArbitration()'s refundFee,
        // which are plain passthroughs of req.fee with no further arithmetic): widening here is
        // what makes `refundFee * req.daoFeeShare` below compute in uint256 instead of uint96 (the
        // wider of the two operand types otherwise), avoiding the same overflow-revert risk as the
        // other widen-before-multiply cases in this contract.
        uint256 refundFee = req.fee;
        uint256 daoCut = refundFee * req.daoFeeShare / FEE_SHARE_DENOMINATOR;
        // daoCut <= refundFee == req.fee by construction (daoFeeShare <= FEE_SHARE_DENOMINATOR),
        // so this never truncates despite the explicit narrowing cast back to the uint96 field.
        // forge-lint: disable-next-line(unsafe-typecast)
        req.fee = uint96(refundFee - daoCut);
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
        uint96 refundFee = req.timeoutArbitration();
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

    function scheduleBondConfig(uint32 newBondMultiplier, uint32 newSlashingMultiplier) external onlyGovernance {
        // BondConfig emits its own (already-specific) BondConfigScheduled event internally.
        $bondConfig.schedule(newBondMultiplier, newSlashingMultiplier, GOVERNANCE_DELAY);
    }

    function applyBondConfig() external {
        $bondConfig.applyPending();
    }

    function scheduleProtocolFundsReceiver(address newValue) external onlyGovernance {
        require(newValue != address(0), InvalidAddress());
        uint64 activeAt = $protocolFundsReceiverConfig.schedule(newValue, GOVERNANCE_DELAY);
        emit ProtocolFundsReceiverScheduled(newValue, activeAt);
    }

    function applyProtocolFundsReceiver() external {
        $protocolFundsReceiverConfig.applyPending();
    }

    function scheduleFee(uint96 newValue) external onlyGovernance {
        require(newValue > 0, ZeroFee());
        uint64 activeAt = $feeConfig.schedule(newValue, GOVERNANCE_DELAY);
        emit FeeScheduled(newValue, activeAt);
    }

    function applyFee() external {
        $feeConfig.applyPending();
    }

    function scheduleDaoFeeShare(uint24 newValue) external onlyGovernance {
        require(newValue <= FEE_SHARE_DENOMINATOR, InvalidFeeShare());
        uint64 activeAt = $daoFeeShareConfig.schedule(newValue, GOVERNANCE_DELAY);
        emit DaoFeeShareScheduled(newValue, activeAt);
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

    function bondMultiplier() external view returns (uint32) {
        return $bondConfig.currentMultiplier();
    }

    function slashingMultiplier() external view returns (uint32) {
        return $bondConfig.currentSlashingMultiplier();
    }

    // Returns (0, 0, 0) if no bond config change is currently scheduled.
    function pendingBondConfig()
        external
        view
        returns (uint32 pendingBondMultiplier, uint32 pendingSlashingMultiplier, uint64 activeAt)
    {
        return $bondConfig.pending();
    }

    function protocolFundsReceiver() external view returns (address) {
        return $protocolFundsReceiverConfig.current();
    }

    // Returns (address(0), 0) if no protocol funds receiver change is currently scheduled.
    function pendingProtocolFundsReceiver() external view returns (address value, uint64 activeAt) {
        return $protocolFundsReceiverConfig.pending();
    }

    function fee() external view returns (uint96) {
        return $feeConfig.current();
    }

    // Returns (0, 0) if no fee change is currently scheduled.
    function pendingFee() external view returns (uint96 value, uint64 activeAt) {
        return $feeConfig.pending();
    }

    function daoFeeShare() external view returns (uint24) {
        // Safe: always <= FEE_SHARE_DENOMINATOR (validated at every schedule/init site), which
        // fits uint24.
        // forge-lint: disable-next-line(unsafe-typecast)
        return uint24($daoFeeShareConfig.current());
    }

    // Returns (0, 0) if no DAO fee share change is currently scheduled.
    function pendingDaoFeeShare() external view returns (uint24 value, uint64 activeAt) {
        (uint96 wideValue, uint64 wideActiveAt) = $daoFeeShareConfig.pending();
        // forge-lint: disable-next-line(unsafe-typecast)
        return (uint24(wideValue), wideActiveAt);
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
