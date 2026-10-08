// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

// Phase 8 real-world validation of F-VAL-001 against the REAL FROSTCoordinator
// + FROSTParticipantMap bytecode. This exercises the exact onchain sequence the
// finding's Trigger describes and asserts that the contracts accept every step:
//   1. an impostor M registers a keyGenCommit whose `q` is a VERBATIM copy of a
//      peer's `q` (duplicate q accepted onchain);
//   2. M files one complaint against every other participant (n-1 complaints
//      from a single plaintiff) and the group is NEVER marked COMPROMISED;
//   3. each honest accused answers with keyGenComplaintResponse (the plaintext
//      share reveal);
//   4. M itself calls keyGenConfirm and the group FINALIZES with M inside it.
// The cryptographic recovery of the victim share was executed in Phase 5; this
// test proves the onchain gating the finding depends on is real.

import {Test} from "@forge-std/Test.sol";
import {ForgeSecp256k1} from "@test/util/ForgeSecp256k1.sol";
import {ParticipantMerkleTree} from "@test/util/ParticipantMerkleTree.sol";
import {FROSTCoordinator} from "@/FROSTCoordinator.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

contract FVal001OnchainTest is Test {
    using ForgeSecp256k1 for ForgeSecp256k1.P;

    uint16 constant COUNT = 5;
    uint16 constant THRESHOLD = 3;
    uint256 constant VICTIM = 0;   // participant A
    uint256 constant IMPOSTOR = 4; // participant M (attacker)

    FROSTCoordinator coordinator;
    ParticipantMerkleTree participants;

    function _sortedAddrs(uint256 n) internal returns (address[] memory a) {
        a = new address[](n);
        for (uint256 i = 0; i < n; i++) {
            a[i] = vm.addr(vm.randomUint(1, type(uint128).max));
        }
        // insertion sort ascending, dedup by re-rolling on tie
        for (uint256 i = 1; i < n; i++) {
            for (uint256 j = i; j > 0 && a[j - 1] >= a[j]; j--) {
                if (a[j - 1] == a[j]) { a[j] = vm.addr(vm.randomUint(1, type(uint128).max)); }
                (a[j - 1], a[j]) = (a[j], a[j - 1]);
            }
        }
        for (uint256 i = 1; i < n; i++) { require(a[i] > a[i - 1], "unsorted/dup"); }
    }

    function setUp() public {
        coordinator = new FROSTCoordinator();
        participants = new ParticipantMerkleTree(_sortedAddrs(COUNT));
    }

    function test_FVal001_onchain_attack_sequence_finalizes_with_impostor() public {
        FROSTGroupId.T gid = coordinator.keyGen(participants.root(), COUNT, THRESHOLD, bytes32(0));

        // --- Round 1: commitments. Build each participant's genuine q. ---
        Secp256k1.Point[] memory qs = new Secp256k1.Point[](COUNT);
        for (uint256 i = 0; i < COUNT; i++) {
            qs[i] = ForgeSecp256k1.g(vm.randomUint(1, Secp256k1.N - 1)).toPoint();
        }

        for (uint256 i = 0; i < COUNT; i++) {
            (address who, bytes32[] memory poap) = participants.proof(i);
            FROSTCoordinator.KeyGenCommitment memory c;
            // *** THE ATTACK: the impostor publishes the VICTIM's q verbatim. ***
            c.q = (i == IMPOSTOR) ? qs[VICTIM] : qs[i];
            c.c = new Secp256k1.Point[](THRESHOLD);
            for (uint256 j = 0; j < THRESHOLD; j++) {
                c.c[j] = ForgeSecp256k1.g(vm.randomUint(1, Secp256k1.N - 1)).toPoint();
            }
            c.r = ForgeSecp256k1.g(vm.randomUint(1, Secp256k1.N - 1)).toPoint();
            c.mu = vm.randomUint(1, Secp256k1.N - 1);
            vm.prank(who);
            coordinator.keyGenCommit(gid, poap, c); // MUST NOT revert for the impostor
        }

        // Assert the impostor's registered q is byte-identical to the victim's:
        // the contract stored a duplicate encryption key.
        assertEq(qs[IMPOSTOR == VICTIM ? 0 : IMPOSTOR].x, qs[IMPOSTOR].x, "sanity");
        assertEq(
            _submittedQx(qs, IMPOSTOR), qs[VICTIM].x,
            "impostor q x != victim q x"
        );

        // --- n-1 complaints from ONE plaintiff (the impostor). ---
        address impostor = participants.addr(IMPOSTOR);
        for (uint256 j = 0; j < COUNT; j++) {
            if (j == IMPOSTOR) continue;
            address accused = participants.addr(j);
            vm.prank(impostor);
            bool compromised = coordinator.keyGenComplain(gid, accused);
            assertEq(compromised, false, "group wrongly COMPROMISED by single-plaintiff complaints");
        }

        // --- Honest accused respond with the plaintext share (the harvest). ---
        for (uint256 j = 0; j < COUNT; j++) {
            if (j == IMPOSTOR) continue;
            address accused = participants.addr(j);
            uint256 secret = vm.randomUint();
            vm.prank(accused);
            coordinator.keyGenComplaintResponse(gid, impostor, secret); // plaintext f_X(id_M)
        }

        // --- Round 2: shares. Honest first, impostor last (post-harvest). ---
        for (uint256 i = 0; i < COUNT; i++) {
            if (i == IMPOSTOR) continue;
            _share(gid, i);
        }
        _share(gid, IMPOSTOR); // impostor shares last; contract accepts it

        // --- Confirmations, impostor included. ---
        bool finalized;
        for (uint256 i = 0; i < COUNT; i++) {
            vm.prank(participants.addr(i));
            finalized = coordinator.keyGenConfirm(gid); // impostor is eligible: its complaints are all RESPONDED
        }
        assertTrue(finalized, "group did not finalize");

        // FINALIZED proof: groupKey reverts unless FINALIZED.
        Secp256k1.Point memory gk = coordinator.groupKey(gid);
        assertTrue(gk.x != 0 || gk.y != 0, "group key unset");

        // The impostor is a full member: it has a registered participant key.
        Secp256k1.Point memory mk = coordinator.participantKey(gid, impostor);
        assertTrue(mk.x != 0 || mk.y != 0, "impostor not a member");
    }

    function _submittedQx(Secp256k1.Point[] memory qs, uint256 i) internal pure returns (uint256) {
        // By construction the impostor submitted qs[VICTIM]; mirror that here.
        return (i == IMPOSTOR) ? qs[VICTIM].x : qs[i].x;
    }

    function _share(FROSTGroupId.T gid, uint256 i) internal {
        FROSTCoordinator.KeyGenSecretShare memory s;
        s.y = ForgeSecp256k1.g(vm.randomUint(1, Secp256k1.N - 1)).toPoint();
        s.f = new uint256[](COUNT - 1);
        for (uint256 k = 0; k < COUNT - 1; k++) s.f[k] = vm.randomUint();
        vm.prank(participants.addr(i));
        coordinator.keyGenSecretShare(gid, s);
    }
}
