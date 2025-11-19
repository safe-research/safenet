// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {MerkleProof} from "@oz/utils/cryptography/MerkleProof.sol";
import {FROST} from "@/libraries/FROST.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

/// @title FROST Signature Shares
/// @notice Accumulate FROST signature shares by commitment root.
library FROSTSignatureShares {
    struct T {
        mapping(bytes32 root => Root) roots;
    }

    struct Root {
        mapping(FROST.Identifier => bytes32) participants;
        Secp256k1.Point r;
        uint256 z;
    }

    error AlreadyIncluded();
    error NotIncluded();

    /// @notice Registers a commitment share for a participant.
    function register(
        T storage self,
        bytes32 root,
        FROST.Identifier identifier,
        Secp256k1.Point memory r,
        uint256 z,
        uint256 cl,
        bytes32[] calldata proof
    ) internal {
        Root storage rt = self.roots[root];
        require(rt.participants[identifier] == bytes32(0), AlreadyIncluded());
        bytes32 leaf = _hash(identifier, r, cl);
        require(MerkleProof.verifyCalldata(proof, root, leaf), NotIncluded());
        rt.participants[identifier] = leaf;
        rt.r = Secp256k1.add(rt.r, r);
        rt.z = addmod(rt.z, z, Secp256k1.N);
    }

    /// @notice Retrieves the current group signature for the specified
    ///         commitment root.
    function groupSignature(T storage self, bytes32 root) internal view returns (FROST.Signature memory signature) {
        Root storage rt = self.roots[root];
        signature.r = rt.r;
        signature.z = rt.z;
    }

    function _hash(FROST.Identifier identifier, Secp256k1.Point memory r, uint256 cl)
        private
        pure
        returns (bytes32 digest)
    {
        assembly ("memory-safe") {
            let ptr := mload(0x40)
            mstore(ptr, identifier)
            mcopy(add(ptr, 0x20), r, 0x40)
            mstore(add(ptr, 0x60), cl)
            digest := keccak256(ptr, 0x80)
        }
    }
}
