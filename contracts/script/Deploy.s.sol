// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTCoordinator} from "@/FROSTCoordinator.sol";
import {Consensus} from "@/Consensus.sol";
import {DeterministicDeployment} from "@script/util/DeterministicDeployment.sol";
import {Genesis} from "@script/util/Genesis.sol";

contract DeployScript is Script {
    using DeterministicDeployment for DeterministicDeployment.Factory;

    function run() public returns (FROSTCoordinator coordinator, Consensus consensus) {
        // Required script arguments:
        address[] memory participants = vm.envAddress("PARTICIPANTS", ",");

        // Optional script arguments:
        bytes32 genesisSalt = vm.envOr("GENESIS_SALT", bytes32(0));
        bytes32 coordinatorSalt = vm.envOr("COORDINATOR_SALT", bytes32(0));
        bytes32 consensusSalt = vm.envOr("CONSENSUS_SALT", bytes32(0));

        vm.startBroadcast();

        coordinator = FROSTCoordinator(
            DeterministicDeployment.CANONICAL.deploy(coordinatorSalt, type(FROSTCoordinator).creationCode)
        );

        FROSTGroupId.T groupId = Genesis.groupId(participants, genesisSalt);
        consensus = Consensus(
            DeterministicDeployment.CANONICAL
                .deployWithArgs(consensusSalt, type(Consensus).creationCode, abi.encode(coordinator, groupId))
        );

        vm.stopBroadcast();

        console.log("FROSTCoordinator:", address(coordinator));
        console.log("Genesis Group ID: %s", vm.toString(FROSTGroupId.T.unwrap(groupId)));
        console.log("Consensus:", address(consensus));
    }
}
