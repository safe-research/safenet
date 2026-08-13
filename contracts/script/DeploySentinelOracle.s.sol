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
        address arbitrator = vm.envAddress("SENTINEL_ARBITRATOR");
        address governance = vm.envAddress("SENTINEL_GOVERNANCE");
        address protocolFundsReceiver = vm.envAddress("SENTINEL_PROTOCOL_FUNDS_RECEIVER");
        // The trusted contract allowed to call `postRequest` -- in production, the Consensus
        // contract itself, hence the env var name; the constructor field is `proposer` since that
        // is the role being filled, not necessarily always the Consensus contract.
        address proposer = vm.envAddress("SENTINEL_CONSENSUS");
        address feeToken = vm.envAddress("SENTINEL_FEE_TOKEN");
        uint96 requestFee = vm.envUint("SENTINEL_REQUEST_FEE").toUint96();
        uint32 commitWindow = vm.envUint("SENTINEL_COMMIT_WINDOW").toUint32();
        uint32 revealWindow = vm.envUint("SENTINEL_REVEAL_WINDOW").toUint32();
        uint32 governanceDelay = vm.envUint("SENTINEL_GOVERNANCE_DELAY").toUint32();
        uint32 bondMultiplier = vm.envUint("SENTINEL_BOND_MULTIPLIER").toUint32();
        uint32 initialSlashingMultiplier = vm.envUint("SENTINEL_INITIAL_SLASHING_MULTIPLIER").toUint32();
        uint24 initialDaoFeeShare = vm.envUint("SENTINEL_INITIAL_DAO_FEE_SHARE").toUint24();
        string memory charterEns = vm.envString("SENTINEL_CHARTER_ENS");
        uint32 arbitrationTimeout = vm.envUint("SENTINEL_ARBITRATION_TIMEOUT").toUint32();

        DeterministicDeployment.Factory factory = getFactory(vm);

        vm.startBroadcast();

        sentinelOracle = factory.deployWithArgs(
            bytes32(0),
            type(SentinelOracle).creationCode,
            abi.encode(
                SentinelOracle.ConstructorParams({
                    arbitrator: arbitrator,
                    governance: governance,
                    protocolFundsReceiver: protocolFundsReceiver,
                    proposer: proposer,
                    feeToken: feeToken,
                    requestFee: requestFee,
                    initialBondMultiplier: bondMultiplier,
                    initialSlashingMultiplier: initialSlashingMultiplier,
                    initialDaoFeeShare: initialDaoFeeShare,
                    commitWindow: commitWindow,
                    revealWindow: revealWindow,
                    governanceDelay: governanceDelay,
                    arbitrationTimeout: arbitrationTimeout,
                    initialCharterEns: charterEns
                })
            )
        );

        vm.stopBroadcast();

        console.log("SentinelOracle deployed at:", sentinelOracle);
    }
}
