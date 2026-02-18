// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {Staking} from "../src/Staking.sol";

contract ProposeAndAcceptValidatorsScript is Script {
    function setUp() public {}

    function run() public {
        uint256 privateKey = vm.envUint("PRIVATE_KEY");

        vm.startBroadcast(privateKey);

        // Required script arguments:
        uint256 executableAt = vm.envUint("EXECUTABLE_AT");
        address[] memory validators = vm.envAddress("ADD_VALIDATORS", ",");
        bool[] memory isRegistration = vm.envBool("IS_REGISTRATION", ",");

        // Read the staking contract address from deployments.json using the chain ID
        string memory json = vm.readFile(string.concat("deployments.json"));
        address stakingContract = vm.parseJsonAddress(json, string.concat(".", vm.toString(block.chainid), ".staking"));
        Staking staking = Staking(stakingContract);

        if (executableAt == 0) {
            staking.proposeValidators(validators, isRegistration);
        } else if (executableAt <= block.timestamp) {
            staking.executeValidatorChanges(validators, isRegistration, executableAt);
            console.log("Executed validator changes for validators: %s", vm.toString(abi.encode(validators)));
        } else {
            console.log(
                "Validator changes not executable yet. Executable at: %d, current time: %d",
                executableAt,
                block.timestamp
            );
        }

        vm.stopBroadcast();
    }
}
