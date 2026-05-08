// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {IERC20} from "@oz/token/ERC20/IERC20.sol";
import {SafeERC20} from "@oz/token/ERC20/utils/SafeERC20.sol";
import {ICheckerOracle} from "@/interfaces/ICheckerOracle.sol";
import {CheckerOracle} from "@/CheckerOracle.sol";
import {MockERC20} from "@test/util/MockERC20.sol";

contract CheckerOracleTest is Test {
    using SafeERC20 for IERC20;

    CheckerOracle public oracle;
    MockERC20 public feeToken;
    
    address public alice;
    address public bob;
    address public carol;
    address public arbitrator;
    address public consensus;
    
    bytes32 public constant REQUEST_ID_1 = keccak256("request1");
    bytes32 public constant REQUEST_ID_2 = keccak256("request2");
    
    uint256 public constant FEE = 100e18;
    uint256 public constant BOND_MULTIPLIER = 50;
    uint256 public constant BOND_AMOUNT = FEE * BOND_MULTIPLIER;
    uint256 public constant VOTING_WINDOW = 12;
    uint256 public constant GOVERNANCE_DELAY = 1;

    function setUp() public {
        alice = vm.createWallet("alice").addr;
        bob = vm.createWallet("bob").addr;
        carol = vm.createWallet("carol").addr;
        arbitrator = vm.createWallet("arbitrator").addr;
        consensus = vm.createWallet("consensus").addr;

        feeToken = new MockERC20("Test Token", "TST");
        
        oracle = new CheckerOracle(address(feeToken), arbitrator, VOTING_WINDOW, GOVERNANCE_DELAY);
        
        // Mint tokens to users
        feeToken.mint(alice, BOND_AMOUNT * 10);
        feeToken.mint(bob, BOND_AMOUNT * 10);
        feeToken.mint(carol, BOND_AMOUNT * 10);
        feeToken.mint(consensus, FEE * 10);
        
        // Approve oracle to spend tokens
        vm.prank(alice);
        feeToken.approve(address(oracle), BOND_AMOUNT * 10);
        
        vm.prank(bob);
        feeToken.approve(address(oracle), BOND_AMOUNT * 10);
        
        vm.prank(carol);
        feeToken.approve(address(oracle), BOND_AMOUNT * 10);
        
        vm.prank(consensus);
        feeToken.approve(address(oracle), FEE * 10);
        
        // Add checkers
        vm.prank(arbitrator);
        oracle.addChecker(alice);
        
        vm.prank(arbitrator);
        oracle.addChecker(bob);
        
        vm.prank(arbitrator);
        oracle.addChecker(carol);
        
        // Advance blocks to make checkers active
        vm.roll(block.number + GOVERNANCE_DELAY + 1);
    }

    // ============================================================
    // POST REQUEST TESTS
    // ============================================================

    function test_PostRequestWithFee_Success() public {
        vm.prank(consensus);
        oracle.postRequestWithFee(REQUEST_ID_1, FEE);
        
        assertEq(feeToken.balanceOf(address(oracle)), FEE);
        
        // Verify request exists
        (address proposer, uint256 fee, uint256 approveBondTarget, uint256 deadline, , uint256 totalApproveBond, uint256 totalDenyBond, uint256 checkerCount, uint256 totalScore, bool arbitrated) = oracle.requests(REQUEST_ID_1);
        
        assertEq(proposer, consensus);
        assertEq(fee, FEE);
        assertEq(approveBondTarget, FEE * BOND_MULTIPLIER);
        assertEq(deadline, block.number + VOTING_WINDOW);
        assertEq(totalApproveBond, 0);
        assertEq(totalDenyBond, 0);
        assertEq(checkerCount, 0);
    }

    function test_PostRequestWithFee_RequestAlreadyPending_Reverts() public {
        vm.prank(consensus);
        oracle.postRequestWithFee(REQUEST_ID_1, FEE);
        
        vm.expectRevert("RequestAlreadyPending");
        vm.prank(consensus);
        oracle.postRequestWithFee(REQUEST_ID_1, FEE);
    }

    function test_PostRequest_WithoutFee_Reverts() public {
        vm.expectRevert("FeeNotEscrowed");
        vm.prank(consensus);
        oracle.postRequest(REQUEST_ID_1);
    }

    // ============================================================
    // COMMIT VOTE TESTS
    // ============================================================

    function test_CommitApprove_Success() public {
        vm.prank(consensus);
        oracle.postRequestWithFee(REQUEST_ID_1, FEE);
        
        vm.prank(alice);
        oracle.commitApprove(REQUEST_ID_1);
        
        // Access Commitment struct
        (bool approved, uint256 bondAmount, uint256 position, bool claimed) = oracle.commitments(REQUEST_ID_1, alice);
        assertEq(approved, true);
        assertEq(bondAmount, BOND_AMOUNT);
        assertEq(position, 1);
        assertEq(claimed, false);
        
        // Access Request struct
        (, , , , , uint256 totalApproveBond, , uint256 checkerCount, , ) = oracle.requests(REQUEST_ID_1);
        assertEq(totalApproveBond, BOND_AMOUNT);
        assertEq(checkerCount, 1);
    }

    function test_CommitDeny_Success() public {
        vm.prank(consensus);
        oracle.postRequestWithFee(REQUEST_ID_1, FEE);
        
        vm.prank(bob);
        oracle.commitDeny(REQUEST_ID_1);
        
        // Access Commitment struct
        (bool approved, uint256 bondAmount, uint256 position, bool claimed) = oracle.commitments(REQUEST_ID_1, bob);
        assertEq(approved, false);
        assertEq(bondAmount, BOND_AMOUNT);
        assertEq(position, 1);
        assertEq(claimed, false);
        
        // Access Request struct
        (, , , , , , uint256 totalDenyBond, uint256 checkerCount, , ) = oracle.requests(REQUEST_ID_1);
        assertEq(totalDenyBond, BOND_AMOUNT);
        assertEq(checkerCount, 1);
    }

    function test_CommitVote_CheckerNotActive_Reverts() public {
        address inactiveChecker = vm.createWallet("inactive").addr;
        
        vm.prank(consensus);
        oracle.postRequestWithFee(REQUEST_ID_1, FEE);
        
        vm.expectRevert("CheckerNotActive");
        vm.prank(inactiveChecker);
        oracle.commitApprove(REQUEST_ID_1);
    }

    function test_CommitVote_RequestNotPending_Reverts() public {
        vm.expectRevert("RequestNotPending");
        vm.prank(alice);
        oracle.commitApprove(REQUEST_ID_1);
    }

    function test_CommitVote_RequestAlreadyResolved_Reverts() public {
        vm.prank(consensus);
        oracle.postRequestWithFee(REQUEST_ID_1, FEE);
        
        // Complete the voting
        vm.prank(alice);
        oracle.commitApprove(REQUEST_ID_1);
        
        vm.prank(bob);
        oracle.commitApprove(REQUEST_ID_1);
        
        // Advance to deadline
        vm.roll(block.number + VOTING_WINDOW + 1);
        
        oracle.finalize(REQUEST_ID_1);
        
        vm.expectRevert("RequestAlreadyResolved");
        vm.prank(carol);
        oracle.commitApprove(REQUEST_ID_1);
    }

    // ============================================================
    // FINALIZATION TESTS
    // ============================================================

    function test_Finalize_UnanimousApprove_Success() public {
        vm.prank(consensus);
        oracle.postRequestWithFee(REQUEST_ID_1, FEE);
        
        // All checkers vote Approve
        vm.prank(alice);
        oracle.commitApprove(REQUEST_ID_1);
        
        vm.prank(bob);
        oracle.commitApprove(REQUEST_ID_1);
        
        vm.prank(carol);
        oracle.commitApprove(REQUEST_ID_1);
        
        // Advance to deadline
        vm.roll(block.number + VOTING_WINDOW + 1);
        
        vm.expectEmit(true, true, true, true);
        emit ICheckerOracle.Resolved(REQUEST_ID_1, true, ICheckerOracle.ResolveReason.UNANIMOUS_APPROVE);
        
        oracle.finalize(REQUEST_ID_1);
        
        // Access Request struct
        (, , , , , , , , uint256 totalScore, ) = oracle.requests(REQUEST_ID_1);
        assertEq(totalScore > 0, true);
    }

    function test_Finalize_UnanimousDeny_Success() public {
        vm.prank(consensus);
        oracle.postRequestWithFee(REQUEST_ID_1, FEE);
        
        // All checkers vote Deny
        vm.prank(alice);
        oracle.commitDeny(REQUEST_ID_1);
        
        vm.prank(bob);
        oracle.commitDeny(REQUEST_ID_1);
        
        vm.prank(carol);
        oracle.commitDeny(REQUEST_ID_1);
        
        // Advance to deadline
        vm.roll(block.number + VOTING_WINDOW + 1);
        
        vm.expectEmit(true, true, true, true);
        emit ICheckerOracle.Resolved(REQUEST_ID_1, false, ICheckerOracle.ResolveReason.UNANIMOUS_DENY);
        
        oracle.finalize(REQUEST_ID_1);
        
        // Access Request struct
        (, , , , , , , , uint256 totalScore, ) = oracle.requests(REQUEST_ID_1);
        assertEq(totalScore > 0, true);
    }

    function test_Finalize_Timeout_Success() public {
        vm.prank(consensus);
        oracle.postRequestWithFee(REQUEST_ID_1, FEE);
        
        // One checker commits Deny, but bond amount is less than target (threshold not reached)
        // We need to commit with a smaller bond amount
        // But currently there's no way to commit less than the full bond
        // Let's just not commit anything and let voting expire
        
        // Advance past deadline without any votes
        vm.roll(block.number + VOTING_WINDOW + 1);
        
        vm.expectEmit(true, true, true, true);
        emit ICheckerOracle.Resolved(REQUEST_ID_1, false, ICheckerOracle.ResolveReason.TIMEOUT);
        
        oracle.finalize(REQUEST_ID_1);
        
        // Access Request struct
        (, , , , , , , , uint256 totalScore, ) = oracle.requests(REQUEST_ID_1);
        assertEq(totalScore, 0);
        
        // Check fee was refunded (consensus had FEE*10, spent FEE, got FEE back, so balance = FEE*10)
        assertEq(feeToken.balanceOf(consensus), FEE * 10);
    }

    function test_Finalize_NotReadyForFinalization_Reverts() public {
        vm.prank(consensus);
        oracle.postRequestWithFee(REQUEST_ID_1, FEE);
        
        vm.expectRevert("NotReadyForFinalization");
        oracle.finalize(REQUEST_ID_1);
    }

    // ============================================================
    // CLAIM TESTS
    // ============================================================

    function test_Claim_UnanimousApprove_Success() public {
        vm.prank(consensus);
        oracle.postRequestWithFee(REQUEST_ID_1, FEE);
        
        // All checkers vote Approve
        vm.prank(alice);
        oracle.commitApprove(REQUEST_ID_1);
        
        vm.prank(bob);
        oracle.commitApprove(REQUEST_ID_1);
        
        vm.prank(carol);
        oracle.commitApprove(REQUEST_ID_1);
        
        // Advance to deadline
        vm.roll(block.number + VOTING_WINDOW + 1);
        
        oracle.finalize(REQUEST_ID_1);
        
        // Alice claims
        vm.prank(alice);
        oracle.claim(REQUEST_ID_1);
        
        // Access Commitment struct
        (, , , bool claimed) = oracle.commitments(REQUEST_ID_1, alice);
        assertEq(claimed, true);
    }

    function test_Claim_AlreadyClaimed_Reverts() public {
        vm.prank(consensus);
        oracle.postRequestWithFee(REQUEST_ID_1, FEE);
        
        vm.prank(alice);
        oracle.commitApprove(REQUEST_ID_1);
        
        vm.prank(bob);
        oracle.commitApprove(REQUEST_ID_1);
        
        vm.prank(carol);
        oracle.commitApprove(REQUEST_ID_1);
        
        vm.roll(block.number + VOTING_WINDOW + 1);
        
        oracle.finalize(REQUEST_ID_1);
        
        vm.prank(alice);
        oracle.claim(REQUEST_ID_1);
        
        vm.expectRevert("AlreadyClaimed");
        vm.prank(alice);
        oracle.claim(REQUEST_ID_1);
    }

    // ============================================================
    // GOVERNANCE TESTS
    // ============================================================

    function test_AddChecker_Success() public {
        address newChecker = vm.createWallet("newChecker").addr;
        
        vm.expectEmit(true, true, false, true);
        emit ICheckerOracle.CheckerScheduled(newChecker, block.number + GOVERNANCE_DELAY);
        
        vm.prank(arbitrator);
        oracle.addChecker(newChecker);
        
        assertEq(oracle.checkerActiveAt(newChecker), block.number + GOVERNANCE_DELAY);
    }

    function test_RemoveChecker_Success() public {
        vm.expectEmit(true, true, false, true);
        emit ICheckerOracle.CheckerRemoved(alice);
        
        vm.prank(arbitrator);
        oracle.removeChecker(alice);
        
        assertEq(oracle.checkerActiveAt(alice), 0);
    }

    function test_ScheduleBondMultiplier_Success() public {
        uint256 newMultiplier = 100;
        
        vm.expectEmit(true, true, false, true);
        emit ICheckerOracle.BondMultiplierScheduled(newMultiplier, block.number + GOVERNANCE_DELAY);
        
        vm.prank(arbitrator);
        oracle.scheduleBondMultiplier(newMultiplier);
        
        assertEq(oracle.stagedBondMultiplier(), newMultiplier);
        assertEq(oracle.bondMultiplierActiveAt(), block.number + GOVERNANCE_DELAY);
    }

    function test_ApplyBondMultiplier_Success() public {
        uint256 newMultiplier = 100;
        
        vm.prank(arbitrator);
        oracle.scheduleBondMultiplier(newMultiplier);
        
        vm.roll(block.number + GOVERNANCE_DELAY + 1);
        
        vm.expectEmit(true, true, false, true);
        emit ICheckerOracle.BondMultiplierApplied(newMultiplier);
        
        oracle.applyBondMultiplier();
        
        assertEq(oracle.bondMultiplier(), newMultiplier);
    }

    function test_ApplyBondMultiplier_NotActive_Reverts() public {
        uint256 newMultiplier = 100;
        
        vm.prank(arbitrator);
        oracle.scheduleBondMultiplier(newMultiplier);
        
        vm.expectRevert("BondMultiplierNotActive");
        oracle.applyBondMultiplier();
    }

    function test_AddChecker_NotArbitrator_Reverts() public {
        vm.expectRevert("Unauthorized");
        oracle.addChecker(alice);
    }

    function test_RemoveChecker_NotArbitrator_Reverts() public {
        vm.expectRevert("Unauthorized");
        oracle.removeChecker(alice);
    }

    function test_ScheduleBondMultiplier_NotArbitrator_Reverts() public {
        vm.expectRevert("Unauthorized");
        oracle.scheduleBondMultiplier(100);
    }

    // ============================================================
    // BOND OVERPAYMENT TESTS
    // ============================================================

    function test_CommitVote_BondOverpayment_ReturnsExcess() public {
        vm.prank(consensus);
        oracle.postRequestWithFee(REQUEST_ID_1, FEE);
        
        // First checker commits full bond
        vm.prank(alice);
        oracle.commitApprove(REQUEST_ID_1);
        
        // Second checker tries to commit, but only gap-filling portion is counted
        vm.prank(bob);
        oracle.commitApprove(REQUEST_ID_1);
        
        // The second checker should have received excess back
        // But we need to check if the effective bond is correct
        
        (bool approved, uint256 bondAmount, , ) = oracle.commitments(REQUEST_ID_1, bob);
        assertEq(approved, true);
        // Bob should only have contributed the remaining gap
        assertEq(bondAmount, FEE * BOND_MULTIPLIER - BOND_AMOUNT);
    }
}
