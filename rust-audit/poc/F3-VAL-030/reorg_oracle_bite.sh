#!/bin/bash
# F3-VAL-030 intended-route PoC: two real Rust validators, real FROSTCoordinator /
# Consensus / SimpleOracle on Anvil, and a natural reorg driven by `anvil_reorg`.
#
# The validators approve-sign a transaction on branch A (oracle approve), their
# signShareWithCallback transactions are broadcast, then the branch that carried
# the approval is reorged out and the oracle resolves the same request DENIED on
# the canonical branch. The validators' already-broadcast share transactions are
# rebroadcast by the transaction queue and complete the group signature on the
# canonical branch, whose callback attests the transaction even though the
# canonical oracle result is approved=false.
#
# Pass: canonical chain has OracleResult(approved=false) and NO surviving
# approved=true result for the request, yet Consensus.getAttestationSignatureId
# is non-zero (a TransactionAttested for a denied request).
#
# Requirements: anvil, forge, cast, jq, cargo. Run from a sourced rv-val env.
set -euo pipefail

ANVIL_PORT="${POC_ANVIL_PORT:-31400}"
ANVIL_RPC_URL="http://127.0.0.1:$ANVIL_PORT"
CHAIN_ID=31337
BLOCK_TIME=1
TIMEOUT="${TIMEOUT:-90}"
MAX_REORG_DEPTH=5
# Keep a rollover far out of this test's window (genesis group signs throughout).
BLOCKS_PER_EPOCH=1000000

PARTICIPANTS=(
    0x70997970C51812dc3A010C7d01b50e0d17dc79C8
    0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC
)
PRIVATE_KEYS=(
    0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d
    0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a
)
# Anvil default deployer account (index 0): deployer, proposer and oracle approver.
SENDER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
SENDER_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Run against the sourced worktree (its built binary and contracts). The helper
# sets REPO_ROOT from its own location.
source "$WT/scripts/lib/shared_test_scripts.sh"

TMPDIR="$(mktemp -d)"
PIDS=()
require_commands anvil cast forge jq cargo
install_cleanup_trap
echo "==> Temp dir $TMPDIR, RPC $ANVIL_RPC_URL"

# Services and contracts are prebuilt in the warm cache; skip the rebuild the
# shared helper would do and just make sure the binary exists.
test -x "$REPO_ROOT/target/debug/validator" || cargo build --manifest-path "$REPO_ROOT/Cargo.toml" -p validator

echo "==> Starting Anvil..."
start_anvil "$BLOCK_TIME" "$ANVIL_PORT" "$TMPDIR/anvil.log" "$ANVIL_RPC_URL"

PARTICIPANTS_CSV=$(IFS=,; echo "${PARTICIPANTS[*]}")
deploy_validator_contracts "$ANVIL_RPC_URL" "$SENDER" "$PARTICIPANTS_CSV" "$CHAIN_ID"

echo "==> Deploying SimpleOracle(approver=$SENDER)..."
ORACLE_CODE=$(jq -r '.bytecode.object' "$REPO_ROOT/contracts/build/out/SimpleOracle.sol/SimpleOracle.json")
ORACLE_INIT="${ORACLE_CODE}$(cast abi-encode 'c(address)' "$SENDER" | sed 's/^0x//')"
SIMPLE_ORACLE=$(cast send --rpc-url "$ANVIL_RPC_URL" --private-key "$SENDER_KEY" --create "$ORACLE_INIT" --json | jq -r .contractAddress)
echo "    SimpleOracle: $SIMPLE_ORACLE"

validator_config() {
    print_validator_config_base \
        "$ANVIL_RPC_URL" "$1" "$2" "$CONSENSUS_ADDR" "$SIMPLE_ORACLE" \
        "$BLOCKS_PER_EPOCH" "$(($BLOCK_TIME * 1000))" PARTICIPANTS
    echo "max_reorg_depth = $MAX_REORG_DEPTH"
}

VA_CONFIG="$TMPDIR/validator_a.toml"; validator_config "${PRIVATE_KEYS[0]}" "$TMPDIR/a.sqlite" > "$VA_CONFIG"
VB_CONFIG="$TMPDIR/validator_b.toml"; validator_config "${PRIVATE_KEYS[1]}" "$TMPDIR/b.sqlite" > "$VB_CONFIG"

echo "==> Starting validators..."
run_rust_process validator "$VA_CONFIG" "$TMPDIR/va.log"; echo "    A pid $LAST_PID"
run_rust_process validator "$VB_CONFIG" "$TMPDIR/vb.log"; echo "    B pid $LAST_PID"
sleep 0.5
assert_processes_alive "FAILURE: a validator exited during startup." "${PIDS[@]}"

trigger_genesis_keygen "$ANVIL_RPC_URL" "$SENDER" "$PARTICIPANTS_CSV" "$COORDINATOR_ADDR"

TRUE_WORD=0000000000000000000000000000000000000000000000000000000000000001
echo "==> Waiting for genesis confirmation..."
DEADLINE=$((SECONDS + TIMEOUT)); GENESIS_GROUP=""; CONFIRMED=0
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    CONF=$(fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" 'KeyGenConfirmed(bytes32,address,bool)')
    GENESIS_GROUP=$(jq -r '.[0].topics[1] // empty' <<< "$CONF")
    if [ -n "$GENESIS_GROUP" ]; then
        CONFIRMED=$(jq --arg g "$GENESIS_GROUP" --arg t "$TRUE_WORD" '[.[]|select(.topics[1]==$g and (.data|endswith($t)))]|length' <<< "$CONF")
        [ "$CONFIRMED" -ge 1 ] && break
    fi
    assert_processes_alive "FAILURE: a validator exited before genesis." "${PIDS[@]}"
    sleep "$BLOCK_TIME"
done
[ "$CONFIRMED" -ge 1 ] || { EXIT_MESSAGE="TIMEOUT: genesis not confirmed."; exit 1; }
echo "    genesis group $GENESIS_GROUP confirmed"

echo "==> Proposing an oracle-backed transaction (oracle = SimpleOracle)..."
env CONSENSUS_ADDRESS="$CONSENSUS_ADDR" ORACLE_ADDRESS="$SIMPLE_ORACLE" \
    TX_CHAIN_ID="$CHAIN_ID" TX_SAFE="$SENDER" TX_TO="$SENDER" TX_NONCE=1 \
    forge script --root "$REPO_ROOT/contracts" ProposeTransactionScript \
    --rpc-url "$ANVIL_RPC_URL" --unlocked --sender "$SENDER" --broadcast >/dev/null

# The oracle request id is the Consensus message (Sign.message, topic[3]); the
# signature id is the first data word of the Sign event.
SIGN=$(fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" 'Sign(address,bytes32,bytes32,bytes32,uint64)')
MESSAGE=$(jq -r '.[-1].topics[3]' <<< "$SIGN")
SID=$(jq -r '"0x"+.[-1].data[2:66]' <<< "$SIGN")
echo "    request/message $MESSAGE"
echo "    signature id    $SID"

# Fork point: everything after this block is replaced by the reorg, so record it
# before the approval lands.
FORK_BLOCK=$(cast block-number --rpc-url "$ANVIL_RPC_URL")
echo "==> Fork point is block $FORK_BLOCK; approving the request on branch A..."
cast send --rpc-url "$ANVIL_RPC_URL" --private-key "$SENDER_KEY" "$SIMPLE_ORACLE" 'approve(bytes32)' "$MESSAGE" --json >/dev/null

# BITE VARIANT: reorg at a FIXED point inside max_reorg_depth, regardless of
# whether shares were broadcast yet. The reorg removes the approval block but is
# shallow enough (depth 4 <= max_reorg_depth 5) that the validators tolerate it
# rather than halting. On the unpatched code the shares are already out by now
# and get rebroadcast (attested on the denied branch); on the remediation-1
# patch the shares have not been broadcast yet (they wait until the approval is
# max_reorg_depth deep), so nothing attests. This isolates the fix as the only
# difference between reproduce and bite.
REORG_AT=$((FORK_BLOCK + 4))
echo "==> Waiting until block $REORG_AT to reorg (fork at $FORK_BLOCK, depth 4)..."
DEADLINE=$((SECONDS + TIMEOUT))
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    NOW=$(cast block-number --rpc-url "$ANVIL_RPC_URL")
    [ "$NOW" -ge "$REORG_AT" ] && break
    assert_processes_alive "FAILURE: a validator exited on branch A." "${PIDS[@]}"
    sleep "$BLOCK_TIME"
done
SS=$(fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" 'SignShared(bytes32,bytes32,address,uint256)')
SHARES=$(jq --arg s "$SID" '[.[]|select(.topics[1]==$s)]|length' <<< "$SS")
REORG_DEPTH=$((NOW - FORK_BLOCK))
echo "==> At block $NOW: $SHARES shares broadcast so far; reorging depth $REORG_DEPTH back to $FORK_BLOCK..."
cast rpc anvil_reorg "$REORG_DEPTH" '[]' --rpc-url "$ANVIL_RPC_URL" >/dev/null

echo "==> Resolving the SAME request DENIED on the canonical branch..."
# The approval was reorged out, so the request is PENDING again; reject it.
cast send --rpc-url "$ANVIL_RPC_URL" --private-key "$SENDER_KEY" "$SIMPLE_ORACLE" 'reject(bytes32)' "$MESSAGE" --json >/dev/null

echo "==> Waiting for the rebroadcast share transactions to attest on the canonical (denied) branch..."
DEADLINE=$((SECONDS + TIMEOUT)); CANON_ATTEST=0x0000000000000000000000000000000000000000000000000000000000000000
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    CANON_ATTEST=$(cast call --rpc-url "$ANVIL_RPC_URL" "$CONSENSUS_ADDR" 'getAttestationSignatureId(bytes32)(bytes32)' "$MESSAGE")
    [ "$CANON_ATTEST" != "0x0000000000000000000000000000000000000000000000000000000000000000" ] && break
    assert_processes_alive "FAILURE: a validator exited after the reorg." "${PIDS[@]}"
    sleep "$BLOCK_TIME"
done

# Canonical oracle results for the request.
ORES=$(fetch_logs "$ANVIL_RPC_URL" "$SIMPLE_ORACLE" 'OracleResult(bytes32,address,bytes,bool)')
APPROVED_CANON=$(jq --arg r "$MESSAGE" '[.[]|select(.topics[1]==$r)|select(.data[66:130]|test("1$"))]|length' <<< "$ORES")
DENIED_CANON=$(jq --arg r "$MESSAGE" '[.[]|select(.topics[1]==$r)|select(.data[66:130]|test("0$"))]|length' <<< "$ORES")
ATTESTED_EVENTS=$(fetch_logs "$ANVIL_RPC_URL" "$CONSENSUS_ADDR" 'TransactionAttested(bytes32,bytes32,address,uint64,bytes32,bytes32,((uint256,uint256),uint256))' | jq 'length')

echo
echo "=== RESULT ==="
echo "canonical attestation id : $CANON_ATTEST"
echo "canonical approved=true  results for request: $APPROVED_CANON"
echo "canonical approved=false results for request: $DENIED_CANON"
echo "canonical TransactionAttested events        : $ATTESTED_EVENTS"

if [ "$CANON_ATTEST" != "0x0000000000000000000000000000000000000000000000000000000000000000" ] \
   && [ "$DENIED_CANON" -ge 1 ] && [ "$APPROVED_CANON" -eq 0 ]; then
    EXIT_MESSAGE="REPRODUCED: canonical chain attested a request whose only oracle result is approved=false."
    exit 0
fi
EXIT_MESSAGE="NOT REPRODUCED: attest=$CANON_ATTEST approved=$APPROVED_CANON denied=$DENIED_CANON"
exit 1
