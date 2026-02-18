// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {IERC20} from "@oz/token/ERC20/IERC20.sol";
import {Staking} from "../src/Staking.sol";

contract StakeSafeScript is Script {
    function run() public {
        uint256 privateKey = vm.envUint("PRIVATE_KEY");

        vm.startBroadcast(privateKey);

        // Required script arguments:
        address safeToken = vm.envAddress("SAFE_TOKEN");
        address validator = vm.envAddress("STAKE_VALIDATOR");
        uint256 amount = vm.envUint("STAKE_AMOUNT");

        // Read the staking contract address from deployments.json using the chain ID
        // forge-lint: disable-next-line(unsafe-cheatcode)
        string memory json = vm.readFile(string.concat("deployments.json"));
        address stakingContract = vm.parseJsonAddress(json, string.concat(".", vm.toString(block.chainid), ".staking"));

        // Check if user has enough SAFE tokens to stake
        uint256 userBalance = IERC20(safeToken).balanceOf(msg.sender);
        require(userBalance >= amount, "Not enough SAFE tokens to stake");

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
