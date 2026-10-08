// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {DeterministicDeployment} from "@script/util/DeterministicDeployment.sol";
import {getFactory} from "@script/util/GetFactory.sol";
import {Safenet7702Executor} from "@/Safenet7702Executor.sol";

contract DeploySafenet7702ExecutorScript is Script {
    using DeterministicDeployment for DeterministicDeployment.Factory;

    function run() public returns (address account) {
        DeterministicDeployment.Factory factory = getFactory(vm);

        vm.startBroadcast();

        account = factory.deploy(bytes32(0), type(Safenet7702Executor).creationCode);

        vm.stopBroadcast();

        console.log("Safenet7702Executor deployed at:", account);
    }
}
