// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Vm} from "@forge-std/Vm.sol";

/**
 * @notice Thrown when the target chain does not implement an EVM feature that the deployed contracts require.
 */
error ChainNotSupported(string feature);

/**
 * @notice Reverts unless the target chain implements the full set of EVM features required by the Safenet
 *         contracts.
 * @param vm The forge VM.
 */
function requireFullChainSupport(Vm vm) {
    _probeChainSupport(vm, "MCOPY", type(MCopyProbe).creationCode);
    _probeChainSupport(vm, "MODEXP", type(ModExpProbe).creationCode);
}

/**
 * @notice Reverts unless the target chain implements the EVM features required by the Safenet contracts for
 *         verification of FROST attestations.
 * @param vm The forge VM.
 */
function requireVerificationChainSupport(Vm vm) {
    _probeChainSupport(vm, "MCOPY", type(MCopyProbe).creationCode);
}

/**
 * @notice Executes a probe's creation code on the target chain, reverting if it does not succeed.
 * @param vm The forge VM.
 * @param feature The name of the EVM feature being probed for.
 * @param code The creation code of the probe contract.
 * @dev The probes are executed with `eth_call` over the RPC endpoint, and deliberately not in the script's own EVM.
 *      Forge simulates scripts in a local EVM whose feature set comes from the `evm_version` configuration and not
 *      from the chain being forked, so an in-script check would only ever assert against the local configuration and
 *      would pass for every target chain. The probe code can fail either by reverting, or by aborting with an invalid
 *      opcode on a chain where the opcode it uses is undefined, so the failure is reported by the call site instead of
 *      from within the probe itself.
 */
function _probeChainSupport(Vm vm, string memory feature, bytes memory code) {
    // Establish that the endpoint answers `eth_call` at all, so that a connection or endpoint error is reported as
    // itself instead of being misattributed to a missing feature by the probe below.
    vm.rpc("eth_call", '[{"data":"0x"},"latest"]');

    string memory params = string.concat('[{"data":"', vm.toString(code), '"},"latest"]');
    try vm.rpc("eth_call", params) returns (bytes memory) {}
    catch {
        revert ChainNotSupported(feature);
    }
}

/**
 * @title MCOPY Probe
 */
contract MCopyProbe {
    constructor() {
        uint256 value = gasleft();
        uint256 copied;
        assembly ("memory-safe") {
            let ptr := mload(0x40)
            mstore(ptr, value)
            mcopy(add(ptr, 0x20), ptr, 0x20)
            copied := mload(add(ptr, 0x20))
        }
        require(copied == value);
    }
}

/**
 * @title MODEXP Probe
 */
contract ModExpProbe {
    constructor() {
        (bool success, bytes memory result) =
            address(5).staticcall(abi.encodePacked(uint256(1), uint256(1), uint256(1), uint8(3), uint8(2), uint8(5)));
        require(success && keccak256(result) == keccak256(hex"04"));
    }
}
