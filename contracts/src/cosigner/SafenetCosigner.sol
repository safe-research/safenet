// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {ConsensusMessages} from "@/libraries/ConsensusMessages.sol";
import {FROST} from "@/libraries/FROST.sol";
import {SafeTransaction} from "@/libraries/SafeTransaction.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";
import {ISafe, IOwnerManager} from "@safe/interfaces/ISafe.sol";
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
 *        uint256                             : byte length of the encoded attestation
 *        abi.encode(uint64, FROST.Signature) : epoch and FROST signature
 *
 *      Safe calls `isValidSignature(safeTxHash, abi.encode(epoch, sig))` on this contract.
 *      To use the escape hatch instead, set the dynamic data to an empty byte sequence
 *      (length = 0, no following bytes) and ensure a matured escape hatch registration exists.
 *
 *      **Escape hatch.**
 *      Any Safe owner can register a time-delayed `removeOwner` call targeting the cosigner
 *      itself by calling `allowEscapeHatch` directly, or `allowEscapeHatchWithSig` when
 *      submitting via a relay service. Registration does not require a Safenet attestation.
 *      After `_ALLOW_TX_DELAY` seconds, the Safe owners can execute the pre-registered
 *      `removeOwner` transaction by passing empty bytes as the cosigner's dynamic signature
 *      data; the cosigner approves it via the empty-signature path in `isValidSignature`.
 *      The registered hash is nonce-bound: if other transactions advance the Safe nonce before
 *      execution, the registration becomes stale and must be repeated. To invalidate a pending
 *      registration, `threshold` owners can execute a dummy transaction to advance the Safe nonce.
 */
contract SafenetCosigner is ISignatureValidator {
    // ============================================================
    // TYPES
    // ============================================================

    struct EpochState {
        uint64 epoch;
        Secp256k1.Point groupKey;
    }

    /**
     * @notice Parameters for an escape-hatch registration.
     * @custom:param safe The Safe account from which this cosigner should be removed.
     * @custom:param prevOwner Owner address preceding this cosigner in the Safe's owner linked list.
     * @custom:param threshold Threshold to set after removal; must equal the Safe's current threshold
     *              or current threshold minus one.
     * @custom:param safeTxGas Gas forwarded to the Safe transaction's inner call.
     * @custom:param baseGas Base gas cost charged to the Safe for the transaction.
     * @custom:param gasPrice Gas price used for the refund calculation.
     * @custom:param gasToken Token used for gas payment; `address(0)` for the native token.
     * @custom:param refundReceiver Recipient of the gas refund; `address(0)` for `tx.origin`.
     * @custom:param nonce The Safe's current nonce at registration time. Binds the registration to a
     *              specific Safe nonce, preventing replay of a signed `EscapeHatchRequest` across
     *              future nonce values.
     */
    struct EscapeHatchRequest {
        address safe;
        address prevOwner;
        uint256 threshold;
        uint256 safeTxGas;
        uint256 baseGas;
        uint256 gasPrice;
        address gasToken;
        address refundReceiver;
        uint256 nonce;
    }

    // ============================================================
    // CONSTANTS
    // ============================================================

    bytes32 private constant _ESCAPE_HATCH_TYPEHASH = keccak256(
        "EscapeHatchRequest(address safe,address prevOwner,uint256 threshold,uint256 safeTxGas,uint256 baseGas,uint256 gasPrice,address gasToken,address refundReceiver,uint256 nonce)"
    );

    // ============================================================
    // STORAGE VARIABLES
    // ============================================================

    bytes32 private immutable _CONSENSUS_DOMAIN_SEPARATOR;

    uint256 private immutable _ALLOW_TX_DELAY;

    bytes32 private immutable _CACHED_DOMAIN_SEPARATOR;

    uint256 private immutable _CACHED_CHAIN_ID;

    // forge-lint: disable-next-line(mixed-case-variable)
    EpochState private $currentEpoch;

    /**
     * @dev Only valid after the first `updateEpoch` call. Callers must validate the returned key
     *      with `Secp256k1.requireNonZero` before use.
     */
    // forge-lint: disable-next-line(mixed-case-variable)
    EpochState private $previousEpoch;

    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(address safe => mapping(bytes32 safeTxHash => uint256 executableAt)) private $allowedTransactions;

    // ============================================================
    // EVENTS
    // ============================================================

    event EpochUpdated(uint64 indexed previousEpoch, uint64 indexed activeEpoch, Secp256k1.Point activeGroupKey);

    event TransactionAllowed(address indexed safe, bytes32 indexed safeTxHash, uint256 executableAt);

    // ============================================================
    // ERRORS
    // ============================================================

    error InvalidEpoch();

    error EpochNotAdvancing();

    error TransactionAlreadyAllowed();

    error InvalidAddress();

    error InvalidParameter();

    error NotSafeOwner();

    error InvalidThreshold();

    error InvalidNonce();

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
        _CACHED_CHAIN_ID = block.chainid;
        // forge-lint: disable-next-item(asm-keccak256)
        _CACHED_DOMAIN_SEPARATOR = keccak256(
            abi.encode(
                keccak256("EIP712Domain(uint256 chainId,address verifyingContract)"), block.chainid, address(this)
            )
        );
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
     *      2. Escape hatch: `_signature` is empty. Approves if a matured escape hatch registration
     *         exists for `(msg.sender, _hash)`. Registrations are not deleted on use because this
     *         function is `view` — replay is prevented by Safe's own nonce advancing after execution.
     */
    function isValidSignature(bytes32 _hash, bytes memory _signature) external view override returns (bytes4) {
        if (_signature.length > 0) {
            if (_signature.length != 128) return bytes4(0);
            (uint64 epoch, FROST.Signature memory sig) = abi.decode(_signature, (uint64, FROST.Signature));
            bytes32 message = ConsensusMessages.transactionProposal(_CONSENSUS_DOMAIN_SEPARATOR, epoch, _hash);
            Secp256k1.Point memory groupKey = _resolveGroupKey(epoch);
            Secp256k1.requireNonZero(groupKey);
            FROST.verify(groupKey, sig, message);
            return EIP1271_MAGIC_VALUE;
        }

        uint256 executableAt = $allowedTransactions[msg.sender][_hash];
        if (executableAt != 0 && block.timestamp >= executableAt) return EIP1271_MAGIC_VALUE;

        return bytes4(0);
    }

    // ============================================================
    // ESCAPE HATCH
    // ============================================================

    /**
     * @dev Registers a time-delayed `removeOwner` call that removes this cosigner from `request.safe`.
     *      Any single Safe owner may call this directly — no Safenet attestation is required,
     *      making the escape hatch available even when Safenet is unavailable.
     *      `request.threshold` must equal the Safe's current threshold or current threshold minus one.
     *      The registered hash is nonce-bound to the Safe's current nonce at registration time;
     *      if other transactions advance the nonce before execution, re-registration is required.
     *      To invalidate a pending registration, advance the Safe nonce with a dummy transaction.
     */
    function allowEscapeHatch(EscapeHatchRequest calldata request) external {
        require(ISafe(payable(request.safe)).isOwner(msg.sender), NotSafeOwner());
        _registerEscapeHatch(request);
    }

    /**
     * @dev Relay-compatible variant of `allowEscapeHatch`. Accepts a Safe-compatible signature
     *      (packed ECDSA, EIP-1271 contract signature, or pre-approved hash) from a Safe owner
     *      over the EIP-712 hash of the `EscapeHatchRequest`, allowing submission via a relay
     *      service where `msg.sender` is not the Safe owner. Signature verification and owner
     *      membership check are both delegated to `ISafe.checkNSignatures`.
     */
    function allowEscapeHatchWithSig(EscapeHatchRequest calldata request, bytes calldata signature) external {
        // forge-lint: disable-next-item(asm-keccak256)
        bytes32 structHash = keccak256(
            abi.encode(
                _ESCAPE_HATCH_TYPEHASH,
                request.safe,
                request.prevOwner,
                request.threshold,
                request.safeTxGas,
                request.baseGas,
                request.gasPrice,
                request.gasToken,
                request.refundReceiver,
                request.nonce
            )
        );
        // forge-lint: disable-next-item(asm-keccak256)
        bytes32 messageHash = keccak256(abi.encodePacked("\x19\x01", _domainSeparator(), structHash));
        ISafe(payable(request.safe)).checkNSignatures(address(0), messageHash, signature, 1);
        _registerEscapeHatch(request);
    }

    // ============================================================
    // VIEW FUNCTIONS
    // ============================================================

    function activeEpoch() external view returns (uint64) {
        return $currentEpoch.epoch;
    }

    function previousEpoch() external view returns (uint64) {
        Secp256k1.requireNonZero($previousEpoch.groupKey);
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

    function _registerEscapeHatch(EscapeHatchRequest calldata request) private {
        ISafe safeContract = ISafe(payable(request.safe));
        require(request.nonce == safeContract.nonce(), InvalidNonce());
        uint256 currentThreshold = safeContract.getThreshold();
        require(request.threshold == currentThreshold || request.threshold == currentThreshold - 1, InvalidThreshold());
        bytes32 safeTxHash = _escapeHatchTxHash(request);
        require($allowedTransactions[request.safe][safeTxHash] == 0, TransactionAlreadyAllowed());
        uint256 executableAt = block.timestamp + _ALLOW_TX_DELAY;
        $allowedTransactions[request.safe][safeTxHash] = executableAt;
        emit TransactionAllowed(request.safe, safeTxHash, executableAt);
    }

    function _escapeHatchTxHash(EscapeHatchRequest calldata request) private view returns (bytes32) {
        return SafeTransaction.hash(
            SafeTransaction.T({
                chainId: block.chainid,
                safe: request.safe,
                to: request.safe,
                value: 0,
                data: abi.encodeCall(IOwnerManager.removeOwner, (request.prevOwner, address(this), request.threshold)),
                operation: SafeTransaction.Operation.CALL,
                safeTxGas: request.safeTxGas,
                baseGas: request.baseGas,
                gasPrice: request.gasPrice,
                gasToken: request.gasToken,
                refundReceiver: request.refundReceiver,
                nonce: request.nonce
            })
        );
    }

    function _domainSeparator() private view returns (bytes32) {
        if (block.chainid == _CACHED_CHAIN_ID) return _CACHED_DOMAIN_SEPARATOR;
        // forge-lint: disable-next-item(asm-keccak256)
        return keccak256(
            abi.encode(
                keccak256("EIP712Domain(uint256 chainId,address verifyingContract)"), block.chainid, address(this)
            )
        );
    }

    function _resolveGroupKey(uint64 epoch) private view returns (Secp256k1.Point memory) {
        if ($currentEpoch.epoch == epoch) return $currentEpoch.groupKey;
        if ($previousEpoch.epoch == epoch) return $previousEpoch.groupKey;
        revert InvalidEpoch();
    }
}
