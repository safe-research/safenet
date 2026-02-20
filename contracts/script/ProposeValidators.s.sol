// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script} from "@forge-std/Script.sol";
import {Staking} from "@/Staking.sol";
import {getStakingAddress} from "@script/util/GetStakingAddress.sol";

contract ProposeValidatorsScript is Script {
    function run() public {
        vm.startBroadcast();

        // Required script arguments:
        address[] memory validators = vm.envAddress("ADD_VALIDATORS", ",");
        bool[] memory isRegistration = vm.envBool("IS_REGISTRATION", ",");

        require(validators.length == isRegistration.length, "Mismatched input lengths");
        require(validators.length > 0, "No validators provided");

        // Calculate the staking contract address using the GetStakingAddress utility
        Staking staking = Staking(getStakingAddress(vm));

        staking.proposeValidators(validators, isRegistration);

        vm.stopBroadcast();
    }
}
