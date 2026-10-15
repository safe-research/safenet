#!/usr/bin/env bash
# F3-XC-160 end to end: two real sentinels (each with a real sentinel-engine) vote on one request
# on a local Anvil with the real SentinelOracle (Aegis amounts in 6-decimal base units: fee
# 400000 = 0.40, bond multiplier 2000 = 800, slashing 1x, commit and reveal windows 6 blocks).
# Sentinel A reads the chain through a proxy that models a load-balanced endpoint with one
# backend a block behind: after the block P holding NewRequest, the logs query for P + 1 (by
# hash) answers -32001 and the revalidating eth_getBlockByNumber(P + 1) answers null, once each.
# Sentinel B reads Anvil directly. Usage: f160.sh <label> <inject: 1 or 0>
set -u
source /home/sj.guest/.cache/kb-runs/safenet/2026-10-08-rust-audit-run3/env/rv-core-a.sh
source "$SAFENET_REVIEW_ROOT/scratch/QA3-CORE/procs.sh"
label=$1-$(date +%s) inject=$2
D=$QA3/f160-$label
mkdir -p "$D"
ROOT=$WT
A=http://127.0.0.1:31204
P=http://127.0.0.1:31205
CHAIN_ID=31337
DEPLOYER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
DEPLOYER_PK=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
TX_SAFE=0x1111111111111111111111111111111111111111
REQUEST_FEE=400000
BOND=800000000
NEW_REQUEST=0x8ec61272960f97d43d7f8f85ed630aa512818577f3d1e548e7627b06dbbbda86
export FOUNDRY_OFFLINE=true

cleanup() { for name in "sentinel-a-$label" "sentinel-b-$label" "engine-a-$label" "engine-b-$label" "proxy-160-$label" "anvil-160-$label"; do stop "$name"; done; }
trap cleanup EXIT
start_anvil "anvil-160-$label" 31204 --block-time 1 || exit 1
if [ "$inject" = 1 ]; then
  echo "{\"inject_after_topic\": \"$NEW_REQUEST\"}" > "$D/ctl.json"
else
  echo '{}' > "$D/ctl.json"
fi
start_proxy "proxy-160-$label" 31205 $A "$D/ctl.json" || exit 1

deploy() { forge script --root "$ROOT/contracts" "$@" --rpc-url $A --private-key $DEPLOYER_PK --broadcast > "$D/forge.log" 2>&1 || { tail -n 3 "$D/forge.log" | cut -c1-300; exit 1; }; }
FACTORY=2 deploy DeployERC20Script
FEE_TOKEN=$(jq -r '.returns.erc20.value' "$ROOT/contracts/build/broadcast/DeployERC20.s.sol/$CHAIN_ID/run-latest.json")
deploy DeployTestConsensusScript
CONSENSUS=$(jq -r '.returns.consensus.value' "$ROOT/contracts/build/broadcast/DeployTestConsensus.s.sol/$CHAIN_ID/run-latest.json")
env FACTORY=2 SENTINEL_ARBITRATOR=$DEPLOYER SENTINEL_GOVERNANCE=$DEPLOYER SENTINEL_PROTOCOL_FUNDS_RECEIVER=$DEPLOYER \
  SENTINEL_CONSENSUS=$CONSENSUS SENTINEL_FEE_TOKEN=$FEE_TOKEN SENTINEL_REQUEST_FEE=$REQUEST_FEE \
  SENTINEL_COMMIT_WINDOW=6 SENTINEL_REVEAL_WINDOW=6 SENTINEL_GOVERNANCE_DELAY=0 SENTINEL_BOND_MULTIPLIER=2000 \
  SENTINEL_INITIAL_SLASHING_MULTIPLIER=1 SENTINEL_INITIAL_DAO_FEE_SHARE=0 SENTINEL_CHARTER_ENS=safenet-charter.safe.eth \
  SENTINEL_ARBITRATION_TIMEOUT=483840 \
  forge script --root "$ROOT/contracts" DeploySentinelOracleScript --rpc-url $A --private-key $DEPLOYER_PK --broadcast > "$D/forge.log" 2>&1 || { tail -n 3 "$D/forge.log" | cut -c1-300; exit 1; }
ORACLE=$(jq -r '.returns.sentinelOracle.value' "$ROOT/contracts/build/broadcast/DeploySentinelOracle.s.sol/$CHAIN_ID/run-latest.json")
echo "fee token $FEE_TOKEN, test consensus $CONSENSUS, oracle $ORACLE"

WALLETS=$(cast wallet new --json --number 3)
SA=$(echo "$WALLETS" | jq -r '.[0].address'); SA_PK=$(echo "$WALLETS" | jq -r '.[0].private_key')
SB=$(echo "$WALLETS" | jq -r '.[1].address'); SB_PK=$(echo "$WALLETS" | jq -r '.[1].private_key')
SP=$(echo "$WALLETS" | jq -r '.[2].address'); SP_PK=$(echo "$WALLETS" | jq -r '.[2].private_key')
for addr in $SA $SB $SP; do
  cast send --rpc-url $A --private-key $DEPLOYER_PK --value 1ether "$addr" > /dev/null
  cast send --rpc-url $A --private-key $DEPLOYER_PK "$FEE_TOKEN" "transfer(address,uint256)" "$addr" 1000000000000 > /dev/null
done
cast send --rpc-url $A --private-key $DEPLOYER_PK "$ORACLE" "addSentinel(address)" "$SA" > /dev/null
cast send --rpc-url $A --private-key $DEPLOYER_PK "$ORACLE" "addSentinel(address)" "$SB" > /dev/null
cast send --rpc-url $A --private-key "$SP_PK" "$FEE_TOKEN" "approve(address,uint256)" "$ORACLE" $REQUEST_FEE > /dev/null
echo "sentinel A (lagging-backend RPC) $SA, sentinel B (direct RPC) $SB, sponsor $SP"

engine_config() { printf 'rpc = "%s"\nbind_address = "%s"\n\n[engine]\nblocklist = []\naddress_poisoning_lookback_blocks = 1000\n' $A "$1"; }
sentinel_config() {
  printf '%s\n' "rpc = \"$1\"" "signer = \"$2\"" "database = \"sqlite:$3?mode=rwc\"" "oracle = \"$ORACLE\"" "consensus = \"$CONSENSUS\"" \
    '' '[sentinel]' "fee_token = \"$FEE_TOKEN\"" 'voting_window = 6' "engine = \"http://$4\"" \
    '' '[observability]' 'log_filter = "info,safenet_core::index=debug,safenet_core::tx=debug,sentinel=debug"' \
    '' '[index]' 'block_time = 1000' 'block_propagation_delay = 0' 'block_retry_delays = []'
}
engine_config 127.0.0.1:31206 > "$D/engine-a.toml"
engine_config 127.0.0.1:31207 > "$D/engine-b.toml"
sentinel_config $P "$SA_PK" "$D/sentinel-a.db" 127.0.0.1:31206 > "$D/sentinel-a.toml"
sentinel_config $A "$SB_PK" "$D/sentinel-b.db" 127.0.0.1:31207 > "$D/sentinel-b.toml"
start_bg "engine-a-$label" "$D/engine-a.log" "$CARGO_TARGET_DIR/debug/sentinel-engine" --config-file "$D/engine-a.toml"
start_bg "engine-b-$label" "$D/engine-b.log" "$CARGO_TARGET_DIR/debug/sentinel-engine" --config-file "$D/engine-b.toml"
start_bg "sentinel-a-$label" "$D/sentinel-a.log" "$CARGO_TARGET_DIR/debug/sentinel" --config-file "$D/sentinel-a.toml"
start_bg "sentinel-b-$label" "$D/sentinel-b.log" "$CARGO_TARGET_DIR/debug/sentinel" --config-file "$D/sentinel-b.toml"
sleep 4

balance() { cast call --rpc-url $A --json "$FEE_TOKEN" "balanceOf(address)(uint256)" "$1" | jq -r '.[0]'; }
SA_BEFORE=$(balance $SA); SB_BEFORE=$(balance $SB)
env CONSENSUS_ADDRESS=$CONSENSUS ORACLE_ADDRESS=$ORACLE TX_CHAIN_ID=$CHAIN_ID TX_SAFE=$TX_SAFE TX_TO=$TX_SAFE TX_NONCE=0 \
  forge script --root "$ROOT/contracts" ProposeTransactionScript --rpc-url $A --private-key "$SP_PK" --broadcast > "$D/forge.log" 2>&1 || { tail -n 3 "$D/forge.log" | cut -c1-300; exit 1; }
LOG=$(cast logs --rpc-url $A --json --from-block 0 --address "$ORACLE" 'NewRequest(bytes32,address,uint96,uint96,uint24,uint96,uint64,uint64)')
ID=$(echo "$LOG" | jq -r '.[0].topics[1]')
PB=$(( $(echo "$LOG" | jq -r '.[0].blockNumber') ))
echo "request $ID opened in block P=$PB (commit deadline P+6, reveal deadline P+12)"
while [ "$(cast block-number --rpc-url $A)" -lt $((PB + 20)) ]; do sleep 1; done
sleep 2

echo "== onchain after block P+20"
commitment() { cast call --rpc-url $A --json "$ORACLE" "getCommitment(bytes32,address)((bytes32,uint96,uint8,bool))" "$ID" "$1" | jq -c '.[0]'; }
REQ=$(cast call --rpc-url $A --json "$ORACLE" "getRequest(bytes32)((uint64,uint24,uint64,uint96,address,uint96,uint8),(uint8,uint96,uint64,uint16,uint16,uint16,uint16,uint24))" "$ID" | jq -c '.[1]')
echo "request progress (state, fee, arbitrationDeadline, committed, revealed, approve, deny, ...): $REQ"
echo "sentinel A commitment (hash, bond, vote, claimed): $(commitment $SA)"
echo "sentinel B commitment (hash, bond, vote, claimed): $(commitment $SB)"
echo "fee token balance change: A $(( $(balance $SA) - SA_BEFORE )), B $(( $(balance $SB) - SB_BEFORE )), oracle holds $(balance $ORACLE)"
echo "== proxy injections"
grep -E 'armed_for_block|injected' "$QA3/logs/proxy-160-$label.jsonl" || echo "(none)"
for s in a b; do
  echo "== sentinel ${s^^} log (selected lines)"
  grep -E 'uncled|invalidating|ignoring|dropping|Reveal|Commit|Claim|Finalize|submitting transaction|unrecoverable' "$D/sentinel-$s.log" \
    | python3 -I -c "
import json, sys
for line in sys.stdin:
    try:
        e = json.loads(line)
    except Exception:
        print(line.rstrip()[:300]); continue
    f = e.get('fields', {})
    msg = f.pop('message', '')
    print(e['timestamp'][11:23], e['level'], msg, {k: v for k, v in f.items() if k in ('number', 'hash', 'request_id', 'state', 'nonce', 'block', 'kind')})
"
done
cleanup
trap - EXIT
running
echo "artifacts in $D"
