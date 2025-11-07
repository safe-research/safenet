// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {MerkleProof} from "@oz/utils/cryptography/MerkleProof.sol";
import {Secp256k1} from "@/lib/Secp256k1.sol";

/// @title FROST Commitment Set
/// @notice A set of nonce commitments for FROST signature ceremonies.
library FROSTCommitmentSet {
    using Secp256k1 for Secp256k1.Point;

    struct T {
        bytes32 participants;
        mapping(uint256 index => bytes32) commitments;
        uint256 count;
    }

    error AlreadyCommitted();
    error NotAuthorized();
    error NotIncluded();

    /// @notice Sets a Merkle root hash of the authorized participants for
    ///         contributing commitments.
    function authorize(T storage self, bytes32 participants) internal {
        self.participants = participants;
    }

    /// @notice Seals the commitment set, making it no longer possible to add
    ///         new commitments.
    function seal(T storage self) internal {
        self.participants = bytes32(uint256(1));
    }

    /// @notice Commits to a hiding nonce `d` and binding nonce `e`.
    function commit(
        T storage self,
        uint256 index,
        Secp256k1.Point memory d,
        Secp256k1.Point memory e,
        address participant,
        bytes32[] calldata authorization
    ) internal {
        d.requireNonZero();
        e.requireNonZero();
        require(self.commitments[index] == bytes32(0), AlreadyCommitted());
        bytes32 participants = self.participants;
        if (participants == bytes32(0)) {
            require(authorization.length == 0, NotAuthorized());
        } else {
            bytes32 leaf = bytes32(uint256(uint160(participant)));
            require(MerkleProof.verifyCalldata(authorization, participants, leaf), NotAuthorized());
        }
        self.commitments[index] = _hash(d, e);
        unchecked {
            self.count++;
        }
    }

    /// @notice Verifies that the specified commitment is part of the set.
    function verify(T storage self, uint256 index, Secp256k1.Point memory d, Secp256k1.Point memory e) internal view {
        require(self.commitments[index] == _hash(d, e), NotIncluded());
    }

    function _hash(Secp256k1.Point memory d, Secp256k1.Point memory e) private pure returns (bytes32 digest) {
        // forge-lint: disable-next-line(asm-keccak256)
        return keccak256(abi.encode(d.x, d.y, e.x, e.y));
    }
}
