// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
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

abstract contract FROSTCoordinatorTestBase is Test {
    using Arrays for address[];
    using Arrays for uint256[];
    using ForgeSecp256k1 for ForgeSecp256k1.P;

    struct Nonces {
        ForgeSecp256k1.P d;
        ForgeSecp256k1.P e;
    }

    uint16 public constant COUNT = 5;
    uint16 public constant THRESHOLD = 3;

    FROSTCoordinator public coordinator;
    ParticipantMerkleTree public participants;

    function setUp() public virtual {
        coordinator = new FROSTCoordinator();
        participants = new ParticipantMerkleTree(_randomSortedAddresses(COUNT));
    }

    /// @dev Trusted dealer key generation. Returns the group ID, per-participant
    /// secret keys, and the group private key scalar (for debugging).
    function _trustedKeyGen(bytes32 context) internal returns (FROSTGroupId.T gid, uint256[] memory s, uint256 gs) {
        s = new uint256[](COUNT);

        uint256[] memory a = new uint256[](THRESHOLD);
        for (uint256 j = 0; j < THRESHOLD; j++) {
            a[j] = vm.randomUint(0, Secp256k1.N - 1);
        }

        FROSTCoordinator.KeyGenCommitment memory commitment;
        // Because we are in a trusted setup, we don't actually need to encrypt
        // anything. Specify a dummy encryption key.
        commitment.q = ForgeSecp256k1.g(1).toPoint();
        // In our trusted key gen setup, we pretend like the first participant
        // has the full polynomial for deriving all the shares, and all other
        // participants do not add anything.
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

        // We don't actually need to encrypt and broadcast secret shares, the
        // trusted dealer computes the private keys for each participant.
        FROSTCoordinator.KeyGenSecretShare memory share;
        share.f = new uint256[](COUNT - 1);
        for (uint256 i = 0; i < COUNT; i++) {
            s[i] = _f(a, i);
            share.y = ForgeSecp256k1.g(s[i]).toPoint();
            vm.prank(participants.addr(i));
            coordinator.keyGenSecretShare(gid, share);
        }

        // We now finalize the key generation.
        for (uint256 i = 0; i < COUNT; i++) {
            vm.prank(participants.addr(i));
            coordinator.keyGenConfirm(gid);
        }

        // For debugging purposes, also provide the group private key to the
        // caller (even if this is typically not available).
        gs = a[0];

        assertEq(
            keccak256(abi.encode(coordinator.groupKey(gid))), keccak256(abi.encode(ForgeSecp256k1.g(gs).toPoint()))
        );
    }

    /// @dev Runs a complete FROST signing ceremony for `signers` (indices into
    /// `participants`). Preprocesses for all COUNT participants, then has only
    /// `signers` reveal nonces and submit shares.
    function _trustedSign(FROSTGroupId.T gid, uint256[] memory s, bytes32 message, uint256[] memory signers)
        internal
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

        sid = coordinator.sign(gid, message);

        _sortByParticipantId(signers);

        for (uint256 i = 0; i < signers.length; i++) {
            uint256 h = signers[i];
            Nonces memory n = nonces[h];
            FROSTCoordinator.SignNonces memory nn = FROSTCoordinator.SignNonces({d: n.d.toPoint(), e: n.e.toPoint()});
            vm.prank(participants.addr(h));
            coordinator.signRevealNonces(sid, nn, nonceProof);
        }

        Secp256k1.Point memory groupKey = coordinator.groupKey(gid);
        FROSTCoordinator.SignSelection memory selection;
        FROST.SignatureShare[] memory shares = new FROST.SignatureShare[](signers.length);

        {
            uint256[] memory bindingFactors;
            {
                FROST.Commitment[] memory coms = new FROST.Commitment[](signers.length);
                for (uint256 i = 0; i < signers.length; i++) {
                    uint256 h = signers[i];
                    Nonces memory n = nonces[h];
                    coms[i] = FROST.Commitment({participant: participants.addr(h), d: n.d.toPoint(), e: n.e.toPoint()});
                }
                bindingFactors = FROST.bindingFactors(groupKey, coms, message);
            }

            ForgeSecp256k1.P memory groupCommitment;
            for (uint256 i = 0; i < signers.length; i++) {
                uint256 h = signers[i];
                Nonces memory n = nonces[h];
                uint256 bindingFactor = bindingFactors[i];
                ForgeSecp256k1.P memory r = ForgeSecp256k1.add(n.d, ForgeSecp256k1.mul(bindingFactor, n.e));
                shares[i].r = r.toPoint();
                shares[i].l = _lagrangeCoefficient(signers, h);
                groupCommitment = ForgeSecp256k1.add(groupCommitment, r);
            }
            selection.r = groupCommitment.toPoint();

            uint256 challenge = FROST.challenge(selection.r, groupKey, message);
            for (uint256 i = 0; i < signers.length; i++) {
                uint256 h = signers[i];
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
            CommitmentShareMerkleTree.S[] memory cs = new CommitmentShareMerkleTree.S[](signers.length);
            for (uint256 i = 0; i < signers.length; i++) {
                uint256 h = signers[i];
                cs[i] = CommitmentShareMerkleTree.S({participant: participants.addr(h), r: shares[i].r, l: shares[i].l});
            }
            commitmentShares = new CommitmentShareMerkleTree(selection.r, cs);
            selection.root = commitmentShares.root();
        }

        for (uint256 i = 0; i < signers.length; i++) {
            uint256 h = signers[i];
            bytes32[] memory proof = commitmentShares.proof(i);
            vm.prank(participants.addr(h));
            coordinator.signShare(sid, selection, shares[i], proof);
        }
    }

    /// @dev Convenience overload that picks a random honest subset via
    /// `_honestParticipants`.
    function _trustedSign(FROSTGroupId.T gid, uint256[] memory s, bytes32 message)
        internal
        returns (FROSTSignatureId.T)
    {
        return _trustedSign(gid, s, message, _honestParticipants());
    }

    function _randomSortedAddresses(uint16 count) internal view returns (address[] memory result) {
        result = new address[](count);
        for (uint256 i = 0; i < result.length; i++) {
            result[i] = vm.randomAddress();
        }
        result.sort();
    }

    function _f(uint256[] memory a, uint256 i) internal view returns (uint256 r) {
        r = a[0];
        uint256 x = FROST.identifier(participants.addr(i));
        uint256 xx = 1;
        for (uint256 j = 1; j < a.length; j++) {
            xx = mulmod(xx, x, Secp256k1.N);
            r = addmod(r, mulmod(a[j], xx, Secp256k1.N), Secp256k1.N);
        }
    }

    function _fc(ForgeSecp256k1.P[] memory c, uint256 i) internal returns (ForgeSecp256k1.P memory r) {
        r = c[0];
        uint256 x = FROST.identifier(participants.addr(i));
        uint256 xx = 1;
        for (uint256 j = 1; j < c.length; j++) {
            xx = mulmod(xx, x, Secp256k1.N);
            r = ForgeSecp256k1.add(r, ForgeSecp256k1.mul(xx, c[j]));
        }
    }

    function _ecdh(uint256 x, uint256 k, ForgeSecp256k1.P memory q) internal returns (uint256 encX) {
        return x ^ ForgeSecp256k1.mul(k, q).toPoint().x;
    }

    function _honestParticipants() internal returns (uint256[] memory result) {
        result = new uint256[](COUNT);
        for (uint256 i = 0; i < COUNT; i++) {
            result[i] = i;
        }

        result = vm.shuffle(result);
        uint256 length = vm.randomUint(THRESHOLD, COUNT);
        assembly ("memory-safe") {
            mstore(result, length)
        }
    }

    function _sortByParticipantId(uint256[] memory ps) internal view {
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

    function _lagrangeCoefficient(uint256[] memory l, uint256 i) internal view returns (uint256 lambda) {
        uint256 numerator = 1;
        uint256 denominator = 1;
        uint256 minusId = Secp256k1.N - FROST.identifier(participants.addr(i));
        for (uint256 j = 0; j < l.length; j++) {
            uint256 jj = l.unsafeMemoryAccess(j);
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
