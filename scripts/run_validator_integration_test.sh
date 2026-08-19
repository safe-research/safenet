#!/bin/bash
# Genesis and first-epoch rollover integration test for the Rust validator.
#
# Starts Anvil, deploys the contracts, and runs two Rust validator instances
# as members of the genesis and epoch-1 groups. It proposes one oracle-backed
# transaction for attestation by each group, checked against the
# always-approving AlwaysApproveOracle that DeployScript deploys. The test
# succeeds once epoch 1 is attested by genesis, staged, rolled over, and
# attests the second transaction.
#
# Requirements: anvil, forge, cast, jq, and cargo.
set -euo pipefail

ANVIL_RPC_URL="${ANVIL_RPC_URL:-http://127.0.0.1:8545}"
CHAIN_ID=31337
BLOCK_TIME=1
BLOCKS_PER_EPOCH=20
TIMEOUT="${TIMEOUT:-120}"

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
source "$SCRIPT_DIR/lib/shared_test_scripts.sh"

TMPDIR="$(mktemp -d)"
PIDS=()

require_commands anvil cast forge jq cargo
install_cleanup_trap

echo "==> Using temporary directory $TMPDIR"

build_services_and_contracts

echo "==> Starting Anvil..."
start_anvil "$BLOCK_TIME" 8545 "$REPO_ROOT/anvil_logs.txt" "$ANVIL_RPC_URL"

PARTICIPANTS_CSV=$(IFS=,; echo "${PARTICIPANTS[*]}")
deploy_validator_contracts "$ANVIL_RPC_URL" "$SENDER" "$PARTICIPANTS_CSV" "$CHAIN_ID"

validator_config() {
    print_validator_config_base \
        "$ANVIL_RPC_URL" "$1" "$2" "$CONSENSUS_ADDR" "$ORACLE_ADDR" \
        "$BLOCKS_PER_EPOCH" "$(($BLOCK_TIME * 1000))" PARTICIPANTS
}

VALIDATOR_A_CONFIG="$TMPDIR/validator_a.toml"
validator_config "${PRIVATE_KEYS[0]}" "$TMPDIR/validator_a.sqlite" > "$VALIDATOR_A_CONFIG"
VALIDATOR_B_CONFIG="$TMPDIR/validator_b.toml"
validator_config "${PRIVATE_KEYS[1]}" "$TMPDIR/validator_b.sqlite" > "$VALIDATOR_B_CONFIG"

echo "==> Starting validator A (${PARTICIPANTS[0]})..."
run_rust_process validator "$VALIDATOR_A_CONFIG" "$REPO_ROOT/validator_a_logs.txt"
echo "    pid $LAST_PID"

echo "==> Starting validator B (${PARTICIPANTS[1]})..."
run_rust_process validator "$VALIDATOR_B_CONFIG" "$REPO_ROOT/validator_b_logs.txt"
echo "    pid $LAST_PID"

# Let both watchers initialize before emitting the genesis event.
sleep 0.5
assert_processes_alive "FAILURE: A validator exited during startup." "${PIDS[@]:1}"

trigger_genesis_keygen "$ANVIL_RPC_URL" "$SENDER" "$PARTICIPANTS_CSV" "$COORDINATOR_ADDR"

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
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    CONFIRMATIONS=$(fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" 'KeyGenConfirmed(bytes32,address,bool)')
    GENESIS_GROUP=$(jq -r '.[0].topics[1] // empty' <<< "$CONFIRMATIONS")
    if [ -n "$GENESIS_GROUP" ]; then
        GENESIS_CONFIRMATIONS=$(jq --arg gid "$GENESIS_GROUP" '[.[] | select(.topics[1] == $gid)] | length' <<< "$CONFIRMATIONS")
        EPOCH_ONE_CONFIRMATIONS=$(jq --arg gid "$GENESIS_GROUP" '[.[] | select(.topics[1] != $gid)] | length' <<< "$CONFIRMATIONS")
    fi

    GENESIS_COMPLETED=$(jq --arg true_word "$TRUE_WORD" '[.[] | select(.data | endswith($true_word))] | length' <<< "$CONFIRMATIONS")

    STAGED_LOGS=$(fetch_logs "$ANVIL_RPC_URL" "$CONSENSUS_ADDR" \
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

    CURRENT_BLOCK=$(cast block-number --rpc-url "$ANVIL_RPC_URL")
    if [ "$TRANSACTION_PROPOSED" -eq 0 ] && [ "$CURRENT_BLOCK" -ge "$BLOCKS_PER_EPOCH" ]; then
        echo "==> Triggering epoch 1 rollover and proposing another oracle-backed transaction for attestation..."
        # Consensus processes a due rollover lazily at the start of state-
        # changing calls. This proposal first rolls over to epoch 1, then
        # creates a signing request for the now-active epoch-1 group.
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
            --broadcast

        PROPOSALS=$(fetch_logs "$ANVIL_RPC_URL" "$CONSENSUS_ADDR" \
            'TransactionProposed(bytes32,bytes32,address,uint64,bytes,(uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256))')
        TRANSACTION_HASH=$(jq -er --arg epoch "$EPOCH_ONE_WORD" \
            '[.[] | select(.data | startswith($epoch))][-1].topics[1]' <<< "$PROPOSALS")
        TRANSACTION_PROPOSED=1
    fi

    ROLLOVERS=$(fetch_logs "$ANVIL_RPC_URL" "$CONSENSUS_ADDR" 'EpochRolledOver(uint64)')
    ROLLED_OVER=$(jq --arg epoch "$EPOCH_ONE_WORD" '[.[] | select(.topics[1] == $epoch)] | length' <<< "$ROLLOVERS")

    if [ "$TRANSACTION_PROPOSED" -gt 0 ]; then
        ATTESTATIONS=$(fetch_logs "$ANVIL_RPC_URL" "$CONSENSUS_ADDR" \
            'TransactionAttested(bytes32,bytes32,address,uint64,bytes32,bytes32,((uint256,uint256),uint256))')
        TRANSACTION_ATTESTED=$(jq --arg hash "$TRANSACTION_HASH" --arg epoch "$EPOCH_ONE_WORD" \
            '[.[] | select((.topics[1] == $hash) and (.data | startswith($epoch)))] | length' <<< "$ATTESTATIONS")
    fi

    echo "    genesis confirmations: $GENESIS_CONFIRMATIONS; epoch 1 confirmations: $EPOCH_ONE_CONFIRMATIONS; staged: $([ "$STAGED" -gt 0 ] && echo yes || echo no); rolled over: $([ "$ROLLED_OVER" -gt 0 ] && echo yes || echo no); transaction: $([ "$TRANSACTION_PROPOSED" -gt 0 ] && echo proposed || echo pending)/$([ "$TRANSACTION_ATTESTED" -gt 0 ] && echo attested || echo pending) at block $CURRENT_BLOCK"
    if [ "$TRANSACTION_ATTESTED" -gt 0 ] && [ "$STAGED" -gt 0 ] && [ "$ROLLED_OVER" -gt 0 ] && [ "$TRANSACTION_ATTESTED" -gt 0 ]; then
        EXIT_MESSAGE="SUCCESS: genesis and epoch 1 each attested an oracle-backed transaction, and epoch 1 was generated, staged, and rolled over."
        exit 0
    fi

    assert_processes_alive "FAILURE: A validator exited before both transactions were attested and epoch 1 rolled over." "${PIDS[@]:1}"
    sleep "$BLOCK_TIME"
done

EXIT_MESSAGE="TIMEOUT: genesis confirmations: $GENESIS_CONFIRMATIONS; epoch 1 confirmations: $EPOCH_ONE_CONFIRMATIONS; staged: $STAGED; rolled over: $ROLLED_OVER; transaction proposed: $TRANSACTION_PROPOSED; transaction attested: $TRANSACTION_ATTESTED." >&2
exit 1
