// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Vm} from "@forge-std/Vm.sol";
import {RealitioV3} from "@reality/interfaces/RealitioV3.sol";
import {RealityModuleETH} from "@reality/RealityModuleETH.sol";

/**
 * @title RealityModuleDeployer
 * @notice Brings up a live `RealityModuleETH` v2.0.0 for tests, working around a version mismatch in the
 *         pinned dependency set.
 * @dev `new RealityModuleETH(...)` does not work here. The constructor calls `setUp`, which calls
 *      `__Ownable_init()`. zodiac-module-reality v2.0.0 was written against OpenZeppelin Upgradeable
 *      <= 4.3, where `__Ownable_init` carried `initializer`, so `setUp` has no initializer modifier of its
 *      own. `contracts/lib/oz-upgradeable-v4` is pinned at v4.9.6, where `__Ownable_init` carries
 *      `onlyInitializing` instead, so construction reverts "Initializable: contract is not initializing".
 *
 *      The workaround runs the real `setUp`, with its real requires and `transferOwnership`, against the
 *      real runtime code: etch the runtime bytecode (the contract has no immutables, so it needs no
 *      constructor-time initialization), open the initialization window by hand, call `setUp`, then close
 *      it. Slot 0 is `Initializable`'s: byte 0 is `_initialized` (uint8), byte 1 is `_initializing` (bool).
 *
 *      This is a fixture workaround, not a shortcut around the module's own logic. It disappears if
 *      `oz-upgradeable-v4` is ever re-pinned to a 4.2/4.3 release, which is what the deployed mastercopy
 *      was compiled against.
 */
library RealityModuleDeployer {
    /// @dev Forge cheatcode address, so this library needs no `Test` base.
    Vm private constant VM = Vm(address(uint160(uint256(keccak256("hevm cheat code")))));

    /// @dev `Initializable` slot 0 with `_initialized == 0` and `_initializing == true`.
    bytes32 private constant INITIALIZING = bytes32(uint256(1) << 8);

    /// @dev `Initializable` slot 0 with `_initialized == 1` and `_initializing == false`.
    bytes32 private constant INITIALIZED = bytes32(uint256(1));

    /**
     * @notice Deploys a fully initialized `RealityModuleETH` v2.0.0 at `at`.
     * @dev The caller is the transient owner during `setUp`, which is why `setUp` can run
     *      `transferOwnership(owner)`. Parameters mirror the module's own constructor.
     * @param at Address to place the module at; must not already hold code.
     * @param owner Owner of the module, expected to be the Safe.
     * @param avatar Avatar the module acts through.
     * @param target Contract the module calls `execTransactionFromModule` on.
     * @param oracle Realitio oracle the module asks questions of.
     * @param timeout Question timeout in seconds.
     * @param cooldown Cooldown in seconds after an answer before execution is allowed.
     * @param expiration Duration a positive answer stays valid, or 0 for forever.
     * @param bond Minimum bond required for an answer to be accepted.
     * @param templateId Realitio template id for proposal questions.
     * @param arbitrator Arbitrator for the module's questions.
     * @return module The initialized module.
     */
    function deploy(
        address at,
        address owner,
        address avatar,
        address target,
        RealitioV3 oracle,
        uint32 timeout,
        uint32 cooldown,
        uint32 expiration,
        uint256 bond,
        uint256 templateId,
        address arbitrator
    ) internal returns (RealityModuleETH module) {
        require(at.code.length == 0, "RealityModuleDeployer: address already has code");

        VM.etch(at, type(RealityModuleETH).runtimeCode);
        module = RealityModuleETH(at);

        VM.store(at, 0, INITIALIZING);
        module.setUp(
            abi.encode(owner, avatar, target, oracle, timeout, cooldown, expiration, bond, templateId, arbitrator)
        );
        VM.store(at, 0, INITIALIZED);
    }
}
