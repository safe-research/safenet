// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script} from "@forge-std/Script.sol";
import {Vm} from "@forge-std/Vm.sol";
import {Staking} from "@/Staking.sol";
import {DeterministicDeployment} from "@script/util/DeterministicDeployment.sol";

using DeterministicDeployment for DeterministicDeployment.Factory;

function getStakingAddress(Vm vm) view returns (address) {
    uint256 factoryId = vm.envUint("FACTORY");
    DeterministicDeployment.Factory factory;
    if (factoryId == 1) {
        factory = DeterministicDeployment.SAFE_SINGLETON_FACTORY;
    } else if (factoryId == 2) {
        factory = DeterministicDeployment.CANONICAL;
    } else {
        revert("Invalid FACTORY choice");
    }

    bytes memory code = type(Staking).creationCode;
    bytes memory args = abi.encode(
        vm.envAddress("STAKING_INITIAL_OWNER"),
        vm.envAddress("SAFE_TOKEN"),
        uint128(vm.envUint("STAKING_INITIAL_WITHDRAWAL_DELAY")),
        vm.envUint("STAKING_CONFIG_TIME_DELAY")
    );
    return factory.deploymentAddressWithArgs(bytes32(0), code, args);
}
