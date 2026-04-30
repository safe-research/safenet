// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {ConsensusMessages} from "@/libraries/ConsensusMessages.sol";
import {FROST} from "@/libraries/FROST.sol";
import {SafeTransaction} from "@/libraries/SafeTransaction.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";
import {Enum} from "@safe/interfaces/Enum.sol";
import {BaseTransactionGuard} from "@safe/base/GuardManager.sol";
import {ISafe} from "@safe/interfaces/ISafe.sol";

/**
 * @title SafenetGuardA
 * @notice Safe Transaction Guard that gates every Safe transaction behind Safenet
 *         threshold-signature attestation.
 *
 * @dev ## Security model
 *
 *      The Safenet Consensus contract lives exclusively on Gnosis Chain. Because cross-chain
 *      calls are not feasible, this guard maintains its own local consensus state.
 *
 *      **Epoch history.** Group keys are stored in `$epochGroupKeys`, a mapping from epoch number
 *      to FROST group public key. Each `updateEpoch` call adds the new epoch's key to the mapping,
 *      so all historic keys remain available for signature verification. `$activeEpoch` tracks the
 *      current epoch number; keys for all past epochs remain resolvable via `_resolveGroupKey`.
 *
 *      **Inline attestation.** FROST attestations are passed as a trailer appended to Safe's
 *      `signatures` bytes: `[safe signatures][abi.encode(epoch, FROST.Signature)][length: uint256]`.
 *      `checkTransaction` decodes and verifies the attestation atomically during execution.
 *      The Safe nonce makes every `safeTxHash` unique, preventing replay without an explicit
 *      spent-signature registry.
 *
 *      **Escape hatch.** Safe owners can register a specific transaction for time-delayed
 *      execution via `allowTransaction`. The guard auto-allows calls to itself for this selector
 *      and for `cancelAllowTransaction`, requiring only the Safe's own threshold signature
 *      rather than a Safenet attestation. `DELEGATECALL` to the guard is never auto-allowed:
 *      it would execute guard functions in the Safe's storage context.
 */
contract SafenetGuardA is BaseTransactionGuard {
    // ============================================================
    // STORAGE VARIABLES
    // ============================================================

    /**
     * @notice EIP-712 domain separator binding this guard to a specific Consensus deployment.
     * @dev Computed at construction as `ConsensusMessages.domain(consensusChainId, consensusAddress)`.
     *      Any mismatch between this value and the domain separator used by the Consensus contract
     *      when producing attestation signatures makes all FROST verifications permanently fail.
     *      There is no recovery path short of redeploying the guard.
     */
    bytes32 private immutable _CONSENSUS_DOMAIN_SEPARATOR;

    /**
     * @notice Seconds added to `block.timestamp` when registering a time-delayed allowance.
     * @dev The security of the escape hatch depends on this value being large enough for Safe
     *      owners to detect and cancel a mistakenly registered hash before it becomes executable.
     */
    uint256 private immutable _ALLOW_TX_DELAY;

    /**
     * @notice The currently active epoch number.
     * @dev Set at construction and advanced on every `updateEpoch` call.
     */
    // forge-lint: disable-next-line(mixed-case-variable)
    uint64 private $activeEpoch;

    /**
     * @notice Maps each epoch number to its FROST group public key.
     * @dev Written at construction for `initialEpoch` and on every `updateEpoch` call for the new
     *      epoch. Once written, an entry is never overwritten — epoch numbers are strictly increasing
     *      so each epoch gets exactly one key. A zero point (`x == 0 && y == 0`) indicates the
     *      epoch was never stored and is used as the sentinel by `_resolveGroupKey`.
     *      All stored keys are guaranteed non-zero because both the constructor and `updateEpoch`
     *      call `Secp256k1.requireNonZero` before writing.
     */
    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(uint64 epoch => Secp256k1.Point groupKey) private $epochGroupKeys;

    /**
     * @notice Time-delayed execution allowances for the escape hatch, keyed by Safe address.
     * @dev A non-zero value is the earliest Unix timestamp at which the corresponding transaction
     *      may execute without a Safenet attestation. Written by `allowTransaction`, consumed and
     *      deleted on use by `checkTransaction`, and deleted early by `cancelAllowTransaction`.
     *      Keying by `msg.sender` (the Safe) ensures no Safe can interfere with another's allowances.
     */
    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(address safe => mapping(bytes32 safeTxHash => uint256 executableAt)) private $allowedTransactions;

    // ============================================================
    // EVENTS
    // ============================================================

    /**
     * @notice Emitted when the guard's active epoch advances via `updateEpoch`.
     * @param previousEpoch The epoch number that was active before this transition.
     * @param activeEpoch   The new active epoch number.
     * @param activeGroupKey The FROST group public key for the new active epoch.
     */
    event EpochUpdated(uint64 indexed previousEpoch, uint64 indexed activeEpoch, Secp256k1.Point activeGroupKey);

    /**
     * @notice Emitted when a Safe registers a transaction for time-delayed execution.
     * @param safe         The Safe that registered the allowance.
     * @param safeTxHash   The hash of the transaction being allowed.
     * @param executableAt The earliest Unix timestamp at which the transaction may execute.
     */
    event TransactionAllowed(address indexed safe, bytes32 indexed safeTxHash, uint256 executableAt);

    /**
     * @notice Emitted when a pending time-delayed allowance is cancelled.
     * @param safe       The Safe that cancelled the allowance.
     * @param safeTxHash The hash whose allowance was removed.
     */
    event AllowanceCancelled(address indexed safe, bytes32 indexed safeTxHash);

    /**
     * @notice Emitted when a transaction executes through the time-delayed escape hatch.
     * @param safe       The Safe that executed the transaction.
     * @param safeTxHash The hash of the transaction that executed.
     */
    event TransactionExecutedViaAllowance(address indexed safe, bytes32 indexed safeTxHash);

    // ============================================================
    // ERRORS
    // ============================================================

    /**
     * @notice Thrown by `checkTransaction` when neither a valid inline attestation nor a matured
     *         time-delayed allowance exists for the transaction.
     */
    error AttestationNotFound();

    /**
     * @notice Thrown by `_resolveGroupKey` when the requested epoch has no group key stored in
     *         `$epochGroupKeys` — i.e., the epoch was never activated via `updateEpoch` or set
     *         as the initial epoch in the constructor.
     */
    error InvalidEpoch();

    /**
     * @notice Thrown by `updateEpoch` when `proposedEpoch` is not strictly greater than the
     *         current active epoch. Prevents epoch replay and backwards transitions.
     */
    error EpochNotAdvancing();

    /**
     * @notice Thrown by `allowTransaction` when an allowance already exists for this Safe and
     *         `safeTxHash`, preventing accidental overwrite of an existing delay timestamp.
     */
    error TransactionAlreadyAllowed();

    /**
     * @notice Thrown by `cancelAllowTransaction` when no allowance exists for this Safe and
     *         `safeTxHash`.
     */
    error AllowanceNotFound();

    /**
     * @notice Thrown by the constructor when `consensusAddress` is the zero address.
     */
    error InvalidAddress();

    /**
     * @notice Thrown by the constructor when `allowTransactionDelay` is zero.
     */
    error InvalidParameter();

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    /**
     * @notice Deploys SafenetGuard with an initial epoch state derived from the Safenet genesis.
     * @dev The domain separator is computed once and stored immutably. An incorrect
     *      `consensusChainId` or `consensusAddress` cannot be corrected after deployment —
     *      all subsequent FROST verifications would fail against the wrong domain.
     * @param consensusChainId  Chain ID of the network hosting the Consensus contract (100 for
     *                          Gnosis Chain). Embedded in the domain separator to bind signatures
     *                          to that specific deployment.
     * @param consensusAddress  Address of the Consensus contract on the consensus chain. Must be
     *                          non-zero; combined with `consensusChainId` to derive the domain
     *                          separator.
     * @param initialEpoch      Epoch number active at deployment time on Consensus. Zero is
     *                          technically valid but means epoch 0 attestations will be accepted.
     * @param initialGroupKey   FROST group public key for `initialEpoch`. Must be a valid non-zero
     *                          point on secp256k1 — required to verify the first rollover signature.
     * @param allowTransactionDelay Seconds that must elapse after `allowTransaction` before the
     *                          registered hash may execute without attestation. Must be non-zero;
     *                          the security of the escape hatch depends on this delay being long
     *                          enough for owners to cancel a mistakenly registered hash.
     */
    constructor(
        uint256 consensusChainId,
        address consensusAddress,
        uint64 initialEpoch,
        Secp256k1.Point memory initialGroupKey,
        uint256 allowTransactionDelay
    ) {
        require(consensusAddress != address(0), InvalidAddress());
        require(allowTransactionDelay != 0, InvalidParameter());
        Secp256k1.requireNonZero(initialGroupKey);
        _CONSENSUS_DOMAIN_SEPARATOR = ConsensusMessages.domain(consensusChainId, consensusAddress);
        _ALLOW_TX_DELAY = allowTransactionDelay;
        $activeEpoch = initialEpoch;
        $epochGroupKeys[initialEpoch] = initialGroupKey;
        emit EpochUpdated(0, initialEpoch, initialGroupKey);
    }

    // ============================================================
    // EPOCH MANAGEMENT
    // ============================================================

    /**
     * @notice Advances the guard's epoch state using a FROST-signed rollover from the current
     *         active validator group.
     * @dev Permissionless — any party holding the rollover signature may call this (validators,
     *      relayers, Safe owners). Multi-epoch jumps are permitted: validators may sign a rollover
     *      that skips intermediate epoch numbers.
     *      To catch up multiple epochs in sequence, batch calls in ascending order via
     *      MultiSendCallOnly.
     *      `rolloverBlock` is a Gnosis Chain block number embedded in the signed message; no check
     *      against `block.number` is performed because the local chain's block number is unrelated.
     *      The new epoch's group key is written to `$epochGroupKeys[proposedEpoch]` and
     *      `$activeEpoch` is advanced. All previously stored keys remain in the mapping.
     * @param proposedEpoch  New epoch number. Must be strictly greater than the active epoch;
     *                       equal or lower values revert with `EpochNotAdvancing`.
     * @param rolloverBlock  Gnosis Chain block number from the epoch rollover message. Required
     *                       to reproduce the exact message that the validators signed.
     * @param newGroupKey    FROST group public key for `proposedEpoch`. Must be a valid non-zero
     *                       secp256k1 point — it will verify all attestations and the next rollover.
     * @param signature      FROST threshold signature from the current active group authorising
     *                       the transition.
     */
    function updateEpoch(
        uint64 proposedEpoch,
        uint64 rolloverBlock,
        Secp256k1.Point calldata newGroupKey,
        FROST.Signature calldata signature
    ) external {
        require(proposedEpoch > $activeEpoch, EpochNotAdvancing());
        Secp256k1.requireNonZero(newGroupKey);
        bytes32 message = ConsensusMessages.epochRollover(
            _CONSENSUS_DOMAIN_SEPARATOR, $activeEpoch, proposedEpoch, rolloverBlock, newGroupKey
        );
        FROST.verify($epochGroupKeys[$activeEpoch], signature, message);
        uint64 prevEpoch = $activeEpoch;
        $activeEpoch = proposedEpoch;
        $epochGroupKeys[proposedEpoch] = newGroupKey;
        emit EpochUpdated(prevEpoch, proposedEpoch, newGroupKey);
    }

    // ============================================================
    // ALLOWANCE (ESCAPE HATCH)
    // ============================================================

    /**
     * @notice Registers a transaction for time-delayed execution without Safenet attestation.
     * @dev Must be called by the Safe itself via `execTransaction`. The guard auto-allows this
     *      selector, so no Safenet attestation is needed for the registration call itself —
     *      authentication is provided by the Safe's own threshold signature mechanism.
     *      After `_ALLOW_TX_DELAY` seconds, the registered hash may execute without attestation
     *      via `checkTransaction`.
     * @param safeTxHash Hash of the transaction to allow. Reverts `TransactionAlreadyAllowed`
     *                   if an allowance is already pending for this Safe and hash.
     */
    function allowTransaction(bytes32 safeTxHash) external {
        require($allowedTransactions[msg.sender][safeTxHash] == 0, TransactionAlreadyAllowed());
        uint256 executableAt = block.timestamp + _ALLOW_TX_DELAY;
        $allowedTransactions[msg.sender][safeTxHash] = executableAt;
        emit TransactionAllowed(msg.sender, safeTxHash, executableAt);
    }

    /**
     * @notice Cancels a pending time-delayed allowance registered by this Safe.
     * @dev Must be called by the Safe itself via `execTransaction`. Also auto-allowed by the guard.
     *      Burning the nonce by executing any other attested transaction at the same nonce also
     *      invalidates the registered hash implicitly.
     * @param safeTxHash Hash whose allowance should be cancelled. Reverts `AllowanceNotFound` if
     *                   no allowance exists for this Safe and hash.
     */
    function cancelAllowTransaction(bytes32 safeTxHash) external {
        require($allowedTransactions[msg.sender][safeTxHash] != 0, AllowanceNotFound());
        delete $allowedTransactions[msg.sender][safeTxHash];
        emit AllowanceCancelled(msg.sender, safeTxHash);
    }

    // ============================================================
    // SAFE TRANSACTION GUARD
    // ============================================================

    /**
     * @notice Pre-execution hook called by Safe's GuardManager for every owner-signed transaction.
     * @dev Execution is permitted if any of the following conditions holds (checked in order):
     *      1. Auto-allow: the call targets this guard with a whitelisted selector
     *         (`allowTransaction`, `cancelAllowTransaction`), zero value, and `CALL` operation.
     *      2. The `signatures` bytes contain a valid inline FROST attestation trailer
     *         (`[abi.encode(epoch, FROST.Signature)][length: uint256]`) — decoded and verified
     *         atomically against the stored group key for `epoch`.
     *      3. A time-delayed allowance exists and its delay has elapsed — the entry is deleted.
     *      Safe increments its nonce before invoking this hook, so the pre-execution nonce used
     *      when the attestation was produced is `ISafe(msg.sender).nonce() - 1`.
     *      Reverts `AttestationNotFound` if none of the above conditions hold.
     */
    function checkTransaction(
        address to,
        uint256 value,
        bytes memory data,
        Enum.Operation operation,
        uint256 safeTxGas,
        uint256 baseGas,
        uint256 gasPrice,
        address gasToken,
        address payable refundReceiver,
        bytes calldata signatures,
        address /* msgSender */
    ) external override {
        if (_isAutoAllowed(to, value, data, operation)) return;

        // Safe increments its nonce before calling checkTransaction. The hash was computed with
        // the pre-increment nonce, so we subtract 1.
        uint256 nonce = ISafe(payable(msg.sender)).nonce() - 1;
        bytes32 safeTxHash = SafeTransaction.hash(
            SafeTransaction.T({
                chainId: block.chainid,
                safe: msg.sender,
                to: to,
                value: value,
                data: data,
                operation: SafeTransaction.Operation(uint8(operation)),
                safeTxGas: safeTxGas,
                baseGas: baseGas,
                gasPrice: gasPrice,
                gasToken: gasToken,
                refundReceiver: address(refundReceiver),
                nonce: nonce
            })
        );

        bytes calldata attestation = _decodeAttestation(signatures);
        if (attestation.length > 0) {
            (uint64 epoch, FROST.Signature memory signature) = abi.decode(attestation, (uint64, FROST.Signature));
            Secp256k1.Point memory groupKey = _resolveGroupKey(epoch);
            bytes32 message = ConsensusMessages.transactionProposal(_CONSENSUS_DOMAIN_SEPARATOR, epoch, safeTxHash);
            FROST.verify(groupKey, signature, message);
            return;
        }

        uint256 executableAt = $allowedTransactions[msg.sender][safeTxHash];
        if (executableAt != 0 && block.timestamp >= executableAt) {
            delete $allowedTransactions[msg.sender][safeTxHash];
            emit TransactionExecutedViaAllowance(msg.sender, safeTxHash);
            return;
        }

        revert AttestationNotFound();
    }

    /**
     * @dev Decodes the attestation from the provided safe tx signature.
     */
    function _decodeAttestation(bytes calldata signatures) internal pure virtual returns (bytes calldata) {
        if (signatures.length < 66) {
            return _emptyContext();
        }

        uint256 end = signatures.length - 32;
        uint256 length = uint256(bytes32(signatures[end:]));
        if (length > end) {
            return _emptyContext();
        }

        return signatures[end - length:end];
    }

    /**
     * @dev Returns an empty calldata slice. Used as a typed zero-value for `bytes calldata`.
     */
    function _emptyContext() internal pure returns (bytes calldata) {
        return msg.data[0:0];
    }

    /**
     * @notice Post-execution hook called by Safe's GuardManager after every owner-signed transaction.
     * @dev Intentionally empty. Reserved for future post-execution logic.
     */
    function checkAfterExecution(bytes32, bool) external pure override {}

    // ============================================================
    // VIEW FUNCTIONS
    // ============================================================

    /**
     * @notice Returns the epoch number of the current active epoch.
     */
    function activeEpoch() external view returns (uint64) {
        return $activeEpoch;
    }

    /**
     * @notice Returns the minimum delay (in seconds) for the time-delayed escape hatch.
     */
    function allowTxDelay() external view returns (uint256) {
        return _ALLOW_TX_DELAY;
    }

    /**
     * @notice Returns the EIP-712 domain separator used to reconstruct Consensus messages.
     */
    function consensusDomainSeparator() external view returns (bytes32) {
        return _CONSENSUS_DOMAIN_SEPARATOR;
    }

    /**
     * @notice Returns the earliest Unix timestamp at which a pending allowance may execute,
     *         or zero if no allowance exists for the given Safe and hash.
     */
    function getAllowedTxTimestamp(address safe, bytes32 safeTxHash) external view returns (uint256 executableAt) {
        return $allowedTransactions[safe][safeTxHash];
    }

    // ============================================================
    // PRIVATE HELPERS
    // ============================================================

    /**
     * @dev Returns the group key for `epoch` from `$epochGroupKeys`.
     *      Reverts `InvalidEpoch` if no key was ever stored for this epoch.
     *      A zero point is treated as "not stored" — guaranteed safe because the constructor
     *      and `updateEpoch` both call `Secp256k1.requireNonZero` before writing.
     */
    function _resolveGroupKey(uint64 epoch) private view returns (Secp256k1.Point memory key) {
        key = $epochGroupKeys[epoch];
        if (key.x == 0 && key.y == 0) revert InvalidEpoch();
    }

    /**
     * @dev Returns `true` if the call should bypass attestation checks.
     *      Requires the target to be this contract, value to be zero, data to contain at least
     *      a 4-byte selector matching a whitelisted escape-hatch function, and operation to be
     *      `CALL`. `DELEGATECALL` is explicitly excluded: it would execute these functions in
     *      the Safe's storage context, corrupting Safe state.
     */
    function _isAutoAllowed(address to, uint256 value, bytes memory data, Enum.Operation operation)
        private
        view
        returns (bool)
    {
        if (to != address(this)) return false;
        if (value != 0) return false;
        if (data.length < 4) return false;
        if (operation != Enum.Operation.Call) return false;
        // forge-lint: disable-next-line(unsafe-typecast)
        bytes4 selector = bytes4(data);
        return selector == SafenetGuardA.allowTransaction.selector
            || selector == SafenetGuardA.cancelAllowTransaction.selector;
    }
}
