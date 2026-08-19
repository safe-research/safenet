// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "@forge-std/Script.sol";
import {DeterministicDeployment} from "@script/util/DeterministicDeployment.sol";
import {getFactory} from "@script/util/GetFactory.sol";
import {AmbGovernanceAdapter} from "@/AmbGovernanceAdapter.sol";

contract DeployAmbGovernanceAdapterScript is Script {
    using DeterministicDeployment for DeterministicDeployment.Factory;

    function run() public returns (address adapter) {
        // Address of the canonical AMB contract on the chain this adapter is deployed to (e.g. the Gnosis
        // Chain "Home" AMB, when the origin chain is Ethereum Mainnet).
        address amb = vm.envAddress("AMB_GOVERNANCE_ADAPTER_AMB");
        // Chain ID of the origin chain calls must come from (e.g. 1 for Ethereum Mainnet).
        uint256 originChainId = vm.envUint("AMB_GOVERNANCE_ADAPTER_ORIGIN_CHAIN_ID");
        // Address on the origin chain authorised to trigger calls through this adapter (e.g. a Safe).
        address originSender = vm.envAddress("AMB_GOVERNANCE_ADAPTER_ORIGIN_SENDER");

        DeterministicDeployment.Factory factory = getFactory(vm);

        vm.startBroadcast();

        adapter = factory.deployWithArgs(
            bytes32(0), type(AmbGovernanceAdapter).creationCode, abi.encode(amb, originChainId, originSender)
        );

        vm.stopBroadcast();

        console.log("AmbGovernanceAdapter deployed at:", adapter);
    }
}
