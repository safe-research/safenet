// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {Consensus} from "@/Consensus.sol";
import {SafeTransaction} from "@/libraries/SafeTransaction.sol";

contract ProposeScript is Script {
    function run() public {
        SafeTransaction.T memory transaction;

        // Required script arguments:
        address consensusAddress = vm.envAddress("CONSENSUS_ADDRESS");
        transaction.chainId = vm.envUint("TX_CHAIN_ID");
        transaction.safe = vm.envAddress("TX_SAFE");
        transaction.to = vm.envAddress("TX_TO");
        transaction.nonce = vm.envUint("TX_NONCE");

        // Optional script arguments:
        transaction.value = vm.envOr("TX_VALUE", uint256(0));
        transaction.data = vm.envOr("TX_DATA", bytes(""));
        transaction.operation = SafeTransaction.Operation(vm.envOr("TX_OPERATION", uint256(0)));
        transaction.safeTxGas = vm.envOr("TX_SAFE_TX_GAS", uint256(0));
        transaction.baseGas = vm.envOr("TX_BASE_GAS", uint256(0));
        transaction.gasPrice = vm.envOr("TX_GAS_PRICE", uint256(0));
        transaction.gasToken = vm.envOr("TX_GAS_TOKEN", address(0));
        transaction.refundReceiver = vm.envOr("TX_REFUND_RECEIVER", address(0));

        vm.startBroadcast();

        bytes32 safeTxHash = Consensus(consensusAddress).proposeTransaction(transaction);

        vm.stopBroadcast();

        console.log("Safe Transaction Hash:", vm.toString(safeTxHash));
    }
}
