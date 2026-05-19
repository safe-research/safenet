// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {SentinelOracle} from "@/SentinelOracle.sol";

contract DeploySentinelOracleScript is Script {
    function run() public returns (SentinelOracle oracle) {
        address arbitrator = vm.envAddress("ARBITRATOR");
        address consensus = vm.envAddress("CONSENSUS");
        address feeToken = vm.envAddress("FEE_TOKEN");
        uint256 requestFee = vm.envUint("REQUEST_FEE");
        uint256 votingWindow = vm.envUint("VOTING_WINDOW");
        uint256 governanceDelay = vm.envUint("GOVERNANCE_DELAY");
        uint256 initialMultiplier = vm.envUint("INITIAL_MULTIPLIER");

        vm.startBroadcast();

        oracle = new SentinelOracle(
            arbitrator, consensus, feeToken, requestFee, votingWindow, governanceDelay, initialMultiplier
        );

        vm.stopBroadcast();

        console.log("SentinelOracle deployed at:", address(oracle));
        console.log("Arbitrator:", arbitrator);
        console.log("Consensus:", consensus);
        console.log("FeeToken:", feeToken);
        console.log("RequestFee:", requestFee);
        console.log("VotingWindow:", votingWindow);
        console.log("GovernanceDelay:", governanceDelay);
        console.log("InitialMultiplier:", initialMultiplier);
    }
}
