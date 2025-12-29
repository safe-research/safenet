// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Staking} from "../../contracts/src/Staking.sol";

contract StakingHarness is Staking {
    constructor(
        address initialOwner,
        address safeToken,
        uint128 initialWithdrawalDelay,
        uint256 configTimeDelay
    )
        Staking(
            initialOwner,
            safeToken,
            initialWithdrawalDelay,
            configTimeDelay
        )
    {}
}
