// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script} from "@forge-std/Script.sol";
import {Staking} from "../../src/Staking.sol";
import {DeterministicDeployment} from "./DeterministicDeployment.sol";

contract GetStakingAddress is Script {
    using DeterministicDeployment for DeterministicDeployment.Factory;

    function getStakingAddress(uint256 factory) public view returns (address) {
        bytes memory code = type(Staking).creationCode;
        bytes memory args = abi.encode(
            vm.envAddress("STAKING_INITIAL_OWNER"),
            vm.envAddress("SAFE_TOKEN"),
            uint128(vm.envUint("STAKING_INITIAL_WITHDRAWAL_DELAY")),
            vm.envUint("STAKING_CONFIG_TIME_DELAY")
        );
        if (factory == 1) {
            return DeterministicDeployment.SAFE_SINGLETON_FACTORY.deploymentAddressWithArgs(bytes32(0), code, args);
        } else if (factory == 2) {
            return DeterministicDeployment.CANONICAL.deploymentAddressWithArgs(bytes32(0), code, args);
        } else {
            revert("Invalid FACTORY choice");
        }
    }
}
