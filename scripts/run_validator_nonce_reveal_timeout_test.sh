#!/bin/bash
# Regression test: a signing ceremony whose nonce commitment round times out
# must continue with the signers that committed their nonces, rather than
# restarting the whole ceremony with a new signing request.
#
# This starts Anvil, deploys the contracts, and runs three Rust validator
# instances through genesis key generation, forming a 2-of-3 group, and the
# staging of epoch 1 (the genesis group's only other signing ceremony). Once
# epoch 1 is staged, validator C is stopped and a Safe transaction is proposed
# for the genesis group. Validators A and B commit their nonces but validator C
# never does, so the nonce commitment round times out with a threshold of
# committed nonces. The test succeeds once the transaction is attested under the
# signature ID of the original signing request, with nonces and signature
# shares from A and B only, and without any further signing request (which is
# what restarting the ceremony would need).
#
# Requirements: anvil, forge, cast, jq, and cargo.
set -euo pipefail

ANVIL_PORT=8551
ANVIL_RPC_URL="${ANVIL_RPC_URL:-http://127.0.0.1:$ANVIL_PORT}"
CHAIN_ID=31337
BLOCK_TIME=1
TIMEOUT="${TIMEOUT:-90}"

# Anvil accounts 1, 2 and 3, one per validator instance.
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
NAMES=(a b c)

# Anvil default deployer account (index 0).
SENDER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/shared_test_scripts.sh"

TMPDIR="$(mktemp -d)"
PIDS=()
VALIDATOR_PIDS=()

require_commands anvil cast forge jq cargo
install_cleanup_trap

echo "==> Using temporary directory $TMPDIR"

build_services_and_contracts

echo "==> Starting Anvil..."
start_anvil "$BLOCK_TIME" "$ANVIL_PORT" "$REPO_ROOT/anvil_logs.txt" "$ANVIL_RPC_URL"

PARTICIPANTS_CSV=$(IFS=,; echo "${PARTICIPANTS[*]}")
deploy_validator_contracts "$ANVIL_RPC_URL" "$SENDER" "$PARTICIPANTS_CSV" "$CHAIN_ID"

# `blocks_per_epoch` is set far out of this test's window, so the genesis group
# stays active and, once epoch 1 is staged, signs nothing but the proposed
# transaction.
validator_config() {
    print_validator_config_base \
        "$ANVIL_RPC_URL" "$1" "$2" "$CONSENSUS_ADDR" "$ORACLE_ADDR" \
        1000000 "$(($BLOCK_TIME * 1000))" PARTICIPANTS
}

for i in "${!PARTICIPANTS[@]}"; do
    name="${NAMES[$i]}"
    config="$TMPDIR/validator_$name.toml"
    validator_config "${PRIVATE_KEYS[$i]}" "$TMPDIR/validator_$name.sqlite" > "$config"

    echo "==> Starting validator ${name^^} (${PARTICIPANTS[$i]})..."
    run_rust_process validator "$config" "$REPO_ROOT/validator_${name}_logs.txt"
    VALIDATOR_PIDS+=("$LAST_PID")
    echo "    pid $LAST_PID"
done

# Let the watchers initialize before emitting the genesis event.
sleep 0.5
assert_processes_alive "FAILURE: A validator exited during startup." "${VALIDATOR_PIDS[@]}"

trigger_genesis_keygen "$ANVIL_RPC_URL" "$SENDER" "$PARTICIPANTS_CSV" "$COORDINATOR_ADDR"

DEADLINE=$((SECONDS + TIMEOUT))
TRUE_WORD=0000000000000000000000000000000000000000000000000000000000000001
GENESIS_GROUP=""
GENESIS_COMPLETED=0
STAGED=0

echo "==> Waiting for genesis to complete and epoch 1 to be staged (timeout: ${TIMEOUT}s)..."
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    CONFIRMATIONS=$(fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" 'KeyGenConfirmed(bytes32,address,bool)')
    GENESIS_GROUP=$(jq -r '.[0].topics[1] // empty' <<< "$CONFIRMATIONS")
    GENESIS_COMPLETED=$(jq --arg true_word "$TRUE_WORD" \
        '[.[] | select(.data | endswith($true_word))] | length' <<< "$CONFIRMATIONS")

    STAGED=$(fetch_logs "$ANVIL_RPC_URL" "$CONSENSUS_ADDR" \
        'EpochStaged(uint64,uint64,uint64,bytes32,(uint256,uint256),bytes32,((uint256,uint256),uint256))' \
        | jq '[.[] | select(.topics[2] == "0x0000000000000000000000000000000000000000000000000000000000000001")] | length')

    echo "    genesis completed: $([ "$GENESIS_COMPLETED" -gt 0 ] && echo yes || echo no); epoch 1 staged: $([ "$STAGED" -gt 0 ] && echo yes || echo no)"
    [ "$GENESIS_COMPLETED" -gt 0 ] && [ "$STAGED" -gt 0 ] && break

    assert_processes_alive "FAILURE: A validator exited before genesis completed." "${VALIDATOR_PIDS[@]}"
    sleep "$BLOCK_TIME"
done

if [ "$GENESIS_COMPLETED" -lt 1 ] || [ "$STAGED" -lt 1 ]; then
    EXIT_MESSAGE="TIMEOUT: genesis did not complete with a staged epoch 1 in time."
    exit 1
fi

echo "==> Stopping validator C (${PARTICIPANTS[2]}) so that it never commits its nonces..."
kill "${VALIDATOR_PIDS[2]}"
wait "${VALIDATOR_PIDS[2]}" 2>/dev/null || true
LIVE_VALIDATOR_PIDS=("${VALIDATOR_PIDS[@]:0:2}")

GENESIS_EPOCH_WORD=0x0000000000000000000000000000000000000000000000000000000000000000

echo "==> Proposing a Safe transaction for the genesis group to sign..."
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
PROPOSAL=$(jq -ec --arg epoch "$GENESIS_EPOCH_WORD" \
    '[.[] | select(.data | startswith($epoch))][-1]' <<< "$PROPOSALS")
TRANSACTION_HASH=$(jq -r '.topics[1]' <<< "$PROPOSAL")

echo "==> Waiting for the genesis group to attest transaction $TRANSACTION_HASH (timeout: ${TIMEOUT}s)..."
DEADLINE=$((SECONDS + TIMEOUT))
ATTESTATION=""
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    ATTESTATIONS=$(fetch_logs "$ANVIL_RPC_URL" "$CONSENSUS_ADDR" \
        'TransactionAttested(bytes32,bytes32,address,uint64,bytes32,bytes32,((uint256,uint256),uint256))')
    ATTESTATION=$(jq -c --arg hash "$TRANSACTION_HASH" --arg epoch "$GENESIS_EPOCH_WORD" \
        '[.[] | select((.topics[1] == $hash) and (.data | startswith($epoch)))][0] // empty' <<< "$ATTESTATIONS")
    [ -n "$ATTESTATION" ] && break

    assert_processes_alive "FAILURE: A validator exited while waiting for the attestation." "${LIVE_VALIDATOR_PIDS[@]}"
    sleep "$BLOCK_TIME"
done

if [ -z "$ATTESTATION" ]; then
    EXIT_MESSAGE="TIMEOUT: the genesis group ($GENESIS_GROUP) did not attest transaction $TRANSACTION_HASH without validator C in time."
    exit 1
fi

# The signature ID is the third non-indexed word of `TransactionAttested`,
# after `epoch` and `oracleDataHash`.
ATTESTED_SID="0x$(jq -r '.data[130:194]' <<< "$ATTESTATION")"

# Restarting the ceremony would need a second signing request for the
# transaction, so there must only be the one that the transaction proposal
# made (in the same transaction), and it must be the one that the attestation
# was produced for. `message` is the third indexed topic of `Sign`, and the
# signature ID its first non-indexed word.
SIGN_LOGS=$(fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" 'Sign(address,bytes32,bytes32,bytes32,uint64)')
SIGN_MESSAGE=$(jq -er --arg tx "$(jq -r '.transactionHash' <<< "$PROPOSAL")" \
    '[.[] | select(.transactionHash == $tx)][0].topics[3]' <<< "$SIGN_LOGS")
SIGN_REQUESTS=$(jq -c --arg message "$SIGN_MESSAGE" \
    '[.[] | select(.topics[3] == $message) | "0x" + .data[2:66]]' <<< "$SIGN_LOGS")
SIGN_REQUEST_COUNT=$(jq 'length' <<< "$SIGN_REQUESTS")
REQUESTED_SID=$(jq -r '.[0]' <<< "$SIGN_REQUESTS")
if [ "$SIGN_REQUEST_COUNT" -ne 1 ] || [ "$REQUESTED_SID" != "$ATTESTED_SID" ]; then
    EXIT_MESSAGE="FAILURE: expected the transaction to be attested by its original signing request $REQUESTED_SID, but it was attested by $ATTESTED_SID after $SIGN_REQUEST_COUNT signing request(s) - the ceremony was likely restarted."
    exit 1
fi

# Both the committed nonces and the signature shares for the signing request
# must come from validators A and B only. `participant` is the first
# non-indexed word of both events, a padded address.
EXPECTED_SIGNERS=$(printf '%s\n' "${PARTICIPANTS[@]:0:2}" | jq -nRc '[inputs | ascii_downcase | ltrimstr("0x")] | sort')
signers_of() {
    jq -c --arg sid "$ATTESTED_SID" \
        '[.[] | select(.topics[1] == $sid) | .data[26:66] | ascii_downcase] | sort' <<< "$1"
}
COMMITTED=$(signers_of "$(fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" \
    'SignRevealedNonces(bytes32,address,((uint256,uint256),(uint256,uint256)))')")
SHARED=$(signers_of "$(fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" \
    'SignShared(bytes32,bytes32,address,uint256)')")
if [ "$COMMITTED" != "$EXPECTED_SIGNERS" ] || [ "$SHARED" != "$EXPECTED_SIGNERS" ]; then
    EXIT_MESSAGE="FAILURE: expected nonces and signature shares from $EXPECTED_SIGNERS, but nonces were committed by $COMMITTED and signature shares were shared by $SHARED."
    exit 1
fi

EXIT_MESSAGE="SUCCESS: the genesis group ($GENESIS_GROUP) attested transaction $TRANSACTION_HASH under its original signing request $ATTESTED_SID, continuing with validators A and B after validator C never committed its nonces."
exit 0
