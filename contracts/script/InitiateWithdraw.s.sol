// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {Staking} from "@/Staking.sol";
import {getStakingContract} from "@script/util/GetStakingContract.sol";

contract InitiateWithdrawScript is Script {
    function run() public {
        Staking staking = getStakingContract(vm);

        // Required script arguments:
        address validator = vm.envAddress("WITHDRAW_VALIDATOR");
        uint256 amount = vm.envUint("WITHDRAW_AMOUNT");

        vm.startBroadcast();

        staking.initiateWithdrawal(validator, amount);
        console.log("Initiated withdrawal of %d SAFE tokens for validator %s", amount, validator);

        vm.stopBroadcast();
    }
}
