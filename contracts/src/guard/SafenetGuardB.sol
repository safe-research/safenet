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
 * @title SafenetGuardB
 * @notice Safe Transaction Guard and Module Guard that gates every Safe transaction behind
 *         Safenet threshold-signature attestation.
 */
contract SafenetGuardB is BaseGuard {
    // ============================================================
    // TYPES
    // ============================================================

    struct EpochState {
        uint64 epoch;
        Secp256k1.Point groupKey;
    }

    // ============================================================
    // STORAGE VARIABLES
    // ============================================================

    bytes32 private immutable _CONSENSUS_DOMAIN_SEPARATOR;

    uint256 private immutable _ALLOW_TX_DELAY;

    // forge-lint: disable-next-line(mixed-case-variable)
    EpochState private $currentEpoch;

    /**
     * @dev Only valid when `$hasPreviousEpoch` is true (after the first `updateEpoch` call).
     */
    // forge-lint: disable-next-line(mixed-case-variable)
    EpochState private $previousEpoch;

    /**
     * @notice True once the first `updateEpoch` call has populated `$previousEpoch`.
     * @dev Guards `previousEpoch()` and `_resolveGroupKey` against treating the zero-initialised
     *      `$previousEpoch` slot as a valid epoch entry before any rollover has occurred.
     *      Without this flag, `_resolveGroupKey(0)` would match the zero-initialised
     *      `$previousEpoch.epoch` and return a zero group key. FROST verification against a zero
     *      key is trivially forgeable, so an attacker could register a fake attestation for any
     *      transaction without a real signing ceremony.
     */
    // forge-lint: disable-next-line(mixed-case-variable)
    bool private $hasPreviousEpoch;

    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(bytes32 safeTxHash => bytes32 sigId) private $attestations;

    /**
     * @notice Per-`(safe, module, tx-params)` execution sequence counter.
     * @dev The key is `keccak256(abi.encode(safe, module, to, value, keccak256(data), operation))`.
     *      `data` is pre-hashed to avoid a large memory copy on every call site.
     *      The key intentionally includes the full tx-param tuple rather than just `(safe, module)`.
     *      A per-module counter would serialise all operations from a module onto a single sequence:
     *      if two different tx-param combinations are attested at consecutive nonces (e.g. TxA at
     *      nonce=0, TxB at nonce=1) but TxB is executed first, the guard reconstructs the wrong hash
     *      and rejects it — TxB is blocked until TxA executes, creating a deadlock. With per-tx-param
     *      counters each combination has an independent track and operations may execute in any order.
     *      The counter is packed with the module address as `(uint256(uint160(module)) << 96) | counter`
     *      and used as the `nonce` field in the module tx hash via `_packedModuleNonce`. The packing
     *      ensures two different modules on the same Safe cannot collide at the same counter value.
     *      The counter increments on both successful execution and cancellation.
     */
    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(bytes32 moduleNonceKey => uint64 nonce) private $moduleNonces;

    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(address safe => mapping(bytes32 safeTxHash => uint256 executableAt)) private $allowedTransactions;

    // ============================================================
    // EVENTS
    // ============================================================

    event EpochUpdated(uint64 indexed previousEpoch, uint64 indexed activeEpoch, Secp256k1.Point activeGroupKey);

    event AttestationSubmitted(bytes32 indexed safeTxHash, uint64 indexed epoch, bytes32 indexed sigId);

    event ModuleAttestationSubmitted(
        address indexed safe,
        address indexed module,
        bytes32 indexed moduleTxHash,
        uint64 nonce,
        uint64 epoch,
        bytes32 sigId
    );

    event ModuleAttestationConsumed(bytes32 indexed moduleTxHash, uint64 nonce, bytes32 indexed sigId);

    event ModuleAttestationCancelled(bytes32 indexed moduleTxHash, uint64 nonce, bytes32 indexed sigId);

    event TransactionAllowed(address indexed safe, bytes32 indexed safeTxHash, uint256 executableAt);

    event AllowanceCancelled(address indexed safe, bytes32 indexed safeTxHash);

    event TransactionExecutedViaAllowance(address indexed safe, bytes32 indexed safeTxHash);

    // ============================================================
    // ERRORS
    // ============================================================

    error AttestationNotFound();

    error AttestationAlreadySubmitted();

    error InvalidEpoch();

    error EpochNotAdvancing();

    error NoModuleAttestationPending();

    error TransactionAlreadyAllowed();

    error AllowanceNotFound();

    error InvalidAddress();

    error InvalidParameter();

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

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
        $currentEpoch = EpochState({epoch: initialEpoch, groupKey: initialGroupKey});
        emit EpochUpdated(0, initialEpoch, initialGroupKey);
    }

    // ============================================================
    // EPOCH MANAGEMENT
    // ============================================================

    /**
     * @dev `$currentEpoch` shifts to `$previousEpoch` and the new epoch becomes `$currentEpoch`.
     *      Only the two most recent epochs are retained; older keys are discarded on each rollover.
     */
    function updateEpoch(
        uint64 proposedEpoch,
        uint64 rolloverBlock,
        Secp256k1.Point calldata newGroupKey,
        FROST.Signature calldata signature
    ) external {
        require(proposedEpoch > $currentEpoch.epoch, EpochNotAdvancing());
        Secp256k1.requireNonZero(newGroupKey);
        bytes32 message = ConsensusMessages.epochRollover(
            _CONSENSUS_DOMAIN_SEPARATOR, $currentEpoch.epoch, proposedEpoch, rolloverBlock, newGroupKey
        );
        FROST.verify($currentEpoch.groupKey, signature, message);
        uint64 prevEpoch = $currentEpoch.epoch;
        $previousEpoch = $currentEpoch;
        $hasPreviousEpoch = true;
        $currentEpoch = EpochState({epoch: proposedEpoch, groupKey: newGroupKey});
        emit EpochUpdated(prevEpoch, proposedEpoch, newGroupKey);
    }

    // ============================================================
    // ATTESTATION
    // ============================================================

    /**
     * @dev For module transactions use `submitModuleAttestation`, which reads and validates the
     *      current module nonce internally.
     *      Only one pending attestation per `safeTxHash` is permitted at a time.
     */
    function submitAttestation(bytes32 safeTxHash, uint64 epoch, FROST.Signature calldata signature) external {
        require($attestations[safeTxHash] == bytes32(0), AttestationAlreadySubmitted());
        // forge-lint: disable-next-line(asm-keccak256)
        bytes32 sigId = keccak256(abi.encode(signature.r.x, signature.r.y, signature.z));
        Secp256k1.Point memory groupKey = _resolveGroupKey(epoch);
        bytes32 message = ConsensusMessages.transactionProposal(_CONSENSUS_DOMAIN_SEPARATOR, epoch, safeTxHash);
        FROST.verify(groupKey, signature, message);
        $attestations[safeTxHash] = sigId;
        emit AttestationSubmitted(safeTxHash, epoch, sigId);
    }

    /**
     * @notice Pre-submits a FROST attestation authorising a module transaction.
     * @dev The module tx hash is computed internally using the current nonce from `$moduleNonces`
     *      for the `(safe, module, tx-params)` key, ensuring the attestation is bound to the
     *      correct execution sequence position. Submitting with a nonce that does not match the
     *      current counter is not possible — the hash would differ and FROST verification would
     *      fail against a sig produced for the old hash.
     *      Only one pending attestation per `(safe, module, tx-params)` nonce is permitted;
     *      `cancelModuleAttestation` must be called to clear a stale pending entry.
     */
    function submitModuleAttestation(
        address safe,
        address module,
        address to,
        uint256 value,
        bytes calldata data,
        Enum.Operation operation,
        uint64 epoch,
        FROST.Signature calldata signature
    ) external {
        // forge-lint: disable-next-line(asm-keccak256)
        bytes32 moduleNonceKey = keccak256(abi.encode(safe, module, to, value, keccak256(data), operation));
        uint64 nonce = $moduleNonces[moduleNonceKey];
        bytes32 moduleTxHash =
            _moduleTransactionHash(safe, to, value, data, operation, _packedModuleNonce(nonce, module));
        require($attestations[moduleTxHash] == bytes32(0), AttestationAlreadySubmitted());
        // forge-lint: disable-next-line(asm-keccak256)
        bytes32 sigId = keccak256(abi.encode(signature.r.x, signature.r.y, signature.z));
        Secp256k1.Point memory groupKey = _resolveGroupKey(epoch);
        bytes32 message = ConsensusMessages.transactionProposal(_CONSENSUS_DOMAIN_SEPARATOR, epoch, moduleTxHash);
        FROST.verify(groupKey, signature, message);
        $attestations[moduleTxHash] = sigId;
        emit ModuleAttestationSubmitted(safe, module, moduleTxHash, nonce, epoch, sigId);
    }

    /**
     * @notice Cancels a pending module attestation and advances the module nonce.
     * @dev Must be called by the Safe itself via `execTransaction`. Auto-allowed by the guard —
     *      no Safenet attestation is needed; authentication is provided by the Safe's threshold
     *      signature on the outer `execTransaction`.
     *      The module transaction hash is recomputed from the supplied parameters using `msg.sender`
     *      as the Safe address and the current nonce for `(safe, module, tx-params)` — the same
     *      convention as `checkModuleTransaction`. A caller can therefore only cancel attestations
     *      registered under their own address.
     *      The module nonce is incremented after cancellation. This permanently orphans the
     *      cancelled signing ceremony: `submitModuleAttestation` always uses the current nonce, so
     *      FROST verification against the new hash will reject any sig produced for the old hash.
     *      A fresh signing ceremony is required to attest the same transaction again.
     * @param module    Address of the module whose pending attestation is being cancelled.
     * @param to        Destination address of the module transaction.
     * @param value     Ether value of the module transaction.
     * @param data      Calldata of the module transaction.
     * @param operation Operation type of the module transaction.
     */
    function cancelModuleAttestation(
        address module,
        address to,
        uint256 value,
        bytes calldata data,
        Enum.Operation operation
    ) external {
        // forge-lint: disable-next-line(asm-keccak256)
        bytes32 moduleNonceKey = keccak256(abi.encode(msg.sender, module, to, value, keccak256(data), operation));
        uint64 nonce = $moduleNonces[moduleNonceKey];
        bytes32 moduleTxHash =
            _moduleTransactionHash(msg.sender, to, value, data, operation, _packedModuleNonce(nonce, module));
        bytes32 sigId = $attestations[moduleTxHash];
        require(sigId != bytes32(0), NoModuleAttestationPending());
        delete $attestations[moduleTxHash];
        $moduleNonces[moduleNonceKey] = nonce + 1;
        emit ModuleAttestationCancelled(moduleTxHash, nonce, sigId);
    }

    // ============================================================
    // ALLOWANCE (ESCAPE HATCH)
    // ============================================================

    /**
     * @dev After `_ALLOW_TX_DELAY` seconds, the registered hash may execute without attestation
     *      via `checkTransaction` or `checkModuleTransaction`.
     *      For module transactions, the `safeTxHash` must be pre-computed with the current module
     *      nonce (readable via `getModuleNonce`). If another module execution advances the nonce
     *      before this allowance is consumed, the allowance will not match and cannot be used.
     */
    function allowTransaction(bytes32 safeTxHash) external {
        require($allowedTransactions[msg.sender][safeTxHash] == 0, TransactionAlreadyAllowed());
        uint256 executableAt = block.timestamp + _ALLOW_TX_DELAY;
        $allowedTransactions[msg.sender][safeTxHash] = executableAt;
        emit TransactionAllowed(msg.sender, safeTxHash, executableAt);
    }

    /**
     * @dev For module transactions — which carry no Safe nonce — this is the only explicit
     *      cancellation path.
     */
    function cancelAllowTransaction(bytes32 safeTxHash) external {
        require($allowedTransactions[msg.sender][safeTxHash] != 0, AllowanceNotFound());
        delete $allowedTransactions[msg.sender][safeTxHash];
        emit AllowanceCancelled(msg.sender, safeTxHash);
    }

    // ============================================================
    // SAFE TRANSACTION GUARD
    // ============================================================

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

        bytes32 sigId = $attestations[safeTxHash];
        if (sigId != bytes32(0)) {
            delete $attestations[safeTxHash];
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

    function checkAfterExecution(bytes32, bool) external pure override {}

    // ============================================================
    // SAFE MODULE GUARD
    // ============================================================

    /**
     * @dev The module tx hash is computed using the current `$moduleNonces` value for
     *      `(msg.sender, module, tx-params)`. On success, the nonce is incremented regardless
     *      of whether the attestation or escape-hatch path was taken. This ensures every
     *      execution consumes exactly one sequence position.
     *      Different tx-param combinations have independent nonce tracks and may execute in
     *      any order. Two executions of the same tx params must execute in nonce order.
     */
    function checkModuleTransaction(
        address to,
        uint256 value,
        bytes memory data,
        Enum.Operation operation,
        address module
    ) external override returns (bytes32 moduleTxHash) {
        if (_isAutoAllowed(to, value, data, operation)) return bytes32(0);

        // forge-lint: disable-next-line(asm-keccak256)
        bytes32 moduleNonceKey = keccak256(abi.encode(msg.sender, module, to, value, keccak256(data), operation));
        uint64 nonce = $moduleNonces[moduleNonceKey];
        moduleTxHash = _moduleTransactionHash(msg.sender, to, value, data, operation, _packedModuleNonce(nonce, module));

        bytes32 sigId = $attestations[moduleTxHash];
        if (sigId != bytes32(0)) {
            delete $attestations[moduleTxHash];
            $moduleNonces[moduleNonceKey] = nonce + 1;
            emit ModuleAttestationConsumed(moduleTxHash, nonce, sigId);
            return moduleTxHash;
        }

        uint256 executableAt = $allowedTransactions[msg.sender][moduleTxHash];
        if (executableAt != 0 && block.timestamp >= executableAt) {
            delete $allowedTransactions[msg.sender][moduleTxHash];
            $moduleNonces[moduleNonceKey] = nonce + 1;
            emit TransactionExecutedViaAllowance(msg.sender, moduleTxHash);
            return moduleTxHash;
        }

        revert AttestationNotFound();
    }

    function checkAfterModuleExecution(bytes32, bool) external pure override {}

    // ============================================================
    // VIEW FUNCTIONS
    // ============================================================

    function activeEpoch() external view returns (uint64) {
        return $currentEpoch.epoch;
    }

    function previousEpoch() external view returns (uint64) {
        if (!$hasPreviousEpoch) revert InvalidEpoch();
        return $previousEpoch.epoch;
    }

    function allowTxDelay() external view returns (uint256) {
        return _ALLOW_TX_DELAY;
    }

    function consensusDomainSeparator() external view returns (bytes32) {
        return _CONSENSUS_DOMAIN_SEPARATOR;
    }

    function getAttestation(bytes32 safeTxHash) external view returns (bytes32 sigId) {
        return $attestations[safeTxHash];
    }

    /**
     * @dev This is the nonce that will be embedded in the next module tx hash for this key.
     *      Validators and relayers should read this before initiating a signing ceremony to
     *      ensure the attestation targets the correct execution sequence position.
     *      The counter increments on both execution and cancellation.
     */
    function getModuleNonce(
        address safe,
        address module,
        address to,
        uint256 value,
        bytes calldata data,
        Enum.Operation operation
    ) external view returns (uint64 nonce) {
        // forge-lint: disable-next-line(asm-keccak256)
        return $moduleNonces[keccak256(abi.encode(safe, module, to, value, keccak256(data), operation))];
    }

    function getAllowedTxTimestamp(address safe, bytes32 safeTxHash) external view returns (uint256 executableAt) {
        return $allowedTransactions[safe][safeTxHash];
    }

    // ============================================================
    // PRIVATE HELPERS
    // ============================================================

    /**
     * @dev Returns the group key for `epoch`. Checks `$currentEpoch` first, then `$previousEpoch`.
     *      Reverts `InvalidEpoch` if neither entry matches, or if `$hasPreviousEpoch` is false
     *      and the epoch does not match the current one.
     */
    function _resolveGroupKey(uint64 epoch) private view returns (Secp256k1.Point memory) {
        if ($currentEpoch.epoch == epoch) return $currentEpoch.groupKey;
        if ($hasPreviousEpoch && $previousEpoch.epoch == epoch) return $previousEpoch.groupKey;
        revert InvalidEpoch();
    }

    /**
     * @dev Returns `(uint256(uint160(module)) << 96) | nonce`. The result is always >= 2^96,
     *      which cannot collide with a regular Safe transaction nonce (a sequential integer
     *      starting from 0 that will never reach 2^96 in practice).
     */
    function _packedModuleNonce(uint64 nonce, address module) private pure returns (uint256) {
        return (uint256(uint160(module)) << 96) | uint256(nonce);
    }

    /**
     * @dev Computes the EIP-712 hash for a module transaction with all gas fields zeroed.
     *      `nonce` must be the packed value from `_packedModuleNonce`.
     */
    function _moduleTransactionHash(
        address safe,
        address to,
        uint256 value,
        bytes memory data,
        Enum.Operation operation,
        uint256 nonce
    ) private view returns (bytes32) {
        return SafeTransaction.hash(
            SafeTransaction.T({
                chainId: block.chainid,
                safe: safe,
                to: to,
                value: value,
                data: data,
                operation: SafeTransaction.Operation(uint8(operation)),
                safeTxGas: 0,
                baseGas: 0,
                gasPrice: 0,
                gasToken: address(0),
                refundReceiver: address(0),
                nonce: nonce
            })
        );
    }

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
        return selector == SafenetGuardB.allowTransaction.selector
            || selector == SafenetGuardB.cancelAllowTransaction.selector
            || selector == SafenetGuardB.cancelModuleAttestation.selector;
    }
}
