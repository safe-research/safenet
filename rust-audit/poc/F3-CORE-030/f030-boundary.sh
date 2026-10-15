#!/usr/bin/env bash
# F3-CORE-030 boundary on the real sentinel binary: with mining frozen after run 1, restart
# against a node whose head is exactly S - 1 or S (S = oldest retained snapshot), then mine
# one block. Usage: f030-boundary.sh <label> <lag>   (lag 6 gives head S - 1, lag 5 gives S)
set -u
source /home/sj.guest/.cache/kb-runs/safenet/2026-10-08-rust-audit-run3/env/rv-core-a.sh
source "$SAFENET_REVIEW_ROOT/scratch/QA3-CORE/procs.sh"
label=$1 lag=$2
D=$QA3/f030b-$label-lag$lag
mkdir -p "$D"
BIN=$CARGO_TARGET_DIR/debug/sentinel
A=http://127.0.0.1:31200
echo '{}' > "$D/ctl.json"
start_anvil "anvil-030b-$label-$lag" 31200 --block-time 1 || exit 1
start_proxy "proxy-030b-$label-$lag" 31201 $A "$D/ctl.json" || exit 1
sed -e "s#sqlite:[^\"]*#sqlite:$D/sentinel.db?mode=rwc#" "$QA3/f030-base/sentinel.toml" > "$D/sentinel.toml"
snapshots() {
  python3 -I -c "import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); print([r[0] for r in c.execute('select block_number from snapshots order by 1')])" "$D/sentinel.db"
}
start_bg "sentinel-030b-$label-$lag-run1" "$D/run1.log" "$BIN" --config-file "$D/sentinel.toml"
for _ in $(seq 1 60); do
  [ "$(cast block-number --rpc-url $A)" -ge 20 ] && break
  sleep 1
done
cast rpc --rpc-url $A evm_setIntervalMining 0 > /dev/null
sleep 3
stop "sentinel-030b-$label-$lag-run1" INT
echo "run 1: mining frozen at upstream head $(cast block-number --rpc-url $A); snapshots $(snapshots)"
echo "{\"lag\": $lag}" > "$D/ctl.json"
echo "restart against a node at head $(cast block-number --rpc-url http://127.0.0.1:31201) (lag $lag); one block mined 3 s later"
start_bg "sentinel-030b-$label-$lag-run2" "$D/run2.log" "$BIN" --config-file "$D/sentinel.toml"
sleep 3
cast rpc --rpc-url $A anvil_mine 1 > /dev/null
sleep 4
pid=$(cat "$QA3/pids/sentinel-030b-$label-$lag-run2.pid")
if kill -0 "$pid" 2>/dev/null; then echo "run 2: still running 4 s after the new block"; else echo "run 2: process exited"; fi
stop "sentinel-030b-$label-$lag-run2" INT
echo "run 2 snapshots: $(snapshots)"
grep -E 'initializing block watcher|unrecoverable|received shutdown|waiting' "$D/run2.log" | cut -c1-300
stop "proxy-030b-$label-$lag"
stop "anvil-030b-$label-$lag"
running
