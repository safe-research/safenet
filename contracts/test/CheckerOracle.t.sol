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
        token.mint(proposer, 100_000);
        token.mint(checker1, 100_000);
        token.mint(checker2, 100_000);
        token.mint(checker3, 100_000);

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
        _postRequest();
    }

    // ============================================================
    // commitApprove / commitDeny
    // ============================================================

    function test_CommitApprove_RecordsCommitmentAndPullsBond() public {
        _postRequest();

        uint256 bond = 5_000;
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

        uint256 bond = 6_000;
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

        // BOND_TARGET is 20_000; commit more than the target — only the gap should be pulled.
        uint256 oversizedBond = BOND_TARGET + 5_000;
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
        oracle.commitApprove(REQUEST_ID, 5_000);
        vm.prank(checker2);
        oracle.commitDeny(REQUEST_ID, 5_000);
        vm.prank(checker3);
        oracle.commitApprove(REQUEST_ID, 5_000);

        assertEq(oracle.getCommitment(REQUEST_ID, checker1).position, 1, "checker1 approve position");
        assertEq(oracle.getCommitment(REQUEST_ID, checker2).position, 1, "checker2 deny position");
        assertEq(oracle.getCommitment(REQUEST_ID, checker3).position, 2, "checker3 approve position");
    }

    function test_Commit_InactiveCheckerReverts() public {
        _postRequest();
        vm.expectRevert(CheckerOracle.CheckerNotActive.selector);
        vm.prank(stranger);
        oracle.commitApprove(REQUEST_ID, 5_000);
    }

    function test_Commit_AfterDeadlineReverts() public {
        _postRequest();
        _advancePastDeadline();
        vm.expectRevert(CheckerOracle.VotingWindowClosed.selector);
        vm.prank(checker1);
        oracle.commitApprove(REQUEST_ID, 5_000);
    }

    function test_Commit_DuplicateReverts() public {
        _postRequest();
        vm.prank(checker1);
        oracle.commitApprove(REQUEST_ID, 5_000);

        vm.expectRevert(CheckerOracle.AlreadyCommitted.selector);
        vm.prank(checker1);
        oracle.commitApprove(REQUEST_ID, 5_000);
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
        oracle.commitApprove(keccak256("nonexistent"), 5_000);
    }

    // ============================================================
    // UNANIMOUS APPROVE FLOW
    // ============================================================

    function test_UnanimousApprove_FeeDistributedAndBondsReturned() public {
        _postRequest();

        // checker1 and checker2 together reach the Approve threshold.
        // BOND_TARGET = 20_000; checker1 = 15_000, checker2 = 5_000 (fills the gap).
        vm.prank(checker1);
        oracle.commitApprove(REQUEST_ID, 15_000); // position 1
        vm.prank(checker2);
        oracle.commitApprove(REQUEST_ID, 5_000); // position 2

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
        assertEq(uint256(req.state), uint256(ICheckerOracle.State.RESOLVED_APPROVED));

        // Score_1 = 15_000 / 1 = 15_000
        // Score_2 = 5_000  / 2 = 2_500
        // approveTotalScore = 17_500
        assertEq(req.approveTotalScore, 17_500);

        uint256 checker1BalBefore = token.balanceOf(checker1);
        uint256 checker2BalBefore = token.balanceOf(checker2);

        vm.prank(checker1);
        oracle.claim(REQUEST_ID);
        vm.prank(checker2);
        oracle.claim(REQUEST_ID);

        // checker1: bond=15_000 returned + fee share = 10_000 × 15_000/17_500 = 8_571
        assertEq(token.balanceOf(checker1), checker1BalBefore + 15_000 + 8_571, "checker1 claim incorrect");

        // checker2: bond=5_000 returned + fee share = 10_000 × 2_500/17_500 = 1_428
        assertEq(token.balanceOf(checker2), checker2BalBefore + 5_000 + 1_428, "checker2 claim incorrect");
    }

    function test_UnanimousApprove_DenyCheckerBondSlashed() public {
        _postRequest();

        vm.prank(checker1);
        oracle.commitDeny(REQUEST_ID, 3_000); // sub-threshold deny
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
        // TODO: reconsider whether bonds should be slashed without arbitration having taken place.
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
        assertEq(uint256(req.state), uint256(ICheckerOracle.State.RESOLVED_DENIED));

        uint256 balBefore = token.balanceOf(checker1);
        vm.prank(checker1);
        oracle.claim(REQUEST_ID);

        // score = BOND_TARGET / 1 = BOND_TARGET; TotalScore = BOND_TARGET → reward = REQUEST_FEE
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
        oracle.commitApprove(REQUEST_ID, 1_000);
        vm.prank(checker2);
        oracle.commitDeny(REQUEST_ID, 2_000);

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

        assertEq(token.balanceOf(checker1), c1Before + 1_000, "checker1 bond refund");
        assertEq(token.balanceOf(checker2), c2Before + 2_000, "checker2 bond refund");
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
        oracle.commitApprove(REQUEST_ID, 5_000);

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
        token.mint(newChecker, 100_000);
        vm.prank(newChecker);
        token.approve(address(oracle), type(uint256).max);

        vm.prank(arbitrator);
        oracle.addChecker(newChecker);

        _postRequest();

        // Still within delay window — checker should not be active
        vm.expectRevert(CheckerOracle.CheckerNotActive.selector);
        vm.prank(newChecker);
        oracle.commitApprove(REQUEST_ID, 1_000);
    }

    function test_AddChecker_ActiveAfterDelay() public {
        address newChecker = vm.createWallet("newChecker").addr;
        token.mint(newChecker, 100_000);
        vm.prank(newChecker);
        token.approve(address(oracle), type(uint256).max);

        vm.prank(arbitrator);
        oracle.addChecker(newChecker);
        vm.roll(block.number + GOVERNANCE_DELAY);

        _postRequest();

        // Should succeed now
        vm.prank(newChecker);
        oracle.commitApprove(REQUEST_ID, 1_000);
        assertEq(oracle.getCommitment(REQUEST_ID, newChecker).bondAmount, 1_000);
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
        oracle.commitApprove(REQUEST_ID, 1_000);
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
