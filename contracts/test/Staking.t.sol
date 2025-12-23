// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {Staking, Ownable} from "../src/Staking.sol";
import {ERC20, IERC20Errors} from "@oz/token/ERC20/ERC20.sol";

contract MockERC20 is ERC20 {
    constructor(string memory name, string memory symbol) ERC20(name, symbol) {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

contract StakingTest is Test {
    Staking public staking;
    MockERC20 public token;

    address public owner = address(0x1);
    address public validator = address(0x2);
    address public staker = address(0x3);
    address public staker2 = address(0x4);
    address public other = address(0x5);

    uint128 public constant INITIAL_WITHDRAW_DELAY = 1 days;
    uint256 public constant CONFIG_TIME_DELAY = 2 days;

    event StakeIncreased(address indexed staker, address indexed validator, uint256 amount);
    event WithdrawalInitiated(
        address indexed staker, address indexed validator, uint64 indexed withdrawalId, uint256 amount
    );
    event WithdrawalClaimed(address indexed staker, address indexed validator, uint256 amount);
    event ValidatorsProposed(
        bytes32 indexed validatorsHash, address[] validator, bool[] isRegistration, uint256 executableAt
    );
    event ValidatorUpdated(address indexed validator, bool isRegistered);
    event WithdrawDelayProposed(uint256 currentDelay, uint256 proposedDelay, uint256 executableAt);
    event WithdrawDelayChanged(uint256 oldDelay, uint256 newDelay);
    event TokensRecovered(address indexed token, address indexed to, uint256 amount);

    function setUp() public {
        vm.startPrank(owner);
        token = new MockERC20("Safe Token", "SAFE");
        staking = new Staking(owner, address(token), INITIAL_WITHDRAW_DELAY, CONFIG_TIME_DELAY);

        // Register validator
        address[] memory validators = new address[](1);
        validators[0] = validator;
        bool[] memory isRegistration = new bool[](1);
        isRegistration[0] = true;

        staking.proposeValidators(validators, isRegistration);
        vm.warp(block.timestamp + CONFIG_TIME_DELAY);
        staking.executeValidatorChanges(validators, isRegistration, block.timestamp);
        vm.stopPrank();

        // Setup staker
        token.mint(staker, 10000 ether);
        token.mint(staker2, 10000 ether);

        vm.prank(staker);
        token.approve(address(staking), type(uint256).max);

        vm.prank(staker2);
        token.approve(address(staking), type(uint256).max);
    }

    // ============================================================
    // HELPER FUNCTIONS
    // ============================================================

    /// @dev Internal function to update validator registration status
    function _updateValidator(address _validator, bool _isRegistration) internal {
        vm.startPrank(owner);
        address[] memory validators = new address[](1);
        validators[0] = _validator;
        bool[] memory isRegistration = new bool[](1);
        isRegistration[0] = _isRegistration;
        staking.proposeValidators(validators, isRegistration);
        vm.warp(block.timestamp + CONFIG_TIME_DELAY);
        staking.executeValidatorChanges(validators, isRegistration, block.timestamp);
        vm.stopPrank();
    }

    /// @dev Registers a new validator through the timelock process
    function _registerValidator(address _validator) internal {
        _updateValidator(_validator, true);
    }

    /// @dev Deregisters a validator through the timelock process
    function _deregisterValidator(address _validator) internal {
        _updateValidator(_validator, false);
    }

    /// @dev Stakes tokens as a specific staker to a validator
    function _stakeAs(address _staker, address _validator, uint256 _amount) internal {
        vm.prank(_staker);
        staking.stake(_validator, _amount);
    }

    /// @dev Initiates a withdrawal and returns the withdrawal ID
    function _initiateWithdrawalAs(address _staker, address _validator, uint256 _amount)
        internal
        returns (uint64 expectedId)
    {
        expectedId = staking.nextWithdrawalId();
        vm.prank(_staker);
        staking.initiateWithdrawal(_validator, _amount);
    }

    /// @dev Asserts complete state consistency for a staker-validator pair
    function _assertStakeState(
        address _staker,
        address _validator,
        uint256 expectedStake,
        uint256 expectedTotalValidatorStake,
        uint256 expectedTotalStakerStake
    ) internal view {
        assertEq(staking.stakes(_staker, _validator), expectedStake);
        assertEq(staking.totalValidatorStakes(_validator), expectedTotalValidatorStake);
        assertEq(staking.totalStakerStakes(_staker), expectedTotalStakerStake);
    }

    // ============================================================
    // CONFIGURATION & ACCESS CONTROL
    // ============================================================

    function test_Constructor() public view {
        assertEq(address(staking.SAFE_TOKEN()), address(token));
        assertEq(staking.CONFIG_TIME_DELAY(), CONFIG_TIME_DELAY);
        assertEq(staking.withdrawDelay(), INITIAL_WITHDRAW_DELAY);
        assertEq(staking.owner(), owner);
    }

    function test_Constructor_Event() public {
        vm.expectEmit(false, false, false, true);
        emit WithdrawDelayChanged(0, INITIAL_WITHDRAW_DELAY);
        new Staking(owner, address(token), INITIAL_WITHDRAW_DELAY, CONFIG_TIME_DELAY);
    }

    function test_RevertWhen_Constructor_InvalidDelay() public {
        vm.expectRevert(Staking.InvalidParameter.selector);
        // forge-lint: disable-next-line(unsafe-typecast)
        new Staking(owner, address(token), uint128(CONFIG_TIME_DELAY + 1), CONFIG_TIME_DELAY);
    }

    function test_RevertWhen_Constructor_ZeroSafeToken() public {
        vm.expectRevert(Staking.InvalidAddress.selector);
        new Staking(owner, address(0), INITIAL_WITHDRAW_DELAY, CONFIG_TIME_DELAY);
    }

    function test_RevertWhen_Constructor_ZeroWithdrawDelay() public {
        vm.expectRevert(Staking.InvalidParameter.selector);
        new Staking(owner, address(token), 0, CONFIG_TIME_DELAY);
    }

    function test_RevertWhen_Constructor_ZeroConfigTimeDelay() public {
        vm.expectRevert(Staking.InvalidParameter.selector);
        new Staking(owner, address(token), INITIAL_WITHDRAW_DELAY, 0);
    }

    // ============================================================
    // VALIDATOR REGISTRATION/DEREGISTRATION
    // ============================================================

    function test_ProposeValidators() public {
        vm.startPrank(owner);
        address[] memory validators = new address[](1);
        validators[0] = other;
        bool[] memory isRegistration = new bool[](1);
        isRegistration[0] = true;

        uint256 expectedExecutableAt = block.timestamp + CONFIG_TIME_DELAY;

        vm.expectEmit(true, false, false, true);
        emit ValidatorsProposed(
            keccak256(abi.encode(validators, isRegistration, expectedExecutableAt)),
            validators,
            isRegistration,
            expectedExecutableAt
        );

        staking.proposeValidators(validators, isRegistration);

        assertEq(
            staking.pendingValidatorChangeHash(),
            keccak256(abi.encode(validators, isRegistration, expectedExecutableAt))
        );
        vm.stopPrank();
    }

    function test_RevertWhen_ProposeValidators_LengthMismatch() public {
        vm.startPrank(owner);
        address[] memory validators = new address[](1);
        validators[0] = other;
        bool[] memory isRegistration = new bool[](0); // Mismatch

        vm.expectRevert(Staking.ArrayLengthMismatch.selector);
        staking.proposeValidators(validators, isRegistration);
        vm.stopPrank();
    }

    function test_RevertWhen_ProposeValidators_ZeroAddress() public {
        vm.startPrank(owner);
        address[] memory validators = new address[](1);
        validators[0] = address(0);
        bool[] memory isRegistration = new bool[](1);
        isRegistration[0] = true;

        vm.expectRevert(Staking.InvalidAddress.selector);
        staking.proposeValidators(validators, isRegistration);
        vm.stopPrank();
    }

    function test_ExecuteValidatorChanges() public {
        vm.startPrank(owner);
        address[] memory validators = new address[](1);
        validators[0] = other;
        bool[] memory isRegistration = new bool[](1);
        isRegistration[0] = true;

        staking.proposeValidators(validators, isRegistration);
        uint256 executableAt = block.timestamp + CONFIG_TIME_DELAY;

        vm.warp(executableAt);

        vm.stopPrank(); // Execution is public

        vm.expectEmit(true, false, false, true);
        emit ValidatorUpdated(other, true);

        assertFalse(staking.isValidator(other));

        staking.executeValidatorChanges(validators, isRegistration, executableAt);

        assertTrue(staking.isValidator(other));
        assertEq(staking.pendingValidatorChangeHash(), bytes32(0));
    }

    function test_RevertWhen_ExecuteValidatorChanges_InvalidParams() public {
        vm.startPrank(owner);
        address[] memory validators = new address[](1);
        validators[0] = other;
        bool[] memory isRegistration = new bool[](1);
        isRegistration[0] = true;

        staking.proposeValidators(validators, isRegistration);
        uint256 executableAt = block.timestamp + CONFIG_TIME_DELAY;
        vm.warp(executableAt);
        vm.stopPrank();

        // Incorrect executable timestamp
        vm.expectRevert(Staking.InvalidProposalHash.selector);
        staking.executeValidatorChanges(validators, isRegistration, executableAt + 1);

        // Incorrect hash (by changing validators)
        address[] memory badValidators = new address[](1);
        badValidators[0] = address(0x99);
        vm.expectRevert(Staking.InvalidProposalHash.selector);
        staking.executeValidatorChanges(badValidators, isRegistration, executableAt);

        // Incorrect hash (by changing isRegistration)
        bool[] memory badRegistration = new bool[](0);
        vm.expectRevert(Staking.InvalidProposalHash.selector);
        staking.executeValidatorChanges(validators, badRegistration, executableAt);
    }

    function test_RevertWhen_ExecuteValidatorChanges_NoProposal() public {
        address[] memory validators = new address[](1);
        validators[0] = other;
        bool[] memory isRegistration = new bool[](1);
        isRegistration[0] = true;

        vm.expectRevert(Staking.NoProposalExists.selector);
        staking.executeValidatorChanges(validators, isRegistration, block.timestamp);
    }

    function test_RevertWhen_ExecuteValidatorChanges_Timelock() public {
        vm.startPrank(owner);
        address[] memory validators = new address[](1);
        validators[0] = other;
        bool[] memory isRegistration = new bool[](1);
        isRegistration[0] = true;

        staking.proposeValidators(validators, isRegistration);
        uint256 executableAt = block.timestamp + CONFIG_TIME_DELAY;
        vm.stopPrank();

        vm.expectRevert(Staking.ProposalNotExecutable.selector);
        staking.executeValidatorChanges(validators, isRegistration, executableAt);
    }

    function test_DeregisterValidator() public {
        vm.startPrank(owner);
        address[] memory validators = new address[](1);
        validators[0] = validator;
        bool[] memory isRegistration = new bool[](1);
        isRegistration[0] = false; // Deregister

        staking.proposeValidators(validators, isRegistration);
        uint256 executableAt = block.timestamp + CONFIG_TIME_DELAY;
        vm.warp(executableAt);

        vm.expectEmit(true, false, false, true);
        emit ValidatorUpdated(validator, false);
        staking.executeValidatorChanges(validators, isRegistration, executableAt);

        assertFalse(staking.isValidator(validator));
        vm.stopPrank();
    }

    function test_BatchValidatorChanges() public {
        vm.startPrank(owner);
        address validator2 = address(0x10);
        address validator3 = address(0x11);

        address[] memory validators = new address[](3);
        validators[0] = validator2;
        validators[1] = validator3;
        validators[2] = validator; // Deregister existing
        bool[] memory isRegistration = new bool[](3);
        isRegistration[0] = true;
        isRegistration[1] = true;
        isRegistration[2] = false;

        staking.proposeValidators(validators, isRegistration);
        uint256 executableAt = block.timestamp + CONFIG_TIME_DELAY;
        vm.warp(executableAt);
        staking.executeValidatorChanges(validators, isRegistration, executableAt);

        assertTrue(staking.isValidator(validator2));
        assertTrue(staking.isValidator(validator3));
        assertFalse(staking.isValidator(validator));
        vm.stopPrank();
    }

    function test_ProposeValidators_OverwritesPending() public {
        vm.startPrank(owner);

        // First proposal
        address[] memory validators1 = new address[](1);
        validators1[0] = other;
        bool[] memory isRegistration1 = new bool[](1);
        isRegistration1[0] = true;
        staking.proposeValidators(validators1, isRegistration1);

        bytes32 firstHash = staking.pendingValidatorChangeHash();

        // Second proposal (overwrites first)
        address validator2 = address(0x10);
        address[] memory validators2 = new address[](1);
        validators2[0] = validator2;
        bool[] memory isRegistration2 = new bool[](1);
        isRegistration2[0] = true;
        staking.proposeValidators(validators2, isRegistration2);

        bytes32 secondHash = staking.pendingValidatorChangeHash();

        assertTrue(firstHash != secondHash);
        assertTrue(secondHash != bytes32(0));
        vm.stopPrank();
    }

    function test_RegisterAlreadyRegisteredValidator() public {
        // validator is already registered in setUp
        assertTrue(staking.isValidator(validator));

        vm.startPrank(owner);
        address[] memory validators = new address[](1);
        validators[0] = validator;
        bool[] memory isRegistration = new bool[](1);
        isRegistration[0] = true; // Register again

        staking.proposeValidators(validators, isRegistration);
        uint256 executableAt = block.timestamp + CONFIG_TIME_DELAY;
        vm.warp(executableAt);
        staking.executeValidatorChanges(validators, isRegistration, executableAt);

        // Should still be registered (no-op)
        assertTrue(staking.isValidator(validator));
        vm.stopPrank();
    }

    function test_DeregisterUnregisteredValidator() public {
        assertFalse(staking.isValidator(other));

        vm.startPrank(owner);
        address[] memory validators = new address[](1);
        validators[0] = other;
        bool[] memory isRegistration = new bool[](1);
        isRegistration[0] = false; // Deregister unregistered

        staking.proposeValidators(validators, isRegistration);
        uint256 executableAt = block.timestamp + CONFIG_TIME_DELAY;
        vm.warp(executableAt);
        staking.executeValidatorChanges(validators, isRegistration, executableAt);

        // Should still be unregistered (no-op)
        assertFalse(staking.isValidator(other));
        vm.stopPrank();
    }

    function test_RevertWhen_ProposeValidators_EmptyArray() public {
        vm.startPrank(owner);
        address[] memory validators = new address[](0);
        bool[] memory isRegistration = new bool[](0);

        vm.expectRevert(Staking.InvalidParameter.selector);
        staking.proposeValidators(validators, isRegistration);
        vm.stopPrank();
    }

    // ============================================================
    // WITHDRAW DELAY CONFIGURATION
    // ============================================================

    function test_ProposeWithdrawDelay() public {
        vm.startPrank(owner);
        uint128 newDelay = 2 days;
        uint256 expectedExecutableAt = block.timestamp + CONFIG_TIME_DELAY;

        vm.expectEmit(false, false, false, true);
        emit WithdrawDelayProposed(INITIAL_WITHDRAW_DELAY, newDelay, expectedExecutableAt);

        staking.proposeWithdrawDelay(newDelay);

        (uint128 val, uint128 execAt) = staking.pendingWithdrawDelayChange();
        assertEq(val, newDelay);
        assertEq(execAt, expectedExecutableAt);
        vm.stopPrank();
    }

    function test_RevertWhen_ProposeWithdrawDelay_Invalid() public {
        vm.startPrank(owner);
        vm.expectRevert(Staking.InvalidParameter.selector);
        // forge-lint: disable-next-line(unsafe-typecast)
        staking.proposeWithdrawDelay(uint128(CONFIG_TIME_DELAY + 1));
        vm.stopPrank();
    }

    function test_ExecuteWithdrawDelayChange() public {
        vm.startPrank(owner);
        uint128 newDelay = 2 days;
        staking.proposeWithdrawDelay(newDelay);
        vm.warp(block.timestamp + CONFIG_TIME_DELAY);
        vm.stopPrank();

        vm.expectEmit(false, false, false, true);
        emit WithdrawDelayChanged(INITIAL_WITHDRAW_DELAY, newDelay);

        staking.executeWithdrawDelayChange();
        assertEq(staking.withdrawDelay(), newDelay);
    }

    function test_RevertWhen_ExecuteWithdrawDelayChange_NoProposal() public {
        vm.expectRevert(Staking.NoProposalExists.selector);
        staking.executeWithdrawDelayChange();
    }

    function test_RevertWhen_ExecuteWithdrawDelayChange_Timelock() public {
        vm.startPrank(owner);
        staking.proposeWithdrawDelay(2 days);
        vm.stopPrank();

        vm.expectRevert(Staking.ProposalNotExecutable.selector);
        staking.executeWithdrawDelayChange();
    }

    // ============================================================
    // STAKING LOGIC
    // ============================================================

    function test_Stake() public {
        uint256 amount = 100 ether;

        vm.prank(staker);
        vm.expectEmit(true, true, false, true);
        emit StakeIncreased(staker, validator, amount);
        staking.stake(validator, amount);

        _assertStakeState(staker, validator, amount, amount, amount);
        assertEq(staking.totalStakedAmount(), amount);
        assertEq(token.balanceOf(address(staking)), amount);
    }

    function test_RevertWhen_Stake_ZeroAmount() public {
        vm.prank(staker);
        vm.expectRevert(Staking.InvalidAmount.selector);
        staking.stake(validator, 0);
    }

    function test_RevertWhen_Stake_UnregisteredValidator() public {
        vm.prank(staker);
        vm.expectRevert(Staking.NotValidator.selector);
        staking.stake(other, 100 ether);
    }

    function test_RevertWhen_Stake_InvalidValidatorAddress() public {
        vm.prank(staker);
        vm.expectRevert(Staking.InvalidAddress.selector);
        staking.stake(address(0), 100 ether);
    }

    function test_Stake_MultipleValidators() public {
        _registerValidator(other);

        uint256 amount = 100 ether;
        _stakeAs(staker, validator, amount);
        _stakeAs(staker, other, amount);

        assertEq(staking.stakes(staker, validator), amount);
        assertEq(staking.stakes(staker, other), amount);
        assertEq(staking.totalStakerStakes(staker), amount * 2);
    }

    function test_Stake_MultipleStakers() public {
        uint256 amount = 100 ether;

        _stakeAs(staker, validator, amount);
        _stakeAs(staker2, validator, amount);

        assertEq(staking.totalValidatorStakes(validator), amount * 2);
    }

    function test_Stake_TotalAmount() public {
        uint256 amount = 100 ether;

        _stakeAs(staker, validator, amount);
        assertEq(staking.totalStakedAmount(), amount);

        _stakeAs(staker2, validator, amount);
        assertEq(staking.totalStakedAmount(), amount * 2);
    }

    function test_Stake_Incremental() public {
        uint256 amount1 = 100 ether;
        uint256 amount2 = 50 ether;

        vm.startPrank(staker);
        staking.stake(validator, amount1);

        _assertStakeState(staker, validator, amount1, amount1, amount1);

        // Stake again to same validator
        staking.stake(validator, amount2);

        _assertStakeState(staker, validator, amount1 + amount2, amount1 + amount2, amount1 + amount2);
        assertEq(staking.totalStakedAmount(), amount1 + amount2);
        vm.stopPrank();
    }

    function test_RevertWhen_Stake_InsufficientAllowance() public {
        // Reset allowance to 0
        vm.prank(staker);
        token.approve(address(staking), 0);

        vm.prank(staker);
        vm.expectRevert(
            abi.encodeWithSelector(IERC20Errors.ERC20InsufficientAllowance.selector, address(staking), 0, 100 ether)
        );
        staking.stake(validator, 100 ether);
    }

    function test_RevertWhen_Stake_InsufficientBalance() public {
        address poorStaker = address(0x99);
        token.mint(poorStaker, 10 ether);

        vm.startPrank(poorStaker);
        token.approve(address(staking), type(uint256).max);

        vm.expectRevert(
            abi.encodeWithSelector(IERC20Errors.ERC20InsufficientBalance.selector, poorStaker, 10 ether, 100 ether)
        );
        staking.stake(validator, 100 ether); // More than balance
        vm.stopPrank();
    }

    // ============================================================
    // WITHDRAWAL QUEUE
    // ============================================================

    function test_InitiateWithdrawal_Head() public {
        uint256 stakeAmount = 100 ether;
        uint256 withdrawAmount = 50 ether;

        vm.startPrank(staker);
        staking.stake(validator, stakeAmount);

        vm.expectEmit(true, true, true, true);
        emit WithdrawalInitiated(staker, validator, 1, withdrawAmount); // ID starts at 1
        staking.initiateWithdrawal(validator, withdrawAmount);

        (uint64 head, uint64 tail) = staking.withdrawalQueues(staker, validator);
        assertEq(head, 1);
        assertEq(tail, 1);

        (uint256 amt, uint256 claimableAt, uint64 prev, uint64 next) = staking.withdrawalNodes(1);
        assertEq(amt, withdrawAmount);
        assertEq(claimableAt, block.timestamp + INITIAL_WITHDRAW_DELAY);
        assertEq(prev, 0);
        assertEq(next, 0);

        assertEq(staking.totalPendingWithdrawals(), withdrawAmount);
        assertEq(staking.totalStakedAmount(), stakeAmount - withdrawAmount);
        vm.stopPrank();
    }

    function test_InitiateWithdrawal_Tail() public {
        uint256 stakeAmount = 100 ether;
        vm.startPrank(staker);
        staking.stake(validator, stakeAmount);

        staking.initiateWithdrawal(validator, 10 ether); // ID 1
        vm.warp(block.timestamp + 100);
        staking.initiateWithdrawal(validator, 10 ether); // ID 2

        (uint64 head, uint64 tail) = staking.withdrawalQueues(staker, validator);
        assertEq(head, 1);
        assertEq(tail, 2);

        (,,, uint64 next1) = staking.withdrawalNodes(1);
        (,, uint64 prev2, uint64 next2) = staking.withdrawalNodes(2);

        assertEq(next1, 2);
        assertEq(prev2, 1);
        assertEq(next2, 0);
        vm.stopPrank();
    }

    function test_InitiateWithdrawal_ReducedDelay_InsertsAtHead() public {
        // 0. Stake first
        vm.startPrank(staker);
        staking.stake(validator, 100 ether);
        vm.stopPrank();

        // 1. Propose to reduce delay to 1 second
        vm.startPrank(owner);
        staking.proposeWithdrawDelay(1);
        vm.stopPrank();

        // 2. Warp to just before execution time
        vm.warp(block.timestamp + CONFIG_TIME_DELAY);

        // 3. Initiate W1 with OLD delay (1 day)
        // Claimable = T + 1d
        vm.startPrank(staker);
        staking.initiateWithdrawal(validator, 10 ether); // ID 1
        vm.stopPrank();

        // 4. Execute new delay (1 second)
        vm.startPrank(owner);
        staking.executeWithdrawDelayChange();
        vm.stopPrank();

        // 5. Initiate W2 with NEW delay (1 second)
        // Claimable = T + 1s
        // Since (T + 1s) < (T + 1d), W2 should be inserted BEFORE W1.
        vm.startPrank(staker);
        staking.initiateWithdrawal(validator, 10 ether); // ID 2

        (uint64 head, uint64 tail) = staking.withdrawalQueues(staker, validator);

        // W2 should be HEAD, W1 should be TAIL
        assertEq(head, 2);
        assertEq(tail, 1);

        (,, uint64 prev2, uint64 next2) = staking.withdrawalNodes(2);
        (,, uint64 prev1, uint64 next1) = staking.withdrawalNodes(1);

        // Check links: 2 -> 1
        assertEq(next2, 1);
        assertEq(prev2, 0);
        assertEq(next1, 0);
        assertEq(prev1, 2);

        vm.stopPrank();
    }

    function test_InitiateWithdrawal_ReducedDelay_InsertsInMiddle() public {
        // 0. Stake enough for 3 withdrawals
        vm.startPrank(staker);
        staking.stake(validator, 300 ether);
        vm.stopPrank();

        // 1. Propose to reduce delay to 1 second (from 1 day = 86400s)
        vm.startPrank(owner);
        staking.proposeWithdrawDelay(1);
        vm.stopPrank();

        // 2. Warp to just before execution time
        vm.warp(block.timestamp + CONFIG_TIME_DELAY);

        // 3. Initiate W1 with OLD delay (1 day) at T=0 (relative to now)
        // Claimable = T + 86400
        vm.startPrank(staker);
        staking.initiateWithdrawal(validator, 10 ether); // ID 1
        vm.stopPrank();

        // 4. Advance time by 100s
        vm.warp(block.timestamp + 100);

        // 5. Initiate W2 with OLD delay (1 day) at T=100
        // Claimable = T + 100 + 86400 = T + 86500
        vm.startPrank(staker);
        staking.initiateWithdrawal(validator, 10 ether); // ID 2
        vm.stopPrank();

        // Queue: W1 (86400) -> W2 (86500)

        // 6. Execute new delay (1 second)
        vm.startPrank(owner);
        staking.executeWithdrawDelayChange();
        vm.stopPrank();

        // 7. Advance time to T=86450
        // We need T + 86450.
        // Current time is T + 100.
        // Delta = 86350.
        vm.warp(block.timestamp + 86350);

        // 8. Initiate W3 with NEW delay (1s) at T=86450
        // Claimable = T + 86450 + 1 = T + 86451
        //
        // Ordering:
        // W1 Claimable: T + 86400
        // W3 Claimable: T + 86451
        // W2 Claimable: T + 86500
        //
        // W3 should be inserted BETWEEN W1 and W2.
        vm.startPrank(staker);
        staking.initiateWithdrawal(validator, 10 ether); // ID 3
        vm.stopPrank();

        (uint64 head, uint64 tail) = staking.withdrawalQueues(staker, validator);
        assertEq(head, 1);
        assertEq(tail, 2);

        (,,, uint64 next1) = staking.withdrawalNodes(1);
        (,, uint64 prev2,) = staking.withdrawalNodes(2);
        (,, uint64 prev3, uint64 next3) = staking.withdrawalNodes(3);

        // W1 -> W3 -> W2
        assertEq(next1, 3);
        assertEq(prev3, 1);
        assertEq(next3, 2);
        assertEq(prev2, 3);
    }

    function test_RevertWhen_InitiateWithdrawal_InsufficientStake() public {
        vm.startPrank(staker);
        staking.stake(validator, 100 ether);

        vm.expectRevert(Staking.InsufficientStake.selector);
        staking.initiateWithdrawal(validator, 101 ether);
        vm.stopPrank();
    }

    function test_InitiateWithdrawalAtPosition_Tail() public {
        // Manually testing insertion logic
        uint256 stakeAmount = 100 ether;
        vm.startPrank(staker);
        staking.stake(validator, stakeAmount);

        // W1
        staking.initiateWithdrawal(validator, 10 ether); // ID 1

        // W2 - Insert after W1
        vm.warp(block.timestamp + 100);
        staking.initiateWithdrawalAtPosition(validator, 10 ether, 1); // ID 2

        (uint64 head, uint64 tail) = staking.withdrawalQueues(staker, validator);
        assertEq(head, 1);
        assertEq(tail, 2);

        (,, uint64 prev2, uint64 next2) = staking.withdrawalNodes(2);
        assertEq(prev2, 1);
        assertEq(next2, 0);

        vm.stopPrank();
    }

    function test_InitiateWithdrawalAtPosition_Middle() public {
        uint256 stakeAmount = 100 ether;
        vm.startPrank(staker);
        staking.stake(validator, stakeAmount);

        // Change withdraw Delay to 20s for easier testing and queue up the next withdraw delay to 1s
        vm.startPrank(owner);
        staking.proposeWithdrawDelay(20);
        vm.warp(block.timestamp + CONFIG_TIME_DELAY);
        staking.executeWithdrawDelayChange();

        staking.proposeWithdrawDelay(1);
        vm.warp(block.timestamp + CONFIG_TIME_DELAY);
        vm.stopPrank();

        // 1. Initiate W1 at time T
        vm.startPrank(staker);
        staking.initiateWithdrawal(validator, 10 ether); // ID 1

        // 2. Advance time by 5 (i.e. T + 5) and insert W2 after W1
        //    New claimableAt is strictly greater than W1's.
        vm.warp(block.timestamp + 5);
        staking.initiateWithdrawalAtPosition(validator, 10 ether, 1); // ID 2

        // 3. Execute the withdraw delay to 1s
        staking.executeWithdrawDelayChange();

        // 4. Advance time by 15 (i.e. T + 20)  and insert W3 after W1
        //    New claimableAt is strictly greater than W1's, and less than W2's,
        //    final ordering by claimableAt is W1 < W3 < W2.
        vm.warp(block.timestamp + 15);
        staking.initiateWithdrawalAtPosition(validator, 10 ether, 1); // ID 3
        vm.stopPrank();

        (uint64 head, uint64 tail) = staking.withdrawalQueues(staker, validator);
        assertEq(head, 1);
        assertEq(tail, 2);

        // Check links: 1 -> 3 -> 2
        (,, uint64 prev1, uint64 next1) = staking.withdrawalNodes(1);
        (,, uint64 prev2, uint64 next2) = staking.withdrawalNodes(2);
        (,, uint64 prev3, uint64 next3) = staking.withdrawalNodes(3);

        assertEq(prev1, 0);
        assertEq(next1, 3);
        assertEq(prev2, 3);
        assertEq(next2, 0);
        assertEq(prev3, 1);
        assertEq(next3, 2);
    }

    function test_InitiateWithdrawalAtPosition_Head() public {
        uint256 stakeAmount = 100 ether;
        vm.prank(staker);
        staking.stake(validator, stakeAmount);

        // Change withdraw Delay to 10s for easier testing
        vm.prank(owner);
        staking.proposeWithdrawDelay(10);
        vm.warp(block.timestamp + CONFIG_TIME_DELAY);

        // 1. Initiate W1 at time T
        vm.startPrank(staker);
        staking.initiateWithdrawal(validator, 10 ether); // ID 1

        // 2. Execute the withdraw delay to 10s
        staking.executeWithdrawDelayChange();

        // 4. Insert W2 after W1
        //    New claimableAt is strictly less than W1's,
        //    final ordering by claimableAt is W2 < W1.
        staking.initiateWithdrawalAtPosition(validator, 10 ether, 0); // ID 2
        vm.stopPrank();

        (uint64 head, uint64 tail) = staking.withdrawalQueues(staker, validator);
        assertEq(head, 2);
        assertEq(tail, 1);

        // Check links: 2 -> 1
        (,, uint64 prev1, uint64 next1) = staking.withdrawalNodes(1);
        (,, uint64 prev2, uint64 next2) = staking.withdrawalNodes(2);

        assertEq(prev1, 2);
        assertEq(next1, 0);
        assertEq(prev2, 0);
        assertEq(next2, 1);
    }

    function test_RevertWhen_InitiateWithdrawalAtPosition_InvalidOrdering() public {
        uint256 stakeAmount = 100 ether;
        vm.prank(staker);
        staking.stake(validator, stakeAmount);

        // Change withdraw Delay to 20s for easier testing and queue up the next withdraw delay to 1s
        vm.startPrank(owner);
        staking.proposeWithdrawDelay(20);
        vm.warp(block.timestamp + CONFIG_TIME_DELAY);
        staking.executeWithdrawDelayChange();
        staking.proposeWithdrawDelay(1);
        vm.warp(block.timestamp + CONFIG_TIME_DELAY);
        vm.stopPrank();

        vm.startPrank(staker);

        // Setup: W1 (T) -> W2 (T+100)
        // W1 Claimable: T + 20s
        staking.initiateWithdrawalAtPosition(validator, 10 ether, 0); // ID 1

        vm.warp(block.timestamp + 10);
        // W2 Claimable: T + 10 + 20s
        staking.initiateWithdrawalAtPosition(validator, 10 ether, 1); // ID 2

        // Queue: W1 -> W2

        // Case 1: Insert at Head (prev=0) but New > Head
        // Current Time: T + 10. New Claimable: T + 10 + 20s.
        // Head (W1): T + 20s.
        // Invalid because New > Head.
        vm.expectRevert(Staking.InvalidOrdering.selector);
        staking.initiateWithdrawalAtPosition(validator, 10 ether, 0);

        // Case 2: Insert in Middle (prev=W1) but New > Next (W2)
        // Advance time further so the new claimableAt is
        // strictly greater than W2's claimableAt.
        vm.warp(block.timestamp + 1);
        vm.expectRevert(Staking.InvalidOrdering.selector);
        staking.initiateWithdrawalAtPosition(validator, 10 ether, 1);

        // Case 3: Insert in Middle (prev=W1) but New < Prev (W1)
        // Current Time: T + 10 + 1s.
        // W1 Claimable: T + 20s.
        // W2 Claimable: T + 30s.
        // New Claimable is less than W1's claimableAt.
        staking.executeWithdrawDelayChange(); // Now delay is 1s
        vm.expectRevert(Staking.InvalidOrdering.selector);
        staking.initiateWithdrawalAtPosition(validator, 10 ether, 1);

        // Case 4: Insert at Tail (prev=W2) but New < Prev (W2)
        // Current Time: T + 10 + 1s.
        // W2 Claimable: T + 30s.
        // New Claimable is less than W2's claimableAt.
        vm.expectRevert(Staking.InvalidOrdering.selector);
        staking.initiateWithdrawalAtPosition(validator, 10 ether, 2);

        vm.stopPrank();
    }

    function test_RevertWhen_InitiateWithdrawalAtPosition_InvalidPrevious() public {
        uint256 stakeAmount = 100 ether;
        vm.startPrank(staker);
        staking.stake(validator, stakeAmount);

        vm.expectRevert(Staking.InvalidWithdrawalNode.selector);
        staking.initiateWithdrawalAtPosition(validator, 10 ether, 999);
        vm.stopPrank();
    }

    function test_InitiateWithdrawal_FullStake() public {
        uint256 stakeAmount = 100 ether;

        vm.startPrank(staker);
        staking.stake(validator, stakeAmount);

        // Withdraw entire stake
        staking.initiateWithdrawal(validator, stakeAmount);

        _assertStakeState(staker, validator, 0, 0, 0);
        assertEq(staking.totalStakedAmount(), 0);
        assertEq(staking.totalPendingWithdrawals(), stakeAmount);
        vm.stopPrank();
    }

    function test_RevertWhen_InitiateWithdrawal_ZeroAmount() public {
        vm.startPrank(staker);
        staking.stake(validator, 100 ether);

        vm.expectRevert(Staking.InvalidAmount.selector);
        staking.initiateWithdrawal(validator, 0);
        vm.stopPrank();
    }

    function test_InitiateWithdrawalAtPosition_EmptyQueue() public {
        vm.startPrank(staker);
        staking.stake(validator, 100 ether);

        // Insert at position 0 (head) when queue is empty
        staking.initiateWithdrawalAtPosition(validator, 10 ether, 0);

        (uint64 head, uint64 tail) = staking.withdrawalQueues(staker, validator);
        assertEq(head, 1);
        assertEq(tail, 1);
        vm.stopPrank();
    }

    // ============================================================
    // CLAIMING
    // ============================================================

    function test_ClaimWithdrawal() public {
        uint256 stakeAmount = 100 ether;
        uint256 withdrawAmount = 10 ether;

        vm.startPrank(staker);
        staking.stake(validator, stakeAmount);
        staking.initiateWithdrawal(validator, withdrawAmount); // ID 1

        // Try claiming early
        vm.expectRevert(Staking.NoClaimableWithdrawal.selector);
        staking.claimWithdrawal(staker, validator);

        // Warp to claimable time
        vm.warp(block.timestamp + INITIAL_WITHDRAW_DELAY);

        uint256 preBalance = token.balanceOf(staker);

        vm.expectEmit(true, true, false, true);
        emit WithdrawalClaimed(staker, validator, withdrawAmount);
        staking.claimWithdrawal(staker, validator);

        assertEq(token.balanceOf(staker), preBalance + withdrawAmount);
        assertEq(staking.totalPendingWithdrawals(), 0);

        // Queue should be empty
        (uint64 head, uint64 tail) = staking.withdrawalQueues(staker, validator);
        assertEq(head, 0);
        assertEq(tail, 0);

        vm.stopPrank();
    }

    function test_ClaimWithdrawal_Multiple() public {
        uint256 stakeAmount = 100 ether;
        vm.startPrank(staker);
        staking.stake(validator, stakeAmount);

        staking.initiateWithdrawal(validator, 10 ether); // ID 1
        vm.warp(block.timestamp + 100);
        staking.initiateWithdrawal(validator, 20 ether); // ID 2

        // Queue: W1 -> W2

        // Warp to claim W1 but not W2
        // W1 claimable at T + Delay
        // W2 claimable at T + 100 + Delay

        vm.warp(block.timestamp + INITIAL_WITHDRAW_DELAY - 100); // At W1 claimable time

        staking.claimWithdrawal(staker, validator); // Claims W1

        // Queue: W2

        (uint64 head, uint64 tail) = staking.withdrawalQueues(staker, validator);
        assertEq(head, 2); // Head moved to W2
        assertEq(tail, 2);

        // Try claim W2 early
        vm.expectRevert(Staking.NoClaimableWithdrawal.selector);
        staking.claimWithdrawal(staker, validator);

        vm.warp(block.timestamp + 100);
        staking.claimWithdrawal(staker, validator); // Claims W2

        // Queue: Empty

        (head, tail) = staking.withdrawalQueues(staker, validator);
        assertEq(head, 0);

        vm.stopPrank();
    }

    function test_RevertWhen_ClaimWithdrawal_QueueEmpty() public {
        vm.startPrank(staker);
        staking.stake(validator, 100 ether);

        // No withdrawals initiated
        vm.expectRevert(Staking.WithdrawalQueueEmpty.selector);
        staking.claimWithdrawal(staker, validator);

        vm.stopPrank();
    }

    function test_ClaimWithdrawal_AnyoneCanClaim() public {
        uint256 stakeAmount = 100 ether;
        uint256 withdrawAmount = 10 ether;

        vm.startPrank(staker);
        staking.stake(validator, stakeAmount);
        staking.initiateWithdrawal(validator, withdrawAmount);
        vm.stopPrank();

        vm.warp(block.timestamp + INITIAL_WITHDRAW_DELAY);

        uint256 stakerBalanceBefore = token.balanceOf(staker);

        // `other` calls claim for `staker`
        vm.prank(other);
        staking.claimWithdrawal(staker, validator);

        // Tokens go to staker, not caller
        assertEq(token.balanceOf(staker), stakerBalanceBefore + withdrawAmount);
    }

    function test_ClaimWithdrawal_ExactlyAtClaimableTime() public {
        uint256 stakeAmount = 100 ether;
        uint256 withdrawAmount = 10 ether;

        vm.startPrank(staker);
        staking.stake(validator, stakeAmount);

        uint256 initiateTime = block.timestamp;
        staking.initiateWithdrawal(validator, withdrawAmount);
        vm.stopPrank();

        // Warp to exactly claimableAt (not 1 second after)
        vm.warp(initiateTime + INITIAL_WITHDRAW_DELAY);

        uint256 stakerBalanceBefore = token.balanceOf(staker);
        staking.claimWithdrawal(staker, validator);

        assertEq(token.balanceOf(staker), stakerBalanceBefore + withdrawAmount);
    }

    // ============================================================
    // TOKEN RECOVERY
    // ============================================================

    function test_RecoverTokens() public {
        // Send random tokens
        MockERC20 randomToken = new MockERC20("Random", "RND");
        randomToken.mint(address(staking), 100 ether);

        vm.startPrank(owner);
        uint256 preBalance = randomToken.balanceOf(owner);

        staking.recoverTokens(address(randomToken), owner);

        assertEq(randomToken.balanceOf(owner), preBalance + 100 ether);
        assertEq(randomToken.balanceOf(address(staking)), 0);
        vm.stopPrank();
    }

    function test_RecoverTokens_ExcessSafeToken() public {
        // Send extra SAFE tokens (not staked)
        token.mint(address(staking), 50 ether);

        vm.startPrank(owner);
        uint256 preBalance = token.balanceOf(owner);

        staking.recoverTokens(address(token), owner);

        assertEq(token.balanceOf(owner), preBalance + 50 ether);
        vm.stopPrank();
    }

    function test_RevertWhen_RecoverTokens_StakedFunds() public {
        vm.startPrank(staker);
        staking.stake(validator, 100 ether);
        vm.stopPrank();

        // Contract has 100 SAFE, but it's all staked
        vm.startPrank(owner);
        vm.expectRevert(Staking.InsufficientRecoverableAmount.selector);
        staking.recoverTokens(address(token), owner);
        vm.stopPrank();
    }

    function test_RevertWhen_RecoverTokens_InvalidAddress() public {
        vm.startPrank(owner);
        vm.expectRevert(Staking.InvalidAddress.selector);
        staking.recoverTokens(address(token), address(0));
        vm.stopPrank();
    }

    function test_RecoverTokens_WithStakedAndPending() public {
        // Stake some tokens
        vm.prank(staker);
        staking.stake(validator, 100 ether);

        // Initiate a withdrawal
        vm.prank(staker);
        staking.initiateWithdrawal(validator, 30 ether);

        // Send extra SAFE tokens (not staked or pending)
        token.mint(address(staking), 50 ether);

        // Contract now has: 100 staked & pending (70 - 30 split) + 50 extra = 150 SAFE
        // Recoverable = 150 - 70 - 30 = 50

        vm.startPrank(owner);
        uint256 preBalance = token.balanceOf(owner);

        staking.recoverTokens(address(token), owner);

        assertEq(token.balanceOf(owner), preBalance + 50 ether);
        vm.stopPrank();
    }

    // ============================================================
    // ACCESS CONTROL
    // ============================================================

    function test_RevertWhen_ProposeValidators_NotOwner() public {
        address[] memory validators = new address[](1);
        validators[0] = other;
        bool[] memory isRegistration = new bool[](1);
        isRegistration[0] = true;

        vm.prank(staker);
        vm.expectRevert(abi.encodeWithSelector(Ownable.OwnableUnauthorizedAccount.selector, staker));
        staking.proposeValidators(validators, isRegistration);
    }

    function test_RevertWhen_ProposeWithdrawDelay_NotOwner() public {
        vm.prank(staker);
        vm.expectRevert(abi.encodeWithSelector(Ownable.OwnableUnauthorizedAccount.selector, staker));
        staking.proposeWithdrawDelay(2 days);
    }

    function test_RevertWhen_RecoverTokens_NotOwner() public {
        MockERC20 randomToken = new MockERC20("Random", "RND");
        randomToken.mint(address(staking), 100 ether);

        vm.prank(staker);
        vm.expectRevert(abi.encodeWithSelector(Ownable.OwnableUnauthorizedAccount.selector, staker));
        staking.recoverTokens(address(randomToken), staker);
    }

    // ============================================================
    // EDGE CASES & BOUNDARY CONDITIONS
    // ============================================================

    function test_DeregisteredValidator_ExistingStakesCanWithdraw() public {
        // Stake first
        _stakeAs(staker, validator, 100 ether);

        // Deregister the validator
        _deregisterValidator(validator);

        assertFalse(staking.isValidator(validator));

        // Staker should still be able to withdraw their stake
        vm.startPrank(staker);
        staking.initiateWithdrawal(validator, 100 ether);

        assertEq(staking.stakes(staker, validator), 0);
        assertEq(staking.totalPendingWithdrawals(), 100 ether);

        // Wait and claim
        vm.warp(block.timestamp + INITIAL_WITHDRAW_DELAY);

        uint256 balanceBefore = token.balanceOf(staker);
        staking.claimWithdrawal(staker, validator);
        assertEq(token.balanceOf(staker), balanceBefore + 100 ether);
        vm.stopPrank();
    }

    function test_DeregisteredValidator_CannotStakeNew() public {
        _deregisterValidator(validator);

        // Cannot stake to deregistered validator
        vm.prank(staker);
        vm.expectRevert(Staking.NotValidator.selector);
        staking.stake(validator, 100 ether);
    }

    // ============================================================
    // VIEW FUNCTION COVERAGE
    // ============================================================

    function test_GetPendingWithdrawals() public {
        uint256 stakeAmount = 100 ether;
        vm.startPrank(staker);
        staking.stake(validator, stakeAmount);

        staking.initiateWithdrawal(validator, 10 ether);
        staking.initiateWithdrawal(validator, 20 ether);

        Staking.WithdrawalInfo[] memory withdrawals = staking.getPendingWithdrawals(staker, validator);
        assertEq(withdrawals.length, 2);
        assertEq(withdrawals[0].amount, 10 ether);
        assertEq(withdrawals[1].amount, 20 ether);
        vm.stopPrank();
    }

    function test_GetNextClaimableWithdrawal() public {
        uint256 stakeAmount = 100 ether;
        vm.startPrank(staker);
        staking.stake(validator, stakeAmount);

        // Empty queue
        (uint256 amt, uint256 time) = staking.getNextClaimableWithdrawal(staker, validator);
        assertEq(amt, 0);
        assertEq(time, 0);

        // One withdrawal
        staking.initiateWithdrawal(validator, 10 ether);
        (amt, time) = staking.getNextClaimableWithdrawal(staker, validator);
        assertEq(amt, 10 ether);
        assertEq(time, block.timestamp + INITIAL_WITHDRAW_DELAY);

        vm.stopPrank();
    }

    function test_GetPendingWithdrawals_EmptyQueue() public view {
        Staking.WithdrawalInfo[] memory withdrawals = staking.getPendingWithdrawals(staker, validator);
        assertEq(withdrawals.length, 0);
    }

    function test_GetPendingWithdrawals_ThreeOrMoreWithdrawals() public {
        vm.startPrank(staker);
        staking.stake(validator, 100 ether);

        staking.initiateWithdrawal(validator, 10 ether);
        vm.warp(block.timestamp + 1);
        staking.initiateWithdrawal(validator, 20 ether);
        vm.warp(block.timestamp + 1);
        staking.initiateWithdrawal(validator, 30 ether);
        vm.warp(block.timestamp + 1);
        staking.initiateWithdrawal(validator, 15 ether);

        Staking.WithdrawalInfo[] memory withdrawals = staking.getPendingWithdrawals(staker, validator);
        assertEq(withdrawals.length, 4);

        // Verify ordering by amount (which reflects insertion order here)
        assertEq(withdrawals[0].amount, 10 ether);
        assertEq(withdrawals[1].amount, 20 ether);
        assertEq(withdrawals[2].amount, 30 ether);
        assertEq(withdrawals[3].amount, 15 ether);

        // Verify claimableAt is increasing
        assertTrue(withdrawals[0].claimableAt <= withdrawals[1].claimableAt);
        assertTrue(withdrawals[1].claimableAt <= withdrawals[2].claimableAt);
        assertTrue(withdrawals[2].claimableAt <= withdrawals[3].claimableAt);
        vm.stopPrank();
    }

    // ============================================================
    // STATE CONSISTENCY AFTER OPERATIONS
    // ============================================================

    function test_StateConsistency_AfterFullWithdrawalAndClaim() public {
        uint256 stakeAmount = 100 ether;

        vm.startPrank(staker);
        staking.stake(validator, stakeAmount);

        // Withdraw all
        staking.initiateWithdrawal(validator, stakeAmount);

        _assertStakeState(staker, validator, 0, 0, 0);
        assertEq(staking.totalStakedAmount(), 0);
        assertEq(staking.totalPendingWithdrawals(), stakeAmount);

        vm.warp(block.timestamp + INITIAL_WITHDRAW_DELAY);
        staking.claimWithdrawal(staker, validator);

        // All state should be zeroed
        _assertStakeState(staker, validator, 0, 0, 0);
        assertEq(staking.totalStakedAmount(), 0);
        assertEq(staking.totalPendingWithdrawals(), 0);

        // Queue should be empty
        (uint64 head, uint64 tail) = staking.withdrawalQueues(staker, validator);
        assertEq(head, 0);
        assertEq(tail, 0);
        vm.stopPrank();
    }

    function test_StateConsistency_MultipleStakersMultipleValidators() public {
        _registerValidator(other);

        // Staker1 stakes to both validators
        _stakeAs(staker, validator, 100 ether);
        _stakeAs(staker, other, 50 ether);

        // Staker2 stakes to validator
        _stakeAs(staker2, validator, 75 ether);

        // Verify aggregates
        assertEq(staking.totalStakedAmount(), 225 ether);
        assertEq(staking.totalPendingWithdrawals(), 0);
        assertEq(staking.totalValidatorStakes(validator), 175 ether);
        assertEq(staking.totalValidatorStakes(other), 50 ether);
        assertEq(staking.totalStakerStakes(staker), 150 ether);
        assertEq(staking.totalStakerStakes(staker2), 75 ether);

        // Partial withdrawals
        vm.prank(staker);
        staking.initiateWithdrawal(validator, 30 ether);

        vm.prank(staker2);
        staking.initiateWithdrawal(validator, 25 ether);

        // Verify after withdrawals
        assertEq(staking.totalStakedAmount(), 170 ether);
        assertEq(staking.totalPendingWithdrawals(), 55 ether);
        assertEq(staking.totalValidatorStakes(validator), 120 ether);
        assertEq(staking.totalValidatorStakes(other), 50 ether);
        assertEq(staking.totalStakerStakes(staker), 120 ether);
        assertEq(staking.totalStakerStakes(staker2), 50 ether);

        // Contract balance should match
        assertEq(token.balanceOf(address(staking)), 225 ether);
    }

    function test_StateConsistency_TokenBalanceMatchesAccountedFunds() public {
        vm.startPrank(staker);
        staking.stake(validator, 100 ether);
        staking.initiateWithdrawal(validator, 30 ether);
        vm.stopPrank();

        vm.prank(staker2);
        staking.stake(validator, 50 ether);

        uint256 expectedBalance = staking.totalStakedAmount() + staking.totalPendingWithdrawals();
        assertEq(token.balanceOf(address(staking)), expectedBalance);
    }
}
