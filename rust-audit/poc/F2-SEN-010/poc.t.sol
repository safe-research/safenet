
    // ------------------------------------------------------------------
    // QA2-SEN-Δ (rust-audit F2-SEN-010): temporary, reverted after the run.
    // Onchain counterpart of poc/F2-SEN-010/poc.rs: a commit mined in block
    // commitDeadline is accepted (committedCount 2), one of the two reveals
    // lands in commitDeadline + 1, and `finalize` reverts FinalizeTooEarly —
    // the fate of the sentinel's premature `Finalize` on its local 1/1 tally.
    // ------------------------------------------------------------------
    function test_QA2SEN010_DeadlineBlockCommitCounts_FinalizeTooEarlyAtOneOfTwoReveals() public {
        _postRequest();
        uint256 commitDeadline = oracle.getRequest(REQUEST_ID).terms.commitDeadline;
        _commit(sentinel1, true, SALT_1); // the early committer (this software)
        vm.roll(commitDeadline); // head == commitDeadline
        _commit(sentinel2, true, SALT_2); // accepted: block.number <= commitDeadline (Requests.sol:117)
        assertEq(oracle.getRequest(REQUEST_ID).progress.committedCount, 2, "commit in the deadline block counts");

        vm.roll(commitDeadline + 1);
        bytes32 lateHash = oracle.hashCommitment(sentinel3, REQUEST_ID, true, SALT_3, "");
        vm.prank(sentinel3);
        vm.expectRevert(SentinelOracleRequest.CommitWindowClosed.selector);
        oracle.commit(REQUEST_ID, lateHash); // boundary: one block later is refused

        _reveal(sentinel1, true, SALT_1); // revealedCount 1 of 2, first block of the reveal window
        SentinelOracleRequest.T memory r = oracle.getRequest(REQUEST_ID);
        emit log_named_uint("commitDeadline", commitDeadline);
        emit log_named_uint("block.number at finalize", block.number);
        emit log_named_uint("revealDeadline", r.terms.revealDeadline);
        emit log_named_uint("committedCount", r.progress.committedCount);
        emit log_named_uint("revealedCount", r.progress.revealedCount);
        assertEq(r.progress.committedCount, 2);
        assertEq(r.progress.revealedCount, 1);
        assertLe(block.number, r.terms.revealDeadline);

        vm.expectRevert(SentinelOracleRequest.FinalizeTooEarly.selector);
        oracle.finalize(REQUEST_ID); // Requests.sol:170-172

        // Same tally after the reveal deadline: finalize is accepted (the recovery path the parked sentinel never takes itself).
        vm.roll(r.terms.revealDeadline + 1);
        oracle.finalize(REQUEST_ID);
    }
