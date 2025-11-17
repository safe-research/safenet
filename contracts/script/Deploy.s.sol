// smart-contracts/script/Deploy.s.sol
pragma solidity ^0.8.19;

import {Script, console} from "@forge-std/Script.sol";
import {FROSTCoordinator} from "@/FROSTCoordinator.sol";

contract DeployScript is Script {
    function run() public returns (FROSTCoordinator) {
        vm.startBroadcast();

        FROSTCoordinator coordinator = new FROSTCoordinator{salt: bytes32(0)}();

        vm.stopBroadcast();

        console.log("FROSTCoordinator deployed to:", address(coordinator));
        return coordinator;
    }
}
