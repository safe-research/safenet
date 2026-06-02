// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {IOracle} from "@/interfaces/IOracle.sol";
import {SentinelOracle} from "@/SentinelOracle.sol";
import {SentinelOracleRequest} from "@/libraries/SentinelOracleRequests.sol";
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
        oracle = new SentinelOracle(
            arbitrator, consensus, address(token), REQUEST_FEE, VOTING_WINDOW, GOVERNANCE_DELAY, BOND_MULTIPLIER
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
        vm.prank(consensus);
        oracle.postRequest(REQUEST_ID, proposer, address(token), REQUEST_FEE);
    }

    function _advancePastDeadline() internal {
        vm.roll(block.number + VOTING_WINDOW + 1);
    }

    // ============================================================
    // UNANIMOUS APPROVE FLOW
    // ============================================================

    function test_UnanimousApprove_FeeDistributedAndBondsReturned() public {
        _postRequest();

        // sentinel1 and sentinel2 each commit exactly BOND_TARGET.
        vm.prank(sentinel1);
        oracle.commitApprove(REQUEST_ID); // position 1
        vm.prank(sentinel2);
        oracle.commitApprove(REQUEST_ID); // position 2

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

        // Score_1 = 20_000 * 1e18 / 1 = 20_000e18
        // Score_2 = 20_000 * 1e18 / 2 = 10_000e18
        // approveTotalScore = 30_000e18
        assertEq(req.approveTotalScore, 30_000e18);

        uint256 sentinel1BalBefore = token.balanceOf(sentinel1);
        uint256 sentinel2BalBefore = token.balanceOf(sentinel2);

        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);
        vm.prank(sentinel2);
        oracle.claim(REQUEST_ID);

        // sentinel1: bond=20_000 returned + fee share = 10_000 * 20_000 / 30_000 = 6_666 (truncated)
        assertEq(token.balanceOf(sentinel1), sentinel1BalBefore + BOND_TARGET + 6_666, "sentinel1 claim incorrect");

        // sentinel2: bond=20_000 returned + fee share = 10_000 * 10_000 / 30_000 = 3_333 (truncated)
        assertEq(token.balanceOf(sentinel2), sentinel2BalBefore + BOND_TARGET + 3_333, "sentinel2 claim incorrect");
    }

    // ============================================================
    // UNANIMOUS DENY FLOW
    // ============================================================

    function test_UnanimousDeny_FeeDistributedToDenySentinels() public {
        _postRequest();

        vm.prank(sentinel1);
        oracle.commitDeny(REQUEST_ID); // position 1, single sentinel

        _advancePastDeadline();

        vm.expectEmit(true, true, false, true);
        emit IOracle.OracleResult(
            REQUEST_ID, proposer, abi.encode(SentinelOracleRequest.ResolveReason.UNANIMOUS_DENY), false
        );

        oracle.finalize(REQUEST_ID);

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(SentinelOracleRequest.State.RESOLVED_DENIED));
        assertEq(req.denyTotalScore, BOND_TARGET * 1e18);

        uint256 balBefore = token.balanceOf(sentinel1);
        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);

        // score = BOND_TARGET / 1 = BOND_TARGET; TotalScore = BOND_TARGET -> reward = REQUEST_FEE
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
        _advancePastDeadline();

        oracle.finalize(REQUEST_ID);
        assertEq(token.balanceOf(proposer), proposerBalBefore);
    }

    // ============================================================
    // CONFLICT -> FROZEN
    // ============================================================

    function test_Conflict_SetsStateFrozen() public {
        uint256 proposerBalBefore = token.balanceOf(proposer);
        _postRequest();

        // sentinel1 approves, sentinel2 denies — both sides have votes → conflict
        vm.prank(sentinel1);
        oracle.commitApprove(REQUEST_ID);

        vm.prank(sentinel2);
        oracle.commitDeny(REQUEST_ID);

        _advancePastDeadline();
        oracle.finalize(REQUEST_ID);

        SentinelOracleRequest.Request memory req = oracle.getRequest(REQUEST_ID);
        assertEq(uint256(req.state), uint256(SentinelOracleRequest.State.FROZEN), "conflicted request should be frozen");

        // ---- Phase 2: arbitration ----

        // Arbitrator resolves: approve wins.
        uint256 arbitratorBalBefore = token.balanceOf(arbitrator);

        vm.expectEmit(true, false, false, true);
        emit SentinelOracle.DisputeResolved(REQUEST_ID, SentinelOracleRequest.State.RESOLVED_APPROVED, BOND_TARGET);
        vm.expectEmit(true, true, false, true);
        emit IOracle.OracleResult(
            REQUEST_ID, proposer, abi.encode(SentinelOracleRequest.ResolveReason.ARBITRATION), true
        );

        vm.prank(arbitrator);
        oracle.resolveDispute(REQUEST_ID, true);

        // Fee is refunded to proposer from slashed amount; arbitrator receives the remainder.
        assertEq(token.balanceOf(proposer), proposerBalBefore, "proposer balance fully restored");
        assertEq(
            token.balanceOf(arbitrator),
            arbitratorBalBefore + BOND_TARGET - REQUEST_FEE,
            "deny bonds slashed to arbitrator"
        );

        // Winning approve sentinel (sentinel1) gets bond back; fee was refunded to proposer so no fee reward.
        uint256 s1Before = token.balanceOf(sentinel1);
        vm.prank(sentinel1);
        oracle.claim(REQUEST_ID);
        assertEq(token.balanceOf(sentinel1), s1Before + BOND_TARGET, "sentinel1 bond returned");

        // Losing deny sentinel (sentinel2) gets nothing - bond already slashed.
        uint256 s2Before = token.balanceOf(sentinel2);
        vm.prank(sentinel2);
        oracle.claim(REQUEST_ID);
        assertEq(token.balanceOf(sentinel2), s2Before, "sentinel2 bond slashed");
    }
}
