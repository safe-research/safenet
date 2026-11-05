# F3-CORE-060 PoC (QA3-CORE)

`qa3_fee.rs` is an example harness for `safenet-core` (copy it to `crates/core/examples/` in a worktree at `361aaf3`): the real `Driver` and `TransactionQueue` (Direct mode, default `blocks_before_resubmit` 2, no fee cap) follow a real Anvil chain through `Provider::connect` over HTTP, with one 21,000-gas action queued at startup through `Driver::queue_action`.

The RPC URL is `rpcproxy.py` with `{"swallow_send": true, "fee_history_reward_wei": 1000000000}`: it records each `eth_sendRawTransaction` and answers with its hash without forwarding it (trigger 2 of the finding: an endpoint that accepts but does not propagate), and reports a 1 gwei tip as every block's reward in `eth_feeHistory`. Without that, Anvil's empty blocks make alloy's estimator return a 1 wei tip (`alloy-provider-2.0.5/src/utils.rs:93-108`); 1 gwei is a round modelling choice, the claim is about multiples. Anvil runs with chain id 100, 1 s blocks and a 0.042 gwei starting base fee. `f060.sh` decodes every swallowed transaction with `cast decode-tx`, then sends the last one to Anvil and reads the tip it paid from the receipt.

## Run

```sh
cp qa3_fee.rs <worktree>/crates/core/examples/ && cargo build -p safenet-core --example qa3_fee
./f060.sh base 90
git apply remediation1-ceiling.patch && cargo build -p safenet-core --example qa3_fee && ./f060.sh ceiling 60
```

## Files

- `fee-compounding-at-361aaf3.txt`, `swallowed-submissions-at-361aaf3.tsv` (every submission: index, time, nonce, tip, max fee, raw), `harness-log-excerpt.txt`
- `fee-compounding-with-remediation1-ceiling.txt`, `remediation1-ceiling.patch` (bumps capped at three times the fresh estimate)
