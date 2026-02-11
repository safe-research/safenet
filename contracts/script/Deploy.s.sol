// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTCoordinator} from "@/FROSTCoordinator.sol";
import {Consensus} from "@/Consensus.sol";
import {DeterministicDeployment} from "@script/util/DeterministicDeployment.sol";

contract DeployScript is Script {
    using DeterministicDeployment for DeterministicDeployment.Factory;

    function run() public returns (FROSTCoordinator coordinator, Consensus consensus) {
        // Required script arguments:
        address[] memory participants = vm.envAddress("PARTICIPANTS", ",");

        // Optional script arguments:
        bytes32 genesisSalt = vm.envOr("GENESIS_SALT", bytes32(0));
        bytes32 coordinatorSalt = vm.envOr("COORDINATOR_SALT", bytes32(0));
        bytes32 consensusSalt = vm.envOr("COORDINATOR_SALT", bytes32(0));

        vm.startBroadcast();

        coordinator = FROSTCoordinator(
            DeterministicDeployment.CANONICAL.deploy(coordinatorSalt, type(FROSTCoordinator).creationCode)
        );

        FROSTGroupId.T groupId = _genesisGroupId(participants, genesisSalt);
        consensus = Consensus(
            DeterministicDeployment.CANONICAL
                .deployWithArgs(consensusSalt, type(Consensus).creationCode, abi.encode(coordinator, groupId))
        );

        vm.stopBroadcast();

        console.log("FROSTCoordinator:", address(coordinator));
        console.log("Genesis Group ID: %s", vm.toString(FROSTGroupId.T.unwrap(groupId)));
        console.log("Consensus:", address(consensus));
    }

    function _merkleRoot(address[] memory participants) internal pure returns (bytes32 result) {
        uint256 depth = 0;
        for (uint256 l = participants.length; l > 1; l = (l + 1) / 2) {
            depth++;
        }

        // forge-lint: disable-next-line(incorrect-shift)
        bytes32[] memory nodes = new bytes32[](1 << depth);
        for (uint256 i = 0; i < participants.length; i++) {
            nodes[i] = keccak256(abi.encode(i + 1, participants[i]));
        }

        for (uint256 w = nodes.length; w > 1; w /= 2) {
            for (uint256 i = 0; i < w; i += 2) {
                (bytes32 a, bytes32 b) = (nodes[i], nodes[i + 1]);
                (bytes32 left, bytes32 right) = a < b ? (a, b) : (b, a);
                nodes[i / 2] = keccak256(abi.encode(left, right));
            }
        }

        return nodes[0];
    }

    function _genesisGroupId(address[] memory participants, bytes32 genesisSalt)
        internal
        pure
        returns (FROSTGroupId.T result)
    {
        bytes32 participantsRoot = _merkleRoot(participants);
        uint16 count = uint16(participants.length);
        uint16 threshold = (count / 2) + 1;
        bytes32 context = genesisSalt == bytes32(0) ? bytes32(0) : keccak256(abi.encodePacked("genesis", genesisSalt));
        return FROSTGroupId.create(participantsRoot, count, threshold, context);
    }
}
