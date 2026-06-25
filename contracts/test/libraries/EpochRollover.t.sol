// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {Vm} from "@forge-std/Vm.sol";
import {ConsensusMessages} from "@/libraries/ConsensusMessages.sol";
import {EpochRollover} from "@/libraries/EpochRollover.sol";
import {FROST} from "@/libraries/FROST.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";
import {ForgeSecp256k1} from "@test/util/ForgeSecp256k1.sol";

contract EpochRolloverTest is Test {
    using EpochRollover for EpochRollover.T;
    using ForgeSecp256k1 for ForgeSecp256k1.P;

    EpochRollover.T internal state;

    uint256 public constant CONSENSUS_CHAIN_ID = 100;
    address public constant CONSENSUS_ADDR = address(0xC01115E1115115);

    // Genesis group: secret key 1 → public key G, active at epoch 1.
    uint256 public constant GENESIS_SK = 1;
    uint256 public constant GENESIS_NK = 2;
    uint64 public constant GENESIS_EPOCH = 1;

    bytes32 internal domainSep;

    function setUp() public {
        domainSep = ConsensusMessages.domain(CONSENSUS_CHAIN_ID, CONSENSUS_ADDR);
    }

    // ============================================================
    // HELPERS
    // ============================================================

    /// @dev External wrapper so a reverting `rollover` reverts at a lower call depth than the cheatcode.
    ///      Only the revert tests need this; the rest call `state.rollover` directly.
    function callRollover(
        bytes32 domainSeparator,
        Secp256k1.Point calldata parentKey,
        uint64 parentEpoch,
        uint64 proposedEpoch,
        uint64 rolloverBlock,
        Secp256k1.Point calldata newGroupKey,
        FROST.Signature calldata signature
    ) external {
        state.rollover(domainSeparator, parentKey, parentEpoch, proposedEpoch, rolloverBlock, newGroupKey, signature);
    }

    /// @dev External wrapper so a reverting `initialize` reverts at a lower call depth than the cheatcode.
    function callInitialize(uint64 epoch, Secp256k1.Point calldata key) external {
        state.initialize(epoch, key);
    }

    function _key(uint256 sk) internal returns (Secp256k1.Point memory) {
        return ForgeSecp256k1.g(sk).toPoint();
    }

    /// @dev Single-signer FROST Schnorr signature: z = nonceKey + challenge(R, Y, msg) * secretKey.
    function _sign(uint256 secretKey, uint256 nonceKey, bytes32 message) internal returns (FROST.Signature memory) {
        Secp256k1.Point memory r = _key(nonceKey);
        Secp256k1.Point memory y = _key(secretKey);
        uint256 c = FROST.challenge(r, y, message);
        uint256 z = addmod(nonceKey, mulmod(c, secretKey, Secp256k1.N), Secp256k1.N);
        return FROST.Signature({r: r, z: z});
    }

    function _seedGenesis() internal {
        state.initialize(GENESIS_EPOCH, _key(GENESIS_SK));
    }

    /// @dev Builds and submits a valid rollover from `(parentSk@parentEpoch)` to `(newSk@proposedEpoch)`.
    function _doRollover(
        uint256 parentSk,
        uint256 parentNk,
        uint64 parentEpoch,
        uint64 proposedEpoch,
        uint64 rolloverBlock,
        uint256 newSk
    ) internal {
        Secp256k1.Point memory parentKey = _key(parentSk);
        Secp256k1.Point memory newKey = _key(newSk);
        bytes32 message = ConsensusMessages.epochRollover(domainSep, parentEpoch, proposedEpoch, rolloverBlock, newKey);
        FROST.Signature memory sig = _sign(parentSk, parentNk, message);
        state.rollover(domainSep, parentKey, parentEpoch, proposedEpoch, rolloverBlock, newKey, sig);
    }

    function _dummySig() internal returns (FROST.Signature memory) {
        return FROST.Signature({r: _key(2), z: 1});
    }

    // ============================================================
    // INITIALIZE
    // ============================================================

    function test_initialize_seedsAndEmits() public {
        Secp256k1.Point memory gk = _key(GENESIS_SK);
        vm.expectEmit(true, false, false, true);
        emit EpochRollover.EpochInitialized(GENESIS_EPOCH, gk);
        state.initialize(GENESIS_EPOCH, gk);

        assertTrue(state.isKnown(gk, GENESIS_EPOCH));
    }

    function test_initialize_revertsOnZeroKey() public {
        vm.expectRevert(Secp256k1.NotOnCurve.selector);
        this.callInitialize(GENESIS_EPOCH, Secp256k1.Point({x: 0, y: 0}));
    }

    // ============================================================
    // ROLLOVER
    // ============================================================

    function test_rollover_recordsChildAndEmits() public {
        _seedGenesis();
        Secp256k1.Point memory parentKey = _key(GENESIS_SK);
        Secp256k1.Point memory newKey = _key(5);
        uint64 proposedEpoch = GENESIS_EPOCH + 1;
        bytes32 message = ConsensusMessages.epochRollover(domainSep, GENESIS_EPOCH, proposedEpoch, 100, newKey);
        FROST.Signature memory sig = _sign(GENESIS_SK, GENESIS_NK, message);

        vm.expectEmit(true, true, false, true);
        emit EpochRollover.EpochRolledOver(GENESIS_EPOCH, proposedEpoch, parentKey, newKey);
        state.rollover(domainSep, parentKey, GENESIS_EPOCH, proposedEpoch, 100, newKey, sig);

        assertTrue(state.isKnown(newKey, proposedEpoch));
    }

    function test_rollover_chainDepthTwo() public {
        _seedGenesis();
        // genesis (1@1) → A (5@2)
        _doRollover(GENESIS_SK, GENESIS_NK, GENESIS_EPOCH, 2, 100, 5);
        assertTrue(state.isKnown(_key(5), 2));
        // A (5@2) → B (6@3): the second rollover verifies against A's key
        _doRollover(5, 7, 2, 3, 100, 6);
        assertTrue(state.isKnown(_key(6), 3));
    }

    function test_rollover_branchingSameEpoch() public {
        _seedGenesis();
        // Two valid rollovers from the same parent, distinct keys, SAME proposed epoch.
        _doRollover(GENESIS_SK, GENESIS_NK, GENESIS_EPOCH, 2, 100, 5);
        _doRollover(GENESIS_SK, 3, GENESIS_EPOCH, 2, 100, 6);

        assertTrue(state.isKnown(_key(5), 2));
        assertTrue(state.isKnown(_key(6), 2));
    }

    // This is never expected to happen and included for completeness only.
    function test_rollover_sameKeyTwoEpochs() public {
        _seedGenesis();
        // Same key (5) recorded at two different epochs via two branches.
        _doRollover(GENESIS_SK, GENESIS_NK, GENESIS_EPOCH, 2, 100, 5);
        _doRollover(GENESIS_SK, 3, GENESIS_EPOCH, 3, 100, 5);

        assertTrue(state.isKnown(_key(5), 2));
        assertTrue(state.isKnown(_key(5), 3));
    }

    function test_rollover_revertsWhenNotAdvancing() public {
        _seedGenesis();
        Secp256k1.Point memory parentKey = _key(GENESIS_SK);
        Secp256k1.Point memory newKey = _key(5);
        FROST.Signature memory dummy = _dummySig();

        vm.expectRevert(EpochRollover.EpochNotAdvancing.selector);
        this.callRollover(domainSep, parentKey, GENESIS_EPOCH, GENESIS_EPOCH, 100, newKey, dummy); // equal

        vm.expectRevert(EpochRollover.EpochNotAdvancing.selector);
        this.callRollover(domainSep, parentKey, GENESIS_EPOCH, 0, 100, newKey, dummy); // lower
    }

    function test_rollover_revertsUnknownParent() public {
        _seedGenesis();
        Secp256k1.Point memory unknownParent = _key(123);
        vm.expectRevert(EpochRollover.UnknownParent.selector);
        this.callRollover(domainSep, unknownParent, GENESIS_EPOCH, 2, 100, _key(5), _dummySig());
    }

    function test_rollover_revertsKnownKeyWrongEpoch() public {
        _seedGenesis();
        // Genesis key is known at epoch 1; passing it as the epoch-2 parent must be rejected.
        Secp256k1.Point memory parentKey = _key(GENESIS_SK);
        vm.expectRevert(EpochRollover.UnknownParent.selector);
        this.callRollover(domainSep, parentKey, GENESIS_EPOCH + 1, 3, 100, _key(5), _dummySig());
    }

    function test_rollover_revertsOnZeroNewKey() public {
        _seedGenesis();
        Secp256k1.Point memory parentKey = _key(GENESIS_SK);
        vm.expectRevert(Secp256k1.NotOnCurve.selector);
        this.callRollover(domainSep, parentKey, GENESIS_EPOCH, 2, 100, Secp256k1.Point({x: 0, y: 0}), _dummySig());
    }

    function test_rollover_revertsOnMismatchedMessageField() public {
        _seedGenesis();
        Secp256k1.Point memory parentKey = _key(GENESIS_SK);
        Secp256k1.Point memory newKey = _key(5);
        // Sign for rolloverBlock 100 but submit with 200 — the reconstructed message differs.
        bytes32 message = ConsensusMessages.epochRollover(domainSep, GENESIS_EPOCH, 2, 100, newKey);
        FROST.Signature memory sig = _sign(GENESIS_SK, GENESIS_NK, message);

        vm.expectRevert(Secp256k1.InvalidMulMulAddWitness.selector);
        this.callRollover(domainSep, parentKey, GENESIS_EPOCH, 2, 200, newKey, sig);
    }

    function test_rollover_idempotentResubmission() public {
        _seedGenesis();
        Secp256k1.Point memory parentKey = _key(GENESIS_SK);
        Secp256k1.Point memory newKey = _key(5);
        bytes32 message = ConsensusMessages.epochRollover(domainSep, GENESIS_EPOCH, 2, 100, newKey);
        FROST.Signature memory sig = _sign(GENESIS_SK, GENESIS_NK, message);

        state.rollover(domainSep, parentKey, GENESIS_EPOCH, 2, 100, newKey, sig);
        assertTrue(state.isKnown(newKey, 2));

        // Re-submitting the identical rollover is a no-op: no revert, no event, state unchanged.
        vm.recordLogs();
        state.rollover(domainSep, parentKey, GENESIS_EPOCH, 2, 100, newKey, sig);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertEq(logs.length, 0);
        assertTrue(state.isKnown(newKey, 2));
    }

    // ============================================================
    // IS KNOWN
    // ============================================================

    function test_isKnown_exactOnPair() public {
        _seedGenesis();
        assertTrue(state.isKnown(_key(GENESIS_SK), GENESIS_EPOCH));
        assertFalse(state.isKnown(_key(GENESIS_SK), GENESIS_EPOCH + 1)); // right key, wrong epoch
        assertFalse(state.isKnown(_key(2), GENESIS_EPOCH)); // wrong key, right epoch
    }
}
