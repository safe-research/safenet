# F3-CORE-001 PoC (QA3-CORE)

The real `Driver` (real `STEP_RETRY_DELAY` of 100 ms, real time, 1 s blocks) follows a chain through an RPC that is mocked at the transport boundary only: an in-process tower service behind `Provider`, answering `eth_getBlockByNumber` and `eth_getLogs` after a 20 ms round trip. The service watches 16 events, as the validator does. Each run starts with a short outage (HTTP 502 on `eth_getLogs`), then a provider limit that answers HTTP 429 above it, the same error the real reqwest transport returns for a 429 (`alloy-transport-http-2.0.5/src/reqwest_transport.rs:63-65`).

Variants, 30 s each, run concurrently:

| Variant | Limit | Expected |
| --- | --- | --- |
| A | fixed window, 10 requests per 150 ms | stall |
| B | A with `block_single_query_retry_count = 1000` (control) | follows the chain |
| C | at most 8 requests in flight | stall |
| D | token bucket, burst 12, 100 per s | stall |
| E | token bucket, burst 20, 50 per s, first per-event attempt inside the outage | stall |
| G | E with the outage over before the first per-event attempt (control) | follows the chain |
| F | restart warp from block 800 with A's window and a 1 s outage | stall on a single-block page |

## Run

In a worktree at `361aaf3`:

```sh
git apply driver-rate-limit-test.patch
cargo test -p safenet-core --lib qa3_rate_limited_rpc_stalls_the_watcher -- --nocapture
QA3_OUTAGE_MS=3000 cargo test -p safenet-core --lib qa3_rate_limited_rpc_stalls_the_watcher -- --nocapture
```

`QA3_OUTAGE_MS` overrides the outage of every variant except the controls (variant F included, so its label then understates the outage).

Bite checks, each on top of the test patch:

```sh
git apply remediation2-block-path.patch   # back to a single query after a failed per-event attempt (new-block path only)
git apply remediation1-backoff.patch      # exponential backoff in Driver::next_input, 100 ms doubling to 6.4 s
```

## Files

- `driver-rate-limit-test.patch`: the test module (in `driver.rs`) and a test-only `Provider::qa3_from_transport`.
- `driver-test-at-361aaf3.txt`, `driver-test-at-361aaf3-outage3s.txt`: the stall at the audit commit.
- `driver-test-with-remediation2-block-path*.txt`: remediation 2 on the new-block path.
- `driver-test-with-remediation1-backoff*.txt`: remediation 1 (backoff) alone.
