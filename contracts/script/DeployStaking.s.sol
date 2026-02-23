// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {Staking} from "@/Staking.sol";
import {DeterministicDeployment} from "@script/util/DeterministicDeployment.sol";
import {getStakingDeploymentParameters} from "@script/util/GetStakingContract.sol";

contract DeployStakingScript is Script {
    using DeterministicDeployment for DeterministicDeployment.Factory;

    function run() public returns (address staking) {
        // Required script arguments:
        (
            address initialOwner,
            address safeToken,
            uint128 initialWithdrawalDelay,
            uint256 configTimeDelay,
            DeterministicDeployment.Factory factory
        ) = getStakingDeploymentParameters(vm);

        vm.startBroadcast();

        staking = factory.deployWithArgs(
            bytes32(0),
            type(Staking).creationCode,
            abi.encode(initialOwner, safeToken, initialWithdrawalDelay, configTimeDelay)
        );

        vm.stopBroadcast();

        console.log("Staking deployed at:", address(staking));
    }
}
