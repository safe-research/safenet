// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {FROST} from "@/libraries/FROST.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";
import {ForgeSecp256k1} from "@test/util/ForgeSecp256k1.sol";

contract FROSTTest is Test {
    using ForgeSecp256k1 for ForgeSecp256k1.P;

    function test_Nonce() public view {
        bytes32 random = hex"2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a";
        uint256 secret = 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80;
        uint256 nonce = FROST.nonce(random, secret);
        assertEq(nonce, 0x03d979abaa17ca44e015f9e248c6cefc167ad21e814256f2a0a02cce70d57ba1);
    }

    function test_BindingFactors() public {
        Secp256k1.Point memory y = Secp256k1.Point({
            x: 0x8318535b54105d4a7aae60c08fc45f9687181b4fdfc625bd1a753fa7397fed75,
            y: 0x3547f11ca8696646f2f3acb08e31016afac23e630c5d11f59f61fef57b0d2aa5
        });
        FROST.Commitment[] memory commitments = new FROST.Commitment[](3);
        commitments[0] = FROST.Commitment({
            identifier: FROST.newIdentifier(1), d: ForgeSecp256k1.g(0xd1).toPoint(), e: ForgeSecp256k1.g(0xe1).toPoint()
        });
        commitments[1] = FROST.Commitment({
            identifier: FROST.newIdentifier(2), d: ForgeSecp256k1.g(0xd2).toPoint(), e: ForgeSecp256k1.g(0xe2).toPoint()
        });
        commitments[2] = FROST.Commitment({
            identifier: FROST.newIdentifier(3), d: ForgeSecp256k1.g(0xd3).toPoint(), e: ForgeSecp256k1.g(0xe3).toPoint()
        });
        bytes32 message = keccak256("hello");

        uint256[] memory bindingFactors = FROST.bindingFactors(y, commitments, message);
        assertEq(bindingFactors.length, 3);
        assertEq(bindingFactors[0], 0x3ace394f1783cd2f9647aaded69596328f98cc57c823ae5652d7275461be9bea);
        assertEq(bindingFactors[1], 0x30df3963e4aee100fa049ec729adf4e75609b4f3f699fa17cf1c593ef1cf3ecf);
        assertEq(bindingFactors[2], 0x04849a66886b4b59b920d847e334fc3f9aa355d8c152e146d3ed03c8c3a8096d);
    }

    function test_Challenge() public view {
        Secp256k1.Point memory r = Secp256k1.Point({
            x: 0x8a3802114b5b6369ae8ba7822bdb029dee0d53fc416225d9198959b83f73215b,
            y: 0x3020f80cae8f515d58686d5c6e4f1d027a1671348b6402f4e43ce525bda00fbc
        });
        Secp256k1.Point memory y = Secp256k1.Point({
            x: 0x8318535b54105d4a7aae60c08fc45f9687181b4fdfc625bd1a753fa7397fed75,
            y: 0x3547f11ca8696646f2f3acb08e31016afac23e630c5d11f59f61fef57b0d2aa5
        });
        bytes32 message = keccak256("hello");

        uint256 c = FROST.challenge(r, y, message);
        assertEq(c, 0x092370ad82e7356eb5fe89e9be058a335705b482eaa9832fb81eddd3723647b4);
    }

    function test_Verify() public view {
        Secp256k1.Point memory y = Secp256k1.Point({
            x: 0x8318535b54105d4a7aae60c08fc45f9687181b4fdfc625bd1a753fa7397fed75,
            y: 0x3547f11ca8696646f2f3acb08e31016afac23e630c5d11f59f61fef57b0d2aa5
        });
        Secp256k1.Point memory r = Secp256k1.Point({
            x: 0x8a3802114b5b6369ae8ba7822bdb029dee0d53fc416225d9198959b83f73215b,
            y: 0x3020f80cae8f515d58686d5c6e4f1d027a1671348b6402f4e43ce525bda00fbc
        });
        uint256 z = 0x209fa63cfb23b425f13b526d8af1301dcec65f9d74354b9af14f5fb86b908f8c;
        bytes32 message = keccak256("hello");

        FROST.verify(y, FROST.Signature(r, z), message);
    }

    function test_KeyGenChallenge() public view {
        Secp256k1.Point memory phi = Secp256k1.Point({
            x: 0x8318535b54105d4a7aae60c08fc45f9687181b4fdfc625bd1a753fa7397fed75,
            y: 0x3547f11ca8696646f2f3acb08e31016afac23e630c5d11f59f61fef57b0d2aa5
        });
        Secp256k1.Point memory r = Secp256k1.Point({
            x: 0x8a3802114b5b6369ae8ba7822bdb029dee0d53fc416225d9198959b83f73215b,
            y: 0x3020f80cae8f515d58686d5c6e4f1d027a1671348b6402f4e43ce525bda00fbc
        });

        uint256 c = FROST.keyGenChallenge(FROST.newIdentifier(1), phi, r);
        assertEq(c, 0xe39fcb3eef980ce5ee77898a6ed247fe78146aca2852ca4cf9f7fdcf23b4d470);
    }
}
