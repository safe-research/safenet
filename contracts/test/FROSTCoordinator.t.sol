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

        FROSTCoordinator.GroupId gid = coordinator.keyGen(0, participants.root(), COUNT, THRESHOLD);

        // Off-by-one errors are one of the two hardest problems in computer
        // science (along with cache invalidation and naming things). We use a
        // lot of `COUNT + 1` length arrays in the code below. This is just to
        // make our code a bit nicer to read (you access data for participant
        // `identifier` with `someArray[identifier]` instead of
        // `someArray[identifier - 1]`). This confusion stems from the fact that
        // FROST participants identifiers start from `1` and not `0`. In
        // general, the `COUNT + 1` arrays are kind of as if they were a
        // `mapping(identifier => ...)`. In other languages, these would be
        // `HashMap<FROST.Identifier, ...>`. For these arrays, we never use
        // `someArray[0]`.

        // Round 1.1
        uint256[][] memory a = new uint256[][](COUNT + 1);
        for (uint256 identifier = 1; identifier <= COUNT; identifier++) {
            a[identifier] = new uint256[](THRESHOLD);
            for (uint256 j = 0; j < THRESHOLD; j++) {
                a[identifier][j] = vm.randomUint(0, Secp256k1.N - 1);
            }
        }

        // Round 1.2
        FROSTCoordinator.KeyGenCommitment[] memory commitments = new FROSTCoordinator.KeyGenCommitment[](COUNT + 1);
        for (uint256 identifier = 1; identifier <= COUNT; identifier++) {
            FROSTCoordinator.KeyGenCommitment memory commitment = commitments[identifier];

            uint256 k = vm.randomUint(0, Secp256k1.N - 1);
            commitment.r = ForgeSecp256k1.g(k).toPoint();
            uint256 c = FROST.keyGenChallenge(
                FROST.newIdentifier(identifier), ForgeSecp256k1.g(a[identifier][0]).toPoint(), commitment.r
            );
            commitment.mu = addmod(k, mulmod(a[identifier][0], c, Secp256k1.N), Secp256k1.N);
        }

        // Round 1.3
        // Note that `cc[identifier]` is equivalent to
        // `commitments[identifier].c`. We need the additional array to keep
        // `ForgeSecp256k1.P` versions of our points, because implementing
        // elliptic curve multiplication natively on the EVM is prohibitively
        // slow, and so we need to use the built-in Forge cheatcodes for doing
        // the elliptic curve operations for the test. These elliptic curve
        // operations are done offchain anyway, so this is not a concern for the
        // actual production system.
        ForgeSecp256k1.P[][] memory cc = new ForgeSecp256k1.P[][](COUNT + 1);
        for (uint256 identifier = 1; identifier <= COUNT; identifier++) {
            FROSTCoordinator.KeyGenCommitment memory commitment = commitments[identifier];
            commitment.c = new Secp256k1.Point[](THRESHOLD);
            cc[identifier] = new ForgeSecp256k1.P[](THRESHOLD);
            for (uint256 j = 0; j < THRESHOLD; j++) {
                cc[identifier][j] = ForgeSecp256k1.g(a[identifier][j]);
                commitment.c[j] = cc[identifier][j].toPoint();
            }
        }

        // Round 1.4
        for (uint256 identifier = 1; identifier <= COUNT; identifier++) {
            (address participant, bytes32[] memory poap) = participants.proof(identifier);
            FROSTCoordinator.KeyGenCommitment memory commitment = commitments[identifier];

            vm.expectEmit();
            emit FROSTCoordinator.KeyGenCommitted(gid, FROST.newIdentifier(identifier), commitment);
            vm.prank(participant);
            coordinator.keyGenCommit(gid, FROST.newIdentifier(identifier), poap, commitment);
        }

        // Round 1.5
        // Note that at this point `commitments` is public information that was
        // included in events emitted during the `KeyGen` process.
        for (uint256 identifier = 1; identifier <= COUNT; identifier++) {
            FROSTCoordinator.KeyGenCommitment memory commitment = commitments[identifier];
            uint256 c = FROST.keyGenChallenge(FROST.newIdentifier(identifier), commitment.c[0], commitment.r);
            Secp256k1.mulmuladd(commitment.mu, c, commitment.c[0], commitment.r);

            commitment.mu = 0;
            commitment.r = Secp256k1.Point({x: 0, y: 0});
        }

        // End of round 1. Note that we already have derived the group public
        // key at this point!
        {
            uint256 groupPrivateKey = 0;
            for (uint256 identifier = 1; identifier <= COUNT; identifier++) {
                groupPrivateKey = addmod(groupPrivateKey, a[identifier][0], Secp256k1.N);
            }
            Vm.Wallet memory groupAccount = vm.createWallet(groupPrivateKey);
            Secp256k1.Point memory groupPublicKey = coordinator.groupKey(gid);

            assertEq(groupPublicKey.x, groupAccount.publicKeyX);
            assertEq(groupPublicKey.y, groupAccount.publicKeyY);
        }

        // Round 2.1*
        FROSTCoordinator.KeyGenSecretShare[] memory shares = new FROSTCoordinator.KeyGenSecretShare[](COUNT + 1);
        for (uint256 identifier = 1; identifier <= COUNT; identifier++) {
            FROSTCoordinator.KeyGenSecretShare memory share = shares[identifier];

            for (uint256 j = 1; j <= COUNT; j++) {
                share.y = Secp256k1.add(share.y, _fc(cc[j], identifier).toPoint());
            }

            share.f = new uint256[](COUNT - 1);
            uint256 i = 0;
            for (uint256 l = 1; l <= COUNT; l++) {
                if (identifier == l) {
                    continue;
                }

                uint256 fi = _f(a[identifier], l);

                // EXTENSION: We apply ECDH to encrypt the `f_i(l)` evaluation
                // for the target participant. This allows us to use the same
                // onchain coordinator for the secret shares and not require an
                // additional secret channel. This also implies that we only
                // completely delete `f` in 2.3, as we need `a_0` to recover the
                // secret shares sent by other participants.
                fi = _ecdh(fi, a[identifier][0], cc[l][0]);

                share.f[i++] = fi;
            }

            vm.expectEmit();
            emit FROSTCoordinator.KeyGenSecretShared(gid, FROST.newIdentifier(identifier), share);
            vm.prank(participants.addr(identifier));
            coordinator.keyGenSecretShare(gid, share);
        }

        // Round 2.2*
        // Note that at this point `shares` is public information that was
        // included in events emitted during the `KeyGen` process.
        uint256[][] memory f = new uint256[][](COUNT + 1);
        for (uint256 identifier = 1; identifier <= COUNT; identifier++) {
            f[identifier] = new uint256[](COUNT + 1);
            for (uint256 l = 1; l <= COUNT; l++) {
                if (identifier == l) {
                    continue;
                }

                // The secret shares, as per the KeyGen algorthim, are only
                // broadcast for every _other_ participant (meaning there are
                // `COUNT - 1` of them). Compute the identifier in the `f` array
                // for a given participant given that the array starts at
                // identifier `0` (unlike participant identifieres), and that
                // the share for `l` is skipped.
                f[identifier][l] = shares[l].f[identifier < l ? identifier - 1 : identifier - 2];

                // EXTENSION: We need to reverse the ECDH we applied in the
                // previous step.
                f[identifier][l] = _ecdh(f[identifier][l], a[identifier][0], cc[l][0]);

                Secp256k1.Point memory gf = ForgeSecp256k1.g(f[identifier][l]).toPoint();
                Secp256k1.Point memory fc = _fc(cc[l], identifier).toPoint();
                assertEq(gf.x, fc.x);
                assertEq(gf.y, fc.y);
            }
            f[identifier][identifier] = _f(a[identifier], identifier);
        }

        // Round 2.3
        uint256[] memory s = new uint256[](COUNT + 1);
        for (uint256 identifier = 1; identifier <= COUNT; identifier++) {
            for (uint256 l = 1; l <= COUNT; l++) {
                s[identifier] = addmod(s[identifier], f[identifier][l], Secp256k1.N);
            }
        }
        a = new uint256[][](0);
        f = new uint256[][](0);

        // Round 2.4
        for (uint256 identifier = 1; identifier <= COUNT; identifier++) {
            Secp256k1.Point memory y = ForgeSecp256k1.g(s[identifier]).toPoint();
            Secp256k1.Point memory yy = coordinator.participantKey(gid, FROST.newIdentifier(identifier));
            assertEq(y.x, yy.x);
            assertEq(y.y, yy.y);
        }
    }

    function test_Sign() public {
        // Implementation of the two-round FROST signing protocol from RFC-9591
        // <https://datatracker.ietf.org/doc/html/rfc9591#section-5>

        (FROSTCoordinator.GroupId gid, uint256[] memory s) = _trustedKeyGen(0);
        FROSTCoordinator.SignatureId sid = coordinator.nextSignatureId(gid);

        // Round 1

        // We setup a commit with **a single** pair of nonces in a Merkle tree
        // full of 0s in order to speed up the test. In practice, we compute and
        // commit to trees with 1024 nonce pairs.
        bytes32[] memory nonceProof = new bytes32[](10);
        Nonces[] memory nonces = new Nonces[](COUNT + 1);
        {
            bytes32[] memory commitments = new bytes32[](COUNT + 1);
            for (uint256 identifier = 1; identifier <= COUNT; identifier++) {
                Nonces memory n = nonces[identifier];
                n.d = ForgeSecp256k1.g(FROST.nonce(bytes32(vm.randomUint()), s[identifier]));
                n.e = ForgeSecp256k1.g(FROST.nonce(bytes32(vm.randomUint()), s[identifier]));
                // forge-lint: disable-next-line(asm-keccak256)
                bytes32 leaf = keccak256(abi.encode(0, n.d.x(), n.d.y(), n.e.x(), n.e.y()));
                commitments[identifier] = MerkleProof.processProof(nonceProof, leaf);
            }
            for (uint256 identifier = 1; identifier <= COUNT; identifier++) {
                vm.prank(participants.addr(identifier));
                coordinator.preprocess(gid, commitments[identifier]);
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
        {
            vm.expectEmit();
            emit FROSTCoordinator.Sign(gid, sid, message);
            FROSTCoordinator.SignatureId actualSid = coordinator.sign(gid, message);
            assertEq(FROSTCoordinator.SignatureId.unwrap(sid), FROSTCoordinator.SignatureId.unwrap(actualSid));
        }
        for (uint256 i = 0; i < honestParticipants.length; i++) {
            uint256 identifier = honestParticipants[i];
            Nonces memory n = nonces[identifier];
            FROSTCoordinator.SignNonces memory nn = FROSTCoordinator.SignNonces({d: n.d.toPoint(), e: n.e.toPoint()});
            vm.expectEmit();
            emit FROSTCoordinator.SignRevealedNonces(sid, FROST.newIdentifier(identifier), nn);
            vm.prank(participants.addr(identifier));
            coordinator.signRevealNonces(sid, nn, nonceProof);
        }

        // The `sign` algorithm from RFC-9591. Note that the algorithms assume a
        // sorted list of participants. Note that at this point, all commitment
        // nonces are available from event data (assuming a block limit for
        // participants to submit nonces before being declared "dishonest").
        // <https://datatracker.ietf.org/doc/html/rfc9591#section-5.2>
        honestParticipants = vm.sort(honestParticipants);
        Secp256k1.Point memory groupKey = coordinator.groupKey(gid);
        uint256[] memory shares = new uint256[](honestParticipants.length);
        CommitmentShareMerkleTree.S[] memory cs = new CommitmentShareMerkleTree.S[](honestParticipants.length);
        {
            uint256[] memory bindingFactors;
            {
                FROST.Commitment[] memory coms = new FROST.Commitment[](honestParticipants.length);
                for (uint256 i = 0; i < honestParticipants.length; i++) {
                    uint256 identifier = honestParticipants[i];
                    Nonces memory n = nonces[identifier];
                    coms[i] = FROST.Commitment({
                        identifier: FROST.newIdentifier(identifier), d: n.d.toPoint(), e: n.e.toPoint()
                    });
                }
                bindingFactors = FROST.bindingFactors(coordinator.groupKey(gid), coms, message);
            }

            ForgeSecp256k1.P memory groupCommitment;
            ForgeSecp256k1.P[] memory r = new ForgeSecp256k1.P[](honestParticipants.length);
            for (uint256 i = 0; i < honestParticipants.length; i++) {
                uint256 identifier = honestParticipants[i];
                Nonces memory n = nonces[identifier];
                uint256 bindingFactor = bindingFactors[i];
                r[i] = ForgeSecp256k1.add(n.d, ForgeSecp256k1.mul(bindingFactor, n.e));
                groupCommitment = ForgeSecp256k1.add(groupCommitment, r[i]);
            }
            uint256 challenge = FROST.challenge(groupCommitment.toPoint(), groupKey, message);

            // Extension: the onchain computed group signature (R, z) is grouped
            // by a commitment share Merkle tree root. This makes it so
            // misbehaving participants can't influence the final onchain
            // signature value for a correctly behaving set.
            for (uint256 i = 0; i < honestParticipants.length; i++) {
                uint256 identifier = honestParticipants[i];
                uint256 lambda = _lagrangeCoefficient(honestParticipants, identifier);
                uint256 cl = mulmod(challenge, lambda, Secp256k1.N);
                cs[i] = CommitmentShareMerkleTree.S({
                    identifier: FROST.newIdentifier(identifier), r: r[i].toPoint(), cl: cl
                });
            }

            for (uint256 i = 0; i < honestParticipants.length; i++) {
                uint256 identifier = honestParticipants[i];
                uint256 sk = s[identifier];
                Nonces memory n = nonces[identifier];
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
            uint256 identifier = honestParticipants[i];
            bytes32 root = commitmentShares.root();
            bytes32[] memory proof = commitmentShares.proof(i);

            vm.expectEmit();
            emit FROSTCoordinator.SignShare(sid, FROST.newIdentifier(identifier), shares[i]);
            vm.prank(participants.addr(identifier));
            coordinator.signShare(sid, root, cs[i].r, shares[i], cs[i].cl, proof);
        }

        (Secp256k1.Point memory rr, uint256 zz) = coordinator.groupSignature(sid, commitmentShares.root());
        FROST.verify(groupKey, rr, zz, message);
    }

    function _randomSortedAddresses(uint64 count) private view returns (address[] memory result) {
        result = new address[](count);
        for (uint256 i = 0; i < result.length; i++) {
            result[i] = vm.randomAddress();
        }
        result.sort();
    }

    function _trustedKeyGen(uint64 domain) private returns (FROSTCoordinator.GroupId gid, uint256[] memory s) {
        gid = coordinator.keyGen(domain, participants.root(), COUNT, THRESHOLD);
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
        for (uint256 identifier = 2; identifier <= COUNT; identifier++) {
            (address participant, bytes32[] memory poap) = participants.proof(identifier);
            vm.prank(participant);
            coordinator.keyGenCommit(gid, FROST.newIdentifier(identifier), poap, commitment);
        }
        {
            for (uint256 j = 0; j < THRESHOLD; j++) {
                commitment.c[j] = ForgeSecp256k1.g(a[j]).toPoint();
            }
            (address participant, bytes32[] memory poap) = participants.proof(1);
            vm.prank(participant);
            coordinator.keyGenCommit(gid, FROST.newIdentifier(1), poap, commitment);
        }

        // We don't actually need to encrypt and broadcast secret shares, the
        // trusted dealer computes the private keys for each participant.
        FROSTCoordinator.KeyGenSecretShare memory share;
        share.f = new uint256[](COUNT - 1);
        for (uint256 identifier = 1; identifier <= COUNT; identifier++) {
            s[identifier] = _f(a, identifier);
            share.y = ForgeSecp256k1.g(s[identifier]).toPoint();
            vm.prank(participants.addr(identifier));
            coordinator.keyGenSecretShare(gid, share);
        }

        // For debugging purposes, also provide the group private key to the
        // caller (even if this is typically not available).
        s[0] = a[0];

        assertEq(
            keccak256(abi.encode(coordinator.groupKey(gid))), keccak256(abi.encode(ForgeSecp256k1.g(s[0]).toPoint()))
        );
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

    function _honestParticipants() private returns (uint256[] memory identifiers) {
        identifiers = new uint256[](COUNT);
        for (uint256 i = 0; i < COUNT; i++) {
            identifiers[i] = i + 1;
        }

        identifiers = vm.shuffle(identifiers);
        uint256 length = vm.randomUint(THRESHOLD, COUNT);
        assembly ("memory-safe") {
            mstore(identifiers, length)
        }
    }

    function _lagrangeCoefficient(uint256[] memory l, uint256 identifier) private view returns (uint256 lambda) {
        uint256 numerator = 1;
        uint256 denominator = 1;
        uint256 minusIdentifier = Secp256k1.N - identifier;
        for (uint256 i = 0; i < l.length; i++) {
            uint256 x = l.unsafeMemoryAccess(i);
            if (x == identifier) {
                continue;
            }
            numerator = mulmod(numerator, x, Secp256k1.N);
            denominator = mulmod(denominator, addmod(x, minusIdentifier, Secp256k1.N), Secp256k1.N);
        }
        return mulmod(numerator, Math.invModPrime(denominator, Secp256k1.N), Secp256k1.N);
    }
}
