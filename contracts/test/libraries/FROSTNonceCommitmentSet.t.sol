// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {FROST} from "@/libraries/FROST.sol";
import {FROSTNonceCommitmentSet} from "@/libraries/FROSTNonceCommitmentSet.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";
import {NoncesChunkMerkleTree} from "@test/util/NoncesChunkMerkleTree.sol";

contract FROSTNonceCommitmentSetTest is Test {
    using FROSTNonceCommitmentSet for FROSTNonceCommitmentSet.T;

    FROSTNonceCommitmentSet.T nonces;

    function test_ConsecutiveCommit() public {
        FROST.Identifier me = FROST.Identifier.wrap(42);
        bytes32 commitment = keccak256("chunk");
        uint64 sequence = 1337;

        assertEq(nonces.commit(me, commitment, sequence), 1);
        assertEq(nonces.commit(me, commitment, sequence), 2);
    }

    function test_CannotVerifyPastSequences() public {
        FROST.Identifier me = FROST.Identifier.wrap(42);
        NoncesChunkMerkleTree.S[] memory ns = new NoncesChunkMerkleTree.S[](3);
        ns[0] = NoncesChunkMerkleTree.S({offset: 0, d: 1, e: 2});
        ns[1] = NoncesChunkMerkleTree.S({offset: 41, d: 3, e: 4});
        ns[2] = NoncesChunkMerkleTree.S({offset: 42, d: 5, e: 6});
        NoncesChunkMerkleTree chunk = new NoncesChunkMerkleTree(ns);

        uint64 chunkIndex = nonces.commit(me, chunk.root(), 42);
        assertEq(chunkIndex, 0);

        // We can use the nonce for the signing current signing sequence:
        (Secp256k1.Point memory d, Secp256k1.Point memory e, bytes32[] memory proof) = chunk.proof(42);
        this.callVerify(me, d, e, 42, proof);

        // However, verifying for previous (already started) sequences reverts:
        (d, e, proof) = chunk.proof(0);
        vm.expectRevert(FROSTNonceCommitmentSet.NotIncluded.selector);
        this.callVerify(me, d, e, 0, proof);
        (d, e, proof) = chunk.proof(41);
        vm.expectRevert(FROSTNonceCommitmentSet.NotIncluded.selector);
        this.callVerify(me, d, e, 41, proof);

        uint64 nextChunkIndex = nonces.commit(me, chunk.root(), 42);
        assertEq(nextChunkIndex, 1);

        // However, if we commit after we have already commited to the chunk for
        // the current sequence, then we will automatically commit to a future
        // chunk and can use all nonces from it.
        (d, e, proof) = chunk.proof(0);
        this.callVerify(me, d, e, 1024, proof);
        (d, e, proof) = chunk.proof(41);
        this.callVerify(me, d, e, 1024 + 41, proof);
        (d, e, proof) = chunk.proof(42);
        this.callVerify(me, d, e, 1024 + 42, proof);
    }

    function callVerify(
        FROST.Identifier identifier,
        Secp256k1.Point memory d,
        Secp256k1.Point memory e,
        uint64 sequence,
        bytes32[] calldata proof
    ) external view {
        nonces.verify(identifier, d, e, sequence, proof);
    }
}
