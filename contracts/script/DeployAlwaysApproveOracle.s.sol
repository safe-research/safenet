// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {AlwaysApproveOracle} from "@/AlwaysApproveOracle.sol";
import {DeterministicDeployment} from "@script/util/DeterministicDeployment.sol";
import {getFactory} from "@script/util/GetFactory.sol";

contract DeployAlwaysApproveOracleScript is Script {
    using DeterministicDeployment for DeterministicDeployment.Factory;

    function run() public returns (AlwaysApproveOracle alwaysApproveOracle) {
        vm.startBroadcast();

        alwaysApproveOracle =
            AlwaysApproveOracle(getFactory(vm).deploy(bytes32(0), type(AlwaysApproveOracle).creationCode));

        vm.stopBroadcast();

        console.log("AlwaysApproveOracle:", address(alwaysApproveOracle));
    }
}
