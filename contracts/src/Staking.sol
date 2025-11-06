// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Ownable} from "@oz/access/Ownable.sol";
import {IERC20} from "@oz/token/ERC20/IERC20.sol";
import {SafeERC20} from "@oz/token/ERC20/utils/SafeERC20.sol";

contract Staking is Ownable {
    using SafeERC20 for IERC20;

    // ============================================================
    // STRUCTS
    // ============================================================

    /*
     * @notice Represents a single withdrawal in the queue
     */
    struct WithdrawalNode {
        uint256 amount;
        uint128 claimedAt;
        uint128 next; // ID of next withdrawal, 0 if last
    }

    /*
     * @notice Tracks the withdrawal queue for a staker-validator pair
     */
    struct WithdrawalQueue {
        uint128 head; // ID of first withdrawal in queue, 0 if empty
        uint128 tail; // ID of last withdrawal in queue, 0 if empty
    }

    /*
     * @notice Represents a pending configuration change proposal
     */
    struct ConfigProposal {
        uint128 value;
        uint128 executableAt; // 0 if no proposal exists
    }

    /*
     * @notice Return type for view functions querying withdrawal info
     */
    struct WithdrawalInfo {
        uint256 amount;
        uint256 claimedAt;
    }

    // ============================================================
    // STORAGE VARIABLES
    // ============================================================

    /*
     * @notice The SAFE token used for staking
     */
    IERC20 public immutable SAFE_TOKEN;

    /*
     * @notice Time delay for configuration changes (immutable, set at deployment)
     */
    uint256 public immutable CONFIG_TIME_DELAY;

    /*
     * @notice Global counter for total staked tokens
     */
    uint256 public totalStakedAmount;

    /*
     * @notice Global counter for total pending withdrawals
     */
    uint256 public totalPendingWithdrawals;

    /*
     * @notice Withdraw time delay before tokens can be claimed
     */
    uint128 public withdrawDelay;

    /*
     * @notice Counter for generating unique withdrawal IDs
     */
    uint128 public nextWithdrawalId;

    // ============================================================
    // MAPPINGS
    // ============================================================

    /*
     * @notice Tracks if an address is a registered validator
     */
    mapping(address validator => bool isRegistered) public isValidator;

    /*
     * @notice Tracks total stake amount for each validator
     */
    mapping(address validator => uint256 totalStake) public totalStakes;

    /*
     * @notice Tracks individual stake amounts: staker => validator => amount
     */
    mapping(address staker => mapping(address validator => uint256 amount)) public stakes;

    /*
     * @notice Tracks withdrawal queues: staker => validator => queue
     */
    mapping(address staker => mapping(address validator => WithdrawalQueue queue)) public withdrawalQueues;

    /*
     * @notice Stores all withdrawal nodes by ID
     */
    mapping(uint128 withdrawalId => WithdrawalNode node) public withdrawalNodes;

    /*
     * @notice Pending proposal for withdraw delay change
     */
    ConfigProposal public pendingWithdrawDelayChange;

    /*
     * @notice Pending proposal for validator changes
     */
    bytes32 public pendingValidatorChangeHash;

    // ============================================================
    // EVENTS
    // ============================================================

    // Staking Operations
    event StakeIncreased(address indexed staker, address indexed validator, uint256 amount);
    event WithdrawalInitiated(address indexed staker, address indexed validator, uint256 amount);
    event WithdrawalClaimed(address indexed staker, address indexed validator, uint256 amount);

    // Validator Management
    event ValidatorsProposed(
        bytes32 indexed validatorsHash, address[] indexed validator, bool[] isRegistration, uint256 executableAt
    );
    event ValidatorUpdated(address indexed validator, bool isRegistered);

    // Configuration Changes
    event WithdrawDelayProposed(uint256 currentDelay, uint256 proposedDelay, uint256 executableAt);
    event WithdrawDelayChanged(uint256 oldDelay, uint256 newDelay);

    // Token Recovery
    event TokensRecovered(address indexed token, address indexed to, uint256 amount);

    // ============================================================
    // ERRORS
    // ============================================================

    /*
     * @notice Thrown when an amount parameter is 0 or invalid
     */
    error InvalidAmount();

    /*
     * @notice Thrown when an address parameter is the zero address
     */
    error InvalidAddress();

    /*
     * @notice Thrown when attempting to stake to a non-registered validator
     */
    error NotValidator();

    /*
     * @notice Thrown when trying to withdraw more than the current stake
     */
    error InsufficientStake();

    /*
     * @notice Thrown when trying to execute a proposal that hasn't been set
     */
    error ProposalNotSet();

    /*
     * @notice Thrown when trying to execute a proposal with an invalid hash
     */
    error InvalidProposalHash();

    /*
     * @notice Thrown when trying to execute a proposal before the timelock expires
     */
    error ProposalNotExecutable();

    /*
     * @notice Thrown when trying to execute a non-existent proposal
     */
    error NoProposalExists();

    /*
     * @notice Thrown when trying to recover more tokens than available
     */
    error InsufficientRecoverableAmount();

    /*
     * @notice Thrown when trying to claim from an empty withdrawal queue
     */
    error WithdrawalQueueEmpty();

    /*
     * @notice Thrown when trying to claim a withdrawal that isn't ready
     */
    error NoClaimableWithdrawal();

    /*
     * @notice Thrown when input arrays have mismatched lengths
     */
    error ArrayLengthMismatch();

    /*
     * @notice Thrown when a parameter is outside acceptable bounds
     */
    error InvalidParameter();

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    constructor(address initialOwner, address safeToken, uint128 initialWithdrawDelay, uint256 configTimeDelay)
        Ownable(initialOwner)
    {
        if (safeToken == address(0)) revert InvalidAddress();
        if (initialWithdrawDelay == 0) revert InvalidParameter();
        if (configTimeDelay == 0) revert InvalidParameter();
        if (initialWithdrawDelay > configTimeDelay) revert InvalidParameter();

        SAFE_TOKEN = IERC20(safeToken);
        CONFIG_TIME_DELAY = configTimeDelay;
        withdrawDelay = initialWithdrawDelay;
        nextWithdrawalId = 1;
    }

    // ============================================================
    // EXTERNAL FUNCTIONS - STAKING OPERATIONS
    // ============================================================

    /*
     * @notice Stake tokens toward a validator
     * @param validator The validator address to stake toward
     * @param amount The amount of tokens to stake
     */
    function stake(address validator, uint256 amount) external {
        if (amount == 0) revert InvalidAmount();
        if (validator == address(0)) revert InvalidAddress();
        if (!isValidator[validator]) revert NotValidator();

        stakes[msg.sender][validator] += amount;
        totalStakes[validator] += amount;
        totalStakedAmount += amount;
        emit StakeIncreased(msg.sender, validator, amount);

        SAFE_TOKEN.safeTransferFrom(msg.sender, address(this), amount);
    }

    /*
     * @notice Initiate a withdrawal of staked tokens
     * @param validator The validator address to withdraw from
     * @param amount The amount of tokens to withdraw
     */
    function initiateWithdrawal(address validator, uint256 amount) external {
        if (amount == 0) revert InvalidAmount();
        if (stakes[msg.sender][validator] < amount) revert InsufficientStake();

        stakes[msg.sender][validator] -= amount;
        totalStakes[validator] -= amount;
        totalStakedAmount -= amount;
        totalPendingWithdrawals += amount;

        // Calculate claimed timestamp
        uint128 claimedAt = uint128(block.timestamp);

        // Generate new withdrawal ID and create node
        uint128 withdrawalId = nextWithdrawalId++;
        withdrawalNodes[withdrawalId] = WithdrawalNode({amount: amount, claimedAt: claimedAt, next: 0});

        // Add to queue
        WithdrawalQueue storage queue = withdrawalQueues[msg.sender][validator];
        if (queue.head == 0) {
            // Queue is empty
            withdrawalQueues[msg.sender][validator] = WithdrawalQueue({head: withdrawalId, tail: withdrawalId});
        } else {
            // Link from tail
            withdrawalNodes[queue.tail].next = withdrawalId;
            queue.tail = withdrawalId;
        }

        emit WithdrawalInitiated(msg.sender, validator, amount);
    }

    /*
     * @notice Claim a pending withdrawal after the delay period
     * @param staker The address that initiated the withdrawal
     * @param validator The validator address to claim from
     */
    function claimWithdrawal(address staker, address validator) external {
        WithdrawalQueue memory queue = withdrawalQueues[staker][validator];
        if (queue.head == 0) revert WithdrawalQueueEmpty();

        WithdrawalNode memory node = withdrawalNodes[queue.head];
        if (block.timestamp < node.claimedAt + withdrawDelay) revert NoClaimableWithdrawal();

        uint256 amount = node.amount;

        if (node.next == 0) {
            // Queue is now empty
            withdrawalQueues[staker][validator] = WithdrawalQueue({head: 0, tail: 0});
        } else {
            withdrawalQueues[staker][validator].head = node.next;
        }

        delete withdrawalNodes[queue.head];
        totalPendingWithdrawals -= amount;
        emit WithdrawalClaimed(staker, validator, amount);

        SAFE_TOKEN.safeTransfer(staker, amount);
    }

    // ============================================================
    // EXTERNAL FUNCTIONS - CONFIGURATION PROPOSALS (OWNER ONLY)
    // ============================================================

    /*
     * @notice Propose a new withdraw delay
     * @param newDelay The proposed withdraw delay in seconds
     */
    function proposeWithdrawDelay(uint128 newDelay) external onlyOwner {
        if (newDelay == 0 || newDelay > CONFIG_TIME_DELAY) revert InvalidParameter();

        uint128 executableAt = uint128(block.timestamp + CONFIG_TIME_DELAY);
        pendingWithdrawDelayChange = ConfigProposal({value: newDelay, executableAt: executableAt});
        emit WithdrawDelayProposed(withdrawDelay, newDelay, executableAt);
    }

    /*
     * @notice Propose validator registration/deregistration changes
     * @param validators Array of validator addresses
     * @param isRegistration Array of booleans (true = register, false = deregister)
     * @dev It is currently possible to propose duplicate validators in a single proposal.
     */
    function proposeValidators(address[] calldata validators, bool[] calldata isRegistration) external onlyOwner {
        if (validators.length == 0) revert InvalidParameter();
        if (validators.length != isRegistration.length) revert ArrayLengthMismatch();

        uint256 executableAt = block.timestamp + CONFIG_TIME_DELAY;
        bytes32 validatorsHash = keccak256(abi.encode(validators, isRegistration, executableAt));
        for (uint256 i = 0; i < validators.length; i++) {
            if (validators[i] == address(0)) revert InvalidAddress();
        }

        pendingValidatorChangeHash = validatorsHash;

        emit ValidatorsProposed(validatorsHash, validators, isRegistration, executableAt);
    }

    // ============================================================
    // EXTERNAL FUNCTIONS - CONFIGURATION EXECUTION (PUBLIC)
    // ============================================================

    /*
     * @notice Execute a pending withdraw delay change
     */
    function executeWithdrawDelayChange() external {
        ConfigProposal memory proposal = pendingWithdrawDelayChange;
        if (proposal.executableAt == 0) revert NoProposalExists();
        if (block.timestamp < proposal.executableAt) revert ProposalNotExecutable();

        uint256 oldDelay = withdrawDelay;
        withdrawDelay = uint128(proposal.value);
        delete pendingWithdrawDelayChange;
        emit WithdrawDelayChanged(oldDelay, proposal.value);
    }

    /*
     * @notice Execute pending validator changes
     */
    function executeValidatorChanges(
        address[] calldata validators,
        bool[] calldata isRegistration,
        uint256 executableAt
    ) external {
        bytes32 proposalHash = pendingValidatorChangeHash;
        if (proposalHash == bytes32(0)) revert NoProposalExists();

        bytes32 validatorsHash = keccak256(abi.encode(validators, isRegistration, executableAt));
        if (proposalHash != validatorsHash) revert InvalidProposalHash();
        if (executableAt == 0) revert ProposalNotSet();
        if (block.timestamp < executableAt) revert ProposalNotExecutable();

        for (uint256 i = 0; i < validators.length; i++) {
            isValidator[validators[i]] = isRegistration[i];
            emit ValidatorUpdated(validators[i], isRegistration[i]);
        }

        pendingValidatorChangeHash = bytes32(0);
    }

    // ============================================================
    // EXTERNAL FUNCTIONS - TOKEN RECOVERY (OWNER ONLY)
    // ============================================================

    /*
     * @notice Recover accidentally sent tokens
     * @param token The token address to recover
     * @param to The address to send recovered tokens to
     */
    function recoverTokens(address token, address to) external onlyOwner {
        if (to == address(0)) revert InvalidAddress();

        uint256 recoverable;
        if (token == address(SAFE_TOKEN)) {
            uint256 balance = SAFE_TOKEN.balanceOf(address(this));
            recoverable = balance - totalStakedAmount - totalPendingWithdrawals;
        } else {
            recoverable = IERC20(token).balanceOf(address(this));
        }

        if (recoverable == 0) revert InsufficientRecoverableAmount();

        emit TokensRecovered(token, to, recoverable);

        IERC20(token).safeTransfer(to, recoverable);
    }

    // ============================================================
    // VIEW FUNCTIONS - WITHDRAWAL QUERIES
    // ============================================================

    /*
     * @notice Get all pending withdrawals for a staker-validator pair
     * @param staker The staker address
     * @param validator The validator address
     * @return An array of withdrawal info
     */
    function getPendingWithdrawals(address staker, address validator) external view returns (WithdrawalInfo[] memory) {
        WithdrawalQueue memory queue = withdrawalQueues[staker][validator];
        if (queue.head == 0) {
            return new WithdrawalInfo[](0);
        }

        // Count withdrawals
        uint256 count = 0;
        uint128 currentId = queue.head;
        while (currentId != 0) {
            count++;
            currentId = withdrawalNodes[currentId].next;
        }

        // Populate array
        WithdrawalInfo[] memory withdrawals = new WithdrawalInfo[](count);
        currentId = queue.head;
        for (uint256 i = 0; i < count; i++) {
            WithdrawalNode memory node = withdrawalNodes[currentId];
            withdrawals[i] = WithdrawalInfo({amount: node.amount, claimedAt: node.claimedAt});
            currentId = node.next;
        }

        return withdrawals;
    }

    /*
     * @notice Get the next claimable withdrawal for a staker-validator pair
     * @param staker The staker address
     * @param validator The validator address
     * @return amount The withdrawal amount
     * @return claimableAt The timestamp when claimable
     */
    function getNextClaimableWithdrawal(address staker, address validator)
        external
        view
        returns (uint256 amount, uint256 claimableAt)
    {
        WithdrawalQueue memory queue = withdrawalQueues[staker][validator];
        if (queue.head == 0) {
            return (0, 0);
        }

        WithdrawalNode memory node = withdrawalNodes[queue.head];
        return (node.amount, node.claimedAt + withdrawDelay);
    }
}
