#!/usr/bin/env bash

set -euo pipefail

ROOT="$(dirname "$0")/.."
# All addresses/private keys below are Anvil's standard test-mnemonic accounts
# (i.e. what `anvil` derives with no custom `--mnemonic`/`--seed`), one per
# role, none reused across roles:
#   (0) deployer   0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
#   (1) alice      0x70997970C51812dc3A010C7d01b50e0d17dc79C8
#   (2) bob        0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC
#   (3) carol      0x90F79bf6EB2c4f870365E785982E1f101E93b906
#   (4) dave       0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65
#   (5) operator   0x9965507D1a55bcC2695C58ba16FB37d819B0A4dc
VALIDATORS=(
    alice:0x70997970C51812dc3A010C7d01b50e0d17dc79C8:0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d
    bob:0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC:0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a
)
# The configured sentinels as `name:private-key:engine-port`.
SENTINELS=(
    carol:0x90F79bf6EB2c4f870365E785982E1f101E93b906:0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6:5473
    dave:0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65:0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a:5474
)
# Anvil account (5). Only ever used via `--unlocked` impersonation
# (cast_send), so unlike the accounts above its private key is never
# referenced by this script — recorded here anyway in case it's ever needed
# for a manual `cast`/wallet import:
# 0x8b3a350cf5c34c9194ca85829a2df0ec3153be0318b5e2d3348e872092edffba
OPERATOR=0x9965507D1a55bcC2695C58ba16FB37d819B0A4dc
# Anvil account (0), also used as the `--sender`/`--from` for every
# broadcast/cast call below (all Anvil accounts are unlocked on the devnet).
# Private key: 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
DEPLOYER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266

# SentinelOracle economics (see the epic's Architecture Decision): a
# 40-cent request fee and a 2x bond multiplier, both read against MyToken's
# 18-decimal default as a dollar-pegged, USDS-style stablecoin, so the bond
# target per commitment (fee * multiplier, SentinelOracle.sol's
# `bondTarget`) is 80 cents.
SENTINEL_REQUEST_FEE=400000000000000000
SENTINEL_COMMIT_WINDOW=5
SENTINEL_REVEAL_WINDOW=5
SENTINEL_GOVERNANCE_DELAY=0
SENTINEL_BOND_MULTIPLIER=2
# Equal to the bond multiplier: a losing/never-revealed bond is slashed in
# full by default, matching pre-governed-slashing behavior.
SENTINEL_INITIAL_SLASHING_MULTIPLIER=2
# 0%: no DAO cut by default on devnet.
SENTINEL_INITIAL_DAO_FEE_SHARE=0
# ENS name of the Charter this Oracle trusts, stored on-chain as a
# human-readable domain (not its namehash) so it can be read directly by
# anyone inspecting the contract.
SENTINEL_CHARTER_ENS="safenet-charter.safe.eth"
# A frozen (conflicting-votes) request waits this many blocks for the
# arbitrator before anyone can permissionlessly time it out and refund
# everyone in full.
SENTINEL_ARBITRATION_TIMEOUT=100
# $10 of the fee token per sentinel: comfortably above the 80-cent bond
# target across more than one request.
SENTINEL_FUNDING_TOKEN=10000000000000000000

# Generous amounts for `--fund-account`: plenty of ETH for gas (an arbitrary
# address, unlike Anvil's own dev accounts, starts out with none) and plenty
# of the fee token to submit many oracle requests.
FUND_ACCOUNT_ETH=10000000000000000000
FUND_ACCOUNT_TOKEN=1000000000000000000000

# ---- Utility functions ----

usage() {
    cat <<EOF
Run a local Safenet development network, with Rust validators and sentinels
backed by sentinel engines voting on a SentinelOracle.

USAGE
    run_devnet.sh [OPTIONS...]

OPTIONS
    -h, --help                  Print this help message.
    --build                     Build the contracts, validator, sentinel, and sentinel engine Podman images.
    --port <PORT>               Specify an alternate host port for the Ethereum RPC.
    --block-time <SECS>         The block time in seconds for the devnet.
    --blocks-per-epoch <NUM>    The number of blocks per Safenet epoch.
    --no-genesis                Do not kick off genesis.
    --clean-configs             Remove leftover validator config directories from
                                 previous runs and exit. Only safe once their pods
                                 have been torn down (e.g. \`podman pod rm -f safenet\`),
                                 since a still-running pod has its config files
                                 mounted from one of these directories.
    --fund-account <ADDRESS>    Fund an additional account with ETH and fee tokens.
EOF
    exit 0
}

fail() {
    echo "ERROR: $1." 1>&2
    exit 1
}

# Parses a `console.log("<label>:", address(...))` line out of a forge
# script's output (e.g. "Consensus: 0x...", "ERC20 deployed at: 0x...").
parse_address() {
    echo "$1" | grep "$2:" | grep -oE '0x[0-9a-fA-F]{40}'
}

forge_script() {
    # We run the Forge scripts that are included in the `contracts`
    # container where the node is already running. Extra arguments (e.g.
    # `-e SOME_VAR=value`) are forwarded to `podman exec`, for scripts that
    # need additional environment variables beyond `PARTICIPANTS`.
    local script=$1
    shift
    podman exec -e PARTICIPANTS=$participants_cs "$@" safenet-node \
        forge script $script \
            --rpc-url http://localhost:8545 \
            --unlocked \
            --sender $DEPLOYER \
            --broadcast
}

simulate_forge_script() {
    # Dry-runs a Forge script (no `--rpc-url`/`--broadcast`) to
    # deterministically precompute its CREATE2 deployment address before the
    # devnet's Anvil node exists yet — needed for the TOML configs generated
    # below, which are mounted into containers that start reading them the
    # moment the pod comes up. Every deployment this script cares about goes
    # through `DeterministicDeployment`'s CREATE2 factory
    # (`contracts/script/util/DeterministicDeployment.sol`), whose address
    # only depends on the factory, salt and (for `MyToken`/`SentinelOracle`)
    # sender-derived constructor arguments — so passing the same `$DEPLOYER`
    # sender as the real, broadcast deployment further down below makes the
    # simulated address always match the real one.
    local script=$1
    shift
    podman run --rm -e PARTICIPANTS=$participants_cs "$@" localhost/safenet-contracts \
        "forge script $script --sender $DEPLOYER"
}

cast_send() {
    # First argument is the (unlocked, impersonated) sender address.
    local from=$1
    shift
    podman exec safenet-node \
        cast send --rpc-url http://localhost:8545 --unlocked --from "$from" "$@" >/dev/null
}

safenet_spec() {
    cat <<EOF
apiVersion: v1
kind: Pod

metadata:
  name: safenet

spec:
  containers:
    - name: node
      image: localhost/safenet-contracts:latest
      args:
        - anvil --host=0.0.0.0 --block-time=${block_time}
      ports:
        - containerPort: 8545
          hostPort: ${port}
EOF

    for validator in "${VALIDATORS[@]}"; do
        parts=(${validator//:/ })
        name=${parts[0]}

        cat <<EOF
    - name: validator-${name}
      image: localhost/safenet-validator:latest
      args:
        - --config-file
        - /config/validator.toml
      volumeMounts:
        - name: config-${name}
          mountPath: /config/validator.toml
EOF
    done

    for sentinel in "${SENTINELS[@]}"; do
        parts=(${sentinel//:/ })
        name=${parts[0]}

        cat <<EOF
    - name: sentinel-engine-${name}
      image: localhost/safenet-sentinel-engine:latest
      args:
        - --config-file
        - /config/sentinel-engine.toml
      volumeMounts:
        - name: config-sentinel-engine-${name}
          mountPath: /config/sentinel-engine.toml
    - name: sentinel-${name}
      image: localhost/safenet-sentinel:latest
      args:
        - --config-file
        - /config/sentinel.toml
      volumeMounts:
        - name: config-${name}
          mountPath: /config/sentinel.toml
EOF
    done

    cat <<EOF
  volumes:
EOF

    for validator in "${VALIDATORS[@]}"; do
        parts=(${validator//:/ })
        name=${parts[0]}

        cat <<EOF
    - name: config-${name}
      hostPath:
        path: ${config_dir}/${name}.toml
        type: File
EOF
    done

    for sentinel in "${SENTINELS[@]}"; do
        parts=(${sentinel//:/ })
        name=${parts[0]}

        cat <<EOF
    - name: config-${name}
      hostPath:
        path: ${config_dir}/${name}.toml
        type: File
    - name: config-sentinel-engine-${name}
      hostPath:
        path: ${config_dir}/${name}-engine.toml
        type: File
EOF
    done
}

# ---- Main script ----

build=no
port=8545
block_time=5
blocks_per_epoch=60
genesis=yes
clean_configs=no
fund_account=
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            usage ;;
        --build)
            build=yes ;;
        --port)
            port="$2"; shift ;;
        --block-time)
            block_time="$2"; shift ;;
        --blocks-per-epoch)
            blocks_per_epoch="$2"; shift ;;
        --no-genesis)
            genesis=no ;;
        --clean-configs)
            clean_configs=yes ;;
        --fund-account)
            fund_account="$2"; shift ;;
        *)
            fail "unexpected argument '$1'" ;;
    esac
    shift
done

# Config directories created below are named `safenet-devnet.<random>` so
# they can be identified and swept up here; each is left behind on exit (see
# the comment below `config_dir`'s assignment).
if [ $clean_configs == yes ]; then
    shopt -s nullglob
    stale=("${TMPDIR:-/tmp}"/safenet-devnet.*)
    shopt -u nullglob
    if [ ${#stale[@]} -eq 0 ]; then
        echo "No leftover devnet config directories found."
    else
        echo "Removing leftover devnet config directories:"
        printf '  %s\n' "${stale[@]}"
        rm -rf "${stale[@]}"
    fi
    exit 0
fi

# Rust validators are configured entirely through a per-instance TOML file
# (`--config-file`), rather than environment variables. Generate one file per
# validator into a temporary directory that is bind-mounted into each
# container. This script exits (once genesis is triggered) long before the
# pod it started does, so `config_dir` must NOT be cleaned up here: doing so
# on script exit would delete the mounted files out from under the still-
# running containers. It is intentionally left behind (see `--clean-configs`
# above), the same way the pod itself is left running for the caller to tear
# down manually.
config_dir="$(mktemp -d "${TMPDIR:-/tmp}/safenet-devnet.XXXXXXXX")"

# For now, we require `podman`. We specifically make use of pods and
# the `play` feature in order to bring up the devnet.
if ! command -v podman &>/dev/null; then
    fail "could not find required command 'podman'"
fi

# Build the container images if requested.
if [ $build == yes ]; then
    podman build -t localhost/safenet-contracts -f "$ROOT/contracts/Dockerfile" "$ROOT"
    podman build -t localhost/safenet-validator -f "$ROOT/crates/validator/Dockerfile" "$ROOT"
    podman build -t localhost/safenet-sentinel -f "$ROOT/crates/sentinel/Dockerfile" "$ROOT"
    podman build -t localhost/safenet-sentinel-engine -f "$ROOT/crates/sentinel-engine/Dockerfile" "$ROOT"
fi

# Compute the participant set based on our configuration. We want to
# extract the address of each of the validators.
participants=()
for validator in "${VALIDATORS[@]}"; do
    parts=(${validator//:/ })
    participants+=(${parts[1]})
done
participants_cs=$(IFS=, ; echo "${participants[*]}")

# TODO: In the future, we should consider bundling the contract
# bytecode with the `validator`/`sentinel` binaries, allowing them to compute
# default contract addresses based on other inputs and using deterministic
# deployments. For now, simulate the deployments with our `contracts` image
# and parse out the resulting addresses (see `simulate_forge_script`). Both
# `DeployERC20Script` and `DeploySentinelOracleScript` select the
# `CANONICAL` CREATE2 factory (`FACTORY=2`, matching
# `run_sentinel_integration_test.sh`): the `SAFE_SINGLETON_FACTORY` that
# `getFactory()` otherwise defaults to isn't deployed on a bare Anvil node.
deployment="$(simulate_forge_script DeployScript)"
consensus="$(parse_address "$deployment" Consensus)"
fee_token="$(parse_address "$(simulate_forge_script DeployERC20Script -e FACTORY=2)" 'ERC20 deployed at')"
sentinel_oracle="$(parse_address "$(simulate_forge_script DeploySentinelOracleScript \
    -e FACTORY=2 \
    -e SENTINEL_ARBITRATOR="$OPERATOR" \
    -e SENTINEL_GOVERNANCE="$OPERATOR" \
    -e SENTINEL_PROTOCOL_FUNDS_RECEIVER="$OPERATOR" \
    -e SENTINEL_CONSENSUS="$consensus" \
    -e SENTINEL_FEE_TOKEN="$fee_token" \
    -e SENTINEL_REQUEST_FEE="$SENTINEL_REQUEST_FEE" \
    -e SENTINEL_COMMIT_WINDOW="$SENTINEL_COMMIT_WINDOW" \
    -e SENTINEL_REVEAL_WINDOW="$SENTINEL_REVEAL_WINDOW" \
    -e SENTINEL_GOVERNANCE_DELAY="$SENTINEL_GOVERNANCE_DELAY" \
    -e SENTINEL_BOND_MULTIPLIER="$SENTINEL_BOND_MULTIPLIER" \
    -e SENTINEL_INITIAL_SLASHING_MULTIPLIER="$SENTINEL_INITIAL_SLASHING_MULTIPLIER" \
    -e SENTINEL_INITIAL_DAO_FEE_SHARE="$SENTINEL_INITIAL_DAO_FEE_SHARE" \
    -e SENTINEL_CHARTER_ENS="$SENTINEL_CHARTER_ENS" \
    -e SENTINEL_ARBITRATION_TIMEOUT="$SENTINEL_ARBITRATION_TIMEOUT")" 'SentinelOracle deployed at')"

# Write each validator's TOML config into `$config_dir`, following the shape
# established by `run_validator_integration_test.sh`'s `validator_config()`
# heredoc. `oracles` points validators at the SentinelOracle above, so they
# honor its attestations on oracle-checked transactions.
for validator in "${VALIDATORS[@]}"; do
    parts=(${validator//:/ })
    name=${parts[0]}
    private_key=${parts[2]}

    {
        echo "rpc = \"http://localhost:8545\""
        echo "signer = \"${private_key}\""
        echo "database = \"sqlite::memory:\""
        echo
        echo "[validator]"
        echo "consensus = \"${consensus}\""
        echo "blocks_per_epoch = ${blocks_per_epoch}"
        echo "oracles = [\"${sentinel_oracle}\"]"
        for address in "${participants[@]}"; do
            echo
            echo "[[validator.participants]]"
            echo "address = \"${address}\""
        done
        echo
        echo "[observability]"
        echo 'log_filter = "trace"'
        echo
        echo "[index]"
        echo "block_time = $((block_time * 1000))"
    } > "$config_dir/${name}.toml"
done

# Write each sentinel and its engine's TOML config into `$config_dir`,
# following the shapes established by `run_sentinel_integration_test.sh`'s
# configuration helpers.
for sentinel in "${SENTINELS[@]}"; do
    parts=(${sentinel//:/ })
    name=${parts[0]}
    private_key=${parts[2]}
    engine_port=${parts[3]}

    {
        echo "rpc = \"http://localhost:8545\""
        echo "bind_address = \"0.0.0.0:${engine_port}\""
        echo
        echo "[engine]"
        echo "address_poisoning_lookback_blocks = 1000"
    } > "$config_dir/${name}-engine.toml"

    {
        echo "rpc = \"http://localhost:8545\""
        echo "signer = \"${private_key}\""
        echo "database = \"sqlite::memory:\""
        echo "oracle = \"${sentinel_oracle}\""
        echo "consensus = \"${consensus}\""
        echo
        echo "[sentinel]"
        echo "fee_token = \"${fee_token}\""
        echo "voting_window = 1"
        echo "blocklist = []"
        echo "engine = \"http://localhost:${engine_port}\""
        echo
        echo "[observability]"
        echo 'log_filter = "trace"'
        echo
        echo "[index]"
        echo "block_time = $((block_time * 1000))"
    } > "$config_dir/${name}.toml"
done

# Create a pod with a fully functional Safenet development network
# from our generated spec.
safenet_spec | podman kube play -

# Deploy the Safenet contracts.
forge_script DeployScript

# Deploy the sentinel fee token and a SentinelOracle whose arbitrator,
# governance, and protocol funds receiver are all $OPERATOR, then register
# and fund each SENTINELS entry against it. These reuse the exact same
# arguments as `simulate_forge_script` above, so the addresses actually
# deployed here match `$fee_token`/`$sentinel_oracle`.
forge_script DeployERC20Script -e FACTORY=2 >/dev/null
forge_script DeploySentinelOracleScript \
    -e FACTORY=2 \
    -e SENTINEL_ARBITRATOR="$OPERATOR" \
    -e SENTINEL_GOVERNANCE="$OPERATOR" \
    -e SENTINEL_PROTOCOL_FUNDS_RECEIVER="$OPERATOR" \
    -e SENTINEL_CONSENSUS="$consensus" \
    -e SENTINEL_FEE_TOKEN="$fee_token" \
    -e SENTINEL_REQUEST_FEE="$SENTINEL_REQUEST_FEE" \
    -e SENTINEL_COMMIT_WINDOW="$SENTINEL_COMMIT_WINDOW" \
    -e SENTINEL_REVEAL_WINDOW="$SENTINEL_REVEAL_WINDOW" \
    -e SENTINEL_GOVERNANCE_DELAY="$SENTINEL_GOVERNANCE_DELAY" \
    -e SENTINEL_BOND_MULTIPLIER="$SENTINEL_BOND_MULTIPLIER" \
    -e SENTINEL_INITIAL_SLASHING_MULTIPLIER="$SENTINEL_INITIAL_SLASHING_MULTIPLIER" \
    -e SENTINEL_INITIAL_DAO_FEE_SHARE="$SENTINEL_INITIAL_DAO_FEE_SHARE" \
    -e SENTINEL_CHARTER_ENS="$SENTINEL_CHARTER_ENS" \
    -e SENTINEL_ARBITRATION_TIMEOUT="$SENTINEL_ARBITRATION_TIMEOUT" >/dev/null

for sentinel in "${SENTINELS[@]}"; do
    parts=(${sentinel//:/ })
    address=${parts[1]}

    cast_send "$OPERATOR" "$sentinel_oracle" 'addSentinel(address)' "$address"
    cast_send "$DEPLOYER" "$fee_token" 'transfer(address,uint256)' "$address" "$SENTINEL_FUNDING_TOKEN"
done

# Fund an additional, caller-supplied account with ETH (gas) and plenty of
# the fee token, if requested via `--fund-account`.
if [ -n "$fund_account" ]; then
    cast_send "$DEPLOYER" "$fund_account" --value "$FUND_ACCOUNT_ETH"
    cast_send "$DEPLOYER" "$fee_token" 'transfer(address,uint256)' "$fund_account" "$FUND_ACCOUNT_TOKEN"
fi

# Kick off genesis, if requested.
if [ $genesis == yes ]; then
    forge_script GenesisScript
fi
