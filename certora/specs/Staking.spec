import "StakingCommon.spec";

// Invariant that proves that the Staking contract's config time delay
// is always non-zero.
invariant configTimeDelayIsNonZero()
    CONFIG_TIME_DELAY() > 0;

// Invariant that proves that the Staking contract's pending withdraw delay
// change value is always non-zero.
invariant pendingWithdrawalDelayChangeShouldEitherBothBeZeroOrNonZero()
    currentContract.pendingWithdrawDelayChange.value != 0 <=> currentContract.pendingWithdrawDelayChange.executableAt != 0;

// Invariant that proves that the Staking contract's withdraw delay is always
// non-zero.
invariant withdrawDelayIsNonZero()
    withdrawDelay() > 0
{
    preserved {
        requireInvariant pendingWithdrawalDelayChangeShouldEitherBothBeZeroOrNonZero();
    }
}

// Invariant that proves that the Staking contract's pending validator change
// hash cannot be computed if any of the validator addresses is zero.
invariant pendingValidatorsHashCannotHaveZeroValidatorAddress(address[] validators, bool[] isRegistration, uint256 executableAt)
    getValidatorsHash(validators, isRegistration, executableAt) == pendingValidatorChangeHash() => addressesNotZero(validators);

// Invariant that proves that the Staking contract never has a validator with
// address zero.
invariant validatorAddressIsNeverZero()
    !isValidator(0)
{
    preserved executeValidatorChanges(address[] validators, bool[] isRegistration, uint256 executableAt) with (env e) {
        requireInvariant pendingValidatorsHashCannotHaveZeroValidatorAddress(validators, isRegistration, executableAt);
    }
}

// Invariant that proves that the Staking contract never has a stake balance;
// i.e. there is no way for an external caller to get the locking contract to
// call `stake`, `initiateWithdrawal` or `initiateWithdrawalAtPosition` on
// itself.
invariant contractCannotOperateOnItself()
    totalStakerStakes(currentContract) == 0
        && withdrawalQueueEmpty(currentContract)
            && checkWithdrawQueueIntegrity(currentContract)
{
    preserved with (env e) {
        require e.msg.sender != currentContract; // Contract cannot call on itself
        requireInvariant stakerAddressIsNeverZero();
    }
}

// Invariant that proves that the Staking contract's total staked amount is
// always greater than or equal to any individual staker's total stakes.
invariant totalStakedIsGreaterThanUserStaked(address staker)
    totalStakedAmount() >= totalStakerStakes(staker)
{
    preserved initiateWithdrawalAtPosition(address validator, uint256 amount, uint64 previousId) with (env e) {
        require staker != e.msg.sender
            => totalStakedAmount() >= totalStakerStakes(staker) + totalStakerStakes(e.msg.sender);
    }
    preserved initiateWithdrawal(address validator, uint256 amount) with (env e) {
        require staker != e.msg.sender
            => totalStakedAmount() >= totalStakerStakes(staker) + totalStakerStakes(e.msg.sender);
    }
}

// Invariant that proves that the pending withdrawal timestamps in the
// withdrawal queue of a staker are always in ascending order.
invariant pendingWithdrawalTimestampShouldAlwaysBeInAscendingOrder(address staker, address validator)
    isPendingWithdrawalsTimestampIncreasing(staker)
{
    preserved {
        requireInvariant withdrawalLinkedListIntegrity(staker);
    }
    preserved initiateWithdrawalAtPosition(address v, uint256 amount, uint64 previousId) with (env e) {
        requireInvariant withdrawalLinkedListIntegrity(staker);
        requireInvariant withdrawalNodeIsReachable(staker, previousId);
    }
}
