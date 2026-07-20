// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {TransactionAnnouncement} from "@/libraries/TransactionAnnouncement.sol";

/**
 * @title TransactionAnnouncementTest
 * @notice Unit tests for the `TransactionAnnouncement` state library, exercised on this contract's own
 *         storage. Reverting paths go through external `this.call*` wrappers (an internal library revert
 *         is at the cheatcode's own call depth, which `vm.expectRevert` cannot observe). Covers the
 *         paths the guard no longer reaches after bounding its constructor timing (notably
 *         `WindowOverflow`), plus the core announce/consume/cancel/renewal lifecycle.
 */
contract TransactionAnnouncementTest is Test {
    using TransactionAnnouncement for TransactionAnnouncement.T;

    TransactionAnnouncement.T internal state;

    address internal constant SAFE = address(0x5AFE);
    bytes32 internal constant ID = keccak256("announcement-id");
    uint256 internal constant DELAY = 1 days;
    uint256 internal constant WINDOW = 3 days;

    // External wrappers so `vm.expectRevert` can observe library reverts at a lower call depth.
    function callAnnounce(address safe, bytes32 id, uint256 delay, uint256 window) external {
        state.announce(safe, id, delay, window);
    }

    function callCancel(address safe, bytes32 id) external {
        state.cancel(safe, id);
    }

    function test_announce_setsWindow() public {
        uint256 t0 = block.timestamp;
        (uint256 activeFrom, uint256 activeUntil) = state.announce(SAFE, ID, DELAY, WINDOW);
        assertEq(activeFrom, t0 + DELAY);
        assertEq(activeUntil, t0 + DELAY + WINDOW);
        (uint256 storedFrom, uint256 storedUntil) = state.windowOf(SAFE, ID);
        assertEq(storedFrom, activeFrom);
        assertEq(storedUntil, activeUntil);
    }

    function test_announce_revertsWhilePendingOrActive() public {
        state.announce(SAFE, ID, DELAY, WINDOW);
        vm.expectRevert(TransactionAnnouncement.AnnouncementAlreadyExists.selector);
        this.callAnnounce(SAFE, ID, DELAY, WINDOW);
    }

    function test_announce_renewsExpired() public {
        state.announce(SAFE, ID, DELAY, WINDOW);
        vm.warp(block.timestamp + DELAY + WINDOW + 1); // expire
        (uint256 activeFrom,) = state.announce(SAFE, ID, DELAY, WINDOW); // must not revert
        assertEq(activeFrom, block.timestamp + DELAY);
    }

    function test_announce_revertsOnWindowOverflow() public {
        // delay fits uint256 (no checked-add panic) but pushes activeUntil past uint128.
        vm.expectRevert(TransactionAnnouncement.WindowOverflow.selector);
        this.callAnnounce(SAFE, ID, uint256(1) << 128, 0);
    }

    function test_consume_onlyWithinWindow() public {
        state.announce(SAFE, ID, DELAY, WINDOW);

        assertFalse(state.consume(SAFE, ID)); // before activeFrom
        vm.warp(block.timestamp + DELAY - 1);
        assertFalse(state.consume(SAFE, ID));

        vm.warp(block.timestamp + 1); // exactly activeFrom
        assertTrue(state.consume(SAFE, ID)); // consumed
        (uint256 activeFrom,) = state.windowOf(SAFE, ID);
        assertEq(activeFrom, 0); // deleted
    }

    function test_consume_falseAfterExpiry() public {
        state.announce(SAFE, ID, DELAY, WINDOW);
        vm.warp(block.timestamp + DELAY + WINDOW + 1);
        assertFalse(state.consume(SAFE, ID));
        // The stored entry is not auto-cleared on a failed consume.
        (uint256 activeFrom,) = state.windowOf(SAFE, ID);
        assertGt(activeFrom, 0);
    }

    function test_cancel_deletes() public {
        state.announce(SAFE, ID, DELAY, WINDOW);
        state.cancel(SAFE, ID);
        (uint256 activeFrom,) = state.windowOf(SAFE, ID);
        assertEq(activeFrom, 0);
    }

    function test_cancel_revertsIfMissing() public {
        vm.expectRevert(TransactionAnnouncement.AnnouncementNotFound.selector);
        this.callCancel(SAFE, ID);
    }
}
