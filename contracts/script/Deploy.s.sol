// smart-contracts/script/Deploy.s.sol
pragma solidity ^0.8.19;

import {Script, console} from "@forge-std/Script.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTCoordinator} from "@/FROSTCoordinator.sol";
import {Consensus} from "@/Consensus.sol";

contract DeployScript is Script {
    bytes32 constant DEFAULT_GROUP_ID = 0xfa3efbc5dd215abad66149ad58d7869f90cf6d2c1bc472be0000000000000000;

    // The canonical Create2 factory address used by Foundry by default
    address constant DETERMINISTIC_FACTORY = 0x4e59b44847b379578588920cA78FbF26c0B4956C;

    function run() public returns (FROSTCoordinator, Consensus) {
        vm.startBroadcast();

        // ------------------------------------
        // 1. Deploy or Attach FROSTCoordinator
        // ------------------------------------
        FROSTCoordinator coordinator;

        // Calculate the init code hash (Creation code + encoded args if any)
        bytes memory coordinatorInitCode = type(FROSTCoordinator).creationCode;
        bytes32 coordinatorSalt = vm.envOr("COORDINATOR_SALT", bytes32(0));

        // Compute the deterministic address
        address expectedCoordinatorAddr =
            vm.computeCreate2Address(coordinatorSalt, keccak256(coordinatorInitCode), DETERMINISTIC_FACTORY);

        if (expectedCoordinatorAddr.code.length > 0) {
            coordinator = FROSTCoordinator(expectedCoordinatorAddr);
            console.log("FROSTCoordinator already exists at:", expectedCoordinatorAddr);
        } else {
            coordinator = new FROSTCoordinator{salt: coordinatorSalt}();
            console.log("FROSTCoordinator deployed to:", address(coordinator));
        }

        // ------------------------------------
        // 2. Deploy or Attach Consensus
        // ------------------------------------
        Consensus consensus;

        bytes32 rawGroupId = vm.envOr("GROUP_ID", DEFAULT_GROUP_ID);
        console.log("Group id: %s", vm.toString(rawGroupId));
        FROSTGroupId.T groupIdWrapped = FROSTGroupId.T.wrap(rawGroupId);

        // Calculate init code: Creation Code + ABI Encoded Constructor Arguments
        bytes memory consensusArgs = abi.encode(address(coordinator), groupIdWrapped);
        bytes memory consensusInitCode = abi.encodePacked(type(Consensus).creationCode, consensusArgs);
        bytes32 consensusSalt = vm.envOr("CONSENSUS_SALT", bytes32(0));

        address expectedConsensusAddr =
            vm.computeCreate2Address(consensusSalt, keccak256(consensusInitCode), DETERMINISTIC_FACTORY);

        if (expectedConsensusAddr.code.length > 0) {
            consensus = Consensus(expectedConsensusAddr);
            console.log("Consensus already exists at:", expectedConsensusAddr);
        } else {
            consensus = new Consensus{salt: consensusSalt}(address(coordinator), groupIdWrapped);
            console.log("Consensus deployed to:", address(consensus));
        }

        vm.stopBroadcast();

        return (coordinator, consensus);
    }
}
