#!/bin/bash
# Validator rotation integration test for the Rust validator.
#
# Starts Anvil, deploys the contracts, and runs two Rust validator instances
# as members of the genesis group. Three more validators are rotated in and
# started with empty databases at different points of the epoch rollover,
# covering each way a validator initializes its state from onchain data:
#
# - C is started once genesis completed, but no epoch is staged (an offline
#   participant makes the epoch 1 key generation fail). It must skip genesis
#   and join the epoch 2 key generation.
# - D is started once the epoch clock reached epoch 1, while the onchain
#   rollover lags behind on genesis and the epoch 2 key generation it is part
#   of is already underway. It must skip epoch 2 and join epoch 3 instead.
# - E is started once epoch 2 is staged. It must join epoch 3.
#
# The test succeeds once each of them confirmed the group of the epoch it
# joins without starting any earlier key generation, and epoch 3 is rolled
# over and attests an oracle-backed transaction.
#
# Requirements: anvil, forge, cast, jq, and cargo.
set -euo pipefail

ANVIL_RPC_URL="${ANVIL_RPC_URL:-http://127.0.0.1:8545}"
CHAIN_ID=31337
BLOCK_TIME=1
BLOCKS_PER_EPOCH=40
KEY_GEN_TIMEOUT=10
TIMEOUT="${TIMEOUT:-300}"

# Anvil accounts 1 and 2 form the genesis group as validators A and B.
GENESIS_NAMES=(a b)
GENESIS_PARTICIPANTS=(
    0x70997970C51812dc3A010C7d01b50e0d17dc79C8
    0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC
)
GENESIS_PRIVATE_KEYS=(
    0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d
    0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a
)

# Anvil accounts 3, 4 and 5 are rotated in as validators C, D and E, joining
# the participant set from the given epochs.
LATE_NAMES=(c d e)
LATE_PARTICIPANTS=(
    0x90F79bf6EB2c4f870365E785982E1f101E93b906
    0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65
    0x9965507D1a55bcC2695C58ba16FB37d819B0A4dc
)
LATE_PRIVATE_KEYS=(
    0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6
    0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a
    0x8b3a350cf5c34c9194ca85829a2df0ec3153be0318b5e2d3348e872092edffba
)
LATE_ACTIVE_FROM=(2 2 3)
# The epoch each late validator is expected to first take part in.
LATE_JOINING_EPOCHS=(2 3 3)

# Anvil account 6 is only part of the epoch 1 participant set, but is never
# started, making the epoch 1 key generation fail.
OFFLINE_PARTICIPANT=0x976EA74026E726554dB657fA54763abd0C3a0aa9

# Anvil default deployer account (index 0).
SENDER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/shared_test_scripts.sh"

TMPDIR="$(mktemp -d)"
PIDS=()

require_commands anvil cast forge jq cargo
install_cleanup_trap

echo "==> Using temporary directory $TMPDIR"

build_services_and_contracts

echo "==> Starting Anvil..."
start_anvil "$BLOCK_TIME" 8545 "$REPO_ROOT/anvil_logs.txt" "$ANVIL_RPC_URL"

GENESIS_PARTICIPANTS_CSV=$(IFS=,; echo "${GENESIS_PARTICIPANTS[*]}")
deploy_validator_contracts "$ANVIL_RPC_URL" "$SENDER" "$GENESIS_PARTICIPANTS_CSV" "$CHAIN_ID"

print_participant() {
    local address=$1 active_from=$2 active_before=${3:-}
    echo
    echo "[[validator.participants]]"
    echo "address = \"$address\""
    echo "active_from = $active_from"
    if [ -n "$active_before" ]; then
        echo "active_before = $active_before"
    fi
}

validator_config() {
    # The key generation timeout is shortened so that the failing epoch 1 key
    # generation is abandoned quickly. It is inserted right after the
    # `[validator]` header, as the base config closes that table.
    print_validator_config_base \
        "$ANVIL_RPC_URL" "$1" "$2" "$CONSENSUS_ADDR" "$ORACLE_ADDR" \
        "$BLOCKS_PER_EPOCH" "$(($BLOCK_TIME * 1000))" GENESIS_PARTICIPANTS \
        | awk -v line="key_gen_timeout = $KEY_GEN_TIMEOUT" '{ print } /^\[validator\]$/ { print line }'
    # TOML allows appending to the `validator.participants` array of tables
    # after other tables were defined.
    print_participant "$OFFLINE_PARTICIPANT" 1 2
    local i
    for i in "${!LATE_PARTICIPANTS[@]}"; do
        print_participant "${LATE_PARTICIPANTS[$i]}" "${LATE_ACTIVE_FROM[$i]}"
    done
}

start_validator() {
    local name=$1 address=$2 private_key=$3
    local config="$TMPDIR/validator_$name.toml"
    validator_config "$private_key" "$TMPDIR/validator_$name.sqlite" > "$config"
    echo "==> Starting validator ${name^^} ($address)..."
    run_rust_process validator "$config" "$REPO_ROOT/validator_${name}_logs.txt"
    echo "    pid $LAST_PID"
}

start_late_validator() {
    local i=$1
    start_validator "${LATE_NAMES[$i]}" "${LATE_PARTICIPANTS[$i]}" "${LATE_PRIVATE_KEYS[$i]}"
}

current_block() {
    cast block-number --rpc-url "$ANVIL_RPC_URL"
}

# Prints the group ID of the staged `epoch`, or nothing if it is not staged.
staged_group() {
    local epoch_word
    epoch_word=$(cast to-uint256 "$1")
    # The group ID is the second non-indexed word in EpochStaged, after
    # rolloverBlock.
    fetch_logs "$ANVIL_RPC_URL" "$CONSENSUS_ADDR" \
        'EpochStaged(uint64,uint64,uint64,bytes32,(uint256,uint256),bytes32,((uint256,uint256),uint256))' \
        | jq -r --arg epoch "$epoch_word" \
            '[.[] | select(.topics[2] == $epoch)][-1].data // empty | "0x" + .[66:130]'
}

is_staged() {
    [ -n "$(staged_group "$1")" ]
}

is_block_reached() {
    [ "$(current_block)" -ge "$1" ]
}

# Whether any group completed its key generation, which can only be genesis
# while the epoch 1 key generation fails.
is_genesis_completed() {
    local true_word
    true_word=$(cast to-uint256 1)
    fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" 'KeyGenConfirmed(bytes32,address,bool)' \
        | jq -e --arg true_word "${true_word#0x}" \
            'any(.[]; .data | endswith($true_word))' > /dev/null
}

is_transaction_attested() {
    local epoch_word
    epoch_word=$(cast to-uint256 "$1")
    fetch_logs "$ANVIL_RPC_URL" "$CONSENSUS_ADDR" \
        'TransactionAttested(bytes32,bytes32,address,uint64,bytes32,bytes32,((uint256,uint256),uint256))' \
        | jq -e --arg hash "$2" --arg epoch "$epoch_word" \
            'any(.[]; (.topics[1] == $hash) and (.data | startswith($epoch)))' > /dev/null
}

# Waits until the given command succeeds, failing on timeout.
wait_for() {
    local description=$1
    shift
    echo "==> Waiting for $description (block $(current_block))..."
    until "$@"; do
        if [ "$SECONDS" -ge "$DEADLINE" ]; then
            EXIT_MESSAGE="TIMEOUT: waiting for $description."
            exit 1
        fi
        assert_processes_alive "FAILURE: A validator exited while waiting for $description." "${PIDS[@]:1}"
        sleep "$BLOCK_TIME"
    done
}

# Fails unless the late validator `i` is started before `block`, and while
# `epoch` is not yet staged, so that it initializes in the intended state.
require_start_window() {
    local i=$1 block=$2 epoch=$3 current
    current=$(current_block)
    if [ "$current" -ge "$block" ] || is_staged "$epoch"; then
        EXIT_MESSAGE="FAILURE: validator ${LATE_NAMES[$i]^^} could not be started before block $block with epoch $epoch unstaged (block $current)."
        exit 1
    fi
}

for i in "${!GENESIS_PARTICIPANTS[@]}"; do
    start_validator "${GENESIS_NAMES[$i]}" "${GENESIS_PARTICIPANTS[$i]}" "${GENESIS_PRIVATE_KEYS[$i]}"
done

# Let both watchers initialize before emitting the genesis event.
sleep 0.5
assert_processes_alive "FAILURE: A validator exited during startup." "${PIDS[@]:1}"

trigger_genesis_keygen "$ANVIL_RPC_URL" "$SENDER" "$GENESIS_PARTICIPANTS_CSV" "$COORDINATOR_ADDR"

DEADLINE=$((SECONDS + TIMEOUT))

wait_for "genesis to complete" is_genesis_completed
require_start_window 0 "$BLOCKS_PER_EPOCH" 1
start_late_validator 0

wait_for "the epoch clock to reach epoch 1" is_block_reached "$BLOCKS_PER_EPOCH"
require_start_window 1 $((2 * BLOCKS_PER_EPOCH)) 2
if is_staged 1; then
    EXIT_MESSAGE="FAILURE: epoch 1 was staged, but its key generation was expected to fail."
    exit 1
fi
start_late_validator 1

wait_for "epoch 2 to be staged" is_staged 2
require_start_window 2 $((2 * BLOCKS_PER_EPOCH)) 3
start_late_validator 2

wait_for "epoch 3 to be staged" is_staged 3
wait_for "the epoch clock to reach epoch 3" is_block_reached $((3 * BLOCKS_PER_EPOCH))

echo "==> Triggering epoch 3 rollover and proposing an oracle-backed transaction for attestation..."
# Consensus processes a due rollover lazily at the start of state-changing
# calls. This proposal first rolls over to epoch 3, then creates a signing
# request for the now-active epoch-3 group.
env \
    CONSENSUS_ADDRESS="$CONSENSUS_ADDR" \
    ORACLE_ADDRESS="$ORACLE_ADDR" \
    TX_CHAIN_ID="$CHAIN_ID" \
    TX_SAFE="$SENDER" \
    TX_TO="$SENDER" \
    TX_NONCE=0 \
    forge script --root "$REPO_ROOT/contracts" ProposeTransactionScript \
    --rpc-url "$ANVIL_RPC_URL" \
    --unlocked \
    --sender "$SENDER" \
    --broadcast

TRANSACTION_HASH=$(fetch_logs "$ANVIL_RPC_URL" "$CONSENSUS_ADDR" \
    'TransactionProposed(bytes32,bytes32,address,uint64,bytes,(uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256))' \
    | jq -r --arg epoch "$(cast to-uint256 3)" '[.[] | select(.data | startswith($epoch))][-1].topics[1] // empty')
if [ -z "$TRANSACTION_HASH" ]; then
    EXIT_MESSAGE="FAILURE: the transaction was not proposed in epoch 3, as epoch 3 did not become active."
    exit 1
fi

wait_for "the transaction to be attested by epoch 3" is_transaction_attested 3 "$TRANSACTION_HASH"

CONFIRMATIONS=$(fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" 'KeyGenConfirmed(bytes32,address,bool)')
for i in "${!LATE_PARTICIPANTS[@]}"; do
    name=${LATE_NAMES[$i]^^}
    epoch=${LATE_JOINING_EPOCHS[$i]}

    group=$(staged_group "$epoch")
    participant=$(cast abi-encode "f(address)" "${LATE_PARTICIPANTS[$i]}")
    if ! jq -e --arg gid "$group" --arg participant "$participant" \
        'any(.[]; (.topics[1] == $gid) and (.data | startswith($participant)))' \
        <<< "$CONFIRMATIONS" > /dev/null; then
        EXIT_MESSAGE="FAILURE: validator $name did not confirm the epoch $epoch group."
        exit 1
    fi

    # The first key generation a late validator starts or observes must be
    # the one for the epoch it joins, meaning it did not replay any earlier
    # ones.
    first_key_gen=$(grep -m1 -E "(starting|observing) key generation" "$REPO_ROOT/validator_${LATE_NAMES[$i]}_logs.txt" || true)
    if [[ "$first_key_gen" != *"Number { number: $epoch }"* ]]; then
        EXIT_MESSAGE="FAILURE: validator $name did not skip the key generations before epoch $epoch, first started: $first_key_gen"
        exit 1
    fi
    echo "    validator $name skipped earlier key generations and joined epoch $epoch"
done

EXIT_MESSAGE="SUCCESS: validators rotated in after genesis skipped the key generations they missed and joined their epochs, and epoch 3 rolled over and attested an oracle-backed transaction."
exit 0
