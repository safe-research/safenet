// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Vm} from "@forge-std/Test.sol";
import {MerkleProof} from "@oz/utils/cryptography/MerkleProof.sol";
import {CommitmentShareMerkleTree} from "@test/util/CommitmentShareMerkleTree.sol";
import {ForgeSecp256k1} from "@test/util/ForgeSecp256k1.sol";
import {FROSTCoordinatorTestBase} from "@test/util/FROSTCoordinatorTestBase.sol";
import {FROSTCoordinator} from "@/FROSTCoordinator.sol";
import {FROST} from "@/libraries/FROST.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

contract FROSTCoordinatorTest is FROSTCoordinatorTestBase {
    using ForgeSecp256k1 for ForgeSecp256k1.P;

    function test_KeyGen() public {
        // Distributed key generation algorithm from the FROST white paper.
        // <https://eprint.iacr.org/2020/852.pdf>

        vm.expectEmit();
        emit FROSTCoordinator.KeyGen(
            FROSTGroupId.create(participants.root(), COUNT, THRESHOLD, bytes32(0)),
            participants.root(),
            COUNT,
            THRESHOLD,
            bytes32(0)
        );
        FROSTGroupId.T gid = coordinator.keyGen(participants.root(), COUNT, THRESHOLD, bytes32(0));

        // Round 1.1
        uint256[] memory q = new uint256[](COUNT);
        uint256[][] memory a = new uint256[][](COUNT);
        for (uint256 i = 0; i < COUNT; i++) {
            q[i] = vm.randomUint(1, Secp256k1.N - 1);
            a[i] = new uint256[](THRESHOLD);
            for (uint256 j = 0; j < THRESHOLD; j++) {
                a[i][j] = vm.randomUint(1, Secp256k1.N - 1);
            }
        }

        // Round 1.2
        FROSTCoordinator.KeyGenCommitment[] memory commitments = new FROSTCoordinator.KeyGenCommitment[](COUNT);
        for (uint256 i = 0; i < COUNT; i++) {
            FROSTCoordinator.KeyGenCommitment memory commitment = commitments[i];

            uint256 k = vm.randomUint(1, Secp256k1.N - 1);
            commitment.r = ForgeSecp256k1.g(k).toPoint();
            uint256 c = FROST.keyGenChallenge(participants.addr(i), ForgeSecp256k1.g(a[i][0]).toPoint(), commitment.r);
            commitment.mu = addmod(k, mulmod(a[i][0], c, Secp256k1.N), Secp256k1.N);
        }

        // Round 1.3
        // Note that `qq[i]` and `cc[i]` are equivalent to `commitments[i].q`
        // and `commitments[i].c` respectively. We need these additional arrays
        // in order to keep `ForgeSecp256k1.P` versions of our points, because
        // implementing elliptic curve multiplication natively on the EVM is
        // prohibitively slow, and so we need to use the built-in Forge
        // cheatcodes for doing the elliptic curve operations for the test.
        // These elliptic curve operations are done offchain anyway, so this is
        // not a concern for the actual production system.
        ForgeSecp256k1.P[] memory qq = new ForgeSecp256k1.P[](COUNT);
        ForgeSecp256k1.P[][] memory cc = new ForgeSecp256k1.P[][](COUNT);
        for (uint256 i = 0; i < COUNT; i++) {
            FROSTCoordinator.KeyGenCommitment memory commitment = commitments[i];
            qq[i] = ForgeSecp256k1.g(q[i]);
            commitment.q = qq[i].toPoint();
            cc[i] = new ForgeSecp256k1.P[](THRESHOLD);
            commitment.c = new Secp256k1.Point[](THRESHOLD);
            for (uint256 j = 0; j < THRESHOLD; j++) {
                cc[i][j] = ForgeSecp256k1.g(a[i][j]);
                commitment.c[j] = cc[i][j].toPoint();
            }
        }

        // Round 1.4
        for (uint256 i = 0; i < COUNT; i++) {
            (address participant, bytes32[] memory poap) = participants.proof(i);
            FROSTCoordinator.KeyGenCommitment memory commitment = commitments[i];

            vm.expectEmit();
            emit FROSTCoordinator.KeyGenCommitted(gid, participant, commitment, i + 1 == COUNT);
            vm.prank(participant);
            coordinator.keyGenCommit(gid, poap, commitment);
        }

        // Round 1.5
        // Note that at this point `commitments` is public information that was
        // included in events emitted during the `KeyGen` process.
        for (uint256 i = 0; i < COUNT; i++) {
            FROSTCoordinator.KeyGenCommitment memory commitment = commitments[i];
            uint256 c = FROST.keyGenChallenge(participants.addr(i), commitment.c[0], commitment.r);
            Secp256k1.mulmuladd(commitment.mu, c, commitment.c[0], commitment.r);

            commitment.mu = 0;
            commitment.r = Secp256k1.Point({x: 0, y: 0});
        }

        // Round 2.1*
        FROSTCoordinator.KeyGenSecretShare[] memory shares = new FROSTCoordinator.KeyGenSecretShare[](COUNT);
        for (uint256 i = 0; i < COUNT; i++) {
            FROSTCoordinator.KeyGenSecretShare memory share = shares[i];

            for (uint256 j = 0; j < COUNT; j++) {
                share.y = Secp256k1.add(share.y, _fc(cc[j], i).toPoint());
            }

            share.f = new uint256[](COUNT - 1);
            uint256 k = 0;
            for (uint256 l = 0; l < COUNT; l++) {
                if (i == l) {
                    continue;
                }

                uint256 fi = _f(a[i], l);

                // EXTENSION: We apply ECDH to encrypt the `f_i(l)` evaluation
                // for the target participant. This allows us to use the same
                // onchain coordinator for the secret shares and not require an
                // additional secret channel. This also implies that we only
                // completely delete `f` in 2.3, as we need `a_0` to recover the
                // secret shares sent by other participants.
                fi = _ecdh(fi, q[i], qq[l]);

                share.f[k++] = fi;
            }

            vm.expectEmit();
            emit FROSTCoordinator.KeyGenSecretShared(gid, participants.addr(i), share, i + 1 == COUNT);
            vm.prank(participants.addr(i));
            coordinator.keyGenSecretShare(gid, share);
        }

        // Round 2.2*
        // Note that at this point `shares` is public information that was
        // included in events emitted during the `KeyGen` process.
        uint256[][] memory f = new uint256[][](COUNT);
        for (uint256 i = 0; i < COUNT; i++) {
            f[i] = new uint256[](COUNT);
            for (uint256 l = 0; l < COUNT; l++) {
                if (i == l) {
                    continue;
                }

                // The secret shares, as per the KeyGen algorthim, are only
                // broadcast for every _other_ participant (meaning there are
                // `COUNT - 1` of them). Compute the index in the `f` array for
                // a given participant given that the share for `l` is skipped.
                f[i][l] = shares[l].f[i < l ? i : i - 1];

                // EXTENSION: We need to reverse the ECDH we applied in the
                // previous step.
                f[i][l] = _ecdh(f[i][l], q[i], qq[l]);

                Secp256k1.Point memory gf = ForgeSecp256k1.g(f[i][l]).toPoint();
                Secp256k1.Point memory fc = _fc(cc[l], i).toPoint();
                assertEq(gf.x, fc.x);
                assertEq(gf.y, fc.y);
            }
            f[i][i] = _f(a[i], i);
        }

        // Round 2.3
        uint256[] memory s = new uint256[](COUNT);
        for (uint256 i = 0; i < COUNT; i++) {
            for (uint256 l = 0; l < COUNT; l++) {
                s[i] = addmod(s[i], f[i][l], Secp256k1.N);
            }
        }

        // Round 2.4
        for (uint256 i = 0; i < COUNT; i++) {
            Secp256k1.Point memory y = ForgeSecp256k1.g(s[i]).toPoint();
            Secp256k1.Point memory yy = coordinator.participantKey(gid, participants.addr(i));
            assertEq(y.x, yy.x);
            assertEq(y.y, yy.y);
        }

        // EXTENSION: Confirmation
        for (uint256 i = 0; i < COUNT; i++) {
            vm.expectEmit();
            emit FROSTCoordinator.KeyGenConfirmed(gid, participants.addr(i), i + 1 == COUNT);
            vm.prank(participants.addr(i));
            coordinator.keyGenConfirm(gid);
        }

        // COMPLETE: Verify for testing purposes the group key that was
        // correctly derived onchain using secret information.
        {
            uint256 groupPrivateKey = 0;
            for (uint256 i = 0; i < COUNT; i++) {
                groupPrivateKey = addmod(groupPrivateKey, a[i][0], Secp256k1.N);
            }
            Vm.Wallet memory groupAccount = vm.createWallet(groupPrivateKey);
            Secp256k1.Point memory groupPublicKey = coordinator.groupKey(gid);

            assertEq(groupPublicKey.x, groupAccount.publicKeyX);
            assertEq(groupPublicKey.y, groupAccount.publicKeyY);
        }
    }

    function test_Sign() public {
        // Implementation of the two-round FROST signing protocol from RFC-9591
        // <https://datatracker.ietf.org/doc/html/rfc9591#section-5>

        (FROSTGroupId.T gid, uint256[] memory s,) = _trustedKeyGen(bytes32(0));

        // Round 1

        // We setup a commit with **a single** pair of nonces in a Merkle tree
        // full of 0s in order to speed up the test. In practice, we compute and
        // commit to trees with 1024 nonce pairs.
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

        // Round 2

        // The complete list of participants is implicitely selects all honest
        // all participants should cooperate. "honest" must be deterministic
        // such that there is no ambiguity on the set for honest validators.
        uint256[] memory honestParticipants = _honestParticipants();

        // The signature aggregator (the coordinator contract) reveals the
        // message to sign and the participants reveal their committed nonces
        // from round 1.
        bytes32 message = keccak256("Hello, Safenet!");

        vm.expectEmit();
        emit FROSTCoordinator.Sign(address(this), gid, message, FROSTSignatureId.create(gid, 0), 0);
        FROSTSignatureId.T sid = coordinator.sign(gid, message);

        for (uint256 i = 0; i < honestParticipants.length; i++) {
            uint256 h = honestParticipants[i];
            Nonces memory n = nonces[h];
            FROSTCoordinator.SignNonces memory nn = FROSTCoordinator.SignNonces({d: n.d.toPoint(), e: n.e.toPoint()});
            vm.expectEmit();
            emit FROSTCoordinator.SignRevealedNonces(sid, participants.addr(h), nn);
            vm.prank(participants.addr(h));
            coordinator.signRevealNonces(sid, nn, nonceProof);
        }

        // The `sign` algorithm from RFC-9591. Note that the algorithms assume a
        // sorted list of participants. Note that at this point, all commitment
        // nonces are available from event data (assuming a block limit for
        // participants to submit nonces before being declared "dishonest").
        // <https://datatracker.ietf.org/doc/html/rfc9591#section-5.2>
        _sortByParticipantId(honestParticipants);
        Secp256k1.Point memory groupKey = coordinator.groupKey(gid);
        FROSTCoordinator.SignSelection memory selection;
        FROST.SignatureShare[] memory shares = new FROST.SignatureShare[](honestParticipants.length);
        {
            uint256[] memory bindingFactors;
            {
                FROST.Commitment[] memory coms = new FROST.Commitment[](honestParticipants.length);
                for (uint256 i = 0; i < honestParticipants.length; i++) {
                    uint256 h = honestParticipants[i];
                    Nonces memory n = nonces[h];
                    coms[i] = FROST.Commitment({participant: participants.addr(h), d: n.d.toPoint(), e: n.e.toPoint()});
                }
                bindingFactors = FROST.bindingFactors(coordinator.groupKey(gid), coms, message);
            }

            ForgeSecp256k1.P memory groupCommitment;
            for (uint256 i = 0; i < honestParticipants.length; i++) {
                uint256 h = honestParticipants[i];
                Nonces memory n = nonces[h];
                uint256 bindingFactor = bindingFactors[i];
                ForgeSecp256k1.P memory r = ForgeSecp256k1.add(n.d, ForgeSecp256k1.mul(bindingFactor, n.e));
                shares[i].r = r.toPoint();
                shares[i].l = _lagrangeCoefficient(honestParticipants, h);
                groupCommitment = ForgeSecp256k1.add(groupCommitment, r);
            }
            selection.r = groupCommitment.toPoint();

            uint256 challenge = FROST.challenge(selection.r, groupKey, message);
            for (uint256 i = 0; i < honestParticipants.length; i++) {
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
            CommitmentShareMerkleTree.S[] memory cs = new CommitmentShareMerkleTree.S[](honestParticipants.length);
            for (uint256 i = 0; i < honestParticipants.length; i++) {
                uint256 h = honestParticipants[i];
                cs[i] = CommitmentShareMerkleTree.S({participant: participants.addr(h), r: shares[i].r, l: shares[i].l});
            }
            commitmentShares = new CommitmentShareMerkleTree(selection.r, cs);
            selection.root = commitmentShares.root();
        }

        for (uint256 i = 0; i < honestParticipants.length; i++) {
            uint256 h = honestParticipants[i];
            bytes32[] memory proof = commitmentShares.proof(i);

            vm.expectEmit();
            emit FROSTCoordinator.SignShared(sid, selection.root, participants.addr(h), shares[i].z);
            vm.prank(participants.addr(h));
            coordinator.signShare(sid, selection, shares[i], proof);
        }

        FROST.Signature memory signature = coordinator.signatureValue(sid);
        FROST.verify(groupKey, signature, message);
    }
}
