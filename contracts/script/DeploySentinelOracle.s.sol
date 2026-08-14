// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {DeterministicDeployment} from "@script/util/DeterministicDeployment.sol";
import {getFactory} from "@script/util/GetFactory.sol";
import {SentinelOracle} from "@/SentinelOracle.sol";

contract DeploySentinelOracleScript is Script {
    using DeterministicDeployment for DeterministicDeployment.Factory;

    function run() public returns (address sentinelOracle) {
        address arbitrator = vm.envAddress("SENTINEL_ARBITRATOR");
        address governance = vm.envAddress("SENTINEL_GOVERNANCE");
        address protocolFundsReceiver = vm.envAddress("SENTINEL_PROTOCOL_FUNDS_RECEIVER");
        // The trusted contract allowed to call `postRequest` -- in production, the Consensus
        // contract itself, hence the env var name; the constructor field is `proposer` since that
        // is the role being filled, not necessarily always the Consensus contract.
        address proposer = vm.envAddress("SENTINEL_CONSENSUS");
        address feeToken = vm.envAddress("SENTINEL_FEE_TOKEN");
        uint256 requestFee = vm.envUint("SENTINEL_REQUEST_FEE");
        uint256 commitWindow = vm.envUint("SENTINEL_COMMIT_WINDOW");
        uint256 revealWindow = vm.envUint("SENTINEL_REVEAL_WINDOW");
        uint256 governanceDelay = vm.envUint("SENTINEL_GOVERNANCE_DELAY");
        uint256 bondMultiplier = vm.envUint("SENTINEL_BOND_MULTIPLIER");
        uint256 initialSlashingMultiplier = vm.envUint("SENTINEL_INITIAL_SLASHING_MULTIPLIER");
        uint256 initialDaoFeeShare = vm.envUint("SENTINEL_INITIAL_DAO_FEE_SHARE");
        string memory charterEns = vm.envString("SENTINEL_CHARTER_ENS");
        uint256 arbitrationTimeout = vm.envUint("SENTINEL_ARBITRATION_TIMEOUT");

        DeterministicDeployment.Factory factory = getFactory(vm);

        vm.startBroadcast();

        sentinelOracle = factory.deployWithArgs(
            bytes32(0),
            type(SentinelOracle).creationCode,
            abi.encode(
                arbitrator,
                governance,
                protocolFundsReceiver,
                proposer,
                feeToken,
                requestFee,
                commitWindow,
                revealWindow,
                governanceDelay,
                bondMultiplier,
                initialSlashingMultiplier,
                initialDaoFeeShare,
                charterEns,
                arbitrationTimeout
            )
        );

        vm.stopBroadcast();

        console.log("SentinelOracle deployed at:", sentinelOracle);
    }
}
