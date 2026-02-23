// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Vm} from "@forge-std/Vm.sol";
import {DeterministicDeployment} from "@script/util/DeterministicDeployment.sol";

using DeterministicDeployment for DeterministicDeployment.Factory;

function getFactory(Vm vm) view returns (DeterministicDeployment.Factory) {
    uint256 factoryId = uint256(vm.envOr("FACTORY", uint256(1)));
    if (factoryId == 1) {
        return DeterministicDeployment.SAFE_SINGLETON_FACTORY;
    } else if (factoryId == 2) {
        return DeterministicDeployment.CANONICAL;
    } else {
        revert("Invalid FACTORY choice");
    }
}
