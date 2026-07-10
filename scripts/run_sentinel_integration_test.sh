#!/bin/bash
# Interop/integration test for the Rust sentinel (epic Phase F1, adapted for
# commit-reveal): runs two independent Rust sentinel instances side by side
# against the same dispute on Anvil, and asserts they agree (no arbitration)
# and settle fees/bonds correctly. Unlike scripts/run_integration_test.sh, this does
# not require a full validator/FROST genesis: a `TestConsensus` contract
# stands in for `Consensus`, only emitting the `OracleTransactionProposed`
# event the sentinels need.
set -eo pipefail
# Job control, so each `&`-backgrounded command below gets its own process
# group (its PID doubling as its PGID) instead of sharing this script's.
# Cleanup then kills each job's *group* (`kill -- -$pid`), reaping the
# compiled-binary grandchildren `cargo run` spawns, without touching whatever
# else happens to share this script's own process group (e.g. a CI runner's
# step wrapper) the way `kill 0` would.
set -m

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# --- Configuration ---
RPC_URL="http://127.0.0.1:8545"
CHAIN_ID=31337
BLOCK_TIME_SECONDS=1
REQUEST_FEE=1000
BOND_MULTIPLIER=2
COMMIT_WINDOW=5
REVEAL_WINDOW=5
GOVERNANCE_DELAY=0
FUNDING_ETH=1ether
FUNDING_TOKEN=1000000
# Anvil account 0 — deployer, MyToken owner, and SentinelOracle arbitrator.
DEPLOYER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
DEPLOYER_PK=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

# --- 1. Build the Rust sentinel ---
# Built up front, before Anvil or anything else starts, so a compile error
# fails fast and no compile time is wasted while other test infrastructure
# sits idle in the background.
echo "Building the Rust sentinel..."
cargo build --package sentinel

# --- 2. Start Anvil with a 1-second block interval ---
echo "Starting Anvil..."
anvil --block-time "$BLOCK_TIME_SECONDS" > "$ROOT/anvil_sentinel_logs.txt" 2>&1 &
PIDS=("$!")

cleanup() {
	echo "Stopping background processes (${PIDS[*]})..."
	for pid in "${PIDS[@]}"; do
		# Negative PID targets the whole process group `set -m` gave this job,
		# so `cargo run`'s compiled-binary children are reaped too rather than
		# left orphaned holding a port.
		kill -- "-$pid" >/dev/null 2>&1 || true
	done
	rm -f "$SENTINEL_A_CONFIG" "$SENTINEL_B_CONFIG"
}
trap cleanup EXIT
sleep 2

# --- 3. Deploy contracts ---
echo "Deploying fee token..."
env FACTORY=2 \
	npm run -w contracts cmd:deploy:testing-erc20 -- --rpc-url "$RPC_URL" --private-key "$DEPLOYER_PK" --broadcast
FEE_TOKEN=$(jq -r '.returns.erc20.value' "$ROOT/contracts/build/broadcast/DeployERC20.s.sol/$CHAIN_ID/run-latest.json")
echo "Fee token deployed at $FEE_TOKEN"

echo "Deploying test consensus..."
npm run -w contracts cmd:deploy:test-consensus -- --rpc-url "$RPC_URL" --private-key "$DEPLOYER_PK" --broadcast
CONSENSUS=$(jq -r '.returns.consensus.value' "$ROOT/contracts/build/broadcast/DeployTestConsensus.s.sol/$CHAIN_ID/run-latest.json")
echo "Test consensus deployed at $CONSENSUS"

echo "Deploying sentinel oracle..."
env \
	FACTORY=2 \
	SENTINEL_ARBITRATOR="$DEPLOYER" \
	SENTINEL_CONSENSUS="$CONSENSUS" \
	SENTINEL_FEE_TOKEN="$FEE_TOKEN" \
	SENTINEL_REQUEST_FEE="$REQUEST_FEE" \
	SENTINEL_COMMIT_WINDOW="$COMMIT_WINDOW" \
	SENTINEL_REVEAL_WINDOW="$REVEAL_WINDOW" \
	SENTINEL_GOVERNANCE_DELAY="$GOVERNANCE_DELAY" \
	SENTINEL_BOND_MULTIPLIER="$BOND_MULTIPLIER" \
	npm run -w contracts cmd:deploy:sentinel-oracle -- --rpc-url "$RPC_URL" --private-key "$DEPLOYER_PK" --broadcast
ORACLE=$(jq -r '.returns.sentinelOracle.value' "$ROOT/contracts/build/broadcast/DeploySentinelOracleV2.s.sol/$CHAIN_ID/run-latest.json")
echo "Sentinel oracle deployed at $ORACLE"

# --- 4. Fund the required accounts ---
echo "Generating sentinel and proposer accounts..."
WALLETS=$(cast wallet new --json --number 3)
SENTINEL_A_ADDR=$(echo "$WALLETS" | jq -r '.[0].address')
SENTINEL_A_PK=$(echo "$WALLETS" | jq -r '.[0].private_key')
SENTINEL_B_ADDR=$(echo "$WALLETS" | jq -r '.[1].address')
SENTINEL_B_PK=$(echo "$WALLETS" | jq -r '.[1].private_key')
PROPOSER_ADDR=$(echo "$WALLETS" | jq -r '.[2].address')
PROPOSER_PK=$(echo "$WALLETS" | jq -r '.[2].private_key')
echo "Sentinel A: $SENTINEL_A_ADDR"
echo "Sentinel B: $SENTINEL_B_ADDR"
echo "Proposer:   $PROPOSER_ADDR"

echo "Funding accounts with ETH and the fee token..."
for addr in "$SENTINEL_A_ADDR" "$SENTINEL_B_ADDR" "$PROPOSER_ADDR"; do
	cast send --rpc-url "$RPC_URL" --private-key "$DEPLOYER_PK" --value "$FUNDING_ETH" "$addr" >/dev/null
	cast send --rpc-url "$RPC_URL" --private-key "$DEPLOYER_PK" \
		"$FEE_TOKEN" "transfer(address,uint256)" "$addr" "$FUNDING_TOKEN" >/dev/null
done

echo "Registering both sentinels..."
cast send --rpc-url "$RPC_URL" --private-key "$DEPLOYER_PK" "$ORACLE" "addSentinel(address)" "$SENTINEL_A_ADDR" >/dev/null
cast send --rpc-url "$RPC_URL" --private-key "$DEPLOYER_PK" "$ORACLE" "addSentinel(address)" "$SENTINEL_B_ADDR" >/dev/null

echo "Approving the oracle to pull the request fee from the proposer..."
cast send --rpc-url "$RPC_URL" --private-key "$PROPOSER_PK" \
	"$FEE_TOKEN" "approve(address,uint256)" "$ORACLE" "$REQUEST_FEE" >/dev/null

# Balances just before either sentinel commits a bond, to measure the fee/bond
# flow against once the dispute resolves. `--json` avoids `cast call`'s
# human-readable output appending a "[1e6]"-style scientific-notation hint,
# which breaks bash arithmetic.
balance_of() {
	cast call --rpc-url "$RPC_URL" --json "$FEE_TOKEN" "balanceOf(address)(uint256)" "$1" | jq -r '.[0]'
}
SENTINEL_A_BALANCE_BEFORE=$(balance_of "$SENTINEL_A_ADDR")
SENTINEL_B_BALANCE_BEFORE=$(balance_of "$SENTINEL_B_ADDR")

# --- 5. Spin up both Rust sentinels ---
# Already built in step 1, so this just runs it — twice, once per account,
# each with its own config file and in-memory database.
sentinel_config() {
	local signer=$1
	cat <<EOF
rpc = "$RPC_URL"
signer = "$signer"
database = "sqlite::memory:"
oracle = "$ORACLE"
consensus = "$CONSENSUS"

[sentinel]
fee_token = "$FEE_TOKEN"
voting_window = $((COMMIT_WINDOW + REVEAL_WINDOW))
blocklist = []

[index]
block_time = $((BLOCK_TIME_SECONDS * 1000))
EOF
}

SENTINEL_A_CONFIG=$(mktemp)
sentinel_config "$SENTINEL_A_PK" >"$SENTINEL_A_CONFIG"
SENTINEL_B_CONFIG=$(mktemp)
sentinel_config "$SENTINEL_B_PK" >"$SENTINEL_B_CONFIG"

echo "Starting sentinel A..."
cargo run --package sentinel -- --config-file "$SENTINEL_A_CONFIG" >"$ROOT/sentinel_a_logs.txt" 2>&1 &
PIDS+=("$!")

echo "Starting sentinel B..."
cargo run --package sentinel -- --config-file "$SENTINEL_B_CONFIG" >"$ROOT/sentinel_b_logs.txt" 2>&1 &
PIDS+=("$!")

# Give both sentinels time to connect and start watching before the dispute
# exists. Neither watcher replays history, so proposing too early makes a
# sentinel miss the block entirely.
sleep 3

# --- 6. Propose a transaction ---
echo "Proposing an oracle-checked transaction..."
env \
	CONSENSUS_ADDRESS="$CONSENSUS" \
	ORACLE_ADDRESS="$ORACLE" \
	TX_CHAIN_ID=1 \
	TX_SAFE=0x1111111111111111111111111111111111111111 \
	TX_TO=0x2222222222222222222222222222222222222222 \
	TX_NONCE=0 \
	npm run -w contracts cmd:propose:oracle -- --rpc-url "$RPC_URL" --private-key "$PROPOSER_PK" --broadcast

REQUEST_ID=$(cast logs --rpc-url "$RPC_URL" --json --from-block 0 --address "$ORACLE" \
	'NewRequest(bytes32,address,uint256,uint256,uint256,uint256)' | jq -r '.[0].topics[1]')
echo "Request id: $REQUEST_ID"

# --- 7. Wait for 10 blocks ---
START_BLOCK=$(cast block-number --rpc-url "$RPC_URL")
TARGET_BLOCK=$((START_BLOCK + 10))
echo "Waiting for block $TARGET_BLOCK (currently $START_BLOCK)..."
TIMEOUT_SECONDS=30
ELAPSED_SECONDS=0
while [ "$(cast block-number --rpc-url "$RPC_URL")" -lt "$TARGET_BLOCK" ]; do
	if [ "$ELAPSED_SECONDS" -ge "$TIMEOUT_SECONDS" ]; then
		echo "FAILED: timed out waiting for block $TARGET_BLOCK; is Anvil still mining?"
		exit 1
	fi
	sleep "$BLOCK_TIME_SECONDS"
	ELAPSED_SECONDS=$((ELAPSED_SECONDS + BLOCK_TIME_SECONDS))
done

# --- 8. Check the final vote ---
# A short grace period on top of the 10-block wait for the last finalize/claim
# transactions to land, in case they landed late in the window above. `|| true`
# on each `cast` failure keeps the loop retrying instead of `set -e` aborting
# the script on a transient RPC hiccup.
REQUEST=""
for _ in $(seq 1 10); do
	REQUEST=$(cast call --rpc-url "$RPC_URL" --json "$ORACLE" \
		"getRequest(bytes32)((address,uint256,uint256,uint256,uint256,uint8,uint256,uint256,uint256,uint256))" \
		"$REQUEST_ID" 2>/dev/null) || true
	STATE=$(echo "$REQUEST" | jq -r '.[0][5]' 2>/dev/null) || true
	[ "$STATE" = "2" ] && break
	sleep "$BLOCK_TIME_SECONDS"
done

echo "Final request state: $REQUEST"
if [ "$STATE" != "2" ]; then
	echo "FAILED: expected state RESOLVED_APPROVED (2), got $STATE"
	exit 1
fi
DENY_SENTINEL_COUNT=$(echo "$REQUEST" | jq -r '.[0][9]')
if [ "$DENY_SENTINEL_COUNT" != "0" ]; then
	echo "FAILED: expected a unanimous approve vote, but denySentinelCount is $DENY_SENTINEL_COUNT"
	exit 1
fi
DISPUTES=$(cast logs --rpc-url "$RPC_URL" --json --from-block 0 --address "$ORACLE" \
	'DisputeResolved(bytes32,uint8,uint256)' | jq 'length')
if [ "$DISPUTES" != "0" ]; then
	echo "FAILED: expected no arbitration, but $DISPUTES DisputeResolved event(s) were emitted"
	exit 1
fi
echo "OK: both sentinels agreed (approved) and no arbitration was triggered."

# --- 9. Check the fee and bond flow ---
SENTINEL_A_COMMITMENT=$(cast call --rpc-url "$RPC_URL" --json "$ORACLE" \
	"getCommitment(bytes32,address)((bytes32,uint256,uint8,bool))" "$REQUEST_ID" "$SENTINEL_A_ADDR")
SENTINEL_B_COMMITMENT=$(cast call --rpc-url "$RPC_URL" --json "$ORACLE" \
	"getCommitment(bytes32,address)((bytes32,uint256,uint8,bool))" "$REQUEST_ID" "$SENTINEL_B_ADDR")
if [ "$(echo "$SENTINEL_A_COMMITMENT" | jq -r '.[0][3]')" != "true" ] || [ "$(echo "$SENTINEL_B_COMMITMENT" | jq -r '.[0][3]')" != "true" ]; then
	echo "FAILED: expected both sentinels to have claimed their bond and reward"
	echo "Sentinel A commitment: $SENTINEL_A_COMMITMENT"
	echo "Sentinel B commitment: $SENTINEL_B_COMMITMENT"
	exit 1
fi

SENTINEL_A_BALANCE_AFTER=$(balance_of "$SENTINEL_A_ADDR")
SENTINEL_B_BALANCE_AFTER=$(balance_of "$SENTINEL_B_ADDR")
ORACLE_BALANCE_AFTER=$(balance_of "$ORACLE")
# Both bonds are returned in full (no slashing on a unanimous vote), so any
# balance gained beyond the bond amount is the sentinel's share of the request
# fee. The two shares should add up to (approximately) the whole fee: the
# equal-split reward can leave a wei or two of rounding dust behind in the
# oracle.
SENTINEL_A_REWARD=$((SENTINEL_A_BALANCE_AFTER - SENTINEL_A_BALANCE_BEFORE))
SENTINEL_B_REWARD=$((SENTINEL_B_BALANCE_AFTER - SENTINEL_B_BALANCE_BEFORE))
TOTAL_REWARD=$((SENTINEL_A_REWARD + SENTINEL_B_REWARD))
echo "Sentinel A fee share: $SENTINEL_A_REWARD"
echo "Sentinel B fee share: $SENTINEL_B_REWARD"
echo "Oracle balance after claims (dust only): $ORACLE_BALANCE_AFTER"
if [ "$SENTINEL_A_REWARD" -le 0 ] || [ "$SENTINEL_B_REWARD" -le 0 ]; then
	echo "FAILED: expected both sentinels to receive a nonzero share of the request fee"
	exit 1
fi
if [ "$TOTAL_REWARD" -gt "$REQUEST_FEE" ] || [ "$((REQUEST_FEE - TOTAL_REWARD))" -gt 2 ]; then
	echo "FAILED: expected the fee shares to add up to ~$REQUEST_FEE, got $TOTAL_REWARD"
	exit 1
fi
if [ "$ORACLE_BALANCE_AFTER" -gt 2 ]; then
	echo "FAILED: expected the oracle to hold no more than rounding dust, got $ORACLE_BALANCE_AFTER"
	exit 1
fi
echo "OK: bonds were returned in full and the request fee was split between the sentinels."

echo "Sentinel integration test finished successfully."
