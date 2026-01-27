using ERC20Harness as erc20Token;

methods {
    // Shieldnet Staking functions
    function SAFE_TOKEN() external returns (address) envfree;
    function CONFIG_TIME_DELAY() external returns (uint256) envfree;
    function stakes(address staker, address validator) external returns (uint256) envfree;
    function isValidator(address validator) external returns (bool) envfree;
    function totalStakedAmount() external returns (uint256) envfree;
    function totalPendingWithdrawals() external returns (uint256) envfree;
    function totalStakerStakes(address staker) external returns (uint256) envfree;
    function pendingValidatorChangeHash() external returns (bytes32) envfree;
    function nextWithdrawalId() external returns (uint64) envfree;
    function withdrawDelay() external returns (uint128) envfree;

    // Ownable functions
    function owner() external returns (address) envfree;
    function renounceOwnership() external;

    // Harnessed functions
    function withdrawalQueueEmpty(address staker) external returns (bool) envfree;
    function withdrawalQueueLength(address staker) external returns (uint256) envfree;
    function checkWithdrawQueueIntegrity(address staker) external returns (bool) envfree;
    function isInWithdrawalQueue(address staker, uint64 withdrawalId) external returns (bool) envfree;
    function getTotalUserPendingWithdrawals(address staker) external returns (uint256) envfree;
    function addressesNotZero(address[] addrs) external returns (bool) envfree;
    function isPendingWithdrawalsTimestampIncreasing(address staker) external returns (bool) envfree;
    function getNextClaimableWithdrawalAmount(address staker) external returns (uint256) envfree;
    function getNextClaimableWithdrawalTimestamp(address staker) external returns (uint256) envfree;
    function getValidatorsHash(address[] validators, bool[] isRegistration, uint256 executableAt) external returns (bytes32) envfree;

    // ERC20 functions
    function erc20Token.allowance(address owner, address spender) external returns (uint256) envfree;
    function erc20Token.balanceOf(address account) external returns (uint256) envfree;
    function erc20Token.totalSupply() external returns (uint256) envfree;

    // Wildcard
    function _.balanceOf(address account) external => DISPATCHER(true);
    function _.transfer(address to, uint256 amount) external => DISPATCHER(true);
}

// Setup function that proves that the ERC20 token (SAFE) used in the Shieldnet
// Staking contract behaves like a well-formed ERC20 token.
function setupRequireERC20TokenInvariants(address a, address b) {
    require erc20Token.totalSupply() == 10^27; // 1 billion tokens with 18 decimals
    require erc20Token.balanceOf(a) <= erc20Token.totalSupply();
    require a != b
        => erc20Token.balanceOf(a) + erc20Token.balanceOf(b)
            <= erc20Token.totalSupply();
}

// Ghost variable that tracks the last timestamp.
ghost mathint ghostLastTimestamp;

// Hook function that tracks the last timestamp.
hook TIMESTAMP uint256 time {
    // We cannot go back in time to 1970.
    require time != 0;
    // The heat death of the universe would have already happened.
    require time < max_uint128 - CONFIG_TIME_DELAY();
    // We cannot go back in time.
    require time >= ghostLastTimestamp;
    ghostLastTimestamp = time;
}

// Hook to check withdrawal linked list integrity on sload of pointers
// using the invariant defined below.
hook Sload uint64 value withdrawalNodes[KEY address staker][KEY uint64 id].previous {
    requireInvariant withdrawalLinkedListIntegrity(staker, id);
    requireInvariant nextWithdrawalIdAndGreaterIdNodeShouldNotExist(staker, value);
    requireInvariant nextWithdrawalIdAndGreaterIdNodeShouldNotExist(staker, id);
}
hook Sload uint64 value withdrawalNodes[KEY address staker][KEY uint64 id].next {
    requireInvariant withdrawalLinkedListIntegrity(staker, id);
    requireInvariant nextWithdrawalIdAndGreaterIdNodeShouldNotExist(staker, value);
    requireInvariant nextWithdrawalIdAndGreaterIdNodeShouldNotExist(staker, id);
}

// Hook to check withdrawal queue head and tail IDs on sload using the invariant
// defined below.
hook Sload uint64 value withdrawalQueues[KEY address staker].head {
    requireInvariant withdrawalIdsAreEitherBothZeroOrNonZero(staker);
}
hook Sload uint64 value withdrawalQueues[KEY address staker].tail {
    requireInvariant withdrawalIdsAreEitherBothZeroOrNonZero(staker);
}

// Invariant that proves that the Shieldnet Staking contract's config time delay
// is always non-zero.
invariant configTimeDelayIsNonZero()
    CONFIG_TIME_DELAY() > 0;

// Invariant that proves that the Shieldnet Staking contract's next withdrawal
// ID is always non-zero.
invariant nextWithdrawalIdIsNonZero()
    nextWithdrawalId() > 0;

// Invariant that proves that the Shieldnet Staking contract's pending
// withdraw delay change value is always non-zero.
invariant pendingWithdrawalDelayChangeShouldEitherBothBeZeroOrNonZero()
    currentContract.pendingWithdrawDelayChange.value != 0 <=> currentContract.pendingWithdrawDelayChange.executableAt != 0;

// Invariant that proves that the Shieldnet Staking contract's withdraw delay
// is always non-zero.
invariant withdrawDelayIsNonZero()
    withdrawDelay() > 0
{
    preserved {
        requireInvariant pendingWithdrawalDelayChangeShouldEitherBothBeZeroOrNonZero();
    }
}

// Invariant that proves that the Shieldnet Staking contract's pending
// withdraw delay is never greater than the config delay.
invariant pendingWithdrawDelayIsLessThanConfigDelay()
    currentContract.pendingWithdrawDelayChange.value <= CONFIG_TIME_DELAY();

// Withdrawal time delay is always smaller than the configuration time delay.
invariant withdrawDelayIsLessThanConfigDelay()
    withdrawDelay() <= CONFIG_TIME_DELAY()
{
    preserved {
        requireInvariant pendingWithdrawDelayIsLessThanConfigDelay();
    }
}

// Invariant that proves that the Shieldnet Staking contract's next withdrawal
// ID's withdrawal node should always be non existent.
invariant nextWithdrawalIdAndGreaterIdNodeShouldNotExist(address staker, uint64 withdrawalId)
    withdrawalId >= nextWithdrawalId() =>
        currentContract.withdrawalNodes[staker][withdrawalId].amount == 0
        && currentContract.withdrawalNodes[staker][withdrawalId].claimableAt == 0
        && currentContract.withdrawalNodes[staker][withdrawalId].next == 0
        && currentContract.withdrawalNodes[staker][withdrawalId].previous == 0
{
    preserved initiateWithdrawal(address validator, uint256 amount) with (env e) {
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(
            e.msg.sender
        );
    }
    preserved initiateWithdrawalAtPosition(address validator, uint256 amount, uint64 previousId) with (env e) {
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(
            e.msg.sender
        );
    }
}

// Invariant that proves that the Shieldnet Staking contract's next withdrawal
// ID is always increasing.
invariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(address staker)
    (currentContract.withdrawalQueues[staker].head != 0 => currentContract.withdrawalQueues[staker].head < nextWithdrawalId()) &&
    (currentContract.withdrawalQueues[staker].tail != 0 => currentContract.withdrawalQueues[staker].tail < nextWithdrawalId());

// Invariant that proves that the Shieldnet Staking contract's withdrawal
// node's next and previous pointers are always less than the next withdrawal ID.
invariant nextWithdrawalIdShouldAlwaysBeGreaterThanPreviousAndNextPointers(address staker, uint64 withdrawalId)
    (currentContract.withdrawalNodes[staker][withdrawalId].previous != 0 => currentContract.withdrawalNodes[staker][withdrawalId].previous < nextWithdrawalId())
    && (currentContract.withdrawalNodes[staker][withdrawalId].next != 0 => currentContract.withdrawalNodes[staker][withdrawalId].next < nextWithdrawalId())
{
    preserved initiateWithdrawal(address validator, uint256 amount) with (env e) {
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(
            e.msg.sender
        );
    }
    preserved initiateWithdrawalAtPosition(address validator, uint256 amount, uint64 previousId) with (env e) {
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(
            e.msg.sender
        );
    }
}

// Invariant that proves that the Shieldnet Staking contract's withdrawal
// node's next and previous pointers can never point to itself.
invariant withdrawalNodeNextOrPreviousCannotBeItself(address staker, uint64 withdrawalId)
    withdrawalId != 0 =>
        currentContract.withdrawalNodes[staker][withdrawalId].next != withdrawalId
        && currentContract.withdrawalNodes[staker][withdrawalId].previous != withdrawalId
{
    preserved initiateWithdrawal(address validator, uint256 amount) with (env e) {
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(
            e.msg.sender
        );
    }
    preserved initiateWithdrawalAtPosition(address validator, uint256 amount, uint64 previousId) with (env e) {
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(
            e.msg.sender
        );
    }
}

// Invariant that proves that the Shieldnet Staking contract's withdrawal
// node with ID zero should never exist.
invariant withdrawalNodeZeroShouldNotExist(address staker)
    currentContract.withdrawalNodes[staker][0].amount == 0
    && currentContract.withdrawalNodes[staker][0].claimableAt == 0
    && currentContract.withdrawalNodes[staker][0].next == 0
    && currentContract.withdrawalNodes[staker][0].previous == 0
{
    preserved {
        requireInvariant nextWithdrawalIdIsNonZero();
    }
}

// Invariant that proves that all non-zero withdrawal nodes are reachable.
invariant withdrawalNodeIsReachable(address staker, uint64 withdrawalId)
    currentContract.withdrawalNodes[staker][withdrawalId].amount != 0
    && currentContract.withdrawalNodes[staker][withdrawalId].claimableAt != 0
        => isInWithdrawalQueue(staker, withdrawalId)
{
    preserved {
        requireInvariant withdrawalNodeZeroShouldNotExist(staker);
        requireInvariant nextWithdrawalIdIsNonZero();
    }
}

// TODO: https://prover.certora.com/output/950385/e0cfede3d65349d184c8c969dc06531c
// Invariant that proves that the Shieldnet Staking contract's withdrawal node
// with non-zero amount and claimableAt timestamps and zero next and previous
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
        requireInvariant withdrawalLinkedListIntegrityNew(staker);
    }
}

// TODO: https://prover.certora.com/output/950385/c0c1531fc371423aa8706bbc55dcda82
// Invariant that proves that the Shieldnet Staking contract's withdrawal
// queue head and tail IDs are either both zero or both non-zero for a given
// staker.
invariant withdrawalIdsAreEitherBothZeroOrNonZero(address staker)
    (currentContract.withdrawalQueues[staker].head == 0
        && currentContract.withdrawalQueues[staker].tail == 0)
    ||
    (currentContract.withdrawalQueues[staker].head != 0
        && currentContract.withdrawalQueues[staker].tail != 0)
{
    preserved {
        requireInvariant withdrawalLinkedListIntegrityNew(staker);
    }
}

// TODO: https://prover.certora.com/output/950385/4bfca09bec1747ffa8bdad918e82e053
// Invariant that proves that the Shieldnet Staking contract's withdrawal
// node's next and previous pointers are not the same except when they are zero.
invariant previousAndNextShouldNotBeSameExceptZero(address staker, uint64 withdrawalId)
    (currentContract.withdrawalNodes[staker][withdrawalId].next != 0 && currentContract.withdrawalNodes[staker][withdrawalId].previous != 0) =>
        currentContract.withdrawalNodes[staker][withdrawalId].next != currentContract.withdrawalNodes[staker][withdrawalId].previous
{
    preserved {
        requireInvariant withdrawalLinkedListIntegrityNew(staker);
    }
}

// Invariant that proves that the Shieldnet Staking contract's withdrawal
// linked list integrity is always maintained.
invariant withdrawalLinkedListIntegrityNew(address staker)
    checkWithdrawQueueIntegrity(staker);

// TODO: https://prover.certora.com/output/950385/78758a564f0c4d93acf482ec2f96c747
// Invariant that proves that the Shieldnet Staking contract's withdrawal
// linked list integrity is always maintained.
invariant withdrawalLinkedListIntegrity(address staker, uint64 withdrawalId)
    (currentContract.withdrawalNodes[staker][withdrawalId].next != 0 =>
        currentContract.withdrawalNodes[staker][currentContract.withdrawalNodes[staker][withdrawalId].next].previous == withdrawalId)
    && (currentContract.withdrawalNodes[staker][withdrawalId].previous != 0 =>
        currentContract.withdrawalNodes[staker][currentContract.withdrawalNodes[staker][withdrawalId].previous].next == withdrawalId);

// Invariant that proves that the Shieldnet Staking contract never has a
// staker with address zero.
invariant stakerAddressIsNeverZero(address staker)
    totalStakerStakes(staker) > 0 => staker != 0;

// Invariant that proves that the Shieldnet Staking contract's pending
// validator change hash cannot be computed if any of the validator addresses
// is zero.
invariant pendingValidatorsHashCannotHaveZeroValidatorAddress(address[] validators, bool[] isRegistration, uint256 executableAt)
    getValidatorsHash(validators, isRegistration, executableAt) == pendingValidatorChangeHash() => addressesNotZero(validators);

// TODO: https://prover.certora.com/output/950385/5fc61f233eff44839dc406a60be3e314 (Errored)
// Invariant that proves that the Shieldnet Staking contract never has a
// validator with address zero.
invariant validatorAddressIsNeverZero()
    !isValidator(0)
{
    preserved executeValidatorChanges(address[] validators, bool[] isRegistration, uint256 executableAt) with (env e) {
        requireInvariant pendingValidatorsHashCannotHaveZeroValidatorAddress(validators, isRegistration, executableAt);
    }
}

// TODO: https://prover.certora.com/output/950385/b544d88434f34c4da7ab1ed4057096f5
// Invariant that proves that the Shieldnet Staking contract never has a stake
// balance; i.e. there is no way for an external caller to get the locking
// contract to call `stake`, `initiateWithdrawal` or `initiateWithdrawalAtPosition`,
// on itself.
invariant contractCannotOperateOnItself(address validator)
    stakes(currentContract, validator) == 0
        && withdrawalQueueEmpty(currentContract)
{
    preserved with (env e) {
        require e.msg.sender != currentContract;
        requireInvariant stakerAddressIsNeverZero(e.msg.sender);
        requireInvariant validatorAddressIsNeverZero();
    }
}

// Invariant that proves that the Shieldnet Staking contract never grants
// allowance to another address; i.e. there is no way for an external caller to
// get the locking contract to call `approve` or `increaseAllowance` on the Safe
// token.
invariant noAllowanceForShieldnetStaking(address spender)
    erc20Token.allowance(currentContract, spender) == 0
    filtered {
        f -> f.contract != erc20Token
    }
{
    preserved constructor() {
        // Assume we don't already start with an allowance set to the staking
        // contract before the contract is deployed.
        require erc20Token.allowance(currentContract, spender) == 0;
    }
}

// Invariant that proves that the Shieldnet Staking contract's balance of the
// Safe token is always greater than or equal to the total amount of tokens
// staked plus the total amount of tokens pending withdrawal.
invariant contractBalanceGreaterThanTotalStakedAndPendingWithdrawals()
    erc20Token.balanceOf(currentContract) >= totalStakedAmount() + totalPendingWithdrawals()
{
    preserved with (env e) {
        setupRequireERC20TokenInvariants(currentContract, e.msg.sender);
        require e.msg.sender != currentContract;
    }
    preserved erc20Token.transferFrom(address from, address to, uint256 value) with (env e) {
        setupRequireERC20TokenInvariants(from, to);
        requireInvariant noAllowanceForShieldnetStaking(e.msg.sender);
    }
}

// Invariant that proves that the Shieldnet Staking contract's total staked
// amount is always greater than or equal to any individual staker's total
// stakes.
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

// TODO: https://prover.certora.com/output/950385/b8816500435a44de835c5d42d50e8478
// Invariant that proves that the Shieldnet Staking contract's withdrawal
// IDs are unique for each staker.
invariant withdrawalIdShouldBeUniqueForEachStaker(address stakerA, address stakerB)
    stakerA != stakerB =>
        (currentContract.withdrawalQueues[stakerA].head != currentContract.withdrawalQueues[stakerB].head
        || (currentContract.withdrawalQueues[stakerA].head == 0
        && currentContract.withdrawalQueues[stakerB].head == 0))
            && (currentContract.withdrawalQueues[stakerA].tail != currentContract.withdrawalQueues[stakerB].tail
            || (currentContract.withdrawalQueues[stakerA].tail == 0
            && currentContract.withdrawalQueues[stakerB].tail == 0));

// TODO: https://prover.certora.com/output/950385/c1aa0a4ff7c447cba00230394ca54b61
// Invariant that proves that the Shieldnet Staking contract's total pending
// withdrawal amount is always greater than or equal to any individual staker's
// total pending withdrawals.
invariant totalPendingWithdrawalIsGreaterThanUserPendingWithdrawals(address staker, address validator)
    totalPendingWithdrawals() >= getTotalUserPendingWithdrawals(staker);

// TODO: https://prover.certora.com/output/950385/01fa87fd6a654c648fa9a4335ef070a5
// Invariant that proves that the previous node pointer of the withdrawal node
// head of a staker's withdrawal queue is always zero.
invariant withdrawalHeadPreviousShouldAlwaysBeZero(address staker)
    currentContract.withdrawalNodes[staker][currentContract.withdrawalQueues[staker].head].previous == 0
{
    preserved {
        requireInvariant withdrawalLinkedListIntegrityNew(staker);
    }
}

// TODO: https://prover.certora.com/output/950385/d1835b4a266a4334833d96b817ab3bef
// Invariant that proves that the next node pointer of the withdrawal node tail
// of a staker's withdrawal queue is always zero.
invariant withdrawalTailNextShouldAlwaysBeZero(address staker)
    currentContract.withdrawalNodes[staker][currentContract.withdrawalQueues[staker].tail].next == 0
{
    preserved {
        requireInvariant withdrawalLinkedListIntegrityNew(staker);
    }
}

// TODO: https://prover.certora.com/output/950385/d8f896a2cca549c18cef32f6d48df52f
// Invariant that proves that the pending withdrawal timestamps in the
// withdrawal queue of a staker are always in ascending order.
invariant pendingWithdrawalTimestampShouldAlwaysBeInAscendingOrder(address staker, address validator)
    isPendingWithdrawalsTimestampIncreasing(staker);

// TODO: https://prover.certora.com/output/950385/3555f96d7d434afbb348803caf0f2fa2
// Invariant that proves that the next claimable withdrawal's amount and
// timestamp are either both zero or both non-zero for a given staker.
invariant pendingWithdrawalAmountShouldAlwaysBeGreaterThanZero(address staker)
    getNextClaimableWithdrawalAmount(staker) != 0 <=> getNextClaimableWithdrawalTimestamp(staker) != 0
{
    preserved with (env e) {
        requireInvariant withdrawDelayIsNonZero();
        requireInvariant withdrawalLinkedListIntegrityNew(staker);
    }
}

// Rule that verifies that any added withdrawal has non-zero amount and claim.
rule initializeWithdrawalIntegrity(method f) filtered {
    f -> f.selector == sig:initiateWithdrawal(address,uint256).selector
      || f.selector == sig:initiateWithdrawalAtPosition(address,uint256,uint64).selector
} {
    env e;
    calldataarg args;

    address staker = e.msg.sender;
    uint64 withdrawalId = nextWithdrawalId();
    uint256 withdrawals = withdrawalQueueLength(staker);

    // We limit the number of total withdrawals to `type(uint64).max - 1`.
    require withdrawalId < max_uint64;
    // The staker cannot call on behalf of the ERC-20
    //require staker != erc20Token;

    requireInvariant withdrawDelayIsLessThanConfigDelay();
    requireInvariant withdrawalNodeZeroShouldNotExist(staker);
    requireInvariant withdrawalLinkedListIntegrityNew(staker);
    requireInvariant nextWithdrawalIdIsNonZero();
    requireInvariant nextWithdrawalIdAndGreaterIdNodeShouldNotExist(staker, withdrawalId);

    assert currentContract.withdrawalNodes[staker][withdrawalId].amount == 0
        && currentContract.withdrawalNodes[staker][withdrawalId].claimableAt == 0;

    f(e, args);

    assert nextWithdrawalId() == withdrawalId + 1;
    assert withdrawalQueueLength(staker) == withdrawals + 1;
    assert isInWithdrawalQueue(staker, withdrawalId);
    assert currentContract.withdrawalNodes[staker][withdrawalId].amount != 0
        && currentContract.withdrawalNodes[staker][withdrawalId].claimableAt != 0;
}

// Rule that ensures that we can always reduce the length of the withdraw queue
// by waiting.
rule canAlwaysClaimWithdrawal(address staker) {
    setupRequireERC20TokenInvariants(currentContract, staker);
    requireInvariant contractBalanceGreaterThanTotalStakedAndPendingWithdrawals();
    require(staker != 0);

    uint256 claimableAt = getNextClaimableWithdrawalTimestamp(staker);
    require(claimableAt > 0);

    env e;
    require(e.block.timestamp >= claimableAt);
    require(e.msg.value == 0);

    claimWithdrawal@withrevert(e, staker);
    bool claimWithdrawalSuccess = !lastReverted;

    assert claimWithdrawalSuccess;
}
