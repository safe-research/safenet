// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

/// @dev Minimal AMB stand-in that lets tests configure the `messageSender`/`messageSourceChainId` an
///      `onlyAmb`-gated call should observe, then relays a call to the target as itself (mirroring how the
///      real AMB calls the destination contract directly, with the bridge contract as `msg.sender`).
contract MockAmb {
    address public messageSender;
    bytes32 public messageSourceChainId;

    function setMessageOrigin(uint256 chainId, address sender) external {
        messageSourceChainId = bytes32(chainId);
        messageSender = sender;
    }

    function relay(address target, bytes calldata data) external returns (bytes memory) {
        (bool success, bytes memory returnData) = target.call(data);
        if (!success) {
            assembly ("memory-safe") {
                revert(add(returnData, 0x20), mload(returnData))
            }
        }
        return returnData;
    }
}
