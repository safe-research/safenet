// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {ConsensusMessages} from "@/libraries/ConsensusMessages.sol";
import {FROST} from "@/libraries/FROST.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";
import {ISignatureValidator} from "@safe/interfaces/ISignatureValidator.sol";

/**
 * @title SafenetCosigner
 * @notice EIP-1271 contract signer that gates Safe transactions behind Safenet
 *         threshold-signature attestation. Deploy once and add as a Safe owner.
 *
 * @dev **Signature construction.**
 *      Safe requires `signatures` entries sorted ascending by owner address. For the
 *      cosigner entry, include a contract signature slot (`v = 0`) and append the FROST
 *      attestation as dynamic data:
 *
 *      Static slot (65 bytes):
 *        r (bytes32) = address(cosigner) left-padded with zeros
 *        s (bytes32) = byte offset of the dynamic data = (total static entries) * 65
 *        v (uint8)   = 0x00
 *
 *      Dynamic data (appended after all static entries):
 *        uint256                    : byte length of the encoded attestation
 *        abi.encode(uint64, FROST.Signature) : epoch and FROST signature
 *
 *      Safe calls `isValidSignature(safeTxHash, abi.encode(epoch, sig))` on this contract.
 *      To use the escape hatch instead, set the dynamic data to an empty byte sequence
 *      (length = 0, no following bytes) and ensure a matured allowance exists for this Safe.
 *      Allowances are registered by calling `allowTransaction` on this contract via the Safe's
 *      own `execTransaction` — requiring the full owner-signature threshold including the
 *      cosigner's approval. This prevents any subset of owners from bypassing the threshold.
 */
contract SafenetCosigner is ISignatureValidator {
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
     * @dev Guards `_resolveGroupKey` against treating the zero-initialised `$previousEpoch`
     *      slot as a valid epoch entry before any rollover has occurred. Without this flag,
     *      `_resolveGroupKey(0)` would return a zero group key — trivially forgeable via FROST.
     */
    // forge-lint: disable-next-line(mixed-case-variable)
    bool private $hasPreviousEpoch;

    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(address safe => mapping(bytes32 safeTxHash => uint256 executableAt)) private $allowedTransactions;

    // ============================================================
    // EVENTS
    // ============================================================

    event EpochUpdated(uint64 indexed previousEpoch, uint64 indexed activeEpoch, Secp256k1.Point activeGroupKey);

    event TransactionAllowed(address indexed safe, bytes32 indexed safeTxHash, uint256 executableAt);

    event AllowanceCancelled(address indexed safe, bytes32 indexed safeTxHash);

    // ============================================================
    // ERRORS
    // ============================================================

    error AttestationNotFound();

    error InvalidEpoch();

    error EpochNotAdvancing();

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
    // EIP-1271
    // ============================================================

    /**
     * @dev Called by Safe during `execTransaction`. `msg.sender` is the Safe.
     *      Two execution paths:
     *      1. FROST attestation: `_signature` = `abi.encode(uint64 epoch, FROST.Signature sig)`.
     *         Verifies the FROST sig against `_hash` under the resolved group key.
     *      2. Escape hatch: `_signature` is empty. Approves if a matured allowance exists for
     *         `(msg.sender, _hash)`. Allowances are not deleted on use because this function is
     *         `view` — replay is prevented by Safe's own nonce advancing after execution.
     */
    function isValidSignature(bytes32 _hash, bytes memory _signature) external view override returns (bytes4) {
        if (_signature.length > 0) {
            (uint64 epoch, FROST.Signature memory sig) = abi.decode(_signature, (uint64, FROST.Signature));
            bytes32 message = ConsensusMessages.transactionProposal(_CONSENSUS_DOMAIN_SEPARATOR, epoch, _hash);
            FROST.verify(_resolveGroupKey(epoch), sig, message);
            return EIP1271_MAGIC_VALUE;
        }

        uint256 executableAt = $allowedTransactions[msg.sender][_hash];
        if (executableAt != 0 && block.timestamp >= executableAt) return EIP1271_MAGIC_VALUE;

        revert AttestationNotFound();
    }

    // ============================================================
    // ALLOWANCE (ESCAPE HATCH)
    // ============================================================

    /**
     * @dev Registers a time-delayed allowance for `safeTxHash`. Must be called by the Safe
     *      itself via `execTransaction`, so the full owner-signature threshold is required.
     *      After `_ALLOW_TX_DELAY` seconds the cosigner approves the hash via the escape-hatch
     *      path in `isValidSignature` (pass empty bytes as the dynamic contract signature data).
     */
    function allowTransaction(bytes32 safeTxHash) external {
        require($allowedTransactions[msg.sender][safeTxHash] == 0, TransactionAlreadyAllowed());
        uint256 executableAt = block.timestamp + _ALLOW_TX_DELAY;
        $allowedTransactions[msg.sender][safeTxHash] = executableAt;
        emit TransactionAllowed(msg.sender, safeTxHash, executableAt);
    }

    /**
     * @dev Cancels a pending allowance. Must be called by the Safe itself via `execTransaction`.
     *      Cancellation requires the Safe's owner-signature threshold — if the remaining human
     *      owners can reach that threshold without the cosigner, cancellation remains possible
     *      even when Safenet is unavailable.
     */
    function cancelAllowTransaction(bytes32 safeTxHash) external {
        require($allowedTransactions[msg.sender][safeTxHash] != 0, AllowanceNotFound());
        delete $allowedTransactions[msg.sender][safeTxHash];
        emit AllowanceCancelled(msg.sender, safeTxHash);
    }

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

    function getAllowedTxTimestamp(address safe, bytes32 safeTxHash) external view returns (uint256 executableAt) {
        return $allowedTransactions[safe][safeTxHash];
    }

    // ============================================================
    // PRIVATE HELPERS
    // ============================================================

    function _resolveGroupKey(uint64 epoch) private view returns (Secp256k1.Point memory) {
        if ($currentEpoch.epoch == epoch) return $currentEpoch.groupKey;
        if ($hasPreviousEpoch && $previousEpoch.epoch == epoch) return $previousEpoch.groupKey;
        revert InvalidEpoch();
    }
}
