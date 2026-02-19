// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {Staking} from "../../src/Staking.sol";
import {DeterministicDeployment} from "./DeterministicDeployment.sol";

contract DeployStakingWithTxBuilderScript is Script {
    using DeterministicDeployment for DeterministicDeployment.Factory;

    function run() public {
        // Required script arguments:
        address initialOwner = vm.envAddress("STAKING_INITIAL_OWNER");
        address safeToken = vm.envAddress("SAFE_TOKEN");
        uint128 initialWithdrawalDelay = uint128(vm.envUint("STAKING_INITIAL_WITHDRAWAL_DELAY"));
        uint256 configTimeDelay = vm.envUint("STAKING_CONFIG_TIME_DELAY");
        string memory safe = vm.toString(vm.envAddress("SAFE"));
        string memory safeOwner = vm.toString(vm.envAddress("SAFE_OWNER"));
        string memory chainId = vm.toString(vm.envUint("CHAIN_ID"));

        // Safe Tx Builder uses millisecond timestamps in exports.
        string memory createdAtStr = vm.toString(vm.unixTime() * 1000);
        // Build the txData (creation code + constructor args)
        bytes memory constructorArgs = abi.encode(initialOwner, safeToken, initialWithdrawalDelay, configTimeDelay);
        string memory txData = vm.toString(abi.encodePacked(bytes32(0), type(Staking).creationCode, constructorArgs));

        // Compute the tx-builder checksum from the canonical serialization (sorted keys, custom object format).
        string memory checksum = _calculateTxBuilderChecksum(createdAtStr, chainId, safe, safeOwner, txData);

        {
            // Emit a tx-builder compatible batch file with a precomputed checksum.
            string memory path = "transactions.json";
            string memory json = string(
                abi.encodePacked(
                    '{"version":"1.0","chainId":"',
                    chainId,
                    '","createdAt":',
                    createdAtStr,
                    ',"meta":{"name":"',
                    "Transactions Batch",
                    '","description":"',
                    "Safenet Staking Beta Deployment Transaction",
                    '","txBuilderVersion":"',
                    "1.18.3",
                    '","createdFromSafeAddress":"',
                    safe,
                    '","createdFromOwnerAddress":"',
                    safeOwner,
                    '","checksum":"',
                    checksum,
                    '"},"transactions":[{"to":"0x914d7Fec6aaC8cd542e72Bca78B30650d45643d7","value":"0","data":"',
                    txData,
                    '","contractMethod":null,"contractInputsValues":null}]}'
                )
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
        string memory safeAddress,
        string memory safeOwner,
        string memory txData
    ) internal pure returns (string memory) {
        // Follows apps/tx-builder/src/lib/checksum.ts serialization rules.
        // Note: meta.name is forced to null for checksum calculation.
        string memory serialized = string(
            abi.encodePacked(
                '{["chainId","createdAt","meta","transactions","version"]',
                _jsonQuote(chainId), // Chain ID
                ",",
                createdAtStr,
                ',{["createdFromOwnerAddress","createdFromSafeAddress","description","name","txBuilderVersion"]',
                _jsonQuote(safeOwner),
                ",",
                _jsonQuote(safeAddress),
                ",",
                _jsonQuote("Safenet Staking Beta Deployment Transaction"),
                ",",
                "null",
                ",",
                _jsonQuote("1.18.3"), // Tx Builder version
                ",},",
                '[{["contractInputsValues","contractMethod","data","to","value"]',
                "null,null,",
                _jsonQuote(txData),
                ",",
                _jsonQuote("0x914d7Fec6aaC8cd542e72Bca78B30650d45643d7"), // Factory address
                ",",
                _jsonQuote("0"), // Value
                ",}],",
                _jsonQuote("1.0"), // Tx Builder batch format version
                ",}"
            )
        );

        return vm.toString(keccak256(bytes(serialized)));
    }

    /// @dev Wraps a string in double quotes and escapes internal quotes/backslashes for JSON encoding.
    function _jsonQuote(string memory value) internal pure returns (string memory) {
        bytes memory input = bytes(value);
        bytes memory output = new bytes(input.length * 2 + 2);
        uint256 outIndex = 0;

        output[outIndex++] = '"';

        for (uint256 i = 0; i < input.length; i++) {
            bytes1 ch = input[i];
            if (ch == '"' || ch == "\\") {
                output[outIndex++] = "\\";
            }
            output[outIndex++] = ch;
        }

        output[outIndex++] = '"';

        // Resize output to actual length to avoid trailing zeros.
        assembly {
            mstore(output, outIndex)
        }

        return string(output);
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
            string(
                abi.encodePacked(
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
            )
        );
    }
}
