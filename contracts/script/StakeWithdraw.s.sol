// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {IERC20} from "@oz/token/ERC20/IERC20.sol";
import {Staking} from "../src/Staking.sol";

contract StakeWithdrawScript is Script {
    function setUp() public {}

    function run() public {
        uint256 privateKey = vm.envUint("PRIVATE_KEY");

        vm.startBroadcast(privateKey);

        // Required script arguments:
        address validator = vm.envAddress("WITHDRAW_VALIDATOR");
        uint256 amount = vm.envUint("WITHDRAW_AMOUNT");

        // Read the staking contract address from deployments.json using the chain ID
        string memory json = vm.readFile(string.concat("deployments.json"));
        Staking staking = Staking(vm.parseJsonAddress(json, string.concat(".", vm.toString(block.chainid), ".staking")));

        // Check if it is a withdrawal initiation or claim
        if (amount > 0) {
            // Check if enough amount available to withdraw with the validator
            uint256 stakedAmount = staking.stakes(msg.sender, validator);
            require(stakedAmount >= amount, "Not enough staked amount to withdraw");

            staking.initiateWithdrawal(validator, amount);
            console.log("Initiated withdrawal of %d SAFE tokens for validator %s", amount, validator);
        } else {
            (uint256 amountToClaim, uint256 claimableAt) = staking.getNextClaimableWithdrawal(msg.sender);
            require(amountToClaim > 0, "No withdrawal available to claim");
            require(block.timestamp >= claimableAt, "Withdrawal not executable yet");

            staking.claimWithdrawal();
            console.log("Claimed withdrawal of %d SAFE tokens for validator %s", amountToClaim, validator);
        }

        vm.stopBroadcast();
    }
}
