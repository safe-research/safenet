// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test, Vm} from "@forge-std/Test.sol";
import {Arrays} from "@oz/utils/Arrays.sol";
import {MerkleProof} from "@oz/utils/cryptography/MerkleProof.sol";
import {Math} from "@oz/utils/math/Math.sol";
import {CommitmentShareMerkleTree} from "@test/util/CommitmentShareMerkleTree.sol";
import {ForgeSecp256k1} from "@test/util/ForgeSecp256k1.sol";
import {ParticipantMerkleTree} from "@test/util/ParticipantMerkleTree.sol";
import {FROSTCoordinator} from "@/FROSTCoordinator.sol";
import {FROST} from "@/libraries/FROST.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

contract FROSTCoordinatorDeclineTest is Test {
    using Arrays for address[];
    using Arrays for uint256[];
    using ForgeSecp256k1 for ForgeSecp256k1.P;

    struct Nonces {
        ForgeSecp256k1.P d;
        ForgeSecp256k1.P e;
    }

    uint16 public constant COUNT = 5;
    uint16 public constant THRESHOLD = 3;
    // count - threshold + 1 = 5 - 3 + 1 = 3
    uint16 public constant DECLINE_THRESHOLD = COUNT - THRESHOLD + 1;

    FROSTCoordinator public coordinator;
    ParticipantMerkleTree public participants;

    function setUp() public {
        coordinator = new FROSTCoordinator();
        participants = new ParticipantMerkleTree(_randomSortedAddresses(COUNT));
    }

    // ============================================================
    // DECLINE BASIC BEHAVIOUR
    // ============================================================

    function test_SignDecline_EmitsSignDeclined() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0)); // solhint-disable-line no-unused-vars
        bytes32 message = keccak256("test message");
        FROSTSignatureId.T sid = coordinator.sign(gid, message);

        address participant = participants.addr(0);
        vm.expectEmit();
        emit FROSTCoordinator.SignDeclined(sid, participant);
        vm.prank(participant);
        coordinator.signDecline(sid);
    }

    function test_SignDecline_IsSignDeclined_ReturnsTrue() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0)); // solhint-disable-line no-unused-vars
        FROSTSignatureId.T sid = coordinator.sign(gid, keccak256("msg"));

        address participant = participants.addr(0);
        vm.prank(participant);
        coordinator.signDecline(sid);

        assertTrue(coordinator.isSignDeclined(sid, participant));
    }

    function test_SignDecline_IsSignDeclined_OtherParticipant_ReturnsFalse() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0)); // solhint-disable-line no-unused-vars
        FROSTSignatureId.T sid = coordinator.sign(gid, keccak256("msg"));

        vm.prank(participants.addr(0));
        coordinator.signDecline(sid);

        assertFalse(coordinator.isSignDeclined(sid, participants.addr(1)));
    }

    function test_SignDecline_BelowThreshold_IsSignRejected_ReturnsFalse() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0)); // solhint-disable-line no-unused-vars
        FROSTSignatureId.T sid = coordinator.sign(gid, keccak256("msg"));

        // Decline with DECLINE_THRESHOLD - 1 participants
        for (uint256 i = 0; i < DECLINE_THRESHOLD - 1; i++) {
            vm.prank(participants.addr(i));
            bool rejected = coordinator.signDecline(sid);
            assertFalse(rejected);
        }

        assertFalse(coordinator.isSignRejected(sid));
    }

    function test_SignDecline_AtThreshold_EmitsSignRejected() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0)); // solhint-disable-line no-unused-vars
        FROSTSignatureId.T sid = coordinator.sign(gid, keccak256("msg"));

        // First DECLINE_THRESHOLD - 1 declines should not emit SignRejected
        for (uint256 i = 0; i < DECLINE_THRESHOLD - 1; i++) {
            vm.prank(participants.addr(i));
            coordinator.signDecline(sid);
        }

        // The DECLINE_THRESHOLD-th decline crosses the threshold
        vm.expectEmit();
        emit FROSTCoordinator.SignRejected(sid);
        vm.prank(participants.addr(DECLINE_THRESHOLD - 1));
        bool rejected = coordinator.signDecline(sid);
        assertTrue(rejected);
    }

    function test_SignDecline_AtThreshold_IsSignRejected_ReturnsTrue() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0)); // solhint-disable-line no-unused-vars
        FROSTSignatureId.T sid = coordinator.sign(gid, keccak256("msg"));

        for (uint256 i = 0; i < DECLINE_THRESHOLD; i++) {
            vm.prank(participants.addr(i));
            coordinator.signDecline(sid);
        }

        assertTrue(coordinator.isSignRejected(sid));
    }

    function test_SignDecline_ReturnsFalse_BelowThreshold() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0)); // solhint-disable-line no-unused-vars
        FROSTSignatureId.T sid = coordinator.sign(gid, keccak256("msg"));

        vm.prank(participants.addr(0));
        bool rejected = coordinator.signDecline(sid);
        assertFalse(rejected);
    }

    function test_SignDecline_ReturnsTrue_AtThreshold() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0)); // solhint-disable-line no-unused-vars
        FROSTSignatureId.T sid = coordinator.sign(gid, keccak256("msg"));

        for (uint256 i = 0; i < DECLINE_THRESHOLD - 1; i++) {
            vm.prank(participants.addr(i));
            coordinator.signDecline(sid);
        }

        vm.prank(participants.addr(DECLINE_THRESHOLD - 1));
        bool rejected = coordinator.signDecline(sid);
        assertTrue(rejected);
    }

    // ============================================================
    // ADDITIONAL DECLINES AFTER THRESHOLD
    // ============================================================

    function test_SignDecline_AfterThreshold_SignDeclinedEmitted_SignRejectedNotReEmitted() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0)); // solhint-disable-line no-unused-vars
        FROSTSignatureId.T sid = coordinator.sign(gid, keccak256("msg"));

        // Reach threshold
        for (uint256 i = 0; i < DECLINE_THRESHOLD; i++) {
            vm.prank(participants.addr(i));
            coordinator.signDecline(sid);
        }

        // One more decline: SignDeclined emitted, SignRejected NOT re-emitted
        vm.recordLogs();
        vm.prank(participants.addr(DECLINE_THRESHOLD));
        bool rejected = coordinator.signDecline(sid);

        Vm.Log[] memory logs = vm.getRecordedLogs();
        assertFalse(rejected);

        bool foundDeclined = false;
        bool foundRejected = false;
        for (uint256 i = 0; i < logs.length; i++) {
            if (logs[i].topics[0] == FROSTCoordinator.SignDeclined.selector) foundDeclined = true;
            if (logs[i].topics[0] == FROSTCoordinator.SignRejected.selector) foundRejected = true;
        }
        assertTrue(foundDeclined);
        assertFalse(foundRejected);
    }

    function test_SignDecline_AfterThreshold_StillRecorded() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0)); // solhint-disable-line no-unused-vars
        FROSTSignatureId.T sid = coordinator.sign(gid, keccak256("msg"));

        for (uint256 i = 0; i < DECLINE_THRESHOLD; i++) {
            vm.prank(participants.addr(i));
            coordinator.signDecline(sid);
        }

        vm.prank(participants.addr(DECLINE_THRESHOLD));
        coordinator.signDecline(sid);

        assertTrue(coordinator.isSignDeclined(sid, participants.addr(DECLINE_THRESHOLD)));
    }

    // ============================================================
    // REVERT CASES
    // ============================================================

    function test_SignDecline_NonParticipant_Reverts() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0)); // solhint-disable-line no-unused-vars
        FROSTSignatureId.T sid = coordinator.sign(gid, keccak256("msg"));

        address nonParticipant = vm.randomAddress();
        vm.expectRevert();
        vm.prank(nonParticipant);
        coordinator.signDecline(sid);
    }

    function test_SignDecline_DoubleDecline_Reverts() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0)); // solhint-disable-line no-unused-vars
        FROSTSignatureId.T sid = coordinator.sign(gid, keccak256("msg"));

        address participant = participants.addr(0);
        vm.prank(participant);
        coordinator.signDecline(sid);

        vm.expectRevert(FROSTCoordinator.AlreadyDeclined.selector);
        vm.prank(participant);
        coordinator.signDecline(sid);
    }

    function test_SignDecline_CeremonyNotStarted_Reverts() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0)); // solhint-disable-line no-unused-vars
        // Create a SID without starting a ceremony
        FROSTSignatureId.T fakeSid = FROSTSignatureId.create(gid, 99);

        address p0 = participants.addr(0);
        vm.expectRevert(FROSTCoordinator.NotSigning.selector);
        vm.prank(p0);
        coordinator.signDecline(fakeSid);
    }

    function test_SignDecline_AfterSigned_Reverts() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0));
        FROSTSignatureId.T sid = _trustedSign(gid, s, keccak256("sign me"));

        address p0 = participants.addr(0);
        vm.expectRevert(FROSTCoordinator.SigningComplete.selector);
        vm.prank(p0);
        coordinator.signDecline(sid);
    }

    // ============================================================
    // SIGSHARE AFTER REJECTION
    // ============================================================

    function test_SignShare_AfterRejection_Reverts() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0));
        bytes32 message = keccak256("decline then sign");
        FROSTSignatureId.T sid = coordinator.sign(gid, message);

        for (uint256 i = 0; i < DECLINE_THRESHOLD; i++) {
            vm.prank(participants.addr(i));
            coordinator.signDecline(sid);
        }

        // Attempt signShare on rejected ceremony — use dummy selection/share/proof
        FROSTCoordinator.SignSelection memory selection;
        FROST.SignatureShare memory share;
        bytes32[] memory proof;
        address pNext = participants.addr(DECLINE_THRESHOLD);
        vm.expectRevert(FROSTCoordinator.CeremonyRejected.selector);
        vm.prank(pNext);
        coordinator.signShare(sid, selection, share, proof);
    }

    // ============================================================
    // SIGNATUREVALUE / SIGNATUREVERIFY AFTER REJECTION
    // ============================================================

    function test_SignatureValue_AfterRejection_Reverts() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0)); // solhint-disable-line no-unused-vars
        FROSTSignatureId.T sid = coordinator.sign(gid, keccak256("msg"));

        for (uint256 i = 0; i < DECLINE_THRESHOLD; i++) {
            vm.prank(participants.addr(i));
            coordinator.signDecline(sid);
        }

        vm.expectRevert(FROSTCoordinator.SignatureRejected.selector);
        coordinator.signatureValue(sid);
    }

    function test_SignatureVerify_AfterRejection_Reverts() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0)); // solhint-disable-line no-unused-vars
        bytes32 message = keccak256("msg");
        FROSTSignatureId.T sid = coordinator.sign(gid, message);

        for (uint256 i = 0; i < DECLINE_THRESHOLD; i++) {
            vm.prank(participants.addr(i));
            coordinator.signDecline(sid);
        }

        vm.expectRevert(FROSTCoordinator.SignatureRejected.selector);
        coordinator.signatureVerify(sid, gid, message);
    }

    // ============================================================
    // SIGNING COMPLETES BEFORE REJECTION THRESHOLD
    // ============================================================

    function test_SignDecline_BelowThreshold_CeremonyStillCompletable() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0));

        // Complete a full ceremony (sequence 0) — demonstrates the group can sign.
        FROSTSignatureId.T completedSid = _trustedSign(gid, s, keccak256("complete before decline"));
        assertFalse(coordinator.isSignRejected(completedSid));

        // Start a new ceremony (sequence 1) and have fewer than DECLINE_THRESHOLD participants decline;
        // the ceremony must remain non-rejected.
        FROSTSignatureId.T nextSid = coordinator.sign(gid, keccak256("next ceremony"));
        for (uint256 i = 0; i < DECLINE_THRESHOLD - 1; i++) {
            vm.prank(participants.addr(i));
            coordinator.signDecline(nextSid);
        }
        assertFalse(coordinator.isSignRejected(nextSid));
    }

    function test_SignatureMessage_ReturnsMessage() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0)); // solhint-disable-line no-unused-vars
        bytes32 message = keccak256("check message");
        FROSTSignatureId.T sid = coordinator.sign(gid, message);

        assertEq(coordinator.signatureMessage(sid), message);
    }

    function test_SignatureMessage_NotStarted_ReturnsZero() public {
        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0)); // solhint-disable-line no-unused-vars
        FROSTSignatureId.T fakeSid = FROSTSignatureId.create(gid, 99);
        assertEq(coordinator.signatureMessage(fakeSid), bytes32(0));
    }

    // ============================================================
    // HELPERS
    // ============================================================

    function _randomSortedAddresses(uint16 count) private view returns (address[] memory result) {
        result = new address[](count);
        for (uint256 i = 0; i < result.length; i++) {
            result[i] = vm.randomAddress();
        }
        result.sort();
    }

    function _trustedKeyGen(bytes32 context) private returns (FROSTGroupId.T gid, uint256[] memory s, uint256 gs) {
        s = new uint256[](COUNT);

        uint256[] memory a = new uint256[](THRESHOLD);
        for (uint256 j = 0; j < THRESHOLD; j++) {
            a[j] = vm.randomUint(0, Secp256k1.N - 1);
        }

        FROSTCoordinator.KeyGenCommitment memory commitment;
        commitment.q = ForgeSecp256k1.g(1).toPoint();
        commitment.c = new Secp256k1.Point[](THRESHOLD);
        for (uint256 i = 1; i < COUNT; i++) {
            bytes32 root = participants.root();
            (address participant, bytes32[] memory poap) = participants.proof(i);
            vm.prank(participant);
            coordinator.keyGenAndCommit(root, COUNT, THRESHOLD, context, poap, commitment);
        }
        {
            for (uint256 j = 0; j < THRESHOLD; j++) {
                commitment.c[j] = ForgeSecp256k1.g(a[j]).toPoint();
            }
            bytes32 root = participants.root();
            (address participant, bytes32[] memory poap) = participants.proof(0);
            vm.prank(participant);
            (gid,) = coordinator.keyGenAndCommit(root, COUNT, THRESHOLD, context, poap, commitment);
        }

        FROSTCoordinator.KeyGenSecretShare memory share;
        share.f = new uint256[](COUNT - 1);
        for (uint256 i = 0; i < COUNT; i++) {
            s[i] = _f(a, i);
            share.y = ForgeSecp256k1.g(s[i]).toPoint();
            vm.prank(participants.addr(i));
            coordinator.keyGenSecretShare(gid, share);
        }

        for (uint256 i = 0; i < COUNT; i++) {
            vm.prank(participants.addr(i));
            coordinator.keyGenConfirm(gid);
        }

        gs = a[0];
    }

    function _trustedSign(FROSTGroupId.T gid, uint256[] memory s, bytes32 message)
        private
        returns (FROSTSignatureId.T sid)
    {
        bytes32[] memory nonceProof = new bytes32[](10);
        Nonces[] memory nonces = new Nonces[](COUNT);
        {
            bytes32[] memory commitments = new bytes32[](COUNT);
            for (uint256 i = 0; i < COUNT; i++) {
                Nonces memory n = nonces[i];
                uint256 d = FROST.nonce(bytes32(vm.randomUint()), s[i]);
                n.d = ForgeSecp256k1.g(d);
                uint256 e = FROST.nonce(bytes32(vm.randomUint()), s[i]);
                n.e = ForgeSecp256k1.g(e);
                // forge-lint: disable-next-line(asm-keccak256)
                bytes32 leaf = keccak256(abi.encode(0, n.d.x(), n.d.y(), n.e.x(), n.e.y()));
                commitments[i] = MerkleProof.processProof(nonceProof, leaf);
            }
            for (uint256 i = 0; i < COUNT; i++) {
                vm.prank(participants.addr(i));
                coordinator.preprocess(gid, commitments[i]);
            }
        }

        // Build the honest participants list and reveal nonces before sorting.
        uint256[] memory honestParticipants = new uint256[](COUNT);
        for (uint256 i = 0; i < COUNT; i++) {
            honestParticipants[i] = i;
        }

        sid = coordinator.sign(gid, message);

        for (uint256 i = 0; i < COUNT; i++) {
            uint256 h = honestParticipants[i];
            Nonces memory n = nonces[h];
            FROSTCoordinator.SignNonces memory nn = FROSTCoordinator.SignNonces({d: n.d.toPoint(), e: n.e.toPoint()});
            vm.prank(participants.addr(h));
            coordinator.signRevealNonces(sid, nn, nonceProof);
        }

        // Binding factors and CommitmentShareMerkleTree require participants sorted by FROST identifier.
        _sortByParticipantId(honestParticipants);

        Secp256k1.Point memory groupKey = coordinator.groupKey(gid);
        FROSTCoordinator.SignSelection memory selection;
        FROST.SignatureShare[] memory shares = new FROST.SignatureShare[](COUNT);
        {
            uint256[] memory bindingFactors;
            {
                FROST.Commitment[] memory coms = new FROST.Commitment[](COUNT);
                for (uint256 i = 0; i < COUNT; i++) {
                    uint256 h = honestParticipants[i];
                    Nonces memory n = nonces[h];
                    coms[i] = FROST.Commitment({participant: participants.addr(h), d: n.d.toPoint(), e: n.e.toPoint()});
                }
                bindingFactors = FROST.bindingFactors(groupKey, coms, message);
            }

            ForgeSecp256k1.P memory groupCommitment;
            for (uint256 i = 0; i < COUNT; i++) {
                uint256 h = honestParticipants[i];
                Nonces memory n = nonces[h];
                ForgeSecp256k1.P memory r = ForgeSecp256k1.add(n.d, ForgeSecp256k1.mul(bindingFactors[i], n.e));
                shares[i].r = r.toPoint();
                shares[i].l = _lagrangeCoefficient(honestParticipants, h);
                groupCommitment = ForgeSecp256k1.add(groupCommitment, r);
            }
            selection.r = groupCommitment.toPoint();

            uint256 challenge = FROST.challenge(selection.r, groupKey, message);
            for (uint256 i = 0; i < COUNT; i++) {
                uint256 h = honestParticipants[i];
                uint256 sk = s[h];
                Nonces memory n = nonces[h];
                shares[i].z = addmod(
                    n.d.w.privateKey,
                    addmod(
                        mulmod(n.e.w.privateKey, bindingFactors[i], Secp256k1.N),
                        mulmod(mulmod(challenge, shares[i].l, Secp256k1.N), sk, Secp256k1.N),
                        Secp256k1.N
                    ),
                    Secp256k1.N
                );
            }
        }

        CommitmentShareMerkleTree commitmentShares;
        {
            CommitmentShareMerkleTree.S[] memory cs = new CommitmentShareMerkleTree.S[](COUNT);
            for (uint256 i = 0; i < COUNT; i++) {
                uint256 h = honestParticipants[i];
                cs[i] = CommitmentShareMerkleTree.S({participant: participants.addr(h), r: shares[i].r, l: shares[i].l});
            }
            commitmentShares = new CommitmentShareMerkleTree(selection.r, cs);
            selection.root = commitmentShares.root();
        }

        for (uint256 i = 0; i < COUNT; i++) {
            bytes32[] memory proof = commitmentShares.proof(i);
            uint256 h = honestParticipants[i];
            vm.prank(participants.addr(h));
            coordinator.signShare(sid, selection, shares[i], proof);
        }
    }

    function _sortByParticipantId(uint256[] memory ps) private view {
        bool ordered = false;
        while (!ordered) {
            ordered = true;
            for (uint256 i = 1; i < ps.length; i++) {
                uint256 a = ps[i - 1];
                uint256 b = ps[i];
                if (FROST.identifier(participants.addr(a)) > FROST.identifier(participants.addr(b))) {
                    ps[i - 1] = b;
                    ps[i] = a;
                    ordered = false;
                }
            }
        }
    }

    function _f(uint256[] memory a, uint256 i) private view returns (uint256 r) {
        r = a[0];
        uint256 x = FROST.identifier(participants.addr(i));
        uint256 xx = 1;
        for (uint256 j = 1; j < a.length; j++) {
            xx = mulmod(xx, x, Secp256k1.N);
            r = addmod(r, mulmod(a[j], xx, Secp256k1.N), Secp256k1.N);
        }
    }

    function _lagrangeCoefficient(uint256[] memory l, uint256 i) private view returns (uint256 lambda) {
        uint256 numerator = 1;
        uint256 denominator = 1;
        uint256 minusId = Secp256k1.N - FROST.identifier(participants.addr(i));
        for (uint256 j = 0; j < l.length; j++) {
            uint256 jj = l[j];
            if (i == jj) {
                continue;
            }
            uint256 x = FROST.identifier(participants.addr(jj));
            numerator = mulmod(numerator, x, Secp256k1.N);
            denominator = mulmod(denominator, addmod(x, minusId, Secp256k1.N), Secp256k1.N);
        }
        return mulmod(numerator, Math.invModPrime(denominator, Secp256k1.N), Secp256k1.N);
    }
}
