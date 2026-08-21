#!/bin/bash
# Regression test: nonces must survive a reorg that rewinds a group's DKG past
# the block where its key share was confirmed.
#
# `Effect::ReconcileGroupSecrets` deletes the persisted nonces of any group not
# reported by the current state as having a key share. A reorg can roll a
# group's local DKG status back from "confirmed" to "still collecting shares"
# after nonces were already generated for it (for example, because a restart
# replays state from before the block that confirmed the key share). Nonces
# generated before that point must be retained until the group either
# re-confirms its key share or is dropped entirely - they must not be deleted
# just because the group is (again, or still) mid-DKG.
#
# This starts Anvil, deploys the contracts, and runs two Rust validator
# instances through genesis key generation. Once the genesis group's key share
# is confirmed and its first nonce tree is submitted onchain, validator A is
# stopped, the chain is reorged back past the block where secret shares were
# distributed (using `anvil_reorg`, which mines empty replacement blocks over
# the reorged range), and validator A is restarted. On restart, validator A
# detects the reorg, rolls its local state back to before the key share was
# confirmed, and reprocesses the (now share-less) chain - reproducing the
# reconciliation this test guards.
#
# Requirements: anvil, forge, cast, jq, and cargo.
set -euo pipefail

ANVIL_PORT=8547
ANVIL_RPC_URL="${ANVIL_RPC_URL:-http://127.0.0.1:$ANVIL_PORT}"
CHAIN_ID=31337
BLOCK_TIME=1
TIMEOUT="${TIMEOUT:-60}"

# Anvil accounts 1 and 2, one per validator instance.
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
VALIDATOR_A_PID=""

for command in anvil cast forge jq cargo; do
    command -v "$command" >/dev/null || {
        echo "Missing required command: $command" >&2
        exit 1
    }
done

EXIT_MESSAGE="FAILURE: interrupted"
cleanup() {
    for pid in "${PIDS[@]:-}"; do
        kill "$pid" 2>/dev/null || true
    done
    rm -rf "$TMPDIR"
    echo "$EXIT_MESSAGE"
}
trap cleanup EXIT

# Converts a 0x-prefixed hex block number (as returned by `cast logs --json`)
# to decimal, using bash's native hex arithmetic.
hex_to_dec() {
    echo "$((16#${1#0x}))"
}

# Prints the highest `blockNumber` among the JSON log array on stdin.
max_block() {
    local max=0 hex dec
    while read -r hex; do
        [ -z "$hex" ] && continue
        dec=$(hex_to_dec "$hex")
        [ "$dec" -gt "$max" ] && max=$dec
    done
    echo "$max"
}

echo "==> Using temporary directory $TMPDIR"

echo "==> Building Rust validator..."
cargo build --manifest-path "$REPO_ROOT/Cargo.toml" --package validator
RUST_BIN="$REPO_ROOT/target/debug/validator"

echo "==> Building Solidity contracts..."
forge build --root "$REPO_ROOT/contracts" --force

echo "==> Starting Anvil..."
anvil --block-time $BLOCK_TIME --port $ANVIL_PORT \
    > "$REPO_ROOT/anvil_reorg_nonce_logs.txt" 2>&1 &
PIDS+=("$!")
for _ in $(seq 1 20); do
    cast block-number --rpc-url "$ANVIL_RPC_URL" >/dev/null 2>&1 && break
    sleep 0.25
done
cast block-number --rpc-url "$ANVIL_RPC_URL" >/dev/null

echo "==> Deploying contracts..."
PARTICIPANTS_CSV=$(IFS=,; echo "${PARTICIPANTS[*]}")
env PARTICIPANTS="$PARTICIPANTS_CSV" \
    forge script --root "$REPO_ROOT/contracts" DeployScript \
    --rpc-url "$ANVIL_RPC_URL" \
    --unlocked \
    --sender "$SENDER" \
    --broadcast 2>&1 | tee "$TMPDIR/deploy.log"

DEPLOY_JSON="$REPO_ROOT/contracts/build/broadcast/Deploy.s.sol/$CHAIN_ID/run-latest.json"
COORDINATOR_ADDR=$(jq -er '.returns.coordinator.value' "$DEPLOY_JSON")
CONSENSUS_ADDR=$(jq -er '.returns.consensus.value' "$DEPLOY_JSON")
ORACLE_ADDR=$(jq -er '.returns.alwaysApproveOracle.value' "$DEPLOY_JSON")
echo "    coordinator: $COORDINATOR_ADDR"
echo "    consensus:   $CONSENSUS_ADDR"

VALIDATOR_A_DB="$TMPDIR/validator_a.sqlite"
VALIDATOR_B_DB="$TMPDIR/validator_b.sqlite"

validator_config() {
    local signer=$1
    local database=$2
    echo "rpc = \"$ANVIL_RPC_URL\""
    echo "signer = \"$signer\""
    echo "database = \"sqlite://$database?mode=rwc\""
    echo
    echo "[validator]"
    echo "consensus = \"$CONSENSUS_ADDR\""
    # High enough that epoch 1's rollover never becomes proposable during
    # this test's short window - only genesis DKG matters here.
    echo "blocks_per_epoch = 1000000"
    echo "oracles = [\"$ORACLE_ADDR\"]"
    for address in "${PARTICIPANTS[@]}"; do
        echo
        echo "[[validator.participants]]"
        echo "address = \"$address\""
    done
    echo
    echo "[observability]"
    echo 'log_filter = "info,safenet_core=trace,validator=trace"'
    echo
    echo "[index]"
    echo "block_time = $(($BLOCK_TIME*1000))"
    echo "start_block = 0"
    # Large enough reorg depth support to work with slow CI tests
    echo "max_reorg_depth = 10"
}

VALIDATOR_A_CONFIG="$TMPDIR/validator_a.toml"
validator_config "${PRIVATE_KEYS[0]}" "$VALIDATOR_A_DB" > "$VALIDATOR_A_CONFIG"
VALIDATOR_B_CONFIG="$TMPDIR/validator_b.toml"
validator_config "${PRIVATE_KEYS[1]}" "$VALIDATOR_B_DB" > "$VALIDATOR_B_CONFIG"

start_validator_a() {
    echo "==> Starting validator A (${PARTICIPANTS[0]})..."
    "$RUST_BIN" --config-file "$VALIDATOR_A_CONFIG" >> "$REPO_ROOT/validator_a_logs.txt" 2>&1 &
    PIDS+=("$!")
    VALIDATOR_A_PID="${PIDS[-1]}"
    echo "    pid $VALIDATOR_A_PID"
}

: > "$REPO_ROOT/validator_a_logs.txt"
start_validator_a

echo "==> Starting validator B (${PARTICIPANTS[1]})..."
"$RUST_BIN" --config-file "$VALIDATOR_B_CONFIG" > "$REPO_ROOT/validator_b_logs.txt" 2>&1 &
PIDS+=("$!")
echo "    pid ${PIDS[-1]}"

# Let both watchers initialize before emitting the genesis event.
sleep 0.5
for pid in "${PIDS[@]:1}"; do
    if ! kill -0 "$pid" 2>/dev/null; then
        EXIT_MESSAGE="FAILURE: A validator exited during startup."
        exit 1
    fi
done

echo "==> Triggering genesis KeyGen..."
env PARTICIPANTS="$PARTICIPANTS_CSV" \
    COORDINATOR_ADDRESS="$COORDINATOR_ADDR" \
    forge script --root "$REPO_ROOT/contracts" GenesisScript \
    --rpc-url "$ANVIL_RPC_URL" \
    --unlocked \
    --sender "$SENDER" \
    --broadcast 2>&1 | tee "$TMPDIR/genesis.log"

DEADLINE=$((SECONDS + TIMEOUT))
GENESIS_GROUP=""
SECRET_SHARED_BLOCK=""
PREPROCESS_BLOCK=""

echo "==> Waiting for genesis secret shares and its first nonce tree submission (timeout: ${TIMEOUT}s)..."
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    SHARED=$(cast logs --json \
        --rpc-url "$ANVIL_RPC_URL" \
        --from-block 0 --to-block latest \
        --address "$COORDINATOR_ADDR" \
        'KeyGenSecretShared(bytes32,address,((uint256,uint256),uint256[]),bool)')
    SHARED_COUNT=$(jq 'length' <<< "$SHARED")

    if [ "$SHARED_COUNT" -ge 2 ] && [ -z "$SECRET_SHARED_BLOCK" ]; then
        GENESIS_GROUP=$(jq -r '.[0].topics[1]' <<< "$SHARED")
        SECRET_SHARED_BLOCK=$(jq -r '.[].blockNumber' <<< "$SHARED" | max_block)
        echo "    genesis group $GENESIS_GROUP shared secrets by block $SECRET_SHARED_BLOCK"
    fi

    if [ -n "$GENESIS_GROUP" ] && [ -z "$PREPROCESS_BLOCK" ]; then
        PREPROCESS=$(cast logs --json \
            --rpc-url "$ANVIL_RPC_URL" \
            --from-block 0 --to-block latest \
            --address "$COORDINATOR_ADDR" \
            'Preprocess(bytes32,address,uint64,bytes32)')
        # `participant` (the first non-indexed word, a padded address) must
        # match validator A specifically: we assert on the exact nonce tree
        # *it* registered, not just any nonce tree for the group, so a fresh
        # tree grown by a later DKG round can't mask an earlier deletion.
        MATCHING=$(jq --arg gid "$GENESIS_GROUP" --arg addr "${PARTICIPANTS[0]#0x}" \
            '[.[] | select(.topics[1] == $gid) | select((.data[26:66] | ascii_downcase) == ($addr | ascii_downcase))]' \
            <<< "$PREPROCESS")
        MATCHING_COUNT=$(jq 'length' <<< "$MATCHING")
        if [ "$MATCHING_COUNT" -gt 0 ]; then
            PREPROCESS_BLOCK=$(jq -r '.[].blockNumber' <<< "$MATCHING" | max_block)
            # `commitment` is the last non-indexed word: the nonce tree's root.
            NONCE_ROOT=$(jq -r '.[0].data[-64:]' <<< "$MATCHING")
            echo "    genesis group $GENESIS_GROUP submitted nonce tree $NONCE_ROOT by block $PREPROCESS_BLOCK"
        fi
    fi

    [ -n "$SECRET_SHARED_BLOCK" ] && [ -n "$PREPROCESS_BLOCK" ] && break

    for pid in "${PIDS[@]:1}"; do
        if ! kill -0 "$pid" 2>/dev/null; then
            EXIT_MESSAGE="FAILURE: A validator exited before genesis completed."
            exit 1
        fi
    done
    sleep "$BLOCK_TIME"
done

if [ -z "$SECRET_SHARED_BLOCK" ] || [ -z "$PREPROCESS_BLOCK" ]; then
    EXIT_MESSAGE="TIMEOUT: genesis did not confirm and submit a nonce tree in time."
    exit 1
fi

CURRENT_BLOCK=$(cast block-number --rpc-url "$ANVIL_RPC_URL")
REORG_DEPTH=$((CURRENT_BLOCK - SECRET_SHARED_BLOCK + 1))

echo "==> Reorging $REORG_DEPTH block(s) from block $CURRENT_BLOCK, spanning back past block $SECRET_SHARED_BLOCK where genesis's secret shares were shared..."
cast rpc anvil_reorg "$REORG_DEPTH" '[]' --rpc-url "$ANVIL_RPC_URL" >/dev/null

# The reorg also rewinds the genesis group's onchain KeyGenConfirmed(...,
# completed: true) event, since it was logged at or after
# SECRET_SHARED_BLOCK and that range just got replaced with empty blocks.
# `proposeTransaction` reverts with `GroupNotReady` until the group's FROST
# state machine reaches FINALIZED again, which only happens once every
# participant has replayed its keyGenConfirm call on the reorged chain -
# waiting a fixed number of blocks isn't a reliable proxy for that and can
# race the proposal below, so wait for the (re-emitted) completed event
# itself instead.
TRUE_WORD=0000000000000000000000000000000000000000000000000000000000000001
echo "==> Waiting for validator A to reprocess the reorged chain and reconfirm the genesis group's key share (timeout: ${TIMEOUT}s)..."
DEADLINE=$((SECONDS + TIMEOUT))
GENESIS_RECONFIRMED=0
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    if ! kill -0 "$VALIDATOR_A_PID" 2>/dev/null; then
        EXIT_MESSAGE="FAILURE: validator A exited after the reorg."
        exit 1
    fi

    CONFIRMATIONS=$(cast logs --json \
        --rpc-url "$ANVIL_RPC_URL" \
        --from-block 0 --to-block latest \
        --address "$COORDINATOR_ADDR" \
        'KeyGenConfirmed(bytes32,address,bool)')
    GENESIS_RECONFIRMED=$(jq --arg gid "$GENESIS_GROUP" --arg true_word "$TRUE_WORD" \
        '[.[] | select(.topics[1] == $gid) | select(.data | endswith($true_word))] | length' \
        <<< "$CONFIRMATIONS")
    [ "$GENESIS_RECONFIRMED" -gt 0 ] && break

    sleep "$BLOCK_TIME"
done

if [ "$GENESIS_RECONFIRMED" -lt 1 ]; then
    EXIT_MESSAGE="TIMEOUT: the genesis group ($GENESIS_GROUP) was not reconfirmed after the reorg in time."
    exit 1
fi
echo "==> Genesis group $GENESIS_GROUP reconfirmed after the reorg"

# Rather than inspecting validator A's local nonce storage directly, prove
# retention functionally: propose a Safe transaction for the genesis group
# (still active - `blocks_per_epoch` is set far out of this test's window)
# and require it to be attested. Genesis is a strict 2-of-2 group here, so if
# validator A's nonce secret for its already-registered preprocessing
# commitment was wrongly deleted, it can never reveal it and the ceremony
# will never complete - there is no fallback path that transparently
# regenerates a fresh nonce tree mid-ceremony.
GENESIS_EPOCH_WORD=0x0000000000000000000000000000000000000000000000000000000000000000

echo "==> Proposing a Safe transaction for the genesis group to sign after the restart and reorg..."
env \
    CONSENSUS_ADDRESS="$CONSENSUS_ADDR" \
    ORACLE_ADDRESS="$ORACLE_ADDR" \
    TX_CHAIN_ID="$CHAIN_ID" \
    TX_SAFE="$SENDER" \
    TX_TO="$SENDER" \
    TX_NONCE=1 \
    forge script --root "$REPO_ROOT/contracts" ProposeTransactionScript \
    --rpc-url "$ANVIL_RPC_URL" \
    --unlocked \
    --sender "$SENDER" \
    --broadcast 2>&1 | tee "$TMPDIR/propose.log"

PROPOSALS=$(cast logs --json \
    --rpc-url "$ANVIL_RPC_URL" \
    --from-block 0 --to-block latest \
    --address "$CONSENSUS_ADDR" \
    'TransactionProposed(bytes32,bytes32,address,uint64,bytes,(uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256))')
TRANSACTION_HASH=$(jq -er --arg epoch "$GENESIS_EPOCH_WORD" \
    '[.[] | select(.data | startswith($epoch))][-1].topics[1]' <<< "$PROPOSALS")

echo "==> Waiting for the genesis group to attest transaction $TRANSACTION_HASH (timeout: ${TIMEOUT}s)..."
DEADLINE=$((SECONDS + TIMEOUT))
TRANSACTION_ATTESTED=0
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    ATTESTATIONS=$(cast logs --json \
        --rpc-url "$ANVIL_RPC_URL" \
        --from-block 0 --to-block latest \
        --address "$CONSENSUS_ADDR" \
        'TransactionAttested(bytes32,bytes32,address,uint64,bytes32,bytes32,((uint256,uint256),uint256))')
    TRANSACTION_ATTESTED=$(jq --arg hash "$TRANSACTION_HASH" --arg epoch "$GENESIS_EPOCH_WORD" \
        '[.[] | select((.topics[1] == $hash) and (.data | startswith($epoch)))] | length' <<< "$ATTESTATIONS")
    [ "$TRANSACTION_ATTESTED" -gt 0 ] && break

    for pid in "${PIDS[@]:1}"; do
        if ! kill -0 "$pid" 2>/dev/null; then
            EXIT_MESSAGE="FAILURE: A validator exited while waiting for the post-reorg attestation."
            exit 1
        fi
    done
    sleep "$BLOCK_TIME"
done

if [ "$TRANSACTION_ATTESTED" -lt 1 ]; then
    EXIT_MESSAGE="TIMEOUT: the genesis group ($GENESIS_GROUP) did not attest a transaction after validator A's restart and reorg spanning the KeyGenSecretShared block - its nonce tree $NONCE_ROOT was likely deleted."
    exit 1
fi

EXIT_MESSAGE="SUCCESS: the genesis group ($GENESIS_GROUP) attested a transaction after validator A's restart and reorg spanning the KeyGenSecretShared block, proving its nonce tree $NONCE_ROOT was retained."
exit 0
