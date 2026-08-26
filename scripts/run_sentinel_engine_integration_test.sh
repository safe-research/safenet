#!/bin/bash
# Starts the reference sentinel engine and replays the external test-vector
# corpus against it.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/shared_test_scripts.sh"

ENGINE_ADDR="127.0.0.1:5473"
ENGINE_URL="http://$ENGINE_ADDR"
RPC_URL="${SENTINEL_ENGINE_RPC_URL:-https://ethereum-rpc.publicnode.com}"

if [ -z "${TEST_VECTORS:-}" ]; then
    echo "TEST_VECTORS must point to the root of the sentinel-test-vectors repository." >&2
    exit 1
fi

TEST_RUNNER="$TEST_VECTORS/bin/run-tests.sh"
if [ ! -x "$TEST_RUNNER" ]; then
    echo "Test-vector runner is missing or not executable: $TEST_RUNNER" >&2
    exit 1
fi

require_commands cargo curl jq

TMPDIR="$(mktemp -d)"
ENGINE_PID=""
cleanup() {
    if [ -n "$ENGINE_PID" ]; then
        kill "$ENGINE_PID" 2>/dev/null || true
        wait "$ENGINE_PID" 2>/dev/null || true
    fi
    rm -rf "$TMPDIR"
}
trap cleanup EXIT

CONFIG_FILE="$TMPDIR/sentinel-engine.toml"
cat >"$CONFIG_FILE" <<EOF
rpc = "$RPC_URL"
bind_address = "$ENGINE_ADDR"

[engine]
blocklist = []
address_poisoning_lookback_blocks = 50000
EOF

echo "==> Building the sentinel engine..."
cargo build --package sentinel-engine

echo "==> Starting the sentinel engine..."
"$ROOT/target/debug/sentinel-engine" --config-file "$CONFIG_FILE" >"$ROOT/sentinel_engine_logs.txt" 2>&1 &
ENGINE_PID=$!

ENGINE_READY=0
for _ in $(seq 1 20); do
    if curl --silent --output /dev/null "$ENGINE_URL"; then
        ENGINE_READY=1
        break
    fi
    if ! kill -0 "$ENGINE_PID" 2>/dev/null; then
        echo "Sentinel engine exited during startup. See $ROOT/sentinel_engine_logs.txt." >&2
        exit 1
    fi
    sleep 0.25
done
if [ "$ENGINE_READY" -ne 1 ]; then
    echo "Sentinel engine did not start at $ENGINE_URL. See $ROOT/sentinel_engine_logs.txt." >&2
    exit 1
fi

echo "==> Running sentinel engine test vectors..."
SENTINEL_ENGINE_URL="$ENGINE_URL" "$TEST_RUNNER" "$@"
