// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {IOracle} from "@/interfaces/IOracle.sol";
import {SentinelOracleV2} from "@/SentinelOracleV2.sol";
import {SentinelOracleRequest} from "@/libraries/SentinelOracleRequestsV2.sol";
import {SentinelOracleCommitment, SentinelOracleCommitmentMap} from "@/libraries/SentinelOracleCommitmentsV2.sol";
import {MockERC20} from "@test/util/MockERC20.sol";

// TODO(A4): rename to SentinelOracle.t.sol once the V1 contract/libraries are removed.
contract SentinelOracleV2Test is Test {
    // ============================================================
    // CONSTANTS
    // ============================================================

    uint256 constant REQUEST_FEE = 10_000; // 1 cent in a 6-decimal token (e.g. USDC)
    uint256 constant BOND_MULTIPLIER = 2;
    uint256 constant BOND_TARGET = REQUEST_FEE * BOND_MULTIPLIER; // 20_000
    uint256 constant COMMIT_WINDOW = 12;
    uint256 constant REVEAL_WINDOW = 12;
    uint256 constant GOVERNANCE_DELAY = 100;

    bytes32 constant REQUEST_ID = keccak256("request-1");
    bytes32 constant SALT_1 = keccak256("salt-1");
    bytes32 constant SALT_2 = keccak256("salt-2");
    bytes32 constant SALT_3 = keccak256("salt-3");

    // ============================================================
    // STATE
    // ============================================================

    SentinelOracleV2 public oracle;
    MockERC20 public token;

    address public arbitrator;
    address public consensus;
    address public proposer;
    address public sentinel1;
    address public sentinel2;
    address public sentinel3;

    // ============================================================
    // SETUP
    // ============================================================

    function setUp() public {
        arbitrator = vm.createWallet("arbitrator").addr;
        consensus = vm.createWallet("consensus").addr;
        proposer = vm.createWallet("proposer").addr;
        sentinel1 = vm.createWallet("sentinel1").addr;
        sentinel2 = vm.createWallet("sentinel2").addr;
        sentinel3 = vm.createWallet("sentinel3").addr;

        token = new MockERC20("Fee Token", "FEE");
        oracle = new SentinelOracleV2(
            arbitrator,
            consensus,
            address(token),
            REQUEST_FEE,
            COMMIT_WINDOW,
            REVEAL_WINDOW,
            GOVERNANCE_DELAY,
            BOND_MULTIPLIER
        );

        // Fund accounts
        token.mint(proposer, 100_000);
        token.mint(sentinel1, 100_000);
        token.mint(sentinel2, 100_000);
        token.mint(sentinel3, 100_000);

        // Approve oracle for fee/bond pulls
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
        vm.prank(consensus);
        oracle.postRequest(REQUEST_ID, proposer, "");
    }

    function _commit(address sentinel, bool approve, bytes32 salt) internal {
        bytes32 hash = oracle.hashCommitment(sentinel, REQUEST_ID, approve, salt);
        vm.prank(sentinel);
        oracle.commit(REQUEST_ID, hash);
    }

    function _reveal(address sentinel, bool approve, bytes32 salt) internal {
        vm.prank(sentinel);
        oracle.reveal(REQUEST_ID, approve, salt);
    }

    function _advancePastCommitDeadline() internal {
        vm.roll(block.number + COMMIT_WINDOW + 1);
    }

    function _advancePastRevealDeadline() internal {
        vm.roll(block.number + REVEAL_WINDOW + 1);
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
        emit SentinelOracleCommitmentMap.Revealed(REQUEST_ID, sentinel1, true, BOND_TARGET);
        _reveal(sentinel1, true, SALT_1);
        _reveal(sentinel2, true, SALT_2);

        // Both committers already revealed — finalize is callable well before revealDeadline.
        SentinelOracleRequest.Request memory pending = oracle.getRequest(REQUEST_ID);
        assertLt(block.number, pending.revealDeadline, "should be finalizing early, before the reveal deadline");

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

    // ============================================================
    // UNANIMOUS DENY FLOW
    // ============================================================

    function test_UnanimousDeny_FeeDistributedToDenySentinels() public {
        _postRequest();

        _commit(sentinel1, false, SALT_1);

        _advancePastCommitDeadline();
        _reveal(sentinel1, false, SALT_1);

        vm.expectEmit(true, true, false, true);
        emit IOracle.OracleResult(
            REQUEST_ID, proposer, abi.encode(SentinelOracleRequest.ResolveReason.UNANIMOUS_DENY), false
        );

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
        uint256 proposerBalBefore = token.balanceOf(proposer);
        _postRequest();

        // Zero commits resolve as soon as commitDeadline passes, without waiting for revealDeadline.
        _advancePastCommitDeadline();
        SentinelOracleRequest.Request memory pending = oracle.getRequest(REQUEST_ID);
        assertLt(block.number, pending.revealDeadline, "should finalize before the reveal deadline");

        oracle.finalize(REQUEST_ID);
        assertEq(token.balanceOf(proposer), proposerBalBefore);

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(SentinelOracleRequest.State.TIMED_OUT));
    }

    // ============================================================
    // CONFLICT -> FROZEN
    // ============================================================

    function test_Conflict_SetsStateFrozen() public {
        uint256 proposerBalBefore = token.balanceOf(proposer);
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

        uint256 arbitratorBalBefore = token.balanceOf(arbitrator);

        vm.expectEmit(true, false, false, true);
        emit SentinelOracleV2.DisputeResolved(REQUEST_ID, SentinelOracleRequest.State.RESOLVED_APPROVED, BOND_TARGET);
        vm.expectEmit(true, true, false, true);
        emit IOracle.OracleResult(
            REQUEST_ID, proposer, abi.encode(SentinelOracleRequest.ResolveReason.ARBITRATION), true
        );

        vm.prank(arbitrator);
        oracle.resolveDispute(REQUEST_ID, true);

        assertEq(token.balanceOf(proposer), proposerBalBefore, "proposer balance fully restored");
        assertEq(
            token.balanceOf(arbitrator),
            arbitratorBalBefore + BOND_TARGET - REQUEST_FEE,
            "deny bonds slashed to arbitrator"
        );

        // Unlike the unanimous-resolution path, the proposer's refund and the arbitrator's cut are
        // carved out of the losing side's slashed bonds — the original fee is untouched by
        // resolveDispute and still flows to the winning revealer via calcFeeReward, exactly as it
        // would without a dispute. (Sole winner here, so it gets the whole fee.)
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
        bytes32 hash = oracle.hashCommitment(sentinel1, REQUEST_ID, true, SALT_1);
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

        uint256 arbitratorBalBefore = token.balanceOf(arbitrator);
        oracle.finalize(REQUEST_ID);

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(SentinelOracleRequest.State.RESOLVED_APPROVED));
        assertEq(req.approveSentinelCount, 2);
        assertEq(req.revealedCount, 2);

        // sentinel3's committed bond (never revealed) is slashed to the arbitrator.
        assertEq(token.balanceOf(arbitrator), arbitratorBalBefore + BOND_TARGET, "unrevealed bond slashed");

        uint256 s1Before = token.balanceOf(sentinel1);
        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);
        assertEq(token.balanceOf(sentinel1), s1Before + BOND_TARGET + REQUEST_FEE / 2, "sentinel1 claim incorrect");

        uint256 s2Before = token.balanceOf(sentinel2);
        vm.prank(sentinel2);
        oracle.claim(REQUEST_ID);
        assertEq(token.balanceOf(sentinel2), s2Before + BOND_TARGET + REQUEST_FEE / 2, "sentinel2 claim incorrect");

        // sentinel3 never revealed — its commitment is still PENDING, so claim must revert.
        vm.expectRevert(SentinelOracleV2.NotRevealed.selector);
        vm.prank(sentinel3);
        oracle.claim(REQUEST_ID);
    }

    // ============================================================
    // PURE TIMEOUT (NOBODY REVEALS) — BONDS REFUNDED, NOT SLASHED
    // ============================================================

    function test_NoReveals_BondsAndFeeRefundedInFull() public {
        uint256 proposerBalBefore = token.balanceOf(proposer);
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

        uint256 arbitratorBalBefore = token.balanceOf(arbitrator);
        oracle.finalize(REQUEST_ID);

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(SentinelOracleRequest.State.TIMED_OUT));
        assertEq(token.balanceOf(proposer), proposerBalBefore, "proposer fee refunded");

        // No established side exists, so no misbehavior can be proven against either committer —
        // nothing is slashed to the arbitrator.
        assertEq(token.balanceOf(arbitrator), arbitratorBalBefore, "no bonds slashed on a pure timeout");

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
}
