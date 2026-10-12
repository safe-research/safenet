#!/bin/bash
# run 3 (rv-core-a, 2026-10-08, audit commit 361aaf3): executes the F2-VAL-066 sketch
# (../SKETCH_unexecuted.sh) for F-CORE-030. It is scripts/run_validator_deep_reorg_test.sh with
# three additions: the validator's exit status is collected with `wait`, `/health` is queried
# while the validator runs and after the reorg, and logs go to an output directory outside the
# repository. The trigger is the same: a reorg deeper than max_reorg_depth while running, which
# makes the validator take its deliberate ExceededMaxReorgDepth exit.
#
# Run inside a private network namespace (fixed ports), from a worktree with the env sourced:
#   integ bash rust-audit/poc/F2-VAL-066/run3/rv-core-a-deep_reorg_exit_status.sh OUTDIR
# Requirements: anvil, forge, cast, jq, cargo, curl.
set -euo pipefail

: "${WT:?source env/rv-core-a.sh first (WT is the worktree root)}"
OUT="${1:?output directory}"
mkdir -p "$OUT"

ANVIL_PORT=8549
ANVIL_RPC_URL="http://127.0.0.1:$ANVIL_PORT"
CHAIN_ID=31337
BLOCK_TIME=1
MAX_REORG_DEPTH=2
REORG_DEPTH=5
TIMEOUT=30
METRICS="127.0.0.1:9464"

PARTICIPANTS=(
    0x70997970C51812dc3A010C7d01b50e0d17dc79C8
    0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC
)
PRIVATE_KEY=0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d
SENDER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266

source "$WT/scripts/lib/shared_test_scripts.sh"

TMPDIR="$(mktemp -d)"
PIDS=()

require_commands anvil cast forge jq cargo curl
install_cleanup_trap

build_services_and_contracts
start_anvil "$BLOCK_TIME" "$ANVIL_PORT" "$OUT/anvil_logs.txt" "$ANVIL_RPC_URL"

PARTICIPANTS_CSV=$(IFS=,; echo "${PARTICIPANTS[*]}")
deploy_validator_contracts "$ANVIL_RPC_URL" "$SENDER" "$PARTICIPANTS_CSV" "$CHAIN_ID"

VALIDATOR_DB="$TMPDIR/validator.sqlite"
VALIDATOR_CONFIG="$TMPDIR/validator.toml"
{
    print_validator_config_base \
        "$ANVIL_RPC_URL" "$PRIVATE_KEY" "$VALIDATOR_DB" "$CONSENSUS_ADDR" "$ORACLE_ADDR" \
        1000000 "$((BLOCK_TIME * 1000))" PARTICIPANTS |
        sed "s|^log_filter = .*|log_filter = \"info\"\nmetrics_address = \"$METRICS\"|"
    echo "max_reorg_depth = $MAX_REORG_DEPTH"
} > "$VALIDATOR_CONFIG"
grep -n '^rpc\|metrics_address\|max_reorg_depth' "$VALIDATOR_CONFIG"

LOG="$OUT/validator_logs.txt"
run_rust_process validator "$VALIDATOR_CONFIG" "$LOG"
VALIDATOR_PID="$LAST_PID"
echo "validator pid $VALIDATOR_PID"

sleep "$((BLOCK_TIME * (MAX_REORG_DEPTH + 2)))"
assert_processes_alive "FAILURE: the validator exited before the reorg was issued." "$VALIDATOR_PID"
echo "health while indexing: $(curl -s "http://$METRICS/health" || echo '(no answer)')"

echo "reorging $REORG_DEPTH blocks, deeper than max_reorg_depth ($MAX_REORG_DEPTH)"
cast rpc anvil_reorg "$REORG_DEPTH" '[]' --rpc-url "$ANVIL_RPC_URL" >/dev/null

DEADLINE=$((SECONDS + TIMEOUT))
while [ "$SECONDS" -lt "$DEADLINE" ] && kill -0 "$VALIDATOR_PID" 2>/dev/null; do
    echo "health while the validator processes the reorg: $(curl -s "http://$METRICS/health" || echo '(no answer)')"
    sleep "$BLOCK_TIME"
done

set +e
wait "$VALIDATOR_PID"
STATUS=$?
set -e
echo "validator exit status: $STATUS"
echo "ExceededMaxReorgDepth lines in the log: $(grep -c ExceededMaxReorgDepth "$LOG")"
grep -o '"message":"unrecoverable[^"]*"' "$LOG" || true

if [ "$STATUS" -eq 0 ] && grep -q ExceededMaxReorgDepth "$LOG"; then
    EXIT_MESSAGE="REPRODUCED: the validator logged ExceededMaxReorgDepth and exited with status 0"
    exit 0
fi
EXIT_MESSAGE="NOT REPRODUCED: exit status $STATUS"
exit 1
