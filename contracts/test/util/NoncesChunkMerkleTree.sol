// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Secp256k1} from "@/libraries/Secp256k1.sol";
import {ForgeSecp256k1} from "@test/util/ForgeSecp256k1.sol";
import {MerkleTreeBase} from "@test/util/MerkleTreeBase.sol";

contract NoncesChunkMerkleTree is MerkleTreeBase {
    using ForgeSecp256k1 for ForgeSecp256k1.P;

    struct S {
        uint256 offset;
        uint256 d;
        uint256 e;
    }

    struct Entry {
        uint256 d;
        uint256 e;
        uint256 x;
    }

    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(uint256 offset => Entry) private $entries;

    constructor(S[] memory nonces) {
        for (uint256 x = 0; x < nonces.length; x++) {
            S memory n = nonces[x];
            $entries[n.offset] = Entry({d: n.d, e: n.e, x: x});
            ForgeSecp256k1.P memory d = ForgeSecp256k1.g(n.d);
            ForgeSecp256k1.P memory e = ForgeSecp256k1.g(n.e);
            _leaf(keccak256(abi.encode(n.offset, d.x(), d.y(), e.x(), e.y())));

            assert(n.d != 0 && n.e != 0);
        }
        _buildWithHeight(10);
    }

    function proof(uint256 offset)
        external
        returns (Secp256k1.Point memory d, Secp256k1.Point memory e, bytes32[] memory prf)
    {
        Entry memory entry = $entries[offset];
        assert(entry.d != 0 && entry.e != 0);
        d = ForgeSecp256k1.g(entry.d).toPoint();
        e = ForgeSecp256k1.g(entry.e).toPoint();
        prf = _proof(entry.x);
    }
}
