// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {MerkleProof} from "@oz/utils/cryptography/MerkleProof.sol";
import {FROST} from "@/libraries/FROST.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

/**
 * @title FROST Signature Shares
 * @notice Accumulate FROST signature shares by commitment root.
 */
library FROSTSignatureShares {
    // ============================================================
    // STRUCTS
    // ============================================================

    /**
     * @notice The main storage struct for tracking signature shares.
     * @param aggregates Mapping from commitment root to aggregate data.
     */
    struct T {
        mapping(bytes32 root => Aggregate) aggregates;
    }

    /**
     * @notice Aggregated signature data for a commitment root.
     * @param participants Mapping from participant identifier to their leaf hash.
     * @param signature The accumulated group signature.
     */
    struct Aggregate {
        mapping(FROST.Identifier => bytes32) participants;
        FROST.Signature signature;
    }

    // ============================================================
    // ERRORS
    // ============================================================

    /**
     * @notice Thrown when a participant is already included in the aggregate.
     */
    error AlreadyIncluded();

    /**
     * @notice Thrown when a participant is not included in the Merkle tree.
     */
    error NotIncluded();

    /**
     * @notice Thrown when the signing process is incomplete.
     */
    error IncompleteSigning();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Registers a commitment share for a participant.
     * @param self The storage struct.
     * @param identifier The participant's FROST identifier.
     * @param share The participant's signature share.
     * @param r The group commitment point.
     * @param root The Merkle root for the commitment.
     * @param proof The Merkle proof for inclusion.
     * @return signature The accumulated group signature after registration.
     */
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

    /**
     * @notice Retrieves the current group signature for the specified
     *         commitment root.
     * @param self The storage struct.
     * @param root The commitment root.
     * @return signature The accumulated group signature.
     */
    function groupSignature(T storage self, bytes32 root) internal view returns (FROST.Signature memory signature) {
        return self.aggregates[root].signature;
    }

    // ============================================================
    // PRIVATE FUNCTIONS
    // ============================================================

    /**
     * @notice Computes the leaf hash for a participant's share.
     * @param identifier The participant's FROST identifier.
     * @param share The participant's signature share.
     * @param r The group commitment point.
     * @return digest The computed leaf hash.
     */
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
