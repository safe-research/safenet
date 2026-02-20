// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {Staking} from "@/Staking.sol";
import {getStakingAddress} from "@script/util/GetStakingAddress.sol";

contract AcceptValidatorsScript is Script {
    function run() public {
        uint256 executableAt = vm.envOr("EXECUTABLE_AT", uint256(0));
        address[] memory validators;
        bool[] memory isRegistration;
        Staking staking;

        if (executableAt == 0) {
            // Read the executableAt value from run-latest.json using the chain ID
            // forge-lint: disable-next-line(unsafe-cheatcode)
            string memory runJson = vm.readFile(
                string.concat(
                    "build/broadcast/ProposeValidators.s.sol/", vm.toString(block.chainid), "/run-latest.json"
                )
            );
            bytes32 actualEventSignature = keccak256("ValidatorsProposed(bytes32,address[],bool[],uint256)");
            bytes32 emittedEventSignature = vm.parseJsonBytes32(runJson, ".receipts[0].logs[0].topics[0]");
            require(actualEventSignature == emittedEventSignature, "Event signature mismatch");

            bytes memory proposeEventData = vm.parseJsonBytes(runJson, ".receipts[0].logs[0].data");
            (validators, isRegistration, executableAt) = abi.decode(proposeEventData, (address[], bool[], uint256));

            staking = Staking(vm.parseJsonAddress(runJson, ".receipts[0].logs[0].address"));
        } else {
            // Required script arguments:
            validators = vm.envAddress("ADD_VALIDATORS", ",");
            isRegistration = vm.envBool("IS_REGISTRATION", ",");

            // Calculate the staking contract address using the GetStakingAddress utility
            staking = getStakingAddress(vm);
        }

        require(validators.length == isRegistration.length, "Mismatched input lengths");
        require(validators.length > 0, "No validators provided");

        vm.startBroadcast();

        if (executableAt <= block.timestamp) {
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
