// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {Staking} from "@/Staking.sol";
import {getStakingAddress} from "@script/util/GetStakingAddress.sol";

contract ClaimWithdrawScript is Script {
    function run() public {
        // Calculate the staking contract address using the GetStakingAddress utility
        Staking staking = getStakingAddress(vm);

        // Check if it is a withdrawal initiation or claim
        (uint256 amountToClaim, uint256 claimableAt) = staking.getNextClaimableWithdrawal(msg.sender);
        require(amountToClaim > 0, "No withdrawal available to claim");
        require(block.timestamp >= claimableAt, "Withdrawal not executable yet");

        vm.startBroadcast();

        staking.claimWithdrawal();
        console.log("Claimed withdrawal of %d SAFE tokens", amountToClaim);

        vm.stopBroadcast();
    }
}
