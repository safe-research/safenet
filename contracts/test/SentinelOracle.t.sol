// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {IOracle} from "@/interfaces/IOracle.sol";
import {BondConfig} from "@/libraries/BondConfig.sol";
import {SentinelOracle} from "@/SentinelOracle.sol";
import {SentinelOracleRequest} from "@/libraries/SentinelOracleRequests.sol";
import {SentinelOracleCommitment, SentinelOracleCommitmentMap} from "@/libraries/SentinelOracleCommitments.sol";
import {MockERC20} from "@test/util/MockERC20.sol";

contract SentinelOracleTest is Test {
    // ============================================================
    // CONSTANTS
    // ============================================================

    uint256 constant REQUEST_FEE = 10_000; // 1 cent in a 6-decimal token (e.g. USDC)
    uint256 constant BOND_MULTIPLIER = 2;
    uint256 constant BOND_TARGET = REQUEST_FEE * BOND_MULTIPLIER; // 20_000
    uint256 constant SLASHING_MULTIPLIER = 2;
    uint256 constant SLASH_AMOUNT = REQUEST_FEE * SLASHING_MULTIPLIER; // 20_000, equal to BOND_TARGET by default
    uint256 constant COMMIT_WINDOW = 12;
    uint256 constant REVEAL_WINDOW = 12;
    uint256 constant GOVERNANCE_DELAY = 100;
    uint256 constant INITIAL_DAO_FEE_SHARE = 0;
    string constant CHARTER_ENS = "safenet-charter.safe.eth";
    uint256 constant ARBITRATION_TIMEOUT = 20;

    bytes32 constant REQUEST_ID = keccak256("request-1");
    bytes32 constant SALT_1 = keccak256("salt-1");
    bytes32 constant SALT_2 = keccak256("salt-2");
    bytes32 constant SALT_3 = keccak256("salt-3");

    // ============================================================
    // STATE
    // ============================================================

    SentinelOracle public oracle;
    MockERC20 public token;

    address public arbitrator;
    address public governance;
    address public protocolFundsReceiver;
    address public proposer;
    address public sponsor;
    address public sentinel1;
    address public sentinel2;
    address public sentinel3;

    // ============================================================
    // SETUP
    // ============================================================

    function setUp() public {
        arbitrator = vm.createWallet("arbitrator").addr;
        governance = vm.createWallet("governance").addr;
        protocolFundsReceiver = vm.createWallet("protocolFundsReceiver").addr;
        proposer = vm.createWallet("proposer").addr;
        sponsor = vm.createWallet("sponsor").addr;
        sentinel1 = vm.createWallet("sentinel1").addr;
        sentinel2 = vm.createWallet("sentinel2").addr;
        sentinel3 = vm.createWallet("sentinel3").addr;

        token = new MockERC20("Fee Token", "FEE");
        oracle = new SentinelOracle(
            arbitrator,
            governance,
            protocolFundsReceiver,
            proposer,
            address(token),
            REQUEST_FEE,
            COMMIT_WINDOW,
            REVEAL_WINDOW,
            GOVERNANCE_DELAY,
            BOND_MULTIPLIER,
            SLASHING_MULTIPLIER,
            INITIAL_DAO_FEE_SHARE,
            CHARTER_ENS,
            ARBITRATION_TIMEOUT
        );

        // Fund accounts
        token.mint(sponsor, 100_000);
        token.mint(sentinel1, 100_000);
        token.mint(sentinel2, 100_000);
        token.mint(sentinel3, 100_000);

        // Approve oracle for fee/bond pulls
        vm.prank(sponsor);
        token.approve(address(oracle), type(uint256).max);
        vm.prank(sentinel1);
        token.approve(address(oracle), type(uint256).max);
        vm.prank(sentinel2);
        token.approve(address(oracle), type(uint256).max);
        vm.prank(sentinel3);
        token.approve(address(oracle), type(uint256).max);

        // Register sentinels (active immediately by rolling past GOVERNANCE_DELAY)
        vm.startPrank(governance);
        oracle.addSentinel(sentinel1);
        oracle.addSentinel(sentinel2);
        oracle.addSentinel(sentinel3);
        vm.stopPrank();

        vm.roll(block.number + GOVERNANCE_DELAY);
    }

    // ============================================================
    // HELPERS
    // ============================================================

    function _postRequest() internal {
        vm.prank(proposer);
        oracle.postRequest(REQUEST_ID, sponsor, "");
    }

    function _commit(address sentinel, bool approve, bytes32 salt) internal {
        _commit(sentinel, approve, salt, "");
    }

    function _commit(address sentinel, bool approve, bytes32 salt, string memory reason) internal {
        bytes32 hash = oracle.hashCommitment(sentinel, REQUEST_ID, approve, salt, reason);
        vm.prank(sentinel);
        oracle.commit(REQUEST_ID, hash);
    }

    function _reveal(address sentinel, bool approve, bytes32 salt) internal {
        _reveal(sentinel, approve, salt, "");
    }

    function _reveal(address sentinel, bool approve, bytes32 salt, string memory reason) internal {
        vm.prank(sentinel);
        oracle.reveal(REQUEST_ID, approve, salt, reason);
    }

    function _advancePastCommitDeadline() internal {
        vm.roll(block.number + COMMIT_WINDOW + 1);
    }

    function _advancePastRevealDeadline() internal {
        vm.roll(block.number + REVEAL_WINDOW + 1);
    }

    // ============================================================
    // GOVERNANCE ACCESS CONTROL
    // ============================================================

    function test_AddSentinel_OnlyGovernance() public {
        address randomAddress = vm.createWallet("random").addr;
        address newSentinel = vm.createWallet("newSentinel").addr;

        vm.expectRevert(SentinelOracle.NotGovernance.selector);
        vm.prank(arbitrator);
        oracle.addSentinel(newSentinel);

        vm.expectRevert(SentinelOracle.NotGovernance.selector);
        vm.prank(randomAddress);
        oracle.addSentinel(newSentinel);

        vm.prank(governance);
        oracle.addSentinel(newSentinel);
    }

    function test_RemoveSentinel_OnlyGovernance() public {
        address randomAddress = vm.createWallet("random").addr;

        vm.expectRevert(SentinelOracle.NotGovernance.selector);
        vm.prank(arbitrator);
        oracle.removeSentinel(sentinel1);

        vm.expectRevert(SentinelOracle.NotGovernance.selector);
        vm.prank(randomAddress);
        oracle.removeSentinel(sentinel1);

        vm.prank(governance);
        oracle.removeSentinel(sentinel1);
    }

    function test_ScheduleBondConfig_OnlyGovernance() public {
        address randomAddress = vm.createWallet("random").addr;

        vm.expectRevert(SentinelOracle.NotGovernance.selector);
        vm.prank(arbitrator);
        oracle.scheduleBondConfig(BOND_MULTIPLIER + 1, SLASHING_MULTIPLIER + 1);

        vm.expectRevert(SentinelOracle.NotGovernance.selector);
        vm.prank(randomAddress);
        oracle.scheduleBondConfig(BOND_MULTIPLIER + 1, SLASHING_MULTIPLIER + 1);

        vm.prank(governance);
        oracle.scheduleBondConfig(BOND_MULTIPLIER + 1, SLASHING_MULTIPLIER + 1);
    }

    function test_ScheduleFee_OnlyGovernance() public {
        address randomAddress = vm.createWallet("random").addr;

        vm.expectRevert(SentinelOracle.NotGovernance.selector);
        vm.prank(arbitrator);
        oracle.scheduleFee(REQUEST_FEE + 1);

        vm.expectRevert(SentinelOracle.NotGovernance.selector);
        vm.prank(randomAddress);
        oracle.scheduleFee(REQUEST_FEE + 1);

        vm.prank(governance);
        oracle.scheduleFee(REQUEST_FEE + 1);
    }

    function test_ScheduleDaoFeeShare_OnlyGovernance() public {
        address randomAddress = vm.createWallet("random").addr;

        vm.expectRevert(SentinelOracle.NotGovernance.selector);
        vm.prank(arbitrator);
        oracle.scheduleDaoFeeShare(10_000);

        vm.expectRevert(SentinelOracle.NotGovernance.selector);
        vm.prank(randomAddress);
        oracle.scheduleDaoFeeShare(10_000);

        vm.prank(governance);
        oracle.scheduleDaoFeeShare(10_000);
    }

    function test_ScheduleProtocolFundsReceiver_OnlyGovernance() public {
        address randomAddress = vm.createWallet("random").addr;
        address newReceiver = vm.createWallet("newReceiver").addr;

        vm.expectRevert(SentinelOracle.NotGovernance.selector);
        vm.prank(arbitrator);
        oracle.scheduleProtocolFundsReceiver(newReceiver);

        vm.expectRevert(SentinelOracle.NotGovernance.selector);
        vm.prank(randomAddress);
        oracle.scheduleProtocolFundsReceiver(newReceiver);

        vm.prank(governance);
        oracle.scheduleProtocolFundsReceiver(newReceiver);
    }

    // ============================================================
    // PROTOCOL FUNDS RECEIVER SCHEDULE/APPLY
    // ============================================================

    function test_ScheduleProtocolFundsReceiver_ZeroAddress_Reverts() public {
        vm.expectRevert(SentinelOracle.InvalidAddress.selector);
        vm.prank(governance);
        oracle.scheduleProtocolFundsReceiver(address(0));
    }

    function test_ProtocolFundsReceiver_ScheduleApplyRoundTrip() public {
        address newReceiver = vm.createWallet("newReceiver").addr;

        assertEq(oracle.protocolFundsReceiver(), protocolFundsReceiver, "starts at the constructor value");

        vm.prank(governance);
        oracle.scheduleProtocolFundsReceiver(newReceiver);

        assertEq(oracle.pendingProtocolFundsReceiver(), newReceiver);
        assertEq(oracle.protocolFundsReceiver(), protocolFundsReceiver, "not active until the delay elapses");

        // Applying too early is a no-op, not a revert -- it's permissionless and harmless to call
        // speculatively.
        oracle.applyProtocolFundsReceiver();
        assertEq(oracle.protocolFundsReceiver(), protocolFundsReceiver, "still not active after an early apply");
        assertEq(oracle.pendingProtocolFundsReceiver(), newReceiver, "pending value survives an early apply");

        vm.roll(block.number + GOVERNANCE_DELAY);
        oracle.applyProtocolFundsReceiver();

        assertEq(oracle.protocolFundsReceiver(), newReceiver, "takes effect once the delay has elapsed");
        assertEq(oracle.pendingProtocolFundsReceiver(), address(0));
        assertEq(oracle.pendingProtocolFundsReceiverActiveAt(), 0);
    }

    function test_ProtocolFundsReceiver_RescheduleBeforeMaturity_OverwritesPending() public {
        address firstReceiver = vm.createWallet("firstReceiver").addr;
        address secondReceiver = vm.createWallet("secondReceiver").addr;

        vm.startPrank(governance);
        oracle.scheduleProtocolFundsReceiver(firstReceiver);

        // Governance notices the mistake before `firstReceiver` matures and corrects it —
        // this must overwrite the still-pending schedule, not revert.
        oracle.scheduleProtocolFundsReceiver(secondReceiver);
        vm.stopPrank();

        assertEq(oracle.pendingProtocolFundsReceiver(), secondReceiver, "second schedule overwrites the first");
        assertEq(oracle.protocolFundsReceiver(), protocolFundsReceiver, "not active until the delay elapses");

        vm.roll(block.number + GOVERNANCE_DELAY);
        oracle.applyProtocolFundsReceiver();

        assertEq(oracle.protocolFundsReceiver(), secondReceiver, "corrected value takes effect, not the mistake");
    }

    // ============================================================
    // BOND CONFIG (BOND MULTIPLIER + SLASHING MULTIPLIER) SCHEDULE/APPLY
    // ============================================================

    function test_ScheduleBondConfig_ZeroBondMultiplier_Reverts() public {
        vm.expectRevert(BondConfig.InvalidBondMultiplier.selector);
        vm.prank(governance);
        oracle.scheduleBondConfig(0, 0);
    }

    function test_ScheduleBondConfig_SlashingMultiplierBelowOne_Reverts() public {
        vm.expectRevert(BondConfig.InvalidSlashingMultiplier.selector);
        vm.prank(governance);
        oracle.scheduleBondConfig(BOND_MULTIPLIER, 0);
    }

    function test_ScheduleBondConfig_SlashingMultiplierAboveBondMultiplier_Reverts() public {
        vm.expectRevert(BondConfig.InvalidSlashingMultiplier.selector);
        vm.prank(governance);
        oracle.scheduleBondConfig(BOND_MULTIPLIER, BOND_MULTIPLIER + 1);
    }

    function test_BondConfig_ScheduleApplyRoundTrip() public {
        uint256 newMultiplier = BOND_MULTIPLIER + 2;
        uint256 newSlashingMultiplier = 1;

        assertEq(oracle.bondMultiplier(), BOND_MULTIPLIER, "starts at the constructor value");
        assertEq(oracle.slashingMultiplier(), SLASHING_MULTIPLIER, "starts at the constructor value");

        vm.prank(governance);
        oracle.scheduleBondConfig(newMultiplier, newSlashingMultiplier);

        assertEq(oracle.pendingBondMultiplier(), newMultiplier);
        assertEq(oracle.pendingSlashingMultiplier(), newSlashingMultiplier);
        assertEq(oracle.bondMultiplier(), BOND_MULTIPLIER, "not active until the delay elapses");
        assertEq(oracle.slashingMultiplier(), SLASHING_MULTIPLIER, "not active until the delay elapses");

        // Applying too early is a no-op, not a revert -- it's permissionless and harmless to call
        // speculatively.
        oracle.applyBondConfig();
        assertEq(oracle.bondMultiplier(), BOND_MULTIPLIER, "still not active after an early apply");
        assertEq(oracle.pendingBondMultiplier(), newMultiplier, "pending value survives an early apply");

        vm.roll(block.number + GOVERNANCE_DELAY);
        oracle.applyBondConfig();

        assertEq(oracle.bondMultiplier(), newMultiplier, "takes effect once the delay has elapsed");
        assertEq(oracle.slashingMultiplier(), newSlashingMultiplier, "takes effect once the delay has elapsed");
        assertEq(oracle.pendingBondMultiplier(), 0);
        assertEq(oracle.pendingSlashingMultiplier(), 0);
        assertEq(oracle.pendingBondMultiplierActiveAt(), 0);
    }

    function test_BondConfig_RescheduleBeforeMaturity_OverwritesPending() public {
        vm.startPrank(governance);
        oracle.scheduleBondConfig(BOND_MULTIPLIER + 1, 1);

        // Governance notices the mistake before the first schedule matures and corrects it -- this
        // must overwrite the still-pending schedule, not revert.
        oracle.scheduleBondConfig(BOND_MULTIPLIER + 3, 2);
        vm.stopPrank();

        assertEq(oracle.pendingBondMultiplier(), BOND_MULTIPLIER + 3, "second schedule overwrites the first");
        assertEq(oracle.pendingSlashingMultiplier(), 2, "second schedule overwrites the first");
        assertEq(oracle.bondMultiplier(), BOND_MULTIPLIER, "not active until the delay elapses");

        vm.roll(block.number + GOVERNANCE_DELAY);
        oracle.applyBondConfig();

        assertEq(oracle.bondMultiplier(), BOND_MULTIPLIER + 3, "corrected value takes effect, not the mistake");
        assertEq(oracle.slashingMultiplier(), 2, "corrected value takes effect, not the mistake");
    }

    function test_ScheduleBondConfig_RequestsInFlightKeepSnapshottedSlashAmount() public {
        uint256 newMultiplier = BOND_MULTIPLIER + 1;
        uint256 newSlashingMultiplier = 1;

        _postRequest();

        vm.prank(governance);
        oracle.scheduleBondConfig(newMultiplier, newSlashingMultiplier);
        vm.roll(block.number + GOVERNANCE_DELAY);

        // The in-flight request was created before the new config matured -- its snapshotted
        // `slashAmount`/`bondTarget` must not retroactively change even though the oracle now
        // reports the new values.
        assertEq(oracle.slashingMultiplier(), newSlashingMultiplier, "governed slashing multiplier has matured");
        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(req.slashAmount, SLASH_AMOUNT, "in-flight request keeps its originally snapshotted slash amount");
        assertEq(req.bondTarget, BOND_TARGET, "in-flight request keeps its originally snapshotted bond target");
    }

    // ============================================================
    // FEE SCHEDULE/APPLY
    // ============================================================

    function test_ScheduleFee_Zero_Reverts() public {
        vm.expectRevert(SentinelOracle.ZeroFee.selector);
        vm.prank(governance);
        oracle.scheduleFee(0);
    }

    function test_Fee_ScheduleApplyRoundTrip() public {
        uint256 newFee = REQUEST_FEE * 2;

        assertEq(oracle.fee(), REQUEST_FEE, "starts at the constructor value");

        vm.prank(governance);
        oracle.scheduleFee(newFee);

        assertEq(oracle.pendingFee(), newFee);
        assertEq(oracle.fee(), REQUEST_FEE, "not active until the delay elapses");

        // Applying too early is a no-op, not a revert -- it's permissionless and harmless to call
        // speculatively.
        oracle.applyFee();
        assertEq(oracle.fee(), REQUEST_FEE, "still not active after an early apply");
        assertEq(oracle.pendingFee(), newFee, "pending value survives an early apply");

        vm.roll(block.number + GOVERNANCE_DELAY);
        oracle.applyFee();

        assertEq(oracle.fee(), newFee, "takes effect once the delay has elapsed");
        assertEq(oracle.pendingFee(), 0);
        assertEq(oracle.pendingFeeActiveAt(), 0);
    }

    function test_Fee_RescheduleBeforeMaturity_OverwritesPending() public {
        uint256 firstFee = REQUEST_FEE * 2;
        uint256 secondFee = REQUEST_FEE * 3;

        vm.startPrank(governance);
        oracle.scheduleFee(firstFee);

        // Governance notices the mistake before `firstFee` matures and corrects it -- this must
        // overwrite the still-pending schedule, not revert.
        oracle.scheduleFee(secondFee);
        vm.stopPrank();

        assertEq(oracle.pendingFee(), secondFee, "second schedule overwrites the first");
        assertEq(oracle.fee(), REQUEST_FEE, "not active until the delay elapses");

        vm.roll(block.number + GOVERNANCE_DELAY);
        oracle.applyFee();

        assertEq(oracle.fee(), secondFee, "corrected value takes effect, not the mistake");
    }

    function test_ScheduleFee_RequestsInFlightKeepSnapshottedFee() public {
        uint256 newFee = REQUEST_FEE * 2;

        _postRequest();

        vm.prank(governance);
        oracle.scheduleFee(newFee);
        vm.roll(block.number + GOVERNANCE_DELAY);

        // The in-flight request was created before the new fee matured -- its snapshotted `fee`
        // must not retroactively change even though `oracle.fee()` now reports the new value.
        assertEq(oracle.fee(), newFee, "governed fee has matured");
        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(req.fee, REQUEST_FEE, "in-flight request keeps its originally snapshotted fee");
    }

    // ============================================================
    // DAO FEE SHARE SCHEDULE/APPLY
    // ============================================================

    function test_ScheduleDaoFeeShare_AboveDenominator_Reverts() public {
        // Precompute before arming expectRevert -- `FEE_SHARE_DENOMINATOR()` is itself an
        // external call, and vm.expectRevert only intercepts the very next one.
        uint256 tooHigh = oracle.FEE_SHARE_DENOMINATOR() + 1;
        vm.expectRevert(SentinelOracle.InvalidFeeShare.selector);
        vm.prank(governance);
        oracle.scheduleDaoFeeShare(tooHigh);
    }

    function test_DaoFeeShare_ScheduleApplyRoundTrip() public {
        uint256 newShare = 10_000;

        assertEq(oracle.daoFeeShare(), INITIAL_DAO_FEE_SHARE, "starts at the constructor value");

        vm.prank(governance);
        oracle.scheduleDaoFeeShare(newShare);

        assertEq(oracle.pendingDaoFeeShare(), newShare);
        assertEq(oracle.daoFeeShare(), INITIAL_DAO_FEE_SHARE, "not active until the delay elapses");

        // Applying too early is a no-op, not a revert -- it's permissionless and harmless to call
        // speculatively.
        oracle.applyDaoFeeShare();
        assertEq(oracle.daoFeeShare(), INITIAL_DAO_FEE_SHARE, "still not active after an early apply");
        assertEq(oracle.pendingDaoFeeShare(), newShare, "pending value survives an early apply");

        vm.roll(block.number + GOVERNANCE_DELAY);
        oracle.applyDaoFeeShare();

        assertEq(oracle.daoFeeShare(), newShare, "takes effect once the delay has elapsed");
        assertEq(oracle.pendingDaoFeeShare(), 0);
        assertEq(oracle.pendingDaoFeeShareActiveAt(), 0);
    }

    function test_DaoFeeShare_RescheduleBeforeMaturity_OverwritesPending() public {
        uint256 firstShare = 10_000;
        uint256 secondShare = 20_000;

        vm.startPrank(governance);
        oracle.scheduleDaoFeeShare(firstShare);

        // Governance notices the mistake before `firstShare` matures and corrects it -- this must
        // overwrite the still-pending schedule, not revert.
        oracle.scheduleDaoFeeShare(secondShare);
        vm.stopPrank();

        assertEq(oracle.pendingDaoFeeShare(), secondShare, "second schedule overwrites the first");
        assertEq(oracle.daoFeeShare(), INITIAL_DAO_FEE_SHARE, "not active until the delay elapses");

        vm.roll(block.number + GOVERNANCE_DELAY);
        oracle.applyDaoFeeShare();

        assertEq(oracle.daoFeeShare(), secondShare, "corrected value takes effect, not the mistake");
    }

    function test_ScheduleDaoFeeShare_RequestsInFlightKeepSnapshottedShare() public {
        uint256 newShare = 10_000;

        _postRequest();

        vm.prank(governance);
        oracle.scheduleDaoFeeShare(newShare);
        vm.roll(block.number + GOVERNANCE_DELAY);

        // The in-flight request was created before the new share matured -- its snapshotted
        // `daoFeeShare` must not retroactively change even though `oracle.daoFeeShare()` now
        // reports the new value.
        assertEq(oracle.daoFeeShare(), newShare, "governed share has matured");
        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(req.daoFeeShare, INITIAL_DAO_FEE_SHARE, "in-flight request keeps its originally snapshotted share");
    }

    // ============================================================
    // CHARTER ENS
    // ============================================================

    function test_CharterEns_RoundTrips() public view {
        assertEq(oracle.charterEns(), CHARTER_ENS);
    }

    // ============================================================
    // EAGER APPLY ON ACTIVITY (NO EXPLICIT apply* CALL NEEDED)
    // ============================================================

    function test_PostRequest_EagerlyAppliesPendingFee() public {
        uint256 newFee = REQUEST_FEE * 2;

        vm.prank(governance);
        oracle.scheduleFee(newFee);
        vm.roll(block.number + GOVERNANCE_DELAY);

        // Nobody ever calls `applyFee()` -- `postRequest` itself must flip the matured pending
        // value into storage as a side effect of touching `$feeConfig`.
        _postRequest();

        assertEq(oracle.fee(), newFee, "postRequest applies the pending fee");
        assertEq(oracle.pendingFee(), 0, "pending slot cleared without a separate apply call");
        assertEq(oracle.pendingFeeActiveAt(), 0);

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(req.fee, newFee, "new request snapshots the newly-applied fee");
        assertEq(req.bondTarget, newFee * BOND_MULTIPLIER, "bond target is derived from the newly-applied fee");
    }

    function test_PostRequest_EagerlyAppliesPendingBondMultiplier() public {
        uint256 newMultiplier = BOND_MULTIPLIER + 1;
        uint256 newSlashingMultiplier = SLASHING_MULTIPLIER + 1;

        vm.prank(governance);
        oracle.scheduleBondConfig(newMultiplier, newSlashingMultiplier);
        vm.roll(block.number + GOVERNANCE_DELAY);

        // Nobody ever calls `applyBondConfig()` -- `postRequest` itself must flip the
        // matured pending value into storage as a side effect of touching `$bondConfig`.
        _postRequest();

        assertEq(oracle.bondMultiplier(), newMultiplier, "postRequest applies the pending multiplier");
        assertEq(oracle.pendingBondMultiplier(), 0, "pending slot cleared without a separate apply call");
        assertEq(oracle.pendingBondMultiplierActiveAt(), 0);
        assertEq(
            oracle.slashingMultiplier(), newSlashingMultiplier, "postRequest applies the pending slashing multiplier"
        );
        assertEq(oracle.pendingSlashingMultiplier(), 0, "pending slot cleared without a separate apply call");

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(req.bondTarget, REQUEST_FEE * newMultiplier, "new request snapshots the newly-applied multiplier");
        assertEq(
            req.slashAmount,
            REQUEST_FEE * newSlashingMultiplier,
            "new request snapshots the newly-applied slashing multiplier"
        );
    }

    function test_PostRequest_EagerlyAppliesPendingDaoFeeShare() public {
        uint256 newShare = 10_000;

        vm.prank(governance);
        oracle.scheduleDaoFeeShare(newShare);
        vm.roll(block.number + GOVERNANCE_DELAY);

        // Nobody ever calls `applyDaoFeeShare()` -- `postRequest` itself must flip the matured
        // pending value into storage as a side effect of touching `$daoFeeShareConfig`.
        _postRequest();

        assertEq(oracle.daoFeeShare(), newShare, "postRequest applies the pending share");
        assertEq(oracle.pendingDaoFeeShare(), 0, "pending slot cleared without a separate apply call");
        assertEq(oracle.pendingDaoFeeShareActiveAt(), 0);

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(req.daoFeeShare, newShare, "new request snapshots the newly-applied share");
    }

    function test_Finalize_EagerlyAppliesPendingProtocolFundsReceiver() public {
        address newReceiver = vm.createWallet("newReceiver2").addr;

        _postRequest();
        _commit(sentinel1, true, SALT_1);
        _commit(sentinel2, true, SALT_2);
        _commit(sentinel3, true, SALT_3);
        _advancePastCommitDeadline();
        _reveal(sentinel1, true, SALT_1);
        _reveal(sentinel2, true, SALT_2);

        vm.prank(governance);
        oracle.scheduleProtocolFundsReceiver(newReceiver);
        // GOVERNANCE_DELAY (100 blocks) comfortably clears the reveal deadline too, so
        // sentinel3's non-reveal is finalizable in the same roll.
        vm.roll(block.number + GOVERNANCE_DELAY);

        uint256 newReceiverBalBefore = token.balanceOf(newReceiver);

        // Nobody ever calls `applyProtocolFundsReceiver()` -- `finalize` itself must flip the
        // matured pending receiver into storage before paying out sentinel3's unrevealed-bond
        // slash.
        oracle.finalize(REQUEST_ID);

        assertEq(oracle.protocolFundsReceiver(), newReceiver, "finalize applies the pending receiver");
        assertEq(oracle.pendingProtocolFundsReceiver(), address(0));
        assertEq(oracle.pendingProtocolFundsReceiverActiveAt(), 0);
        assertEq(
            token.balanceOf(newReceiver),
            newReceiverBalBefore + BOND_TARGET,
            "unrevealed bond slashed to the newly-applied receiver, not the stale one"
        );
    }

    // ============================================================
    // UNANIMOUS APPROVE FLOW
    // ============================================================

    function test_UnanimousApprove_FeeDistributedAndBondsReturned() public {
        _postRequest();

        vm.expectEmit(true, true, false, true);
        emit SentinelOracleCommitmentMap.Committed(REQUEST_ID, sentinel1, BOND_TARGET);
        _commit(sentinel1, true, SALT_1);
        _commit(sentinel2, true, SALT_2);
        _advancePastCommitDeadline();

        vm.expectEmit(true, true, false, true);
        emit SentinelOracleCommitmentMap.Revealed(REQUEST_ID, sentinel1, true, BOND_TARGET, "");
        _reveal(sentinel1, true, SALT_1);
        _reveal(sentinel2, true, SALT_2);

        // Both committers already revealed — finalize is callable well before revealDeadline.
        SentinelOracleRequest.Request memory pending = oracle.getRequest(REQUEST_ID);
        assertLt(block.number, pending.revealDeadline, "should be finalizing early, before the reveal deadline");

        uint256 proposerBalBefore = token.balanceOf(sponsor);

        vm.expectEmit(true, true, false, true);
        emit IOracle.OracleResult(REQUEST_ID, sponsor, "", true);
        oracle.finalize(REQUEST_ID);

        // Sponsor's fee was NOT refunded (it's distributed to sentinels).
        assertEq(token.balanceOf(sponsor), proposerBalBefore, "sponsor should not receive fee on approve");

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(SentinelOracleRequest.State.RESOLVED_APPROVED));
        assertEq(req.approveSentinelCount, 2);
        assertEq(req.denySentinelCount, 0);
        assertEq(req.revealedCount, 2);

        uint256 sentinel1BalBefore = token.balanceOf(sentinel1);
        uint256 sentinel2BalBefore = token.balanceOf(sentinel2);

        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);
        vm.prank(sentinel2);
        oracle.claim(REQUEST_ID);

        // Equal-share reward: fee / winningSideCount = 10_000 / 2 = 5_000 each.
        assertEq(
            token.balanceOf(sentinel1), sentinel1BalBefore + BOND_TARGET + REQUEST_FEE / 2, "sentinel1 claim incorrect"
        );
        assertEq(
            token.balanceOf(sentinel2), sentinel2BalBefore + BOND_TARGET + REQUEST_FEE / 2, "sentinel2 claim incorrect"
        );
    }

    function test_UnanimousApprove_DaoFeeShareCutGoesToProtocolFundsReceiver() public {
        uint256 daoShare = 10_000; // 10% of the fee
        vm.prank(governance);
        oracle.scheduleDaoFeeShare(daoShare);
        vm.roll(block.number + GOVERNANCE_DELAY);

        _postRequest();
        _commit(sentinel1, true, SALT_1);
        _commit(sentinel2, true, SALT_2);
        _advancePastCommitDeadline();
        _reveal(sentinel1, true, SALT_1);
        _reveal(sentinel2, true, SALT_2);

        uint256 receiverBalBefore = token.balanceOf(protocolFundsReceiver);
        uint256 expectedCut = REQUEST_FEE * daoShare / oracle.FEE_SHARE_DENOMINATOR();

        oracle.finalize(REQUEST_ID);

        assertEq(
            token.balanceOf(protocolFundsReceiver),
            receiverBalBefore + expectedCut,
            "DAO's cut reaches the protocol funds receiver"
        );

        uint256 sentinel1BalBefore = token.balanceOf(sentinel1);
        uint256 sentinel2BalBefore = token.balanceOf(sentinel2);

        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);
        vm.prank(sentinel2);
        oracle.claim(REQUEST_ID);

        // Winning sentinels split only the post-cut remainder of the fee.
        uint256 remainingFee = REQUEST_FEE - expectedCut;
        assertEq(
            token.balanceOf(sentinel1),
            sentinel1BalBefore + BOND_TARGET + remainingFee / 2,
            "sentinel1's reward reflects the post-cut fee"
        );
        assertEq(
            token.balanceOf(sentinel2),
            sentinel2BalBefore + BOND_TARGET + remainingFee / 2,
            "sentinel2's reward reflects the post-cut fee"
        );
    }

    // ============================================================
    // UNANIMOUS DENY FLOW
    // ============================================================

    function test_UnanimousDeny_FeeDistributedToDenySentinels() public {
        _postRequest();

        _commit(sentinel1, false, SALT_1);

        _advancePastCommitDeadline();
        _reveal(sentinel1, false, SALT_1);

        vm.expectEmit(true, true, false, true);
        emit IOracle.OracleResult(REQUEST_ID, sponsor, "", false);

        oracle.finalize(REQUEST_ID);

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(SentinelOracleRequest.State.RESOLVED_DENIED));
        assertEq(req.denySentinelCount, 1);

        uint256 balBefore = token.balanceOf(sentinel1);
        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);

        // Sole revealer on the winning side gets the whole fee.
        assertEq(
            token.balanceOf(sentinel1), balBefore + BOND_TARGET + REQUEST_FEE, "deny sentinel should receive full fee"
        );
    }

    // ============================================================
    // NO COMMITMENTS FLOW
    // ============================================================

    function test_NoCommitments_FeeRefunded() public {
        uint256 proposerBalBefore = token.balanceOf(sponsor);
        _postRequest();

        // Zero commits resolve as soon as commitDeadline passes, without waiting for revealDeadline.
        _advancePastCommitDeadline();
        SentinelOracleRequest.Request memory pending = oracle.getRequest(REQUEST_ID);
        assertLt(block.number, pending.revealDeadline, "should finalize before the reveal deadline");

        oracle.finalize(REQUEST_ID);
        assertEq(token.balanceOf(sponsor), proposerBalBefore);

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(SentinelOracleRequest.State.TIMED_OUT));
    }

    // ============================================================
    // CONFLICT -> FROZEN
    // ============================================================

    function test_Conflict_SetsStateFrozen() public {
        uint256 proposerBalBefore = token.balanceOf(sponsor);
        _postRequest();

        // sentinel1 approves, sentinel2 denies — both sides have revealed votes → conflict
        _commit(sentinel1, true, SALT_1);
        _commit(sentinel2, false, SALT_2);

        _advancePastCommitDeadline();
        _reveal(sentinel1, true, SALT_1);
        _reveal(sentinel2, false, SALT_2);

        oracle.finalize(REQUEST_ID);

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(SentinelOracleRequest.State.FROZEN), "conflicted request should be frozen");

        // ---- Phase 2: arbitration ----

        uint256 receiverBalBefore = token.balanceOf(protocolFundsReceiver);
        string memory context = "sentinel1's evidence was conclusive";

        vm.expectEmit(true, false, false, true);
        emit SentinelOracle.DisputeResolved(
            REQUEST_ID, SentinelOracleRequest.State.RESOLVED_APPROVED, BOND_TARGET, context
        );

        vm.prank(arbitrator);
        oracle.resolveDispute(REQUEST_ID, true, context);

        assertEq(token.balanceOf(sponsor), proposerBalBefore, "sponsor balance fully restored");
        assertEq(
            token.balanceOf(protocolFundsReceiver),
            receiverBalBefore + BOND_TARGET - REQUEST_FEE,
            "deny bonds slashed to protocol funds receiver, not the arbitrator"
        );
        assertEq(token.balanceOf(arbitrator), 0, "arbitrator itself receives nothing from resolveDispute");

        // Unlike the unanimous-resolution path, the sponsor's refund and the protocol funds
        // receiver's cut are carved out of the losing side's slashed bonds — the original fee is
        // untouched by resolveDispute and still flows to the winning revealer via calcFeeReward,
        // exactly as it would without a dispute. (Sole winner here, so it gets the whole fee.)
        uint256 s1Before = token.balanceOf(sentinel1);
        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);
        assertEq(
            token.balanceOf(sentinel1), s1Before + BOND_TARGET + REQUEST_FEE, "sentinel1 bond + fee reward returned"
        );

        uint256 s2Before = token.balanceOf(sentinel2);
        vm.prank(sentinel2);
        oracle.claim(REQUEST_ID);
        assertEq(token.balanceOf(sentinel2), s2Before, "sentinel2 bond slashed");
    }

    function test_Conflict_ArbitrationDaoFeeShare_ProposerRefundUnaffected() public {
        uint256 daoShare = 10_000; // 10% of the fee
        vm.prank(governance);
        oracle.scheduleDaoFeeShare(daoShare);
        vm.roll(block.number + GOVERNANCE_DELAY);

        uint256 proposerBalBefore = token.balanceOf(sponsor);
        _postRequest();

        _commit(sentinel1, true, SALT_1);
        _commit(sentinel2, false, SALT_2);
        _advancePastCommitDeadline();
        _reveal(sentinel1, true, SALT_1);
        _reveal(sentinel2, false, SALT_2);

        oracle.finalize(REQUEST_ID);

        uint256 receiverBalBefore = token.balanceOf(protocolFundsReceiver);
        uint256 expectedCut = REQUEST_FEE * daoShare / oracle.FEE_SHARE_DENOMINATOR();

        vm.prank(arbitrator);
        oracle.resolveDispute(REQUEST_ID, true, "sentinel1's evidence was conclusive");

        // The sponsor's arbitration refund stays whole -- the DAO's cut comes only out of the
        // winning sentinel's fee-equivalent reward, not this refund.
        assertEq(
            token.balanceOf(sponsor), proposerBalBefore, "sponsor balance fully restored, unaffected by daoFeeShare"
        );
        assertEq(
            token.balanceOf(protocolFundsReceiver),
            receiverBalBefore + BOND_TARGET - REQUEST_FEE + expectedCut,
            "arbitration remainder and DAO's cut both land on the protocol funds receiver"
        );

        uint256 s1Before = token.balanceOf(sentinel1);
        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);
        assertEq(
            token.balanceOf(sentinel1),
            s1Before + BOND_TARGET + (REQUEST_FEE - expectedCut),
            "winning sentinel's fee reward reflects the DAO's cut"
        );
    }

    function test_ResolveDispute_EmptyContext_Accepted() public {
        _postRequest();
        _commit(sentinel1, true, SALT_1);
        _commit(sentinel2, false, SALT_2);
        _advancePastCommitDeadline();
        _reveal(sentinel1, true, SALT_1);
        _reveal(sentinel2, false, SALT_2);
        oracle.finalize(REQUEST_ID);

        vm.prank(arbitrator);
        oracle.resolveDispute(REQUEST_ID, true, "");
    }

    // ============================================================
    // PARTIAL BOND SLASHING (ARBITRATION PATH)
    // ============================================================

    function test_Conflict_MinimumSlashingMultiplier_LosingSentinelReclaimsRemainder() public {
        // Minimum allowed slashing multiplier (1): the slash covers exactly the fee, and the
        // losing sentinel reclaims everything above that out of its bond via claim().
        vm.prank(governance);
        oracle.scheduleBondConfig(BOND_MULTIPLIER, 1);
        vm.roll(block.number + GOVERNANCE_DELAY);

        _postRequest();
        _commit(sentinel1, true, SALT_1);
        _commit(sentinel2, false, SALT_2);
        _advancePastCommitDeadline();
        _reveal(sentinel1, true, SALT_1);
        _reveal(sentinel2, false, SALT_2);
        oracle.finalize(REQUEST_ID);

        vm.prank(arbitrator);
        oracle.resolveDispute(REQUEST_ID, true, "sentinel1's evidence was conclusive");

        uint256 s2Before = token.balanceOf(sentinel2);
        vm.prank(sentinel2);
        oracle.claim(REQUEST_ID);
        assertEq(
            token.balanceOf(sentinel2),
            s2Before + (BOND_TARGET - REQUEST_FEE),
            "losing sentinel reclaims its bond above the fee-covering slash"
        );
    }

    function test_Conflict_MidRangeSlashingMultiplier_LosingSentinelReclaimsPartialRemainder() public {
        uint256 newBondMultiplier = 4;
        uint256 newSlashingMultiplier = 2;
        vm.prank(governance);
        oracle.scheduleBondConfig(newBondMultiplier, newSlashingMultiplier);
        vm.roll(block.number + GOVERNANCE_DELAY);

        _postRequest();
        SentinelOracleRequest.Request memory posted = oracle.getRequest(REQUEST_ID);
        assertEq(posted.bondTarget, REQUEST_FEE * newBondMultiplier);
        assertEq(posted.slashAmount, REQUEST_FEE * newSlashingMultiplier);

        _commit(sentinel1, true, SALT_1);
        _commit(sentinel2, false, SALT_2);
        _advancePastCommitDeadline();
        _reveal(sentinel1, true, SALT_1);
        _reveal(sentinel2, false, SALT_2);
        oracle.finalize(REQUEST_ID);

        uint256 receiverBalBefore = token.balanceOf(protocolFundsReceiver);
        vm.prank(arbitrator);
        oracle.resolveDispute(REQUEST_ID, true, "sentinel1's evidence was conclusive");

        assertEq(
            token.balanceOf(protocolFundsReceiver),
            receiverBalBefore + (REQUEST_FEE * newSlashingMultiplier) - REQUEST_FEE,
            "arbitration remainder reflects the governed slash amount, not the full bond"
        );

        uint256 s2Before = token.balanceOf(sentinel2);
        vm.prank(sentinel2);
        oracle.claim(REQUEST_ID);
        assertEq(
            token.balanceOf(sentinel2),
            s2Before + (REQUEST_FEE * newBondMultiplier - REQUEST_FEE * newSlashingMultiplier),
            "losing sentinel reclaims the unslashed remainder of its bond"
        );
    }

    function test_Conflict_WithUnrevealedCommitter_NoFundsTrappedOrDoubleSlashed() public {
        // A partial slashing multiplier makes each committer's unslashed remainder nonzero, so a
        // double-slash of sentinel3's bond (once swept as `unrevealedBond`, again deducted in
        // `claim()`) would be visible instead of masked by an already-zero remainder.
        uint256 newBondMultiplier = 4;
        uint256 newSlashingMultiplier = 2;
        vm.prank(governance);
        oracle.scheduleBondConfig(newBondMultiplier, newSlashingMultiplier);
        vm.roll(block.number + GOVERNANCE_DELAY);

        _postRequest();
        SentinelOracleRequest.Request memory posted = oracle.getRequest(REQUEST_ID);
        uint256 bondTarget = posted.bondTarget;
        uint256 slashAmount = posted.slashAmount;

        // sentinel1 approves, sentinel2 denies (conflict -> FROZEN), sentinel3 commits but never
        // reveals. `finalize()` already sweeps sentinel3's `unrevealedBond` slash to the protocol
        // funds receiver unconditionally -- including on the path to FROZEN, before the
        // `newState == FROZEN` early return -- so `resolveDispute` must not additionally count
        // sentinel3 in `slashed`, or its bond would be slashed twice and later claims would revert
        // for lack of contract balance.
        _commit(sentinel1, true, SALT_1);
        _commit(sentinel2, false, SALT_2);
        _commit(sentinel3, true, SALT_3);
        _advancePastCommitDeadline();
        _reveal(sentinel1, true, SALT_1);
        _reveal(sentinel2, false, SALT_2);
        _advancePastRevealDeadline();

        uint256 receiverBalBefore = token.balanceOf(protocolFundsReceiver);
        oracle.finalize(REQUEST_ID);

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(SentinelOracleRequest.State.FROZEN));
        assertEq(
            token.balanceOf(protocolFundsReceiver),
            receiverBalBefore + slashAmount,
            "sentinel3's unrevealed bond is swept to the protocol funds receiver even while FROZEN"
        );

        vm.prank(arbitrator);
        oracle.resolveDispute(REQUEST_ID, true, "sentinel1's evidence was conclusive");

        // `slashed` in `resolveDispute` must only cover sentinel2 (the revealed loser) --
        // sentinel3's slash was already swept above and must not be counted again.
        assertEq(
            token.balanceOf(protocolFundsReceiver),
            receiverBalBefore + slashAmount + (slashAmount - REQUEST_FEE),
            "resolveDispute's remainder covers only the revealed loser, not the already-swept unrevealed bond"
        );

        uint256 s1Before = token.balanceOf(sentinel1);
        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);
        assertEq(token.balanceOf(sentinel1), s1Before + bondTarget + REQUEST_FEE, "winner claims bond + full fee");

        uint256 s2Before = token.balanceOf(sentinel2);
        vm.prank(sentinel2);
        oracle.claim(REQUEST_ID);
        assertEq(
            token.balanceOf(sentinel2), s2Before + (bondTarget - slashAmount), "loser reclaims unslashed remainder"
        );

        // sentinel3 never revealed -- its slash was already swept via `unrevealedBond`, so
        // `claim()` must not deduct it again; it reclaims exactly `bondTarget - slashAmount`, and
        // the contract must have the balance on hand to pay it (no trapped/missing funds).
        uint256 s3Before = token.balanceOf(sentinel3);
        vm.prank(sentinel3);
        oracle.claim(REQUEST_ID);
        assertEq(
            token.balanceOf(sentinel3),
            s3Before + (bondTarget - slashAmount),
            "non-revealer reclaims its unslashed remainder without being slashed twice"
        );

        assertEq(token.balanceOf(address(oracle)), 0, "every deposited token was accounted for -- nothing trapped");
    }

    // ============================================================
    // COMMIT-REVEAL EDGE CASES
    // ============================================================

    function test_Reveal_BeforeCommitDeadline_Reverts() public {
        _postRequest();
        _commit(sentinel1, true, SALT_1);

        vm.expectRevert(SentinelOracleRequest.RevealWindowNotOpen.selector);
        _reveal(sentinel1, true, SALT_1);
    }

    function test_Reveal_AfterRevealDeadline_Reverts() public {
        _postRequest();
        _commit(sentinel1, true, SALT_1);

        _advancePastCommitDeadline();
        _advancePastRevealDeadline();

        vm.expectRevert(SentinelOracleRequest.RevealWindowClosed.selector);
        _reveal(sentinel1, true, SALT_1);
    }

    function test_Reveal_WrongSalt_Reverts() public {
        _postRequest();
        _commit(sentinel1, true, SALT_1);
        _advancePastCommitDeadline();

        vm.expectRevert(SentinelOracleCommitmentMap.InvalidReveal.selector);
        _reveal(sentinel1, true, SALT_2);
    }

    function test_Reveal_WrongVote_Reverts() public {
        _postRequest();
        _commit(sentinel1, true, SALT_1);
        _advancePastCommitDeadline();

        vm.expectRevert(SentinelOracleCommitmentMap.InvalidReveal.selector);
        _reveal(sentinel1, false, SALT_1);
    }

    function test_Reveal_ReasonEmittedVerbatim() public {
        _postRequest();
        string memory reason = "destination is blocklisted";
        _commit(sentinel1, false, SALT_1, reason);
        _advancePastCommitDeadline();

        vm.expectEmit(true, true, false, true);
        emit SentinelOracleCommitmentMap.Revealed(REQUEST_ID, sentinel1, false, BOND_TARGET, reason);
        _reveal(sentinel1, false, SALT_1, reason);
    }

    function test_Reveal_EmptyReason_Accepted() public {
        _postRequest();
        _commit(sentinel1, true, SALT_1, "");
        _advancePastCommitDeadline();

        _reveal(sentinel1, true, SALT_1, "");
    }

    function test_Reveal_WrongReason_Reverts() public {
        _postRequest();
        _commit(sentinel1, true, SALT_1, "reason A");
        _advancePastCommitDeadline();

        vm.expectRevert(SentinelOracleCommitmentMap.InvalidReveal.selector);
        _reveal(sentinel1, true, SALT_1, "reason B");
    }

    /// @notice Parity vector shared with `crates/sentinel/src/hashing.rs`'s `commit_hash_parity`
    /// test — keep both in sync if either implementation or expected hash changes.
    /// Inputs: sentinel=0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045, requestId=1, approve=true,
    /// salt=keccak256("test-salt"), reason="destination is not blocklisted".
    function test_HashCommitment_ParityWithRustImplementation() public view {
        address sentinel = 0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045;
        bytes32 requestId = bytes32(uint256(1));
        bytes32 salt = 0x8bcfa1e0aed22543ed44d41a95e315383294a18f9fb6e67ee082afcd585a6ff1;

        bytes32 hash = oracle.hashCommitment(sentinel, requestId, true, salt, "destination is not blocklisted");

        assertEq(hash, bytes32(0x109cc7dede05c71271a7347e049111921bc1f9f5b8f43d724c24ffbf4b1bdb6c));
    }

    function test_Reveal_WithoutCommit_Reverts() public {
        _postRequest();
        _advancePastCommitDeadline();

        vm.expectRevert(SentinelOracleCommitmentMap.NotCommitted.selector);
        _reveal(sentinel1, true, SALT_1);
    }

    function test_DoubleCommit_Reverts() public {
        _postRequest();
        _commit(sentinel1, true, SALT_1);

        // Precompute the hash before arming expectRevert — hashCommitment() is itself an external
        // call, and vm.expectRevert only intercepts the very next one.
        bytes32 hash = oracle.hashCommitment(sentinel1, REQUEST_ID, true, SALT_1, "");
        vm.expectRevert(SentinelOracleCommitmentMap.AlreadyCommitted.selector);
        vm.prank(sentinel1);
        oracle.commit(REQUEST_ID, hash);
    }

    function test_DoubleReveal_Reverts() public {
        _postRequest();
        _commit(sentinel1, true, SALT_1);
        _advancePastCommitDeadline();
        _reveal(sentinel1, true, SALT_1);

        vm.expectRevert(SentinelOracleCommitmentMap.AlreadyRevealed.selector);
        _reveal(sentinel1, true, SALT_1);
    }

    // ============================================================
    // PARTIAL REVEAL + NON-REVEAL SLASHING
    // ============================================================

    function test_PartialReveal_ResolvesAndSlashesNonRevealer() public {
        _postRequest();

        // Three commit approve, but only two ever reveal.
        _commit(sentinel1, true, SALT_1);
        _commit(sentinel2, true, SALT_2);
        _commit(sentinel3, true, SALT_3);

        _advancePastCommitDeadline();
        _reveal(sentinel1, true, SALT_1);
        _reveal(sentinel2, true, SALT_2);

        // revealedCount (2) != committedCount (3), so finalize must wait for the reveal deadline.
        vm.expectRevert(SentinelOracleRequest.FinalizeTooEarly.selector);
        oracle.finalize(REQUEST_ID);

        _advancePastRevealDeadline();

        uint256 receiverBalBefore = token.balanceOf(protocolFundsReceiver);
        oracle.finalize(REQUEST_ID);

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(SentinelOracleRequest.State.RESOLVED_APPROVED));
        assertEq(req.approveSentinelCount, 2);
        assertEq(req.revealedCount, 2);

        // sentinel3's committed bond (never revealed) is slashed to the protocol funds receiver,
        // not the arbitrator.
        assertEq(token.balanceOf(protocolFundsReceiver), receiverBalBefore + BOND_TARGET, "unrevealed bond slashed");

        uint256 s1Before = token.balanceOf(sentinel1);
        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);
        assertEq(token.balanceOf(sentinel1), s1Before + BOND_TARGET + REQUEST_FEE / 2, "sentinel1 claim incorrect");

        uint256 s2Before = token.balanceOf(sentinel2);
        vm.prank(sentinel2);
        oracle.claim(REQUEST_ID);
        assertEq(token.balanceOf(sentinel2), s2Before + BOND_TARGET + REQUEST_FEE / 2, "sentinel2 claim incorrect");

        // sentinel3 never revealed and its whole bond was slashed (slashingMultiplier ==
        // bondMultiplier here) -- claim() now succeeds regardless, but returns nothing.
        uint256 s3Before = token.balanceOf(sentinel3);
        vm.prank(sentinel3);
        oracle.claim(REQUEST_ID);
        assertEq(token.balanceOf(sentinel3), s3Before, "fully slashed bond leaves nothing to claim");
    }

    function test_PartialReveal_PartialSlashingMultiplier_NonRevealerReclaimsRemainder() public {
        uint256 newSlashingMultiplier = 1;
        vm.prank(governance);
        oracle.scheduleBondConfig(BOND_MULTIPLIER, newSlashingMultiplier);
        vm.roll(block.number + GOVERNANCE_DELAY);

        _postRequest();

        // Three commit approve, but only two ever reveal.
        _commit(sentinel1, true, SALT_1);
        _commit(sentinel2, true, SALT_2);
        _commit(sentinel3, true, SALT_3);

        _advancePastCommitDeadline();
        _reveal(sentinel1, true, SALT_1);
        _reveal(sentinel2, true, SALT_2);
        _advancePastRevealDeadline();

        uint256 receiverBalBefore = token.balanceOf(protocolFundsReceiver);
        oracle.finalize(REQUEST_ID);

        // Only the governed slash amount (fee-covering, at multiplier 1) is forfeited, not the
        // whole bond.
        assertEq(
            token.balanceOf(protocolFundsReceiver),
            receiverBalBefore + REQUEST_FEE * newSlashingMultiplier,
            "unrevealed bond partially slashed"
        );

        // sentinel3 never revealed, but can now reclaim the unslashed remainder of its bond --
        // previously this reverted with NotRevealed().
        uint256 s3Before = token.balanceOf(sentinel3);
        vm.prank(sentinel3);
        oracle.claim(REQUEST_ID);
        assertEq(
            token.balanceOf(sentinel3),
            s3Before + (BOND_TARGET - REQUEST_FEE * newSlashingMultiplier),
            "non-revealer reclaims the unslashed remainder of its bond"
        );
    }

    // ============================================================
    // PURE TIMEOUT (NOBODY REVEALS) — BONDS REFUNDED, NOT SLASHED
    // ============================================================

    function test_NoReveals_BondsAndFeeRefundedInFull() public {
        uint256 proposerBalBefore = token.balanceOf(sponsor);
        _postRequest();

        // Both commit, but neither ever reveals.
        _commit(sentinel1, true, SALT_1);
        _commit(sentinel2, false, SALT_2);

        _advancePastCommitDeadline();

        // Nobody revealed, so there's no early-finalize signal (revealedCount never reaches
        // committedCount) — finalize must wait for the full reveal window.
        vm.expectRevert(SentinelOracleRequest.FinalizeTooEarly.selector);
        oracle.finalize(REQUEST_ID);

        _advancePastRevealDeadline();

        uint256 receiverBalBefore = token.balanceOf(protocolFundsReceiver);
        oracle.finalize(REQUEST_ID);

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(SentinelOracleRequest.State.TIMED_OUT));
        assertEq(token.balanceOf(sponsor), proposerBalBefore, "sponsor fee refunded");

        // No established side exists, so no misbehavior can be proven against either committer —
        // nothing is slashed to the protocol funds receiver.
        assertEq(token.balanceOf(protocolFundsReceiver), receiverBalBefore, "no bonds slashed on a pure timeout");

        // Both commitments are still `Vote.PENDING`, but `claim()` succeeds anyway on `TIMED_OUT`.
        uint256 s1Before = token.balanceOf(sentinel1);
        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);
        assertEq(token.balanceOf(sentinel1), s1Before + BOND_TARGET, "sentinel1 bond refunded in full");

        uint256 s2Before = token.balanceOf(sentinel2);
        vm.prank(sentinel2);
        oracle.claim(REQUEST_ID);
        assertEq(token.balanceOf(sentinel2), s2Before + BOND_TARGET, "sentinel2 bond refunded in full");
    }

    function test_NoReveals_DoubleClaimReverts() public {
        _postRequest();
        _commit(sentinel1, true, SALT_1);
        _advancePastCommitDeadline();
        _advancePastRevealDeadline();
        oracle.finalize(REQUEST_ID);

        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);

        vm.expectRevert(SentinelOracleCommitment.AlreadyClaimed.selector);
        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);
    }

    // ============================================================
    // ARBITRATION TIMEOUT
    // ============================================================

    function _freezeRequest() internal {
        _postRequest();
        _commit(sentinel1, true, SALT_1);
        _commit(sentinel2, false, SALT_2);
        _advancePastCommitDeadline();
        _reveal(sentinel1, true, SALT_1);
        _reveal(sentinel2, false, SALT_2);
        oracle.finalize(REQUEST_ID);

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(SentinelOracleRequest.State.FROZEN));
    }

    function test_TimeoutArbitration_RevertsWhenNotFrozen() public {
        _postRequest();
        _advancePastCommitDeadline();
        _advancePastRevealDeadline();

        vm.expectRevert(SentinelOracleRequest.RequestNotFrozen.selector);
        oracle.timeoutArbitration(REQUEST_ID);
    }

    function test_TimeoutArbitration_RevertsBeforeDeadline() public {
        _freezeRequest();

        vm.expectRevert(SentinelOracleRequest.ArbitrationNotTimedOut.selector);
        oracle.timeoutArbitration(REQUEST_ID);

        vm.roll(block.number + ARBITRATION_TIMEOUT);
        vm.expectRevert(SentinelOracleRequest.ArbitrationNotTimedOut.selector);
        oracle.timeoutArbitration(REQUEST_ID);
    }

    function test_TimeoutArbitration_RefundsFeeAndBondsInFull() public {
        uint256 proposerBalBefore = token.balanceOf(sponsor);
        _freezeRequest();

        vm.roll(block.number + ARBITRATION_TIMEOUT + 1);

        uint256 receiverBalBefore = token.balanceOf(protocolFundsReceiver);

        oracle.timeoutArbitration(REQUEST_ID);

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(SentinelOracleRequest.State.TIMED_OUT));
        assertEq(token.balanceOf(sponsor), proposerBalBefore, "sponsor fee refunded in full");
        assertEq(
            token.balanceOf(protocolFundsReceiver),
            receiverBalBefore,
            "no bonds slashed on an arbitration timeout, same as any other timeout"
        );

        uint256 s1Before = token.balanceOf(sentinel1);
        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);
        assertEq(token.balanceOf(sentinel1), s1Before + BOND_TARGET, "sentinel1 bond refunded in full via claim");

        uint256 s2Before = token.balanceOf(sentinel2);
        vm.prank(sentinel2);
        oracle.claim(REQUEST_ID);
        assertEq(token.balanceOf(sentinel2), s2Before + BOND_TARGET, "sentinel2 bond refunded in full via claim");
    }

    function test_TimeoutArbitration_Permissionless() public {
        _freezeRequest();
        vm.roll(block.number + ARBITRATION_TIMEOUT + 1);

        address randomAddress = vm.createWallet("random").addr;
        vm.prank(randomAddress);
        oracle.timeoutArbitration(REQUEST_ID);

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(SentinelOracleRequest.State.TIMED_OUT));
    }

    function test_TimeoutArbitration_UnrevealedCommitter_NoDoubleRefund() public {
        _postRequest();
        SentinelOracleRequest.Request memory posted = oracle.getRequest(REQUEST_ID);
        uint256 bondTarget = posted.bondTarget;
        uint256 slashAmount = posted.slashAmount;

        // sentinel1 approves, sentinel2 denies (conflict -> FROZEN); sentinel3 commits but never
        // reveals. `finalize()` sweeps sentinel3's `unrevealedBond` slash to the protocol funds
        // receiver immediately, even on the path to FROZEN (see
        // `test_Conflict_WithUnrevealedCommitter_NoFundsTrappedOrDoubleSlashed`) -- so once the
        // dispute times out instead of being arbitrated, sentinel3 must only reclaim the unslashed
        // remainder via claim(), not their full bond, or they are refunded twice for the one bond.
        _commit(sentinel1, true, SALT_1);
        _commit(sentinel2, false, SALT_2);
        _commit(sentinel3, true, SALT_3);
        _advancePastCommitDeadline();
        _reveal(sentinel1, true, SALT_1);
        _reveal(sentinel2, false, SALT_2);
        _advancePastRevealDeadline();

        uint256 receiverBalBefore = token.balanceOf(protocolFundsReceiver);
        oracle.finalize(REQUEST_ID);
        assertEq(
            token.balanceOf(protocolFundsReceiver),
            receiverBalBefore + slashAmount,
            "sentinel3's unrevealed bond is swept even on the path to FROZEN"
        );

        vm.roll(block.number + ARBITRATION_TIMEOUT + 1);
        oracle.timeoutArbitration(REQUEST_ID);

        // sentinel1 and sentinel2 each revealed and contributed to the freeze -- an arbitration
        // timeout is nobody's fault, so they are made whole exactly like any other timeout.
        uint256 s1Before = token.balanceOf(sentinel1);
        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);
        assertEq(token.balanceOf(sentinel1), s1Before + bondTarget, "revealed sentinel1 refunded in full");

        uint256 s2Before = token.balanceOf(sentinel2);
        vm.prank(sentinel2);
        oracle.claim(REQUEST_ID);
        assertEq(token.balanceOf(sentinel2), s2Before + bondTarget, "revealed sentinel2 refunded in full");

        // sentinel3 never revealed, so its slash already happened at `finalize()` time -- claim()
        // must return only the unslashed remainder, not the full bond on top of that sweep.
        uint256 s3Before = token.balanceOf(sentinel3);
        vm.prank(sentinel3);
        oracle.claim(REQUEST_ID);
        assertEq(
            token.balanceOf(sentinel3),
            s3Before + (bondTarget - slashAmount),
            "non-revealer reclaims only the unslashed remainder, not a second full refund"
        );
    }
}
