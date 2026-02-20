// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {Staking} from "../../src/Staking.sol";
import {DeterministicDeployment} from "./DeterministicDeployment.sol";
import {getStackingDeploymentParameters} from "./GetStakingAddress.sol";

contract DeployStakingWithTxBuilderScript is Script {
    using DeterministicDeployment for DeterministicDeployment.Factory;

    function run() public {
        // Required script arguments:
        (address initialOwner, address safeToken, uint128 initialWithdrawalDelay, uint256 configTimeDelay,) =
            getStackingDeploymentParameters(vm);
        string memory chainId = vm.toString(vm.envOr("CHAIN_ID", uint256(1)));
        string memory factory =
            vm.toString(DeterministicDeployment.Factory.unwrap(DeterministicDeployment.SAFE_SINGLETON_FACTORY));

        // Safe Tx Builder uses millisecond timestamps in exports.
        string memory createdAtStr = vm.toString(vm.unixTime() * 1000);
        // Build the txData (creation code + constructor args)
        bytes memory constructorArgs = abi.encode(initialOwner, safeToken, initialWithdrawalDelay, configTimeDelay);
        string memory txData = vm.toString(abi.encodePacked(bytes32(0), type(Staking).creationCode, constructorArgs));

        // Compute the tx-builder checksum from the canonical serialization (sorted keys, custom object format).
        string memory checksum = _calculateTxBuilderChecksum(createdAtStr, chainId, txData, factory);

        {
            // Emit a tx-builder compatible batch file with a precomputed checksum.
            string memory path = "build/staking-deployment.json";
            string memory json = string.concat(
                '{"version":"1.0","chainId":"',
                chainId,
                '","createdAt":',
                createdAtStr,
                ',"meta":{"name":"',
                "Staking Deployment",
                '","description":"',
                "Safenet Staking Beta Deployment Transaction",
                '","txBuilderVersion":"',
                "1.18.3",
                '","checksum":"',
                checksum,
                '"},"transactions":[{"to":"',
                factory,
                '","value":"0","data":"',
                txData,
                '","contractMethod":null,"contractInputsValues":null}]}'
            );

            // Write the JSON to the file system
            // forge-lint: disable-next-line(unsafe-cheatcode)
            vm.writeFile(path, json);
        }

        address predictedAddress = DeterministicDeployment.SAFE_SINGLETON_FACTORY
            .deploymentAddressWithArgs(bytes32(0), type(Staking).creationCode, constructorArgs);
        console.log("Predicted Staking address:", predictedAddress);

        verifyCommand(
            predictedAddress, vm.parseUint(chainId), initialOwner, safeToken, initialWithdrawalDelay, configTimeDelay
        );
    }

    function _calculateTxBuilderChecksum(
        string memory createdAtStr,
        string memory chainId,
        string memory txData,
        string memory factory
    ) internal pure returns (string memory) {
        // Follows apps/tx-builder/src/lib/checksum.ts serialization rules.
        // Note: meta.name is forced to null for checksum calculation.
        string memory serialized = string.concat(
            '{["chainId","createdAt","meta","transactions","version"]"',
            chainId, // Chain ID
            '",',
            createdAtStr,
            ',{["description","name","txBuilderVersion"]"',
            "Safenet Staking Beta Deployment Transaction",
            '",',
            "null",
            ',"',
            "1.18.3", // Tx Builder version
            '",},',
            '[{["contractInputsValues","contractMethod","data","to","value"]',
            'null,null,"',
            txData,
            '","',
            factory, // Factory address
            '","',
            "0", // Value
            '",}],"',
            "1.0", // Tx Builder batch format version
            '",}'
        );

        return vm.toString(keccak256(bytes(serialized)));
    }

    function verifyCommand(
        address stakingAddress,
        uint256 chainId,
        address initialOwner,
        address safeToken,
        uint128 initialWithdrawalDelay,
        uint256 configTimeDelay
    ) public pure {
        console.log(
            "Verify command:",
            string.concat(
                "forge verify-contract --watch ",
                vm.toString(stakingAddress),
                " src/Staking.sol:Staking --verifier etherscan --chain-id ",
                vm.toString(chainId),
                ' --constructor-args $(cast abi-encode "constructor(address,address,uint128,uint256)" "',
                vm.toString(initialOwner),
                '" "',
                vm.toString(safeToken),
                '" "',
                vm.toString(initialWithdrawalDelay),
                '" "',
                vm.toString(configTimeDelay),
                '") --etherscan-api-key ETHERSCAN_MULTICHAIN_KEY'
            )
        );
    }
}
