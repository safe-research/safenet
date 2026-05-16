// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {IOracle} from "@/interfaces/IOracle.sol";
import {SentinelOracle} from "@/SentinelOracle.sol";
import {BondConfig} from "@/libraries/BondConfig.sol";
import {SentinelMap} from "@/libraries/SentinelMap.sol";
import {SentinelOracleRequest, SentinelOracleRequestMap} from "@/libraries/SentinelOracleRequests.sol";
import {SentinelOracleCommitment, SentinelOracleCommitmentMap} from "@/libraries/SentinelOracleCommitments.sol";
import {MockERC20} from "@test/util/MockERC20.sol";

contract SentinelOracleTest is Test {
    // ============================================================
    // CONSTANTS
    // ============================================================

    uint256 constant REQUEST_FEE = 10_000; // 1 cent in a 6-decimal token (e.g. USDC)
    uint256 constant BOND_MULTIPLIER = 2;
    uint256 constant BOND_TARGET = REQUEST_FEE * BOND_MULTIPLIER; // 20_000
    uint256 constant VOTING_WINDOW = 12;
    uint256 constant GOVERNANCE_DELAY = 100;

    bytes32 constant REQUEST_ID = keccak256("request-1");

    // ============================================================
    // STATE
    // ============================================================

    SentinelOracle public oracle;
    MockERC20 public token;

    address public arbitrator;
    address public proposer;
    address public sentinel1;
    address public sentinel2;
    address public sentinel3;

    // ============================================================
    // SETUP
    // ============================================================

    function setUp() public {
        arbitrator = vm.createWallet("arbitrator").addr;
        proposer = vm.createWallet("proposer").addr;
        sentinel1 = vm.createWallet("sentinel1").addr;
        sentinel2 = vm.createWallet("sentinel2").addr;
        sentinel3 = vm.createWallet("sentinel3").addr;

        token = new MockERC20("Fee Token", "FEE");
        oracle = new SentinelOracle(
            arbitrator, address(token), REQUEST_FEE, VOTING_WINDOW, GOVERNANCE_DELAY, BOND_MULTIPLIER
        );

        // Fund accounts
        token.mint(proposer, 100_000);
        token.mint(sentinel1, 100_000);
        token.mint(sentinel2, 100_000);
        token.mint(sentinel3, 100_000);

        // Approve oracle for fee pulls
        vm.prank(proposer);
        token.approve(address(oracle), type(uint256).max);
        vm.prank(sentinel1);
        token.approve(address(oracle), type(uint256).max);
        vm.prank(sentinel2);
        token.approve(address(oracle), type(uint256).max);
        vm.prank(sentinel3);
        token.approve(address(oracle), type(uint256).max);

        // Register sentinels (active immediately by rolling past GOVERNANCE_DELAY)
        vm.startPrank(arbitrator);
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
        oracle.postRequest(REQUEST_ID);
    }

    function _advancePastDeadline() internal {
        vm.roll(block.number + VOTING_WINDOW + 1);
    }

    // ============================================================
    // UNANIMOUS APPROVE FLOW
    // ============================================================

    function test_UnanimousApprove_FeeDistributedAndBondsReturned() public {
        _postRequest();

        // sentinel1 and sentinel2 together reach the Approve threshold.
        // BOND_TARGET = 20_000; sentinel1 = 15_000, sentinel2 = 5_000 (fills the gap).
        vm.prank(sentinel1);
        oracle.commitApprove(REQUEST_ID, 15_000); // position 1
        vm.prank(sentinel2);
        oracle.commitApprove(REQUEST_ID, 5_000); // position 2

        _advancePastDeadline();

        uint256 proposerBalBefore = token.balanceOf(proposer);

        vm.expectEmit(true, true, false, true);
        emit IOracle.OracleResult(
            REQUEST_ID, proposer, abi.encode(SentinelOracleRequest.ResolveReason.UNANIMOUS_APPROVE), true
        );

        oracle.finalize(REQUEST_ID);

        // Proposer's fee was NOT refunded (it's distributed to sentinels).
        assertEq(token.balanceOf(proposer), proposerBalBefore, "proposer should not receive fee on approve");

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(SentinelOracleRequest.State.RESOLVED_APPROVED));

        // Score_1 = 15_000 * 1e18 / 1 = 15_000e18
        // Score_2 = 5_000  * 1e18 / 2 = 2_500e18
        // approveTotalScore = 17_500e18
        assertEq(req.approveTotalScore, 17_500e18);

        uint256 sentinel1BalBefore = token.balanceOf(sentinel1);
        uint256 sentinel2BalBefore = token.balanceOf(sentinel2);

        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);
        vm.prank(sentinel2);
        oracle.claim(REQUEST_ID);

        // sentinel1: bond=15_000 returned + fee share = 10_000 × 15_000/17_500 = 8_571
        assertEq(token.balanceOf(sentinel1), sentinel1BalBefore + 15_000 + 8_571, "sentinel1 claim incorrect");

        // sentinel2: bond=5_000 returned + fee share = 10_000 × 2_500/17_500 = 1_428
        assertEq(token.balanceOf(sentinel2), sentinel2BalBefore + 5_000 + 1_428, "sentinel2 claim incorrect");
    }

    function test_UnanimousApprove_DenySentinelBondReturned() public {
        _postRequest();

        vm.prank(sentinel1);
        oracle.commitDeny(REQUEST_ID, 3_000); // sub-threshold deny
        vm.prank(sentinel2);
        oracle.commitApprove(REQUEST_ID, BOND_TARGET); // full approve threshold

        _advancePastDeadline();
        oracle.finalize(REQUEST_ID);

        uint256 balBefore = token.balanceOf(sentinel1);

        vm.expectEmit(true, true, false, true);
        emit SentinelOracle.Claimed(REQUEST_ID, sentinel1, 3_000, 0);

        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);

        // Losing-side bond is returned without penalty — slashing only happens via Phase 2 arbitration.
        assertEq(token.balanceOf(sentinel1), balBefore + 3_000, "losing sentinel bond should be returned");
    }

    // ============================================================
    // UNANIMOUS DENY FLOW
    // ============================================================

    function test_UnanimousDeny_FeeDistributedToDenySentinels() public {
        _postRequest();

        vm.prank(sentinel1);
        oracle.commitDeny(REQUEST_ID, BOND_TARGET); // position 1, single sentinel fills threshold

        _advancePastDeadline();

        vm.expectEmit(true, true, false, true);
        emit IOracle.OracleResult(
            REQUEST_ID, proposer, abi.encode(SentinelOracleRequest.ResolveReason.UNANIMOUS_DENY), false
        );

        oracle.finalize(REQUEST_ID);

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(SentinelOracleRequest.State.RESOLVED_DENIED));

        uint256 balBefore = token.balanceOf(sentinel1);
        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);

        // score = BOND_TARGET / 1 = BOND_TARGET; TotalScore = BOND_TARGET → reward = REQUEST_FEE
        assertEq(
            token.balanceOf(sentinel1), balBefore + BOND_TARGET + REQUEST_FEE, "deny sentinel should receive full fee"
        );
    }

    // ============================================================
    // TIMEOUT FLOW
    // ============================================================

    function test_Timeout_FeeRefundedAndBondsReturned() public {
        _postRequest();

        // Post partial bonds on both sides — neither threshold reached
        vm.prank(sentinel1);
        oracle.commitApprove(REQUEST_ID, 1_000);
        vm.prank(sentinel2);
        oracle.commitDeny(REQUEST_ID, 2_000);

        _advancePastDeadline();

        uint256 proposerBalBefore = token.balanceOf(proposer);

        vm.expectEmit(true, true, false, true);
        emit IOracle.OracleResult(REQUEST_ID, proposer, abi.encode(SentinelOracleRequest.ResolveReason.TIMEOUT), false);

        oracle.finalize(REQUEST_ID);

        assertEq(token.balanceOf(proposer), proposerBalBefore + REQUEST_FEE, "fee should be refunded to proposer");

        // sentinels get bonds back
        uint256 s1Before = token.balanceOf(sentinel1);
        uint256 s2Before = token.balanceOf(sentinel2);

        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);
        vm.prank(sentinel2);
        oracle.claim(REQUEST_ID);

        assertEq(token.balanceOf(sentinel1), s1Before + 1_000, "sentinel1 bond refund");
        assertEq(token.balanceOf(sentinel2), s2Before + 2_000, "sentinel2 bond refund");
    }

    function test_Timeout_NoBonds_FeeRefunded() public {
        _postRequest();
        _advancePastDeadline();

        uint256 proposerBalBefore = token.balanceOf(proposer);
        oracle.finalize(REQUEST_ID);
        assertEq(token.balanceOf(proposer), proposerBalBefore + REQUEST_FEE);
    }

    // ============================================================
    // CONFLICT → FROZEN
    // ============================================================

    function test_Conflict_SetsStateFrozen() public {
        _postRequest();

        // Fill Approve threshold
        vm.prank(sentinel1);
        oracle.commitApprove(REQUEST_ID, BOND_TARGET);

        // sentinel2 and sentinel3 together fill Deny threshold
        vm.prank(sentinel2);
        oracle.commitDeny(REQUEST_ID, BOND_TARGET / 2);
        vm.prank(sentinel3);
        oracle.commitDeny(REQUEST_ID, BOND_TARGET / 2);

        _advancePastDeadline();

        vm.expectEmit(true, false, false, false);
        emit SentinelOracle.ArbitrationTriggered(REQUEST_ID);

        oracle.finalize(REQUEST_ID);

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(SentinelOracleRequest.State.FROZEN), "conflicted request should be frozen");
    }

    // ============================================================
    // GOVERNANCE — SENTINEL MANAGEMENT
    // ============================================================

    function test_AddSentinel_NotActiveBeforeDelay() public {
        address newSentinel = vm.createWallet("sentinel_pending").addr;

        uint256 addedAt = block.number;
        vm.prank(arbitrator);
        oracle.addSentinel(newSentinel);

        assertEq(oracle.sentinelActiveAt(newSentinel), addedAt + GOVERNANCE_DELAY);

        token.mint(newSentinel, 100_000);
        vm.prank(newSentinel);
        token.approve(address(oracle), type(uint256).max);
        _postRequest();

        vm.prank(newSentinel);
        vm.expectRevert(SentinelOracle.SentinelNotActive.selector);
        oracle.commitApprove(REQUEST_ID, BOND_TARGET);
    }

    function test_RemoveSentinel_PreventsCommit() public {
        vm.prank(arbitrator);
        oracle.removeSentinel(sentinel1);

        _postRequest();

        vm.prank(sentinel1);
        vm.expectRevert(SentinelOracle.SentinelNotActive.selector);
        oracle.commitApprove(REQUEST_ID, BOND_TARGET);
    }

    function test_AddSentinel_RevertsIfNotArbitrator() public {
        address newSentinel = vm.createWallet("sentinel_unauth").addr;
        vm.expectRevert(SentinelOracle.NotArbitrator.selector);
        oracle.addSentinel(newSentinel);
    }

    function test_RemoveSentinel_RevertsIfNotArbitrator() public {
        vm.expectRevert(SentinelOracle.NotArbitrator.selector);
        oracle.removeSentinel(sentinel1);
    }

    function test_AddSentinel_RevertsIfAlreadyScheduled() public {
        vm.prank(arbitrator);
        vm.expectRevert(SentinelMap.SentinelAlreadyScheduled.selector);
        oracle.addSentinel(sentinel1);
    }

    function test_RemoveSentinel_RevertsIfNotScheduled() public {
        address unknown = vm.createWallet("unknown").addr;
        vm.prank(arbitrator);
        vm.expectRevert(SentinelMap.SentinelNotScheduled.selector);
        oracle.removeSentinel(unknown);
    }

    // ============================================================
    // GOVERNANCE — BOND MULTIPLIER
    // ============================================================

    function test_BondMultiplier_InitialValue() public view {
        assertEq(oracle.bondMultiplier(), BOND_MULTIPLIER);
        assertEq(oracle.pendingBondMultiplier(), 0);
        assertEq(oracle.pendingBondMultiplierActiveAt(), 0);
    }

    function test_ScheduleBondMultiplier_UpdatesPendingAndApplies() public {
        uint256 newMultiplier = 4;

        vm.prank(arbitrator);
        oracle.scheduleBondMultiplier(newMultiplier);

        assertEq(oracle.pendingBondMultiplier(), newMultiplier);
        assertEq(oracle.pendingBondMultiplierActiveAt(), block.number + GOVERNANCE_DELAY);
        assertEq(oracle.bondMultiplier(), BOND_MULTIPLIER, "active multiplier unchanged before delay");

        vm.roll(block.number + GOVERNANCE_DELAY);
        oracle.applyBondMultiplier();

        assertEq(oracle.bondMultiplier(), newMultiplier);
        assertEq(oracle.pendingBondMultiplier(), 0);
        assertEq(oracle.pendingBondMultiplierActiveAt(), 0);
    }

    function test_ApplyBondMultiplier_RevertsBeforeDelay() public {
        vm.prank(arbitrator);
        oracle.scheduleBondMultiplier(4);

        vm.expectRevert(BondConfig.MultiplierNotReady.selector);
        oracle.applyBondMultiplier();
    }

    function test_ApplyBondMultiplier_RevertsIfNoPending() public {
        vm.expectRevert(BondConfig.NoPendingMultiplier.selector);
        oracle.applyBondMultiplier();
    }

    function test_ScheduleBondMultiplier_RevertsIfNotArbitrator() public {
        vm.expectRevert(SentinelOracle.NotArbitrator.selector);
        oracle.scheduleBondMultiplier(3);
    }

    // ============================================================
    // COMMIT VALIDATION
    // ============================================================

    function test_Commit_RevertsOnZeroBond() public {
        _postRequest();
        vm.prank(sentinel1);
        vm.expectRevert(SentinelOracle.ZeroBond.selector);
        oracle.commitApprove(REQUEST_ID, 0);
    }

    function test_Commit_RevertsIfSentinelNotActive() public {
        address inactive = vm.createWallet("inactive").addr;
        token.mint(inactive, 100_000);
        vm.prank(inactive);
        token.approve(address(oracle), type(uint256).max);

        _postRequest();

        vm.prank(inactive);
        vm.expectRevert(SentinelOracle.SentinelNotActive.selector);
        oracle.commitApprove(REQUEST_ID, BOND_TARGET);
    }

    function test_Commit_RevertsIfAlreadyCommitted() public {
        _postRequest();

        vm.prank(sentinel1);
        oracle.commitApprove(REQUEST_ID, 1_000);

        vm.prank(sentinel1);
        vm.expectRevert(SentinelOracleCommitmentMap.AlreadyCommitted.selector);
        oracle.commitApprove(REQUEST_ID, 1_000);
    }

    function test_Commit_RevertsAfterDeadline() public {
        _postRequest();
        _advancePastDeadline();

        vm.prank(sentinel1);
        vm.expectRevert(SentinelOracleRequest.VotingWindowClosed.selector);
        oracle.commitApprove(REQUEST_ID, BOND_TARGET);
    }

    function test_Commit_RevertsIfThresholdAlreadyReached() public {
        _postRequest();

        vm.prank(sentinel1);
        oracle.commitApprove(REQUEST_ID, BOND_TARGET);

        vm.prank(sentinel2);
        vm.expectRevert(SentinelOracleRequest.ThresholdAlreadyReached.selector);
        oracle.commitApprove(REQUEST_ID, 1_000);
    }

    // ============================================================
    // REQUEST / FINALIZE VALIDATION
    // ============================================================

    function test_PostRequest_RevertsOnDuplicate() public {
        _postRequest();
        vm.prank(proposer);
        vm.expectRevert(SentinelOracleRequestMap.RequestAlreadyExists.selector);
        oracle.postRequest(REQUEST_ID);
    }

    function test_Finalize_RevertsIfVotingWindowOpen() public {
        _postRequest();
        vm.expectRevert(SentinelOracleRequest.VotingWindowOpen.selector);
        oracle.finalize(REQUEST_ID);
    }

    function test_Finalize_RevertsIfAlreadyFinalized() public {
        _postRequest();
        _advancePastDeadline();
        oracle.finalize(REQUEST_ID);

        vm.expectRevert(SentinelOracleRequest.RequestNotPending.selector);
        oracle.finalize(REQUEST_ID);
    }

    // ============================================================
    // CLAIM VALIDATION
    // ============================================================

    function test_Claim_RevertsIfNothingCommitted() public {
        _postRequest();
        _advancePastDeadline();
        oracle.finalize(REQUEST_ID);

        vm.expectRevert(SentinelOracleCommitment.NothingToClaim.selector);
        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);
    }

    function test_Claim_RevertsIfAlreadyClaimed() public {
        _postRequest();
        vm.prank(sentinel1);
        oracle.commitApprove(REQUEST_ID, BOND_TARGET);
        _advancePastDeadline();
        oracle.finalize(REQUEST_ID);

        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);

        vm.prank(sentinel1);
        vm.expectRevert(SentinelOracleCommitment.AlreadyClaimed.selector);
        oracle.claim(REQUEST_ID);
    }

    function test_Claim_RevertsIfRequestNotResolved() public {
        _postRequest();
        vm.prank(sentinel1);
        oracle.commitApprove(REQUEST_ID, BOND_TARGET);

        vm.prank(sentinel1);
        vm.expectRevert(SentinelOracleRequest.RequestNotResolved.selector);
        oracle.claim(REQUEST_ID);
    }

    // ============================================================
    // VIEW FUNCTIONS
    // ============================================================

    function test_GetCommitment_ReturnsCorrectData() public {
        _postRequest();

        vm.prank(sentinel1);
        oracle.commitApprove(REQUEST_ID, 15_000);

        SentinelOracleCommitment.Commitment memory c = oracle.getCommitment(REQUEST_ID, sentinel1);
        assertEq(c.bondAmount, 15_000);
        assertEq(c.position, 1);
        assertTrue(c.approved);
        assertFalse(c.claimed);
    }
}
