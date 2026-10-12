# F3-XC-160 PoC (QA3-CORE)

1. Watcher, `watcher-test.patch`: a copy of `recovers_from_an_uncled_block_with_unavailable_logs` whose revalidation answers `null`. The watcher emits `Uncle { 1000 }`, then the very same block 1000 (same hash) as new.
2. End to end, `f160.sh <label> <inject>`: two real sentinels, each with a real sentinel-engine, vote on one request against the real `SentinelOracle` on a local Anvil (1 s blocks; Aegis amounts in 6-decimal base units: fee 400000 = 0.40, bond multiplier 2000 = 800, slashing 1x; commit and reveal windows 6 blocks; arbitration timeout 483,840). Sentinel A reads the chain through `rpcproxy.py`; sentinel B reads Anvil directly. With `inject` 1 the proxy models a load-balanced endpoint with one backend a block behind: after it serves the logs of block P holding `NewRequest`, it answers the next `eth_getLogs` by hash of block P + 1 with -32001 and the next `eth_getBlockByNumber(P + 1)` with `null`, once each. With `inject` 0 it only forwards (control).

## Run

```sh
git apply watcher-test.patch
cargo test -p safenet-core --lib qa3_lagging_backend_uncles_a_canonical_block -- --nocapture
./f160.sh control 0 && ./f160.sh inject 1        # needs procs.sh, the sentinel and sentinel-engine binaries, forge and Anvil 1.5.1
git apply remediation1.patch && cargo build -p sentinel && ./f160.sh inject-fix 1
```

## Files

- `watcher-test-at-361aaf3.txt`, `watcher-test-with-remediation1.txt` (the existing test `revalidate_invalidates_a_block_missing_from_rpc` also fails with the remediation)
- `e2e-control-no-injection.txt`, `e2e-injection-at-361aaf3.txt`, `e2e-injection-with-remediation1.txt`
- `e2e-injection-sentinel-a-log-excerpt.txt`, `e2e-injection-proxy-log.jsonl`, `e2e-sentinel-a.toml` (key redacted)
- `anvil-client-answers.txt`: what Anvil 1.5.1 itself answers for logs of an unknown block hash (`[]`) and a block above its head (`null`).
