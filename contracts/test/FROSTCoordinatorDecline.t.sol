// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {Arrays} from "@oz/utils/Arrays.sol";
import {ParticipantMerkleTree} from "@test/util/ParticipantMerkleTree.sol";
import {FROSTCoordinator} from "@/FROSTCoordinator.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";
import {ForgeSecp256k1} from "@test/util/ForgeSecp256k1.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

contract FROSTCoordinatorDeclineTest is Test {
    using Arrays for address[];
    using ForgeSecp256k1 for ForgeSecp256k1.P;

    uint16 public constant COUNT = 5;
    uint16 public constant THRESHOLD = 3;
    // count - threshold + 1 = 5 - 3 + 1 = 3
    uint16 public constant DECLINE_THRESHOLD = COUNT - THRESHOLD + 1;

    FROSTCoordinator public coordinator;
    ParticipantMerkleTree public participants;

    function setUp() public {
        coordinator = new FROSTCoordinator();
        address[] memory addrs = new address[](COUNT);
        for (uint256 i = 0; i < COUNT; i++) {
            addrs[i] = vm.randomAddress();
        }
        addrs.sort();
        participants = new ParticipantMerkleTree(addrs);
    }

    function test_SignDecline_ThresholdReached_EmitsSignRejected() public {
        FROSTGroupId.T gid = _keyGen();
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

    // ============================================================
    // HELPERS
    // ============================================================

    function _keyGen() private returns (FROSTGroupId.T gid) {
        FROSTCoordinator.KeyGenCommitment memory commitment;
        commitment.q = ForgeSecp256k1.g(1).toPoint();
        commitment.c = new Secp256k1.Point[](THRESHOLD);
        for (uint256 j = 0; j < THRESHOLD; j++) {
            commitment.c[j] = ForgeSecp256k1.g(j + 1).toPoint();
        }
        bytes32 root = participants.root();
        for (uint256 i = 0; i < COUNT; i++) {
            (address addr, bytes32[] memory poap) = participants.proof(i);
            vm.prank(addr);
            (gid,) = coordinator.keyGenAndCommit(root, COUNT, THRESHOLD, bytes32(0), poap, commitment);
        }
        FROSTCoordinator.KeyGenSecretShare memory share;
        share.f = new uint256[](COUNT - 1);
        for (uint256 i = 0; i < COUNT; i++) {
            share.y = ForgeSecp256k1.g(i + 1).toPoint();
            vm.prank(participants.addr(i));
            coordinator.keyGenSecretShare(gid, share);
        }
        for (uint256 i = 0; i < COUNT; i++) {
            vm.prank(participants.addr(i));
            coordinator.keyGenConfirm(gid);
        }
    }
}
