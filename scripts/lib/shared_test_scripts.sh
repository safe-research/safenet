# Shared helpers for bash Anvil integration tests driving Rust binaries built
# from this repository (currently just `validator`) against the Solidity
# contracts.
#
# Meant to be `source`d, not executed. Callers are expected to have already
# set `set -euo pipefail` and to maintain two globals themselves:
#   PIDS      - array of background PIDs to kill on exit (Anvil, validators, ...)
#   TMPDIR    - a directory to remove on exit
# `install_cleanup_trap` wires both into an EXIT trap and also owns
# `EXIT_MESSAGE`, printed on exit to report success/failure/timeout.
#
# `REPO_ROOT` is computed once below, from this file's own location, rather
# than threaded through every function as a parameter - there is no case
# where a caller wants a repository other than the one it's running from.
_LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$_LIB_DIR/../.." && pwd)"
unset _LIB_DIR

# Fails fast with a clear message if any of the given commands are missing.
require_commands() {
    local command
    for command in "$@"; do
        command -v "$command" >/dev/null || {
            echo "Missing required command: $command" >&2
            exit 1
        }
    done
}

# Installs the standard EXIT trap: kills every PID in `PIDS`, removes `TMPDIR`,
# and prints `EXIT_MESSAGE` - which callers update as the test progresses and
# leave at its default to report an unexpected interruption.
install_cleanup_trap() {
    EXIT_MESSAGE="FAILURE: interrupted"
    cleanup() {
        local pid
        for pid in "${PIDS[@]:-}"; do
            kill "$pid" 2>/dev/null || true
        done
        rm -rf "$TMPDIR"
        echo "$EXIT_MESSAGE"
    }
    trap cleanup EXIT
}

# Prints the highest `blockNumber` among the JSON log array on stdin, each a
# 0x-prefixed hex string (as returned by `cast logs --json`).
max_block() {
    local max=0 hex dec
    while read -r hex; do
        [ -z "$hex" ] && continue
        dec="$((16#${hex#0x}))"
        [ "$dec" -gt "$max" ] && max=$dec
    done
    echo "$max"
}

# Fetches every log for `event_sig` emitted by `address` from genesis to the
# latest block, as a JSON array.
fetch_logs() {
    local rpc_url=$1 address=$2 event_sig=$3
    cast logs --json \
        --rpc-url "$rpc_url" \
        --from-block 0 --to-block latest \
        --address "$address" \
        "$event_sig"
}

# Starts Anvil in the background (interval-mined at `block_time` seconds,
# listening on `port`), appends its PID to `PIDS`, and blocks until it
# accepts RPC requests at `rpc_url`.
start_anvil() {
    local block_time=$1 port=$2 log_file=$3 rpc_url=$4 attempt
    anvil --block-time "$block_time" --port "$port" > "$log_file" 2>&1 &
    PIDS+=("$!")
    for attempt in $(seq 1 20); do
        cast block-number --rpc-url "$rpc_url" >/dev/null 2>&1 && return 0
        sleep 0.25
    done
    cast block-number --rpc-url "$rpc_url" >/dev/null
}

# Sets `EXIT_MESSAGE` and exits 1 if any of the given PIDs is no longer alive.
assert_processes_alive() {
    local message=$1
    shift
    local pid
    for pid in "$@"; do
        if ! kill -0 "$pid" 2>/dev/null; then
            EXIT_MESSAGE="$message"
            exit 1
        fi
    done
}

# Builds the Solidity contracts and the Rust service binaries integration
# tests run against.
build_services_and_contracts() {
    echo "==> Building Rust services..."
    cargo build --manifest-path "$REPO_ROOT/Cargo.toml"

    echo "==> Building Solidity contracts..."
    forge build --root "$REPO_ROOT/contracts" --force
}

# Deploys the validator contracts (Coordinator, Consensus, and the
# always-approving oracle) via `DeployScript`. Sets `COORDINATOR_ADDR`,
# `CONSENSUS_ADDR`, and `ORACLE_ADDR`.
deploy_validator_contracts() {
    local rpc_url=$1 sender=$2 participants_csv=$3 chain_id=$4

    echo "==> Deploying contracts..."
    env PARTICIPANTS="$participants_csv" \
        forge script --root "$REPO_ROOT/contracts" DeployScript \
        --rpc-url "$rpc_url" \
        --unlocked \
        --sender "$sender" \
        --broadcast

    local deploy_json="$REPO_ROOT/contracts/build/broadcast/Deploy.s.sol/$chain_id/run-latest.json"
    COORDINATOR_ADDR=$(jq -er '.returns.coordinator.value' "$deploy_json")
    CONSENSUS_ADDR=$(jq -er '.returns.consensus.value' "$deploy_json")
    ORACLE_ADDR=$(jq -er '.returns.alwaysApproveOracle.value' "$deploy_json")
    echo "    coordinator: $COORDINATOR_ADDR"
    echo "    consensus:   $CONSENSUS_ADDR"
    echo "    oracle:      $ORACLE_ADDR"
}

# Prints the common prefix of a validator TOML config to stdout: connection,
# signer, database, the `[validator]` table (consensus, blocks_per_epoch,
# oracles, and one `[[validator.participants]]` entry per address in the
# `participants_array_name` array), observability, and the `[index]` table's
# `block_time`/`start_block`. Callers append any config specific to their own
# test (e.g. `max_reorg_depth`) after calling this.
print_validator_config_base() {
    local rpc_url=$1 signer=$2 database=$3 consensus_addr=$4 oracle_addr=$5
    local blocks_per_epoch=$6 block_time_ms=$7 participants_array_name=$8
    local -n participants="$participants_array_name"

    echo "rpc = \"$rpc_url\""
    echo "signer = \"$signer\""
    echo "database = \"sqlite://$database?mode=rwc\""
    echo
    echo "[validator]"
    echo "consensus = \"$consensus_addr\""
    echo "blocks_per_epoch = $blocks_per_epoch"
    echo "oracles = [\"$oracle_addr\"]"
    local address
    for address in "${participants[@]}"; do
        echo
        echo "[[validator.participants]]"
        echo "address = \"$address\""
    done
    echo
    echo "[observability]"
    echo 'log_filter = "info,safenet_core=trace,validator=trace"'
    echo
    echo "[index]"
    echo "block_time = $block_time_ms"
    echo "start_block = 0"
}

# Starts the Rust binary built for `service` (e.g. "validator") against
# `config_file`, redirecting output to `log_file` (truncating it first unless
# `mode` is "append"). Appends its PID to `PIDS` and sets `LAST_PID`.
run_rust_process() {
    local service=$1 config_file=$2 log_file=$3 mode=${4:-truncate}
    if [ "$mode" = "truncate" ]; then
        : > "$log_file"
    fi
    "$REPO_ROOT/target/debug/$service" --config-file "$config_file" >> "$log_file" 2>&1 &
    PIDS+=("$!")
    LAST_PID="${PIDS[-1]}"
}

# Triggers the genesis key generation via `GenesisScript`.
trigger_genesis_keygen() {
    local rpc_url=$1 sender=$2 participants_csv=$3 coordinator_addr=$4

    echo "==> Triggering genesis KeyGen..."
    env PARTICIPANTS="$participants_csv" \
        COORDINATOR_ADDRESS="$coordinator_addr" \
        forge script --root "$REPO_ROOT/contracts" GenesisScript \
        --rpc-url "$rpc_url" \
        --unlocked \
        --sender "$sender" \
        --broadcast
}
