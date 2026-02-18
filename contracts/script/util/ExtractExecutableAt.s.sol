// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";

contract ExtractExecutableAtScript is Script {
    function setUp() public {}

    function run() public view {
        // Read the executableAt value from run-latest.json using the chain ID
        string memory json = vm.readFile(
            string.concat(
                "build/broadcast/ProposeAndAcceptValidators.s.sol/", vm.toString(block.chainid), "/run-latest.json"
            )
        );
        bytes memory proposeEventData = vm.parseJsonBytes(json, ".receipts[0].logs[0].data");
        (,, uint256 executableAt) = abi.decode(proposeEventData, (address[], bool[], uint256));
        console.log("Extracted executableAt value: %d", executableAt);
    }
}
