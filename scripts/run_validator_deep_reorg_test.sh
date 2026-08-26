#!/bin/bash
# Regression test for https://github.com/safe-research/safenet/issues/820: a
# reorg deeper than the configured `max_reorg_depth` must make the validator
# fail loudly instead of silently continuing to index an unverifiable chain.
#
# This is deliberately the simplest possible reproduction: a single Anvil node
# and a single Rust validator instance with a small `max_reorg_depth`. Once
# the validator has indexed a few blocks, the chain is reorged back further
# than that depth via `anvil_reorg`, and the test asserts the validator
# process exits rather than carrying on, logging the expected error.
#
# Requirements: anvil, forge, cast, jq, and cargo.
set -euo pipefail

ANVIL_PORT=8549
ANVIL_RPC_URL="${ANVIL_RPC_URL:-http://127.0.0.1:$ANVIL_PORT}"
CHAIN_ID=31337
BLOCK_TIME=1
MAX_REORG_DEPTH=2
REORG_DEPTH=5
TIMEOUT="${TIMEOUT:-30}"

# Anvil accounts 1 and 2. A genesis group needs at least two participants to
# be valid; only the first is actually run as a Rust process below - this
# test never triggers genesis key generation, so the second is never needed
# beyond satisfying that validity check.
PARTICIPANTS=(
    0x70997970C51812dc3A010C7d01b50e0d17dc79C8
    0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC
)
PRIVATE_KEY=0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d

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
start_anvil "$BLOCK_TIME" "$ANVIL_PORT" "$REPO_ROOT/anvil_logs.txt" "$ANVIL_RPC_URL"

PARTICIPANTS_CSV=$(IFS=,; echo "${PARTICIPANTS[*]}")
deploy_validator_contracts "$ANVIL_RPC_URL" "$SENDER" "$PARTICIPANTS_CSV" "$CHAIN_ID"

VALIDATOR_DB="$TMPDIR/validator.sqlite"
VALIDATOR_CONFIG="$TMPDIR/validator.toml"
{
    print_validator_config_base \
        "$ANVIL_RPC_URL" "$PRIVATE_KEY" "$VALIDATOR_DB" "$CONSENSUS_ADDR" "$ORACLE_ADDR" \
        1000000 "$((BLOCK_TIME * 1000))" PARTICIPANTS
    # Small and deliberately below REORG_DEPTH, so the reorg below exceeds it.
    echo "max_reorg_depth = $MAX_REORG_DEPTH"
} > "$VALIDATOR_CONFIG"

echo "==> Starting validator..."
run_rust_process validator "$VALIDATOR_CONFIG" "$REPO_ROOT/validator_logs.txt"
VALIDATOR_PID="$LAST_PID"
echo "    pid $VALIDATOR_PID"

sleep 0.5
assert_processes_alive "FAILURE: the validator exited during startup." "$VALIDATOR_PID"

echo "==> Waiting for the validator to index a few blocks before reorging..."
sleep "$((BLOCK_TIME * (MAX_REORG_DEPTH + 2)))"
assert_processes_alive "FAILURE: the validator exited before the reorg was issued." "$VALIDATOR_PID"

echo "==> Reorging $REORG_DEPTH block(s), deeper than max_reorg_depth ($MAX_REORG_DEPTH)..."
cast rpc anvil_reorg "$REORG_DEPTH" '[]' --rpc-url "$ANVIL_RPC_URL" >/dev/null

echo "==> Waiting for the validator to detect the reorg and fail loudly (timeout: ${TIMEOUT}s)..."
DEADLINE=$((SECONDS + TIMEOUT))
while [ "$SECONDS" -lt "$DEADLINE" ] && kill -0 "$VALIDATOR_PID" 2>/dev/null; do
    sleep "$BLOCK_TIME"
done

if kill -0 "$VALIDATOR_PID" 2>/dev/null; then
    EXIT_MESSAGE="FAILURE: the validator kept running after a reorg exceeding max_reorg_depth; it should have failed loudly."
    exit 1
fi

if ! grep -q "ExceededMaxReorgDepth" "$REPO_ROOT/validator_logs.txt"; then
    EXIT_MESSAGE="FAILURE: the validator exited, but its logs do not mention the expected max-reorg-depth error."
    exit 1
fi

EXIT_MESSAGE="SUCCESS: the validator failed loudly after a reorg exceeding the configured max_reorg_depth."
exit 0
