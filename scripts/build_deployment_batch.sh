#!/usr/bin/env bash
#
# Runs the "Deployment Steps" from the Safenet testnet deployment runbook as dry runs
# (no --broadcast, no private key) and assembles their to/data into a Safe Transaction Builder
# batch, meant to be reviewed and executed by the SAFE_ADDRESS Safe (e.g. by importing the output
# file into https://app.safe.global's Transaction Builder).
#
# This coordinates the existing contracts/script/*.s.sol front doors 1:1 with the runbook steps —
# it does not reimplement any of their deployment logic:
#   1. DeployConsensusScript    (coordinator via the CANONICAL CREATE2 factory, consensus via the
#                               FACTORY-selected one, same as steps 2 and 3)
#   2. DeployERC20Script        (fee token, skipped if SENTINEL_FEE_TOKEN is already set), plus a
#                               mint(...) to each SENTINEL_ADDRESSES entry when a fresh token is
#                               deployed, so sentinels can actually afford to post bonds
#   3. DeploySentinelOracleScript
#   4. addSentinel(...) for each address in SENTINEL_ADDRESSES
#
# Each forge script dry-run still needs a live --rpc-url (read-only) to see the target chain's
# deployed CREATE2 factory and to predict each contract's deterministic address; it never signs or
# submits anything.
#
# Besides the batch itself, this also writes a companion <output>.addresses.env file (e.g.
# contracts/build/safenet-deployment.addresses.env) with everything `just verify_deployment` needs
# afterwards (once the Safe has actually executed the batch) that isn't already in .env.testnet:
# the coordinator address and the genesis group ID.
#
# Usage:
#   cp contracts/.env.testnet.sample contracts/.env.testnet && $EDITOR contracts/.env.testnet
#   scripts/build_deployment_batch.sh [env-file] [output.json]
#
# env-file defaults to contracts/.env.testnet. On top of the runbook's own .env.testnet variables
# (FACTORY, PARTICIPANTS, GENESIS_SALT, SENTINEL_ARBITRATOR, etc. — see
# contracts/.env.testnet.sample; .env.testnet itself is gitignored since it ends up holding a
# private key), this also reads:
#   SAFE_ADDRESS          Required. The Safe that will execute the batch; used as the forge
#                         scripts' --sender so predicted addresses that depend on the caller (e.g.
#                         the fee token's owner) resolve to the Safe, not forge's default sender.
#   SENTINEL_ADDRESSES    Optional, comma-separated. Sentinel addresses to register on the newly
#                         deployed SentinelOracle via one `addSentinel` batch entry each, and (when
#                         a fresh fee token is deployed) to mint SENTINEL_MINT_AMOUNT to.
#   SENTINEL_MINT_AMOUNT  Optional, raw token units (18 decimals). Defaults to 100000 tokens.
#                         Ignored if SENTINEL_FEE_TOKEN is already set, since the batch may not own
#                         that token.
#
# SENTINEL_CONSENSUS/SENTINEL_FEE_TOKEN/SENTINEL_ORACLE do not need to be set by hand: each step's
# predicted address is fed into the next step automatically, the same way the runbook has you copy
# each step's console output into .env.testnet before running the next command.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CONTRACTS_DIR="$ROOT/contracts"
ENV_FILE="${1:-$CONTRACTS_DIR/.env.testnet}"
OUT_FILE="${2:-$CONTRACTS_DIR/build/safenet-deployment.json}"

if [[ ! -f "$ENV_FILE" ]]; then
    echo "error: env file not found: $ENV_FILE" >&2
    exit 1
fi

set -a
# shellcheck disable=SC1090
source "$ENV_FILE"
set +a

if [ -z "${RPC_URL:-}" ]; then
    echo "RPC_URL must be set in $ENV_FILE" >&2
    exit 1
fi
if [ -z "${SAFE_ADDRESS:-}" ]; then
    echo "SAFE_ADDRESS must be set (the Safe that will execute the batch)" >&2
    exit 1
fi

# 100000 tokens (18 decimals): comfortably above the bond a sentinel needs to post per request
# (bondTarget = fee * bondMultiplier, see SentinelOracle.postRequest), across many requests.
SENTINEL_MINT_AMOUNT="${SENTINEL_MINT_AMOUNT:-100000000000000000000000}"

CHAIN_ID="$(cast chain-id --rpc-url "$RPC_URL")"
TXS=()
IFS=',' read -ra SENTINELS <<< "${SENTINEL_ADDRESSES:-}"

# Runs a forge script dry run (no --broadcast, no private key) and prints its full console output,
# so callers capture it the normal way (`output="$(dry_run ...)"`). Also echoes that same output to
# this script's own stderr first, so it's still visible to whoever is running this script — all at
# once once forge finishes, rather than live, since it has to be captured before it can be returned.
dry_run() {
    local contract="$1"
    local output
    output="$(cd "$CONTRACTS_DIR" && forge script "$contract" --rpc-url "$RPC_URL" --sender "$SAFE_ADDRESS" 2>&1)"
    echo "$output" >&2
    echo "$output"
}

# Prints the path to a forge script's dry-run broadcast artifact, or nothing if it doesn't exist.
# It won't exist when the script recorded zero transactions, which happens when every CREATE2
# deploy's target address already has code (see DeterministicDeployment.deployWithArgs's
# `if (result.code.length == 0)` guard): forge only writes a broadcast artifact when there's at
# least one transaction to persist, e.g. when re-running this script against a chain where
# everything is already deployed, just to regenerate the addresses file for `just verify_deployment`.
artifact_path() {
    local path="$CONTRACTS_DIR/build/broadcast/$1.s.sol/$CHAIN_ID/dry-run/run-latest.json"
    [[ -f "$path" ]] && echo "$path"
    return 0
}

# Extracts a named return value from a captured `forge script` run's "== Return ==" block (e.g.
# "coordinator: contract FROSTCoordinator 0x2f88...") into the variable named by $1 — used instead
# of the broadcast artifact's `.returns` (see dry_run above) since forge always prints this,
# regardless of whether any transaction was recorded. Aborts loudly if the extracted value doesn't
# look like a hex value, rather than letting a blank/garbled value silently propagate into a later
# forge script as an unparseable environment variable. Assigns directly into $1 (rather than being
# called as `var="$(forge_return ...)"`) specifically so that abort can actually exit this script:
# command substitution runs in a subshell, where `exit` would only end the subshell.
forge_return() {
    local var="$1" output="$2" name="$3" value
    value="$(awk -v name="$name:" '$1 == name { print $NF }' <<< "$output")"
    if [[ "$value" != 0x* ]]; then
        echo "error: failed to extract $name from the forge script output above" >&2
        exit 1
    fi
    printf -v "$var" '%s' "$value"
}

# Appends every CALL/CREATE2 transaction from a dry-run artifact to TXS as a raw-data Safe Tx
# Builder entry. Plain CREATE transactions (to: null — none of these scripts produce one, but a
# Safe transaction has no way to express "deploy without a target" if one ever did) can't be
# represented as a Safe "to"+"data" call and are intentionally skipped. A blank artifact (see
# dry_run above) means zero transactions to add, which is correct: nothing needs to happen again
# for an already-deployed contract.
add_deploy_txs() {
    local artifact="$1" line
    [[ -z "$artifact" ]] && return
    while IFS= read -r line; do
        TXS+=("$line")
    done < <(jq -c '
        .transactions[]
        | select(.transactionType == "CALL" or .transactionType == "CREATE2")
        | {
            contractInputsValues: null,
            contractMethod: null,
            data: (.transaction.data // .transaction.input),
            to: .transaction.to,
            value: (
                if .transaction.value == "0x0" then "0"
                else error("non-zero value transaction not supported: " + .transaction.value)
                end
            )
          }
    ' "$artifact")
}

echo "--- 1. Deploy consensus (and coordinator) ---"

DEPLOY_OUTPUT="$(dry_run DeployConsensusScript)"
add_deploy_txs "$(artifact_path DeployConsensus)"
forge_return COORDINATOR "$DEPLOY_OUTPUT" coordinator
forge_return GROUP_ID "$DEPLOY_OUTPUT" groupId
# Make SENTINEL_CONSENSUS accessible for the deployment scripts
forge_return SENTINEL_CONSENSUS "$DEPLOY_OUTPUT" consensus
export SENTINEL_CONSENSUS

echo "--- 2. Deploy test token (skipped if SENTINEL_FEE_TOKEN is already set) ---"

if [[ -z "${SENTINEL_FEE_TOKEN:-}" ]]; then
    ERC20_OUTPUT="$(dry_run DeployERC20Script)"
    add_deploy_txs "$(artifact_path DeployERC20)"
    forge_return SENTINEL_FEE_TOKEN "$ERC20_OUTPUT" erc20
    export SENTINEL_FEE_TOKEN

    # Fund each sentinel with SENTINEL_MINT_AMOUNT so it can actually post bonds.
    for sentinel in "${SENTINELS[@]}"; do
        [[ -z "$sentinel" ]] && continue
        TXS+=("$(jq -cn --arg to "$SENTINEL_FEE_TOKEN" --arg sentinel "$sentinel" --arg amount "$SENTINEL_MINT_AMOUNT" '{
            contractInputsValues: {to: $sentinel, amount: $amount},
            contractMethod: {
                inputs: [
                    {internalType: "address", name: "to", type: "address"},
                    {internalType: "uint256", name: "amount", type: "uint256"}
                ],
                name: "mint",
                payable: false
            },
            data: null,
            to: $to,
            value: "0"
        }')")
    done
else
    echo "Using existing fee token address: $SENTINEL_FEE_TOKEN" >&2
fi

echo "--- 3. Deploy sentinel oracle ---"

ORACLE_OUTPUT="$(dry_run DeploySentinelOracleScript)"
add_deploy_txs "$(artifact_path DeploySentinelOracle)"
forge_return SENTINEL_ORACLE "$ORACLE_OUTPUT" sentinelOracle

echo "--- 4. Enable sentinel(s) on oracle ---"

for sentinel in "${SENTINELS[@]}"; do
    [[ -z "$sentinel" ]] && continue
    TXS+=("$(jq -cn --arg to "$SENTINEL_ORACLE" --arg sentinel "$sentinel" '{
        contractInputsValues: {sentinel: $sentinel},
        contractMethod: {
            inputs: [{internalType: "address", name: "sentinel", type: "address"}],
            name: "addSentinel",
            payable: false
        },
        data: null,
        to: $to,
        value: "0"
    }')")
done

echo "--- 5. Assemble the Safe Transaction Builder batch ---"

mkdir -p "$(dirname "$OUT_FILE")"
TRANSACTIONS_JSON="$(printf '%s\n' "${TXS[@]}" | jq -s '.')"
jq -n \
    --arg chainId "$CHAIN_ID" \
    --argjson createdAt "$(date +%s%3N)" \
    --argjson transactions "$TRANSACTIONS_JSON" \
    '{chainId: $chainId, createdAt: $createdAt, meta: {}, transactions: $transactions, version: "1.0"}' \
    > "$OUT_FILE"

# Companion file for `just verify_deployment`: everything it needs to verify these contracts once
# the Safe has actually executed the batch above, that isn't already in ENV_FILE.
ADDRESSES_FILE="${OUT_FILE%.json}.addresses.env"
cat > "$ADDRESSES_FILE" <<EOF
CHAIN_ID=$CHAIN_ID
COORDINATOR=$COORDINATOR
GROUP_ID=$GROUP_ID
SENTINEL_CONSENSUS=$SENTINEL_CONSENSUS
SENTINEL_FEE_TOKEN=$SENTINEL_FEE_TOKEN
SENTINEL_ORACLE=$SENTINEL_ORACLE
EOF

echo >&2
echo "Safe Transaction Builder batch ($(jq '.transactions | length' "$OUT_FILE") transactions) written to: $OUT_FILE" >&2
echo "Addresses for \`just verify_deployment\` written to: $ADDRESSES_FILE" >&2
