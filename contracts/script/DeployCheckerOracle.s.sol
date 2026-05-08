// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {CheckerOracle} from "@/CheckerOracle.sol";

/**
 * @title Deploy CheckerOracle Script
 * @notice Deployment script for the CheckerOracle contract.
 */
contract DeployCheckerOracleScript is Script {
    function run() external returns (CheckerOracle checkerOracle) {
        // Required script arguments:
        address feeToken = vm.envAddress("FEE_TOKEN");
        address arbitrator = vm.envAddress("ARBITRATOR");

        // Optional script arguments:
        uint256 votingWindow = vm.envOr("VOTING_WINDOW", uint256(12)); // 12 blocks ≈ 1 minute on Gnosis Chain
        uint256 governanceDelay = vm.envOr("GOVERNANCE_DELAY", uint256(1)); // 1 block delay

        vm.startBroadcast();

        checkerOracle = new CheckerOracle(feeToken, arbitrator, votingWindow, governanceDelay);

        vm.stopBroadcast();

        console.log("CheckerOracle deployed at:", address(checkerOracle));
        console.log("Fee Token:", feeToken);
        console.log("Arbitrator:", arbitrator);
        console.log("Voting Window:", votingWindow, "blocks");
        console.log("Governance Delay:", governanceDelay, "blocks");
    }
}
