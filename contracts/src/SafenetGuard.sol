// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {ConsensusMessages} from "@/libraries/ConsensusMessages.sol";
import {FROST} from "@/libraries/FROST.sol";
import {SafeTransaction} from "@/libraries/SafeTransaction.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";
import {Enum} from "@safe/interfaces/Enum.sol";
import {BaseGuard} from "@safe/examples/guards/BaseGuard.sol";
import {ISafe} from "@safe/interfaces/ISafe.sol";

/**
 * @title SafenetGuard
 * @notice Safe Transaction Guard and Module Guard that gates every Safe transaction behind
 *         Safenet threshold-signature attestation.
 *
 * @dev ## Security model
 *
 *      The Safenet Consensus contract lives exclusively on Gnosis Chain. Because cross-chain
 *      calls are not feasible, this guard maintains its own local consensus state and accepts
 *      pre-submitted FROST attestations delivered by validators or relayers before a transaction
 *      executes.
 *
 *      **Epoch pair.** Two `EpochState` entries are kept: `_currentEpoch` and `_previousEpoch`.
 *      Attestations are accepted for either epoch, allowing signing ceremonies that straddle a
 *      rollover boundary to be submitted after the epoch has advanced. `_resolveGroupKey` checks
 *      the current epoch first and falls back to the previous one when present.
 *
 *      **Unified attestation.** Both regular and module transactions use `submitAttestation`.
 *      Each call stores `sigId = keccak256(sig.r.x, sig.r.y, sig.z)` against the `safeTxHash`.
 *      The sigId lifecycle differs by execution path:
 *      - Regular transactions: `checkTransaction` deletes the `_attestations` entry on execution
 *        and does NOT write `_usedModuleSigs`. The Safe nonce makes every regular `safeTxHash`
 *        unique, so deleting the entry is sufficient to prevent replay.
 *      - Module transactions: `checkModuleTransaction` deletes the `_attestations` entry AND
 *        permanently marks the sigId in `_usedModuleSigs`. Module transactions carry no nonce,
 *        so the same `safeTxHash` can recur; spending the sigId prevents the same signing
 *        ceremony from unlocking multiple executions. `submitAttestation` also reads
 *        `_usedModuleSigs` to block resubmission of a spent ceremony for any hash.
 *
 *      **Escape hatch.** Safe owners can register a specific transaction for time-delayed
 *      execution via `allowTransaction`. The guard auto-allows calls to itself for this selector
 *      and for `cancelAllowTransaction` and `cancelModuleAttestation`, requiring only the Safe's
 *      own threshold signature rather than a Safenet attestation. `DELEGATECALL` to the guard is
 *      never auto-allowed: it would execute guard functions in the Safe's storage context.
 */
contract SafenetGuard is BaseGuard {
    // ============================================================
    // TYPES
    // ============================================================

    /**
     * @notice Pairs an epoch number with the FROST group public key active during that epoch.
     * @custom:member epoch    The epoch number assigned by the Consensus contract.
     * @custom:member groupKey The FROST group public key used to verify signatures for this epoch.
     */
    struct EpochState {
        uint64 epoch;
        Secp256k1.Point groupKey;
    }

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
     * @notice The most recently activated epoch: its number and FROST group key.
     * @dev Set at construction and overwritten on every `updateEpoch` call.
     *      Always holds a valid entry — there is no uninitialised state.
     */
    EpochState private _currentEpoch;

    /**
     * @notice The epoch immediately preceding `_currentEpoch`.
     * @dev Only valid when `_hasPreviousEpoch` is true (after the first `updateEpoch` call).
     *      Retaining the previous epoch allows attestations produced during a signing ceremony
     *      that straddles a rollover boundary to be submitted after the epoch has advanced.
     */
    EpochState private _previousEpoch;

    /**
     * @notice True once the first `updateEpoch` call has populated `_previousEpoch`.
     * @dev Guards `previousEpoch()` and `_resolveGroupKey` against treating the zero-initialised
     *      `_previousEpoch` slot as a valid epoch entry before any rollover has occurred.
     *      Without this flag, `_resolveGroupKey(0)` would match the zero-initialised
     *      `_previousEpoch.epoch` and return a zero group key. FROST verification against a zero
     *      key is trivially forgeable (z·G == R holds for any z when Y = 0), so an attacker could
     *      register a fake attestation for any transaction without a real signing ceremony.
     */
    bool private _hasPreviousEpoch;

    /**
     * @notice Pending attestation registry shared by regular and module transactions.
     * @dev A non-zero value indicates a valid FROST attestation has been submitted for that hash
     *      and not yet consumed by execution. The stored value is the sigId of the signing ceremony.
     *      Written exclusively by `submitAttestation`. Cleared by `checkTransaction`,
     *      `checkModuleTransaction`, and `cancelModuleAttestation`. `bytes32(0)` is the sentinel
     *      for "no pending attestation".
     */
    mapping(bytes32 safeTxHash => bytes32 sigId) private _attestations;

    /**
     * @notice Registry of FROST signing ceremonies permanently spent by module executions.
     * @dev Written exclusively by `checkModuleTransaction` when it consumes a module attestation.
     *      Regular transaction executions never write here — the Safe nonce makes each regular
     *      `safeTxHash` unique, so deleting the `_attestations` entry is sufficient.
     *      `submitAttestation` reads this mapping to prevent a ceremony spent by a module
     *      execution from being resubmitted as a fresh pending attestation for any `safeTxHash`.
     *      Entries are permanent; once `true`, a sigId can never again unlock a module execution.
     */
    mapping(bytes32 sigId => bool spent) private _usedModuleSigs;

    /**
     * @notice Time-delayed execution allowances for the escape hatch, keyed by Safe address.
     * @dev A non-zero value is the earliest Unix timestamp at which the corresponding transaction
     *      may execute without a Safenet attestation. Written by `allowTransaction`, consumed and
     *      deleted on use by `checkTransaction` or `checkModuleTransaction`, and deleted early by
     *      `cancelAllowTransaction`. Keying by `msg.sender` (the Safe) ensures no Safe can
     *      interfere with another's allowances.
     */
    mapping(address safe => mapping(bytes32 safeTxHash => uint256 executableAt)) private _allowedTransactions;

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
     * @notice Emitted when a FROST attestation is verified and stored for a transaction.
     * @param safeTxHash The EIP-712 hash of the attested Safe transaction.
     * @param epoch      The epoch under which the attestation was produced.
     * @param sigId      The keccak256 identifier of the FROST signing ceremony.
     */
    event AttestationSubmitted(bytes32 indexed safeTxHash, uint64 indexed epoch, bytes32 indexed sigId);

    /**
     * @notice Emitted when a module attestation is consumed by `checkModuleTransaction`.
     * @param safeTxHash The hash of the module transaction that executed.
     * @param sigId      The signing ceremony identifier permanently marked spent.
     */
    event ModuleAttestationConsumed(bytes32 indexed safeTxHash, bytes32 indexed sigId);

    /**
     * @notice Emitted when a pending module attestation is cleared by `cancelModuleAttestation`
     *         without spending the sigId.
     * @param safeTxHash The hash whose pending attestation was removed.
     */
    event ModuleAttestationCancelled(bytes32 indexed safeTxHash);

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
     * @notice Thrown by `checkTransaction` and `checkModuleTransaction` when neither a valid
     *         attestation nor a matured time-delayed allowance exists for the transaction.
     */
    error AttestationNotFound();

    /**
     * @notice Thrown by `submitAttestation` when `_attestations` already holds a non-zero entry
     *         for the supplied `safeTxHash`. Call `cancelModuleAttestation` first to replace a
     *         stale pending module attestation.
     */
    error AttestationAlreadySubmitted();

    /**
     * @notice Thrown by `submitAttestation` when the requested epoch matches neither the current
     *         nor the previous epoch, and by `previousEpoch` before any rollover has occurred.
     */
    error InvalidEpoch();

    /**
     * @notice Thrown by `updateEpoch` when `proposedEpoch` is not strictly greater than the
     *         current active epoch. Prevents epoch replay and backwards transitions.
     */
    error EpochNotAdvancing();

    /**
     * @notice Thrown by `submitAttestation` when the derived sigId is already present in
     *         `_usedModuleSigs`. Prevents a ceremony spent by a module execution from being
     *         resubmitted as a new pending attestation for any `safeTxHash`.
     */
    error SignatureAlreadySpent();

    /**
     * @notice Thrown by `cancelModuleAttestation` when `_attestations` holds no entry for the
     *         computed module transaction hash under `msg.sender`.
     */
    error NoModuleAttestationPending();

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
        _currentEpoch = EpochState({epoch: initialEpoch, groupKey: initialGroupKey});
        emit EpochUpdated(0, initialEpoch, initialGroupKey);
    }

    // ============================================================
    // EPOCH MANAGEMENT
    // ============================================================

    /**
     * @notice Advances the guard's epoch state using a FROST-signed rollover from the current
     *         active validator group.
     * @dev Permissionless — any party holding the rollover signature may call this (validators,
     *      relayers, Safe owners). Multi-epoch jumps are permitted: if validators sign a rollover
     *      that skips intermediate epoch numbers.
     *      To catch up multiple epochs in sequence, batch calls in ascending order via
     *      MultiSendCallOnly.
     *      `rolloverBlock` is a Gnosis Chain block number embedded in the signed message; no check
     *      against `block.number` is performed because the local chain's block number is unrelated.
     *      `_currentEpoch` shifts to `_previousEpoch` and the new epoch becomes `_currentEpoch`.
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
        require(proposedEpoch > _currentEpoch.epoch, EpochNotAdvancing());
        Secp256k1.requireNonZero(newGroupKey);
        bytes32 message = ConsensusMessages.epochRollover(
            _CONSENSUS_DOMAIN_SEPARATOR, _currentEpoch.epoch, proposedEpoch, rolloverBlock, newGroupKey
        );
        FROST.verify(_currentEpoch.groupKey, signature, message);
        uint64 prevEpoch = _currentEpoch.epoch;
        _previousEpoch = _currentEpoch;
        _hasPreviousEpoch = true;
        _currentEpoch = EpochState({epoch: proposedEpoch, groupKey: newGroupKey});
        emit EpochUpdated(prevEpoch, proposedEpoch, newGroupKey);
    }

    // ============================================================
    // ATTESTATION
    // ============================================================

    /**
     * @notice Pre-submits a FROST attestation authorising a Safe transaction (regular or module).
     * @dev Permissionless — typically called by validators or relayers immediately after a signing
     *      ceremony completes. Must be called before the Safe executes the transaction.
     *      The sigId is checked against `_usedModuleSigs` before signature verification. This
     *      prevents a ceremony spent by a prior module execution from being resubmitted for any
     *      `safeTxHash`, including a different one. For regular transactions this check is applied
     *      consistently even though the nonce independently prevents replay.
     *      Only one pending attestation per `safeTxHash` is permitted at a time. If a stale module
     *      attestation exists, call `cancelModuleAttestation` first to clear it.
     *      Module transactions must be hashed with all gas and nonce fields set to zero, matching
     *      the convention used by `checkModuleTransaction` and `cancelModuleAttestation`.
     * @param safeTxHash EIP-712 hash of the Safe transaction. For module transactions this must
     *                   be computed with zeroed gas and nonce fields.
     * @param epoch      Epoch in which the attestation was produced. Must be either the current
     *                   or the previous epoch; any other value reverts with `InvalidEpoch`.
     * @param signature  FROST threshold signature over the transaction proposal message.
     */
    function submitAttestation(bytes32 safeTxHash, uint64 epoch, FROST.Signature calldata signature) external {
        require(_attestations[safeTxHash] == bytes32(0), AttestationAlreadySubmitted());
        // forge-lint: disable-next-line(asm-keccak256)
        bytes32 sigId = keccak256(abi.encode(signature.r.x, signature.r.y, signature.z));
        require(!_usedModuleSigs[sigId], SignatureAlreadySpent());
        Secp256k1.Point memory groupKey = _resolveGroupKey(epoch);
        bytes32 message = ConsensusMessages.transactionProposal(_CONSENSUS_DOMAIN_SEPARATOR, epoch, safeTxHash);
        FROST.verify(groupKey, signature, message);
        _attestations[safeTxHash] = sigId;
        emit AttestationSubmitted(safeTxHash, epoch, sigId);
    }

    /**
     * @notice Clears a pending module attestation that will not be executed.
     * @dev Must be called by the Safe itself via `execTransaction`. No Safenet attestation is
     *      required — the guard auto-allows this selector, and authentication is provided by the
     *      Safe's threshold signature on the outer `execTransaction`.
     *      The module transaction hash is recomputed from the supplied parameters using `msg.sender`
     *      as the Safe address and zeroed gas/nonce fields — the same convention as
     *      `checkModuleTransaction`. A caller can therefore only cancel attestations registered
     *      under their own address; passing identical parameters from a different address produces
     *      a different hash and finds no entry.
     *      Cancellation does not spend the sigId: the signing ceremony remains reusable and a
     *      subsequent `submitAttestation` call with the same signature will succeed.
     * @param to        Destination address of the module transaction.
     * @param value     Ether value of the module transaction.
     * @param data      Calldata of the module transaction.
     * @param operation Operation type of the module transaction.
     */
    function cancelModuleAttestation(address to, uint256 value, bytes calldata data, Enum.Operation operation)
        external
    {
        bytes32 safeTxHash = SafeTransaction.hash(
            SafeTransaction.T({
                chainId: block.chainid,
                safe: msg.sender,
                to: to,
                value: value,
                data: data,
                operation: SafeTransaction.Operation(uint8(operation)),
                safeTxGas: 0,
                baseGas: 0,
                gasPrice: 0,
                gasToken: address(0),
                refundReceiver: address(0),
                nonce: 0
            })
        );
        require(_attestations[safeTxHash] != bytes32(0), NoModuleAttestationPending());
        delete _attestations[safeTxHash];
        emit ModuleAttestationCancelled(safeTxHash);
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
     *      via `checkTransaction` or `checkModuleTransaction`.
     * @param safeTxHash Hash of the transaction to allow. Reverts `TransactionAlreadyAllowed`
     *                   if an allowance is already pending for this Safe and hash.
     */
    function allowTransaction(bytes32 safeTxHash) external {
        require(_allowedTransactions[msg.sender][safeTxHash] == 0, TransactionAlreadyAllowed());
        uint256 executableAt = block.timestamp + _ALLOW_TX_DELAY;
        _allowedTransactions[msg.sender][safeTxHash] = executableAt;
        emit TransactionAllowed(msg.sender, safeTxHash, executableAt);
    }

    /**
     * @notice Cancels a pending time-delayed allowance registered by this Safe.
     * @dev Must be called by the Safe itself via `execTransaction`. Also auto-allowed by the guard.
     *      For regular Safe transactions, burning the nonce by executing any other attested
     *      transaction at the same nonce also invalidates the registered hash implicitly.
     *      For module transactions — which carry no nonce — this is the only explicit cancellation
     *      path.
     * @param safeTxHash Hash whose allowance should be cancelled. Reverts `AllowanceNotFound` if
     *                   no allowance exists for this Safe and hash.
     */
    function cancelAllowTransaction(bytes32 safeTxHash) external {
        require(_allowedTransactions[msg.sender][safeTxHash] != 0, AllowanceNotFound());
        delete _allowedTransactions[msg.sender][safeTxHash];
        emit AllowanceCancelled(msg.sender, safeTxHash);
    }

    // ============================================================
    // SAFE TRANSACTION GUARD
    // ============================================================

    /**
     * @notice Pre-execution hook called by Safe's GuardManager for every owner-signed transaction.
     * @dev Execution is permitted if any of the following conditions holds (checked in order):
     *      1. Auto-allow: the call targets this guard with a whitelisted selector
     *         (`allowTransaction`, `cancelAllowTransaction`, `cancelModuleAttestation`), zero
     *         value, and `CALL` operation.
     *      2. A pending attestation exists for the `safeTxHash` — the entry is deleted on passage.
     *      3. A time-delayed allowance exists and its delay has elapsed — the entry is deleted.
     *      Safe increments its nonce before invoking this hook, so the pre-execution nonce used
     *      when the attestation was registered is `ISafe(msg.sender).nonce() - 1`.
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
        bytes memory, /* signatures */
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

        bytes32 sigId = _attestations[safeTxHash];
        if (sigId != bytes32(0)) {
            delete _attestations[safeTxHash];
            return;
        }

        uint256 executableAt = _allowedTransactions[msg.sender][safeTxHash];
        if (executableAt != 0 && block.timestamp >= executableAt) {
            delete _allowedTransactions[msg.sender][safeTxHash];
            emit TransactionExecutedViaAllowance(msg.sender, safeTxHash);
            return;
        }

        revert AttestationNotFound();
    }

    /**
     * @notice Post-execution hook called by Safe's GuardManager after every owner-signed transaction.
     * @dev Intentionally empty. Reserved for future post-execution logic.
     */
    function checkAfterExecution(bytes32, bool) external pure override {}

    // ============================================================
    // SAFE MODULE GUARD
    // ============================================================

    /**
     * @notice Pre-execution hook called by Safe's ModuleManager for every module transaction.
     * @dev Execution is permitted under the same three conditions as `checkTransaction`.
     *      When a pending attestation is consumed, the sigId is permanently marked spent in
     *      `_usedModuleSigs` — this prevents the same signing ceremony from unlocking a future
     *      execution even if the module resubmits with identical parameters.
     *      Module transactions carry no nonce, so all gas and nonce fields in the `SafeTransaction.T`
     *      struct are set to zero. Validators and relayers must hash with the same zeroed fields
     *      when calling `submitAttestation`.
     *      Reverts `AttestationNotFound` if none of the conditions hold.
     * @return moduleTxHash The computed Safe transaction hash for this module execution. Returns
     *         `bytes32(0)` for auto-allowed calls — `checkAfterModuleExecution` is a no-op so
     *         this is safe, but any future post-hook logic must not treat a zero return as
     *         meaningful.
     */
    function checkModuleTransaction(
        address to,
        uint256 value,
        bytes memory data,
        Enum.Operation operation,
        address /* module */
    )
        external
        override
        returns (bytes32 moduleTxHash)
    {
        if (_isAutoAllowed(to, value, data, operation)) return bytes32(0);

        // Module transactions carry no nonce; all gas and nonce fields are zeroed.
        moduleTxHash = SafeTransaction.hash(
            SafeTransaction.T({
                chainId: block.chainid,
                safe: msg.sender,
                to: to,
                value: value,
                data: data,
                operation: SafeTransaction.Operation(uint8(operation)),
                safeTxGas: 0,
                baseGas: 0,
                gasPrice: 0,
                gasToken: address(0),
                refundReceiver: address(0),
                nonce: 0
            })
        );

        bytes32 sigId = _attestations[moduleTxHash];
        if (sigId != bytes32(0)) {
            _usedModuleSigs[sigId] = true;
            delete _attestations[moduleTxHash];
            emit ModuleAttestationConsumed(moduleTxHash, sigId);
            return moduleTxHash;
        }

        uint256 executableAt = _allowedTransactions[msg.sender][moduleTxHash];
        if (executableAt != 0 && block.timestamp >= executableAt) {
            delete _allowedTransactions[msg.sender][moduleTxHash];
            emit TransactionExecutedViaAllowance(msg.sender, moduleTxHash);
            return moduleTxHash;
        }

        revert AttestationNotFound();
    }

    /**
     * @notice Post-execution hook called by Safe's ModuleManager after every module transaction.
     * @dev Intentionally empty. Reserved for future post-execution logic.
     */
    function checkAfterModuleExecution(bytes32, bool) external pure override {}

    // ============================================================
    // VIEW FUNCTIONS
    // ============================================================

    /**
     * @notice Returns the epoch number of the current active epoch.
     */
    function activeEpoch() external view returns (uint64) {
        return _currentEpoch.epoch;
    }

    /**
     * @notice Returns the epoch number of the previous epoch.
     * @dev Only valid after the first `updateEpoch` call. Reverts `InvalidEpoch` before any
     *      rollover has occurred.
     */
    function previousEpoch() external view returns (uint64) {
        if (!_hasPreviousEpoch) revert InvalidEpoch();
        return _previousEpoch.epoch;
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
     * @notice Returns the sigId stored for a pending attestation, or `bytes32(0)` if none.
     * @dev Applies to both regular and module transaction attestations.
     */
    function getAttestation(bytes32 safeTxHash) external view returns (bytes32 sigId) {
        return _attestations[safeTxHash];
    }

    /**
     * @notice Returns whether a signing ceremony has been permanently spent by a module execution.
     * @dev A `true` result means the sigId was consumed by `checkModuleTransaction` and can
     *      never again be used to unlock a module execution or pass the `submitAttestation`
     *      resubmission check.
     */
    function isModuleSigSpent(bytes32 sigId) external view returns (bool) {
        return _usedModuleSigs[sigId];
    }

    /**
     * @notice Returns the earliest Unix timestamp at which a pending allowance may execute,
     *         or zero if no allowance exists for the given Safe and hash.
     */
    function getAllowedTxTimestamp(address safe, bytes32 safeTxHash) external view returns (uint256 executableAt) {
        return _allowedTransactions[safe][safeTxHash];
    }

    // ============================================================
    // PRIVATE HELPERS
    // ============================================================

    /**
     * @dev Returns the group key for `epoch`. Checks current first, then previous.
     *      Reverts `InvalidEpoch` if neither entry matches.
     */
    function _resolveGroupKey(uint64 epoch) private view returns (Secp256k1.Point memory) {
        if (_currentEpoch.epoch == epoch) return _currentEpoch.groupKey;
        if (_hasPreviousEpoch && _previousEpoch.epoch == epoch) return _previousEpoch.groupKey;
        revert InvalidEpoch();
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
        return selector == SafenetGuard.allowTransaction.selector
            || selector == SafenetGuard.cancelAllowTransaction.selector
            || selector == SafenetGuard.cancelModuleAttestation.selector;
    }
}
