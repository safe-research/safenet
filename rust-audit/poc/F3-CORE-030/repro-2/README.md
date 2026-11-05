# F3-CORE-030 second reproduction (QA3-2B)

Real `sentinel` binary plus two real Anvils, the Critic's second-reproduction recipe. No mocks: a stale RPC head at startup is the whole trigger.

## Command

```
source .../env/qa-2b.sh
python3 -I core030.py <sentinel-binary> <label>
```

`core030.py` runs the sentinel against Anvil1 (chain 100, 5 s blocks) until it has snapshotted to block ~32, stops it, starts a second Anvil forked at block 12 (far below the oldest retained snapshot) mining forward, and restarts the sentinel against it.

## Result: reproduced

Pristine binary (`sentinel-361aaf3`), restart against the behind node:

```
before stop: anvil1 head=32, sentinel block metrics seen=32 processed=32
run 1 exit status: 0
anvil2 forked at 12, head now 12
initializing block watcher {latest: 12, safe: 7, resume: Some(BlockStatus { latest: 32, safe: 27 })}
ERROR unrecoverable driver error; exiting {err: State(BadUpdate)}
restart against the behind node EXITED after 1.5s, status 0
```

The node head (12) is below the oldest retained snapshot (safe 27), so the first live block is far below the resume point and the state machine rejects it as `BadUpdate`; the driver exits with status 0.

## Bite check: sound

Remediation option 1 applied to `crates/core/src/index/blocks.rs` `initialize` (wait while `latest.number < indexed.safe`). Rebuilt sentinel, same scenario:

```
WARN RPC node is behind the oldest retained snapshot; waiting for it to catch up {latest: 12, safe: 27}
... (repeats as anvil2 mines 13..26) ...
WARN RPC node is behind the oldest retained snapshot; waiting for it to catch up {latest: 26, safe: 27}
DEBUG initializing block watcher {latest: 27, safe: 22, resume: Some(BlockStatus { latest: 32, safe: 27 })}
```

The patched sentinel waits instead of exiting, then resumes once the node reaches block 27. No `BadUpdate` exit. Remediation is sound.
