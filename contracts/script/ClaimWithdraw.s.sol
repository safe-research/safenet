// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {Staking} from "@/Staking.sol";
import {getStakingContract} from "@script/util/GetStakingContract.sol";

contract ClaimWithdrawScript is Script {
    function run() public {
        Staking staking = getStakingContract(vm);

        // Check if it is a withdrawal initiation or claim
        (uint256 amountToClaim,) = staking.getNextClaimableWithdrawal(msg.sender);

        vm.startBroadcast();

        staking.claimWithdrawal();
        console.log("Claimed withdrawal of %d SAFE tokens", amountToClaim);

        vm.stopBroadcast();
    }
}
