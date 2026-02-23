// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script} from "@forge-std/Script.sol";
import {Staking} from "@/Staking.sol";
import {getStakingContract} from "@script/util/GetStakingContract.sol";

contract ProposeValidatorsScript is Script {
    function run() public {
        // Required script arguments:
        address[] memory validators = vm.envAddress("ADD_VALIDATORS", ",");
        bool[] memory isRegistration = vm.envBool("IS_REGISTRATION", ",");

        Staking staking = getStakingContract(vm);

        vm.startBroadcast();

        staking.proposeValidators(validators, isRegistration);

        vm.stopBroadcast();
    }
}
