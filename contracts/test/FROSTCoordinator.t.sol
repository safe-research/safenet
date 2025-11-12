// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test, Vm} from "@forge-std/Test.sol";
import {Arrays} from "@oz/utils/Arrays.sol";
import {Hashes} from "@oz/utils/cryptography/Hashes.sol";
import {Math} from "@oz/utils/math/Math.sol";
import {CommitmentShareMerkleTree} from "@test/util/CommitmentShareMerkleTree.sol";
import {ForgeSecp256k1} from "@test/util/ForgeSecp256k1.sol";
import {ParticipantMerkleTree} from "@test/util/ParticipantMerkleTree.sol";
import {FROSTCoordinator} from "@/FROSTCoordinator.sol";
import {FROST} from "@/lib/FROST.sol";
import {Secp256k1} from "@/lib/Secp256k1.sol";

contract FROSTCoordinatorTest is Test {
    using Arrays for address[];
    using Arrays for uint256[];
    using ForgeSecp256k1 for ForgeSecp256k1.P;

    struct Nonces {
        ForgeSecp256k1.P d;
        ForgeSecp256k1.P e;
    }

    uint64 public constant COUNT = 5;
    uint64 public constant THRESHOLD = 3;

    FROSTCoordinator public coordinator;
    ParticipantMerkleTree public participants;

    function setUp() public {
        // When debegging, it may be useful to use deterministic values to check
        // intermediate steps. Uncomment the following line in order to set a
        // deterministic seed and ensure that all random values are predictable.
        //vm.setSeed(0x5afe);

        coordinator = new FROSTCoordinator();
        participants = new ParticipantMerkleTree(_randomSortedAddresses(COUNT));
    }

    function test_KeyGen() public {
        // Distributed key generation algorithm from the FROST white paper.
        // <https://eprint.iacr.org/2020/852.pdf>

        FROSTCoordinator.GroupId id = coordinator.keygen(0, participants.root(), COUNT, THRESHOLD);

        // Off-by-one errors are one of the two hardest problems in computer
        // science (along with cache invalidation and naming things). We use a
        // lot of `COUNT + 1` length arrays in the code below. This is just to
        // make our code a bit nicer to read (you access data for participant
        // `index` with `someArray[index]` instead of `someArray[index - 1]`).
        // This confusion stems from the fact that FROST participants are
        // indexed starting from `1` and not `0`. In general, the `COUNT + 1`
        // arrays are kind of as if they were a `mapping(index => ...)`. In
        // other languages, these would be `HashMap<index, ...>`. For these
        // arrays, we never use `someArray[0]`.

        // Round 1.1
        uint256[][] memory a = new uint256[][](COUNT + 1);
        for (uint256 index = 1; index <= COUNT; index++) {
            a[index] = new uint256[](THRESHOLD);
            for (uint256 j = 0; j < THRESHOLD; j++) {
                a[index][j] = vm.randomUint(0, Secp256k1.N - 1);
            }
        }

        // Round 1.2
        FROSTCoordinator.KeyGenCommitment[] memory commitments = new FROSTCoordinator.KeyGenCommitment[](COUNT + 1);
        for (uint256 index = 1; index <= COUNT; index++) {
            FROSTCoordinator.KeyGenCommitment memory commitment = commitments[index];

            uint256 k = vm.randomUint(0, Secp256k1.N - 1);
            commitment.r = ForgeSecp256k1.g(k).toPoint();
            bytes32 c = _h(index, ForgeSecp256k1.g(a[index][0]).toPoint(), commitment.r);
            commitment.mu = addmod(k, mulmod(a[index][0], uint256(c), Secp256k1.N), Secp256k1.N);
        }

        // Round 1.3
        // Note that `cc[index]` is equivalent to `commitments[index].c`. We
        // need the additional array to keep `ForgeSecp256k1.P` versions of our
        // points, because implementing elliptic curve multiplication natively
        // on the EVM is prohibitively slow, and so we need to use the built-in
        // Forge cheatcodes for doing the elliptic curve operations for the
        // test. These elliptic curve operations are done offchain anyway, so
        // this is not a concern for the actual production system.
        ForgeSecp256k1.P[][] memory cc = new ForgeSecp256k1.P[][](COUNT + 1);
        for (uint256 index = 1; index <= COUNT; index++) {
            FROSTCoordinator.KeyGenCommitment memory commitment = commitments[index];
            commitment.c = new Secp256k1.Point[](THRESHOLD);
            cc[index] = new ForgeSecp256k1.P[](THRESHOLD);
            for (uint256 j = 0; j < THRESHOLD; j++) {
                cc[index][j] = ForgeSecp256k1.g(a[index][j]);
                commitment.c[j] = cc[index][j].toPoint();
            }
        }

        // Round 1.4
        for (uint256 index = 1; index <= COUNT; index++) {
            (address participant, bytes32[] memory poap) = participants.proof(index);
            FROSTCoordinator.KeyGenCommitment memory commitment = commitments[index];

            vm.expectEmit();
            emit FROSTCoordinator.KeyGenCommitted(id, index, commitment);
            vm.prank(participant);
            coordinator.keygenCommit(id, index, poap, commitment);
        }

        // Round 1.5
        // Note that at this point `commitments` is public information that was
        // included in events emitted during the `KeyGen` process.
        for (uint256 index = 1; index <= COUNT; index++) {
            FROSTCoordinator.KeyGenCommitment memory commitment = commitments[index];
            bytes32 c = _h(index, commitment.c[0], commitment.r);
            Secp256k1.mulmuladd(commitment.mu, uint256(c), commitment.c[0], commitment.r);

            commitment.mu = 0;
            commitment.r = Secp256k1.Point({x: 0, y: 0});
        }

        // End of round 1. Note that we already have derived the group public
        // key at this point!
        {
            uint256 groupPrivateKey = 0;
            for (uint256 index = 1; index <= COUNT; index++) {
                groupPrivateKey = addmod(groupPrivateKey, a[index][0], Secp256k1.N);
            }
            Vm.Wallet memory groupAccount = vm.createWallet(groupPrivateKey);
            Secp256k1.Point memory groupPublicKey = coordinator.groupKey(id);

            assertEq(groupPublicKey.x, groupAccount.publicKeyX);
            assertEq(groupPublicKey.y, groupAccount.publicKeyY);
        }

        // Round 2.1*
        FROSTCoordinator.KeyGenSecretShare[] memory shares = new FROSTCoordinator.KeyGenSecretShare[](COUNT + 1);
        for (uint256 index = 1; index <= COUNT; index++) {
            FROSTCoordinator.KeyGenSecretShare memory share = shares[index];

            for (uint256 j = 1; j <= COUNT; j++) {
                share.y = Secp256k1.add(share.y, _fc(cc[j], index).toPoint());
            }

            share.f = new uint256[](COUNT - 1);
            uint256 i = 0;
            for (uint256 l = 1; l <= COUNT; l++) {
                if (index == l) {
                    continue;
                }

                uint256 fi = _f(a[index], l);

                // EXTENSION: We apply ECDH to encrypt the `f_i(l)` evaluation
                // for the target participant. This allows us to use the same
                // onchain coordinator for the secret shares and not require an
                // additional secret channel. This also implies that we only
                // completely delete `f` in 2.3, as we need `a_0` to recover the
                // secret shares sent by other participants.
                fi = _ecdh(fi, a[index][0], cc[l][0]);

                share.f[i++] = fi;
            }

            vm.expectEmit();
            emit FROSTCoordinator.KeyGenSecretShared(id, index, share);
            vm.prank(participants.addr(index));
            coordinator.keygenSecretShare(id, share);
        }

        // Round 2.2*
        // Note that at this point `shares` is public information that was
        // included in events emitted during the `KeyGen` process.
        uint256[][] memory f = new uint256[][](COUNT + 1);
        for (uint256 index = 1; index <= COUNT; index++) {
            f[index] = new uint256[](COUNT + 1);
            for (uint256 l = 1; l <= COUNT; l++) {
                if (index == l) {
                    continue;
                }

                // The secret shares, as per the KeyGen algorthim, are only
                // broadcast for every _other_ participant (meaning there are
                // `COUNT - 1` of them). Compute the index in the `f` array
                // for a given participant given that the array starts at
                // index `0` (unlike participant indexes), and that the share
                // for `l` is skipped.
                f[index][l] = shares[l].f[index < l ? index - 1 : index - 2];

                // EXTENSION: We need to reverse the ECDH we applied in the
                // previous step.
                f[index][l] = _ecdh(f[index][l], a[index][0], cc[l][0]);

                Secp256k1.Point memory gf = ForgeSecp256k1.g(f[index][l]).toPoint();
                Secp256k1.Point memory fc = _fc(cc[l], index).toPoint();
                assertEq(gf.x, fc.x);
                assertEq(gf.y, fc.y);
            }
            f[index][index] = _f(a[index], index);
        }

        // Round 2.3
        uint256[] memory s = new uint256[](COUNT + 1);
        for (uint256 index = 1; index <= COUNT; index++) {
            for (uint256 l = 1; l <= COUNT; l++) {
                s[index] = addmod(s[index], f[index][l], Secp256k1.N);
            }
        }
        a = new uint256[][](0);
        f = new uint256[][](0);

        // Round 2.4
        for (uint256 index = 1; index <= COUNT; index++) {
            Secp256k1.Point memory y = ForgeSecp256k1.g(s[index]).toPoint();
            Secp256k1.Point memory yy = coordinator.participantKey(id, index);
            assertEq(y.x, yy.x);
            assertEq(y.y, yy.y);
        }
    }

    function test_Sign() public {
        // Implementation of the two-round FROST signing protocol from RFC-9591
        // <https://datatracker.ietf.org/doc/html/rfc9591#section-5>

        (FROSTCoordinator.GroupId id, uint256[] memory s) = _trustedKeyGen(0);
        FROSTCoordinator.SignatureId sig = coordinator.nextSignatureId(id);

        // Round 1

        // We setup a commit with **a single** pair of nonces in a Merkle tree
        // full of 0s in order to speed up the test. In practice, we compute and
        // commit to trees with 1024 nonce pairs.
        bytes32[] memory nonceProof = new bytes32[](10);
        Nonces[] memory nonces = new Nonces[](COUNT + 1);
        {
            bytes32[] memory commitments = new bytes32[](COUNT + 1);
            for (uint256 index = 1; index <= COUNT; index++) {
                Secp256k1.Point memory secret = ForgeSecp256k1.g(s[index]).toPoint();
                Nonces memory n = nonces[index];
                n.d = ForgeSecp256k1.g(FROST.nonce(bytes32(vm.randomUint()), secret));
                n.e = ForgeSecp256k1.g(FROST.nonce(bytes32(vm.randomUint()), secret));
                // forge-lint: disable-next-line(asm-keccak256)
                bytes32 digest = keccak256(abi.encode(n.d.x(), n.d.y(), n.e.x(), n.e.y()));
                for (uint256 i = 0; i < nonceProof.length; i++) {
                    digest = Hashes.efficientKeccak256(digest, 0);
                }
                commitments[index] = digest;
            }
            for (uint256 index = 1; index <= COUNT; index++) {
                vm.prank(participants.addr(index));
                coordinator.preprocess(id, commitments[index]);
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
        bytes32 message = keccak256("Hello, Shieldnet!");
        vm.expectEmit();
        emit FROSTCoordinator.Sign(id, sig, message);
        assertEq(
            FROSTCoordinator.SignatureId.unwrap(sig), FROSTCoordinator.SignatureId.unwrap(coordinator.sign(id, message))
        );
        for (uint256 i = 0; i < honestParticipants.length; i++) {
            uint256 index = honestParticipants[i];
            Nonces memory n = nonces[index];
            FROSTCoordinator.SignNonces memory nn = FROSTCoordinator.SignNonces({d: n.d.toPoint(), e: n.e.toPoint()});
            vm.expectEmit();
            emit FROSTCoordinator.SignRevealedNonces(sig, index, nn);
            vm.prank(participants.addr(index));
            coordinator.signRevealNonces(sig, nn, nonceProof);
        }

        // The `sign` algorithm from RFC-9591. Note that the algorithms assume a
        // sorted list of participants. Note that at this point, all commitment
        // nonces are available from event data (assuming a block limit for
        // participants to submit nonces before being declared "dishonest").
        // <https://datatracker.ietf.org/doc/html/rfc9591#section-5.2>
        honestParticipants = vm.sort(honestParticipants);
        Secp256k1.Point memory groupKey = coordinator.groupKey(id);
        uint256[] memory shares = new uint256[](honestParticipants.length);
        CommitmentShareMerkleTree.S[] memory cs = new CommitmentShareMerkleTree.S[](honestParticipants.length);
        {
            uint256[] memory bindingFactors;
            {
                FROST.Commitment[] memory coms = new FROST.Commitment[](honestParticipants.length);
                for (uint256 i = 0; i < honestParticipants.length; i++) {
                    uint256 index = honestParticipants[i];
                    Nonces memory n = nonces[index];
                    coms[i] = FROST.Commitment({index: index, d: n.d.toPoint(), e: n.e.toPoint()});
                }
                bindingFactors = FROST.bindingFactors(coordinator.groupKey(id), coms, message);
            }

            ForgeSecp256k1.P memory groupCommitment;
            ForgeSecp256k1.P[] memory r = new ForgeSecp256k1.P[](honestParticipants.length);
            for (uint256 i = 0; i < honestParticipants.length; i++) {
                uint256 index = honestParticipants[i];
                Nonces memory n = nonces[index];
                uint256 bindingFactor = bindingFactors[i];
                r[i] = ForgeSecp256k1.add(n.d, ForgeSecp256k1.mul(bindingFactor, n.e));
                groupCommitment = ForgeSecp256k1.add(groupCommitment, r[i]);
            }
            uint256 challenge = FROST.challenge(groupCommitment.toPoint(), groupKey, message);

            // Extension: the onchain computed group signature (R, z) is grouped by
            // a commitment share Merkle tree root. This makes it so misbehaving
            // participants can't influence the final onchain signature value for
            // a correctly behaving set.
            for (uint256 i = 0; i < honestParticipants.length; i++) {
                uint256 index = honestParticipants[i];
                uint256 lambda = _lagrangeCoefficient(honestParticipants, index);
                uint256 cl = mulmod(challenge, lambda, Secp256k1.N);
                cs[i] = CommitmentShareMerkleTree.S({index: index, r: r[i].toPoint(), cl: cl});
            }

            for (uint256 i = 0; i < honestParticipants.length; i++) {
                uint256 index = honestParticipants[i];
                uint256 sk = s[index];
                Nonces memory n = nonces[index];
                shares[i] = addmod(
                    n.d.w.privateKey,
                    addmod(
                        mulmod(n.e.w.privateKey, bindingFactors[i], Secp256k1.N),
                        mulmod(cs[i].cl, sk, Secp256k1.N),
                        Secp256k1.N
                    ),
                    Secp256k1.N
                );
            }
        }
        CommitmentShareMerkleTree commitmentShares = new CommitmentShareMerkleTree(cs);

        for (uint256 i = 0; i < honestParticipants.length; i++) {
            uint256 index = honestParticipants[i];
            bytes32 root = commitmentShares.root();
            bytes32[] memory proof = commitmentShares.proof(i);

            vm.expectEmit();
            emit FROSTCoordinator.SignShare(sig, index, shares[i]);
            vm.prank(participants.addr(index));
            coordinator.signShare(sig, root, cs[i].r, shares[i], cs[i].cl, proof);
        }

        (Secp256k1.Point memory rr, uint256 zz) = coordinator.groupSignature(sig, commitmentShares.root());
        FROST.verify(groupKey, rr, zz, message);
    }

    function _randomSortedAddresses(uint64 count) private view returns (address[] memory result) {
        result = new address[](count);
        for (uint256 i = 0; i < result.length; i++) {
            result[i] = vm.randomAddress();
        }
        result.sort();
    }

    function _trustedKeyGen(uint64 domain) private returns (FROSTCoordinator.GroupId id, uint256[] memory s) {
        id = coordinator.keygen(domain, participants.root(), COUNT, THRESHOLD);
        s = new uint256[](COUNT + 1);

        uint256[] memory a = new uint256[](THRESHOLD);
        for (uint256 j = 0; j < THRESHOLD; j++) {
            a[j] = vm.randomUint(0, Secp256k1.N - 1);
        }

        // In our trusted key gen setup, we pretend like the first participant
        // has the full polynomial for deriving all the shares, and all other
        // participants do not add anything.
        FROSTCoordinator.KeyGenCommitment memory commitment;
        commitment.c = new Secp256k1.Point[](THRESHOLD);
        for (uint256 index = 2; index <= COUNT; index++) {
            (address participant, bytes32[] memory poap) = participants.proof(index);
            vm.prank(participant);
            coordinator.keygenCommit(id, index, poap, commitment);
        }
        {
            for (uint256 j = 0; j < THRESHOLD; j++) {
                commitment.c[j] = ForgeSecp256k1.g(a[j]).toPoint();
            }
            (address participant, bytes32[] memory poap) = participants.proof(1);
            vm.prank(participant);
            coordinator.keygenCommit(id, 1, poap, commitment);
        }

        // We don't actually need to encrypt and broadcast secret shares, the
        // trusted dealer computes the private keys for each participant.
        FROSTCoordinator.KeyGenSecretShare memory share;
        share.f = new uint256[](COUNT - 1);
        for (uint256 index = 1; index <= COUNT; index++) {
            s[index] = _f(a, index);
            share.y = ForgeSecp256k1.g(s[index]).toPoint();
            vm.prank(participants.addr(index));
            coordinator.keygenSecretShare(id, share);
        }

        // For debugging purposes, also provide the group private key to the
        // caller (even if this is typically not available).
        s[0] = a[0];

        assertEq(
            keccak256(abi.encode(coordinator.groupKey(id))), keccak256(abi.encode(ForgeSecp256k1.g(s[0]).toPoint()))
        );
    }

    function _h(uint256 index, Secp256k1.Point memory ga0, Secp256k1.Point memory r)
        private
        pure
        returns (bytes32 digest)
    {
        // NOTE: This hash function is for demonstration only - we should use a
        // suitable hash function for the actual proof.
        return keccak256(abi.encodePacked(index, "test", ga0.x, ga0.y, r.x, r.y));
    }

    function _f(uint256[] memory a, uint256 x) private pure returns (uint256 r) {
        r = a[0];
        uint256 xx = 1;
        for (uint256 i = 1; i < a.length; i++) {
            xx = mulmod(xx, x, Secp256k1.N);
            r = addmod(r, mulmod(a[i], xx, Secp256k1.N), Secp256k1.N);
        }
    }

    function _fc(ForgeSecp256k1.P[] memory c, uint256 x) private returns (ForgeSecp256k1.P memory r) {
        r = c[0];
        uint256 xx = 1;
        for (uint256 i = 1; i < c.length; i++) {
            xx = mulmod(xx, x, Secp256k1.N);
            r = ForgeSecp256k1.add(r, ForgeSecp256k1.mul(xx, c[i]));
        }
    }

    function _ecdh(uint256 x, uint256 k, ForgeSecp256k1.P memory q) private returns (uint256 encX) {
        return x ^ ForgeSecp256k1.mul(k, q).toPoint().x;
    }

    function _honestParticipants() private returns (uint256[] memory indexes) {
        indexes = new uint256[](COUNT);
        for (uint256 i = 0; i < COUNT; i++) {
            indexes[i] = i + 1;
        }

        indexes = vm.shuffle(indexes);
        uint256 length = vm.randomUint(THRESHOLD, COUNT);
        assembly ("memory-safe") {
            mstore(indexes, length)
        }
    }

    function _lagrangeCoefficient(uint256[] memory l, uint256 index) private view returns (uint256 lambda) {
        uint256 numerator = 1;
        uint256 denominator = 1;
        uint256 minusIndex = Secp256k1.N - index;
        for (uint256 i = 0; i < l.length; i++) {
            uint256 x = l.unsafeMemoryAccess(i);
            if (x == index) {
                continue;
            }
            numerator = mulmod(numerator, x, Secp256k1.N);
            denominator = mulmod(denominator, addmod(x, minusIndex, Secp256k1.N), Secp256k1.N);
        }
        return mulmod(numerator, Math.invModPrime(denominator, Secp256k1.N), Secp256k1.N);
    }
}
