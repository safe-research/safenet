#!/usr/bin/env bash
#
# Verifies the contracts deployed by `just deployment_batch` (FROSTCoordinator, Consensus, the fee
# token if one was deployed, and SentinelOracle) via `forge verify-contract`. Uses Etherscan when
# ETHERSCAN_KEY is set in the env file, otherwise falls back to Sourcify (forge's own default
# verifier) — no API key required either way.
#
# Run this only after the Safe has actually executed the batch on-chain: verification compares
# against already-deployed bytecode, so it fails if the addresses predicted by
# `just deployment_batch` haven't been deployed to yet.
#
# Usage:
#   scripts/verify_deployment.sh [env-file] [addresses-file]
#
# env-file defaults to contracts/.env.testnet (see contracts/.env.testnet.sample) and supplies
# RPC_URL, ETHERSCAN_KEY, SAFE_ADDRESS, and the SENTINEL_* SentinelOracle config.
#
# addresses-file defaults to contracts/build/safenet-deployment.addresses.env, written alongside
# the batch by `just deployment_batch` — run that first. It supplies everything the env file
# doesn't: CHAIN_ID, COORDINATOR, GROUP_ID, SENTINEL_CONSENSUS, SENTINEL_FEE_TOKEN,
# and SENTINEL_ORACLE.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CONTRACTS_DIR="$ROOT/contracts"
ENV_FILE="${1:-$CONTRACTS_DIR/.env.testnet}"
ADDRESSES_FILE="${2:-$CONTRACTS_DIR/build/safenet-deployment.addresses.env}"

if [[ ! -f "$ENV_FILE" ]]; then
    echo "error: env file not found: $ENV_FILE" >&2
    exit 1
fi
if [[ ! -f "$ADDRESSES_FILE" ]]; then
    echo "error: addresses file not found: $ADDRESSES_FILE (run \`just deployment_batch\` first)" >&2
    exit 1
fi

set -a
# shellcheck disable=SC1090
source "$ENV_FILE"
# shellcheck disable=SC1090
source "$ADDRESSES_FILE"
set +a

if [ -z "${RPC_URL:-}" ]; then
    echo "RPC_URL must be set in $ENV_FILE" >&2
    exit 1
fi

VERIFIER_ARGS=(--verifier sourcify)
if [ -n "${ETHERSCAN_KEY:-}" ]; then
    VERIFIER_ARGS=(--verifier etherscan --etherscan-api-key "$ETHERSCAN_KEY")
fi

verify() {
    local address="$1" contract="$2"
    shift 2
    echo "Verifying $contract at $address..." >&2
    (cd "$CONTRACTS_DIR" && forge verify-contract "$address" "$contract" \
        --chain "$CHAIN_ID" --watch "${VERIFIER_ARGS[@]}" "$@")
}

verify "$COORDINATOR" src/FROSTCoordinator.sol:FROSTCoordinator

verify "$SENTINEL_CONSENSUS" src/Consensus.sol:Consensus \
    --constructor-args "$(cast abi-encode "constructor(address,bytes32)" "$COORDINATOR" "$GROUP_ID")"

verify "$SENTINEL_FEE_TOKEN" script/util/MyToken.sol:MyToken \
    --constructor-args "$(cast abi-encode "constructor(address)" "$SAFE_ADDRESS")"

verify "$SENTINEL_ORACLE" src/SentinelOracle.sol:SentinelOracle \
    --constructor-args "$(cast abi-encode \
        "constructor((address,address,address,address,address,uint96,uint32,uint32,uint24,uint32,uint32,uint32,uint32,string))" \
        "($SENTINEL_ARBITRATOR,$SENTINEL_GOVERNANCE,$SENTINEL_PROTOCOL_FUNDS_RECEIVER,$SENTINEL_CONSENSUS,$SENTINEL_FEE_TOKEN,$SENTINEL_REQUEST_FEE,$SENTINEL_BOND_MULTIPLIER,$SENTINEL_INITIAL_SLASHING_MULTIPLIER,$SENTINEL_INITIAL_DAO_FEE_SHARE,$SENTINEL_COMMIT_WINDOW,$SENTINEL_REVEAL_WINDOW,$SENTINEL_GOVERNANCE_DELAY,$SENTINEL_ARBITRATION_TIMEOUT,\"$SENTINEL_CHARTER_ENS\")")"
