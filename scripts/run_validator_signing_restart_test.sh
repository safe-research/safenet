#!/bin/bash
# Regression test: a transaction signing ceremony whose signature share round
# times out must be restarted with a new signing request and complete, without
# waiting for an oracle result that was already emitted for the transaction.
#
# This starts Anvil, deploys the contracts, and runs three Rust validator
# instances through genesis key generation, forming a 2-of-3 group, and the
# staging of epoch 1 (the genesis group's only other signing ceremony). Anvil's
# interval mining is then paused so that blocks are mined by this script, and
# a Safe transaction is proposed for the genesis group. As soon as validator
# C's nonce commitment is in the transaction pool (but not yet mined),
# validator C is stopped and interval mining resumes. All three validators
# commit their nonces, but only A and B publish signature shares, so the
# signature share round times out and the ceremony is restarted by A and B
# with a new signing request. The test succeeds once the transaction is
# attested under that new signing request, with nonces and signature shares
# from A and B only.
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

# Selector of `FROSTCoordinator.signCommitNonces`.
SIGN_COMMIT_NONCES_SELECTOR=0x38e9bdda

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

# From here on, blocks are only mined by this script until validator C is
# stopped, so that its nonce commitment can be caught in the transaction pool
# before it is mined and C gets a chance to publish its signature share.
echo "==> Pausing interval mining..."
cast rpc --rpc-url "$ANVIL_RPC_URL" evm_setIntervalMining 0 >/dev/null

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
    --broadcast &
PROPOSAL_PID=$!
PIDS+=("$PROPOSAL_PID")

echo "==> Mining blocks until validator C's nonce commitment is pending (timeout: ${TIMEOUT}s)..."
DEADLINE=$((SECONDS + TIMEOUT))
VALIDATOR_C=$(tr '[:upper:]' '[:lower:]' <<< "${PARTICIPANTS[2]}")
COMMIT_PENDING=0
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    COMMIT_PENDING=$(cast rpc --rpc-url "$ANVIL_RPC_URL" txpool_content | jq \
        --arg from "$VALIDATOR_C" --arg selector "$SIGN_COMMIT_NONCES_SELECTOR" \
        '[.pending | to_entries[] | select((.key | ascii_downcase) == $from)
            | .value[] | select(.input | startswith($selector))] | length')
    [ "$COMMIT_PENDING" -gt 0 ] && break

    assert_processes_alive "FAILURE: A validator exited before validator C committed its nonces." "${VALIDATOR_PIDS[@]}"
    cast rpc --rpc-url "$ANVIL_RPC_URL" evm_mine >/dev/null
    sleep "$BLOCK_TIME"
done

if [ "$COMMIT_PENDING" -eq 0 ]; then
    EXIT_MESSAGE="TIMEOUT: validator C did not commit its nonces for the proposed transaction in time."
    exit 1
fi

echo "==> Stopping validator C (${PARTICIPANTS[2]}) so that it never publishes its signature share..."
kill "${VALIDATOR_PIDS[2]}"
wait "${VALIDATOR_PIDS[2]}" 2>/dev/null || true
LIVE_VALIDATOR_PIDS=("${VALIDATOR_PIDS[@]:0:2}")

echo "==> Resuming interval mining..."
cast rpc --rpc-url "$ANVIL_RPC_URL" evm_setIntervalMining "$BLOCK_TIME" >/dev/null
wait "$PROPOSAL_PID"

GENESIS_EPOCH_WORD=0x0000000000000000000000000000000000000000000000000000000000000000
PROPOSALS=$(fetch_logs "$ANVIL_RPC_URL" "$CONSENSUS_ADDR" \
    'TransactionProposed(bytes32,bytes32,address,uint64,bytes,(uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256))')
PROPOSAL=$(jq -ec --arg epoch "$GENESIS_EPOCH_WORD" \
    '[.[] | select(.data | startswith($epoch))][-1]' <<< "$PROPOSALS")
TRANSACTION_HASH=$(jq -r '.topics[1]' <<< "$PROPOSAL")

# The proposal makes the transaction's first signing request in the same
# transaction. `message` is its third indexed topic.
SIGN_EVENT='Sign(address,bytes32,bytes32,bytes32,uint64)'
SIGN_MESSAGE=$(fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" "$SIGN_EVENT" | jq -er \
    --arg tx "$(jq -r '.transactionHash' <<< "$PROPOSAL")" \
    '[.[] | select(.transactionHash == $tx)][0].topics[3]')

# Prints the signature IDs of every signing request for the transaction, in
# the order they were made. The signature ID is the first non-indexed word.
signing_requests() {
    fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" "$SIGN_EVENT" | jq -c --arg message "$SIGN_MESSAGE" \
        '[.[] | select(.topics[3] == $message) | "0x" + .data[2:66]]'
}

echo "==> Waiting for the genesis group to attest transaction $TRANSACTION_HASH (timeout: ${TIMEOUT}s)..."
DEADLINE=$((SECONDS + TIMEOUT))
ATTESTATION=""
SIGNING_REQUESTS="[]"
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    SIGNING_REQUESTS=$(signing_requests)
    ATTESTATIONS=$(fetch_logs "$ANVIL_RPC_URL" "$CONSENSUS_ADDR" \
        'TransactionAttested(bytes32,bytes32,address,uint64,bytes32,bytes32,((uint256,uint256),uint256))')
    ATTESTATION=$(jq -c --arg hash "$TRANSACTION_HASH" --arg epoch "$GENESIS_EPOCH_WORD" \
        '[.[] | select((.topics[1] == $hash) and (.data | startswith($epoch)))][0] // empty' <<< "$ATTESTATIONS")

    echo "    signing requests: $(jq 'length' <<< "$SIGNING_REQUESTS"); attested: $([ -n "$ATTESTATION" ] && echo yes || echo no) at block $(cast block-number --rpc-url "$ANVIL_RPC_URL")"
    [ -n "$ATTESTATION" ] && break

    assert_processes_alive "FAILURE: A validator exited while waiting for the attestation." "${LIVE_VALIDATOR_PIDS[@]}"
    sleep "$BLOCK_TIME"
done

if [ -z "$ATTESTATION" ]; then
    # Point out which stage the last signing request timed out in, if any.
    # Only the "signing ceremony timed out" log has a `stage` field.
    STAGE=$(jq -rR --arg sid "Some($(jq -r '.[-1]' <<< "$SIGNING_REQUESTS"))" \
        'fromjson? | select(.fields.signature_id == $sid) | .fields.stage // empty' \
        "$REPO_ROOT/validator_a_logs.txt" | head -n 1 || true)
    EXIT_MESSAGE="TIMEOUT: the genesis group ($GENESIS_GROUP) did not attest transaction $TRANSACTION_HASH in time after $(jq 'length' <<< "$SIGNING_REQUESTS") signing request(s) $SIGNING_REQUESTS${STAGE:+ (validator A timed out the last one in its $STAGE stage)}."
    exit 1
fi

# The signature ID is the third non-indexed word of `TransactionAttested`,
# after `epoch` and `oracleDataHash`.
ATTESTED_SID="0x$(jq -r '.data[130:194]' <<< "$ATTESTATION")"

# The ceremony must have been restarted exactly once, and the attestation
# must be for the restarted signing request.
FIRST_SID=$(jq -r '.[0]' <<< "$SIGNING_REQUESTS")
RESTARTED_SID=$(jq -r '.[1] // empty' <<< "$SIGNING_REQUESTS")
if [ "$(jq 'length' <<< "$SIGNING_REQUESTS")" -ne 2 ] || [ "$RESTARTED_SID" != "$ATTESTED_SID" ]; then
    EXIT_MESSAGE="FAILURE: expected the transaction to be attested by a single restarted signing request, but it was attested by $ATTESTED_SID after signing requests $SIGNING_REQUESTS."
    exit 1
fi

# Prints the sorted participants (lowercase, without `0x`) of the
# `SignRevealedNonces` or `SignShared` logs in `$2` for the signature ID `$1`.
# `participant` is the first non-indexed word of both events, a padded address.
participants_of() {
    jq -c --arg sid "$1" \
        '[.[] | select(.topics[1] == $sid) | .data[26:66] | ascii_downcase] | sort' <<< "$2"
}
expected_participants() {
    printf '%s\n' "$@" | jq -nRc '[inputs | ascii_downcase | ltrimstr("0x")] | sort'
}
COMMITTED_LOGS=$(fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" \
    'SignRevealedNonces(bytes32,address,((uint256,uint256),(uint256,uint256)))')
SHARED_LOGS=$(fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" \
    'SignShared(bytes32,bytes32,address,uint256)')

# The first ceremony must have timed out in its signature share round: every
# validator committed its nonces, but validator C never shared.
EVERYONE=$(expected_participants "${PARTICIPANTS[@]}")
A_AND_B=$(expected_participants "${PARTICIPANTS[@]:0:2}")
FIRST_COMMITTED=$(participants_of "$FIRST_SID" "$COMMITTED_LOGS")
FIRST_SHARED=$(participants_of "$FIRST_SID" "$SHARED_LOGS")
if [ "$FIRST_COMMITTED" != "$EVERYONE" ] || [ "$FIRST_SHARED" != "$A_AND_B" ]; then
    EXIT_MESSAGE="FAILURE: expected the first signing request $FIRST_SID to have nonces from $EVERYONE and signature shares from $A_AND_B, but nonces were committed by $FIRST_COMMITTED and signature shares were shared by $FIRST_SHARED."
    exit 1
fi

# The restarted ceremony must only include validators A and B.
RESTARTED_COMMITTED=$(participants_of "$RESTARTED_SID" "$COMMITTED_LOGS")
RESTARTED_SHARED=$(participants_of "$RESTARTED_SID" "$SHARED_LOGS")
if [ "$RESTARTED_COMMITTED" != "$A_AND_B" ] || [ "$RESTARTED_SHARED" != "$A_AND_B" ]; then
    EXIT_MESSAGE="FAILURE: expected the restarted signing request $RESTARTED_SID to have nonces and signature shares from $A_AND_B, but nonces were committed by $RESTARTED_COMMITTED and signature shares were shared by $RESTARTED_SHARED."
    exit 1
fi

EXIT_MESSAGE="SUCCESS: the genesis group ($GENESIS_GROUP) attested transaction $TRANSACTION_HASH under the restarted signing request $RESTARTED_SID, with validators A and B after validator C never published its signature share for $FIRST_SID."
exit 0
