#!/usr/bin/env bash
# Start the shipped sentinel sample (rpc and database pointed at loopback only) against a local
# Anvil with Gnosis's chain id, let it run for a few seconds, then stop both processes this
# script started.
set -u
S="$1"; PORT="$2"; BIN="$3"
anvil --host 127.0.0.1 --port "$PORT" --chain-id 100 --block-time 1 --prune-history > "$S/anvil.log" 2>&1 &
ANVIL=$!
echo "anvil pid $ANVIL"
for _ in $(seq 1 50); do
  if curl -s -X POST -H 'content-type: application/json' --data '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' "http://127.0.0.1:$PORT" > /dev/null 2>&1; then break; fi
  sleep 0.2
done
for cfg in sample window0; do
  echo "=== $cfg: effective rpc $(grep '^rpc' "$S/$cfg.toml")"
  timeout -s INT 12 "$BIN" --config-file "$S/$cfg.toml" > "$S/$cfg.log" 2>&1
  echo "=== $cfg exit $?"
done
kill "$ANVIL"
wait "$ANVIL" 2>/dev/null
echo "anvil stopped"
