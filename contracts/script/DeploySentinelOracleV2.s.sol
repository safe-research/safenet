// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {DeterministicDeployment} from "@script/util/DeterministicDeployment.sol";
import {getFactory} from "@script/util/GetFactory.sol";
import {SentinelOracleV2} from "@/SentinelOracleV2.sol";

// TODO(A4): rename to DeploySentinelOracle.s.sol once the V1 contract/script are removed.
contract DeploySentinelOracleV2Script is Script {
    using DeterministicDeployment for DeterministicDeployment.Factory;

    function run() public returns (address sentinelOracle) {
        address arbitrator = vm.envAddress("SENTINEL_ARBITRATOR");
        address consensus = vm.envAddress("SENTINEL_CONSENSUS");
        address feeToken = vm.envAddress("SENTINEL_FEE_TOKEN");
        uint256 requestFee = vm.envUint("SENTINEL_REQUEST_FEE");
        uint256 commitWindow = vm.envUint("SENTINEL_COMMIT_WINDOW");
        uint256 revealWindow = vm.envUint("SENTINEL_REVEAL_WINDOW");
        uint256 governanceDelay = vm.envUint("SENTINEL_GOVERNANCE_DELAY");
        uint256 bondMultiplier = vm.envUint("SENTINEL_BOND_MULTIPLIER");

        DeterministicDeployment.Factory factory = getFactory(vm);

        vm.startBroadcast();

        sentinelOracle = factory.deployWithArgs(
            bytes32(0),
            type(SentinelOracleV2).creationCode,
            abi.encode(
                arbitrator, consensus, feeToken, requestFee, commitWindow, revealWindow, governanceDelay, bondMultiplier
            )
        );

        vm.stopBroadcast();

        console.log("SentinelOracleV2 deployed at:", sentinelOracle);
    }
}
