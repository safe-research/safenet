#!/bin/bash
# Cross-implementation genesis key generation integration test.
#
# Starts Anvil, deploys the contracts, and runs the TypeScript and Rust
# validators as members of the genesis group. The test succeeds once both
# implementations have confirmed the generated key onchain.
#
# Requirements: anvil, forge, cast, jq, cargo, node, and npm.
set -euo pipefail

ANVIL_RPC_URL="${ANVIL_RPC_URL:-http://127.0.0.1:8545}"
CHAIN_ID=31337
TIMEOUT="${TIMEOUT:-120}"

# Anvil accounts 1 and 2 are the TypeScript and Rust validators respectively.
PARTICIPANTS=(
    0x70997970C51812dc3A010C7d01b50e0d17dc79C8
    0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC
)
PRIVATE_KEYS=(
    0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d
    0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a
)

# Anvil default deployer account (index 0).
SENDER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TMPDIR="$(mktemp -d)"
PIDS=()

dump_validator_logs() {
    echo
    for name in ts rust; do
        echo "=== validator_${name}.log ==="
        cat "$TMPDIR/validator_${name}.log" 2>/dev/null || echo "(empty)"
        echo
    done
}

cleanup() {
    for pid in "${PIDS[@]:-}"; do
        kill "$pid" 2>/dev/null || true
    done
    dump_validator_logs
    rm -rf "$TMPDIR"
}
trap cleanup EXIT

for command in anvil cast forge jq cargo node npm; do
    command -v "$command" >/dev/null || {
        echo "Missing required command: $command" >&2
        exit 1
    }
done

echo "==> Using temporary directory $TMPDIR"

echo "==> Building Rust validator..."
cargo build --manifest-path "$REPO_ROOT/Cargo.toml" --package validator
RUST_BIN="$REPO_ROOT/target/debug/validator"

echo "==> Starting Anvil..."
anvil --block-time 1 > "$TMPDIR/anvil.log" 2>&1 &
PIDS+=("$!")
for _ in $(seq 1 20); do
    cast block-number --rpc-url "$ANVIL_RPC_URL" >/dev/null 2>&1 && break
    sleep 0.25
done
cast block-number --rpc-url "$ANVIL_RPC_URL" >/dev/null

echo "==> Deploying contracts..."
PARTICIPANTS_CSV=$(IFS=,; echo "${PARTICIPANTS[*]}")
env PARTICIPANTS="$PARTICIPANTS_CSV" \
    npm run --prefix "$REPO_ROOT" --workspace contracts cmd:deploy -- \
    --rpc-url "$ANVIL_RPC_URL" \
    --unlocked \
    --sender "$SENDER" \
    --broadcast 2>&1 | tee "$TMPDIR/deploy.log"

DEPLOY_JSON="$REPO_ROOT/contracts/build/broadcast/Deploy.s.sol/$CHAIN_ID/run-latest.json"
COORDINATOR_ADDR=$(jq -er '.returns.coordinator.value' "$DEPLOY_JSON")
CONSENSUS_ADDR=$(jq -er '.returns.consensus.value' "$DEPLOY_JSON")
echo "    coordinator: $COORDINATOR_ADDR"
echo "    consensus:   $CONSENSUS_ADDR"

TS_PARTICIPANTS_JSON=$(printf '%s\n' "${PARTICIPANTS[@]}" | jq -R '{address: ., activeFrom: 0}' | jq -sc '.')

echo "==> Starting TypeScript validator (${PARTICIPANTS[0]})..."
env \
    RPC_URL="$ANVIL_RPC_URL" \
    PRIVATE_KEY="${PRIVATE_KEYS[0]}" \
    CONSENSUS_ADDRESS="$CONSENSUS_ADDR" \
    COORDINATOR_ADDRESS="$COORDINATOR_ADDR" \
    CHAIN_ID="$CHAIN_ID" \
    PARTICIPANTS="$TS_PARTICIPANTS_JSON" \
    STORAGE_FILE="$TMPDIR/validator_ts.sqlite" \
    BLOCK_TIME_OVERRIDE=1000 \
    START_FROM_BLOCK=0 \
    LOG_LEVEL=debug \
    METRICS_PORT=0 \
    npm run --prefix "$REPO_ROOT" --workspace validator dev \
    > "$TMPDIR/validator_ts.log" 2>&1 &
PIDS+=("$!")
echo "    pid ${PIDS[-1]}"

RUST_CFG="$TMPDIR/validator_rust.toml"
{
    echo "rpc = \"$ANVIL_RPC_URL\""
    echo "signer = \"${PRIVATE_KEYS[1]}\""
    echo "database = \"sqlite://$TMPDIR/validator_rust.sqlite?mode=rwc\""
    echo
    echo "[validator]"
    echo "consensus = \"$CONSENSUS_ADDR\""
    for address in "${PARTICIPANTS[@]}"; do
        echo
        echo "[[validator.participants]]"
        echo "address = \"$address\""
    done
    echo
    echo "[observability]"
    echo 'log_filter = "info,safenet_core=debug,validator=debug"'
    echo
    echo "[index]"
    echo "block_time = 1000"
    echo "start_block = 0"
} > "$RUST_CFG"

echo "==> Starting Rust validator (${PARTICIPANTS[1]})..."
"$RUST_BIN" --config-file "$RUST_CFG" > "$TMPDIR/validator_rust.log" 2>&1 &
PIDS+=("$!")
echo "    pid ${PIDS[-1]}"

# Let both watchers initialize before emitting the genesis event.
sleep 2
for pid in "${PIDS[@]:1}"; do
    if ! kill -0 "$pid" 2>/dev/null; then
        echo "A validator exited during startup." >&2
        dump_validator_logs
        exit 1
    fi
done

echo "==> Triggering genesis KeyGen..."
env PARTICIPANTS="$PARTICIPANTS_CSV" \
    COORDINATOR_ADDRESS="$COORDINATOR_ADDR" \
    npm run --prefix "$REPO_ROOT" --workspace contracts cmd:genesis -- \
    --rpc-url "$ANVIL_RPC_URL" \
    --unlocked \
    --sender "$SENDER" \
    --broadcast 2>&1 | tee "$TMPDIR/genesis.log"

EXPECTED="${#PARTICIPANTS[@]}"
DEADLINE=$((SECONDS + TIMEOUT))
TRUE_WORD=0000000000000000000000000000000000000000000000000000000000000001

echo "==> Waiting for $EXPECTED KeyGenConfirmed events (timeout: ${TIMEOUT}s)..."
COUNT=0
COMPLETED=0
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    LOGS=$(cast logs --json \
        --rpc-url "$ANVIL_RPC_URL" \
        --from-block 0 \
        --to-block latest \
        --address "$COORDINATOR_ADDR" \
        'KeyGenConfirmed(bytes32,address,bool)')
    COUNT=$(jq 'length' <<< "$LOGS")
    COMPLETED=$(jq --arg true_word "$TRUE_WORD" '[.[] | select(.data | endswith($true_word))] | length' <<< "$LOGS")
    echo "    confirmations: $COUNT / $EXPECTED (ceremony completed: $([ "$COMPLETED" -gt 0 ] && echo yes || echo no))"
    if [ "$COUNT" -ge "$EXPECTED" ] && [ "$COMPLETED" -gt 0 ]; then
        echo
        echo "SUCCESS: TypeScript and Rust validators confirmed the same genesis group key."
        exit 0
    fi

    for pid in "${PIDS[@]:1}"; do
        if ! kill -0 "$pid" 2>/dev/null; then
            echo "A validator exited before genesis key generation completed." >&2
            dump_validator_logs
            exit 1
        fi
    done
    sleep 2
done

echo
echo "TIMEOUT: received $COUNT / $EXPECTED confirmations; ceremony completed: $([ "$COMPLETED" -gt 0 ] && echo yes || echo no)." >&2
dump_validator_logs
exit 1
