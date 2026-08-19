// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {IAMB} from "@/interfaces/IAMB.sol";

/**
 * @title AMB Governance Adapter
 * @notice Relays a governance call from a trusted sender on a configurable origin chain (e.g. a Safe on
 *         Ethereum Mainnet) to an arbitrary contract on this chain (e.g. a SentinelOracle deployed on
 *         Gnosis Chain), via the Gnosis Chain canonical Arbitrary Message Bridge (AMB).
 * @dev Deployed on the destination chain and set as the trusted "governance" address of another contract.
 *      The origin sender calls the origin-side AMB's `requireToPassMessage`, targeting this contract's
 *      `execute` function; the destination-side AMB then calls `execute` here on the origin sender's
 *      behalf. `execute` only proceeds if the call was relayed by the configured AMB, for a message that
 *      originated on the configured chain from the configured sender -- see `onlyAmb`. There is no
 *      restriction on which contract or function `execute` may call: the origin sender is trusted to only
 *      encode calls it intends to make (typically calls into the governance-gated functions of whatever
 *      contract this adapter is configured as the governance address of, but that is not enforced here).
 */
contract AmbGovernanceAdapter {
    // ============================================================
    // IMMUTABLES
    // ============================================================

    /**
     * @notice The canonical AMB contract on this (destination) chain that relays messages from the origin
     *         chain, e.g. the Gnosis Chain "Home" AMB when the origin chain is Ethereum Mainnet.
     */
    IAMB public immutable AMB;

    /**
     * @notice The chain ID of the origin chain from which calls are accepted, e.g. 1 for Ethereum Mainnet.
     */
    uint256 public immutable ORIGIN_CHAIN_ID;

    /**
     * @notice The address on the origin chain that is authorised to trigger calls, e.g. a Safe.
     */
    address public immutable ORIGIN_SENDER;

    // ============================================================
    // EVENTS
    // ============================================================

    /**
     * @notice Emitted after successfully relaying a bridged call.
     */
    event Executed(address indexed to, bytes data, bytes returnData);

    // ============================================================
    // ERRORS
    // ============================================================

    /**
     * @notice Thrown when constructed with a zero address for the AMB or origin sender.
     */
    error InvalidAddress();

    /**
     * @notice Thrown when `execute` is called by anything other than the configured AMB.
     */
    error NotAmb();

    /**
     * @notice Thrown when the relayed message did not originate on the configured origin chain.
     */
    error UnauthorizedOriginChain();

    /**
     * @notice Thrown when the relayed message was not sent by the configured origin sender.
     */
    error UnauthorizedOriginSender();

    // ============================================================
    // MODIFIERS
    // ============================================================

    // forge-lint: disable-start(unwrapped-modifier-logic)

    /**
     * @notice Restricts `execute` to messages relayed by the configured AMB from the configured origin
     *         chain and sender.
     */
    modifier onlyAmb() {
        require(msg.sender == address(AMB), NotAmb());
        require(AMB.messageSourceChainId() == bytes32(ORIGIN_CHAIN_ID), UnauthorizedOriginChain());
        require(AMB.messageSender() == ORIGIN_SENDER, UnauthorizedOriginSender());
        _;
    }

    // forge-lint: disable-end(unwrapped-modifier-logic)

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    /**
     * @notice Constructs the adapter.
     * @param amb The canonical AMB contract on this (destination) chain.
     * @param originChainId The chain ID of the origin chain that calls must come from.
     * @param originSender The address on the origin chain authorised to trigger calls, e.g. a Safe.
     */
    constructor(address amb, uint256 originChainId, address originSender) {
        require(amb != address(0), InvalidAddress());
        require(originSender != address(0), InvalidAddress());
        AMB = IAMB(amb);
        ORIGIN_CHAIN_ID = originChainId;
        ORIGIN_SENDER = originSender;
    }

    // ============================================================
    // EXTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Executes an arbitrary call, relayed from the origin chain via the AMB.
     * @param to The address to call.
     * @param data The calldata to call `to` with.
     * @return returnData The data returned by the call.
     * @dev Callable only by the configured AMB, relaying a message from the configured origin chain and
     *      sender -- see `onlyAmb`. The call is arbitrary and unrestricted; it is the origin sender's
     *      responsibility to only encode calls it intends to make.
     */
    function execute(address to, bytes calldata data) external onlyAmb returns (bytes memory returnData) {
        bool success;
        (success, returnData) = to.call(data);
        if (!success) {
            // Propagate the revert data, so a failed governance call surfaces its original reason instead
            // of a generic one.
            assembly ("memory-safe") {
                revert(add(returnData, 0x20), mload(returnData))
            }
        }
        emit Executed(to, data, returnData);
    }
}
