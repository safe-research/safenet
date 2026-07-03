// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {MockCoordinator} from "@test/util/MockCoordinator.sol";
import {Consensus} from "@/Consensus.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";

contract DeployTestConsensusScript is Script {
    function run() public returns (address consensus) {
        vm.startBroadcast();

        // The real `Consensus`, backed by a coordinator that no-ops `sign()`.
        // Sentinels only need `OracleTransactionProposed` and the resulting
        // `postRequest` call; they never verify a FROST signature, so the
        // genesis group id is an arbitrary placeholder — `MockCoordinator`
        // never checks it against real group state.
        MockCoordinator coordinator = new MockCoordinator();
        consensus = address(new Consensus(address(coordinator), FROSTGroupId.T.wrap(bytes32(0))));

        vm.stopBroadcast();

        console.log("Consensus deployed at:", consensus);
    }
}
