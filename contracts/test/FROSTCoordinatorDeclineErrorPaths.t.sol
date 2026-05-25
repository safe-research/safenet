// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROSTCoordinatorTestBase} from "@test/util/FROSTCoordinatorTestBase.sol";
import {FROSTCoordinator} from "@/FROSTCoordinator.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTParticipantMap} from "@/libraries/FROSTParticipantMap.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";

contract FROSTCoordinatorDeclineErrorPathsTest is FROSTCoordinatorTestBase {
    // count - threshold + 1 = 5 - 3 + 1 = 3
    uint16 public constant DECLINE_THRESHOLD = COUNT - THRESHOLD + 1;

    function test_SignDecline_NonParticipant_Reverts() public {
        (FROSTGroupId.T gid,,) = _trustedKeyGen(bytes32(0));
        FROSTSignatureId.T sid = coordinator.sign(gid, keccak256("msg"));

        vm.expectRevert(FROSTParticipantMap.InvalidParticipant.selector);
        vm.prank(address(0xdead));
        coordinator.signDecline(sid);
    }

    function test_SignDecline_AlreadyDeclined_Reverts() public {
        (FROSTGroupId.T gid,,) = _trustedKeyGen(bytes32(0));
        FROSTSignatureId.T sid = coordinator.sign(gid, keccak256("msg"));

        vm.prank(participants.addr(0));
        coordinator.signDecline(sid);

        vm.expectRevert(FROSTCoordinator.AlreadyDeclined.selector);
        vm.prank(participants.addr(0));
        coordinator.signDecline(sid);
    }

    function test_SignDecline_NotSigning_Reverts() public {
        // A SID that was never created maps to message == bytes32(0).
        FROSTSignatureId.T fakeSid = FROSTSignatureId.T.wrap(bytes32(uint256(1)));

        vm.expectRevert(FROSTCoordinator.NotSigning.selector);
        vm.prank(participants.addr(0));
        coordinator.signDecline(fakeSid);
    }

    function test_SignatureVerify_AfterRejection_Reverts_SignatureRejected() public {
        (FROSTGroupId.T gid,,) = _trustedKeyGen(bytes32(0));
        bytes32 message = keccak256("msg");
        FROSTSignatureId.T sid = coordinator.sign(gid, message);

        for (uint256 i = 0; i < DECLINE_THRESHOLD; i++) {
            vm.prank(participants.addr(i));
            coordinator.signDecline(sid);
        }

        vm.expectRevert(FROSTCoordinator.SignatureRejected.selector);
        coordinator.signatureVerify(sid, gid, message);
    }

    function test_SignatureValue_AfterRejection_Reverts_SignatureRejected() public {
        (FROSTGroupId.T gid,,) = _trustedKeyGen(bytes32(0));
        FROSTSignatureId.T sid = coordinator.sign(gid, keccak256("msg"));

        for (uint256 i = 0; i < DECLINE_THRESHOLD; i++) {
            vm.prank(participants.addr(i));
            coordinator.signDecline(sid);
        }

        vm.expectRevert(FROSTCoordinator.SignatureRejected.selector);
        coordinator.signatureValue(sid);
    }
}
