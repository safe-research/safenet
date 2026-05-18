// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {SafenetCosigner} from "@/cosigner/SafenetCosigner.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";
import {DeterministicDeployment} from "@script/util/DeterministicDeployment.sol";
import {getFactory} from "@script/util/GetFactory.sol";

contract DeployCosignerScript is Script {
    using DeterministicDeployment for DeterministicDeployment.Factory;

    function run() public returns (address cosigner) {
        // Required script arguments:
        uint256 consensusChainId = vm.envUint("CONSENSUS_CHAIN_ID");
        address consensusAddress = vm.envAddress("CONSENSUS_ADDRESS");
        uint64 initialEpoch = uint64(vm.envUint("INITIAL_EPOCH"));
        uint256 groupKeyX = vm.envUint("INITIAL_GROUP_KEY_X");
        uint256 groupKeyY = vm.envUint("INITIAL_GROUP_KEY_Y");

        // Optional script arguments:
        uint256 allowTxDelay = vm.envOr("ALLOW_TX_DELAY", uint256(60));
        DeterministicDeployment.Factory factory = getFactory(vm);

        Secp256k1.Point memory initialGroupKey = Secp256k1.Point({x: groupKeyX, y: groupKeyY});

        vm.startBroadcast();

        cosigner = factory.deployWithArgs(
            bytes32(0),
            type(SafenetCosigner).creationCode,
            abi.encode(consensusChainId, consensusAddress, initialEpoch, initialGroupKey, allowTxDelay)
        );

        vm.stopBroadcast();

        console.log("SafenetCosigner deployed at:", cosigner);
    }
}
