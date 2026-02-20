// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {Staking} from "@/Staking.sol";
import {getStakingAddress} from "@script/util/GetStakingAddress.sol";

contract InitiateWithdrawScript is Script {
    function run() public {
        vm.startBroadcast();

        // Calculate the staking contract address using the GetStakingAddress utility
        Staking staking = Staking(getStakingAddress(vm));

        // Required script arguments:
        address validator = vm.envAddress("WITHDRAW_VALIDATOR");
        uint256 amount = vm.envUint("WITHDRAW_AMOUNT");

        require(validator != address(0), "Invalid validator address");
        require(amount > 0, "Invalid withdrawal amount");

        // Check if enough amount available to withdraw with the validator
        uint256 stakedAmount = staking.stakes(msg.sender, validator);
        require(stakedAmount >= amount, "Not enough staked amount to withdraw");

        staking.initiateWithdrawal(validator, amount);
        console.log("Initiated withdrawal of %d SAFE tokens for validator %s", amount, validator);

        vm.stopBroadcast();
    }
}
