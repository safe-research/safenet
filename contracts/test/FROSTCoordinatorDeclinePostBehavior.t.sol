// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROSTCoordinatorTestBase} from "@test/util/FROSTCoordinatorTestBase.sol";
import {FROSTCoordinator} from "@/FROSTCoordinator.sol";
import {FROST} from "@/libraries/FROST.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";

contract FROSTCoordinatorDeclinePostBehaviorTest is FROSTCoordinatorTestBase {
    // count - threshold + 1 = 5 - 3 + 1 = 3
    uint16 public constant DECLINE_THRESHOLD = COUNT - THRESHOLD + 1;

    function test_SignDecline_PartialDeclines_CeremonyCompletes() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0));
        bytes32 message = keccak256("msg");

        // Select only the last THRESHOLD participants as signers so the
        // non-selected participants (0 and 1) can freely decline without
        // their nonce commitment being part of selection.r.
        uint256[] memory signers = new uint256[](THRESHOLD);
        for (uint256 i = 0; i < THRESHOLD; i++) {
            signers[i] = COUNT - THRESHOLD + i;
        }
        SignCeremonySetup memory setup = _signCeremonySetup(gid, s, message, signers);

        // Non-selected participants decline — below DECLINE_THRESHOLD (3), so no rejection.
        vm.prank(participants.addr(0));
        assertFalse(coordinator.signDecline(setup.sid));
        vm.prank(participants.addr(1));
        assertFalse(coordinator.signDecline(setup.sid));

        // Selected signers submit their shares; the last one should complete the ceremony.
        bool signed;
        for (uint256 i = 0; i < setup.signers.length; i++) {
            uint256 h = setup.signers[i];
            bytes32[] memory proof = setup.tree.proof(i);
            vm.prank(participants.addr(h));
            signed = coordinator.signShare(setup.sid, setup.selection, setup.shares[i], proof);
        }
        assertTrue(signed);

        // Signature is retrievable and valid.
        FROST.Signature memory sig = coordinator.signatureValue(setup.sid);
        FROST.verify(coordinator.groupKey(gid), sig, message);
    }

    function test_SignDecline_AfterSigningComplete_Reverts_SigningComplete() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0));

        uint256[] memory signers = new uint256[](THRESHOLD);
        for (uint256 i = 0; i < THRESHOLD; i++) {
            signers[i] = i;
        }
        FROSTSignatureId.T sid = _trustedSign(gid, s, keccak256("msg"), signers);

        // A non-signing participant tries to decline after the ceremony completed.
        vm.expectRevert(FROSTCoordinator.SigningComplete.selector);
        vm.prank(participants.addr(THRESHOLD));
        coordinator.signDecline(sid);
    }
}
