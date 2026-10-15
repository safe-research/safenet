#!/usr/bin/env bash
# F3-CORE-030 real-binary reproduction: restart the sentinel against an RPC node that is
# behind the oldest retained snapshot. Usage: f030.sh <label>
# The proxy serves the same Anvil chain as a node `lag` blocks behind (no fork, same hashes).
set -u
source /home/sj.guest/.cache/kb-runs/safenet/2026-10-08-rust-audit-run3/env/rv-core-a.sh
source "$SAFENET_REVIEW_ROOT/scratch/QA3-CORE/procs.sh"
label=$1
D=$QA3/f030-$label
mkdir -p "$D"
BIN=$CARGO_TARGET_DIR/debug/sentinel
echo '{}' > "$D/ctl.json"
start_anvil "anvil-030-$label" 31200 --block-time 1 || exit 1
start_proxy "proxy-030-$label" 31201 http://127.0.0.1:31200 "$D/ctl.json" || exit 1

printf '%s\n' \
  'rpc = "http://127.0.0.1:31201"' \
  'signer = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"' \
  "database = \"sqlite:$D/sentinel.db?mode=rwc\"" \
  'oracle = "0x0101010101010101010101010101010101010101"' \
  'consensus = "0x0202020202020202020202020202020202020202"' \
  '' '[sentinel]' \
  'fee_token = "0x0303030303030303030303030303030303030303"' \
  'voting_window = 6' \
  'engine = "http://127.0.0.1:31209"' \
  '' '[observability]' \
  'log_filter = "info,safenet_core::driver=debug,safenet_core::index=debug"' \
  '' '[index]' \
  '# Anvil mines a block per second; the trigger depends on block numbers only.' \
  'block_time = 1000' \
  'block_propagation_delay = 0' \
  'block_retry_delays = []' > "$D/sentinel.toml"

snapshots() {
  python3 -I -c "import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); print([r[0] for r in c.execute('select block_number from snapshots order by 1')])" "$D/sentinel.db"
}

echo "== run 1: sentinel follows the chain through an up-to-date node"
start_bg "sentinel-030-$label-run1" "$D/run1.log" "$BIN" --config-file "$D/sentinel.toml"
for _ in $(seq 1 60); do
  [ "$(cast block-number --rpc-url http://127.0.0.1:31200)" -ge 25 ] && break
  sleep 1
done
sleep 2
stop "sentinel-030-$label-run1" INT
L=$(python3 -I -c "import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); print(c.execute('select max(block_number) from snapshots').fetchone()[0])" "$D/sentinel.db")
echo "run 1 stopped (SIGINT); last processed block L=$L; snapshots: $(snapshots)"
tail -n 2 "$D/run1.log"

echo '{"lag": 30}' > "$D/ctl.json"
echo "== node now lags 30 blocks: upstream head $(cast block-number --rpc-url http://127.0.0.1:31200), proxy head $(cast block-number --rpc-url http://127.0.0.1:31201)"

for run in 2 3; do
  echo "== run $run: restart against the lagging node (20 s limit)"
  start=$(date +%s.%N)
  timeout -s INT 20 "$BIN" --config-file "$D/sentinel.toml" > "$D/run$run.log" 2>&1
  status=$?
  end=$(date +%s.%N)
  echo "run $run exit status: $status after $(python3 -c "print(round($end-$start,1))") s; snapshots: $(snapshots)"
  grep -E 'initializing block watcher|unrecoverable|BadUpdate|received shutdown|waiting' "$D/run$run.log" | cut -c1-400
done

if [ "$label" != "base" ]; then
  echo "== run 4: restart against the lagging node, node catches up 8 s later"
  start_bg "sentinel-030-$label-run4" "$D/run4.log" "$BIN" --config-file "$D/sentinel.toml"
  sleep 8
  echo '{}' > "$D/ctl.json"
  sleep 8
  stop "sentinel-030-$label-run4" INT
  echo "run 4 snapshots: $(snapshots)"
  grep -E 'initializing block watcher|unrecoverable|BadUpdate|received shutdown|waiting' "$D/run4.log" | cut -c1-400
else
  echo '{}' > "$D/ctl.json"
  echo "== run 4: restart once the node has caught up (10 s limit)"
  timeout -s INT 10 "$BIN" --config-file "$D/sentinel.toml" > "$D/run4.log" 2>&1
  echo "run 4 exit status: $?; snapshots: $(snapshots)"
  grep -E 'initializing block watcher|unrecoverable|BadUpdate|received shutdown' "$D/run4.log" | cut -c1-400
fi

stop "proxy-030-$label"
stop "anvil-030-$label"
running
