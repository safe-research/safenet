// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROSTCoordinator} from "@/FROSTCoordinator.sol";
import {IFROSTCoordinatorCallback} from "@/interfaces/IFROSTCoordinatorCallback.sol";
import {ConsensusMessages} from "@/libraries/ConsensusMessages.sol";
import {FROST} from "@/libraries/FROST.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";
import {MetaTransaction} from "@/libraries/MetaTransaction.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

contract Consensus is IFROSTCoordinatorCallback {
    using ConsensusMessages for bytes32;
    using MetaTransaction for MetaTransaction.T;

    struct Epochs {
        uint64 active;
        uint64 staged;
        uint64 rolloverBlock;
        uint64 _padding;
    }

    event EpochProposed(
        uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 rolloverBlock, Secp256k1.Point groupKey
    );
    event EpochStaged(
        uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 rolloverBlock, Secp256k1.Point groupKey
    );
    event EpochRolledOver(uint64 indexed newActiveEpoch);
    event TransactionProposed(bytes32 indexed message, uint64 epoch, MetaTransaction.T transaction);
    event TransactionAttested(bytes32 indexed message);

    error InvalidRollover();
    error UnknownSignatureSelector();
    error NotCoordinator();

    FROSTCoordinator private immutable _COORDINATOR;

    // forge-lint: disable-next-line(mixed-case-variable)
    Epochs private $epochs;
    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(uint64 epoch => FROSTGroupId.T) private $groups;
    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(bytes32 message => FROSTSignatureId.T) private $attestations;

    constructor(address coordinator, FROSTGroupId.T group) {
        _COORDINATOR = FROSTCoordinator(coordinator);
        $groups[0] = group;
    }

    // forge-lint: disable-next-line(unwrapped-modifier-logic)
    modifier onlyCoordinator() {
        require(msg.sender == address(_COORDINATOR), NotCoordinator());
        _;
    }

    /// @notice Compute the EIP-712 domain separator used by the consensus
    ///         contract.
    function domainSeparator() public view returns (bytes32 result) {
        return ConsensusMessages.domain(block.chainid, address(this));
    }

    /// @notice Gets a transaction attestation.
    function getAttestation(uint64 epoch, MetaTransaction.T memory transaction)
        external
        view
        returns (bytes32 message, FROST.Signature memory signature)
    {
        message = domainSeparator().transactionProposal(epoch, transaction.hash());
        signature = getAttestationByMessage(message);
    }

    /// @notice Gets a transaction attestation by its hashed message.
    function getAttestationByMessage(bytes32 message) public view returns (FROST.Signature memory signature) {
        return _COORDINATOR.signatureValue($attestations[message]);
    }

    /// @notice Gets the active epoch and its group ID.
    function getActiveEpoch() external view returns (uint64 epoch, FROSTGroupId.T group) {
        Epochs memory epochs = $epochs;
        if (_epochsShouldRollover(epochs)) {
            epoch = epochs.staged;
        } else {
            epoch = epochs.active;
        }
        group = $groups[epoch];
    }

    /// @notice Proposes a new epoch that to be rolled over to.
    function proposeEpoch(uint64 proposedEpoch, uint64 rolloverBlock, FROSTGroupId.T group) public {
        Epochs memory epochs = _processRollover();
        _requireValidRollover(epochs, proposedEpoch, rolloverBlock);
        Secp256k1.Point memory groupKey = _COORDINATOR.groupKey(group);
        bytes32 message = domainSeparator().epochRollover(epochs.active, proposedEpoch, rolloverBlock, groupKey);
        emit EpochProposed(epochs.active, proposedEpoch, rolloverBlock, groupKey);
        _COORDINATOR.sign($groups[epochs.active], message);
    }

    /// @notice Stages an epoch to automatically rollover.
    function stageEpoch(uint64 proposedEpoch, uint64 rolloverBlock, FROSTGroupId.T group, FROSTSignatureId.T signature)
        public
    {
        Epochs memory epochs = _processRollover();
        _requireValidRollover(epochs, proposedEpoch, rolloverBlock);
        Secp256k1.Point memory groupKey = _COORDINATOR.groupKey(group);
        bytes32 message = domainSeparator().epochRollover(epochs.active, proposedEpoch, rolloverBlock, groupKey);
        _COORDINATOR.signatureVerify(signature, $groups[epochs.active], message);
        epochs.staged = proposedEpoch;
        epochs.rolloverBlock = rolloverBlock;
        $epochs = epochs;
        $groups[proposedEpoch] = group;
        emit EpochStaged(epochs.active, proposedEpoch, rolloverBlock, groupKey);
    }

    function proposeTransaction(MetaTransaction.T memory transaction) external returns (bytes32 message) {
        Epochs memory epochs = _processRollover();
        message = domainSeparator().transactionProposal(epochs.active, transaction.hash());
        emit TransactionProposed(message, epochs.active, transaction);
        _COORDINATOR.sign($groups[epochs.active], message);
    }

    /// @notice Attest to a transaction.
    function attestTransaction(uint64 epoch, bytes32 transactionHash, FROSTSignatureId.T signature) public {
        // Note that we do not impose a time limit for a transaction to be
        // attested to in the consensus contract. In theory, we have enough
        // space in our `Epochs` struct to also keep track of the previous epoch
        // and then we could check here that `epoch` is either `epochs.active`
        // or `epochs.previous`. This isn't a useful distinction, however: in
        // fact, if there is a reverted transaction with a valid FROST signature
        // onchain, then there is a valid attestation for the transaction
        // (regardless of whether or not this contract accepts it). Therefore,
        // it isn't useful for us to be restritive here.

        bytes32 message = domainSeparator().transactionProposal(epoch, transactionHash);
        _COORDINATOR.signatureVerify(signature, $groups[epoch], message);
        $attestations[message] = signature;
        emit TransactionAttested(message);
    }

    /// @inheritdoc IFROSTCoordinatorCallback
    function onKeyGenCompleted(FROSTGroupId.T group, bytes calldata context) external onlyCoordinator {
        (uint64 proposedEpoch, uint64 rolloverBlock) = abi.decode(context, (uint64, uint64));
        proposeEpoch(proposedEpoch, rolloverBlock, group);
    }

    /// @inheritdoc IFROSTCoordinatorCallback
    function onSignCompleted(FROSTSignatureId.T signature, bytes calldata context) external onlyCoordinator {
        // forge-lint: disable-next-line(unsafe-typecast)
        bytes4 selector = bytes4(context);
        if (selector == this.stageEpoch.selector) {
            (uint64 proposedEpoch, uint64 rolloverBlock, FROSTGroupId.T group) =
                abi.decode(context[4:], (uint64, uint64, FROSTGroupId.T));
            stageEpoch(proposedEpoch, rolloverBlock, group, signature);
        } else if (selector == this.attestTransaction.selector) {
            (uint64 epoch, bytes32 transactionHash) = abi.decode(context[4:], (uint64, bytes32));
            attestTransaction(epoch, transactionHash, signature);
        } else {
            revert UnknownSignatureSelector();
        }
    }

    function _processRollover() private returns (Epochs memory epochs) {
        epochs = $epochs;
        if (_epochsShouldRollover(epochs)) {
            epochs.active = epochs.staged;
            epochs.staged = 0;
            $epochs = epochs;
            // Note that we intentionally don't reset `$epochs.rolloverBlock` and
            // `$groups.staged` since the `$epochs.staged == 0` uniquely
            // determines whether or not there is staged rollover.
            emit EpochRolledOver(epochs.active);
        }
    }

    function _epochsShouldRollover(Epochs memory epochs) private view returns (bool result) {
        return epochs.staged != 0 && epochs.rolloverBlock <= block.number;
    }

    function _requireValidRollover(Epochs memory epochs, uint64 proposedEpoch, uint64 rolloverBlock) private view {
        require(epochs.active < proposedEpoch && rolloverBlock > block.number && epochs.staged == 0, InvalidRollover());
    }
}
