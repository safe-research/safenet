# F3-CORE-005 PoC (QA3-CORE)

Two levels.

1. Real sentinel binary, `f005.sh`: Anvil with chain id 100 (so the watcher uses the Gnosis timings: 5 s blocks, 500 ms propagation delay, the retry delays) mines a block every 5 s of wall time; `--timestamp now+offset` keeps every header timestamp `offset` seconds ahead of the host clock, which is the same as a host clock `offset` seconds behind. A poller records when each block first appears on Anvil; the lag is the time until the sentinel logs it as a new canonical block (trace log of `safenet_core::index::blocks`).
2. Unit, `unit-test.patch`: paused tokio time, a time-aware mock chain (block `1000 + k` exists from `5 k` s on; `null` before that) behind a test-only `Provider::qa3_from_transport`, Gnosis timings, the real `BlockWatcher`.

## Run

```sh
./f005.sh synced 0 45 && ./f005.sh behind10 10 45      # needs procs.sh, the sentinel binary at 361aaf3 and Anvil 1.5.1
git apply unit-test.patch
cargo test -p safenet-core --lib qa3_host_clock_behind_delays_every_block_by_the_offset -- --nocapture
```

Bite check: `remediation1-skip-wait.patch` (the fetch does not wait while the newest header is ahead of the host clock), rebuilt into the sentinel for the binary run; `unit-test-plus-remediation1.patch` for the unit run.

## Files

- `sentinel-lag-at-361aaf3.txt`, `sentinel-lag-with-remediation1.txt`
- `unit-test-at-361aaf3.txt`, `unit-test-with-remediation1.txt`
