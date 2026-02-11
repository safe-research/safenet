// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script} from "@forge-std/Script.sol";
import {FROSTCoordinator} from "@/FROSTCoordinator.sol";
import {DeterministicDeployment} from "@script/util/DeterministicDeployment.sol";
import {Genesis} from "@script/util/Genesis.sol";

contract GenesisScript is Script {
    using DeterministicDeployment for DeterministicDeployment.Factory;

    function run() public {
        // Required script arguments:
        address[] memory participants = vm.envAddress("PARTICIPANTS", ",");

        // Optional script arguments:
        bytes32 genesisSalt = vm.envOr("GENESIS_SALT", bytes32(0));
        address coordinatorAddress = vm.envOr(
            "COORDINATOR_ADDRESS",
            DeterministicDeployment.CANONICAL.deploymentAddress(bytes32(0), type(FROSTCoordinator).creationCode)
        );

        vm.startBroadcast();

        (bytes32 participantsRoot, uint16 count, uint16 threshold, bytes32 context) =
            Genesis.groupParameters(participants, genesisSalt);
        FROSTCoordinator(coordinatorAddress).keyGen(participantsRoot, count, threshold, context);

        vm.stopBroadcast();
    }
}
