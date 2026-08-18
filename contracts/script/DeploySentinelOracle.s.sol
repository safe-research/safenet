// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {SafeCast} from "@oz/utils/math/SafeCast.sol";
import {DeterministicDeployment} from "@script/util/DeterministicDeployment.sol";
import {getFactory} from "@script/util/GetFactory.sol";
import {SentinelOracle} from "@/SentinelOracle.sol";

contract DeploySentinelOracleScript is Script {
    using DeterministicDeployment for DeterministicDeployment.Factory;
    using SafeCast for uint256;

    function run() public returns (address sentinelOracle) {
        SentinelOracle.ConstructorParams memory config = SentinelOracle.ConstructorParams({
            arbitrator: vm.envAddress("SENTINEL_ARBITRATOR"),
            governance: vm.envAddress("SENTINEL_GOVERNANCE"),
            protocolFundsReceiver: vm.envAddress("SENTINEL_PROTOCOL_FUNDS_RECEIVER"),
            proposer: vm.envAddress("SENTINEL_CONSENSUS"),
            feeToken: vm.envAddress("SENTINEL_FEE_TOKEN"),
            requestFee: vm.envUint("SENTINEL_REQUEST_FEE").toUint96(),
            initialBondMultiplier: vm.envUint("SENTINEL_BOND_MULTIPLIER").toUint32(),
            initialSlashingMultiplier: vm.envUint("SENTINEL_INITIAL_SLASHING_MULTIPLIER").toUint32(),
            initialDaoFeeShare: vm.envUint("SENTINEL_INITIAL_DAO_FEE_SHARE").toUint24(),
            commitWindow: vm.envUint("SENTINEL_COMMIT_WINDOW").toUint32(),
            revealWindow: vm.envUint("SENTINEL_REVEAL_WINDOW").toUint32(),
            governanceDelay: vm.envUint("SENTINEL_GOVERNANCE_DELAY").toUint32(),
            arbitrationTimeout: vm.envUint("SENTINEL_ARBITRATION_TIMEOUT").toUint32(),
            initialCharterEns: vm.envString("SENTINEL_CHARTER_ENS")
        });

        DeterministicDeployment.Factory factory = getFactory(vm);

        vm.startBroadcast();

        sentinelOracle = factory.deployWithArgs(bytes32(0), type(SentinelOracle).creationCode, abi.encode(config));

        vm.stopBroadcast();

        console.log("SentinelOracle deployed at:", sentinelOracle);
    }
}
