#!/usr/bin/env bash
# Run f008_usdce.py against a loopback Anvil (Gnosis chain id) that this script starts and stops.
set -u
S="$1"; PORT="$2"; CACHE="$3"; FORK="${4:-latest}"
echo "hardfork: $FORK"
anvil --host 127.0.0.1 --port "$PORT" --chain-id 100 --prune-history --hardfork "$FORK" > "$S/anvil-f008.log" 2>&1 &
ANVIL=$!
for _ in $(seq 1 50); do
  if cast chain-id --rpc-url "http://127.0.0.1:$PORT" > /dev/null 2>&1; then break; fi
  sleep 0.2
done
echo "anvil pid $ANVIL, hardfork: $(grep -i -m1 'hardfork\|spec' "$S/anvil-f008.log" || true)"
python3 -I "$S/f008_usdce.py" "$CACHE" "$PORT"
kill "$ANVIL"
wait "$ANVIL" 2>/dev/null
echo "anvil stopped"
