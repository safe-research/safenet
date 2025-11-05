// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test, Vm} from "@forge-std/Test.sol";
import {ForgeSecp256k1} from "@test/util/ForgeSecp256k1.sol";
import {ParticipantMerkleTree} from "@test/util/ParticipantMerkleTree.sol";
import {FROSTCoordinator} from "@/base/FROSTCoordinator.sol";
import {Secp256k1} from "@/lib/Secp256k1.sol";

contract FROSTCoordinatorTest is Test {
    using ForgeSecp256k1 for ForgeSecp256k1.P;

    uint128 public constant COUNT = 5;
    uint128 public constant THRESHOLD = 3;

    FROSTCoordinator public coordinator;
    ParticipantMerkleTree public participants;

    function setUp() public {
        coordinator = new FROSTCoordinator();
        participants = new ParticipantMerkleTree(_randomSortedAddresses(uint256(COUNT)));
    }

    function test_KeyGen() public {
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
        // points, because implementing elliptic curve addition natively on the
        // EVM is prohibitively slow, and so we need to use the built-in Forge
        // cheatcodes for doing the elliptic curve operations for the test.
        // These elliptic curve operations are done offchain anyway, so this
        // is not a concern for the actual production system.
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
            vm.prank(participant);
            coordinator.keygenCommit(id, index, poap, commitments[index]);
        }

        // Round 1.5
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

            vm.prank(participants.addr(index));
            coordinator.keygenSecretShare(id, index, share);
        }

        // Round 2.2*
        uint256[][] memory f = new uint256[][](COUNT + 1);
        for (uint256 index = 1; index <= COUNT; index++) {
            f[index] = new uint256[](COUNT + 1);
            for (uint256 l = 1; l <= COUNT; l++) {
                if (index == l) {
                    continue;
                }

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

    function _randomSortedAddresses(uint256 length) private view returns (address[] memory result) {
        result = new address[](length);
        for (uint256 i = 0; i < length; i++) {
            result[i] = vm.randomAddress();
        }
        for (uint256 i = 0; i < length; i++) {
            for (uint256 j = i + 1; j < length; j++) {
                if (result[i] > result[j]) {
                    address temp = result[i];
                    result[i] = result[j];
                    result[j] = temp;
                }
            }
        }
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
}
