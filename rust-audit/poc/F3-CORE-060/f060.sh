#!/usr/bin/env bash
# F3-CORE-060: the real Driver and TransactionQueue (Direct mode, defaults) follow a real Anvil
# chain through a proxy that accepts eth_sendRawTransaction without forwarding it (an endpoint
# that accepts but does not propagate). One action is queued at startup. Every swallowed raw
# transaction is decoded, then the last one is sent to Anvil to see the tip it pays.
# Usage: f060.sh <label> [seconds]
set -u
source /home/sj.guest/.cache/kb-runs/safenet/2026-10-08-rust-audit-run3/env/rv-core-a.sh
source "$SAFENET_REVIEW_ROOT/scratch/QA3-CORE/procs.sh"
label=$1-$(date +%s) seconds=${2:-90}
D=$QA3/f060-$label
mkdir -p "$D"
A=http://127.0.0.1:31202
# Gnosis chain id; a 0.042 gwei starting base fee; the proxy reports a 1 gwei tip in every
# block's fee history, as on Gnosis where the tip is most of the price.
start_anvil "anvil-060-$label" 31202 --chain-id 100 --block-time 1 --block-base-fee-per-gas 42000000 || exit 1
echo '{"swallow_send": true, "fee_history_reward_wei": 1000000000}' > "$D/ctl.json"
start_proxy "proxy-060-$label" 31203 $A "$D/ctl.json" || exit 1
echo "== harness: real Driver and queue for $seconds s through the swallowing proxy"
"$CARGO_TARGET_DIR/debug/examples/qa3_fee" http://127.0.0.1:31203 "$seconds" "$D/queue.db" > "$D/harness.log" 2>&1
grep -E 'transaction queue config|harness stopped' "$D/harness.log"
grep -c 'submitting transaction' "$D/harness.log" | sed 's/^/submissions logged by the queue: /'

echo "== swallowed submissions (decoded with cast decode-tx)"
python3 -I - "$QA3/logs/proxy-060-$label.jsonl" "$D/swallowed.tsv" <<'EOF'
import json, subprocess, sys
rows = []
for line in open(sys.argv[1]):
    entry = json.loads(line)
    if entry.get("swallowed"):
        tx = json.loads(subprocess.run(["cast", "decode-tx", entry["raw"], "--json"], capture_output=True, text=True, check=True).stdout)
        rows.append((entry["t"], int(tx["nonce"], 16), int(tx["maxPriorityFeePerGas"], 16), int(tx["maxFeePerGas"], 16), entry["raw"]))
first_tip = rows[0][2]
with open(sys.argv[2], "w") as out:
    for i, (t, nonce, tip, max_fee, raw) in enumerate(rows):
        out.write(f"{i}\t{t}\t{nonce}\t{tip}\t{max_fee}\t{raw}\n")
for i, (t, nonce, tip, max_fee, raw) in enumerate(rows):
    if i < 3 or i % 10 == 0 or i == len(rows) - 1:
        print(f"submission {i:3d} at +{t - rows[0][0]:5.1f} s: nonce {nonce} tip {tip:>16} wei ({tip / first_tip:8.2f}x first) max fee {max_fee:>16} wei; 21000 gas x max fee = {21000 * max_fee / 1e18:.9f} native")
EOF

echo "== land the last swallowed transaction on Anvil"
last=$(tail -n 1 "$D/swallowed.tsv")
raw=$(echo "$last" | cut -f6)
first_tip=$(head -n 1 "$D/swallowed.tsv" | cut -f4)
published=$(timeout 30 cast publish --rpc-url $A "$raw" 2>&1 | tail -n 1)
if [ "${published#\{}" != "$published" ]; then
  receipt=$published
else
  for _ in $(seq 1 20); do
    receipt=$(cast receipt --rpc-url $A "$published" --json 2>/dev/null) && break
    sleep 1
  done
fi
echo "published: $(echo "$receipt" | jq -r .transactionHash)"
block=$(echo "$receipt" | jq -r .blockNumber)
egp=$(echo "$receipt" | jq -r .effectiveGasPrice)
base=$(cast block --rpc-url $A "$block" --json | jq -r .baseFeePerGas)
python3 -I -c "
import sys
egp, base, gas, first = (int(x, 0) for x in sys.argv[1:5])
print(f'landed in block {int(sys.argv[5], 0)}: status {sys.argv[6]}, effectiveGasPrice {egp} wei, base fee {base} wei, tip paid {egp - base} wei = {(egp - base) / first:.1f}x the first submission tip; fee paid {gas * egp / 1e18:.9f} native for {gas} gas')
" "$egp" "$base" "$(echo "$receipt" | jq -r .gasUsed)" "$first_tip" "$block" "$(echo "$receipt" | jq -r .status)"

stop "proxy-060-$label"
stop "anvil-060-$label"
running
