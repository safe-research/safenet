// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {SafenetGuard} from "@/guard/SafenetGuard.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";
import {DeterministicDeployment} from "@script/util/DeterministicDeployment.sol";
import {getFactory} from "@script/util/GetFactory.sol";

contract DeploySafenetGuardScript is Script {
    using DeterministicDeployment for DeterministicDeployment.Factory;

    function run() public returns (address guard) {
        // Required script arguments:
        uint256 consensusChainId = vm.envUint("CONSENSUS_CHAIN_ID");
        address consensusAddress = vm.envAddress("CONSENSUS_ADDRESS");
        uint256 rawEpoch = vm.envUint("INITIAL_EPOCH");
        require(rawEpoch <= type(uint64).max, "INITIAL_EPOCH overflow: value exceeds uint64");
        // forge-lint: disable-next-line(unsafe-typecast)
        uint64 initialEpoch = uint64(rawEpoch);
        uint256 groupKeyX = vm.envUint("INITIAL_GROUP_KEY_X");
        uint256 groupKeyY = vm.envUint("INITIAL_GROUP_KEY_Y");

        // Optional script arguments:
        uint256 allowTxDelay = vm.envOr("ALLOW_TX_DELAY", uint256(3 days));
        uint256 allowTxWindow = vm.envOr("ALLOW_TX_WINDOW", uint256(1 hours));
        DeterministicDeployment.Factory factory = getFactory(vm);

        Secp256k1.Point memory initialGroupKey = Secp256k1.Point({x: groupKeyX, y: groupKeyY});

        vm.startBroadcast();

        guard = factory.deployWithArgs(
            bytes32(0),
            type(SafenetGuard).creationCode,
            abi.encode(consensusChainId, consensusAddress, initialEpoch, initialGroupKey, allowTxDelay, allowTxWindow)
        );

        vm.stopBroadcast();

        console.log("SafenetGuard deployed at:", guard);
    }
}
