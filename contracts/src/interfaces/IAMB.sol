// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

/**
 * @title AMB Interface
 * @notice Interface for the Gnosis Chain canonical Arbitrary Message Bridge (AMB), covering the subset used
 *         to send a message and to validate one that was received.
 * @dev See https://docs.gnosischain.com/bridges/About%20Token%20Bridges/amb-bridge. Each side of the bridge
 *      (e.g. the "Foreign" AMB on Ethereum Mainnet and the "Home" AMB on Gnosis Chain) implements this same
 *      interface.
 */
interface IAMB {
    /**
     * @notice Requests that a message be relayed to a contract on the chain at the other end of the bridge.
     * @param target The contract to call on the destination chain.
     * @param data The calldata to call `target` with on the destination chain.
     * @param gas The gas limit to use for the call on the destination chain.
     * @return messageId A unique identifier for the relayed message.
     */
    function requireToPassMessage(address target, bytes memory data, uint256 gas) external returns (bytes32 messageId);

    /**
     * @notice The address that sent the message currently being relayed, on the chain it was sent from.
     * @dev Only meaningful while this AMB is mid-execution of a relayed call, i.e. when called by the
     *      contract the message targets, from within the call the AMB itself made to it.
     * @return sender The original sender address on the source chain.
     */
    function messageSender() external view returns (address sender);

    /**
     * @notice The chain ID of the chain the message currently being relayed was sent from.
     * @dev Only meaningful in the same context as `messageSender`.
     * @return chainId The source chain ID, as a `bytes32`.
     */
    function messageSourceChainId() external view returns (bytes32 chainId);
}
