// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Hashes} from "@oz/utils/cryptography/Hashes.sol";
import {MerkleProof} from "@oz/utils/cryptography/MerkleProof.sol";
import {FROST} from "@/lib/FROST.sol";
import {Secp256k1} from "@/lib/Secp256k1.sol";

/// @title FROST Participant Map
/// @notice A mapping of FROST participants to their identifiers and public
///         verification shares.
library FROSTParticipantMap {
    using FROST for FROST.Identifier;
    using Secp256k1 for Secp256k1.Point;

    struct T {
        bytes32 root;
        mapping(address participant => FROST.Identifier) identifiers;
        mapping(FROST.Identifier => Secp256k1.Point) keys;
    }

    error InvalidRootHash();
    error AlreadyInitialized();
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

    /// @notice Registers a participant to the merkle tree.
    function register(T storage self, FROST.Identifier identifier, address participant, bytes32[] calldata poap)
        internal
    {
        identifier.requireValidIdentifier();
        require(FROST.Identifier.unwrap(self.identifiers[participant]) == 0, AlreadyRegistered());
        bytes32 leaf = Hashes.efficientKeccak256(
            bytes32(FROST.Identifier.unwrap(identifier)), bytes32(uint256(uint160(participant)))
        );
        require(MerkleProof.verifyCalldata(poap, self.root, leaf), NotParticipating());
        self.identifiers[participant] = identifier;
    }

    /// @notice Sets the participant's verification share.
    function set(T storage self, address participant, Secp256k1.Point memory y)
        internal
        returns (FROST.Identifier identifier)
    {
        y.requireNonZero();
        identifier = identifierOf(self, participant);
        Secp256k1.Point storage key = self.keys[identifier];
        require(key.x | key.y == 0, AlreadySet());
        key.x = y.x;
        key.y = y.y;
    }

    /// @notice Gets the participant's identifier.
    function identifierOf(T storage self, address participant) internal view returns (FROST.Identifier identifier) {
        identifier = self.identifiers[participant];
        require(FROST.Identifier.unwrap(identifier) != 0, NotParticipating());
    }

    /// @notice Gets the participants verification share.
    function getKey(T storage self, FROST.Identifier identifier) internal view returns (Secp256k1.Point memory y) {
        y = self.keys[identifier];
        require(y.x | y.y != 0, NotParticipating());
    }
}
