// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {MerkleTreeBase} from "@test/util/MerkleTreeBase.sol";
import {FROST} from "@/lib/FROST.sol";

contract ParticipantMerkleTree is MerkleTreeBase {
    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(address participant => FROST.Identifier) private $identifiers;
    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(FROST.Identifier => address participant) private $addresses;

    constructor(address[] memory participants) {
        address last = address(0);
        for (uint256 i = 0; i < participants.length; i++) {
            // Note that for FROST, participant identifers start from 1.
            FROST.Identifier identifier = FROST.newIdentifier(i + 1);
            address participant = participants[i];
            $identifiers[participant] = identifier;
            $addresses[identifier] = participant;
            _leaf(keccak256(abi.encode(identifier, participant)));

            assert(participant > last);
            last = participant;
        }
        _build();
    }

    function addr(uint256 identifier) external view returns (address participant) {
        participant = $addresses[FROST.newIdentifier(identifier)];
        assert(participant != address(0));
    }

    function proof(uint256 identifier) external view returns (address participant, bytes32[] memory poap) {
        participant = $addresses[FROST.newIdentifier(identifier)];
        poap = _proof(identifier - 1);
        assert(participant != address(0));
    }
}
