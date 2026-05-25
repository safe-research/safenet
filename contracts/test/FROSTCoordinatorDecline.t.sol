// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROSTCoordinatorTestBase} from "@test/util/FROSTCoordinatorTestBase.sol";
import {FROSTCoordinator} from "@/FROSTCoordinator.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";

contract FROSTCoordinatorDeclineTest is FROSTCoordinatorTestBase {
    // count - threshold + 1 = 5 - 3 + 1 = 3
    uint16 public constant DECLINE_THRESHOLD = COUNT - THRESHOLD + 1;

    function test_SignDecline_ThresholdReached_EmitsSignRejected() public {
        (FROSTGroupId.T gid,,) = _trustedKeyGen(bytes32(0));
        FROSTSignatureId.T sid = coordinator.sign(gid, keccak256("msg"));

        for (uint256 i = 0; i < DECLINE_THRESHOLD - 1; i++) {
            vm.prank(participants.addr(i));
            assertFalse(coordinator.signDecline(sid));
        }

        vm.expectEmit();
        emit FROSTCoordinator.SignRejected(sid);
        vm.prank(participants.addr(DECLINE_THRESHOLD - 1));
        bool rejected = coordinator.signDecline(sid);
        assertTrue(rejected);
    }
}
