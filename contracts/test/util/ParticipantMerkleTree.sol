// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {MerkleTreeBase} from "@test/util/MerkleTreeBase.sol";

contract ParticipantMerkleTree is MerkleTreeBase {
    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(address participant => uint256 index) private $indexes;
    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(uint256 index => address participant) private $addresses;

    constructor(address[] memory participants) {
        address last = address(0);
        for (uint256 i = 0; i < participants.length; i++) {
            // Note that for FROST, participant indexes start from 1.
            uint256 index = i + 1;
            address participant = participants[i];
            $indexes[participant] = index;
            $addresses[index] = participant;
            _leaf(keccak256(abi.encode(index, participant)));

            assert(participant > last);
            last = participant;
        }
        _build();
    }

    function addr(uint256 index) external view returns (address participant) {
        participant = $addresses[index];
        assert(participant != address(0));
    }

    function proof(uint256 index) external view returns (address participant, bytes32[] memory poap) {
        participant = $addresses[index];
        poap = _proof(index - 1);
        assert(participant != address(0));
    }

    function proof(address participant) external view returns (uint256 index, bytes32[] memory poap) {
        index = $indexes[participant];
        poap = _proof(index - 1);
    }
}
