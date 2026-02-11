using ERC20Harness as erc20Token;

methods {
    // Staking functions
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
    function getPendingWithdrawals(address staker) external returns (Staking.WithdrawalInfo[]) envfree;
    function getNextClaimableWithdrawal(address staker) external returns (uint256, uint256) envfree;
    function totalValidatorStakes(address validator) external returns (uint256) envfree;

    // Ownable functions
    function owner() external returns (address) envfree;

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

// Setup function that proves that the ERC20 token (SAFE) used in the Staking
// contract behaves like a well-formed ERC20 token.
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
    requireInvariant nextWithdrawalIdAndGreaterIdNodeShouldNotExist(staker, value);
    requireInvariant nextWithdrawalIdAndGreaterIdNodeShouldNotExist(staker, id);
}
hook Sload uint64 value withdrawalNodes[KEY address staker][KEY uint64 id].next {
    requireInvariant nextWithdrawalIdAndGreaterIdNodeShouldNotExist(staker, value);
    requireInvariant nextWithdrawalIdAndGreaterIdNodeShouldNotExist(staker, id);
}

// Hook to check withdrawal queue head and tail IDs on sload using the invariant
// defined below.
hook Sload uint64 value withdrawalQueues[KEY address staker].head {
    requireInvariant withdrawalQueueHeadAndTailAreEitherBothZeroOrNonZero(staker);
}
hook Sload uint64 value withdrawalQueues[KEY address staker].tail {
    requireInvariant withdrawalQueueHeadAndTailAreEitherBothZeroOrNonZero(staker);
}

// Invariant that proves that the Staking contract's next withdrawal ID is
// always increasing.
invariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(address staker)
    (currentContract.withdrawalQueues[staker].head != 0 => currentContract.withdrawalQueues[staker].head < nextWithdrawalId()) &&
    (currentContract.withdrawalQueues[staker].tail != 0 => currentContract.withdrawalQueues[staker].tail < nextWithdrawalId())
{
    preserved {
        requireInvariant withdrawalLinkedListIntegrity(staker);
    }
}

// Invariant that proves that the Staking contract's withdrawal linked list
// integrity is always maintained.
invariant withdrawalLinkedListIntegrity(address staker)
    checkWithdrawQueueIntegrity(staker)
{
    preserved {
        requireInvariant nextWithdrawalIdIsNonZero();
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(staker);
        requireInvariant withdrawalQueueHeadAndTailAreEitherBothZeroOrNonZero(staker);
    }
    preserved initiateWithdrawalAtPosition(address validator, uint256 amount, uint64 previousId) with (env e) {
        requireInvariant withdrawalNodeIsReachable(staker, previousId);
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(staker);
    }
}

// Invariant that proves that all non-zero withdrawal nodes are reachable.
invariant withdrawalNodeIsReachable(address staker, uint64 withdrawalId)
    currentContract.withdrawalNodes[staker][withdrawalId].amount != 0
    || currentContract.withdrawalNodes[staker][withdrawalId].claimableAt != 0
        => isInWithdrawalQueue(staker, withdrawalId)
{
    preserved {
        requireInvariant nextWithdrawalIdIsNonZero();
        requireInvariant withdrawalNodeZeroShouldNotExist(staker);
        requireInvariant withdrawalLinkedListIntegrity(staker);
        requireInvariant withdrawalNodeIntegrity(staker, withdrawalId);
        requireInvariant nextWithdrawalIdAndGreaterIdNodeShouldNotExist(staker, withdrawalId);
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(staker);
    }
    preserved initiateWithdrawalAtPosition(address validator, uint256 amount, uint64 previousId) with (env e) {
        requireInvariant withdrawalNodeIsReachable(staker, previousId);
        requireInvariant nextWithdrawalIdIsNonZero();
        requireInvariant withdrawalNodeZeroShouldNotExist(staker);
        requireInvariant withdrawalLinkedListIntegrity(staker);
        requireInvariant withdrawalNodeIntegrity(staker, withdrawalId);
        requireInvariant withdrawalNodeIntegrity(staker, previousId);
        requireInvariant nextWithdrawalIdAndGreaterIdNodeShouldNotExist(staker, withdrawalId);
        requireInvariant nextWithdrawalIdAndGreaterIdNodeShouldNotExist(staker, previousId);
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(staker);
    }
}

// Invariant that proves that the Staking contract's withdrawal queue head and
// tail IDs are either both zero or both non-zero for a given staker.
invariant withdrawalQueueHeadAndTailAreEitherBothZeroOrNonZero(address staker)
    (currentContract.withdrawalQueues[staker].head == 0
        && currentContract.withdrawalQueues[staker].tail == 0)
    ||
    (currentContract.withdrawalQueues[staker].head != 0
        && currentContract.withdrawalQueues[staker].tail != 0)
{
    preserved {
        requireInvariant withdrawalLinkedListIntegrity(staker);
        requireInvariant nextWithdrawalIdIsNonZero();
        requireInvariant withdrawalNodeZeroShouldNotExist(staker);
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

// Invariant that proves that the Staking contract's next withdrawal ID's
// withdrawal node should always be non existent.
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
        requireInvariant withdrawalLinkedListIntegrity(staker);
    }
    preserved initiateWithdrawalAtPosition(address validator, uint256 amount, uint64 previousId) with (env e) {
        requireInvariant nextWithdrawalIdShouldAlwaysBeGreaterThanHeadAndTailPointers(
            e.msg.sender
        );
        requireInvariant withdrawalLinkedListIntegrity(staker);
    }
}

// Invariant that proves that the Staking contract never has a staker with
// address zero.
invariant stakerAddressIsNeverZero()
    totalStakerStakes(0) == 0;

// Invariant that proves that the Staking contract's next withdrawal ID is
// always non-zero.
invariant nextWithdrawalIdIsNonZero()
    nextWithdrawalId() > 0;

// Withdrawal time delay is always smaller than the configuration time delay.
invariant withdrawDelayIsLessThanConfigDelay()
    withdrawDelay() <= CONFIG_TIME_DELAY()
{
    preserved {
        requireInvariant pendingWithdrawDelayIsLessThanConfigDelay();
    }
}

// Invariant that proves that the Staking contract's withdrawal node with ID
// zero should never exist.
invariant withdrawalNodeZeroShouldNotExist(address staker)
    currentContract.withdrawalNodes[staker][0].amount == 0
    && currentContract.withdrawalNodes[staker][0].claimableAt == 0
    && currentContract.withdrawalNodes[staker][0].next == 0
    && currentContract.withdrawalNodes[staker][0].previous == 0
{
    preserved {
        requireInvariant nextWithdrawalIdIsNonZero();
        requireInvariant withdrawalLinkedListIntegrity(staker);
    }
}

// Invariant that proves that the withdrawal node should have either both amount and claimableAt as zero or both non-zero.
invariant withdrawalNodeIntegrity(address staker, uint64 withdrawalId)
    currentContract.withdrawalNodes[staker][withdrawalId].amount != 0 <=> currentContract.withdrawalNodes[staker][withdrawalId].claimableAt != 0
{
    preserved {
        requireInvariant stakerAddressIsNeverZero();
        requireInvariant withdrawDelayIsLessThanConfigDelay();
        requireInvariant nextWithdrawalIdAndGreaterIdNodeShouldNotExist(staker, withdrawalId);

    }
}

// Invariant that proves that the Staking contract's pending withdrawal delay is
// never greater than the config delay.
invariant pendingWithdrawDelayIsLessThanConfigDelay()
    currentContract.pendingWithdrawDelayChange.value <= CONFIG_TIME_DELAY();

// Invariant that proves that the Staking contract never grants allowance to
// another address; i.e. there is no way for an external caller to get the
// locking contract to call `approve` or `increaseAllowance` on the Safe token.
invariant noAllowanceForSelf(address spender)
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

// Invariant that proves that the Staking contract's balance of the Safe token
// is always greater than or equal to the total amount of tokens staked plus the
// total amount of tokens pending withdrawal.
invariant contractBalanceGreaterThanTotalStakedAndPendingWithdrawals()
    erc20Token.balanceOf(currentContract) >= totalStakedAmount() + totalPendingWithdrawals()
{
    preserved with (env e) {
        setupRequireERC20TokenInvariants(currentContract, e.msg.sender);
        require e.msg.sender != currentContract;
    }
    preserved erc20Token.transferFrom(address from, address to, uint256 value) with (env e) {
        setupRequireERC20TokenInvariants(from, to);
        requireInvariant noAllowanceForSelf(e.msg.sender);
    }
}
