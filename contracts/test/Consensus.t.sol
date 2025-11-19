// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test, Vm} from "@forge-std/Test.sol";
import {Hashes} from "@oz/utils/cryptography/Hashes.sol";
import {Consensus} from "@/Consensus.sol";
import {FROSTCoordinator} from "@/FROSTCoordinator.sol";
import {FROST} from "@/libraries/FROST.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

contract ConsensusTest is Test {
    Vm.Wallet public group;

    FROSTCoordinator public coordinator;
    Consensus public consensus;

    function setUp() public {
        group = vm.createWallet("group");
        coordinator = new FROSTCoordinator();
        FROSTCoordinator.GroupId gid = _keyGen(0);
        consensus = new Consensus(address(coordinator), gid);
    }

    function test_EpochRolloverMessage() public {
        uint256 chainId = block.chainid;
        vm.chainId(23);
        vm.etch(0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97, address(consensus).code);
        bytes32 message = Consensus(0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97)
            .epochRolloverMessage(
                0,
                1,
                0xbaddad42,
                Secp256k1.Point({
                    x: 0x8318535b54105d4a7aae60c08fc45f9687181b4fdfc625bd1a753fa7397fed75,
                    y: 0x3547f11ca8696646f2f3acb08e31016afac23e630c5d11f59f61fef57b0d2aa5
                })
            );
        vm.chainId(chainId);

        assertEq(message, hex"31e313d5239d0a1ffe5ab3bd4d9853d63a2fc30e2adf791e56834fbe68bc3f5f");
    }

    function _keyGen(uint64 epoch) private returns (FROSTCoordinator.GroupId gid) {
        // Perform a partial key generation ceremony where leading to a group
        // key equal to `group`. This makes signing very easy for testing. Use a
        // random second participant in order to always generate a fresh group
        // ID. We don't need to complete the ceremony in order to use it with
        // the consensus contract.

        bytes32 context = bytes32((uint256(uint160(address(consensus))) << 96) | uint256(epoch));
        bytes32 participantsRoot;
        bytes32[] memory participationProof = new bytes32[](1);
        {
            participationProof[0] =
                Hashes.efficientKeccak256(bytes32(uint256(2)), bytes32(uint256(uint160(vm.randomAddress()))));
            bytes32 leaf = Hashes.efficientKeccak256(bytes32(uint256(1)), bytes32(uint256(uint160(address(this)))));
            (bytes32 left, bytes32 right) =
                leaf < participationProof[0] ? (leaf, participationProof[0]) : (participationProof[0], leaf);
            participantsRoot = Hashes.efficientKeccak256(left, right);
        }

        FROSTCoordinator.KeyGenCommitment memory commitment;
        commitment.c = new Secp256k1.Point[](2);
        commitment.c[0].x = group.publicKeyX;
        commitment.c[0].y = group.publicKeyY;
        gid = coordinator.keyGenAndCommit(
            participantsRoot, 2, 2, context, FROST.newIdentifier(1), participationProof, commitment
        );

        assertEq(
            keccak256(abi.encode(coordinator.groupKey(gid))), keccak256(abi.encode(group.publicKeyX, group.publicKeyY))
        );
    }
}
