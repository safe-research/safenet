// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {IERC20} from "@oz/token/ERC20/IERC20.sol";
import {Staking} from "@/Staking.sol";
import {getStakingAddress} from "@script/util/GetStakingAddress.sol";

contract StakeSafeScript is Script {
    function run() public {
        // Required script arguments:
        address safeToken = vm.envAddress("SAFE_TOKEN");
        address validator = vm.envAddress("STAKE_VALIDATOR");
        uint256 amount = vm.envUint("STAKE_AMOUNT");

        require(safeToken != address(0), "Invalid SAFE token address");
        require(validator != address(0), "Invalid validator address");
        require(amount > 0, "Invalid stake amount");

        // Calculate the staking contract address using the GetStakingAddress utility
        address stakingContract = address(getStakingAddress(vm));

        // Check if user has enough SAFE tokens to stake
        uint256 userBalance = IERC20(safeToken).balanceOf(msg.sender);
        require(userBalance >= amount, "Not enough SAFE tokens to stake");

        vm.startBroadcast();

        // Check if enough allowance to the staking contract
        if (IERC20(safeToken).allowance(msg.sender, stakingContract) < amount) {
            // If not, approve the staking contract to spend the required amount of SAFE tokens
            IERC20(safeToken).approve(stakingContract, amount);
        }

        // Stake SAFE Tokens
        Staking(stakingContract).stake(validator, amount);

        vm.stopBroadcast();

        console.log(
            "Staked %d SAFE tokens for validator %s in Staking contract at %s", amount, validator, stakingContract
        );
    }
}
