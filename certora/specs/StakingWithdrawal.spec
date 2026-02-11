import "StakingCommon.spec";

// Invariant that proves that the Staking contract's withdrawal node's next and
// previous pointers are always less than the next withdrawal ID.
invariant nextWithdrawalIdShouldAlwaysBeGreaterThanPreviousAndNextPointers(address staker, uint64 withdrawalId)
    (currentContract.withdrawalNodes[staker][withdrawalId].previous != 0 => currentContract.withdrawalNodes[staker][withdrawalId].previous < nextWithdrawalId())
    && (currentContract.withdrawalNodes[staker][withdrawalId].next != 0 => currentContract.withdrawalNodes[staker][withdrawalId].next < nextWithdrawalId())
{
    preserved initiateWithdrawal(address validator, uint256 amount) with (env e) {
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(
            e.msg.sender
        );
        requireInvariant withdrawalLinkedListIntegrity(staker);
    }
    preserved initiateWithdrawalAtPosition(address validator, uint256 amount, uint64 previousId) with (env e) {
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(
            e.msg.sender
        );
        requireInvariant withdrawalLinkedListIntegrity(staker);
    }
}

// Invariant that proves that the Staking contract's withdrawal node's next and
// previous pointers can never point to itself.
invariant withdrawalNodeNextOrPreviousCannotBeItself(address staker, uint64 withdrawalId)
    withdrawalId != 0 =>
        currentContract.withdrawalNodes[staker][withdrawalId].next != withdrawalId
        && currentContract.withdrawalNodes[staker][withdrawalId].previous != withdrawalId
{
    preserved initiateWithdrawal(address validator, uint256 amount) with (env e) {
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(
            e.msg.sender
        );
        requireInvariant withdrawalLinkedListIntegrity(staker);
    }
    preserved initiateWithdrawalAtPosition(address validator, uint256 amount, uint64 previousId) with (env e) {
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(
            e.msg.sender
        );
        requireInvariant withdrawalLinkedListIntegrity(staker);
    }
}

// Invariant that proves that the Staking contract's withdrawal node with
// non-zero amount and claimableAt timestamps and zero next and previous
// pointers is the only node in the withdrawal queue for a given staker.
invariant withdrawalNodeNextAndPreviousZeroIntegrity(address staker, uint64 withdrawalId)
    currentContract.withdrawalNodes[staker][withdrawalId].amount != 0
    && currentContract.withdrawalNodes[staker][withdrawalId].claimableAt != 0
    && currentContract.withdrawalNodes[staker][withdrawalId].next == 0
    && currentContract.withdrawalNodes[staker][withdrawalId].previous == 0
        => currentContract.withdrawalQueues[staker].head == withdrawalId
        && currentContract.withdrawalQueues[staker].tail == withdrawalId
{
    preserved {
        requireInvariant withdrawalLinkedListIntegrity(staker);
        requireInvariant nextWithdrawalIdIsNonZero();
        requireInvariant withdrawalNodeZeroShouldNotExist(staker);
        requireInvariant withdrawalNodeIntegrity(staker, withdrawalId);
        requireInvariant nextWithdrawalIdAndGreaterIdNodeShouldNotExist(staker, withdrawalId);
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(staker);

    }
    preserved initiateWithdrawalAtPosition(address validator, uint256 amount, uint64 previousId) with (env e) {
        requireInvariant withdrawalNodeIsReachable(staker, previousId);
        requireInvariant withdrawalLinkedListIntegrity(staker);
        requireInvariant nextWithdrawalIdIsNonZero();
        requireInvariant withdrawalNodeZeroShouldNotExist(staker);
        requireInvariant withdrawalNodeIntegrity(staker, withdrawalId);
        requireInvariant withdrawalNodeIntegrity(staker, previousId);
        requireInvariant nextWithdrawalIdAndGreaterIdNodeShouldNotExist(staker, withdrawalId);
        requireInvariant nextWithdrawalIdAndGreaterIdNodeShouldNotExist(staker, previousId);
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(staker);
    }
}

// Invariant that proves that the Staking contract's withdrawal node's next and
// previous pointers are not the same except when they are zero.
invariant previousAndNextShouldNotBeSameExceptZero(address staker, uint64 withdrawalId)
    (currentContract.withdrawalNodes[staker][withdrawalId].next != 0 && currentContract.withdrawalNodes[staker][withdrawalId].previous != 0) =>
        currentContract.withdrawalNodes[staker][withdrawalId].next != currentContract.withdrawalNodes[staker][withdrawalId].previous
{
    preserved {
        requireInvariant withdrawalLinkedListIntegrity(staker);
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(staker);
    }
    preserved initiateWithdrawalAtPosition(address validator, uint256 amount, uint64 previousId) with (env e) {
        requireInvariant withdrawalNodeIsReachable(staker, previousId);
        requireInvariant withdrawalLinkedListIntegrity(staker);
        requireInvariant nextWithdrawalIdIsNonZero();
        requireInvariant withdrawalNodeZeroShouldNotExist(staker);
        requireInvariant withdrawalNodeIntegrity(staker, previousId);
        requireInvariant nextWithdrawalIdAndGreaterIdNodeShouldNotExist(staker, previousId);
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(staker);
    }
}

invariant headAndTailPointerShouldBeValidWithdrawalNodes(address staker)
    (currentContract.withdrawalQueues[staker].head != 0 =>
        currentContract.withdrawalNodes[staker][currentContract.withdrawalQueues[staker].head].amount != 0
        && currentContract.withdrawalNodes[staker][currentContract.withdrawalQueues[staker].head].claimableAt != 0)
    &&
    (currentContract.withdrawalQueues[staker].tail != 0 =>
        currentContract.withdrawalNodes[staker][currentContract.withdrawalQueues[staker].tail].amount != 0
        && currentContract.withdrawalNodes[staker][currentContract.withdrawalQueues[staker].tail].claimableAt != 0)
{
    preserved {
        requireInvariant withdrawalLinkedListIntegrity(staker);
        requireInvariant withdrawDelayIsLessThanConfigDelay();
    }
}

// Invariant that proves that the Staking contract's withdrawal linked list
// integrity is always maintained.
invariant withdrawalLinkedListPointerIntegrity(address staker, uint64 withdrawalId)
    (currentContract.withdrawalNodes[staker][withdrawalId].next != 0 =>
        !withdrawalQueueEmpty(staker) &&
        isInWithdrawalQueue(staker, withdrawalId) &&
        currentContract.withdrawalNodes[staker][currentContract.withdrawalNodes[staker][withdrawalId].next].previous == withdrawalId)
    && (currentContract.withdrawalNodes[staker][withdrawalId].previous != 0 =>
        !withdrawalQueueEmpty(staker) &&
        isInWithdrawalQueue(staker, withdrawalId) &&
        currentContract.withdrawalNodes[staker][currentContract.withdrawalNodes[staker][withdrawalId].previous].next == withdrawalId)
{
    preserved {
        requireInvariant withdrawalLinkedListIntegrity(staker);
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(staker);
        requireInvariant headAndTailPointerShouldBeValidWithdrawalNodes(staker);
    }
    preserved initiateWithdrawalAtPosition(address validator, uint256 amount, uint64 previousId) with (env e) {
        requireInvariant withdrawalLinkedListIntegrity(staker);
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(staker);
        requireInvariant headAndTailPointerShouldBeValidWithdrawalNodes(staker);
        requireInvariant withdrawalNodeIsReachable(staker, previousId);
        requireInvariant withdrawalQueueHeadAndTailAreEitherBothZeroOrNonZero(staker);
    }
}

// Invariant that proves that the Staking contract's withdrawal IDs are unique
// for each staker; i.e. two different stakers cannot both have a valid
// withdrawal node (non-zero amount and claimableAt) for the same ID.
invariant withdrawalIdShouldBeUniqueForEachStaker(address stakerA, address stakerB, uint64 withdrawalId)
    stakerA != stakerB =>
        !(currentContract.withdrawalNodes[stakerA][withdrawalId].amount != 0 &&
          currentContract.withdrawalNodes[stakerA][withdrawalId].claimableAt != 0 &&
          currentContract.withdrawalNodes[stakerB][withdrawalId].amount != 0 &&
          currentContract.withdrawalNodes[stakerB][withdrawalId].claimableAt != 0)
{
    preserved {
        requireInvariant withdrawalNodeIntegrity(stakerA, withdrawalId);
        requireInvariant withdrawalNodeIntegrity(stakerB, withdrawalId);
        requireInvariant nextWithdrawalIdAndGreaterIdNodeShouldNotExist(stakerA, withdrawalId);
        requireInvariant nextWithdrawalIdAndGreaterIdNodeShouldNotExist(stakerB, withdrawalId);
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(stakerA);
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(stakerB);
    }
}

// Invariant that proves that the previous node pointer of the withdrawal node
// head of a staker's withdrawal queue is always zero.
invariant withdrawalHeadPreviousAndTailNextShouldAlwaysBeZero(address staker)
    currentContract.withdrawalNodes[staker][currentContract.withdrawalQueues[staker].head].previous == 0 &&
    currentContract.withdrawalNodes[staker][currentContract.withdrawalQueues[staker].tail].next == 0
{
    preserved {
        requireInvariant withdrawalLinkedListIntegrity(staker);
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(staker);
        requireInvariant withdrawalNodeZeroShouldNotExist(staker);
    }
    preserved initiateWithdrawalAtPosition(address validator, uint256 amount, uint64 previousId) with (env e) {
        requireInvariant withdrawalLinkedListIntegrity(staker);
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(staker);
        requireInvariant withdrawalNodeZeroShouldNotExist(staker);
        requireInvariant withdrawalNodeIsReachable(staker, previousId);
    }
}
