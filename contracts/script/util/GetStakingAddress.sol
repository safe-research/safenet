// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Vm} from "@forge-std/Vm.sol";
import {Staking} from "@/Staking.sol";
import {DeterministicDeployment} from "@script/util/DeterministicDeployment.sol";

using DeterministicDeployment for DeterministicDeployment.Factory;

function getStakingAddress(Vm vm) view returns (Staking) {
    address stakingAddress = vm.envOr("STAKING_ADDRESS", address(0));
    if (stakingAddress != address(0)) {
        return Staking(stakingAddress);
    }

    uint256 factoryId = vm.envUint("FACTORY");
    DeterministicDeployment.Factory factory;
    if (factoryId == 1) {
        factory = DeterministicDeployment.SAFE_SINGLETON_FACTORY;
    } else if (factoryId == 2) {
        factory = DeterministicDeployment.CANONICAL;
    } else {
        revert("Invalid FACTORY choice");
    }

    (address initialOwner, address safeToken, uint128 initialWithdrawalDelay, uint256 configTimeDelay,) =
        getStackingDeploymentParameters(vm);
    bytes memory code = type(Staking).creationCode;
    bytes memory args = abi.encode(initialOwner, safeToken, initialWithdrawalDelay, configTimeDelay);
    return Staking(factory.deploymentAddressWithArgs(bytes32(0), code, args));
}

function getStackingDeploymentParameters(Vm vm)
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
    uint256 factoryId = uint256(vm.envOr("FACTORY", uint256(1)));
    if (factoryId == 1) {
        factory = DeterministicDeployment.SAFE_SINGLETON_FACTORY;
    } else if (factoryId == 2) {
        factory = DeterministicDeployment.CANONICAL;
    } else {
        revert("Invalid FACTORY choice");
    }
}
