// smart-contracts/script/Deploy.s.sol
pragma solidity ^0.8.19;

import {Script, console} from "@forge-std/Script.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTCoordinator} from "@/FROSTCoordinator.sol";
import {Consensus} from "@/Consensus.sol";

contract DeployScript is Script {
    bytes32 constant DEFAULT_GROUP_ID = 0xfa3efbc5dd215abad66149ad58d7869f90cf6d2c1bc472be0000000000000000;

    function run() public returns (FROSTCoordinator, Consensus) {
        vm.startBroadcast();

        FROSTCoordinator coordinator = new FROSTCoordinator{salt: bytes32(0)}();
        console.log("FROSTCoordinator deployed to:", address(coordinator));

        bytes32 rawGroupId = vm.envOr("GROUP_ID", DEFAULT_GROUP_ID);

        Consensus consensus = new Consensus{salt: bytes32(0)}(address(coordinator), FROSTGroupId.T.wrap(rawGroupId));
        console.log("Consensus deployed to:", address(consensus));

        vm.stopBroadcast();

        return (coordinator, consensus);
    }
}
