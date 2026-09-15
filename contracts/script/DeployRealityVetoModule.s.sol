// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {RealityVetoModule} from "@/veto/RealityVetoModule.sol";
import {DeterministicDeployment} from "@script/util/DeterministicDeployment.sol";
import {getFactory} from "@script/util/GetFactory.sol";

/**
 * @title Deploy Reality Veto Module
 * @notice Deploys a {RealityVetoModule} bound to one Safe, one Reality module and one vetoer.
 * @dev All three constructor arguments are immutable or Safe-gated afterwards, so these environment
 *      values are the whole configuration surface. Enabling the module on the Safe is a separate
 *      governance transaction; see `script/README.md`.
 */
contract DeployRealityVetoModuleScript is Script {
    using DeterministicDeployment for DeterministicDeployment.Factory;

    function run() public returns (address vetoModule) {
        // Required script arguments:
        address safe = vm.envAddress("VETO_SAFE_ADDRESS");
        address realityModule = vm.envAddress("VETO_REALITY_MODULE_ADDRESS");
        address vetoer = vm.envAddress("VETOER_ADDRESS");

        DeterministicDeployment.Factory factory = getFactory(vm);

        vm.startBroadcast();

        vetoModule = factory.deployWithArgs(
            bytes32(0), type(RealityVetoModule).creationCode, abi.encode(safe, realityModule, vetoer)
        );

        vm.stopBroadcast();

        console.log("RealityVetoModule deployed at:", vetoModule);
    }
}
