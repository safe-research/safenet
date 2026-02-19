// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script} from "@forge-std/Script.sol";
import {Staking} from "../src/Staking.sol";
import {GetStakingAddress} from "./util/GetStakingAddress.sol";

contract ProposeValidatorsScript is Script {
    function run() public {
        vm.startBroadcast();

        // Required script arguments:
        address[] memory validators = vm.envAddress("ADD_VALIDATORS", ",");
        bool[] memory isRegistration = vm.envBool("IS_REGISTRATION", ",");

        // Calculate the staking contract address using the GetStakingAddress utility and the FACTORY environment variable
        Staking staking = Staking(new GetStakingAddress().getStakingAddress(vm.envUint("FACTORY")));

        staking.proposeValidators(validators, isRegistration);

        vm.stopBroadcast();
    }
}
