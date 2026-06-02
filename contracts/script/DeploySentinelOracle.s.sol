// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {SentinelOracle} from "@/SentinelOracle.sol";

contract DeploySentinelOracleScript is Script {
    function run() public returns (address sentinelOracle) {
        address arbitrator = vm.envAddress("SENTINEL_ARBITRATOR");
        address consensus = vm.envAddress("SENTINEL_CONSENSUS");
        address feeToken = vm.envAddress("SENTINEL_FEE_TOKEN");
        uint256 requestFee = vm.envUint("SENTINEL_REQUEST_FEE");
        uint256 votingWindow = vm.envUint("SENTINEL_VOTING_WINDOW");
        uint256 governanceDelay = vm.envUint("SENTINEL_GOVERNANCE_DELAY");
        uint256 bondMultiplier = vm.envUint("SENTINEL_BOND_MULTIPLIER");

        vm.startBroadcast();

        sentinelOracle = address(
            new SentinelOracle(
                arbitrator, consensus, feeToken, requestFee, votingWindow, governanceDelay, bondMultiplier
            )
        );

        vm.stopBroadcast();

        console.log("SentinelOracle deployed at:", sentinelOracle);
    }
}
