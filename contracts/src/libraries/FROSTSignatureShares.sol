// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {MerkleProof} from "@oz/utils/cryptography/MerkleProof.sol";
import {FROST} from "@/libraries/FROST.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

/// @title FROST Signature Shares
/// @notice Accumulate FROST signature shares by commitment root.
library FROSTSignatureShares {
    struct T {
        mapping(bytes32 root => Aggregate) aggregates;
    }

    struct Aggregate {
        mapping(FROST.Identifier => bytes32) participants;
        FROST.Signature signature;
    }

    error AlreadyIncluded();
    error NotIncluded();
    error IncompleteSigning();

    /// @notice Registers a commitment share for a participant.
    function register(
        T storage self,
        FROST.Identifier identifier,
        FROST.SignatureShare memory share,
        Secp256k1.Point memory r,
        bytes32 root,
        bytes32[] calldata proof
    ) internal returns (FROST.Signature memory signature) {
        Aggregate storage aggregate = self.aggregates[root];
        require(aggregate.participants[identifier] == bytes32(0), AlreadyIncluded());
        bytes32 leaf = _hash(identifier, share, r);
        require(MerkleProof.verifyCalldata(proof, root, leaf), NotIncluded());
        aggregate.participants[identifier] = leaf;
        signature.r = Secp256k1.add(aggregate.signature.r, share.r);
        signature.z = addmod(aggregate.signature.z, share.z, Secp256k1.N);
        aggregate.signature = signature;
    }

    /// @notice Retrieves the current group signature for the specified
    ///         commitment root.
    function groupSignature(T storage self, bytes32 root) internal view returns (FROST.Signature memory signature) {
        return self.aggregates[root].signature;
    }

    function _hash(FROST.Identifier identifier, FROST.SignatureShare memory share, Secp256k1.Point memory r)
        private
        pure
        returns (bytes32 digest)
    {
        Secp256k1.Point memory ri = share.r;
        uint256 l = share.l;
        assembly ("memory-safe") {
            let ptr := mload(0x40)
            mstore(ptr, identifier)
            mcopy(add(ptr, 0x20), ri, 0x40)
            mstore(add(ptr, 0x60), l)
            mcopy(add(ptr, 0x80), r, 0x40)
            digest := keccak256(ptr, 0xc0)
        }
    }
}
