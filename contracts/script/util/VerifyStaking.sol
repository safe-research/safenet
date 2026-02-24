// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Vm} from "@forge-std/Vm.sol";
import {console} from "@forge-std/Script.sol";

function verifyStakingCommand(
    Vm vm,
    address stakingAddress,
    uint256 chainId,
    address initialOwner,
    address safeToken,
    uint128 initialWithdrawalDelay,
    uint256 configTimeDelay
) pure {
    console.log(
        "Verify command:",
        string.concat(
            "forge verify-contract --watch ",
            vm.toString(stakingAddress),
            " src/Staking.sol:Staking --verifier etherscan --chain-id ",
            vm.toString(chainId),
            ' --constructor-args $(cast abi-encode "constructor(address,address,uint128,uint256)" "',
            vm.toString(initialOwner),
            '" "',
            vm.toString(safeToken),
            '" "',
            vm.toString(initialWithdrawalDelay),
            '" "',
            vm.toString(configTimeDelay),
            '") --etherscan-api-key ETHERSCAN_MULTICHAIN_KEY'
        )
    );
}
