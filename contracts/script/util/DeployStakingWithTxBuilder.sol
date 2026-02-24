// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {Staking} from "../../src/Staking.sol";
import {DeterministicDeployment} from "./DeterministicDeployment.sol";
import {getStakingDeploymentParameters} from "./GetStakingContract.sol";
import {verifyStakingCommand} from "./VerifyStaking.sol";

contract DeployStakingWithTxBuilderScript is Script {
    using DeterministicDeployment for DeterministicDeployment.Factory;

    function run() public {
        // Required script arguments:
        (
            address initialOwner,
            address safeToken,
            uint128 initialWithdrawalDelay,
            uint256 configTimeDelay,
            DeterministicDeployment.Factory factory
        ) = getStakingDeploymentParameters(vm);
        uint256 chainId = vm.envOr("CHAIN_ID", uint256(1));

        // Safe Tx Builder uses millisecond timestamps in exports.
        uint256 createdAt = vm.unixTime() * 1000;
        // Build the txData (creation code + constructor args)
        bytes memory constructorArgs = abi.encode(initialOwner, safeToken, initialWithdrawalDelay, configTimeDelay);
        bytes memory txData = abi.encodePacked(bytes32(0), type(Staking).creationCode, constructorArgs);

        // Compute the tx-builder checksum from the canonical serialization (sorted keys, custom object format).
        string memory checksum = _calculateTxBuilderChecksum(createdAt, chainId, txData, factory);
        console.log("Checksum:", checksum);

        {
            // Emit a tx-builder compatible batch file with a precomputed checksum.
            string memory path = "build/staking-deployment.json";

            string memory transaction = "tx";
            vm.serializeJson(transaction, '{"contractMethod": null, "contractInputsValues": null}');
            vm.serializeAddress(transaction, "to", factory.addr());
            vm.serializeString(transaction, "value", "0");
            transaction = vm.serializeBytes(transaction, "data", txData);

            string memory meta = "meta";
            vm.serializeString(meta, "name", "Staking Deployment");
            vm.serializeString(meta, "description", "Safenet Staking Beta Deployment Transaction");
            vm.serializeString(meta, "txBuilderVersion", "1.18.3");
            meta = vm.serializeString(meta, "checksum", checksum);

            string memory root = "root";
            vm.serializeJson(root, string.concat('{"transactions": [', transaction, "]}"));
            vm.serializeString(root, "meta", meta);
            vm.serializeString(root, "version", "1.0");
            vm.serializeString(root, "chainId", vm.toString(chainId));
            root = vm.serializeUint(root, "createdAt", createdAt);

            // Write the JSON to the file system
            // forge-lint: disable-next-line(unsafe-cheatcode)
            vm.writeFile(path, root);
        }

        address predictedAddress =
            factory.deploymentAddressWithArgs(bytes32(0), type(Staking).creationCode, constructorArgs);
        console.log("Predicted Staking address:", predictedAddress);

        verifyStakingCommand(
            vm, predictedAddress, chainId, initialOwner, safeToken, initialWithdrawalDelay, configTimeDelay
        );
    }

    function _calculateTxBuilderChecksum(
        uint256 createdAt,
        uint256 chainId,
        bytes memory txData,
        DeterministicDeployment.Factory factory
    ) internal pure returns (string memory) {
        // Follows apps/tx-builder/src/lib/checksum.ts serialization rules.
        // Note: meta.name is forced to null for checksum calculation.
        string memory serialized = string.concat(
            '{["chainId","createdAt","meta","transactions","version"]"',
            vm.toString(chainId), // Chain ID
            '",',
            vm.toString(createdAt),
            ',{["description","name","txBuilderVersion"]"',
            "Safenet Staking Beta Deployment Transaction",
            '",',
            "null",
            ',"',
            "1.18.3", // Tx Builder version
            '",},',
            '[{["contractInputsValues","contractMethod","data","to","value"]',
            'null,null,"', // contract inputs and method
            vm.toString(txData),
            '","',
            vm.toString(factory.addr()), // Factory address
            '","',
            "0", // Value
            '",}],"',
            "1.0", // Tx Builder batch format version
            '",}'
        );

        return vm.toString(keccak256(bytes(serialized)));
    }
}
