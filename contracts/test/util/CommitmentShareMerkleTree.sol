// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {MerkleTreeBase} from "@test/util/MerkleTreeBase.sol";
import {FROST} from "@/libraries/FROST.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

contract CommitmentShareMerkleTree is MerkleTreeBase {
    struct S {
        FROST.Identifier identifier;
        Secp256k1.Point r;
        uint256 l;
    }

    // forge-lint: disable-start(mixed-case-variable)
    mapping(uint256 x => mapping(uint256 y => bytes32 digest)) private $tree;
    bytes32 private $root;
    uint256 private $height;
    // forge-lint: disable-end(mixed-case-variable)

    constructor(Secp256k1.Point memory r, S[] memory shares) {
        uint256 last = 0;
        for (uint256 i = 0; i < shares.length; i++) {
            S memory share = shares[i];
            _leaf(keccak256(abi.encode(share.identifier, share.r.x, share.r.y, share.l, r.x, r.y)));

            assert(FROST.Identifier.unwrap(share.identifier) > last);
            last = FROST.Identifier.unwrap(share.identifier);
        }
        _build();
    }

    function proof(uint256 x) external view returns (bytes32[] memory result) {
        return _proof(x);
    }
}
