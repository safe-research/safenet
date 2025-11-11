// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {MerkleTreeBase} from "@test/util/MerkleTreeBase.sol";
import {Secp256k1} from "@/lib/Secp256k1.sol";

abstract contract MerkleTreeBase {
    // forge-lint: disable-start(mixed-case-variable)
    mapping(uint256 x => mapping(uint256 y => bytes32 digest)) private $tree;
    bytes32 private $root;
    uint256 private $width;
    uint256 private $height;
    // forge-lint: disable-end(mixed-case-variable)

    function root() external view returns (bytes32 r) {
        return $root;
    }

    function _leaf(bytes32 value) internal {
        assert($root == bytes32(0));

        uint256 x = $width++;
        $tree[x][0] = value;
    }

    function _build() internal {
        assert($root == bytes32(0));

        uint256 l = $width;
        uint256 y = 0;
        while (l > 1) {
            l = (l + 1) >> 1;
            y++;
            for (uint256 x = 0; x < l; x++) {
                uint256 xx = x << 1;
                uint256 yy = y - 1;

                bytes32 a = $tree[xx][yy];
                bytes32 b = $tree[xx + 1][yy];

                (bytes32 left, bytes32 right) = a < b ? (a, b) : (b, a);
                $tree[x][y] = keccak256(abi.encode(left, right));
            }
        }

        assert(l > 0);
        $root = $tree[0][y];
        $height = y;
    }

    function _proof(uint256 x) internal view returns (bytes32[] memory proof) {
        assert($root != bytes32(0));
        assert($width > x);

        proof = new bytes32[]($height);
        for (uint256 y = 0; y < proof.length; y++) {
            proof[y] = $tree[x ^ 1][y];
            x >>= 1;
        }
    }
}
