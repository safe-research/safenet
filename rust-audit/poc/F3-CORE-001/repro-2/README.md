# F3-CORE-001 second reproduction (QA3-2B)

Real `sentinel` binary behind a loopback JSON-RPC proxy in front of Anvil (chain 100, 5 s blocks). Distinct from the suggested unit test: the real indexer stalls under a real rate limit.

Route: the proxy serves everything during warmup, then a short eth_getLogs blackout forces three single-query failures on a new block so the event watcher escalates to one eth_getLogs per watched event (11 for the sentinel). From then on the proxy caps concurrent eth_getLogs below 11 (a limit that would serve a single query but cannot admit 11 at once), so no per-event attempt ever completes.

## Command

```
source .../env/qa-2b.sh
python3 -I core001.py <sentinel-binary> <label>
```

## Result: reproduced

Pristine `sentinel-361aaf3`:

```
after warmup: block metrics seen=10 processed=10; /health=(200, 'OK')
during stall:  block metrics seen=11 processed=10; /health=(200, 'OK'); anvil head=20
12 s later:    block metrics seen=11 processed=10; eth_getLogs rate ~90/s; alive=True
per-event bursts widths seen: [1, 2, 4, 12..22]  (>=8-wide bursts: 257)
'failed to get next blockchain update; retrying after delay' warn count: 518
verdict: indexer_stalled=True health_ok_while_stalled=True process_alive=True escalated_to_per_event_11=True high_request_rate=True
```

The indexer is permanently stuck at block 11 while the chain advances to 20, firing ~90 eth_getLogs per second in 11-wide per-event bursts with a flat 100 ms retry and no backoff. `/health` keeps returning 200 OK, so no restart policy or liveness probe reacts.

## Bite check: sound

Remediation option 2 (run the per-event queries sequentially instead of concurrently), rebuilt sentinel, same scenario:

```
during stall: seen=21 processed=21 (tracking anvil head)
12 s later:   seen=23 processed=23; eth_getLogs rate ~0/s
>=8-wide per-event bursts: 0; burst widths seen: [1..5]
verdict: indexer_stalled=False escalated_to_per_event_11=False
```

Sequential per-event queries (1 at a time) pass the concurrency cap, so the attempt completes and the indexer recovers once the blackout ends. Remediation sound.
