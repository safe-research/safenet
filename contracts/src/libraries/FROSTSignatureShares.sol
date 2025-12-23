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
     * @notice The main storage struct for tracking aggregated signature shares.
     * @custom:param aggregates Mapping from the Merkle root of a signing participant set to their aggregate signature
     *               data.
     * @dev This library's state is organized by the Merkle root of the signing participant set.
     *      Each `root` corresponds to a unique signing ceremony instance.
     */
    struct T {
        mapping(bytes32 root => Aggregate) aggregates;
    }

    /**
     * @notice Aggregated signature data for a specific group of signing participants.
     * @custom:param participants A mapping to track which participants have already submitted their
     *               shares for this ceremony, preventing replay or duplicate submissions.
     *               The value is the participant's leaf hash in the Merkle tree.
     * @custom:param signature The accumulated group signature, which is built up as each participant
     *               submits their share.
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
     * @notice Registers and aggregates a participant's signature share.
     * @param self The storage struct.
     * @param identifier The participant's FROST identifier.
     * @param share The participant's signature share to be aggregated.
     * @param r The group commitment for this signing ceremony.
     * @param root The Merkle root of the set of signing set for this signing ceremony.
     * @param proof The Merkle proof demonstrating the participant's inclusion in the signing set.
     * @return signature The updated, accumulated group signature after incorporating the new share.
     * @dev This function performs two key actions:
     *      1. Authorization: It verifies using a Merkle `proof` that the `identifier` is part of the set of signers
     *         defined by the `root`.
     *      2. Aggregation: It adds the participant's share to the collective group signature. The final group
     *         signature is `(R, z)` where
     *         `R = ∑ R_i` and `z = ∑ z_i`.
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
     * @notice Retrieves the current group signature for the specified commitment root.
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
