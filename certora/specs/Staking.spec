using ERC20Harness as erc20Token;

methods {
    // Shieldnet Staking functions
    function totalStakedAmount() external returns (uint256) envfree;
    function totalPendingWithdrawals() external returns (uint256) envfree;
    function totalStakerStakes(address staker) external returns (uint256) envfree;

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

// Invariant that proves that the Shieldnet Staking contract never grants
// allowance to another address; i.e. there is no way for an external caller to
// get the locking contract to call `approve` or `increaseAllowance` on the Safe
// token.
invariant noAllowanceForShieldnetStaking(address spender)
    erc20Token.allowance(currentContract, spender) == 0
    filtered {
        f -> f.contract != erc20Token
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
