// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Vm} from "@forge-std/Vm.sol";
import {Staking} from "@/Staking.sol";
import {DeterministicDeployment} from "@script/util/DeterministicDeployment.sol";
import {getFactory} from "@script/util/GetFactory.sol";

using DeterministicDeployment for DeterministicDeployment.Factory;

function getStakingContract(Vm vm) view returns (Staking) {
    address stakingAddress = vm.envOr("STAKING_ADDRESS", address(0));
    if (stakingAddress != address(0)) {
        return Staking(stakingAddress);
    }

    (
        address initialOwner,
        address safeToken,
        uint128 initialWithdrawalDelay,
        uint256 configTimeDelay,
        DeterministicDeployment.Factory factory
    ) = getStakingDeploymentParameters(vm);
    bytes memory code = type(Staking).creationCode;
    bytes memory args = abi.encode(initialOwner, safeToken, initialWithdrawalDelay, configTimeDelay);
    return Staking(factory.deploymentAddressWithArgs(bytes32(0), code, args));
}

function getStakingDeploymentParameters(Vm vm)
    view
    returns (
        address initialOwner,
        address safeToken,
        uint128 initialWithdrawalDelay,
        uint256 configTimeDelay,
        DeterministicDeployment.Factory factory
    )
{
    initialOwner = vm.envOr("STAKING_INITIAL_OWNER", 0x8CF60B289f8d31F737049B590b5E4285Ff0Bd1D1);
    safeToken = vm.envOr("SAFE_TOKEN", 0x5aFE3855358E112B5647B952709E6165e1c1eEEe);
    initialWithdrawalDelay = uint128(vm.envOr("STAKING_INITIAL_WITHDRAWAL_DELAY", uint256(172800)));
    configTimeDelay = uint256(vm.envOr("STAKING_CONFIG_TIME_DELAY", uint256(604800)));
    factory = getFactory(vm);
}
