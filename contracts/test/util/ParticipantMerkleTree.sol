// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {MerkleTreeBase} from "@test/util/MerkleTreeBase.sol";

contract ParticipantMerkleTree is MerkleTreeBase {
    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(uint256 i => address participant) private $addresses;

    constructor(address[] memory participants) {
        address last = address(0);
        for (uint256 i = 0; i < participants.length; i++) {
            address participant = participants[i];
            $addresses[i] = participant;
            _leaf(bytes32(uint256(uint160(participant))));

            assert(participant > last);
            last = participant;
        }
        _build();
    }

    function addr(uint256 i) public view returns (address participant) {
        participant = $addresses[i];
        assert(participant != address(0));
    }

    function proof(uint256 i) external view returns (address participant, bytes32[] memory poap) {
        participant = addr(i);
        poap = _proof(i);
    }
}
