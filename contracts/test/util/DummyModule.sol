// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Enum} from "@safe/interfaces/Enum.sol";
import {ISafe} from "@safe/interfaces/ISafe.sol";

/// @dev Minimal Safe module used in tests. Forwards calls through the Safe's ModuleManager so the
///      real checkModuleTransaction hook is exercised.
contract DummyModule {
    function execute(address safe, address to, uint256 value, bytes calldata data, Enum.Operation operation)
        external
        returns (bool success)
    {
        success = ISafe(payable(safe)).execTransactionFromModule(to, value, data, operation);
    }
}
