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

    /**
     * @notice Represents a single withdrawal as a node in a doubly linked list.
     * @custom:param amount The amount of tokens to withdraw.
     * @custom:param claimableAt The block timestamp when the withdrawal becomes claimable.
     * @custom:param previous The ID of the previous withdrawal in the queue (0 if this node is the head).
     * @custom:param next The ID of the next withdrawal in the queue (0 if this node is the tail).
     * @dev The withdrawal queue for each staker is implemented as a doubly linked list to allow for
     *      efficient claims from queue.
     */
    struct WithdrawalNode {
        uint256 amount;
        uint128 claimableAt;
        uint64 previous;
        uint64 next;
    }

    /**
     * @notice Tracks the head and tail of the withdrawal queue for a staker.
     * @custom:param head The ID of the first withdrawal in the queue (0 if empty).
     * @custom:param tail The ID of the last withdrawal in the queue (0 if empty).
     * @dev This struct points to the start and end of a doubly linked list of WithdrawalNode.
     */
    struct WithdrawalQueue {
        uint64 head;
        uint64 tail;
    }

    /**
     * @notice Represents a pending configuration change proposal.
     * @custom:param value The proposed new value for the configuration parameter.
     * @custom:param executableAt The timestamp when the proposal can be executed (0 if no proposal exists).
     */
    struct ConfigProposal {
        uint128 value;
        uint128 executableAt;
    }

    /**
     * @notice Return type for view functions querying withdrawal info.
     * @custom:param amount The amount of tokens in the withdrawal.
     * @custom:param claimableAt The timestamp when the withdrawal becomes claimable.
     */
    struct WithdrawalInfo {
        uint256 amount;
        uint256 claimableAt;
    }

    // ============================================================
    // STORAGE VARIABLES
    // ============================================================

    /**
     * @notice The SAFE token used for staking.
     */
    IERC20 public immutable SAFE_TOKEN;

    /**
     * @notice Time delay for configuration changes (immutable, set at deployment).
     */
    uint256 public immutable CONFIG_TIME_DELAY;

    /**
     * @notice Global counter for total staked tokens.
     */
    uint256 public totalStakedAmount;

    /**
     * @notice Global counter for total pending withdrawals.
     */
    uint256 public totalPendingWithdrawals;

    /**
     * @notice Withdraw time delay before tokens can be claimed.
     */
    uint128 public withdrawDelay;

    /**
     * @notice Counter for generating unique withdrawal IDs.
     */
    uint64 public nextWithdrawalId;

    // ============================================================
    // MAPPINGS
    // ============================================================

    /**
     * @notice Tracks if an address is a registered validator.
     */
    mapping(address validator => bool isRegistered) public isValidator;

    /**
     * @notice Tracks total stake amount for each validator.
     */
    mapping(address validator => uint256 totalStake) public totalValidatorStakes;

    /**
     * @notice Tracks individual stake amounts: staker => validator => amount.
     */
    mapping(address staker => mapping(address validator => uint256 amount)) public stakes;

    /**
     * @notice Tracks total stake amount for each staker across all validators.
     */
    mapping(address staker => uint256 totalStake) public totalStakerStakes;

    /**
     * @notice Tracks withdrawal queues: staker => queue.
     */
    mapping(address staker => WithdrawalQueue queue) public withdrawalQueues;

    /**
     * @notice Stores all withdrawal nodes by staker -> withdrawal ID.
     */
    mapping(address staker => mapping(uint64 withdrawalId => WithdrawalNode node)) public withdrawalNodes;

    /**
     * @notice Pending proposal for withdraw delay change.
     */
    ConfigProposal public pendingWithdrawDelayChange;

    /**
     * @notice Pending proposal for validator changes.
     */
    bytes32 public pendingValidatorChangeHash;

    // ============================================================
    // EVENTS
    // ============================================================

    // Staking Operations

    /**
     * @notice Emitted when a stake is increased.
     * @param staker The address of the staker.
     * @param validator The validator address the stake is increased toward.
     * @param amount The amount of tokens staked.
     */
    event StakeIncreased(address indexed staker, address indexed validator, uint256 amount);

    /**
     * @notice Emitted when a withdrawal is initiated.
     * @param staker The address of the staker.
     * @param validator The validator address the withdrawal is initiated from.
     * @param withdrawalId The unique ID of the initiated withdrawal.
     * @param amount The amount of tokens to withdraw.
     */
    event WithdrawalInitiated(
        address indexed staker, address indexed validator, uint64 indexed withdrawalId, uint256 amount
    );

    /**
     * @notice Emitted when a withdrawal is claimed after the delay period.
     * @param staker The address of the staker.
     * @param withdrawalId The unique ID of the withdrawal being claimed.
     * @param amount The amount of tokens claimed.
     */
    event WithdrawalClaimed(address indexed staker, uint64 indexed withdrawalId, uint256 amount);

    // Validator Management

    /**
     * @notice Emitted when validator registration/deregistration is proposed.
     * @param validatorsHash The hash of the proposed validators change.
     * @param validator The array of validator addresses.
     * @param isRegistration The array of booleans indicating registration (true) or deregistration (false).
     * @param executableAt The timestamp when the proposal can be executed.
     */
    event ValidatorsProposed(
        bytes32 indexed validatorsHash, address[] validator, bool[] isRegistration, uint256 executableAt
    );

    /**
     * @notice Emitted when a validator is registered or deregistered.
     * @param validator The validator address.
     * @param isRegistered True if registered, false if deregistered.
     */
    event ValidatorUpdated(address indexed validator, bool isRegistered);

    // Configuration Changes

    /**
     * @notice Emitted when a withdraw delay change is proposed.
     * @param currentDelay The current withdraw delay.
     * @param proposedDelay The proposed new withdraw delay.
     * @param executableAt The timestamp when the proposal can be executed.
     */
    event WithdrawDelayProposed(uint256 currentDelay, uint256 proposedDelay, uint256 executableAt);

    /**
     * @notice Emitted when a withdraw delay change is executed.
     * @param oldDelay The old withdraw delay.
     * @param newDelay The new withdraw delay.
     */
    event WithdrawDelayChanged(uint256 oldDelay, uint256 newDelay);

    // Token Recovery

    /**
     * @notice Emitted when tokens are recovered.
     * @param token The token address recovered.
     * @param to The address tokens are sent to.
     * @param amount The amount of tokens recovered.
     */
    event TokensRecovered(address indexed token, address indexed to, uint256 amount);

    // ============================================================
    // ERRORS
    // ============================================================

    /**
     * @notice Thrown when an amount parameter is 0 or invalid.
     */
    error InvalidAmount();

    /**
     * @notice Thrown when an address parameter is the zero address.
     */
    error InvalidAddress();

    /**
     * @notice Thrown when attempting to stake to a non-registered validator.
     */
    error NotValidator();

    /**
     * @notice Thrown when trying to withdraw more than the current stake.
     */
    error InsufficientStake();

    /**
     * @notice Thrown when trying to execute a proposal that hasn't been set.
     */
    error ProposalNotSet();

    /**
     * @notice Thrown when trying to execute a proposal with an invalid hash.
     */
    error InvalidProposalHash();

    /**
     * @notice Thrown when trying to execute a proposal before the timelock expires.
     */
    error ProposalNotExecutable();

    /**
     * @notice Thrown when trying to execute a non-existent proposal.
     */
    error NoProposalExists();

    /**
     * @notice Thrown when trying to recover more tokens than available.
     */
    error InsufficientRecoverableAmount();

    /**
     * @notice Thrown when trying to claim from an empty withdrawal queue.
     */
    error WithdrawalQueueEmpty();

    /**
     * @notice Thrown when trying to claim a withdrawal that isn't ready.
     */
    error NoClaimableWithdrawal();

    /**
     * @notice Thrown when input arrays have mismatched lengths.
     */
    error ArrayLengthMismatch();

    /**
     * @notice Thrown when a parameter is outside acceptable bounds.
     */
    error InvalidParameter();

    /**
     * @notice Thrown when the previous ID doesn't exist.
     */
    error InvalidPreviousId();

    /**
     * @notice Thrown when the specified ordering in the withdrawal queue is invalid.
     */
    error InvalidOrdering();

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    /**
     * @notice Constructs the Staking contract.
     * @param initialOwner The initial owner of the contract.
     * @param safeToken The address of the SAFE token used for staking.
     * @param initialWithdrawDelay The initial withdraw delay in seconds.
     * @param configTimeDelay The time delay for configuration changes in seconds.
     */
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

        emit WithdrawDelayChanged(0, initialWithdrawDelay);
    }

    // ============================================================
    // EXTERNAL FUNCTIONS - STAKING OPERATIONS
    // ============================================================

    /**
     * @notice Stake tokens toward a validator.
     * @param validator The validator address to stake toward.
     * @param amount The amount of tokens to stake.
     */
    function stake(address validator, uint256 amount) external {
        require(amount != 0, InvalidAmount());
        require(validator != address(0), InvalidAddress());
        require(isValidator[validator], NotValidator());

        stakes[msg.sender][validator] += amount;
        totalValidatorStakes[validator] += amount;
        totalStakerStakes[msg.sender] += amount;
        totalStakedAmount += amount;
        emit StakeIncreased(msg.sender, validator, amount);

        SAFE_TOKEN.safeTransferFrom(msg.sender, address(this), amount);
    }

    /**
     * @notice Internal function to handle initiate withdrawal logic.
     * @param user The address initiating the withdrawal.
     * @param amount The amount to withdraw.
     * @param validator The validator address to withdraw from.
     * @return withdrawalId The unique ID of the initiated withdrawal.
     * @return claimableAt The timestamp when the withdrawal becomes claimable.
     */
    function _initiateWithdrawal(address user, uint256 amount, address validator)
        internal
        returns (uint64 withdrawalId, uint128 claimableAt)
    {
        require(amount != 0, InvalidAmount());
        require(stakes[user][validator] >= amount, InsufficientStake());

        // Calculating & casting claimable timestamp.
        claimableAt = uint128(block.timestamp + withdrawDelay);

        stakes[user][validator] -= amount;
        totalValidatorStakes[validator] -= amount;
        totalStakerStakes[user] -= amount;
        totalStakedAmount -= amount;
        totalPendingWithdrawals += amount;

        // Generate new withdrawal ID.
        withdrawalId = nextWithdrawalId++;

        emit WithdrawalInitiated(user, validator, withdrawalId, amount);
    }

    /**
     * @notice Initiate a withdrawal from a validator, automatically inserting it into a sorted queue.
     * @param validator The validator address to withdraw from.
     * @param amount The amount of tokens to withdraw.
     * @dev This function creates a new withdrawal request and inserts it into a doubly linked list that acts as a
     *      queue for the staker. The queue is kept sorted by the `claimableAt` timestamp to ensure withdrawals can be
     *      processed in order. This function handles the sorting on-chain by traversing the list backwards from the
     *      tail to find the correct insertion point. WARNING: This traversal can be gas-intensive if the queue is
     *      long. For a more gas-efficient method, consider `initiateWithdrawalAtPosition`.
     */
    function initiateWithdrawal(address validator, uint256 amount) external {
        (uint64 withdrawalId, uint128 claimableAt) = _initiateWithdrawal(msg.sender, amount, validator);

        // Create a withdrawal node.
        mapping(uint64 => WithdrawalNode) storage stakerNodes = withdrawalNodes[msg.sender];
        stakerNodes[withdrawalId] = WithdrawalNode({amount: amount, claimableAt: claimableAt, previous: 0, next: 0});

        // Add to the withdrawal queue.
        WithdrawalQueue storage queue = withdrawalQueues[msg.sender];
        // If queue is empty, set head and tail to new node.
        if (queue.head == 0) {
            withdrawalQueues[msg.sender] = WithdrawalQueue({head: withdrawalId, tail: withdrawalId});
        } else {
            // Check if the claimableAt of the tail is higher than the claimableAt of the new node. If so, traverse
            // backwards to find the correct position.
            uint64 currentId = queue.tail;
            while (currentId != 0 && stakerNodes[currentId].claimableAt > claimableAt) {
                currentId = stakerNodes[currentId].previous;
            }
            if (currentId == queue.tail) {
                // Higher chances of this happening in most cases, so check first.
                // Insert at tail.
                stakerNodes[withdrawalId].previous = queue.tail;
                stakerNodes[queue.tail].next = withdrawalId;
                queue.tail = withdrawalId;
            } else if (currentId == 0) {
                // Insert at head.
                stakerNodes[withdrawalId].next = queue.head;
                stakerNodes[queue.head].previous = withdrawalId;
                queue.head = withdrawalId;
            } else {
                // Insert in the middle.
                uint64 nextId = stakerNodes[currentId].next;
                stakerNodes[withdrawalId].previous = currentId;
                stakerNodes[withdrawalId].next = nextId;
                stakerNodes[currentId].next = withdrawalId;
                stakerNodes[nextId].previous = withdrawalId;
            }
        }
    }

    /**
     * @notice Initiate a withdrawal at a specific position in the queue, for advanced users.
     * @param validator The validator address to withdraw from.
     * @param amount The amount of tokens to withdraw.
     * @param previousId The ID of the withdrawal node after which to insert the new node (0 to insert at the head).
     * @dev This is a gas-efficient alternative to `initiateWithdrawal`. It allows the caller to specify the exact
     *      insertion point in the doubly linked list. The caller is responsible for providing a `previousId` that
     *      maintains the sorted order of the queue (by `claimableAt` timestamp). The contract performs checks to
     *      prevent out-of-order insertions, but it is less foolproof than the automatic version. Incorrect usage can
     *      lead to transaction reverts.
     */
    function initiateWithdrawalAtPosition(address validator, uint256 amount, uint64 previousId) external {
        (uint64 withdrawalId, uint128 claimableAt) = _initiateWithdrawal(msg.sender, amount, validator);

        uint64 nextId;
        mapping(uint64 => WithdrawalNode) storage stakerNodes = withdrawalNodes[msg.sender];
        // Check if the IDs are correct and claimableAt ordering is correct.
        if (previousId == 0) {
            // Inserting at head - get the current head as nextId.
            nextId = withdrawalQueues[msg.sender].head;
        } else {
            require(stakerNodes[previousId].claimableAt > 0, InvalidPreviousId());
            require(stakerNodes[previousId].claimableAt <= claimableAt, InvalidOrdering());

            nextId = stakerNodes[previousId].next;
        }

        // Validate ordering if queue is not empty.
        if (nextId != 0) {
            require(stakerNodes[nextId].claimableAt >= claimableAt, InvalidOrdering());
        }

        // Create a withdrawal node.
        stakerNodes[withdrawalId] =
            WithdrawalNode({amount: amount, claimableAt: claimableAt, previous: previousId, next: nextId});

        // Update previous and next nodes.
        if (previousId != 0) {
            stakerNodes[previousId].next = withdrawalId;
        } else {
            // Inserting at head.
            withdrawalQueues[msg.sender].head = withdrawalId;
        }

        if (nextId != 0) {
            stakerNodes[nextId].previous = withdrawalId;
        } else {
            // Inserting at tail.
            withdrawalQueues[msg.sender].tail = withdrawalId;
        }
    }

    /**
     * @notice Claim a pending withdrawal after the delay period has passed.
     * @param staker The address that initiated the withdrawal.
     * @dev This function processes the first withdrawal in the queue (the "head" of the linked list). It verifies that
     *      the `withdrawDelay` has passed. Upon success, it removes the withdrawal node from the queue, updates the
     *      queue's head to the next node, and transfers the staked tokens back to the staker.
     */
    function claimWithdrawal(address staker) external {
        uint64 queueHead = withdrawalQueues[staker].head;
        require(queueHead != 0, WithdrawalQueueEmpty());

        WithdrawalNode memory node = withdrawalNodes[staker][queueHead];
        require(block.timestamp >= node.claimableAt, NoClaimableWithdrawal());

        uint256 amount = node.amount;

        if (node.next == 0) {
            // Queue is now empty.
            withdrawalQueues[staker] = WithdrawalQueue({head: 0, tail: 0});
        } else {
            withdrawalQueues[staker].head = node.next;
            withdrawalNodes[staker][node.next].previous = 0;
        }

        delete withdrawalNodes[staker][queueHead];
        totalPendingWithdrawals -= amount;
        emit WithdrawalClaimed(staker, queueHead, amount);

        SAFE_TOKEN.safeTransfer(staker, amount);
    }

    // ============================================================
    // EXTERNAL FUNCTIONS - CONFIGURATION PROPOSALS (OWNER ONLY)
    // ============================================================

    /**
     * @notice Propose a new withdraw delay.
     * @param newDelay The proposed withdraw delay in seconds.
     */
    function proposeWithdrawDelay(uint128 newDelay) external onlyOwner {
        require(newDelay != 0 && newDelay <= CONFIG_TIME_DELAY, InvalidParameter());

        uint128 executableAt = (block.timestamp + CONFIG_TIME_DELAY).toUint128();
        pendingWithdrawDelayChange = ConfigProposal({value: newDelay, executableAt: executableAt});
        emit WithdrawDelayProposed(withdrawDelay, newDelay, executableAt);
    }

    /**
     * @notice Propose validator registration/deregistration changes. This will overwrite the existing pending proposal
     *         for validator changes.
     * @param validators Array of validator addresses.
     * @param isRegistration Array of booleans (true = register, false = deregister).
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

    /**
     * @notice Execute a pending withdraw delay change.
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

    /**
     * @notice Execute pending validator changes.
     * @param validators Array of validator addresses.
     * @param isRegistration Array of booleans representing registration (true) or deregistration (false).
     * @param executableAt The timestamp when the proposal can be executed.
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

    /**
     * @notice Recover accidentally sent tokens.
     * @param token The token address to recover.
     * @param to The address to send recovered tokens to.
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

    /**
     * @notice Get all pending withdrawals for a staker pair.
     * @param staker The staker address.
     * @return An array of withdrawal info.
     */
    function getPendingWithdrawals(address staker) external view returns (WithdrawalInfo[] memory) {
        WithdrawalQueue memory queue = withdrawalQueues[staker];
        if (queue.head == 0) {
            return new WithdrawalInfo[](0);
        }

        // Count the pending withdrawals.
        uint256 count = 0;
        uint64 currentId = queue.head;
        while (currentId != 0) {
            count++;
            currentId = withdrawalNodes[staker][currentId].next;
        }

        // Populate the withdrawals array.
        WithdrawalInfo[] memory withdrawals = new WithdrawalInfo[](count);
        currentId = queue.head;
        for (uint256 i = 0; i < count; i++) {
            WithdrawalNode memory node = withdrawalNodes[staker][currentId];
            withdrawals[i] = WithdrawalInfo({amount: node.amount, claimableAt: node.claimableAt});
            currentId = node.next;
        }

        return withdrawals;
    }

    /**
     * @notice Get the next claimable withdrawal for a staker pair.
     * @param staker The staker address.
     * @return amount The withdrawal amount.
     * @return claimableAt The timestamp when claimable.
     */
    function getNextClaimableWithdrawal(address staker) external view returns (uint256 amount, uint256 claimableAt) {
        WithdrawalQueue memory queue = withdrawalQueues[staker];
        if (queue.head == 0) {
            return (0, 0);
        }

        WithdrawalNode memory node = withdrawalNodes[staker][queue.head];
        return (node.amount, node.claimableAt);
    }

    // ============================================================
    // INTERNAL HELPER FUNCTIONS
    // ============================================================

    /**
     * @notice Computes the hash of the validators for a configuration change.
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
