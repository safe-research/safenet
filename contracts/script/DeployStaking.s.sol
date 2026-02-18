// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {SafeSingletonFactory} from "./util/SafeSingletonFactory.sol";
import {Staking} from "../src/Staking.sol";
import {DeterministicDeployment} from "@script/util/DeterministicDeployment.sol";

contract StakingScript is Script {
    using DeterministicDeployment for DeterministicDeployment.Factory;

    function setUp() public {}

    function run() public returns (Staking staking) {
        uint256 privateKey = vm.envUint("PRIVATE_KEY");

        vm.startBroadcast(privateKey);

        // Required script arguments:
        address initialOwner = vm.envAddress("STAKING_INITIAL_OWNER");
        address safeToken = vm.envAddress("SAFE_TOKEN");
        uint128 initialWithdrawalDelay = uint128(vm.envUint("STAKING_INITIAL_WITHDRAWAL_DELAY"));
        uint256 configTimeDelay = vm.envUint("STAKING_CONFIG_TIME_DELAY");
        uint256 factoryChoice = vm.envUint("FACTORY");

        if (factoryChoice == 1) {
            // Deploy the Staking contract using the SafeSingletonFactory
            staking = Staking(
                SafeSingletonFactory.deploy({
                    salt: bytes32(0),
                    code: abi.encodePacked(
                        type(Staking).creationCode,
                        abi.encode(initialOwner, safeToken, initialWithdrawalDelay, configTimeDelay)
                    )
                })
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

        // Write deployment info to deployments.json keyed by chain ID
        _writeDeployment(address(staking));
    }

    function _writeDeployment(address staking) internal {
        string memory path = "deployments.json";
        string memory chainId = vm.toString(block.chainid);

        // Build the deployment JSON object for this chain
        string memory obj = "deployment";
        string memory deploymentJson = vm.serializeAddress(obj, "staking", staking);

        // Create the file with an empty object if it doesn't exist yet
        if (!vm.exists(path)) {
            vm.writeJson("{}", path);
        }

        // Write/update the chain-specific entry
        vm.writeJson(deploymentJson, path, string.concat(".", chainId));

        console.log("Deployment saved to %s (chain %s)", path, chainId);
    }
}
