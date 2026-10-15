#!/bin/bash
# F-VAL-004 Phase 8: try to induce a permanent genesis stall by restarting a
# validator inside the KeyGenSetup window (after the KeyGen event is indexed and
# the CollectingCommitments{secrets:None} snapshot commits, but before the
# validator has published+confirmed its commitment). If genesis never finalizes
# and the restarted validator never (re)publishes its commitment, the stall
# reproduced: nothing re-issues KeyGenSetup and no genesis timeout arm exists.
set -uo pipefail
export PATH="$HOME/.foundry/bin:$HOME/.cargo/bin:$PATH"
ANVIL_PORT=8551; RPC="http://127.0.0.1:$ANVIL_PORT"; CHAIN_ID=31337; BLOCK_TIME=1
SC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; REPO_ROOT="/home/shebin.guest/safe/safenet"
source "$REPO_ROOT/scripts/lib/shared_test_scripts.sh"
PARTICIPANTS=(0x70997970C51812dc3A010C7d01b50e0d17dc79C8 0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC)
PRIVATE_KEYS=(0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d 0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a)
SENDER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
KILL_DELAY="${1:-1.2}"   # seconds after genesis trigger to SIGKILL validator A
TMPDIR="$SC/f004_tmp"; rm -rf "$TMPDIR"; mkdir -p "$TMPDIR"; PIDS=(); VA_PID=""
cleanup(){ for p in "${PIDS[@]:-}"; do kill "$p" 2>/dev/null; done; }
trap cleanup EXIT
anvil --block-time "$BLOCK_TIME" --port "$ANVIL_PORT" > "$SC/f004_anvil.txt" 2>&1 & PIDS+=("$!")
for i in $(seq 1 40); do cast block-number --rpc-url "$RPC" >/dev/null 2>&1 && break; sleep 0.25; done
echo "effective rpc=$RPC chain=$(cast chain-id --rpc-url $RPC)"
PARTICIPANTS_CSV=$(IFS=,; echo "${PARTICIPANTS[*]}")
deploy_validator_contracts "$RPC" "$SENDER" "$PARTICIPANTS_CSV" "$CHAIN_ID" >/dev/null 2>&1
echo "contracts: coord=$COORDINATOR_ADDR consensus=$CONSENSUS_ADDR"
mkcfg(){ print_validator_config_base "$RPC" "$1" "$2" "$CONSENSUS_ADDR" "$ORACLE_ADDR" 1000000 "$((BLOCK_TIME*1000))" PARTICIPANTS; }
mkcfg "${PRIVATE_KEYS[0]}" "$TMPDIR/va.sqlite" > "$TMPDIR/va.toml"
mkcfg "${PRIVATE_KEYS[1]}" "$TMPDIR/vb.sqlite" > "$TMPDIR/vb.toml"
: > "$SC/f004_valA.txt"; : > "$SC/f004_valB.txt"
start_va(){ "$REPO_ROOT/target/debug/validator" --config-file "$TMPDIR/va.toml" >> "$SC/f004_valA.txt" 2>&1 & VA_PID="$!"; PIDS+=("$VA_PID"); }
"$REPO_ROOT/target/debug/validator" --config-file "$TMPDIR/vb.toml" >> "$SC/f004_valB.txt" 2>&1 & PIDS+=("$!")
start_va
sleep 1
echo "==> trigger genesis, then SIGKILL A after ${KILL_DELAY}s"
trigger_genesis_keygen "$RPC" "$SENDER" "$PARTICIPANTS_CSV" "$COORDINATOR_ADDR" >/dev/null 2>&1
sleep "$KILL_DELAY"
kill -9 "$VA_PID" 2>/dev/null
echo "    killed A (pid $VA_PID) at block $(cast block-number --rpc-url $RPC)"
# did A publish its commitment before the kill?
COMMITS_BEFORE=$(fetch_logs "$RPC" "$COORDINATOR_ADDR" 'KeyGenCommitted(bytes32,address,(((uint256,uint256),(uint256,uint256)[],(uint256,uint256),uint256)),bool)' 2>/dev/null | jq 'length' 2>/dev/null || echo "?")
echo "    KeyGenCommitted logs before restart = ${COMMITS_BEFORE}"
sleep 1
echo "==> RESTART A"
start_va
# wait up to 40s: does genesis finalize (KeyGenConfirmed completed=true)?
DONE=0
for i in $(seq 1 40); do
  CONF=$(fetch_logs "$RPC" "$COORDINATOR_ADDR" 'KeyGenConfirmed(bytes32,address,bool)' 2>/dev/null)
  DONE=$(jq '[.[]|select(.data|endswith("0000000000000000000000000000000000000000000000000000000000000001"))]|length' <<<"$CONF" 2>/dev/null || echo 0)
  ACOMMIT=$(fetch_logs "$RPC" "$COORDINATOR_ADDR" 'KeyGenCommitted(bytes32,address,(((uint256,uint256),(uint256,uint256)[],(uint256,uint256),uint256)),bool)' 2>/dev/null | jq --arg a "0x70997970c51812dc3a010c7d01b50e0d17dc79c8" '[.[]|select((.data[26:66]|ascii_downcase)==($a[2:]))]|length' 2>/dev/null || echo "?")
  [ "${DONE:-0}" -ge 1 ] && break; sleep 1
done
echo "===== VERDICT (kill_delay=${KILL_DELAY}) ====="
echo "genesis finalized (completed=true) = ${DONE:-0}"
echo "validator A KeyGenCommitted count after restart = ${ACOMMIT:-?}"
GROUPSTATUS=$(cast call "$COORDINATOR_ADDR" "groupKey(bytes32)((uint256,uint256))" "$GID_UNUSED" 2>/dev/null || true)
if [ "${DONE:-0}" -ge 1 ]; then echo "RESULT: genesis RECOVERED (finalized) -> stall NOT reproduced at this delay"; else echo "RESULT: genesis DID NOT finalize within 40s after restart -> STALL candidate"; fi
