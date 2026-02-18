// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

library SafeSingletonFactory {
    address constant ADDRESS = 0x914d7Fec6aaC8cd542e72Bca78B30650d45643d7;

    /**
     * @notice Function to deploy a contract using CREATE2.
     * @param salt The salt to use for the CREATE2 deployment.
     * @param code The bytecode of the contract to deploy.
     * @return result The address of the deployed contract.
     */
    function deploy(bytes32 salt, bytes memory code) internal returns (address result) {
        bytes memory creation = abi.encodePacked(salt, code);
        assembly {
            mstore(0, 0)
            if iszero(call(gas(), ADDRESS, 0, add(creation, 0x20), mload(creation), 12, 20)) { revert(0, 0) }
            result := mload(0)
        }
    }
}
