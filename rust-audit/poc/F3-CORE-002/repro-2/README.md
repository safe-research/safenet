# F3-CORE-002 second reproduction (QA3-2B)

Real `sentinel` binary with `use_client_filtering = true`, behind a loopback JSON-RPC proxy in front of Anvil (chain 100). Distinct from the suggested unit test: the real indexer loses a block's watched events.

Route: the proxy returns an empty eth_getLogs for the proposal block's hash (its header bloom is non-zero, it holds NewRequest and TransactionProposed) for a window longer than the three client-filtered attempts. After three bloom mismatches the watcher falls back to per-event node-filtered queries, whose empty answer is accepted, so the block is committed with no events.

## Command

```
source .../env/qa-2b.sh
python3 -I core002.py <sentinel-binary> <label> [suppress-seconds]
```

## Result: reproduced

Pristine `sentinel-361aaf3`, 40 s suppression:

```
proposal in block 11 logsBloom_nonzero=True; suppressing its logs for 40s
while suppressed (anvil head 14): seen=14  (advanced PAST the proposal block, committed empty)
onchain committed=0 state=PENDING; engine checks=0
after suppression ended: seen=25; onchain committed=0; engine checks=0
'incomplete logs served for block, bloom filter mismatch' warn count: 3
verdict: advanced_past_proposal_while_suppressed=True engine_ran_eventually=False sentinel_lost_the_event=True
```

The integrity check fires exactly three times then gives up; block 11 is committed empty, the sentinel never runs an engine check, never votes, and never re-queries the block even after its logs become available. The proposal is lost for good.

## Bite check: sound

Remediation option 2 (`may_contain_log` on the per-event fallback answer), rebuilt sentinel:

- 40 s suppression: `seen=11 processed=10` while suppressed (the watcher stalls on block 11 instead of committing it empty), `engine_ran_eventually=True` once logs return.
- 15 s suppression (ends before the 6-block commit window): `state=RESOLVED_APPROVED committed=1`, `request_committed_after=True` -- the sentinel reads the proposal after the retry, votes, and the request resolves.

The fix stops the silent loss; the suspect empty block is retried until the real logs arrive. Remediation sound.
