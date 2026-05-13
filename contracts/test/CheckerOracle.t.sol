// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {IOracle} from "@/interfaces/IOracle.sol";
import {CheckerOracle} from "@/CheckerOracle.sol";
import {SentinelManager} from "@/SentinelManager.sol";
import {MockERC20} from "@test/util/MockERC20.sol";

contract CheckerOracleTest is Test {
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

    CheckerOracle public oracle;
    MockERC20 public token;

    address public arbitrator;
    address public proposer;
    address public sentinel1;
    address public sentinel2;
    address public sentinel3;
    address public stranger;

    // ============================================================
    // SETUP
    // ============================================================

    function setUp() public {
        arbitrator = vm.createWallet("arbitrator").addr;
        proposer = vm.createWallet("proposer").addr;
        sentinel1 = vm.createWallet("sentinel1").addr;
        sentinel2 = vm.createWallet("sentinel2").addr;
        sentinel3 = vm.createWallet("sentinel3").addr;
        stranger = vm.createWallet("stranger").addr;

        token = new MockERC20("Fee Token", "FEE");
        oracle = new CheckerOracle(
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
    // postRequest
    // ============================================================

    function test_PostRequest_LocksFeAndEmitsNewRequest() public {
        uint256 balanceBefore = token.balanceOf(proposer);

        vm.expectEmit(true, true, false, true);
        emit CheckerOracle.NewRequest(REQUEST_ID, proposer, REQUEST_FEE, BOND_TARGET, block.number + VOTING_WINDOW);

        _postRequest();

        assertEq(token.balanceOf(proposer), balanceBefore - REQUEST_FEE, "fee not pulled");
        assertEq(token.balanceOf(address(oracle)), REQUEST_FEE, "fee not escrowed");

        CheckerOracle.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(req.proposer, proposer);
        assertEq(req.fee, REQUEST_FEE);
        assertEq(req.approveBondTarget, BOND_TARGET);
        assertEq(uint256(req.state), uint256(CheckerOracle.State.PENDING));
    }

    function test_PostRequest_DuplicateReverts() public {
        _postRequest();
        vm.expectRevert(CheckerOracle.RequestAlreadyExists.selector);
        _postRequest();
    }

    // ============================================================
    // commitApprove / commitDeny
    // ============================================================

    function test_CommitApprove_RecordsCommitmentAndPullsBond() public {
        _postRequest();

        uint256 bond = 5_000;
        uint256 balanceBefore = token.balanceOf(sentinel1);

        vm.expectEmit(true, true, false, true);
        emit CheckerOracle.Committed(REQUEST_ID, sentinel1, true, bond, 1);

        vm.prank(sentinel1);
        oracle.commitApprove(REQUEST_ID, bond);

        assertEq(token.balanceOf(sentinel1), balanceBefore - bond, "bond not pulled");

        CheckerOracle.Commitment memory c = oracle.getCommitment(REQUEST_ID, sentinel1);
        assertTrue(c.approved);
        assertEq(c.bondAmount, bond);
        assertEq(c.position, 1);
        assertFalse(c.claimed);

        CheckerOracle.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(req.totalApproveBond, bond);
        assertEq(req.approveSentinelCount, 1);
    }

    function test_CommitDeny_RecordsCommitmentAndPullsBond() public {
        _postRequest();

        uint256 bond = 6_000;
        vm.prank(sentinel1);
        oracle.commitDeny(REQUEST_ID, bond);

        CheckerOracle.Commitment memory c = oracle.getCommitment(REQUEST_ID, sentinel1);
        assertFalse(c.approved);
        assertEq(c.bondAmount, bond);
        assertEq(c.position, 1);

        assertEq(oracle.getRequest(REQUEST_ID).totalDenyBond, bond);
    }

    function test_Commit_ExcessBondCappedAtRemainingGap() public {
        _postRequest();

        // BOND_TARGET is 20_000; commit more than the target — only the gap should be pulled.
        uint256 oversizedBond = BOND_TARGET + 5_000;
        uint256 balanceBefore = token.balanceOf(sentinel1);

        vm.prank(sentinel1);
        oracle.commitApprove(REQUEST_ID, oversizedBond);

        CheckerOracle.Commitment memory c = oracle.getCommitment(REQUEST_ID, sentinel1);
        assertEq(c.bondAmount, BOND_TARGET, "effective bond should be capped at target");
        assertEq(token.balanceOf(sentinel1), balanceBefore - BOND_TARGET, "only gap amount pulled");
    }

    function test_Commit_PositionsIncrementPerSideIndependently() public {
        _postRequest();

        vm.prank(sentinel1);
        oracle.commitApprove(REQUEST_ID, 5_000);
        vm.prank(sentinel2);
        oracle.commitDeny(REQUEST_ID, 5_000);
        vm.prank(sentinel3);
        oracle.commitApprove(REQUEST_ID, 5_000);

        assertEq(oracle.getCommitment(REQUEST_ID, sentinel1).position, 1, "sentinel1 approve position");
        assertEq(oracle.getCommitment(REQUEST_ID, sentinel2).position, 1, "sentinel2 deny position");
        assertEq(oracle.getCommitment(REQUEST_ID, sentinel3).position, 2, "sentinel3 approve position");
    }

    function test_Commit_InactiveSentinelReverts() public {
        _postRequest();
        vm.expectRevert(SentinelManager.SentinelNotActive.selector);
        vm.prank(stranger);
        oracle.commitApprove(REQUEST_ID, 5_000);
    }

    function test_Commit_AfterDeadlineReverts() public {
        _postRequest();
        _advancePastDeadline();
        vm.expectRevert(CheckerOracle.VotingWindowClosed.selector);
        vm.prank(sentinel1);
        oracle.commitApprove(REQUEST_ID, 5_000);
    }

    function test_Commit_DuplicateReverts() public {
        _postRequest();
        vm.prank(sentinel1);
        oracle.commitApprove(REQUEST_ID, 5_000);

        vm.expectRevert(CheckerOracle.AlreadyCommitted.selector);
        vm.prank(sentinel1);
        oracle.commitApprove(REQUEST_ID, 5_000);
    }

    function test_Commit_ThresholdReachedReverts() public {
        _postRequest();

        // sentinel1 fills the entire Approve threshold
        vm.prank(sentinel1);
        oracle.commitApprove(REQUEST_ID, BOND_TARGET);

        // sentinel2 tries to also commit Approve — threshold already met
        vm.expectRevert(CheckerOracle.ThresholdAlreadyReached.selector);
        vm.prank(sentinel2);
        oracle.commitApprove(REQUEST_ID, 1);
    }

    function test_Commit_ZeroBondReverts() public {
        _postRequest();
        vm.expectRevert(CheckerOracle.ZeroBond.selector);
        vm.prank(sentinel1);
        oracle.commitApprove(REQUEST_ID, 0);
    }

    function test_Commit_NonExistentRequestReverts() public {
        vm.expectRevert(CheckerOracle.RequestNotFound.selector);
        vm.prank(sentinel1);
        oracle.commitApprove(keccak256("nonexistent"), 5_000);
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

        vm.expectEmit(true, false, false, true);
        emit CheckerOracle.Resolved(REQUEST_ID, true, CheckerOracle.ResolveReason.UNANIMOUS_APPROVE);
        vm.expectEmit(true, true, false, true);
        emit IOracle.OracleResult(REQUEST_ID, proposer, abi.encode(CheckerOracle.ResolveReason.UNANIMOUS_APPROVE), true);

        oracle.finalize(REQUEST_ID);

        // Proposer's fee was NOT refunded (it's distributed to sentinels).
        assertEq(token.balanceOf(proposer), proposerBalBefore, "proposer should not receive fee on approve");

        CheckerOracle.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(CheckerOracle.State.RESOLVED_APPROVED));

        // Score_1 = 15_000 / 1 = 15_000
        // Score_2 = 5_000  / 2 = 2_500
        // approveTotalScore = 17_500
        assertEq(req.approveTotalScore, 17_500);

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
        emit CheckerOracle.Claimed(REQUEST_ID, sentinel1, 3_000, 0);

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

        vm.expectEmit(true, false, false, true);
        emit CheckerOracle.Resolved(REQUEST_ID, false, CheckerOracle.ResolveReason.UNANIMOUS_DENY);
        vm.expectEmit(true, true, false, true);
        emit IOracle.OracleResult(REQUEST_ID, proposer, abi.encode(CheckerOracle.ResolveReason.UNANIMOUS_DENY), false);

        oracle.finalize(REQUEST_ID);

        CheckerOracle.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(CheckerOracle.State.RESOLVED_DENIED));

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

        vm.expectEmit(true, false, false, true);
        emit CheckerOracle.Resolved(REQUEST_ID, false, CheckerOracle.ResolveReason.TIMEOUT);
        vm.expectEmit(true, true, false, true);
        emit IOracle.OracleResult(REQUEST_ID, proposer, abi.encode(CheckerOracle.ResolveReason.TIMEOUT), false);

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

    function test_Timeout_NoBonds_FeeRefundedNoSentinels() public {
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
        oracle.finalize(REQUEST_ID);

        CheckerOracle.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(CheckerOracle.State.FROZEN), "conflicted request should be frozen");
    }

    // ============================================================
    // finalize GUARDS
    // ============================================================

    function test_Finalize_BeforeDeadlineReverts() public {
        _postRequest();
        vm.expectRevert(CheckerOracle.VotingWindowOpen.selector);
        oracle.finalize(REQUEST_ID);
    }

    function test_Finalize_NonExistentRequestReverts() public {
        vm.expectRevert(CheckerOracle.RequestNotFound.selector);
        oracle.finalize(keccak256("nonexistent"));
    }

    function test_Finalize_AlreadyResolvedReverts() public {
        _postRequest();
        _advancePastDeadline();
        oracle.finalize(REQUEST_ID);

        vm.expectRevert(CheckerOracle.RequestNotPending.selector);
        oracle.finalize(REQUEST_ID);
    }

    // ============================================================
    // claim GUARDS
    // ============================================================

    function test_Claim_BeforeResolveReverts() public {
        _postRequest();
        vm.prank(sentinel1);
        oracle.commitApprove(REQUEST_ID, 5_000);

        vm.expectRevert(CheckerOracle.RequestNotResolved.selector);
        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);
    }

    function test_Claim_NoCommitmentReverts() public {
        _postRequest();
        _advancePastDeadline();
        oracle.finalize(REQUEST_ID);

        vm.expectRevert(CheckerOracle.NothingToClaim.selector);
        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);
    }

    function test_Claim_DoubleClaimReverts() public {
        _postRequest();
        vm.prank(sentinel1);
        oracle.commitApprove(REQUEST_ID, BOND_TARGET);
        _advancePastDeadline();
        oracle.finalize(REQUEST_ID);

        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);

        vm.expectRevert(CheckerOracle.AlreadyClaimed.selector);
        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);
    }

    // ============================================================
    // SENTINEL MANAGEMENT
    // ============================================================

    function test_AddSentinel_SchedulesWithGovernanceDelay() public {
        address newSentinel = vm.createWallet("newSentinel").addr;
        uint256 expectedActiveAt = block.number + GOVERNANCE_DELAY;

        vm.expectEmit(true, false, false, true);
        emit SentinelManager.SentinelScheduled(newSentinel, expectedActiveAt);

        vm.prank(arbitrator);
        oracle.addSentinel(newSentinel);

        assertEq(oracle.sentinelActiveAt(newSentinel), expectedActiveAt);
    }

    function test_AddSentinel_NotActiveBeforeDelay() public {
        address newSentinel = vm.createWallet("newSentinel").addr;
        token.mint(newSentinel, 100_000);
        vm.prank(newSentinel);
        token.approve(address(oracle), type(uint256).max);

        vm.prank(arbitrator);
        oracle.addSentinel(newSentinel);

        _postRequest();

        // Still within delay window — sentinel should not be active
        vm.expectRevert(SentinelManager.SentinelNotActive.selector);
        vm.prank(newSentinel);
        oracle.commitApprove(REQUEST_ID, 1_000);
    }

    function test_AddSentinel_ActiveAfterDelay() public {
        address newSentinel = vm.createWallet("newSentinel").addr;
        token.mint(newSentinel, 100_000);
        vm.prank(newSentinel);
        token.approve(address(oracle), type(uint256).max);

        vm.prank(arbitrator);
        oracle.addSentinel(newSentinel);
        vm.roll(block.number + GOVERNANCE_DELAY);

        _postRequest();

        // Should succeed now
        vm.prank(newSentinel);
        oracle.commitApprove(REQUEST_ID, 1_000);
        assertEq(oracle.getCommitment(REQUEST_ID, newSentinel).bondAmount, 1_000);
    }

    function test_AddSentinel_DuplicateReverts() public {
        vm.expectRevert(SentinelManager.SentinelAlreadyScheduled.selector);
        vm.prank(arbitrator);
        oracle.addSentinel(sentinel1); // already added in setUp
    }

    function test_AddSentinel_NotArbitratorReverts() public {
        vm.expectRevert(SentinelManager.NotArbitrator.selector);
        vm.prank(stranger);
        oracle.addSentinel(vm.createWallet("x").addr);
    }

    function test_RemoveSentinel_ImmediatelyPreventsCommits() public {
        vm.prank(arbitrator);
        oracle.removeSentinel(sentinel1);

        assertEq(oracle.sentinelActiveAt(sentinel1), 0, "sentinel should have activeAt=0 after removal");

        _postRequest();

        vm.expectRevert(SentinelManager.SentinelNotActive.selector);
        vm.prank(sentinel1);
        oracle.commitApprove(REQUEST_ID, 1_000);
    }

    function test_RemoveSentinel_EmitsEvent() public {
        vm.expectEmit(true, false, false, false);
        emit SentinelManager.SentinelRemoved(sentinel1);

        vm.prank(arbitrator);
        oracle.removeSentinel(sentinel1);
    }

    function test_RemoveSentinel_NotScheduledReverts() public {
        address notASentinel = vm.createWallet("notASentinel").addr;
        vm.expectRevert(SentinelManager.SentinelNotScheduled.selector);
        vm.prank(arbitrator);
        oracle.removeSentinel(notASentinel);
    }

    function test_RemoveSentinel_CanReAddAfterRemoval() public {
        vm.prank(arbitrator);
        oracle.removeSentinel(sentinel1);

        // Re-add should succeed since activeAt is 0
        vm.prank(arbitrator);
        oracle.addSentinel(sentinel1);

        assertGt(oracle.sentinelActiveAt(sentinel1), 0, "sentinel should be re-scheduled");
    }

    function test_RemoveSentinel_NotArbitratorReverts() public {
        vm.expectRevert(SentinelManager.NotArbitrator.selector);
        vm.prank(stranger);
        oracle.removeSentinel(sentinel1);
    }

    // ============================================================
    // BOND MULTIPLIER GOVERNANCE
    // ============================================================

    function test_ScheduleBondMultiplier_StagedNotAppliedImmediately() public {
        uint256 newMultiplier = 5;
        uint256 expectedActiveAt = block.number + GOVERNANCE_DELAY;

        vm.expectEmit(false, false, false, true);
        emit CheckerOracle.BondMultiplierScheduled(newMultiplier, expectedActiveAt);

        vm.prank(arbitrator);
        oracle.scheduleBondMultiplier(newMultiplier);

        assertEq(oracle.bondMultiplier(), BOND_MULTIPLIER, "multiplier should not change immediately");
        assertEq(oracle.pendingBondMultiplier(), newMultiplier);
        assertEq(oracle.pendingBondMultiplierActiveAt(), expectedActiveAt);
    }

    function test_ApplyBondMultiplier_BeforeDelayReverts() public {
        vm.prank(arbitrator);
        oracle.scheduleBondMultiplier(5);

        vm.expectRevert(CheckerOracle.MultiplierNotReady.selector);
        oracle.applyBondMultiplier();
    }

    function test_ApplyBondMultiplier_AfterDelaySucceeds() public {
        uint256 newMultiplier = 5;
        vm.prank(arbitrator);
        oracle.scheduleBondMultiplier(newMultiplier);

        vm.roll(block.number + GOVERNANCE_DELAY);

        vm.expectEmit(false, false, false, true);
        emit CheckerOracle.BondMultiplierApplied(newMultiplier);

        oracle.applyBondMultiplier();

        assertEq(oracle.bondMultiplier(), newMultiplier, "multiplier should be updated");
        assertEq(oracle.pendingBondMultiplier(), 0, "pending multiplier should be cleared");
        assertEq(oracle.pendingBondMultiplierActiveAt(), 0, "active-at should be cleared");
    }

    function test_ApplyBondMultiplier_NoPendingReverts() public {
        vm.expectRevert(CheckerOracle.NoPendingMultiplier.selector);
        oracle.applyBondMultiplier();
    }

    function test_ScheduleBondMultiplier_NewBondTargetUsedForNextRequest() public {
        uint256 newMultiplier = 3;
        vm.prank(arbitrator);
        oracle.scheduleBondMultiplier(newMultiplier);
        vm.roll(block.number + GOVERNANCE_DELAY);
        oracle.applyBondMultiplier();

        vm.prank(proposer);
        oracle.postRequest(REQUEST_ID);

        CheckerOracle.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(req.approveBondTarget, REQUEST_FEE * newMultiplier, "new multiplier should apply to new requests");
    }

    function test_ScheduleBondMultiplier_ZeroReverts() public {
        vm.expectRevert(CheckerOracle.InvalidMultiplier.selector);
        vm.prank(arbitrator);
        oracle.scheduleBondMultiplier(0);
    }

    function test_ScheduleBondMultiplier_NotArbitratorReverts() public {
        vm.expectRevert(SentinelManager.NotArbitrator.selector);
        vm.prank(stranger);
        oracle.scheduleBondMultiplier(5);
    }
}
