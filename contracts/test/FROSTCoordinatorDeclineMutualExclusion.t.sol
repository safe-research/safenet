// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROSTCoordinatorTestBase} from "@test/util/FROSTCoordinatorTestBase.sol";
import {FROSTCoordinator} from "@/FROSTCoordinator.sol";
import {FROST} from "@/libraries/FROST.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";

contract FROSTCoordinatorDeclineMutualExclusionTest is FROSTCoordinatorTestBase {
    function test_SignShare_AfterSignDecline_Reverts_AlreadyDeclined() public {
        (FROSTGroupId.T gid,,) = _trustedKeyGen(bytes32(0));
        FROSTSignatureId.T sid = coordinator.sign(gid, keccak256("msg"));

        address participant = participants.addr(0);
        vm.prank(participant);
        coordinator.signDecline(sid);

        // AlreadyDeclined is checked before cryptographic verification, so dummy data suffices.
        FROSTCoordinator.SignSelection memory selection;
        FROST.SignatureShare memory share;
        vm.expectRevert(FROSTCoordinator.AlreadyDeclined.selector);
        vm.prank(participant);
        coordinator.signShare(sid, selection, share, new bytes32[](0));
    }

    function test_SignDecline_AfterSignShare_Reverts_AlreadyShared() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0));

        // Select exactly THRESHOLD signers so a single share does not complete the ceremony:
        // accumulator.r = R_0 != R_0 + R_1 + R_2 = selection.r.
        uint256[] memory signers = new uint256[](THRESHOLD);
        for (uint256 i = 0; i < THRESHOLD; i++) {
            signers[i] = i;
        }
        SignCeremonySetup memory setup = _signCeremonySetup(gid, s, keccak256("msg"), signers);

        address signer = participants.addr(setup.signers[0]);
        bytes32[] memory proof = setup.tree.proof(0);
        vm.prank(signer);
        coordinator.signShare(setup.sid, setup.selection, setup.shares[0], proof);

        vm.expectRevert(FROSTCoordinator.AlreadyShared.selector);
        vm.prank(signer);
        coordinator.signDecline(setup.sid);
    }
}
