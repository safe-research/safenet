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

    function withdrawalQueueEmpty(address staker) public returns (bool isEmpty) {
        (uint256 amount, uint256 claimableAt) = this.getNextClaimableWithdrawal(staker);
        isEmpty = (amount == 0 && claimableAt == 0);
    }

    function getTotalUserPendingWithdrawals(address staker) public view returns (uint256 totalUserPendingWithdrawals) {
        WithdrawalInfo[] memory pendingWithdrawals = this.getPendingWithdrawals(staker);
        for (uint256 i = 0; i < pendingWithdrawals.length; i++) {
            totalUserPendingWithdrawals += pendingWithdrawals[i].amount;
        }
    }

    function getValidatorsHash(address[] calldata validators, bool[] calldata isRegistration, uint256 executableAt) public
        view
        returns (bytes32)
    {
        return _getValidatorsHash(validators, isRegistration, executableAt);
    }

    function addressesNotZero(address[] calldata addrs) public pure returns (bool) {
        for (uint256 i = 0; i < addrs.length; i++) {
            if (addrs[i] == address(0)) {
                return false;
            }
        }
        return true;
    }

    function isPendingWithdrawalsTimestampIncreasing(address staker) public view returns (bool) {
        WithdrawalInfo[] memory pendingWithdrawals = this.getPendingWithdrawals(staker);
        for (uint256 i = 1; i < pendingWithdrawals.length; i++) {
            if (pendingWithdrawals[i - 1].claimableAt >= pendingWithdrawals[i].claimableAt) {
                return false;
            }
        }
        return true;
    }

    function getNextClaimableWithdrawalAmount(address staker) public view returns (uint256) {
        (uint256 amount, ) = this.getNextClaimableWithdrawal(staker);
        return amount;
    }

    function getNextClaimableWithdrawalTimestamp(address staker) public view returns (uint256) {
        (, uint256 claimableAt) = this.getNextClaimableWithdrawal(staker);
        return claimableAt;
    }
}
