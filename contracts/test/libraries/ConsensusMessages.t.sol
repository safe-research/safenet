// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {ConsensusMessages} from "@/libraries/ConsensusMessages.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

contract ConsensusMessagesTest is Test {
    using ConsensusMessages for bytes32;

    function test_EpochRollover() public pure {
        bytes32 message = ConsensusMessages.domain(23, 0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97)
            .epochRollover(
                0,
                1,
                0xbaddad42,
                Secp256k1.Point({
                    x: 0x8318535b54105d4a7aae60c08fc45f9687181b4fdfc625bd1a753fa7397fed75,
                    y: 0x3547f11ca8696646f2f3acb08e31016afac23e630c5d11f59f61fef57b0d2aa5
                })
            );

        assertEq(message, hex"c1e4d484d6c376741c904290cc043f4afb4618f9d567dcdd0edcbf22abae57f7");
    }
}
