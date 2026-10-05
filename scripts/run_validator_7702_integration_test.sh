#!/bin/bash
# EIP-7702 transaction batching integration test for the Rust validator.
#
# Starts Anvil, deploys the contracts and a `Safenet7702Executor`, and runs two
# Rust validator instances with the executor configured, so that each
# delegates its signer account to it and sends every transaction as a
# self-call to `execute`. After genesis key generation, three transactions
# are proposed in a single block, and the test succeeds once the genesis group
# has attested all of them and every transaction either validator sent is
# such a self-call with none of its calls failing. One validator is then
# restarted without the executor, and its signer account must be undelegated
# by the time it has helped attest a fourth transaction.
#
# Prints the gas limit and gas used of each `execute` transaction, to check
# the offchain batch gas estimate against what batches actually use. It does
# not assert how many transactions the validators send: how many actions
# coalesce into a batch depends on timing.
#
# Requirements: anvil, forge, cast, jq, and cargo.
set -euo pipefail

ANVIL_PORT=8551
ANVIL_RPC_URL="${ANVIL_RPC_URL:-http://127.0.0.1:$ANVIL_PORT}"
CHAIN_ID=31337
BLOCK_TIME=1
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

# Anvil default deployer account (index 0). Its key is needed to sign the
# EIP-7702 authorization that lets it batch the proposals itself.
SENDER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
SENDER_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

EXECUTE_SIG='execute((address,uint256,uint256,bytes)[])'
CALL_FAILED_SIG='CallFailed(uint256,bytes)'
SAFE_TRANSACTION='(uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256)'
TRANSACTION_PROPOSED_SIG="TransactionProposed(bytes32,bytes32,address,uint64,bytes,$SAFE_TRANSACTION)"
TRANSACTION_ATTESTED_SIG='TransactionAttested(bytes32,bytes32,address,uint64,bytes32,bytes32,((uint256,uint256),uint256))'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/shared_test_scripts.sh"

TMPDIR="$(mktemp -d)"
PIDS=()
VALIDATOR_A_PID=""
VALIDATOR_B_PID=""

require_commands anvil cast forge jq cargo
install_cleanup_trap

echo "==> Using temporary directory $TMPDIR"

build_services_and_contracts

echo "==> Starting Anvil..."
start_anvil "$BLOCK_TIME" "$ANVIL_PORT" "$REPO_ROOT/anvil_logs.txt" "$ANVIL_RPC_URL"

PARTICIPANTS_CSV=$(IFS=,; echo "${PARTICIPANTS[*]}")
deploy_validator_contracts "$ANVIL_RPC_URL" "$SENDER" "$PARTICIPANTS_CSV" "$CHAIN_ID"
deploy_safenet_7702_executor "$ANVIL_RPC_URL" "$SENDER" "$CHAIN_ID"

DELEGATED_CODE=$(echo "0xef0100${EXECUTOR_ADDR#0x}" | tr '[:upper:]' '[:lower:]')
EXECUTE_SELECTOR=$(cast sig "$EXECUTE_SIG")
CALL_FAILED_TOPIC=$(cast keccak "$CALL_FAILED_SIG")

# `blocks_per_epoch` is set far out of this test's window, so that the
# genesis group attests every transaction.
validator_config() {
    print_validator_config_base \
        "$ANVIL_RPC_URL" "$1" "$2" "$CONSENSUS_ADDR" "$ORACLE_ADDR" \
        1000000 "$(($BLOCK_TIME * 1000))" PARTICIPANTS "${3:-}"
}

VALIDATOR_A_DB="$TMPDIR/validator_a.sqlite"
VALIDATOR_A_CONFIG="$TMPDIR/validator_a.toml"
validator_config "${PRIVATE_KEYS[0]}" "$VALIDATOR_A_DB" "$EXECUTOR_ADDR" > "$VALIDATOR_A_CONFIG"
VALIDATOR_B_CONFIG="$TMPDIR/validator_b.toml"
validator_config "${PRIVATE_KEYS[1]}" "$TMPDIR/validator_b.sqlite" "$EXECUTOR_ADDR" > "$VALIDATOR_B_CONFIG"

start_validator_a() {
    echo "==> Starting validator A (${PARTICIPANTS[0]})..."
    run_rust_process validator "$VALIDATOR_A_CONFIG" "$REPO_ROOT/validator_a_logs.txt" "$1"
    VALIDATOR_A_PID="$LAST_PID"
    echo "    pid $VALIDATOR_A_PID"
}

start_validator_a truncate

echo "==> Starting validator B (${PARTICIPANTS[1]})..."
run_rust_process validator "$VALIDATOR_B_CONFIG" "$REPO_ROOT/validator_b_logs.txt"
VALIDATOR_B_PID="$LAST_PID"
echo "    pid $VALIDATOR_B_PID"

# Let both watchers initialize before emitting the genesis event.
sleep 0.5
assert_processes_alive "FAILURE: A validator exited during startup." "$VALIDATOR_A_PID" "$VALIDATOR_B_PID"

trigger_genesis_keygen "$ANVIL_RPC_URL" "$SENDER" "$PARTICIPANTS_CSV" "$COORDINATOR_ADDR"

account_code() {
    cast code "$1" --rpc-url "$ANVIL_RPC_URL" | tr '[:upper:]' '[:lower:]'
}

echo "==> Waiting for genesis to complete and both validators to delegate to the executor (timeout: ${TIMEOUT}s)..."
DEADLINE=$((SECONDS + TIMEOUT))
TRUE_WORD=0000000000000000000000000000000000000000000000000000000000000001
GENESIS_COMPLETED=0
DELEGATED=0
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    GENESIS_COMPLETED=$(fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" 'KeyGenConfirmed(bytes32,address,bool)' \
        | jq --arg true_word "$TRUE_WORD" '[.[] | select(.data | endswith($true_word))] | length')
    DELEGATED=0
    for address in "${PARTICIPANTS[@]}"; do
        if [ "$(account_code "$address")" = "$DELEGATED_CODE" ]; then
            DELEGATED=$((DELEGATED + 1))
        fi
    done
    echo "    genesis completed: $([ "$GENESIS_COMPLETED" -gt 0 ] && echo yes || echo no); validators delegated: $DELEGATED/${#PARTICIPANTS[@]}"
    [ "$GENESIS_COMPLETED" -gt 0 ] && [ "$DELEGATED" -eq "${#PARTICIPANTS[@]}" ] && break

    assert_processes_alive "FAILURE: A validator exited before genesis completed." "$VALIDATOR_A_PID" "$VALIDATOR_B_PID"
    sleep "$BLOCK_TIME"
done

if [ "$GENESIS_COMPLETED" -lt 1 ] || [ "$DELEGATED" -ne "${#PARTICIPANTS[@]}" ]; then
    EXIT_MESSAGE="TIMEOUT: genesis did not complete with both validators delegated to the executor in time."
    exit 1
fi

PROPOSAL_BLOCK=$(cast block-number --rpc-url "$ANVIL_RPC_URL")

propose_transaction_call() {
    local nonce=$1
    local calldata
    calldata=$(cast calldata "proposeTransaction(address,bytes,$SAFE_TRANSACTION)" \
        "$ORACLE_ADDR" 0x \
        "($CHAIN_ID,$SENDER,$SENDER,0,0x,0,0,0,0,0x0000000000000000000000000000000000000000,0x0000000000000000000000000000000000000000,$nonce)")
    echo "($CONSENSUS_ADDR,0,1000000,$calldata)"
}

echo "==> Proposing three transactions in one batched self-call..."
cast send --rpc-url "$ANVIL_RPC_URL" --private-key "$SENDER_KEY" \
    --auth "$EXECUTOR_ADDR" \
    "$SENDER" "$EXECUTE_SIG" \
    "[$(propose_transaction_call 1),$(propose_transaction_call 2),$(propose_transaction_call 3)]" \
    >/dev/null

PROPOSALS=$(cast logs --json \
    --rpc-url "$ANVIL_RPC_URL" \
    --from-block "$PROPOSAL_BLOCK" --to-block latest \
    --address "$CONSENSUS_ADDR" \
    "$TRANSACTION_PROPOSED_SIG")
PROPOSAL_COUNT=$(jq 'length' <<< "$PROPOSALS")
PROPOSAL_BLOCKS=$(jq '[.[].blockNumber] | unique | length' <<< "$PROPOSALS")
if [ "$PROPOSAL_COUNT" -ne 3 ] || [ "$PROPOSAL_BLOCKS" -ne 1 ]; then
    EXIT_MESSAGE="FAILURE: expected three transactions proposed in one block, got $PROPOSAL_COUNT across $PROPOSAL_BLOCKS block(s)."
    exit 1
fi
TRANSACTION_HASHES=$(jq '[.[].topics[1]]' <<< "$PROPOSALS")

# Waits until every transaction hash in the JSON array `hashes` is attested,
# then sets `ATTESTED_BLOCK` to the block of the last attestation.
wait_for_attestations() {
    local hashes=$1 expected attested=0 attestations
    expected=$(jq 'length' <<< "$hashes")
    echo "==> Waiting for $expected attestation(s) (timeout: ${TIMEOUT}s)..."
    DEADLINE=$((SECONDS + TIMEOUT))
    while [ "$SECONDS" -lt "$DEADLINE" ]; do
        attestations=$(fetch_logs "$ANVIL_RPC_URL" "$CONSENSUS_ADDR" "$TRANSACTION_ATTESTED_SIG" \
            | jq --argjson hashes "$hashes" '[.[] | select(.topics[1] | IN($hashes[]))]')
        attested=$(jq 'length' <<< "$attestations")
        echo "    attested: $attested/$expected"
        if [ "$attested" -ge "$expected" ]; then
            ATTESTED_BLOCK=$(jq -r '.[].blockNumber' <<< "$attestations" | max_block)
            return 0
        fi

        assert_processes_alive "FAILURE: A validator exited before the transactions were attested." "$VALIDATOR_A_PID" "$VALIDATOR_B_PID"
        sleep "$BLOCK_TIME"
    done

    EXIT_MESSAGE="TIMEOUT: only $attested of $expected transaction(s) were attested."
    exit 1
}

wait_for_attestations "$TRANSACTION_HASHES"

echo "==> Checking every validator transaction up to block $ATTESTED_BLOCK is a self-call to execute with no failed calls..."
PARTICIPANTS_JSON=$(printf '%s\n' "${PARTICIPANTS[@]}" | jq -R 'ascii_downcase' | jq -s .)
BATCHES=0
for block in $(seq 1 "$ATTESTED_BLOCK"); do
    TRANSACTIONS=$(cast block "$block" --full --json --rpc-url "$ANVIL_RPC_URL" \
        | jq -c --argjson participants "$PARTICIPANTS_JSON" \
            '.transactions[] | select(.from | ascii_downcase | IN($participants[]))')
    while read -r transaction; do
        [ -z "$transaction" ] && continue
        FROM=$(jq -r '.from | ascii_downcase' <<< "$transaction")
        TO=$(jq -r '.to | ascii_downcase' <<< "$transaction")
        INPUT=$(jq -r '.input' <<< "$transaction")
        HASH=$(jq -r '.hash' <<< "$transaction")
        if [ "$TO" != "$FROM" ] || [ "${INPUT:0:10}" != "$EXECUTE_SELECTOR" ]; then
            EXIT_MESSAGE="FAILURE: validator $FROM sent $HASH in block $block to $TO with selector ${INPUT:0:10}, not a self-call to execute."
            exit 1
        fi
        RECEIPT=$(cast receipt "$HASH" --json --rpc-url "$ANVIL_RPC_URL")
        if [ "$(jq -r '.status' <<< "$RECEIPT")" != "0x1" ]; then
            EXIT_MESSAGE="FAILURE: validator $FROM's batch $HASH in block $block reverted."
            exit 1
        fi
        # The executor swallows failing calls and only logs them, so a batch
        # that succeeds may still have dropped some of its calls.
        FAILED_CALLS=$(jq --arg account "$FROM" --arg topic "$CALL_FAILED_TOPIC" \
            '[.logs[] | select((.address | ascii_downcase) == $account and .topics[0] == $topic)] | length' \
            <<< "$RECEIPT")
        if [ "$FAILED_CALLS" -ne 0 ]; then
            EXIT_MESSAGE="FAILURE: $FAILED_CALLS call(s) in validator $FROM's batch $HASH in block $block failed."
            exit 1
        fi
        CALLS=$(cast decode-calldata "$EXECUTE_SIG" "$INPUT" --json | jq '.[0] | length')
        echo "    block $block: $FROM type $(($(jq -r '.type' <<< "$transaction"))), $CALLS call(s), gas limit $(($(jq -r '.gas' <<< "$transaction"))), gas used $(($(jq -r '.gasUsed' <<< "$RECEIPT")))"
        BATCHES=$((BATCHES + 1))
    done <<< "$TRANSACTIONS"
done
echo "    $BATCHES batch(es) checked"

echo "==> Restarting validator A without the executor..."
kill "$VALIDATOR_A_PID"
wait "$VALIDATOR_A_PID" 2>/dev/null || true
validator_config "${PRIVATE_KEYS[0]}" "$VALIDATOR_A_DB" > "$VALIDATOR_A_CONFIG"
start_validator_a append

echo "==> Proposing a fourth transaction..."
env \
    CONSENSUS_ADDRESS="$CONSENSUS_ADDR" \
    ORACLE_ADDRESS="$ORACLE_ADDR" \
    TX_CHAIN_ID="$CHAIN_ID" \
    TX_SAFE="$SENDER" \
    TX_TO="$SENDER" \
    TX_NONCE=4 \
    forge script --root "$REPO_ROOT/contracts" ProposeTransactionScript \
    --rpc-url "$ANVIL_RPC_URL" \
    --unlocked \
    --sender "$SENDER" \
    --broadcast

TRANSACTION_HASH=$(fetch_logs "$ANVIL_RPC_URL" "$CONSENSUS_ADDR" "$TRANSACTION_PROPOSED_SIG" \
    | jq -er '.[-1].topics[1]')
wait_for_attestations "[\"$TRANSACTION_HASH\"]"

VALIDATOR_A_CODE=$(account_code "${PARTICIPANTS[0]}")
if [ "$VALIDATOR_A_CODE" != "0x" ]; then
    EXIT_MESSAGE="FAILURE: validator A's signer account still has code $VALIDATOR_A_CODE after restarting without the executor."
    exit 1
fi

EXIT_MESSAGE="SUCCESS: the validators batched every transaction through the executor, attested three transactions proposed in one block, and validator A undelegated after restarting without the executor."
exit 0
