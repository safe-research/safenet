#!/bin/bash
# Cross-implementation genesis and first-epoch rollover integration test.
#
# Starts Anvil, deploys the contracts, and runs the TypeScript and Rust
# validators as members of the genesis and epoch-1 groups. It proposes one
# transaction for attestation by each group. The test succeeds once epoch 1 is
# attested by genesis, staged, rolled over, and attests the second transaction.
#
# Requirements: anvil, forge, cast, jq, cargo, node, and npm.
set -euo pipefail

ANVIL_RPC_URL="${ANVIL_RPC_URL:-http://127.0.0.1:8545}"
CHAIN_ID=31337
BLOCKS_PER_EPOCH=60
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

for command in anvil cast forge jq cargo node npm; do
    command -v "$command" >/dev/null || {
        echo "Missing required command: $command" >&2
        exit 1
    }
done

dump_validator_logs() {
    echo
    for name in ts rust; do
        echo "=== validator_${name}.log ==="
        cat "$TMPDIR/validator_${name}.log" 2>/dev/null || echo "(empty)"
        echo
    done
}

EXIT_MESSAGE="FAILURE: interrupted"
cleanup() {
    for pid in "${PIDS[@]:-}"; do
        kill "$pid" 2>/dev/null || true
    done
    dump_validator_logs
    echo "$EXIT_MESSAGE"
    rm -rf "$TMPDIR"
}
trap cleanup EXIT

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
    BLOCKS_PER_EPOCH="$BLOCKS_PER_EPOCH" \
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
    echo "blocks_per_epoch = $BLOCKS_PER_EPOCH"
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
        EXIT_MESSAGE="FAILURE: A validator exited during startup."
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

DEADLINE=$((SECONDS + TIMEOUT))
EPOCH_ONE_WORD=0x0000000000000000000000000000000000000000000000000000000000000001
TRUE_WORD=0000000000000000000000000000000000000000000000000000000000000001

echo "==> Waiting for transaction attestations and the epoch 1 rollover (timeout: ${TIMEOUT}s)..."
GENESIS_CONFIRMATIONS=0
EPOCH_ONE_GROUP=""
EPOCH_ONE_CONFIRMATIONS=0
TRANSACTION_PROPOSED=0
TRANSACTION_HASH=""
TRANSACTION_ATTESTED=0
STAGED=0
ROLLED_OVER=0
EPOCH_ONE_TRANSACTION_PROPOSED=0
EPOCH_ONE_TRANSACTION_HASH=""
EPOCH_ONE_TRANSACTION_ATTESTED=0
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    CONFIRMATIONS=$(cast logs --json \
        --rpc-url "$ANVIL_RPC_URL" \
        --from-block 0 \
        --to-block latest \
        --address "$COORDINATOR_ADDR" \
        'KeyGenConfirmed(bytes32,address,bool)')
    GENESIS_GROUP=$(jq -r '.[0].topics[1] // empty' <<< "$CONFIRMATIONS")
    if [ -n "$GENESIS_GROUP" ]; then
        GENESIS_CONFIRMATIONS=$(jq --arg gid "$GENESIS_GROUP" '[.[] | select(.topics[1] == $gid)] | length' <<< "$CONFIRMATIONS")
        EPOCH_ONE_CONFIRMATIONS=$(jq --arg gid "$GENESIS_GROUP" '[.[] | select(.topics[1] != $gid)] | length' <<< "$CONFIRMATIONS")
    fi

    GENESIS_COMPLETED=$(jq --arg true_word "$TRUE_WORD" '[.[] | select(.data | endswith($true_word))] | length' <<< "$CONFIRMATIONS")
    if [ "$TRANSACTION_PROPOSED" -eq 0 ] && [ "$GENESIS_COMPLETED" -gt 0 ]; then
        echo "==> Proposing a transaction for genesis attestation..."
        env \
            CONSENSUS_ADDRESS="$CONSENSUS_ADDR" \
            TX_CHAIN_ID="$CHAIN_ID" \
            TX_SAFE="$SENDER" \
            TX_TO="$SENDER" \
            TX_NONCE=0 \
            npm run --prefix "$REPO_ROOT" --workspace contracts cmd:propose -- \
            --rpc-url "$ANVIL_RPC_URL" \
            --unlocked \
            --sender "$SENDER" \
            --broadcast 2>&1 | tee "$TMPDIR/propose.log"

        PROPOSALS=$(cast logs --json \
            --rpc-url "$ANVIL_RPC_URL" \
            --from-block 0 \
            --to-block latest \
            --address "$CONSENSUS_ADDR" \
            'TransactionProposed(bytes32,uint256,address,uint64,(uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256))')
        TRANSACTION_HASH=$(jq -er '.[-1].topics[1]' <<< "$PROPOSALS")
        TRANSACTION_PROPOSED=1
    fi

    if [ "$TRANSACTION_PROPOSED" -gt 0 ]; then
        ATTESTATIONS=$(cast logs --json \
            --rpc-url "$ANVIL_RPC_URL" \
            --from-block 0 \
            --to-block latest \
            --address "$CONSENSUS_ADDR" \
            'TransactionAttested(bytes32,uint256,address,uint64,bytes32,((uint256,uint256),uint256))')
        TRANSACTION_ATTESTED=$(jq --arg hash "$TRANSACTION_HASH" '[.[] | select(.topics[1] == $hash)] | length' <<< "$ATTESTATIONS")
    fi

    STAGED_LOGS=$(cast logs --json \
        --rpc-url "$ANVIL_RPC_URL" \
        --from-block 0 \
        --to-block latest \
        --address "$CONSENSUS_ADDR" \
        'EpochStaged(uint64,uint64,uint64,bytes32,(uint256,uint256),bytes32,((uint256,uint256),uint256))')
    STAGED=$(jq --arg epoch "$EPOCH_ONE_WORD" '[.[] | select(.topics[2] == $epoch)] | length' <<< "$STAGED_LOGS")
    if [ "$STAGED" -gt 0 ]; then
        # The group ID is the second non-indexed word in EpochStaged, after
        # rolloverBlock. Pin the confirmation count to this group because an
        # epoch-2 key generation may begin after block 60.
        EPOCH_ONE_GROUP=$(jq -er --arg epoch "$EPOCH_ONE_WORD" \
            '[.[] | select(.topics[2] == $epoch)][-1].data | "0x" + .[66:130]' \
            <<< "$STAGED_LOGS")
        EPOCH_ONE_CONFIRMATIONS=$(jq --arg gid "$EPOCH_ONE_GROUP" \
            '[.[] | select(.topics[1] == $gid)] | length' <<< "$CONFIRMATIONS")
    fi

    if [ "$EPOCH_ONE_TRANSACTION_PROPOSED" -eq 0 ] && [ "$TRANSACTION_ATTESTED" -gt 0 ] && [ "$STAGED" -gt 0 ]; then
        CURRENT_BLOCK=$(cast block-number --rpc-url "$ANVIL_RPC_URL")
        if [ "$CURRENT_BLOCK" -ge "$BLOCKS_PER_EPOCH" ]; then
            echo "==> Triggering epoch 1 rollover and proposing another transaction for attestation..."
            # Consensus processes a due rollover lazily at the start of state-
            # changing calls. This proposal first rolls over to epoch 1, then
            # creates a signing request for the now-active epoch-1 group.
            env \
                CONSENSUS_ADDRESS="$CONSENSUS_ADDR" \
                TX_CHAIN_ID="$CHAIN_ID" \
                TX_SAFE="$SENDER" \
                TX_TO="$SENDER" \
                TX_NONCE=1 \
                npm run --prefix "$REPO_ROOT" --workspace contracts cmd:propose -- \
                --rpc-url "$ANVIL_RPC_URL" \
                --unlocked \
                --sender "$SENDER" \
                --broadcast 2>&1 | tee "$TMPDIR/propose_epoch_one.log"

            PROPOSALS=$(cast logs --json \
                --rpc-url "$ANVIL_RPC_URL" \
                --from-block 0 \
                --to-block latest \
                --address "$CONSENSUS_ADDR" \
                'TransactionProposed(bytes32,uint256,address,uint64,(uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256))')
            EPOCH_ONE_TRANSACTION_HASH=$(jq -er --arg epoch "$EPOCH_ONE_WORD" \
                '[.[] | select(.data | startswith($epoch))][-1].topics[1]' <<< "$PROPOSALS")
            EPOCH_ONE_TRANSACTION_PROPOSED=1
        fi
    fi

    ROLLOVERS=$(cast logs --json \
        --rpc-url "$ANVIL_RPC_URL" \
        --from-block 0 \
        --to-block latest \
        --address "$CONSENSUS_ADDR" \
        'EpochRolledOver(uint64)')
    ROLLED_OVER=$(jq --arg epoch "$EPOCH_ONE_WORD" '[.[] | select(.topics[1] == $epoch)] | length' <<< "$ROLLOVERS")

    if [ "$EPOCH_ONE_TRANSACTION_PROPOSED" -gt 0 ]; then
        ATTESTATIONS=$(cast logs --json \
            --rpc-url "$ANVIL_RPC_URL" \
            --from-block 0 \
            --to-block latest \
            --address "$CONSENSUS_ADDR" \
            'TransactionAttested(bytes32,uint256,address,uint64,bytes32,((uint256,uint256),uint256))')
        EPOCH_ONE_TRANSACTION_ATTESTED=$(jq --arg hash "$EPOCH_ONE_TRANSACTION_HASH" --arg epoch "$EPOCH_ONE_WORD" \
            '[.[] | select((.topics[1] == $hash) and (.data | startswith($epoch)))] | length' <<< "$ATTESTATIONS")
    fi

    echo "    genesis confirmations: $GENESIS_CONFIRMATIONS; genesis transaction: $([ "$TRANSACTION_PROPOSED" -gt 0 ] && echo proposed || echo pending)/$([ "$TRANSACTION_ATTESTED" -gt 0 ] && echo attested || echo pending); epoch 1 confirmations: $EPOCH_ONE_CONFIRMATIONS; staged: $([ "$STAGED" -gt 0 ] && echo yes || echo no); rolled over: $([ "$ROLLED_OVER" -gt 0 ] && echo yes || echo no); epoch 1 transaction: $([ "$EPOCH_ONE_TRANSACTION_PROPOSED" -gt 0 ] && echo proposed || echo pending)/$([ "$EPOCH_ONE_TRANSACTION_ATTESTED" -gt 0 ] && echo attested || echo pending)"
    if [ "$TRANSACTION_ATTESTED" -gt 0 ] && [ "$STAGED" -gt 0 ] && [ "$ROLLED_OVER" -gt 0 ] && [ "$EPOCH_ONE_TRANSACTION_ATTESTED" -gt 0 ]; then
        EXIT_MESSAGE="SUCCESS: genesis and epoch 1 each attested a transaction, and epoch 1 was generated, staged, and rolled over."
        exit 0
    fi

    for pid in "${PIDS[@]:1}"; do
        if ! kill -0 "$pid" 2>/dev/null; then
            EXIT_MESSAGE="FAILURE: A validator exited before both transactions were attested and epoch 1 rolled over." >&2
            exit 1
        fi
    done
    sleep 2
done

EXIT_MESSAGE="TIMEOUT: genesis confirmations: $GENESIS_CONFIRMATIONS; genesis transaction proposed: $TRANSACTION_PROPOSED; genesis transaction attested: $TRANSACTION_ATTESTED; epoch 1 confirmations: $EPOCH_ONE_CONFIRMATIONS; staged: $STAGED; rolled over: $ROLLED_OVER; epoch 1 transaction proposed: $EPOCH_ONE_TRANSACTION_PROPOSED; epoch 1 transaction attested: $EPOCH_ONE_TRANSACTION_ATTESTED." >&2
exit 1
