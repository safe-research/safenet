import "StakingCommon.spec";

// Setup function for environment variables for rules.
function eSetup(env e) {
    require e.msg.sender != 0;
    require e.msg.sender != currentContract;
    require e.msg.sender != erc20Token;
    require e.msg.value == 0;
}

// Setup function for max amount related preconditions for staking and withdrawing rules.
function amountMaxSetup(env e, address validator, uint256 amount) {
    require stakes(e.msg.sender, validator) + amount < erc20Token.totalSupply();
    require totalValidatorStakes(validator) + amount < erc20Token.totalSupply();
    require totalStakerStakes(e.msg.sender) + amount < erc20Token.totalSupply();
    require totalStakedAmount() + amount < erc20Token.totalSupply();
}

// Setup function for min amount related preconditions for withdrawing rules.
function amountMinSetup(env e, address validator, uint256 amount) {
    require stakes(e.msg.sender, validator) >= amount;
    require totalValidatorStakes(validator) >= amount;
    require totalStakerStakes(e.msg.sender) >= amount;
    require totalStakedAmount() >= amount;
}

// Rule that verifies that getting pending withdrawal for any staker never reverts.
rule getPendingWithdrawalNeverReverts(address staker) {
    requireInvariant withdrawalLinkedListIntegrity(staker);
    getPendingWithdrawals@withrevert(staker);
    assert !lastReverted;
}

// Rule that verifies that getting the next claimable withdrawal info
// for any staker never reverts.
rule getNextClaimableWithdrawalNeverReverts(address staker) {
    requireInvariant withdrawalLinkedListIntegrity(staker);
    getNextClaimableWithdrawal@withrevert(staker);
    assert !lastReverted;
}

// Rule that verifies that staking can always succeed when the caller has enough balance
// and allowance, the amount is non-zero, the validator is valid and there are no overflows,
// else it fails otherwise.
rule canAlwaysStake(env e, address validator, uint256 amount)
{
    eSetup(e);
    setupRequireERC20TokenInvariants(currentContract, e.msg.sender);
    amountMaxSetup(e, validator, amount);

    uint256 previousStakerStakes = totalStakerStakes(e.msg.sender);
    uint256 previousTotalStaked = totalStakedAmount();

    bool enoughBalance = erc20Token.balanceOf(e.msg.sender) >= amount;
    bool enoughAllowance = erc20Token.allowance(e.msg.sender, currentContract) >= amount;

    stake@withrevert(e, validator, amount);
    bool stakeSuccess = !lastReverted;

    assert stakeSuccess => totalStakerStakes(e.msg.sender) == previousStakerStakes + amount
                            &&  totalStakedAmount() == previousTotalStaked + amount;
    assert !stakeSuccess => totalStakerStakes(e.msg.sender) == previousStakerStakes
                            &&  totalStakedAmount() == previousTotalStaked
                            && (amount == 0
                                || validator == 0
                                || !currentContract.isValidator(validator)
                                || !enoughBalance
                                || !enoughAllowance);
}

// Rule that verifies that staking is commutative.
rule stakeIsCommutative(env e, address validator, uint256 amount1, uint256 amount2) {
    require amount1 + amount2 < max_uint256;
    uint256 combinedAmount = assert_uint256(amount1 + amount2);

    eSetup(e);
    setupRequireERC20TokenInvariants(currentContract, e.msg.sender);
    amountMaxSetup(e, validator, combinedAmount);

    storage init = lastStorage;

    stake(e, validator, amount1);
    stake(e, validator, amount2);

    storage separateStake = lastStorage;

    stake(e, validator, combinedAmount) at init;

    assert separateStake == lastStorage;
}

// Rule that verifies that any added withdrawal has non-zero amount and claim.
rule initializeWithdrawalIntegrity(env e, calldataarg args, method f) filtered {
    f -> f.selector == sig:initiateWithdrawal(address,uint256).selector
      || f.selector == sig:initiateWithdrawalAtPosition(address,uint256,uint64).selector
} {
    address staker = e.msg.sender;
    uint64 withdrawalId = nextWithdrawalId();
    uint256 withdrawals = withdrawalQueueLength(staker);

    requireInvariant withdrawDelayIsLessThanConfigDelay();
    requireInvariant withdrawalNodeZeroShouldNotExist(staker);
    requireInvariant withdrawalLinkedListIntegrity(staker);
    requireInvariant nextWithdrawalIdIsNonZero();
    requireInvariant nextWithdrawalIdAndGreaterIdNodeShouldNotExist(staker, withdrawalId);
    requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(staker);

    // We limit the number of total withdrawals to `type(uint64).max - 1`.
    require withdrawalId < max_uint64;

    assert currentContract.withdrawalNodes[staker][withdrawalId].amount == 0
        && currentContract.withdrawalNodes[staker][withdrawalId].claimableAt == 0;

    f(e, args);

    if(f.selector == sig:initiateWithdrawalAtPosition(address,uint256,uint64).selector) {
        requireInvariant withdrawalNodeIsReachable(staker, currentContract.withdrawalNodes[staker][withdrawalId].previous);
    }

    assert nextWithdrawalId() == withdrawalId + 1;
    assert withdrawalQueueLength(staker) == withdrawals + 1;
    assert isInWithdrawalQueue(staker, withdrawalId);
    assert currentContract.withdrawalNodes[staker][withdrawalId].amount != 0
        && currentContract.withdrawalNodes[staker][withdrawalId].claimableAt != 0;
}

// Rule that verifies that initiating a withdrawal can only succeed when the caller has enough staked balance,
// the amount is non-zero, the validator is valid and there are no overflows, else it fails otherwise.
rule canAlwaysWithdraw(env e, address validator, uint256 amount) {
    eSetup(e);
    setupRequireERC20TokenInvariants(currentContract, e.msg.sender);
    amountMaxSetup(e, validator, amount);
    amountMinSetup(e, validator, amount);

    require nextWithdrawalId() < max_uint64; // To prevent overflow of withdrawal IDs
    require totalPendingWithdrawals() + amount < erc20Token.totalSupply();

    uint256 previousStakes = stakes(e.msg.sender, validator);
    uint256 previousStakerStakes = totalStakerStakes(e.msg.sender);
    uint256 previousTotalStaked = totalStakedAmount();
    uint256 previousValidatorStakes = totalValidatorStakes(validator);
    uint256 previousTotalPendingWithdrawals = totalPendingWithdrawals();

    bool enoughStaked = stakes(e.msg.sender, validator) >= amount;

    initiateWithdrawal@withrevert(e, validator, amount);
    bool initiateWithdrawalSuccess = !lastReverted;

    assert initiateWithdrawalSuccess => stakes(e.msg.sender, validator) == previousStakes - amount
                                    &&  totalStakerStakes(e.msg.sender) == previousStakerStakes - amount
                                    &&  totalStakedAmount() == previousTotalStaked - amount
                                    && totalValidatorStakes(validator) == previousValidatorStakes - amount
                                    && totalPendingWithdrawals() == previousTotalPendingWithdrawals + amount;
    assert !initiateWithdrawalSuccess => stakes(e.msg.sender, validator) == previousStakes
                                    &&  totalStakerStakes(e.msg.sender) == previousStakerStakes
                                    &&  totalStakedAmount() == previousTotalStaked
                                    && totalValidatorStakes(validator) == previousValidatorStakes
                                    && totalPendingWithdrawals() == previousTotalPendingWithdrawals
                                    && (amount == 0 || validator == 0 || !currentContract.isValidator(validator) || !enoughStaked);
}

// Rule that verifies that withdrawing at position can always succeed when the caller has enough staked balance,
// the amount is non-zero, the validator is valid, the previous ID is valid and there are no overflows, else it fails otherwise.
rule canAlwaysWithdrawAtPosition(env e, address validator, uint256 amount, uint64 previousId) {
    eSetup(e);
    setupRequireERC20TokenInvariants(currentContract, e.msg.sender);
    amountMaxSetup(e, validator, amount);
    amountMinSetup(e, validator, amount);

    uint128 previousIdClaimableAt = currentContract.withdrawalNodes[e.msg.sender][previousId].claimableAt;
    uint64 nextId = currentContract.withdrawalNodes[e.msg.sender][previousId].next;
    uint128 nextIdClaimableAt = currentContract.withdrawalNodes[e.msg.sender][nextId].claimableAt;

    require previousId > 0 && previousId < nextWithdrawalId(); // Previous ID must be valid and non-zero
    require previousIdClaimableAt != 0; // Previous node must exist in the withdrawal queue
    require previousIdClaimableAt <= e.block.timestamp + withdrawDelay();
    require nextId == 0 || nextIdClaimableAt >= e.block.timestamp + withdrawDelay();
    require e.block.timestamp + withdrawDelay() < max_uint128; // To prevent overflow of claimableAt timestamp
    require nextWithdrawalId() < max_uint64; // To prevent overflow of withdrawal IDs
    require totalPendingWithdrawals() + amount < erc20Token.totalSupply();

    uint256 previousStakes = stakes(e.msg.sender, validator);
    uint256 previousStakerStakes = totalStakerStakes(e.msg.sender);
    uint256 previousTotalStaked = totalStakedAmount();
    uint256 previousValidatorStakes = totalValidatorStakes(validator);
    uint256 previousTotalPendingWithdrawals = totalPendingWithdrawals();

    bool enoughStaked = stakes(e.msg.sender, validator) >= amount;

    initiateWithdrawalAtPosition@withrevert(e, validator, amount, previousId);
    bool initiateWithdrawalSuccess = !lastReverted;

    assert initiateWithdrawalSuccess => stakes(e.msg.sender, validator) == previousStakes - amount
                                    &&  totalStakerStakes(e.msg.sender) == previousStakerStakes - amount
                                    &&  totalStakedAmount() == previousTotalStaked - amount
                                    && totalValidatorStakes(validator) == previousValidatorStakes - amount
                                    && totalPendingWithdrawals() == previousTotalPendingWithdrawals + amount;
    assert !initiateWithdrawalSuccess => stakes(e.msg.sender, validator) == previousStakes
                                    &&  totalStakerStakes(e.msg.sender) == previousStakerStakes
                                    &&  totalStakedAmount() == previousTotalStaked
                                    && totalValidatorStakes(validator) == previousValidatorStakes
                                    && totalPendingWithdrawals() == previousTotalPendingWithdrawals
                                    && (amount == 0 || validator == 0 || !currentContract.isValidator(validator) || !enoughStaked);
}

// Rule that ensures that we can always reduce the length of the withdraw queue
// by waiting.
rule canAlwaysClaimWithdrawal(env e) {
    eSetup(e);
    setupRequireERC20TokenInvariants(currentContract, e.msg.sender);
    requireInvariant contractBalanceGreaterThanTotalStakedAndPendingWithdrawals();
    // requireInvariant totalPendingWithdrawalIsGreaterThanUserPendingWithdrawals(e.msg.sender);

    uint256 claimableAt = getNextClaimableWithdrawalTimestamp(e.msg.sender);
    require(claimableAt > 0);
    require(e.block.timestamp >= claimableAt);
    require(getNextClaimableWithdrawalAmount(e.msg.sender) <= totalPendingWithdrawals());

    claimWithdrawal@withrevert(e);
    bool claimWithdrawalSuccess = !lastReverted;

    assert claimWithdrawalSuccess;
}

// Rule that verifies that only the owner can propose a withdrawal delay change.
rule onlyOwnerCanProposeWithdrawalDelay(env e, uint128 newDelay) {
    eSetup(e);
    proposeWithdrawDelay@withrevert(e, newDelay);

    assert !lastReverted => e.msg.sender == currentContract.owner();
}

// Rule that verifies that only the owner can propose a validator change.
rule onlyOwnerCanProposeValidator(env e, address[] validators, bool[] isRegistration) {
    eSetup(e);
    proposeValidators@withrevert(e, validators, isRegistration);

    assert !lastReverted => e.msg.sender == currentContract.owner();
}

// Rule that verifies that a staker cannot withdraw more than they have staked.
rule cannotWithdrawMoreThanStaked(env e, address validator, uint256 amount) {
    eSetup(e);
    require stakes(e.msg.sender, validator) < amount;

    initiateWithdrawal@withrevert(e, validator, amount);

    assert lastReverted;
}
