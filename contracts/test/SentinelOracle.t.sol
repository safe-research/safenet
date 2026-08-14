// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {IOracle} from "@/interfaces/IOracle.sol";
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

    SentinelOracle public oracle;
    MockERC20 public token;

    address public arbitrator;
    address public governance;
    address public protocolFundsReceiver;
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
        governance = vm.createWallet("governance").addr;
        protocolFundsReceiver = vm.createWallet("protocolFundsReceiver").addr;
        consensus = vm.createWallet("consensus").addr;
        proposer = vm.createWallet("proposer").addr;
        sentinel1 = vm.createWallet("sentinel1").addr;
        sentinel2 = vm.createWallet("sentinel2").addr;
        sentinel3 = vm.createWallet("sentinel3").addr;

        token = new MockERC20("Fee Token", "FEE");
        oracle = new SentinelOracle(
            arbitrator,
            governance,
            protocolFundsReceiver,
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
        vm.prank(consensus);
        oracle.postRequest(REQUEST_ID, proposer, "");
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

    function test_ScheduleBondMultiplier_OnlyGovernance() public {
        address randomAddress = vm.createWallet("random").addr;

        vm.expectRevert(SentinelOracle.NotGovernance.selector);
        vm.prank(arbitrator);
        oracle.scheduleBondMultiplier(BOND_MULTIPLIER + 1);

        vm.expectRevert(SentinelOracle.NotGovernance.selector);
        vm.prank(randomAddress);
        oracle.scheduleBondMultiplier(BOND_MULTIPLIER + 1);

        vm.prank(governance);
        oracle.scheduleBondMultiplier(BOND_MULTIPLIER + 1);
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

        vm.prank(governance);
        oracle.scheduleBondMultiplier(newMultiplier);
        vm.roll(block.number + GOVERNANCE_DELAY);

        // Nobody ever calls `applyBondMultiplier()` -- `postRequest` itself must flip the
        // matured pending value into storage as a side effect of touching `$bondConfig`.
        _postRequest();

        assertEq(oracle.bondMultiplier(), newMultiplier, "postRequest applies the pending multiplier");
        assertEq(oracle.pendingBondMultiplier(), 0, "pending slot cleared without a separate apply call");
        assertEq(oracle.pendingBondMultiplierActiveAt(), 0);

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(req.bondTarget, REQUEST_FEE * newMultiplier, "new request snapshots the newly-applied multiplier");
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

        uint256 receiverBalBefore = token.balanceOf(protocolFundsReceiver);
        string memory context = "sentinel1's evidence was conclusive";

        vm.expectEmit(true, false, false, true);
        emit SentinelOracle.DisputeResolved(
            REQUEST_ID, SentinelOracleRequest.State.RESOLVED_APPROVED, BOND_TARGET, context
        );
        vm.expectEmit(true, true, false, true);
        emit IOracle.OracleResult(
            REQUEST_ID, proposer, abi.encode(SentinelOracleRequest.ResolveReason.ARBITRATION), true
        );

        vm.prank(arbitrator);
        oracle.resolveDispute(REQUEST_ID, true, context);

        assertEq(token.balanceOf(proposer), proposerBalBefore, "proposer balance fully restored");
        assertEq(
            token.balanceOf(protocolFundsReceiver),
            receiverBalBefore + BOND_TARGET - REQUEST_FEE,
            "deny bonds slashed to protocol funds receiver, not the arbitrator"
        );
        assertEq(token.balanceOf(arbitrator), 0, "arbitrator itself receives nothing from resolveDispute");

        // Unlike the unanimous-resolution path, the proposer's refund and the protocol funds
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

        // sentinel3 never revealed — its commitment is still PENDING, so claim must revert.
        vm.expectRevert(SentinelOracle.NotRevealed.selector);
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

        uint256 receiverBalBefore = token.balanceOf(protocolFundsReceiver);
        oracle.finalize(REQUEST_ID);

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(SentinelOracleRequest.State.TIMED_OUT));
        assertEq(token.balanceOf(proposer), proposerBalBefore, "proposer fee refunded");

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
}
