// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {IOracle} from "@/interfaces/IOracle.sol";
import {ICheckerOracle} from "@/interfaces/ICheckerOracle.sol";
import {CheckerOracle} from "@/CheckerOracle.sol";
import {MockERC20} from "@test/util/MockERC20.sol";

contract CheckerOracleTest is Test {
    // ============================================================
    // CONSTANTS
    // ============================================================

    uint256 constant REQUEST_FEE = 1000e18;
    uint256 constant BOND_MULTIPLIER = 2; // small multiplier for test manageability
    uint256 constant BOND_TARGET = REQUEST_FEE * BOND_MULTIPLIER; // 2000e18
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
    address public checker1;
    address public checker2;
    address public checker3;
    address public stranger;

    // ============================================================
    // SETUP
    // ============================================================

    function setUp() public {
        arbitrator = vm.createWallet("arbitrator").addr;
        proposer = vm.createWallet("proposer").addr;
        checker1 = vm.createWallet("checker1").addr;
        checker2 = vm.createWallet("checker2").addr;
        checker3 = vm.createWallet("checker3").addr;
        stranger = vm.createWallet("stranger").addr;

        token = new MockERC20("Fee Token", "FEE");
        oracle = new CheckerOracle(
            arbitrator, address(token), REQUEST_FEE, VOTING_WINDOW, GOVERNANCE_DELAY, BOND_MULTIPLIER
        );

        // Fund accounts
        token.mint(proposer, 10_000e18);
        token.mint(checker1, 10_000e18);
        token.mint(checker2, 10_000e18);
        token.mint(checker3, 10_000e18);

        // Approve oracle for fee pulls
        vm.prank(proposer);
        token.approve(address(oracle), type(uint256).max);
        vm.prank(checker1);
        token.approve(address(oracle), type(uint256).max);
        vm.prank(checker2);
        token.approve(address(oracle), type(uint256).max);
        vm.prank(checker3);
        token.approve(address(oracle), type(uint256).max);

        // Register checkers (active immediately by rolling past GOVERNANCE_DELAY)
        vm.startPrank(arbitrator);
        oracle.addChecker(checker1);
        oracle.addChecker(checker2);
        oracle.addChecker(checker3);
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
        emit ICheckerOracle.NewRequest(REQUEST_ID, proposer, REQUEST_FEE, BOND_TARGET, block.number + VOTING_WINDOW);

        _postRequest();

        assertEq(token.balanceOf(proposer), balanceBefore - REQUEST_FEE, "fee not pulled");
        assertEq(token.balanceOf(address(oracle)), REQUEST_FEE, "fee not escrowed");

        ICheckerOracle.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(req.proposer, proposer);
        assertEq(req.fee, REQUEST_FEE);
        assertEq(req.approveBondTarget, BOND_TARGET);
        assertEq(uint256(req.state), uint256(ICheckerOracle.State.PENDING));
    }

    function test_PostRequest_DuplicateReverts() public {
        _postRequest();
        vm.expectRevert(CheckerOracle.RequestAlreadyExists.selector);
        vm.prank(proposer);
        oracle.postRequest(REQUEST_ID);
    }

    // ============================================================
    // commitApprove / commitDeny
    // ============================================================

    function test_CommitApprove_RecordsCommitmentAndPullsBond() public {
        _postRequest();

        uint256 bond = 500e18;
        uint256 balanceBefore = token.balanceOf(checker1);

        vm.expectEmit(true, true, false, true);
        emit ICheckerOracle.Committed(REQUEST_ID, checker1, true, bond, 1);

        vm.prank(checker1);
        oracle.commitApprove(REQUEST_ID, bond);

        assertEq(token.balanceOf(checker1), balanceBefore - bond, "bond not pulled");

        ICheckerOracle.Commitment memory c = oracle.getCommitment(REQUEST_ID, checker1);
        assertTrue(c.approved);
        assertEq(c.bondAmount, bond);
        assertEq(c.position, 1);
        assertFalse(c.claimed);

        ICheckerOracle.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(req.totalApproveBond, bond);
        assertEq(req.approveCheckerCount, 1);
    }

    function test_CommitDeny_RecordsCommitmentAndPullsBond() public {
        _postRequest();

        uint256 bond = 600e18;
        vm.prank(checker1);
        oracle.commitDeny(REQUEST_ID, bond);

        ICheckerOracle.Commitment memory c = oracle.getCommitment(REQUEST_ID, checker1);
        assertFalse(c.approved);
        assertEq(c.bondAmount, bond);
        assertEq(c.position, 1);

        assertEq(oracle.getRequest(REQUEST_ID).totalDenyBond, bond);
    }

    function test_Commit_ExcessBondCappedAtRemainingGap() public {
        _postRequest();

        // BOND_TARGET is 2000e18; commit more than the target — only the gap should be pulled.
        uint256 oversizedBond = BOND_TARGET + 500e18;
        uint256 balanceBefore = token.balanceOf(checker1);

        vm.prank(checker1);
        oracle.commitApprove(REQUEST_ID, oversizedBond);

        ICheckerOracle.Commitment memory c = oracle.getCommitment(REQUEST_ID, checker1);
        assertEq(c.bondAmount, BOND_TARGET, "effective bond should be capped at target");
        assertEq(token.balanceOf(checker1), balanceBefore - BOND_TARGET, "only gap amount pulled");
    }

    function test_Commit_PositionsIncrementPerSideIndependently() public {
        _postRequest();

        vm.prank(checker1);
        oracle.commitApprove(REQUEST_ID, 500e18);
        vm.prank(checker2);
        oracle.commitDeny(REQUEST_ID, 500e18);
        vm.prank(checker3);
        oracle.commitApprove(REQUEST_ID, 500e18);

        assertEq(oracle.getCommitment(REQUEST_ID, checker1).position, 1, "checker1 approve position");
        assertEq(oracle.getCommitment(REQUEST_ID, checker2).position, 1, "checker2 deny position");
        assertEq(oracle.getCommitment(REQUEST_ID, checker3).position, 2, "checker3 approve position");
    }

    function test_Commit_InactiveCheckerReverts() public {
        _postRequest();
        vm.expectRevert(CheckerOracle.CheckerNotActive.selector);
        vm.prank(stranger);
        oracle.commitApprove(REQUEST_ID, 500e18);
    }

    function test_Commit_AfterDeadlineReverts() public {
        _postRequest();
        _advancePastDeadline();
        vm.expectRevert(CheckerOracle.VotingWindowClosed.selector);
        vm.prank(checker1);
        oracle.commitApprove(REQUEST_ID, 500e18);
    }

    function test_Commit_DuplicateReverts() public {
        _postRequest();
        vm.prank(checker1);
        oracle.commitApprove(REQUEST_ID, 500e18);

        vm.expectRevert(CheckerOracle.AlreadyCommitted.selector);
        vm.prank(checker1);
        oracle.commitApprove(REQUEST_ID, 500e18);
    }

    function test_Commit_ThresholdReachedReverts() public {
        _postRequest();

        // checker1 fills the entire Approve threshold
        vm.prank(checker1);
        oracle.commitApprove(REQUEST_ID, BOND_TARGET);

        // checker2 tries to also commit Approve — threshold already met
        vm.expectRevert(CheckerOracle.ThresholdAlreadyReached.selector);
        vm.prank(checker2);
        oracle.commitApprove(REQUEST_ID, 1);
    }

    function test_Commit_ZeroBondReverts() public {
        _postRequest();
        vm.expectRevert(CheckerOracle.ZeroBond.selector);
        vm.prank(checker1);
        oracle.commitApprove(REQUEST_ID, 0);
    }

    function test_Commit_NonExistentRequestReverts() public {
        vm.expectRevert(CheckerOracle.RequestNotFound.selector);
        vm.prank(checker1);
        oracle.commitApprove(keccak256("nonexistent"), 500e18);
    }

    // ============================================================
    // UNANIMOUS APPROVE FLOW
    // ============================================================

    function test_UnanimousApprove_FeeDistributedAndBondsReturned() public {
        _postRequest();

        // checker1 and checker2 together reach the Approve threshold.
        // BOND_TARGET = 2000e18; checker1 = 1500, checker2 = 500 (fills the gap).
        vm.prank(checker1);
        oracle.commitApprove(REQUEST_ID, 1500e18); // position 1
        vm.prank(checker2);
        oracle.commitApprove(REQUEST_ID, 500e18); // position 2

        _advancePastDeadline();

        uint256 proposerBalBefore = token.balanceOf(proposer);

        vm.expectEmit(true, false, false, true);
        emit ICheckerOracle.Resolved(REQUEST_ID, true, ICheckerOracle.ResolveReason.UNANIMOUS_APPROVE);
        vm.expectEmit(true, true, false, true);
        emit IOracle.OracleResult(
            REQUEST_ID, proposer, abi.encode(ICheckerOracle.ResolveReason.UNANIMOUS_APPROVE), true
        );

        oracle.finalize(REQUEST_ID);

        // Proposer's fee was NOT refunded (it's distributed to checkers).
        assertEq(token.balanceOf(proposer), proposerBalBefore, "proposer should not receive fee on approve");

        ICheckerOracle.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(ICheckerOracle.State.RESOLVED));
        assertTrue(req.approvedOutcome);
        assertEq(req.checkerCount, 2);

        // Score_1 = 1500 × (2+1-1) = 1500×2 = 3000
        // Score_2 = 500  × (2+1-2) = 500×1  = 500
        // TotalScore = 3500
        uint256 expectedTotalScore = 1500e18 * 2 + 500e18 * 1;
        assertEq(req.totalScore, expectedTotalScore);

        uint256 checker1BalBefore = token.balanceOf(checker1);
        uint256 checker2BalBefore = token.balanceOf(checker2);

        vm.prank(checker1);
        oracle.claim(REQUEST_ID);
        vm.prank(checker2);
        oracle.claim(REQUEST_ID);

        // checker1: bond=1500 returned + fee share = REQUEST_FEE × 3000/3500
        uint256 checker1Reward = REQUEST_FEE * 3000e18 / expectedTotalScore;
        assertEq(token.balanceOf(checker1), checker1BalBefore + 1500e18 + checker1Reward, "checker1 claim incorrect");

        // checker2: bond=500 returned + fee share = REQUEST_FEE × 500/3500
        uint256 checker2Reward = REQUEST_FEE * 500e18 / expectedTotalScore;
        assertEq(token.balanceOf(checker2), checker2BalBefore + 500e18 + checker2Reward, "checker2 claim incorrect");
    }

    function test_UnanimousApprove_DenyCheckerBondSlashed() public {
        _postRequest();

        vm.prank(checker1);
        oracle.commitDeny(REQUEST_ID, 300e18); // sub-threshold deny
        vm.prank(checker2);
        oracle.commitApprove(REQUEST_ID, BOND_TARGET); // full approve threshold

        _advancePastDeadline();
        oracle.finalize(REQUEST_ID);

        uint256 balBefore = token.balanceOf(checker1);

        vm.expectEmit(true, true, false, true);
        emit ICheckerOracle.Claimed(REQUEST_ID, checker1, 0, 0);

        vm.prank(checker1);
        oracle.claim(REQUEST_ID);

        // Losing-side checker's bond is slashed — nothing returned.
        assertEq(token.balanceOf(checker1), balBefore, "losing checker bond should be slashed");
    }

    // ============================================================
    // UNANIMOUS DENY FLOW
    // ============================================================

    function test_UnanimousDeny_FeeDistributedToDenyCheckers() public {
        _postRequest();

        vm.prank(checker1);
        oracle.commitDeny(REQUEST_ID, BOND_TARGET); // position 1, single checker fills threshold

        _advancePastDeadline();

        vm.expectEmit(true, false, false, true);
        emit ICheckerOracle.Resolved(REQUEST_ID, false, ICheckerOracle.ResolveReason.UNANIMOUS_DENY);
        vm.expectEmit(true, true, false, true);
        emit IOracle.OracleResult(REQUEST_ID, proposer, abi.encode(ICheckerOracle.ResolveReason.UNANIMOUS_DENY), false);

        oracle.finalize(REQUEST_ID);

        ICheckerOracle.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(ICheckerOracle.State.RESOLVED));
        assertFalse(req.approvedOutcome);
        assertEq(req.checkerCount, 1);

        uint256 balBefore = token.balanceOf(checker1);
        vm.prank(checker1);
        oracle.claim(REQUEST_ID);

        // Single deny checker: score = BOND_TARGET × (1+1-1) = BOND_TARGET × 1 = BOND_TARGET
        // TotalScore = BOND_TARGET → reward = REQUEST_FEE × BOND_TARGET / BOND_TARGET = REQUEST_FEE
        assertEq(
            token.balanceOf(checker1), balBefore + BOND_TARGET + REQUEST_FEE, "deny checker should receive full fee"
        );
    }

    // ============================================================
    // TIMEOUT FLOW
    // ============================================================

    function test_Timeout_FeeRefundedAndBondsReturned() public {
        _postRequest();

        // Post partial bonds on both sides — neither threshold reached
        vm.prank(checker1);
        oracle.commitApprove(REQUEST_ID, 100e18);
        vm.prank(checker2);
        oracle.commitDeny(REQUEST_ID, 200e18);

        _advancePastDeadline();

        uint256 proposerBalBefore = token.balanceOf(proposer);

        vm.expectEmit(true, false, false, true);
        emit ICheckerOracle.Resolved(REQUEST_ID, false, ICheckerOracle.ResolveReason.TIMEOUT);
        vm.expectEmit(true, true, false, true);
        emit IOracle.OracleResult(REQUEST_ID, proposer, abi.encode(ICheckerOracle.ResolveReason.TIMEOUT), false);

        oracle.finalize(REQUEST_ID);

        assertEq(token.balanceOf(proposer), proposerBalBefore + REQUEST_FEE, "fee should be refunded to proposer");

        // checkers get bonds back
        uint256 c1Before = token.balanceOf(checker1);
        uint256 c2Before = token.balanceOf(checker2);

        vm.prank(checker1);
        oracle.claim(REQUEST_ID);
        vm.prank(checker2);
        oracle.claim(REQUEST_ID);

        assertEq(token.balanceOf(checker1), c1Before + 100e18, "checker1 bond refund");
        assertEq(token.balanceOf(checker2), c2Before + 200e18, "checker2 bond refund");
    }

    function test_Timeout_NoBonds_FeeRefundedNoCheckers() public {
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
        vm.prank(checker1);
        oracle.commitApprove(REQUEST_ID, BOND_TARGET);

        // checker2 and checker3 together fill Deny threshold
        vm.prank(checker2);
        oracle.commitDeny(REQUEST_ID, BOND_TARGET / 2);
        vm.prank(checker3);
        oracle.commitDeny(REQUEST_ID, BOND_TARGET / 2);

        _advancePastDeadline();
        oracle.finalize(REQUEST_ID);

        ICheckerOracle.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(ICheckerOracle.State.FROZEN), "conflicted request should be frozen");
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
        vm.prank(checker1);
        oracle.commitApprove(REQUEST_ID, 500e18);

        vm.expectRevert(CheckerOracle.RequestNotResolved.selector);
        vm.prank(checker1);
        oracle.claim(REQUEST_ID);
    }

    function test_Claim_NoCommitmentReverts() public {
        _postRequest();
        _advancePastDeadline();
        oracle.finalize(REQUEST_ID);

        vm.expectRevert(CheckerOracle.NothingToClaim.selector);
        vm.prank(checker1);
        oracle.claim(REQUEST_ID);
    }

    function test_Claim_DoubleClaimReverts() public {
        _postRequest();
        vm.prank(checker1);
        oracle.commitApprove(REQUEST_ID, BOND_TARGET);
        _advancePastDeadline();
        oracle.finalize(REQUEST_ID);

        vm.prank(checker1);
        oracle.claim(REQUEST_ID);

        vm.expectRevert(CheckerOracle.AlreadyClaimed.selector);
        vm.prank(checker1);
        oracle.claim(REQUEST_ID);
    }

    // ============================================================
    // CHECKER MANAGEMENT
    // ============================================================

    function test_AddChecker_SchedulesWithGovernanceDelay() public {
        address newChecker = vm.createWallet("newChecker").addr;
        uint256 expectedActiveAt = block.number + GOVERNANCE_DELAY;

        vm.expectEmit(true, false, false, true);
        emit ICheckerOracle.CheckerScheduled(newChecker, expectedActiveAt);

        vm.prank(arbitrator);
        oracle.addChecker(newChecker);

        assertEq(oracle.checkerActiveAt(newChecker), expectedActiveAt);
    }

    function test_AddChecker_NotActiveBeforeDelay() public {
        address newChecker = vm.createWallet("newChecker").addr;
        token.mint(newChecker, 10_000e18);
        vm.prank(newChecker);
        token.approve(address(oracle), type(uint256).max);

        vm.prank(arbitrator);
        oracle.addChecker(newChecker);

        _postRequest();

        // Still within delay window — checker should not be active
        vm.expectRevert(CheckerOracle.CheckerNotActive.selector);
        vm.prank(newChecker);
        oracle.commitApprove(REQUEST_ID, 100e18);
    }

    function test_AddChecker_ActiveAfterDelay() public {
        address newChecker = vm.createWallet("newChecker").addr;
        token.mint(newChecker, 10_000e18);
        vm.prank(newChecker);
        token.approve(address(oracle), type(uint256).max);

        vm.prank(arbitrator);
        oracle.addChecker(newChecker);
        vm.roll(block.number + GOVERNANCE_DELAY);

        _postRequest();

        // Should succeed now
        vm.prank(newChecker);
        oracle.commitApprove(REQUEST_ID, 100e18);
        assertEq(oracle.getCommitment(REQUEST_ID, newChecker).bondAmount, 100e18);
    }

    function test_AddChecker_DuplicateReverts() public {
        vm.expectRevert(CheckerOracle.CheckerAlreadyScheduled.selector);
        vm.prank(arbitrator);
        oracle.addChecker(checker1); // already added in setUp
    }

    function test_AddChecker_NotArbitratorReverts() public {
        vm.expectRevert(CheckerOracle.NotArbitrator.selector);
        vm.prank(stranger);
        oracle.addChecker(vm.createWallet("x").addr);
    }

    function test_RemoveChecker_ImmediatelyPreventsCommits() public {
        vm.prank(arbitrator);
        oracle.removeChecker(checker1);

        assertEq(oracle.checkerActiveAt(checker1), 0, "checker should have activeAt=0 after removal");

        _postRequest();

        vm.expectRevert(CheckerOracle.CheckerNotActive.selector);
        vm.prank(checker1);
        oracle.commitApprove(REQUEST_ID, 100e18);
    }

    function test_RemoveChecker_EmitsEvent() public {
        vm.expectEmit(true, false, false, false);
        emit ICheckerOracle.CheckerRemoved(checker1);

        vm.prank(arbitrator);
        oracle.removeChecker(checker1);
    }

    function test_RemoveChecker_NotScheduledReverts() public {
        address notAChecker = vm.createWallet("notAChecker").addr;
        vm.expectRevert(CheckerOracle.CheckerNotScheduled.selector);
        vm.prank(arbitrator);
        oracle.removeChecker(notAChecker);
    }

    function test_RemoveChecker_CanReAddAfterRemoval() public {
        vm.prank(arbitrator);
        oracle.removeChecker(checker1);

        // Re-add should succeed since activeAt is 0
        vm.prank(arbitrator);
        oracle.addChecker(checker1);

        assertGt(oracle.checkerActiveAt(checker1), 0, "checker should be re-scheduled");
    }

    function test_RemoveChecker_NotArbitratorReverts() public {
        vm.expectRevert(CheckerOracle.NotArbitrator.selector);
        vm.prank(stranger);
        oracle.removeChecker(checker1);
    }

    // ============================================================
    // BOND MULTIPLIER GOVERNANCE
    // ============================================================

    function test_ScheduleBondMultiplier_StagedNotAppliedImmediately() public {
        uint256 newMultiplier = 5;
        uint256 expectedActiveAt = block.number + GOVERNANCE_DELAY;

        vm.expectEmit(false, false, false, true);
        emit ICheckerOracle.BondMultiplierScheduled(newMultiplier, expectedActiveAt);

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
        emit ICheckerOracle.BondMultiplierApplied(newMultiplier);

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

        ICheckerOracle.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(req.approveBondTarget, REQUEST_FEE * newMultiplier, "new multiplier should apply to new requests");
    }

    function test_ScheduleBondMultiplier_ZeroReverts() public {
        vm.expectRevert(CheckerOracle.InvalidMultiplier.selector);
        vm.prank(arbitrator);
        oracle.scheduleBondMultiplier(0);
    }

    function test_ScheduleBondMultiplier_NotArbitratorReverts() public {
        vm.expectRevert(CheckerOracle.NotArbitrator.selector);
        vm.prank(stranger);
        oracle.scheduleBondMultiplier(5);
    }
}
