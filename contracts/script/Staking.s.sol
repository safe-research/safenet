// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {Staking} from "@/Staking.sol";
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

        require(initialOwner != address(0), "Invalid initial owner address");
        require(safeToken != address(0), "Invalid SAFE token address");
        require(initialWithdrawalDelay != 0, "Invalid initial withdrawal delay");
        require(configTimeDelay != 0, "Invalid configuration time delay");
        require(
            initialWithdrawalDelay <= configTimeDelay,
            "Initial withdrawal delay must be less than or equal to config time delay"
        );

        DeterministicDeployment.Factory factory;

        if (factoryChoice == 1) {
            factory = DeterministicDeployment.SAFE_SINGLETON_FACTORY;
        } else if (factoryChoice == 2) {
            factory = DeterministicDeployment.CANONICAL;
        } else {
            revert("Invalid FACTORY choice");
        }

        factory.deployWithArgs(
            bytes32(0),
            type(Staking).creationCode,
            abi.encode(initialOwner, safeToken, initialWithdrawalDelay, configTimeDelay)
        );

        vm.stopBroadcast();

        console.log("Staking deployed at:", address(staking));
    }
}
