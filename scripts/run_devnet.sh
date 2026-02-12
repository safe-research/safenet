#!/usr/bin/env bash

set -euo pipefail

ROOT="$(dirname "$0")/.."
VALIDATORS=(
    alice:0x70997970C51812dc3A010C7d01b50e0d17dc79C8:0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d
    bob:0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC:0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a
)

usage() {
    cat <<EOF
Run a local Safenet development network.

USAGE
    run_devnet.sh [OPTIONS...]

OPTIONS
    -h, --help                  Print this help message.
    --build                     Build the contracts and validator Podman images.
    --port <PORT>               Specify an alternate host port for the Ethereum RPC.
    --block-time <SECS>         The block time in seconds for the devnet.
    --blocks-per-epoch <NUM>    The number of blocks per Safenet epoch.
    --no-genesis                Do not kick off genesis.
EOF
    exit 0
}

fail() {
    echo "ERROR: $1." 1>&2
    exit 1
}

build=no
port=8545
block_time=5
blocks_per_epoch=60
genesis=yes
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
        *)
            fail "unexpected argument '$1'" ;;
    esac
    shift
done

# For now, we require `podman`. We specifically make use of pods and
# the `play` feature in order to bring up the devnet.
if ! command -v podman &>/dev/null; then
    fail "could not find required command 'podman'"
fi

# Build the container images if requested.
if [ $build == yes ]; then
    podman build -t localhost/safenet-contracts -f "$ROOT/contracts/Dockerfile" "$ROOT"
    podman build -t localhost/safenet-validator -f "$ROOT/validator/Dockerfile" "$ROOT"
fi

# Compute the participant set based on our configuration. We want to
# extract the address of each of the validators.
participants=()
for validator in "${VALIDATORS[@]}"; do
    parts=(${validator//:/ })
    participants+=(${parts[1]})
done
participants=$(IFS=, ; echo "${participants[*]}")

# TODO: In the future, we should consider bundling the contract
# bytecode with the `validator` binary, allowing it to compute default
# contract addresses based on other inputs and using deterministic
# deployments. For now, simulate a deployment with our `contracts`
# image and parse out the contract addresses.
deployment="$(podman run --rm -e PARTICIPANTS=$participants localhost/safenet-contracts 'forge script DeployScript')"
parse_deployment() {
    echo "$deployment" | grep "$1:" | grep -oE '0x[0-9a-fA-F]{40}'
}
coordinator="$(parse_deployment FROSTCoordinator)"
consensus="$(parse_deployment Consensus)"

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

    for validator in ${VALIDATORS[@]}; do
        parts=(${validator//:/ })
        name=${parts[0]}
        private_key=${parts[2]}

        cat <<EOF
    - name: validator-${name}
      image: localhost/safenet-validator:latest
      env:
        - name: LOG_LEVEL
          value: debug
        - name: METRICS_PORT
          value: 0
        - name: RPC_URL
          value: http://localhost:8545
        - name: CHAIN_ID
          value: 31337
        - name: BLOCK_TIME_OVERRIDE
          value: ${block_time}
        - name: CONSENSUS_ADDRESS
          value: ${consensus}
        - name: COORDINATOR_ADDRESS
          value: ${coordinator}
        - name: PARTICIPANTS
          value: ${participants}
        - name: BLOCKS_PER_EPOCH
          value: ${blocks_per_epoch}
        - name: PRIVATE_KEY
          value: ${private_key}
EOF
    done
}

# Create a pod with a fully functional Safenet development network
# from our generated spec.
safenet_spec | podman kube play -

forge_script() {
    # We run the Forge scripts that are included in the `contracts`
    # container where the node is already running.
    podman exec -e PARTICIPANTS=$participants safenet-node \
        forge script $1 \
            --rpc-url http://localhost:8545 \
            --unlocked \
            --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 \
            --broadcast
}

# Deploy the Safenet contracts.
forge_script DeployScript

# Kick off genesis, if requested.
if [ $genesis == yes ]; then
    forge_script GenesisScript
fi
