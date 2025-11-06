// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Ownable} from "@oz/access/Ownable.sol";
import {IERC20} from "@oz/token/ERC20/IERC20.sol";
import {SafeERC20} from "@oz/token/ERC20/utils/SafeERC20.sol";
import {SafeCast} from "@oz/utils/math/SafeCast.sol";

contract Staking is Ownable {
    using SafeCast for uint256;
    using SafeERC20 for IERC20;

    // ============================================================
    // STRUCTS
    // ============================================================

    /*
     * @notice Represents a single withdrawal in the queue
     */
    struct WithdrawalNode {
        uint256 amount;
        uint128 claimableAt;
        uint64 previous; // ID of previous withdrawal, 0 if first
        uint64 next; // ID of next withdrawal, 0 if last
    }

    /*
     * @notice Tracks the withdrawal queue for a staker-validator pair
     */
    struct WithdrawalQueue {
        uint64 head; // ID of first withdrawal in queue, 0 if empty
        uint64 tail; // ID of last withdrawal in queue, 0 if empty
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
        uint256 claimableAt;
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
    uint64 public nextWithdrawalId;

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
    mapping(uint64 withdrawalId => WithdrawalNode node) public withdrawalNodes;

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
    event WithdrawalInitiated(
        address indexed staker, address indexed validator, uint64 indexed withdrawalId, uint256 amount
    );
    event WithdrawalClaimed(address indexed staker, address indexed validator, uint256 amount);

    // Validator Management
    event ValidatorsProposed(
        bytes32 indexed validatorsHash, address[] validator, bool[] isRegistration, uint256 executableAt
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

    /*
     * @notice Thrown when the specified ordering in the withdrawal queue is invalid
     */
    error InvalidOrdering();

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    constructor(address initialOwner, address safeToken, uint128 initialWithdrawDelay, uint256 configTimeDelay)
        Ownable(initialOwner)
    {
        require(safeToken != address(0), InvalidAddress());
        require(initialWithdrawDelay != 0, InvalidParameter());
        require(configTimeDelay != 0, InvalidParameter());
        require(initialWithdrawDelay <= configTimeDelay, InvalidParameter());

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
        require(amount != 0, InvalidAmount());
        require(validator != address(0), InvalidAddress());
        require(isValidator[validator], NotValidator());

        stakes[msg.sender][validator] += amount;
        totalStakes[validator] += amount;
        totalStakedAmount += amount;
        emit StakeIncreased(msg.sender, validator, amount);

        SAFE_TOKEN.safeTransferFrom(msg.sender, address(this), amount);
    }

    /*
     * @notice Internal function to handle initial withdrawal logic
     * @param user The address initiating the withdrawal
     * @param amount The amount to withdraw
     * @param validator The validator address to withdraw from
     * @return withdrawalId The unique ID of the initiated withdrawal
     * @return claimableAt The timestamp when the withdrawal becomes claimable
     */
    function _initialWithdrawal(address user, uint256 amount, address validator)
        internal
        returns (uint64 withdrawalId, uint128 claimableAt)
    {
        require(amount != 0, InvalidAmount());
        require(stakes[user][validator] >= amount, InsufficientStake());

        // Calculating & casting claimable timestamp
        claimableAt = uint128(block.timestamp + withdrawDelay);

        stakes[user][validator] -= amount;
        totalStakes[validator] -= amount;
        totalStakedAmount -= amount;
        totalPendingWithdrawals += amount;

        // Generate new withdrawal ID
        withdrawalId = nextWithdrawalId++;

        emit WithdrawalInitiated(user, validator, withdrawalId, amount);
    }

    /*
     * @notice Initiate a withdrawal from a validator
     * @param validator The validator address to withdraw from
     * @param amount The amount of tokens to withdraw
     * @dev   This function should be used in the interface for normal withdrawals. It inserts the new withdrawal
     *        into the queue in the correct position based on the claimableAt timestamp.
     */
    function initiateWithdrawal(address validator, uint256 amount) external {
        (uint64 withdrawalId, uint128 claimableAt) = _initialWithdrawal(msg.sender, amount, validator);

        // Create node
        withdrawalNodes[withdrawalId] = WithdrawalNode({amount: amount, claimableAt: claimableAt, previous: 0, next: 0});

        // Add to queue
        WithdrawalQueue storage queue = withdrawalQueues[msg.sender][validator];
        // If queue is empty, set head and tail to new node
        if (queue.head == 0) {
            withdrawalQueues[msg.sender][validator] = WithdrawalQueue({head: withdrawalId, tail: withdrawalId});
        } else {
            // Check if the claimableAt of the tail is higher than the claimableAt of the new node
            // If so, traverse backwards to find the correct position
            uint64 currentId = queue.tail;
            while (currentId != 0 && withdrawalNodes[currentId].claimableAt > claimableAt) {
                currentId = withdrawalNodes[currentId].previous;
            }
            if (currentId == queue.tail) {
                // Higher chances of this happening in most cases, so check first
                // Insert at tail
                withdrawalNodes[withdrawalId].previous = queue.tail;
                withdrawalNodes[queue.tail].next = withdrawalId;
                queue.tail = withdrawalId;
            } else if (currentId == 0) {
                // Insert at head
                withdrawalNodes[withdrawalId].next = queue.head;
                withdrawalNodes[queue.head].previous = withdrawalId;
                queue.head = withdrawalId;
            } else {
                // Insert in the middle
                uint64 nextId = withdrawalNodes[currentId].next;
                withdrawalNodes[withdrawalId].previous = currentId;
                withdrawalNodes[withdrawalId].next = nextId;
                withdrawalNodes[currentId].next = withdrawalId;
                withdrawalNodes[nextId].previous = withdrawalId;
            }
        }

        emit WithdrawalInitiated(msg.sender, validator, withdrawalId, amount);
    }

    /*
     * @notice Initiate a withdrawal from a validator at a specific position in the queue
     * @param validator The validator address to withdraw from
     * @param amount The amount of tokens to withdraw
     * @param previousId The ID of the previous withdrawal in the queue (0 if inserting at head)
     * @dev   This function allows users to specify the position of their withdrawal in the queue.
     *        It is the caller's responsibility to ensure the correct ordering based on claimableAt timestamps.
     *        This is an advanced function and should be used with caution.
     */
    function initiateWithdrawalAtPosition(address validator, uint256 amount, uint64 previousId) external {
        (uint64 withdrawalId, uint128 claimableAt) = _initialWithdrawal(msg.sender, amount, validator);

        uint64 nextId;
        // Check if the Id's are correct and claimableAt ordering is correct
        if (previousId == 0) {
            // Inserting at head - get the current head as nextId
            nextId = withdrawalQueues[msg.sender][validator].head;
        } else {
            require(withdrawalNodes[previousId].claimableAt <= claimableAt, InvalidOrdering());

            nextId = withdrawalNodes[previousId].next;
        }

        // Validate ordering if queue is not empty
        if (nextId != 0) {
            require(withdrawalNodes[nextId].claimableAt >= claimableAt, InvalidOrdering());
        }

        // Create node
        withdrawalNodes[withdrawalId] =
            WithdrawalNode({amount: amount, claimableAt: claimableAt, previous: previousId, next: nextId});

        // Update previous and next nodes
        if (previousId != 0) {
            withdrawalNodes[previousId].next = withdrawalId;
        } else {
            // Inserting at head
            withdrawalQueues[msg.sender][validator].head = withdrawalId;
        }

        if (nextId != 0) {
            withdrawalNodes[nextId].previous = withdrawalId;
        } else {
            // Inserting at tail
            withdrawalQueues[msg.sender][validator].tail = withdrawalId;
        }
    }

    /*
     * @notice Claim a pending withdrawal after the delay period
     * @param staker The address that initiated the withdrawal
     * @param validator The validator address to claim from
     */
    function claimWithdrawal(address staker, address validator) external {
        WithdrawalQueue memory queue = withdrawalQueues[staker][validator];
        require(queue.head != 0, WithdrawalQueueEmpty());

        WithdrawalNode memory node = withdrawalNodes[queue.head];
        require(block.timestamp >= node.claimableAt, NoClaimableWithdrawal());

        uint256 amount = node.amount;

        if (node.next == 0) {
            // Queue is now empty
            withdrawalQueues[staker][validator] = WithdrawalQueue({head: 0, tail: 0});
        } else {
            withdrawalQueues[staker][validator].head = node.next;
            withdrawalNodes[node.next].previous = 0;
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
        require(newDelay != 0 && newDelay <= CONFIG_TIME_DELAY, InvalidParameter());

        uint128 executableAt = (block.timestamp + CONFIG_TIME_DELAY).toUint128();
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
        require(validators.length != 0, InvalidParameter());
        require(validators.length == isRegistration.length, ArrayLengthMismatch());

        uint256 executableAt = block.timestamp + CONFIG_TIME_DELAY;
        bytes32 validatorsHash = _getValidatorsHash(validators, isRegistration, executableAt);
        for (uint256 i = 0; i < validators.length; i++) {
            require(validators[i] != address(0), InvalidAddress());
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
        require(proposal.executableAt != 0, NoProposalExists());
        require(block.timestamp >= proposal.executableAt, ProposalNotExecutable());

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
        require(proposalHash != bytes32(0), NoProposalExists());

        bytes32 validatorsHash = _getValidatorsHash(validators, isRegistration, executableAt);
        require(proposalHash == validatorsHash, InvalidProposalHash());
        require(executableAt != 0, ProposalNotSet());
        require(block.timestamp >= executableAt, ProposalNotExecutable());

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
        require(to != address(0), InvalidAddress());

        uint256 recoverable;
        if (token == address(SAFE_TOKEN)) {
            uint256 balance = SAFE_TOKEN.balanceOf(address(this));
            recoverable = balance - totalStakedAmount - totalPendingWithdrawals;
        } else {
            recoverable = IERC20(token).balanceOf(address(this));
        }

        require(recoverable != 0, InsufficientRecoverableAmount());

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
        uint64 currentId = queue.head;
        while (currentId != 0) {
            count++;
            currentId = withdrawalNodes[currentId].next;
        }

        // Populate array
        WithdrawalInfo[] memory withdrawals = new WithdrawalInfo[](count);
        currentId = queue.head;
        for (uint256 i = 0; i < count; i++) {
            WithdrawalNode memory node = withdrawalNodes[currentId];
            withdrawals[i] = WithdrawalInfo({amount: node.amount, claimableAt: node.claimableAt});
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
        return (node.amount, node.claimableAt);
    }

    // ============================================================
    // INTERNAL HELPER FUNCTIONS
    // ============================================================

    /*
     * @notice Compute the hash of the validators for a configuration change.
     * @param validators The validators affected by the configuration change.
     * @param isRegistration Whether or not the validator should be registered or unregistered.
     * @param executableAt The timestamp once the validator change can be executed.
     * @return validatorsHash The digest for the validators configuration change.
     */
    function _getValidatorsHash(address[] calldata validators, bool[] calldata isRegistration, uint256 executableAt)
        internal
        pure
        returns (bytes32 validatorsHash)
    {
        // forge-lint: disable-next-line(asm-keccak256)
        return keccak256(abi.encode(validators, isRegistration, executableAt));
    }
}
