// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Vm} from "@forge-std/Test.sol";
import {FROSTCoordinatorTestBase} from "@test/util/FROSTCoordinatorTestBase.sol";
import {FROSTCoordinator} from "@/FROSTCoordinator.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";

contract FROSTCoordinatorDeclineThresholdTest is FROSTCoordinatorTestBase {
    function test_SignDecline_BelowThreshold_DoesNotReject() public {
        (FROSTGroupId.T gid,,) = _trustedKeyGen(bytes32(0));
        FROSTSignatureId.T sid = coordinator.sign(gid, keccak256("msg"));

        vm.recordLogs();
        for (uint256 i = 0; i < DECLINE_THRESHOLD - 1; i++) {
            vm.prank(participants.addr(i));
            assertFalse(coordinator.signDecline(sid));
        }

        Vm.Log[] memory logs = vm.getRecordedLogs();
        bytes32 rejectedTopic = FROSTCoordinator.SignRejected.selector;
        for (uint256 i = 0; i < logs.length; i++) {
            assertNotEq(logs[i].topics[0], rejectedTopic);
        }
    }

    function test_SignDecline_AboveThreshold_NoReEmit() public {
        (FROSTGroupId.T gid,,) = _trustedKeyGen(bytes32(0));
        FROSTSignatureId.T sid = coordinator.sign(gid, keccak256("msg"));

        // Reach the rejection threshold.
        for (uint256 i = 0; i < DECLINE_THRESHOLD - 1; i++) {
            vm.prank(participants.addr(i));
            coordinator.signDecline(sid);
        }
        vm.prank(participants.addr(DECLINE_THRESHOLD - 1));
        assertTrue(coordinator.signDecline(sid));

        // One more decline beyond the threshold: SignDeclined emitted, SignRejected is not.
        vm.recordLogs();
        vm.prank(participants.addr(DECLINE_THRESHOLD));
        bool result = coordinator.signDecline(sid);
        assertFalse(result);

        Vm.Log[] memory logs = vm.getRecordedLogs();
        assertEq(logs.length, 1);
        assertEq(logs[0].topics[0], FROSTCoordinator.SignDeclined.selector);
    }
}
