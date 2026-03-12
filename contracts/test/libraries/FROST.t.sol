// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {FROST} from "@/libraries/FROST.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";
import {ForgeSecp256k1} from "@test/util/ForgeSecp256k1.sol";

contract FROSTTest is Test {
    using ForgeSecp256k1 for ForgeSecp256k1.P;

    function test_Identifier() public view {
        address participant = 0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045;
        uint256 id = FROST.identifier(participant);
        assertEq(id, 0xe3faf3d5fec69256091d32a1e942082b9541ff7f2c928745c0d01e922879745b);
    }

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
            participant: address(3), d: ForgeSecp256k1.g(0xd3).toPoint(), e: ForgeSecp256k1.g(0xe3).toPoint()
        });
        commitments[1] = FROST.Commitment({
            participant: address(2), d: ForgeSecp256k1.g(0xd2).toPoint(), e: ForgeSecp256k1.g(0xe2).toPoint()
        });
        commitments[2] = FROST.Commitment({
            participant: address(1), d: ForgeSecp256k1.g(0xd1).toPoint(), e: ForgeSecp256k1.g(0xe1).toPoint()
        });
        bytes32 message = keccak256("hello");

        // Note that commitments must be ordered _by identifier_ and not by
        // participant.
        assertLt(FROST.identifier(commitments[0].participant), FROST.identifier(commitments[1].participant));
        assertLt(FROST.identifier(commitments[1].participant), FROST.identifier(commitments[2].participant));

        uint256[] memory bindingFactors = FROST.bindingFactors(y, commitments, message);
        assertEq(bindingFactors.length, 3);
        // TODO: Derive these values with `frost-secp256k1` crate.
        assertEq(bindingFactors[0], 0x245a80a97dadad3e97e2be4cdfc330c48e591ef9e8786d28490b22d4276249d6);
        assertEq(bindingFactors[1], 0x5eeb6723fe85fe18a3828e4ec862989f3b207ef95ce42e82944ac098e5d4b50e);
        assertEq(bindingFactors[2], 0x19231f8e14a3a1918fd193a21f1c8e045e8192bec2f8f420013c138624a9f553);
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

        uint256 c = FROST.keyGenChallenge(address(1), phi, r);
        assertEq(c, 0xd11b55fe7ad428ecdeefa22b651e2caeb2b8a8443271fc60f84a2ca9ef1aa167);
    }
}
