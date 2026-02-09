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

    function withdrawalQueueEmpty(address staker) public view returns (bool isEmpty) {
        (uint256 amount, uint256 claimableAt) = this.getNextClaimableWithdrawal(staker);
        isEmpty = (amount == 0 && claimableAt == 0);
    }

    function withdrawalQueueLength(address staker) public view returns (uint256 length) {
        return this.getPendingWithdrawals(staker).length;
    }

    function isInWithdrawalQueue(address staker, uint64 withdrawalId) public view returns (bool result) {
        WithdrawalQueue storage queue = withdrawalQueues[staker];
        mapping(uint64 => WithdrawalNode) storage nodes = withdrawalNodes[staker];

        for (uint64 currentId = queue.head; currentId != 0; currentId = nodes[currentId].next) {
            if (currentId == withdrawalId) {
                return true;
            }
        }
        return false;
    }

    function checkWithdrawQueueIntegrity(address staker) public view returns (bool result) {
        // Check the integrity of the withdrawal queue linked list pointers.

        WithdrawalQueue memory queue = withdrawalQueues[staker];
        mapping(uint64 => WithdrawalNode) storage nodes = withdrawalNodes[staker];

        if (queue.head == 0 && queue.tail == 0) {
            // Empty queue.
            return true;
        } else if (queue.head == queue.tail) {
            // Queue with a single element.
            WithdrawalNode memory node = nodes[queue.head];
            return node.next == 0 && node.previous == 0;
        } else {
            // Queue with two or more elements. Ensure the list is the same forwards and backwards.
            uint256 count = 0;
            uint64 currentId;
            for (currentId = queue.head; currentId != 0; currentId = nodes[currentId].next) {
                count++;
            }
            if (count < 2) {
                return false;
            }

            uint64[] memory withdrawalIds = new uint64[](count);
            currentId = queue.head;
            for (uint256 i = 0; i < count; i++) {
                withdrawalIds[i] = currentId;
                currentId = nodes[currentId].next;
            }
            currentId = queue.tail;
            for (uint256 i = count - 1; i >= 0; i--) {
                if (withdrawalIds[i] != currentId) {
                    return false;
                }
                currentId = nodes[currentId].previous;
            }

            return nodes[queue.head].previous == 0;
        }
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
            if (pendingWithdrawals[i - 1].claimableAt > pendingWithdrawals[i].claimableAt) {
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
