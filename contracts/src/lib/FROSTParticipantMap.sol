// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Hashes} from "@oz/utils/cryptography/Hashes.sol";
import {MerkleProof} from "@oz/utils/cryptography/MerkleProof.sol";
import {Secp256k1} from "@/lib/Secp256k1.sol";

/// @title FROST Participant Map
/// @notice A mapping of FROST participants to their indexes and verification
///         shares.
library FROSTParticipantMap {
    using Secp256k1 for Secp256k1.Point;

    struct T {
        bytes32 root;
        mapping(address participant => uint256 index) indexes;
        mapping(uint256 index => Secp256k1.Point) keys;
    }

    error InvalidRootHash();
    error AlreadyInitialized();
    error InvalidIndex();
    error AlreadyRegistered();
    error NotParticipating();
    error AlreadySet();

    /// @notice Initializes a merkle map with a root.
    function init(T storage self, bytes32 root) internal {
        require(root != bytes32(0), InvalidRootHash());
        require(self.root == bytes32(0), AlreadyInitialized());
        self.root = root;
    }

    /// @notice Returns whether or not a group is initialized. A sealed group is
    ///         not considered initialized.
    function initialized(T storage self) internal view returns (bool result) {
        return self.root != bytes32(0);
    }

    /// @notice Registers a particpant to the merkle tree for the specified index.
    function register(T storage self, uint256 index, address participant, bytes32[] calldata poap) internal {
        require(index != 0, InvalidIndex());
        require(self.indexes[participant] == 0, AlreadyRegistered());
        bytes32 leaf = Hashes.efficientKeccak256(bytes32(index), bytes32(uint256(uint160(participant))));
        require(MerkleProof.verifyCalldata(poap, self.root, leaf), NotParticipating());
        self.indexes[participant] = index;
    }

    /// @notice Sets the participant's verification share.
    function set(T storage self, address participant, Secp256k1.Point memory y) internal returns (uint256 index) {
        y.requireNonZero();
        index = indexOf(self, participant);
        Secp256k1.Point storage key = self.keys[index];
        require(key.x | key.y == 0, AlreadySet());
        key.x = y.x;
        key.y = y.y;
    }

    /// @notice Gets the participant's index.
    function indexOf(T storage self, address participant) internal view returns (uint256 index) {
        index = self.indexes[participant];
        require(index != 0, NotParticipating());
    }

    /// @notice Gets the participants verification share.
    function getKey(T storage self, uint256 index) internal view returns (Secp256k1.Point memory y) {
        y = self.keys[index];
        require(y.x | y.y != 0, NotParticipating());
    }
}
