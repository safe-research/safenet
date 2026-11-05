#!/usr/bin/env bash
# CA3 Batched-mode control (executor set), otherwise identical to e2e.sh.
# F3-SEN-001 end-to-end: the real sentinel binary, Direct mode (no executor),
# against a local Anvil carrying the real Gnosis USDC.e code+storage and a
# SentinelOracle deployed with the Aegis production parameters. One proposal is
# driven through; we observe the sentinel's own `approve` and `commit`.
#
# Binary-agnostic: it runs whatever $WT/target/debug/sentinel currently is, so
# the bite check is "rebuild the binary with a raised gas limit and re-run me".
#
# Arg 1: a label for the output subdir (e.g. "buggy" or "fixed").
set -euo pipefail
LABEL="${1:-buggy}"
D="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$D/out-$LABEL"
mkdir -p "$OUT"
: "${WT:?source the rv-sen env file first}"

PORT=31500
RPC="http://127.0.0.1:$PORT"
ENGINE_PORT=31501
PROXY=0x2a22f9c3b484c3629090FeED35F17Ff8F88f76F0   # USDC.e (Aegis fee token)
IMPL=0x107CF7fb73EA48D1D200989b156Ce1894d7AfEC7
DEPLOYER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
DEPLOYER_PK=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
TX_SAFE=0x1111111111111111111111111111111111111111

PIDS=()
cleanup() {
	for pid in "${PIDS[@]:-}"; do kill -- "-$pid" >/dev/null 2>&1 || true; done
}
trap cleanup EXIT

echo "== [1] fresh Anvil on $PORT (chain 100, 1s blocks)"
pkill -f "anvil --port $PORT" >/dev/null 2>&1 || true
sleep 1
setsid anvil --port "$PORT" --chain-id 100 --block-time 1 --prune-history 1000 --silent \
	>"$OUT/anvil.log" 2>&1 &
PIDS+=("$!")
sleep 3

echo "== [2] etch real USDC.e (proxy+impl code, 14 cached slots)"
cast rpc --rpc-url "$RPC" anvil_setCode "$PROXY" "$(cat "$D/proxy.code")" >/dev/null
cast rpc --rpc-url "$RPC" anvil_setCode "$IMPL" "$(cat "$D/impl.code")" >/dev/null
while read -r slot val; do
	cast rpc --rpc-url "$RPC" anvil_setStorageAt "$PROXY" "$slot" "$val" >/dev/null
done <"$D/slots.txt"

echo "== [3] deploy TestConsensus + SentinelOracle (Aegis parameters)"
forge script --root "$WT/contracts" DeployTestConsensusScript --rpc-url "$RPC" \
	--private-key "$DEPLOYER_PK" --broadcast >"$OUT/deploy_consensus.log" 2>&1
CONSENSUS=$(jq -r '.returns.consensus.value' "$WT/contracts/build/broadcast/DeployTestConsensus.s.sol/100/run-latest.json")
env FACTORY=2 \
	SENTINEL_ARBITRATOR="$DEPLOYER" SENTINEL_GOVERNANCE="$DEPLOYER" \
	SENTINEL_PROTOCOL_FUNDS_RECEIVER="$DEPLOYER" SENTINEL_CONSENSUS="$CONSENSUS" \
	SENTINEL_FEE_TOKEN="$PROXY" \
	SENTINEL_REQUEST_FEE=400000 SENTINEL_COMMIT_WINDOW=6 SENTINEL_REVEAL_WINDOW=6 \
	SENTINEL_GOVERNANCE_DELAY=51840 SENTINEL_BOND_MULTIPLIER=2000 \
	SENTINEL_INITIAL_SLASHING_MULTIPLIER=1 SENTINEL_INITIAL_DAO_FEE_SHARE=0 \
	SENTINEL_ARBITRATION_TIMEOUT=483840 SENTINEL_CHARTER_ENS=charter.safenet-gov.eth \
	forge script --root "$WT/contracts" DeploySentinelOracleScript --rpc-url "$RPC" \
	--private-key "$DEPLOYER_PK" --broadcast >"$OUT/deploy_oracle.log" 2>&1
ORACLE=$(jq -r '.returns.sentinelOracle.value' "$WT/contracts/build/broadcast/DeploySentinelOracle.s.sol/100/run-latest.json")
echo "   CONSENSUS=$CONSENSUS"
echo "   ORACLE=$ORACLE  fee_token=$PROXY (USDC.e)"
echo "   fee=$(cast call --rpc-url "$RPC" "$ORACLE" 'fee()(uint96)')  bondMult=$(cast call --rpc-url "$RPC" "$ORACLE" 'bondMultiplier()(uint32)')"

echo "== [3b] deploy Safenet7702Executor (Batched-mode control)"
env FACTORY=2 forge script --root "$WT/contracts" DeploySafenet7702ExecutorScript --rpc-url "$RPC" \
	--private-key "$DEPLOYER_PK" --broadcast >"$OUT/deploy_executor.log" 2>&1
EXECUTOR=$(jq -er '.returns.account.value' "$WT/contracts/build/broadcast/DeploySafenet7702Executor.s.sol/100/run-latest.json")
echo "   EXECUTOR=$EXECUTOR code_len=$(cast code --rpc-url "$RPC" "$EXECUTOR" | wc -c)"

echo "== [4] sentinel + sponsor accounts, funded with ETH and USDC.e"
SENTINEL_PK=0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d
SENTINEL=$(cast wallet address "$SENTINEL_PK")
SPONSOR_PK=0x8b3a350cf5c34c9194ca85829a2df0ec3153be0318b5e2d3348e872092edffba
SPONSOR=$(cast wallet address "$SPONSOR_PK")
for a in "$SENTINEL" "$SPONSOR"; do
	cast rpc --rpc-url "$RPC" anvil_setBalance "$a" 0xde0b6b3a7640000 >/dev/null # 1 ETH
	BAL_SLOT=$(cast index address "$a" 9)
	# 1,000,000 USDC.e (6 decimals) each: covers the 800 USDC.e bond and the 0.40 fee.
	cast rpc --rpc-url "$RPC" anvil_setStorageAt "$PROXY" "$BAL_SLOT" \
		0x000000000000000000000000000000000000000000000000000000e8d4a51000 >/dev/null
done
echo "   SENTINEL=$SENTINEL  usdc=$(cast call --rpc-url "$RPC" "$PROXY" 'balanceOf(address)(uint256)' "$SENTINEL")"
echo "   SPONSOR =$SPONSOR  usdc=$(cast call --rpc-url "$RPC" "$PROXY" 'balanceOf(address)(uint256)' "$SPONSOR")"

echo "== [5] register the sentinel, then mine past the 51840-block governance delay"
cast send --rpc-url "$RPC" --private-key "$DEPLOYER_PK" "$ORACLE" 'addSentinel(address)' "$SENTINEL" >/dev/null
ACTIVE_AT=$(cast call --rpc-url "$RPC" "$ORACLE" 'sentinelActiveAt(address)(uint256)' "$SENTINEL" | awk '{print $1}')
cast rpc --rpc-url "$RPC" anvil_mine 0xca80 >/dev/null # 51840 empty blocks, instant
echo "   activeAt=$ACTIVE_AT  head=$(cast block-number --rpc-url "$RPC")  isActive(by activeAt)=yes"
# Sponsor approves the oracle to pull the 0.40 fee (normal approve, cast estimates gas).
cast send --rpc-url "$RPC" --private-key "$SPONSOR_PK" "$PROXY" 'approve(address,uint256)' "$ORACLE" 400000 >/dev/null

echo "== [6] start stub engine + the REAL sentinel binary (Direct mode)"
python3 -I "$D/engine_stub.py" "$ENGINE_PORT" >"$OUT/engine.log" 2>&1 &
PIDS+=("$!")
CONFIG="$OUT/sentinel.toml"
cat >"$CONFIG" <<TOML
rpc = "$RPC"
signer = "$SENTINEL_PK"
database = "sqlite::memory:"
oracle = "$ORACLE"
consensus = "$CONSENSUS"

[sentinel]
fee_token = "$PROXY"
voting_window = 6
engine = "http://127.0.0.1:$ENGINE_PORT"

[transactions]
executor = "$EXECUTOR"

[index]
block_time = 1000
block_propagation_delay = 0
block_retry_delays = []
TOML
SAFENET_TEST_VERBOSE=1 RUST_LOG=info,sentinel=debug,safenet_core=debug \
	"$WT/target/debug/sentinel" --config-file "$CONFIG" >"$OUT/sentinel.log" 2>&1 &
PIDS+=("$!")
sleep 4

echo "== [7] propose one oracle-checked transaction"
START_BLOCK=$(cast block-number --rpc-url "$RPC")
env CONSENSUS_ADDRESS="$CONSENSUS" ORACLE_ADDRESS="$ORACLE" TX_CHAIN_ID=100 \
	TX_SAFE="$TX_SAFE" TX_TO="$TX_SAFE" TX_NONCE=0 \
	forge script --root "$WT/contracts" ProposeTransactionScript --rpc-url "$RPC" \
	--private-key "$SPONSOR_PK" --broadcast >"$OUT/propose.log" 2>&1
REQUEST_ID=$(cast logs --rpc-url "$RPC" --json --from-block "$START_BLOCK" --address "$ORACLE" \
	'NewRequest(bytes32,address,uint96,uint96,uint24,uint96,uint64,uint64)' | jq -r '.[-1].topics[1]')
PROP_BLOCK=$(cast logs --rpc-url "$RPC" --json --from-block "$START_BLOCK" --address "$ORACLE" \
	'NewRequest(bytes32,address,uint96,uint96,uint24,uint96,uint64,uint64)' | jq -r '.[-1].blockNumber')
echo "   request_id=$REQUEST_ID  proposed in block $((PROP_BLOCK))  commitDeadline=+6"

echo "== [8] let the commit window (6 blocks) pass with margin"
sleep 16

echo "== [9] OBSERVE"
echo "-- Committed events for this request (expect 0 in the buggy run):"
cast logs --rpc-url "$RPC" --json --from-block "$START_BLOCK" --address "$ORACLE" \
	'Committed(bytes32,address,uint96)' "$REQUEST_ID" | jq -r 'length as $n | "   count=\($n)"'
echo "-- allowance(sentinel -> oracle): $(cast call --rpc-url "$RPC" "$PROXY" 'allowance(address,address)(uint256)' "$SENTINEL" "$ORACLE")"
echo "-- getCommitment(request, sentinel) vote (0=NONE): $(cast call --rpc-url "$RPC" "$ORACLE" 'getCommitment(bytes32,address)((bytes32,uint96,uint8,bool))' "$REQUEST_ID" "$SENTINEL" | tr -d '()' | awk -F, '{print $3}')"
echo "-- sentinel USDC.e balance now: $(cast call --rpc-url "$RPC" "$PROXY" 'balanceOf(address)(uint256)' "$SENTINEL")"
echo "-- oracle USDC.e balance now  : $(cast call --rpc-url "$RPC" "$PROXY" 'balanceOf(address)(uint256)' "$ORACLE")"

echo "-- the sentinel's own transactions (scan blocks $START_BLOCK..head):"
HEAD=$(cast block-number --rpc-url "$RPC")
python3 -I "$D/scan_txs.py" "$RPC" "$SENTINEL" "$START_BLOCK" "$HEAD" "$PROXY" "$ORACLE"

echo "-- tail of the sentinel log (drop / reveal decisions):"
grep -iE "ignoring|drop|reveal|commit|approve|out of gas|revert|finaliz" "$OUT/sentinel.log" | tail -25 || true

echo "== done ($LABEL). Outputs in $OUT"
