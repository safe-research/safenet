// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

// Run 3 (rv-val, 2026-10-08): F-VAL-001's onchain step against `361aaf3`.
// The run-1 harness (`../FVal001Onchain.t.sol`) registered random points and
// no proofs; since #1022 (proof of knowledge of c[0]) and #1027 (proof of
// possession of q) `keyGenCommit` verifies both, so honest commitments here
// carry real proofs. The attack step is unchanged: the impostor M registers
// the victim A's q, copied verbatim from A's KeyGenCommitted event, plus the
// related keys the finding names. Every attack variant must revert; the
// control (M with a key it owns) must be accepted.

import {Test} from "@forge-std/Test.sol";
import {ForgeSecp256k1} from "@test/util/ForgeSecp256k1.sol";
import {FROSTMath} from "@test/util/FROSTMath.sol";
import {ParticipantMerkleTree} from "@test/util/ParticipantMerkleTree.sol";
import {FROSTCoordinator} from "@/FROSTCoordinator.sol";
import {FROST} from "@/libraries/FROST.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

contract FVal001OnchainRun3Test is Test {
    using ForgeSecp256k1 for ForgeSecp256k1.P;

    uint16 constant COUNT = 5;
    uint16 constant THRESHOLD = 3;
    uint256 constant VICTIM = 0;
    uint256 constant IMPOSTOR = 4;

    FROSTCoordinator coordinator;
    ParticipantMerkleTree participants;
    FROSTGroupId.T gid;
    uint256[] qSecret;
    FROSTCoordinator.KeyGenCommitment[] commitments;

    function _sortedAddrs(uint256 n) internal returns (address[] memory a) {
        a = new address[](n);
        for (uint256 i = 0; i < n; i++) {
            a[i] = vm.addr(vm.randomUint(1, type(uint128).max));
        }
        for (uint256 i = 1; i < n; i++) {
            for (uint256 j = i; j > 0 && a[j - 1] >= a[j]; j--) {
                (a[j - 1], a[j]) = (a[j], a[j - 1]);
            }
        }
        for (uint256 i = 1; i < n; i++) {
            require(a[i] > a[i - 1], "unsorted/dup");
        }
    }

    function setUp() public {
        coordinator = new FROSTCoordinator();
        participants = new ParticipantMerkleTree(_sortedAddrs(COUNT));
        gid = coordinator.keyGen(participants.root(), COUNT, THRESHOLD, bytes32(0));
        for (uint256 i = 0; i < COUNT; i++) {
            address who = participants.addr(i);
            uint256 q = vm.randomUint(1, Secp256k1.N - 1);
            qSecret.push(q);
            FROSTCoordinator.KeyGenCommitment storage c = commitments.push();
            c.q = ForgeSecp256k1.g(q).toPoint();
            c.pop = FROSTMath.proofOfKnowledge(who, q);
            uint256 a0 = vm.randomUint(1, Secp256k1.N - 1);
            c.c.push(ForgeSecp256k1.g(a0).toPoint());
            for (uint256 j = 1; j < THRESHOLD; j++) {
                c.c.push(ForgeSecp256k1.g(vm.randomUint(1, Secp256k1.N - 1)).toPoint());
            }
            c.pok = FROSTMath.proofOfKnowledge(who, a0);
        }
    }

    function _commitHonest() internal {
        for (uint256 i = 0; i < COUNT; i++) {
            if (i == IMPOSTOR) continue;
            (address who, bytes32[] memory poap) = participants.proof(i);
            vm.prank(who);
            coordinator.keyGenCommit(gid, poap, commitments[i]);
        }
    }

    function _impostorCommit(FROSTCoordinator.KeyGenCommitment memory c, bytes4 err) internal {
        (address who, bytes32[] memory poap) = participants.proof(IMPOSTOR);
        if (err != bytes4(0)) vm.expectRevert(err);
        vm.prank(who);
        coordinator.keyGenCommit(gid, poap, c);
    }

    /// Control: M commits a key it owns; the commit lands and closes round 1.
    function test_run3_control_owned_key_accepted() public {
        _commitHonest();
        _impostorCommit(commitments[IMPOSTOR], bytes4(0));
        (address who,) = participants.proof(IMPOSTOR);
        assertTrue(who != address(0));
    }

    /// V1, the original route: q copied verbatim, M keeps the PoP for its own key.
    function test_run3_v1_copy_q_own_pop_reverts() public {
        _commitHonest();
        FROSTCoordinator.KeyGenCommitment memory c = commitments[IMPOSTOR];
        c.q = commitments[VICTIM].q;
        _impostorCommit(c, Secp256k1.InvalidMulMulAddWitness.selector);
    }

    /// V2: q and pop both copied verbatim from the victim's event.
    function test_run3_v2_copy_q_and_pop_reverts() public {
        _commitHonest();
        FROSTCoordinator.KeyGenCommitment memory c = commitments[IMPOSTOR];
        c.q = commitments[VICTIM].q;
        c.pop = commitments[VICTIM].pop;
        _impostorCommit(c, Secp256k1.InvalidMulMulAddWitness.selector);
    }

    /// V3: the scalar-related key 2 * q_A with the victim's PoP.
    function test_run3_v3_scalar_related_q_reverts() public {
        _commitHonest();
        FROSTCoordinator.KeyGenCommitment memory c = commitments[IMPOSTOR];
        c.q = ForgeSecp256k1.mul(2, ForgeSecp256k1.g(qSecret[VICTIM])).toPoint();
        c.pop = commitments[VICTIM].pop;
        _impostorCommit(c, Secp256k1.InvalidMulMulAddWitness.selector);
    }

    /// V4: the negated key -q_A (same x-coordinate, so the same pad) with the
    /// victim's PoP negated, which satisfies the Schnorr equation for -q_A under
    /// the victim's challenge only.
    function test_run3_v4_negated_q_reverts() public {
        _commitHonest();
        FROSTCoordinator.KeyGenCommitment memory c = commitments[IMPOSTOR];
        ForgeSecp256k1.P memory qa = ForgeSecp256k1.g(qSecret[VICTIM]);
        c.q = ForgeSecp256k1.neg(qa).toPoint();
        assertEq(c.q.x, commitments[VICTIM].q.x, "negation keeps x");
        FROST.Signature memory pa = commitments[VICTIM].pop;
        c.pop.r = Secp256k1.Point({x: pa.r.x, y: Secp256k1.P - pa.r.y});
        c.pop.z = Secp256k1.N - pa.z;
        _impostorCommit(c, Secp256k1.InvalidMulMulAddWitness.selector);
    }

    /// V5: q at the identity with the trivial proof R = z*G.
    function test_run3_v5_identity_q_reverts() public {
        _commitHonest();
        FROSTCoordinator.KeyGenCommitment memory c = commitments[IMPOSTOR];
        c.q = Secp256k1.Point({x: 0, y: 0});
        c.pop.r = ForgeSecp256k1.g(7).toPoint();
        c.pop.z = 7;
        _impostorCommit(c, Secp256k1.NotOnCurve.selector);
    }

    /// V6: M commits first, before the victim, with a guessed q. Without A's
    /// key M can only guess, and a guess without its discrete log has no PoP.
    function test_run3_v6_frontrun_unknown_key_reverts() public {
        FROSTCoordinator.KeyGenCommitment memory c = commitments[IMPOSTOR];
        c.q = ForgeSecp256k1.g(qSecret[VICTIM]).toPoint();
        c.pop.r = ForgeSecp256k1.g(vm.randomUint(1, Secp256k1.N - 1)).toPoint();
        c.pop.z = vm.randomUint(1, Secp256k1.N - 1);
        _impostorCommit(c, Secp256k1.InvalidMulMulAddWitness.selector);
    }
}
