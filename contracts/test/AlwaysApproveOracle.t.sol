// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test, Vm} from "@forge-std/Test.sol";
import {IOracle} from "@/interfaces/IOracle.sol";
import {AlwaysApproveOracle} from "@/AlwaysApproveOracle.sol";

contract AlwaysApproveOracleTest is Test {
    AlwaysApproveOracle public oracle;
    address public requester;

    bytes32 constant REQUEST_ID = keccak256("requestId");

    function setUp() public {
        requester = vm.createWallet("requester").addr;
        oracle = new AlwaysApproveOracle();
    }

    function test_PostRequest_EmitsOracleResult_Immediately() public {
        vm.expectEmit(true, true, false, true);
        emit IOracle.OracleResult(REQUEST_ID, requester, "", true);

        vm.prank(requester);
        oracle.postRequest(REQUEST_ID);
    }

    function test_PostRequest_EmitsApprovedTrue() public {
        vm.recordLogs();

        vm.prank(requester);
        oracle.postRequest(REQUEST_ID);

        Vm.Log[] memory logs = vm.getRecordedLogs();
        assertEq(logs.length, 1);

        // topic[0] = event selector, topic[1] = requestId, topic[2] = proposer
        assertEq(logs[0].topics[1], REQUEST_ID);
        assertEq(logs[0].topics[2], bytes32(uint256(uint160(requester))));

        // data = abi.encode(result, approved) — last word is the bool
        (, bool approved) = abi.decode(logs[0].data, (bytes, bool));
        assertTrue(approved);
    }

    function test_PostRequest_ProposerIsCallerAddress() public {
        address anotherCaller = vm.createWallet("other").addr;

        vm.expectEmit(true, true, false, true);
        emit IOracle.OracleResult(REQUEST_ID, anotherCaller, "", true);

        vm.prank(anotherCaller);
        oracle.postRequest(REQUEST_ID);
    }
}
