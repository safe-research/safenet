# QA3-CORE process helpers. Source after the env file. Only touches processes started here,
# tracked by pid files under $QA3/pids.
QA3=/home/sj.guest/.cache/kb-runs/safenet/2026-10-08-rust-audit-run3/scratch/QA3-CORE
mkdir -p "$QA3/pids" "$QA3/logs"

# start_anvil <name> <port> [extra anvil args...]
# start_bg <name> <log> <command...>: any background process, pid tracked
start_bg() {
  local name=$1 logf=$2; shift 2
  setsid nohup bash -c 'echo $$ > "$0"; exec "$@"' "$QA3/pids/$name.pid" "$@" > "$logf" 2>&1 &
  sleep 0.3
}

start_anvil() {
  local name=$1 port=$2; shift 2
  setsid nohup bash -c 'echo $$ > "$0"; exec anvil --host 127.0.0.1 --port "$@"' "$QA3/pids/$name.pid" "$port" --prune-history "$@" > "$QA3/logs/$name.log" 2>&1 &
  sleep 0.3
  for _ in $(seq 1 50); do
    cast block-number --rpc-url "http://127.0.0.1:$port" > /dev/null 2>&1 && return 0
    sleep 0.2
  done
  echo "anvil $name did not start" >&2; return 1
}

# start_proxy <name> <port> <upstream_url> <control_json>
start_proxy() {
  local name=$1 port=$2 upstream=$3 control=$4
  setsid nohup bash -c 'echo $$ > "$0"; exec python3 -I "$@"' "$QA3/pids/$name.pid" "$QA3/rpcproxy.py" "$port" "$upstream" "$control" "$QA3/logs/$name.jsonl" > "$QA3/logs/$name.out" 2>&1 &
  sleep 0.3
  for _ in $(seq 1 50); do
    cast chain-id --rpc-url "http://127.0.0.1:$port" > /dev/null 2>&1 && return 0
    sleep 0.2
  done
  echo "proxy $name did not start" >&2; return 1
}

# stop <name> [signal]: stop a process started by this helper
stop() {
  local name=$1 sig=${2:-TERM} pid
  pid=$(cat "$QA3/pids/$name.pid" 2>/dev/null) || return 0
  if kill -0 "$pid" 2>/dev/null; then
    kill -"$sig" "$pid" 2>/dev/null
    for _ in $(seq 1 50); do kill -0 "$pid" 2>/dev/null || break; sleep 0.2; done
  fi
  mv "$QA3/pids/$name.pid" "$QA3/pids/$name.pid.stopped"
}

# running: list the processes this helper still tracks
running() {
  for f in "$QA3"/pids/*.pid; do
    [ -e "$f" ] || continue
    local pid; pid=$(cat "$f")
    if kill -0 "$pid" 2>/dev/null; then echo "$(basename "$f" .pid) $pid"; fi
  done
}
