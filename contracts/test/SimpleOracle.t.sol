// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {IOracle} from "@/interfaces/IOracle.sol";
import {SimpleOracle} from "@/SimpleOracle.sol";

contract SimpleOracleTest is Test {
    SimpleOracle public oracle;
    address public approver;
    address public sponsor;

    bytes32 constant REQUEST_ID = keccak256("requestId");

    function setUp() public {
        approver = vm.createWallet("approver").addr;
        sponsor = vm.createWallet("sponsor").addr;
        oracle = new SimpleOracle(approver);
    }

    function test_PostRequest_RecordsSponsor() public {
        vm.prank(sponsor);
        oracle.postRequest(REQUEST_ID, sponsor, "");

        // Verify by successfully approving — would revert with RequestNotPending if not recorded.
        vm.prank(approver);
        oracle.approve(REQUEST_ID);
    }

    function test_PostRequest_AlreadyPending_Reverts() public {
        vm.prank(sponsor);
        oracle.postRequest(REQUEST_ID, sponsor, "");

        vm.expectRevert(SimpleOracle.RequestAlreadyPending.selector);
        vm.prank(sponsor);
        oracle.postRequest(REQUEST_ID, sponsor, "");
    }

    function test_Approve_EmitsOracleResult_True() public {
        vm.prank(sponsor);
        oracle.postRequest(REQUEST_ID, sponsor, "");

        vm.expectEmit(true, true, false, true);
        emit IOracle.OracleResult(REQUEST_ID, sponsor, "", true);

        vm.prank(approver);
        oracle.approve(REQUEST_ID);
    }

    function test_Reject_EmitsOracleResult_False() public {
        vm.prank(sponsor);
        oracle.postRequest(REQUEST_ID, sponsor, "");

        vm.expectEmit(true, true, false, true);
        emit IOracle.OracleResult(REQUEST_ID, sponsor, "", false);

        vm.prank(approver);
        oracle.reject(REQUEST_ID);
    }

    function test_Approve_NotApprover_Reverts() public {
        vm.prank(sponsor);
        oracle.postRequest(REQUEST_ID, sponsor, "");

        vm.expectRevert(SimpleOracle.NotApprover.selector);
        oracle.approve(REQUEST_ID);
    }

    function test_Reject_NotApprover_Reverts() public {
        vm.prank(sponsor);
        oracle.postRequest(REQUEST_ID, sponsor, "");

        vm.expectRevert(SimpleOracle.NotApprover.selector);
        oracle.reject(REQUEST_ID);
    }

    function test_Approve_RequestNotPending_Reverts() public {
        vm.expectRevert(SimpleOracle.RequestNotPending.selector);
        vm.prank(approver);
        oracle.approve(REQUEST_ID);
    }

    function test_Reject_RequestNotPending_Reverts() public {
        vm.expectRevert(SimpleOracle.RequestNotPending.selector);
        vm.prank(approver);
        oracle.reject(REQUEST_ID);
    }

    function test_Approve_ClearsRequest() public {
        vm.prank(sponsor);
        oracle.postRequest(REQUEST_ID, sponsor, "");

        vm.prank(approver);
        oracle.approve(REQUEST_ID);

        // Second approval should revert because the request was cleared.
        vm.expectRevert(SimpleOracle.RequestNotPending.selector);
        vm.prank(approver);
        oracle.approve(REQUEST_ID);
    }

    function test_Reject_ClearsRequest() public {
        vm.prank(sponsor);
        oracle.postRequest(REQUEST_ID, sponsor, "");

        vm.prank(approver);
        oracle.reject(REQUEST_ID);

        // Second rejection should revert because the request was cleared.
        vm.expectRevert(SimpleOracle.RequestNotPending.selector);
        vm.prank(approver);
        oracle.reject(REQUEST_ID);
    }
}
