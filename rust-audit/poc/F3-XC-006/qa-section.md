## QA (QA3-SEN, 2026-10-08)

Result: Reproduced (by existing passing tests plus an end-to-end sighting; no new test written, to avoid duplicating upstream coverage).

Route: the sentinel's early `Finalize` can never succeed. Contract side: `contracts/test/SentinelOracle.t.sol:694-725` (`test_UnanimousApprove_FeeDistributedAndBondsReturned`) shows the last reveal auto-finalizes and emits `OracleResult` before `revealDeadline`, after which `oracle.finalize(id)` reverts `RequestNotPending` (PASS at 361aaf3). Sentinel side: `crates/sentinel/src/service.rs` `flow_unanimous_approve_finalizes_via_early_reveal_and_claims` shows the sentinel queues a `Finalize` after the last reveal (PASS). End-to-end: in the F3-SEN-001 bite run the real sentinel binary's nonce 3 `oracle.finalize` returns status 0 (`RequestNotPending`) between its successful reveal (nonce 2) and claim (nonce 4).

Command and output: `poc/F3-XC-006/README.md`; `forge test --match-test test_UnanimousApprove_FeeDistributedAndBondsReturned`; `cargo test -p sentinel --bins flow_unanimous_approve_finalizes_via_early_reveal_and_claims`; `poc/F3-SEN-001/out-fixed-run.txt`.

Bite check: not applicable (no new test; the two cited tests already assert the exact behavior and pass). Remediation option 1 (drop the early `Finalize`, rely on the oracle's own finalize in the last reveal) is sound; only gas and a queue slot are at stake, so Informational stands.

Certainty: 95 (Reproduced via existing tests and an end-to-end sighting, plus Critic Confirmed; the auto-finalize in the last reveal is unconditional).
