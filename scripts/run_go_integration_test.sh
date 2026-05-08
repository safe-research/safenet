#!/bin/bash
# Integration test for the Go validator (validator-go/).
#
# Starts Anvil, deploys contracts, runs four Go validator processes, triggers
# the genesis KeyGen ceremony, and verifies that all participants confirm by
# checking for KeyGenConfirmed on-chain events.
#
# Requirements: anvil, forge, jq, go, cast (all part of the Foundry toolchain).
set -euo pipefail

ANVIL_RPC_URL="http://127.0.0.1:8545"

# Standard Anvil test accounts used as genesis participants.
PARTICIPANTS=(
    0x70997970C51812dc3A010C7d01b50e0d17dc79C8
    0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC
    0x90F79bf6EB2c4f870365E785982E1f101E93b906
    0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65
)

# Corresponding private keys for the above addresses.
PRIVATE_KEYS=(
    0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d
    0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a
    0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6
    0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a
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
# 3. Build the Go validator binary.
# ---------------------------------------------------------------------------
echo "==> Building validator-go..."
(cd "$REPO_ROOT/validator-go" && go build -o "$TMPDIR/validator-go" .)

# ---------------------------------------------------------------------------
# 4. Write per-participant config files and start validator processes.
# ---------------------------------------------------------------------------
# Build the [[participants]] block shared by every config.
PARTICIPANTS_BLOCK=""
for addr in "${PARTICIPANTS[@]}"; do
    PARTICIPANTS_BLOCK+="[[participants]]
address = \"$addr\"

"
done

echo "==> Starting ${#PARTICIPANTS[@]} validator processes..."
for i in "${!PARTICIPANTS[@]}"; do
    CFG="$TMPDIR/config_$i.toml"
    cat > "$CFG" <<EOF
rpc_url           = "$ANVIL_RPC_URL"
private_key       = "${PRIVATE_KEYS[$i]}"
consensus_address = "$CONSENSUS_ADDR"
storage_file      = "$TMPDIR/state_$i.sqlite"

$PARTICIPANTS_BLOCK
EOF
    "$TMPDIR/validator-go" -config "$CFG" > "$TMPDIR/validator_$i.log" 2>&1 &
    VALIDATOR_PIDS+=($!)
    echo "    validator $i (${PARTICIPANTS[$i]}): pid ${VALIDATOR_PIDS[-1]}"
done

# Give validators time to start and complete the history replay.
sleep 3

# ---------------------------------------------------------------------------
# 5. Trigger the genesis KeyGen ceremony.
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
# 6. Wait for all participants to emit KeyGenConfirmed on-chain.
# ---------------------------------------------------------------------------
# topic0 for event KeyGenConfirmed(bytes32 indexed gid, address participant, bool confirmed)
KEYGEN_CONFIRMED_TOPIC=0x2553b3b5476eaf8b6ccc0c1656cd21552f8e85959654fd47d733f6e94bc65202
EXPECTED="${#PARTICIPANTS[@]}"
TIMEOUT=90
DEADLINE=$((SECONDS + TIMEOUT))

echo "==> Waiting for $EXPECTED KeyGenConfirmed events (timeout: ${TIMEOUT}s)..."
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
        echo "SUCCESS: all $EXPECTED participants confirmed genesis keygen."
        exit 0
    fi
    sleep 3
done

# ---------------------------------------------------------------------------
# Failure: dump validator logs to aid debugging.
# ---------------------------------------------------------------------------
echo ""
echo "TIMEOUT: only $COUNT / $EXPECTED KeyGenConfirmed events received within ${TIMEOUT}s."
echo ""
for i in "${!PARTICIPANTS[@]}"; do
    echo "=== validator $i log ==="
    cat "$TMPDIR/validator_$i.log"
    echo ""
done
exit 1
