// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {CheckerOracle} from "@/CheckerOracle.sol";

/**
 * @notice Deploys a CheckerOracle (WardensGame) instance.
 *
 * Required environment variables:
 *   ARBITRATOR   — Foundation address for governance and arbitration.
 *   FEE_TOKEN    — ERC-20 token address for fees and bonds.
 *   REQUEST_FEE  — Fixed fee (in token base units) pulled from the proposer per request.
 *
 * Optional environment variables (defaults shown):
 *   VOTING_WINDOW     — Voting duration in blocks (default: 12 ≈ 1 min on Gnosis Chain).
 *   GOVERNANCE_DELAY  — Block delay for checker/multiplier changes (default: 100).
 *   BOND_MULTIPLIER   — Initial bond multiplier; bondTarget = fee × multiplier (default: 50).
 */
contract DeployCheckerOracleScript is Script {
    function run() public returns (CheckerOracle checkerOracle) {
        address arbitrator = vm.envAddress("ARBITRATOR");
        address feeToken = vm.envAddress("FEE_TOKEN");
        uint256 requestFee = vm.envUint("REQUEST_FEE");

        uint256 votingWindow = vm.envOr("VOTING_WINDOW", uint256(12));
        uint256 governanceDelay = vm.envOr("GOVERNANCE_DELAY", uint256(100));
        uint256 bondMultiplier = vm.envOr("BOND_MULTIPLIER", uint256(50));

        vm.startBroadcast();

        checkerOracle = new CheckerOracle(
            arbitrator,
            feeToken,
            requestFee,
            votingWindow,
            governanceDelay,
            bondMultiplier
        );

        vm.stopBroadcast();

        console.log("CheckerOracle:", address(checkerOracle));
        console.log("ARBITRATOR:", arbitrator);
        console.log("FEE_TOKEN:", feeToken);
        console.log("REQUEST_FEE:", requestFee);
        console.log("VOTING_WINDOW:", votingWindow);
        console.log("GOVERNANCE_DELAY:", governanceDelay);
        console.log("bondMultiplier:", bondMultiplier);
    }
}
