#!/bin/bash
# Cross-implementation genesis integration test.
#
# Starts Anvil, deploys contracts, and runs one validator of each
# implementation (TypeScript, Rust, Go) as genesis participants. Triggers the
# genesis KeyGen ceremony and verifies that all three participants confirm
# on-chain within the timeout.
#
# Requirements: anvil, forge, jq, cast (Foundry), cargo, go, node/npm.
set -euo pipefail

ANVIL_RPC_URL="http://127.0.0.1:8545"
CHAIN_ID=31337

# One participant per validator implementation (Anvil accounts 1-3).
#   Index 0 → TypeScript validator
#   Index 1 → Rust validator
#   Index 2 → Go validator
PARTICIPANTS=(
    0x70997970C51812dc3A010C7d01b50e0d17dc79C8
    0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC
    0x90F79bf6EB2c4f870365E785982E1f101E93b906
)
PRIVATE_KEYS=(
    0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d
    0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a
    0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6
)

# Anvil default deployer account (index 0).
SENDER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

TMPDIR="$(mktemp -d)"
ANVIL_PID=""
VALIDATOR_PIDS=()

cleanup() {
    [ -n "$ANVIL_PID" ] && kill "$ANVIL_PID" 2>/dev/null || true
    for pid in "${VALIDATOR_PIDS[@]:-}"; do
        kill "$pid" 2>/dev/null || true
    done
    rm -rf "$TMPDIR"
}
trap cleanup EXIT

echo "==> Using temporary director $TMPDIR"

# ---------------------------------------------------------------------------
# 1. Start Anvil with a 1-second block time.
# ---------------------------------------------------------------------------
echo "==> Starting Anvil..."
anvil --block-time 1 > "$TMPDIR/anvil.log" 2>&1 &
ANVIL_PID=$!
sleep 2

# ---------------------------------------------------------------------------
# 2. Deploy contracts (FROSTCoordinator + Consensus).
# ---------------------------------------------------------------------------
echo "==> Deploying contracts..."
PARTICIPANTS_CSV=$(IFS=, ; echo "${PARTICIPANTS[*]}")
env PARTICIPANTS="$PARTICIPANTS_CSV" \
    npm run --prefix "$REPO_ROOT" -w contracts cmd:deploy -- \
    --rpc-url "$ANVIL_RPC_URL" \
    --unlocked \
    --sender "$SENDER" \
    --broadcast 2>&1 | tee "$TMPDIR/deploy.log"

DEPLOY_JSON="$REPO_ROOT/contracts/build/broadcast/Deploy.s.sol/31337/run-latest.json"
COORDINATOR_ADDR=$(jq -r '.returns.coordinator.value' < "$DEPLOY_JSON")
CONSENSUS_ADDR=$(jq -r '.returns.consensus.value' < "$DEPLOY_JSON")
echo "    coordinator: $COORDINATOR_ADDR"
echo "    consensus:   $CONSENSUS_ADDR"

# ---------------------------------------------------------------------------
# 3. Build Rust and Go validator binaries.
# ---------------------------------------------------------------------------
echo "==> Building validator-rust..."
cargo build -p validator-rust --release -q
RUST_BIN="$REPO_ROOT/target/release/validator-rust"

echo "==> Building validator-go..."
(cd "$REPO_ROOT/validator-go" && go build -o "$TMPDIR/validator-go" .)

# ---------------------------------------------------------------------------
# 4. Build shared config fragments.
# ---------------------------------------------------------------------------

# TOML [[participants]] block (used by Rust and Go configs).
TOML_PARTICIPANTS_BLOCK=""
for addr in "${PARTICIPANTS[@]}"; do
    TOML_PARTICIPANTS_BLOCK+="[[participants]]
address = \"$addr\"

"
done

# JSON participants array (used by the TypeScript validator env var).
TS_PARTICIPANTS_JSON="["
for i in "${!PARTICIPANTS[@]}"; do
    [ "$i" -gt 0 ] && TS_PARTICIPANTS_JSON+=","
    TS_PARTICIPANTS_JSON+="{\"address\":\"${PARTICIPANTS[$i]}\",\"activeFrom\":0}"
done
TS_PARTICIPANTS_JSON+="]"

# ---------------------------------------------------------------------------
# 5. Start the TypeScript validator (participant 0).
# ---------------------------------------------------------------------------
echo "==> Starting TypeScript validator (${PARTICIPANTS[0]})..."
env \
    RPC_URL="$ANVIL_RPC_URL" \
    PRIVATE_KEY="${PRIVATE_KEYS[0]}" \
    CONSENSUS_ADDRESS="$CONSENSUS_ADDR" \
    COORDINATOR_ADDRESS="$COORDINATOR_ADDR" \
    CHAIN_ID="$CHAIN_ID" \
    PARTICIPANTS="$TS_PARTICIPANTS_JSON" \
    GENESIS_SALT="0x0000000000000000000000000000000000000000000000000000000000000000" \
    npm run --prefix "$REPO_ROOT" -w validator dev \
    > "$TMPDIR/validator_ts.log" 2>&1 &
VALIDATOR_PIDS+=($!)
echo "    pid ${VALIDATOR_PIDS[-1]}"

# ---------------------------------------------------------------------------
# 6. Start the Rust validator (participant 1).
# ---------------------------------------------------------------------------
echo "==> Starting Rust validator (${PARTICIPANTS[1]})..."
RUST_CFG="$TMPDIR/config_rust.toml"
cat > "$RUST_CFG" <<EOF
rpc_url           = "$ANVIL_RPC_URL"
private_key       = "${PRIVATE_KEYS[1]}"
consensus_address = "$CONSENSUS_ADDR"
storage_file      = "$TMPDIR/state_rust.sqlite"

$TOML_PARTICIPANTS_BLOCK
EOF
"$RUST_BIN" --config-file "$RUST_CFG" --log-level info,validator_rust=debug \
    > "$TMPDIR/validator_rust.log" 2>&1 &
VALIDATOR_PIDS+=($!)
echo "    pid ${VALIDATOR_PIDS[-1]}"

# ---------------------------------------------------------------------------
# 7. Start the Go validator (participant 2).
# ---------------------------------------------------------------------------
echo "==> Starting Go validator (${PARTICIPANTS[2]})..."
GO_CFG="$TMPDIR/config_go.toml"
cat > "$GO_CFG" <<EOF
rpc_url           = "$ANVIL_RPC_URL"
private_key       = "${PRIVATE_KEYS[2]}"
consensus_address = "$CONSENSUS_ADDR"
storage_file      = "$TMPDIR/state_go.sqlite"

$TOML_PARTICIPANTS_BLOCK
EOF
"$TMPDIR/validator-go" -config "$GO_CFG" > "$TMPDIR/validator_go.log" 2>&1 &
VALIDATOR_PIDS+=($!)
echo "    pid ${VALIDATOR_PIDS[-1]}"

# Give all three validators time to start and complete the history replay.
sleep 3

# ---------------------------------------------------------------------------
# 8. Trigger the genesis KeyGen ceremony.
# ---------------------------------------------------------------------------
echo "==> Triggering genesis KeyGen..."
env PARTICIPANTS="$PARTICIPANTS_CSV" \
    COORDINATOR_ADDRESS="$COORDINATOR_ADDR" \
    npm run --prefix "$REPO_ROOT" -w contracts cmd:genesis -- \
    --rpc-url "$ANVIL_RPC_URL" \
    --unlocked \
    --sender "$SENDER" \
    --broadcast 2>&1 | tee "$TMPDIR/genesis.log"

# ---------------------------------------------------------------------------
# 9. Poll for KeyGenConfirmed events until all 3 participants confirm.
# ---------------------------------------------------------------------------
# topic0 for: event KeyGenConfirmed(bytes32 indexed gid, address participant, bool confirmed)
KEYGEN_CONFIRMED_TOPIC=0x2553b3b5476eaf8b6ccc0c1656cd21552f8e85959654fd47d733f6e94bc65202
EXPECTED="${#PARTICIPANTS[@]}"
TIMEOUT=120
DEADLINE=$((SECONDS + TIMEOUT))

echo "==> Waiting for $EXPECTED KeyGenConfirmed events (timeout: ${TIMEOUT}s)..."
COUNT=0
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    COUNT=$(cast logs \
        --rpc-url "$ANVIL_RPC_URL" \
        --from-block 0 \
        --to-block latest \
        "$KEYGEN_CONFIRMED_TOPIC" \
        --address "$COORDINATOR_ADDR" 2>/dev/null \
        | grep -c "^- address:" || true)
    echo "    KeyGenConfirmed: $COUNT / $EXPECTED"
    if [ "$COUNT" -ge "$EXPECTED" ]; then
        echo ""
        echo "SUCCESS: all $EXPECTED participants (TypeScript + Rust + Go) confirmed genesis keygen."
        exit 0
    fi
    sleep 3
done

# ---------------------------------------------------------------------------
# Failure: dump all validator logs for debugging.
# ---------------------------------------------------------------------------
echo ""
echo "TIMEOUT: only $COUNT / $EXPECTED KeyGenConfirmed events received within ${TIMEOUT}s."
echo ""
for name in ts rust go; do
    echo "=== validator_${name}.log ==="
    cat "$TMPDIR/validator_${name}.log" 2>/dev/null || echo "(empty)"
    echo ""
done
exit 1
