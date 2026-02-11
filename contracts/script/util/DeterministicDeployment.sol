// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

/**
 * @title Deterministic Contract Deployments
 * @notice Library for performing deterministic CREATE2 contract deployments with CREATE2 factory contracts.
 */
library DeterministicDeployment {
    type Factory is address;

    /**
     * @notice The canonical CREATE2 factory address used by Foundry by default. Anvil will always deploy this factory
     *         when spinning up a test node.
     */
    Factory constant CANONICAL = Factory.wrap(0x4e59b44847b379578588920cA78FbF26c0B4956C);

    /**
     * @notice The CREATE2 factory did not create an account at the expected address.
     */
    error NotCreated(address account);

    /**
     * @notice Computes the deterministic deployment address of a contract using the specified factory and salt.
     * @param self The CREATE2 factory executing the deployment.
     * @param salt The CREATE2 salt.
     * @param code The initialization code of the contract.
     * @return result The public address of the deterministic deployment.
     */
    function deploymentAddress(Factory self, bytes32 salt, bytes memory code) internal pure returns (address result) {
        return deploymentAddressByHash(self, salt, keccak256(code));
    }

    /**
     * @notice Computes the deterministic deployment address of a contract with constructor arguments using the
     *         specified factory and salt.
     * @param self The CREATE2 factory executing the deployment.
     * @param salt The CREATE2 salt.
     * @param code The initialization code of the contract.
     * @param args The ABI-encoded constructor arguments.
     * @return result The public address of the deterministic deployment.
     */
    function deploymentAddressWithArgs(Factory self, bytes32 salt, bytes memory code, bytes memory args)
        internal
        pure
        returns (address result)
    {
        return deploymentAddressByHash(self, salt, keccak256(abi.encodePacked(code, args)));
    }

    /**
     * @notice Computes the deterministic deployment address of a contract using the specified factory and salt.
     * @param self The CREATE2 factory executing the deployment.
     * @param salt The CREATE2 salt.
     * @param codeHash The contract code hash.
     * @return result The public address of the deterministic deployment.
     */
    function deploymentAddressByHash(Factory self, bytes32 salt, bytes32 codeHash)
        internal
        pure
        returns (address result)
    {
        return address(uint160(uint256(keccak256(abi.encodePacked(hex"ff", self, salt, codeHash)))));
    }

    /**
     * @notice Creates a deterministic deployment of a contract using the specified factory and salt.
     * @param self The CREATE2 factory executing the deployment.
     * @param salt The CREATE2 salt.
     * @param code The initialization code of the contract.
     * @return result The public address of the deterministic deployment.
     */
    function deploy(Factory self, bytes32 salt, bytes memory code) internal returns (address result) {
        return deployWithArgs(self, salt, code, "");
    }

    /**
     * @notice Creates a deterministic deployment of a contract with constructor arguments using the specified factory
     *         and salt.
     * @param self The CREATE2 factory executing the deployment.
     * @param salt The CREATE2 salt.
     * @param code The initialization code of the contract.
     * @param args The ABI-encoded constructor arguments.
     * @return result The public address of the deterministic deployment.
     */
    function deployWithArgs(Factory self, bytes32 salt, bytes memory code, bytes memory args)
        internal
        returns (address result)
    {
        result = deploymentAddressWithArgs(self, salt, code, args);
        if (result.code.length == 0) {
            (bool success, bytes memory rdata) = Factory.unwrap(self).call(abi.encodePacked(salt, code, args));
            if (!success) {
                // Propagate the revert data.
                assembly ("memory-safe") {
                    revert(add(rdata, 0x20), mload(rdata))
                }
            }

            // In order to support multiple CREATE2 factory implementations (which return the created contract with or
            // without padding), just assert that the expected deployment address has code after the call instead of
            // checking the return bytes.
            require(result.code.length != 0, NotCreated(result));
        }
    }
}
