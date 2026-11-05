#!/usr/bin/env bash
# F3-CORE-005 on the real sentinel binary: Anvil with chain id 100 (so the watcher uses the
# Gnosis timings: 5 s blocks, 500 ms propagation delay, retry delays) mines a block every 5 s of
# wall time, with header timestamps `offset` seconds ahead of the host clock (equivalently, a host
# clock `offset` seconds behind the chain). The lag is the time from Anvil mining a block (wall
# clock, from its RPC) to the sentinel logging it as a new canonical block.
# Usage: f005.sh <label> <offset seconds> [seconds to run]
set -u
source /home/sj.guest/.cache/kb-runs/safenet/2026-10-08-rust-audit-run3/env/rv-core-a.sh
source "$SAFENET_REVIEW_ROOT/scratch/QA3-CORE/procs.sh"
label=$1-$(date +%s) offset=$2 seconds=${3:-50}
D=$QA3/f005-$label
mkdir -p "$D"
A=http://127.0.0.1:31206
cleanup() { stop "sentinel-005-$label"; stop "anvil-005-$label"; }
trap cleanup EXIT
start_anvil "anvil-005-$label" 31206 --chain-id 100 --block-time 5 --timestamp $(( $(date +%s) + offset )) || exit 1
printf '%s\n' "rpc = \"$A\"" 'signer = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"' \
  "database = \"sqlite:$D/sentinel.db?mode=rwc\"" 'oracle = "0x0101010101010101010101010101010101010101"' \
  'consensus = "0x0202020202020202020202020202020202020202"' '' '[sentinel]' \
  'fee_token = "0x0303030303030303030303030303030303030303"' 'voting_window = 6' 'engine = "http://127.0.0.1:31209"' \
  '' '[observability]' 'log_filter = "info,safenet_core::index::blocks=trace"' > "$D/sentinel.toml"
start_bg "sentinel-005-$label" "$D/sentinel.log" "$CARGO_TARGET_DIR/debug/sentinel" --config-file "$D/sentinel.toml"
# Record when each block first appears on Anvil (wall clock), polling every 100 ms.
python3 -I - "$A" "$seconds" "$D/mined.tsv" <<'EOF'
import json, sys, time, urllib.request
url, seconds, out = sys.argv[1], float(sys.argv[2]), sys.argv[3]
def rpc(method, params):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    with urllib.request.urlopen(urllib.request.Request(url, data=body, headers={"Content-Type": "application/json"})) as r:
        return json.loads(r.read())["result"]
seen = {}
end = time.time() + seconds
while time.time() < end:
    n = int(rpc("eth_blockNumber", []), 16)
    if n not in seen:
        seen[n] = time.time()
    time.sleep(0.1)
with open(out, "w") as f:
    for n, t in sorted(seen.items()):
        ts = int(rpc("eth_getBlockByNumber", [hex(n), False])["timestamp"], 16)
        f.write(f"{n}\t{t:.3f}\t{ts}\n")
EOF
python3 -I - "$D/mined.tsv" "$D/sentinel.log" "$offset" <<'EOF'
import json, sys
from datetime import datetime
mined = {}
for line in open(sys.argv[1]):
    n, t, ts = line.split()
    mined[int(n)] = (float(t), int(ts))
lags = []
for line in open(sys.argv[2]):
    try:
        e = json.loads(line)
    except Exception:
        continue
    f = e.get("fields", {})
    if f.get("message") != "new canonical block":
        continue
    n = int(f["number"])
    logged = datetime.fromisoformat(e["timestamp"].replace("Z", "+00:00")).timestamp()
    if n in mined and n > min(mined):
        t, ts = mined[n]
        lags.append((n, round(logged - t, 2), ts - round(t)))
print(f"header timestamps ahead of the host clock by about {sys.argv[3]} s")
for n, lag, ahead in lags:
    print(f"block {n}: header ts - wall mined time = {ahead:+d} s; seen by the sentinel {lag:5.2f} s after Anvil mined it")
if lags:
    print(f"mean lag over {len(lags)} blocks: {sum(l for _, l, _ in lags) / len(lags):.2f} s")
EOF
