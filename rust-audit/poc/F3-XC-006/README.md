# F3-XC-006 PoC (QA3-SEN)

No new PoC written: the claim is already covered by two passing tests at 361aaf3, and I observed it end to end. Writing a duplicate would violate the no-duplication rule.

## Contract side (the early Finalize reverts)

`contracts/test/SentinelOracle.t.sol:694-725` `test_UnanimousApprove_FeeDistributedAndBondsReturned`: after both sentinels reveal, the last reveal auto-finalizes (emits `OracleResult` before `revealDeadline`), and a subsequent `oracle.finalize(id)` reverts with `SentinelOracleRequest.RequestNotPending`.

```
cd contracts && forge test --match-test test_UnanimousApprove_FeeDistributedAndBondsReturned --match-contract SentinelOracleTest
# [PASS] test_UnanimousApprove_FeeDistributedAndBondsReturned()
```

## Sentinel side (the early Finalize is queued)

`crates/sentinel/src/service.rs` `flow_unanimous_approve_finalizes_via_early_reveal_and_claims`: after the last reveal the sentinel queues a `Finalize` action.

```
cargo test -p sentinel --bins flow_unanimous_approve_finalizes_via_early_reveal_and_claims
# test ... ok
```

## End-to-end sighting

In the F3-SEN-001 bite run (`../F3-SEN-001/out-fixed-run.txt`), the real sentinel binary's transactions were: nonce 2 `oracle.reveal` status OK (auto-finalizes onchain), nonce 3 `oracle.finalize` status 0 FAILED (`RequestNotPending`), nonce 4 `oracle.claim` status OK. So the early `Finalize` reverts on a live chain while the claim still succeeds. Informational: only gas and a queue slot are lost.
