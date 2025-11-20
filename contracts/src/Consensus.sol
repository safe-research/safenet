// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROSTCoordinator} from "@/FROSTCoordinator.sol";
import {IFROSTCoordinatorCallback} from "@/interfaces/IFROSTCoordinatorCallback.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

contract Consensus is IFROSTCoordinatorCallback {
    struct Epochs {
        uint64 active;
        uint64 staged;
        uint64 rolloverAt;
        uint64 _padding;
    }

    struct Groups {
        FROSTGroupId.T active;
        FROSTGroupId.T staged;
    }

    event EpochProposed(
        uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 timestamp, Secp256k1.Point groupKey
    );
    event EpochStaged(
        uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 timestamp, Secp256k1.Point groupKey
    );
    event EpochRolledOver(uint64 indexed newActiveEpoch);

    error InvalidRollover();
    error UnknownSignatureSelector();

    /// @custom:precomputed keccak256("EIP712Domain(uint256 chainId,address verifyingContract)
    bytes32 private constant _DOMAIN_TYPEHASH = hex"47e79534a245952e8b16893a336b85a3d9ea9fa8c573f3d803afb92a79469218";

    /// @custom:precomputed keccak256("EpochRollover(uint64 activeEpoch,uint64 proposedEpoch,uint64 rolloverAt,uint256 groupKeyX,uint256 groupKeyY)")
    bytes32 private constant _EPOCH_ROLLOVER_TYPEHASH =
        hex"0d6c6e1100d6b156230ad2ed7a6f7f9b1c2e03d20d5ed86d36323486bb3773a6";

    FROSTCoordinator private immutable _COORDINATOR;

    // forge-lint: disable-next-line(mixed-case-variable)
    Epochs private $epochs;
    // forge-lint: disable-next-line(mixed-case-variable)
    Groups private $groups;

    constructor(address coordinator, FROSTGroupId.T group) {
        _COORDINATOR = FROSTCoordinator(coordinator);
        $groups.active = group;
    }

    /// @notice Compute the EIP-712 domain separator used by the consensus
    ///         contract.
    function domainSeparator() public view returns (bytes32 result) {
        assembly ("memory-safe") {
            let ptr := mload(0x40)
            mstore(ptr, _DOMAIN_TYPEHASH)
            mstore(add(ptr, 0x20), chainid())
            mstore(add(ptr, 0x40), address())
            result := keccak256(ptr, 0x60)
        }
    }

    /// @notice Gets the active epoch and its group ID.
    function getActiveEpoch() external view returns (uint64 epoch, FROSTGroupId.T group) {
        Epochs memory epochs = $epochs;
        if (_epochsShouldRollover(epochs)) {
            epoch = epochs.staged;
            group = $groups.staged;
        } else {
            epoch = epochs.active;
            group = $groups.active;
        }
    }

    /// @notice Proposes a new epoch that to be rolled over to.
    function proposeEpoch(uint64 proposedEpoch, uint64 rolloverAt, FROSTGroupId.T group) public {
        (Epochs memory epochs, FROSTGroupId.T activeGroup) = _processRollover();
        _requireValidRollover(epochs, proposedEpoch, rolloverAt);
        Secp256k1.Point memory groupKey = _COORDINATOR.groupKey(group);
        bytes32 message = epochRolloverMessage(epochs.active, proposedEpoch, rolloverAt, groupKey);
        emit EpochProposed(epochs.active, proposedEpoch, rolloverAt, groupKey);
        _COORDINATOR.sign(activeGroup, message);
    }

    /// @notice Stages an epoch to automatically rollover.
    function stageEpoch(uint64 proposedEpoch, uint64 rolloverAt, FROSTGroupId.T group, FROSTSignatureId.T signature)
        public
    {
        (Epochs memory epochs, FROSTGroupId.T activeGroup) = _processRollover();
        _requireValidRollover(epochs, proposedEpoch, rolloverAt);
        Secp256k1.Point memory groupKey = _COORDINATOR.groupKey(group);
        bytes32 message = epochRolloverMessage(epochs.active, proposedEpoch, rolloverAt, groupKey);
        _COORDINATOR.signatureVerify(signature, activeGroup, message);
        $epochs = Epochs({active: epochs.active, staged: proposedEpoch, rolloverAt: rolloverAt, _padding: 0});
        $groups.staged = group;
        emit EpochStaged(epochs.active, proposedEpoch, rolloverAt, groupKey);
    }

    function onKeyGenCompleted(FROSTGroupId.T group, bytes calldata context) external {
        (uint64 proposedEpoch, uint64 rolloverAt) = abi.decode(context, (uint64, uint64));
        proposeEpoch(proposedEpoch, rolloverAt, group);
    }

    function onSignCompleted(FROSTSignatureId.T signature, bytes calldata context) external {
        // forge-lint: disable-next-line(unsafe-typecast)
        bytes4 selector = bytes4(context);
        if (selector == this.stageEpoch.selector) {
            (uint64 proposedEpoch, uint64 rolloverAt, FROSTGroupId.T group) =
                abi.decode(context[4:], (uint64, uint64, FROSTGroupId.T));
            stageEpoch(proposedEpoch, rolloverAt, group, signature);
        } else if (selector == "sign") {
            // TODO: post safe transaction signature
        } else {
            revert UnknownSignatureSelector();
        }
    }

    function epochRolloverMessage(
        uint64 activeEpoch,
        uint64 proposedEpoch,
        uint64 rolloverAt,
        Secp256k1.Point memory groupKey
    ) public view returns (bytes32 message) {
        bytes32 domain = domainSeparator();
        assembly ("memory-safe") {
            let ptr := mload(0x40)
            mstore(ptr, _EPOCH_ROLLOVER_TYPEHASH)
            mstore(add(ptr, 0x20), activeEpoch)
            mstore(add(ptr, 0x40), proposedEpoch)
            mstore(add(ptr, 0x60), rolloverAt)
            mcopy(add(ptr, 0x80), groupKey, 0x40)
            mstore(add(ptr, 0x22), keccak256(ptr, 0xc0))
            mstore(ptr, hex"1901")
            mstore(add(ptr, 0x02), domain)
            message := keccak256(ptr, 0x42)
        }
    }

    function _processRollover() private returns (Epochs memory epochs, FROSTGroupId.T activeGroup) {
        epochs = $epochs;
        if (_epochsShouldRollover(epochs)) {
            epochs.active = epochs.staged;
            epochs.staged = 0;
            $epochs = epochs;
            activeGroup = $groups.staged;
            $groups.active = activeGroup;
            // Note that we intentionally don't reset `$epochs.rolloverAt` and
            // `$groups.staged` since the `$epochs.staged == 0` uniquely
            // determines whether or not there is staged rollover.
            emit EpochRolledOver(epochs.active);
        } else {
            activeGroup = $groups.active;
        }
    }

    function _epochsShouldRollover(Epochs memory epochs) private view returns (bool result) {
        return epochs.staged != 0 && epochs.rolloverAt <= block.timestamp;
    }

    function _requireValidRollover(Epochs memory epochs, uint64 proposedEpoch, uint64 rolloverAt) private view {
        require(epochs.active < proposedEpoch && rolloverAt > block.timestamp && epochs.staged == 0, InvalidRollover());
    }
}
