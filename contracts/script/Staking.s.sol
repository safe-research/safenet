// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {Staking} from "../src/Staking.sol";
import {DeterministicDeployment} from "@script/util/DeterministicDeployment.sol";

contract StakingScript is Script {
    using DeterministicDeployment for DeterministicDeployment.Factory;

    function run() public returns (Staking staking) {
        vm.startBroadcast();

        // Required script arguments:
        address initialOwner = vm.envAddress("STAKING_INITIAL_OWNER");
        address safeToken = vm.envAddress("SAFE_TOKEN");
        uint128 initialWithdrawalDelay = uint128(vm.envUint("STAKING_INITIAL_WITHDRAWAL_DELAY"));
        uint256 configTimeDelay = vm.envUint("STAKING_CONFIG_TIME_DELAY");
        uint256 factoryChoice = vm.envUint("FACTORY");

        if (factoryChoice == 1) {
            // Deploy the Staking contract using the SafeSingletonFactory
            staking = Staking(
                DeterministicDeployment.SAFE_SINGLETON_FACTORY
                    .deployWithArgs(
                        bytes32(0),
                        type(Staking).creationCode,
                        abi.encode(initialOwner, safeToken, initialWithdrawalDelay, configTimeDelay)
                    )
            );
        } else if (factoryChoice == 2) {
            // Deploy the Staking contract using the DeterministicDeployment factory
            staking = Staking(
                DeterministicDeployment.CANONICAL
                    .deployWithArgs(
                        bytes32(0),
                        type(Staking).creationCode,
                        abi.encode(initialOwner, safeToken, initialWithdrawalDelay, configTimeDelay)
                    )
            );
        } else {
            revert("Invalid FACTORY choice");
        }

        vm.stopBroadcast();

        console.log("Staking deployed at:", address(staking));
    }
}
