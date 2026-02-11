#!/bin/bash
set -e

# --- Configuration ---
ANVIL_RPC_URL="http://127.0.0.1:8545"
PARTICIPANTS=(
    0x70997970C51812dc3A010C7d01b50e0d17dc79C8
    0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC
    0x90F79bf6EB2c4f870365E785982E1f101E93b906
    0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65
)

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
# Use anvil default account
echo "Deploying contracts..."
env \
    PARTICIPANTS=$(IFS=, ; echo "${PARTICIPANTS[*]}") \
npm run -w contracts cmd:deploy -- \
    --rpc-url $ANVIL_RPC_URL \
    --unlocked \
    --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 \
    --broadcast

# --- 4. Run Client Integration Tests ---
echo "Running integration tests..."
npm test -w validator -- integration

echo "Integration tests finished successfully."
