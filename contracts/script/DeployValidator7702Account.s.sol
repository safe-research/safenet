// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {DeterministicDeployment} from "@script/util/DeterministicDeployment.sol";
import {getFactory} from "@script/util/GetFactory.sol";
import {Validator7702Account} from "@/Validator7702Account.sol";

contract DeployValidator7702AccountScript is Script {
    using DeterministicDeployment for DeterministicDeployment.Factory;

    function run() public returns (address account) {
        DeterministicDeployment.Factory factory = getFactory(vm);

        vm.startBroadcast();

        account = factory.deploy(bytes32(0), type(Validator7702Account).creationCode);

        vm.stopBroadcast();

        console.log("Validator7702Account deployed at:", account);
    }
}
