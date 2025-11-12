// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {MerkleTreeBase} from "@test/util/MerkleTreeBase.sol";
import {Secp256k1} from "@/lib/Secp256k1.sol";

contract CommitmentShareMerkleTree is MerkleTreeBase {
    struct S {
        uint256 index;
        Secp256k1.Point r;
        uint256 cl;
    }

    // forge-lint: disable-start(mixed-case-variable)
    mapping(uint256 x => mapping(uint256 y => bytes32 digest)) private $tree;
    bytes32 private $root;
    uint256 private $height;
    // forge-lint: disable-end(mixed-case-variable)

    constructor(S[] memory shares) {
        uint256 last = 0;
        for (uint256 i = 0; i < shares.length; i++) {
            S memory share = shares[i];
            _leaf(keccak256(abi.encode(share.index, share.r.x, share.r.y, share.cl)));

            assert(share.index > last);
            last = share.index;
        }
        _build();
    }

    function proof(uint256 x) external view returns (bytes32[] memory result) {
        return _proof(x);
    }
}
