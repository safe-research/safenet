#!/bin/bash
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# --- Configuration ---
ANVIL_RPC_URL="http://127.0.0.1:8545"
PARTICIPANTS=(
    0x70997970C51812dc3A010C7d01b50e0d17dc79C8
    0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC
    0x90F79bf6EB2c4f870365E785982E1f101E93b906
    0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65
)
# Anvil account 0 — used as deployer for all contracts
DEPLOYER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266

# --- 1. Start Anvil in the background ---
echo "Starting Anvil..."
# Mute anvil logs
anvil > ./anvil_logs.txt &
ANVIL_PID=$!
echo "Anvil started with PID $ANVIL_PID"

# --- 2. Setup Cleanup ---
trap 'echo "Stopping Anvil (PID $ANVIL_PID)..." && kill $ANVIL_PID' EXIT
sleep 2

# --- 3. Deploy Contracts ---
echo "Deploying contracts..."
env \
    PARTICIPANTS=$(IFS=, ; echo "${PARTICIPANTS[*]}") \
npm run -w contracts cmd:deploy -- \
    --rpc-url $ANVIL_RPC_URL \
    --unlocked \
    --sender $DEPLOYER \
    --broadcast

CONSENSUS=$(jq -r '.returns.consensus.value' \
    "$SCRIPT_DIR/../contracts/build/broadcast/Deploy.s.sol/31337/run-latest.json")
echo "Consensus deployed at $CONSENSUS"

# --- 4. Deploy Fee Token ---
echo "Deploying fee token..."
env FACTORY=2 \
npm run -w contracts cmd:deploy:testing-erc20 -- \
    --rpc-url $ANVIL_RPC_URL \
    --unlocked \
    --sender $DEPLOYER \
    --broadcast

FEE_TOKEN=$(jq -r '.returns.erc20.value' \
    "$SCRIPT_DIR/../contracts/build/broadcast/DeployERC20.s.sol/31337/run-latest.json")
echo "Fee token deployed at $FEE_TOKEN"

# --- 5. Deploy Sentinel Oracle ---
echo "Deploying Sentinel Oracle..."
env \
    SENTINEL_CONSENSUS=$CONSENSUS \
    SENTINEL_FEE_TOKEN=$FEE_TOKEN \
    SENTINEL_REQUEST_FEE=1000 \
    SENTINEL_VOTING_WINDOW=10 \
    SENTINEL_GOVERNANCE_DELAY=0 \
    SENTINEL_BOND_MULTIPLIER=2 \
npm run -w contracts cmd:deploy:sentinel-oracle -- \
    --rpc-url $ANVIL_RPC_URL \
    --unlocked \
    --sender $DEPLOYER \
    --broadcast

# --- 6. Run Client Integration Tests ---
echo "Running integration tests..."
npm test -w validator -- integration

echo "Integration tests finished successfully."
