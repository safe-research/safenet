#!/bin/bash
# Regression test: signing nonces must survive a reorg that rewinds a group's
# DKG past the block where its key share was confirmed.
#
# Such a reorg rolls the group's local DKG status back from "confirmed" to
# "still collecting shares", after nonces were already committed for one of its
# signing ceremonies. The validator must then re-share and re-confirm its key
# share, and the nonces it stored for the ceremony must still be the ones it
# signs with: once the ceremony is requested again under the same signature ID,
# it re-commits the same nonces rather than generating new ones.
#
# This starts Anvil, deploys the contracts, and runs two Rust validator
# instances through genesis key generation. Once the genesis group is confirmed,
# validator B is stopped and a Safe transaction is proposed, so that validator
# A commits its nonces for the transaction's signing ceremony but cannot sign
# with them. The chain is then reorged back past the block where secret shares
# were distributed (using `anvil_reorg`, which mines empty replacement blocks
# over the reorged range), and validator B is restarted. Both validators detect
# the reorg, roll their local state back to before the key share was
# confirmed, and reprocess the (now share-less) chain. Finally, the same
# transaction is proposed again and must be attested.
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

VALIDATOR_A_DB="$TMPDIR/validator_a.sqlite"
VALIDATOR_B_DB="$TMPDIR/validator_b.sqlite"

validator_config() {
    print_validator_config_base \
        "$ANVIL_RPC_URL" "$1" "$2" "$CONSENSUS_ADDR" "$ORACLE_ADDR" \
        1000000 "$(($BLOCK_TIME * 1000))" PARTICIPANTS

    # Large enough reorg depth support to work with slow CI tests, as the
    # reorg spans the whole DKG confirmation and a signing request.
    echo "max_reorg_depth = 20"
}

VALIDATOR_A_CONFIG="$TMPDIR/validator_a.toml"
validator_config "${PRIVATE_KEYS[0]}" "$VALIDATOR_A_DB" > "$VALIDATOR_A_CONFIG"
VALIDATOR_B_CONFIG="$TMPDIR/validator_b.toml"
validator_config "${PRIVATE_KEYS[1]}" "$VALIDATOR_B_DB" > "$VALIDATOR_B_CONFIG"

start_validator_a() {
    echo "==> Starting validator A (${PARTICIPANTS[0]})..."
    run_rust_process validator "$VALIDATOR_A_CONFIG" "$REPO_ROOT/validator_a_logs.txt" "$1"
    VALIDATOR_A_PID="$LAST_PID"
    echo "    pid $VALIDATOR_A_PID"
}

start_validator_b() {
    echo "==> Starting validator B (${PARTICIPANTS[1]})..."
    run_rust_process validator "$VALIDATOR_B_CONFIG" "$REPO_ROOT/validator_b_logs.txt" "$1"
    VALIDATOR_B_PID="$LAST_PID"
    echo "    pid $VALIDATOR_B_PID"
}

# Proposes the same Safe transaction for the genesis group (still active -
# `blocks_per_epoch` is set far out of this test's window) every time, so that
# a proposal after the reorg requests the same message to be signed.
propose_transaction() {
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
        --broadcast 2>&1 | tee -a "$TMPDIR/propose.log"
}

# Prints validator A's nonce commitments for signature ID `$1` as a JSON array
# of the events' non-indexed data after the `participant` word: the committed
# `(d, e)` points.
validator_a_nonces() {
    fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" \
        'SignRevealedNonces(bytes32,address,((uint256,uint256),(uint256,uint256)))' |
        jq --arg sid "$1" --arg addr "${PARTICIPANTS[0]#0x}" \
            '[.[] | select(.topics[1] == $sid) | select((.data[26:66] | ascii_downcase) == ($addr | ascii_downcase)) | .data[66:]]'
}

# Prints the number of `KeyGenConfirmed(..., confirmed: true)` events for the
# genesis group, i.e. whether every participant confirmed its key share.
TRUE_WORD=0000000000000000000000000000000000000000000000000000000000000001
genesis_confirmations() {
    fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" 'KeyGenConfirmed(bytes32,address,bool)' |
        jq --arg gid "$GENESIS_GROUP" --arg true_word "$TRUE_WORD" \
            '[.[] | select(.topics[1] == $gid) | select(.data | endswith($true_word))] | length'
}

start_validator_a truncate
start_validator_b truncate

# Let both watchers initialize before emitting the genesis event.
sleep 0.5
assert_processes_alive "FAILURE: A validator exited during startup." "$VALIDATOR_A_PID" "$VALIDATOR_B_PID"

trigger_genesis_keygen "$ANVIL_RPC_URL" "$SENDER" "$PARTICIPANTS_CSV" "$COORDINATOR_ADDR"

DEADLINE=$((SECONDS + TIMEOUT))
GENESIS_GROUP=""
SECRET_SHARED_BLOCK=""
GENESIS_CONFIRMED=0

echo "==> Waiting for genesis secret shares and key share confirmations (timeout: ${TIMEOUT}s)..."
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    SHARED=$(fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" \
        'KeyGenSecretShared(bytes32,address,((uint256,uint256),uint256[]),bool)')
    SHARED_COUNT=$(jq 'length' <<< "$SHARED")

    if [ "$SHARED_COUNT" -ge 2 ] && [ -z "$SECRET_SHARED_BLOCK" ]; then
        GENESIS_GROUP=$(jq -r '.[0].topics[1]' <<< "$SHARED")
        SECRET_SHARED_BLOCK=$(jq -r '.[].blockNumber' <<< "$SHARED" | max_block)
        echo "    genesis group $GENESIS_GROUP shared secrets by block $SECRET_SHARED_BLOCK"
    fi

    if [ -n "$GENESIS_GROUP" ]; then
        GENESIS_CONFIRMED=$(genesis_confirmations)
        [ "$GENESIS_CONFIRMED" -gt 0 ] && break
    fi

    assert_processes_alive "FAILURE: A validator exited before genesis completed." "$VALIDATOR_A_PID" "$VALIDATOR_B_PID"
    sleep "$BLOCK_TIME"
done

if [ "$GENESIS_CONFIRMED" -lt 1 ]; then
    EXIT_MESSAGE="TIMEOUT: the genesis group was not confirmed in time."
    exit 1
fi
echo "    genesis group $GENESIS_GROUP confirmed"

# Genesis is a strict 2-of-2 group here, so without validator B the signing
# ceremony stops after validator A commits its nonces, and validator A never
# uses them. Otherwise, validator A would burn its nonces signing the
# ceremony that the reorg undoes, and could not take part in the same
# signature ID again.
echo "==> Stopping validator B..."
kill "$VALIDATOR_B_PID"
wait "$VALIDATOR_B_PID" 2>/dev/null || true

echo "==> Proposing a Safe transaction for the genesis group to sign before the reorg..."
propose_transaction

echo "==> Waiting for validator A to commit its nonces (timeout: ${TIMEOUT}s)..."
DEADLINE=$((SECONDS + TIMEOUT))
SIGNATURE_ID=""
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    assert_processes_alive "FAILURE: validator A exited before committing its nonces." "$VALIDATOR_A_PID"

    REQUESTS=$(fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" 'Sign(address,bytes32,bytes32,bytes32,uint64)')
    # `sid` is the first non-indexed word.
    SIGNATURE_ID=$(jq -r --arg gid "$GENESIS_GROUP" \
        '[.[] | select(.topics[2] == $gid) | "0x" + .data[2:66]][-1] // empty' <<< "$REQUESTS")
    if [ -n "$SIGNATURE_ID" ]; then
        NONCES=$(validator_a_nonces "$SIGNATURE_ID")
        [ "$(jq 'length' <<< "$NONCES")" -gt 0 ] && break
    fi

    sleep "$BLOCK_TIME"
done

if [ -z "$SIGNATURE_ID" ] || [ "$(jq 'length' <<< "$NONCES")" -lt 1 ]; then
    EXIT_MESSAGE="TIMEOUT: validator A did not commit its nonces in time."
    exit 1
fi
COMMITTED_NONCES=$(jq -r '.[0]' <<< "$NONCES")
echo "    validator A committed nonces for signature $SIGNATURE_ID"

CURRENT_BLOCK=$(cast block-number --rpc-url "$ANVIL_RPC_URL")
REORG_DEPTH=$((CURRENT_BLOCK - SECRET_SHARED_BLOCK + 1))

echo "==> Reorging $REORG_DEPTH block(s) from block $CURRENT_BLOCK, spanning back past block $SECRET_SHARED_BLOCK where genesis's secret shares were shared..."
cast rpc anvil_reorg "$REORG_DEPTH" '[]' --rpc-url "$ANVIL_RPC_URL" >/dev/null

start_validator_b append

# The reorg also rewinds the genesis group's onchain KeyGenConfirmed(...,
# confirmed: true) event, since it was logged at or after
# SECRET_SHARED_BLOCK and that range just got replaced with empty blocks.
# `proposeTransaction` reverts with `GroupNotReady` until the group's FROST
# state machine reaches FINALIZED again, which only happens once every
# participant has replayed its keyGenConfirm call on the reorged chain -
# waiting a fixed number of blocks isn't a reliable proxy for that and can
# race the proposal below, so wait for the (re-emitted) completed event
# itself instead.
echo "==> Waiting for the validators to reprocess the reorged chain and reconfirm the genesis group's key share (timeout: ${TIMEOUT}s)..."
DEADLINE=$((SECONDS + TIMEOUT))
GENESIS_RECONFIRMED=0
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    assert_processes_alive "FAILURE: A validator exited after the reorg." "$VALIDATOR_A_PID" "$VALIDATOR_B_PID"

    GENESIS_RECONFIRMED=$(genesis_confirmations)
    [ "$GENESIS_RECONFIRMED" -gt 0 ] && break

    sleep "$BLOCK_TIME"
done

if [ "$GENESIS_RECONFIRMED" -lt 1 ]; then
    EXIT_MESSAGE="TIMEOUT: the genesis group ($GENESIS_GROUP) was not reconfirmed after the reorg in time."
    exit 1
fi
echo "==> Genesis group $GENESIS_GROUP reconfirmed after the reorg"

# The reorg also undid the proposal, so propose it again. It is the group's
# first signing request on the reorged chain too, so it gets the same
# signature ID as before.
echo "==> Proposing the same Safe transaction for the genesis group to sign after the reorg..."
propose_transaction

GENESIS_EPOCH_WORD=0x0000000000000000000000000000000000000000000000000000000000000000
PROPOSALS=$(fetch_logs "$ANVIL_RPC_URL" "$CONSENSUS_ADDR" \
    'TransactionProposed(bytes32,bytes32,address,uint64,bytes,(uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256))')
TRANSACTION_HASH=$(jq -er --arg epoch "$GENESIS_EPOCH_WORD" \
    '[.[] | select(.data | startswith($epoch))][-1].topics[1]' <<< "$PROPOSALS")

echo "==> Waiting for the genesis group to attest transaction $TRANSACTION_HASH (timeout: ${TIMEOUT}s)..."
DEADLINE=$((SECONDS + TIMEOUT))
TRANSACTION_ATTESTED=0
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    ATTESTATIONS=$(fetch_logs "$ANVIL_RPC_URL" "$CONSENSUS_ADDR" \
        'TransactionAttested(bytes32,bytes32,address,uint64,bytes32,bytes32,((uint256,uint256),uint256))')
    TRANSACTION_ATTESTED=$(jq --arg hash "$TRANSACTION_HASH" --arg epoch "$GENESIS_EPOCH_WORD" \
        '[.[] | select((.topics[1] == $hash) and (.data | startswith($epoch)))] | length' <<< "$ATTESTATIONS")
    [ "$TRANSACTION_ATTESTED" -gt 0 ] && break

    assert_processes_alive "FAILURE: A validator exited while waiting for the post-reorg attestation." "$VALIDATOR_A_PID" "$VALIDATOR_B_PID"
    sleep "$BLOCK_TIME"
done

if [ "$TRANSACTION_ATTESTED" -lt 1 ]; then
    EXIT_MESSAGE="TIMEOUT: the genesis group ($GENESIS_GROUP) did not attest a transaction after a reorg spanning the KeyGenSecretShared block."
    exit 1
fi

# Validator A only commits once per ceremony, so its single commitment for the
# signature ID on the reorged chain is the one the group signed with.
RECOMMITTED_NONCES=$(validator_a_nonces "$SIGNATURE_ID" | jq -r '.[0] // empty')
if [ "$RECOMMITTED_NONCES" != "$COMMITTED_NONCES" ]; then
    EXIT_MESSAGE="FAILURE: validator A did not re-commit the nonces it stored for signature $SIGNATURE_ID before the reorg."
    exit 1
fi

EXIT_MESSAGE="SUCCESS: the genesis group ($GENESIS_GROUP) attested a transaction after a reorg spanning the KeyGenSecretShared block, with validator A re-committing the nonces it stored for signature $SIGNATURE_ID before the reorg."
exit 0
