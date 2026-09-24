#!/bin/bash
# F-VAL-033 Phase 8 live reproduction:
# backup validator DBs mid-run (nonces present, seq 0 unused) -> group signs m
# (burns nonce for seq 0) -> reorg drops the sign -> re-propose m' (rebinds seq 0)
# -> STOP validators, RESTORE backup, RESTART -> observe whether the same nonce
# (d,e) is revealed again for a DIFFERENT message m'.
set -uo pipefail
export PATH="$HOME/.foundry/bin:$HOME/.cargo/bin:$PATH"

ANVIL_PORT=8549
RPC="http://127.0.0.1:$ANVIL_PORT"
CHAIN_ID=31337
BLOCK_TIME=1
SC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="/home/shebin.guest/safe/safenet"
source "$REPO_ROOT/scripts/lib/shared_test_scripts.sh"

PARTICIPANTS=(0x70997970C51812dc3A010C7d01b50e0d17dc79C8 0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC)
PRIVATE_KEYS=(0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d 0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a)
SENDER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
PARTA=0x70997970c51812dc3a010c7d01b50e0d17dc79c8
GENESIS_EPOCH_WORD=0x0000000000000000000000000000000000000000000000000000000000000000
SIGN_T0=0xb48d242879f9f3df555c800db966f65cba128c7213198748fa202ed54e092691
REVEAL_T0=0xa8415ae8824ba92b55156b0447b9b9bbc3ba63988b076fb0c8d8e180893d1a46
fetch_t0(){ cast logs --json --rpc-url "$RPC" --from-block 0 --to-block latest --address "$COORDINATOR_ADDR" "$1"; }

TMPDIR="$SC/f033b_tmp"; rm -rf "$TMPDIR"; mkdir -p "$TMPDIR"
PIDS=(); VA_PID=""; VB_PID=""
cleanup(){ for p in "${PIDS[@]:-}"; do kill "$p" 2>/dev/null; done; }
trap cleanup EXIT

echo "==> anvil on $RPC"; anvil --block-time "$BLOCK_TIME" --port "$ANVIL_PORT" > "$SC/f033b_anvil.txt" 2>&1 & PIDS+=("$!")
for i in $(seq 1 40); do cast block-number --rpc-url "$RPC" >/dev/null 2>&1 && break; sleep 0.25; done
echo "    effective rpc = $RPC (chain $(cast chain-id --rpc-url $RPC))"

PARTICIPANTS_CSV=$(IFS=,; echo "${PARTICIPANTS[*]}")
deploy_validator_contracts "$RPC" "$SENDER" "$PARTICIPANTS_CSV" "$CHAIN_ID"

VA_DB="$TMPDIR/va.sqlite"; VB_DB="$TMPDIR/vb.sqlite"
mkcfg(){ print_validator_config_base "$RPC" "$1" "$2" "$CONSENSUS_ADDR" "$ORACLE_ADDR" 1000000 "$((BLOCK_TIME*1000))" PARTICIPANTS; echo "max_reorg_depth = 10"; }
mkcfg "${PRIVATE_KEYS[0]}" "$VA_DB" > "$TMPDIR/va.toml"
mkcfg "${PRIVATE_KEYS[1]}" "$VB_DB" > "$TMPDIR/vb.toml"

start_va(){ "$REPO_ROOT/target/debug/validator" --config-file "$TMPDIR/va.toml" >> "$SC/f033b_valA.txt" 2>&1 & VA_PID="$!"; PIDS+=("$VA_PID"); }
start_vb(){ "$REPO_ROOT/target/debug/validator" --config-file "$TMPDIR/vb.toml" >> "$SC/f033b_valB.txt" 2>&1 & VB_PID="$!"; PIDS+=("$VB_PID"); }
: > "$SC/f033b_valA.txt"; : > "$SC/f033b_valB.txt"
echo "==> starting validators"; start_va; start_vb; sleep 1
trigger_genesis_keygen "$RPC" "$SENDER" "$PARTICIPANTS_CSV" "$COORDINATOR_ADDR"

# wait genesis finalized + preprocess (nonce trees) by both
echo "==> waiting for genesis finalize + nonce trees"
GID=""; for i in $(seq 1 90); do
  CONF=$(fetch_logs "$RPC" "$COORDINATOR_ADDR" 'KeyGenConfirmed(bytes32,address,bool)')
  DONE=$(jq '[.[]|select(.data|endswith("0000000000000000000000000000000000000000000000000000000000000001"))]|length' <<<"$CONF")
  GID=$(jq -r '.[0].topics[1] // empty' <<<"$CONF")
  PRE=$(fetch_logs "$RPC" "$COORDINATOR_ADDR" 'Preprocess(bytes32,address,uint64,bytes32)')
  NPRE=$(jq 'length' <<<"$PRE")
  [ "${DONE:-0}" -ge 1 ] && [ "${NPRE:-0}" -ge 2 ] && break
  sleep 1
done
echo "    genesis group=$GID finalized=$DONE preprocess=$NPRE at block $(cast block-number --rpc-url $RPC)"
# Wait until valA has actually revealed a nonce at least once (=> genesis participation is
# durably in its snapshot, and one nonce offset is already consumed). This avoids backing up
# a snapshot taken before the KeyGenConfirmed transition committed.
echo "==> waiting until valA reveals its first nonce (durable participation)"
for i in $(seq 1 60); do
  RC=$(grep -c "revealing nonce commitment" "$SC/f033b_valA.txt" 2>/dev/null || echo 0)
  [ "${RC:-0}" -ge 1 ] && break; sleep 1
done
echo "    valA reveal count before backup = ${RC:-0} at block $(cast block-number --rpc-url $RPC)"

# ---- BACKUP (mid-run, seq 0 unused) ----
echo "==> BACKUP validator DBs (SIGSTOP for consistent copy)"
kill -STOP "$VA_PID"; kill -STOP "$VB_PID"; sleep 0.5
rm -rf "$TMPDIR/backup"; mkdir -p "$TMPDIR/backup"
cp "$VA_DB"* "$TMPDIR/backup/" 2>/dev/null; cp "$VB_DB"* "$TMPDIR/backup/" 2>/dev/null
ls -la "$TMPDIR/backup" | sed 's/^/    /'
kill -CONT "$VA_PID"; kill -CONT "$VB_PID"

# ---- propose m (TX_NONCE=1) ----
echo "==> propose transaction m (TX_NONCE=1)"
env CONSENSUS_ADDRESS="$CONSENSUS_ADDR" ORACLE_ADDRESS="$ORACLE_ADDR" TX_CHAIN_ID="$CHAIN_ID" TX_SAFE="$SENDER" TX_TO="$SENDER" TX_NONCE=1 \
  forge script --root "$REPO_ROOT/contracts" ProposeTransactionScript --rpc-url "$RPC" --unlocked --sender "$SENDER" --broadcast >/dev/null 2>&1
SIGS=$(fetch_logs "$RPC" "$CONSENSUS_ADDR" 'TransactionProposed(bytes32,bytes32,address,uint64,bytes,(uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256))')
MHASH=$(jq -er --arg e "$GENESIS_EPOCH_WORD" '[.[]|select(.data|startswith($e))][-1].topics[1]' <<<"$SIGS")
echo "    message m proposed, safeTxHash=$MHASH"
# wait attested
for i in $(seq 1 60); do
  ATT=$(fetch_logs "$RPC" "$CONSENSUS_ADDR" 'TransactionAttested(bytes32,bytes32,address,uint64,bytes32,bytes32,((uint256,uint256),uint256))')
  A1=$(jq --arg h "$MHASH" '[.[]|select(.topics[1]==$h)]|length' <<<"$ATT")
  [ "${A1:-0}" -ge 1 ] && break; sleep 1
done
echo "    m attested=$A1 at block $(cast block-number --rpc-url $RPC)"
# capture Sign(m) block + valA revealed nonce
SIGN=$(fetch_t0 "$SIGN_T0"); echo "    [dbg] Sign logs count=$(jq 'length' <<<"$SIGN")"
# There is exactly one Sign per proposal; take the latest (=Sign(m), sequence 0).
SIGN_M_BLOCK=$(jq -r '[.[]][-1].blockNumber' <<<"$SIGN")
SID_M=$(jq -r '[.[]][-1].data[0:66]' <<<"$SIGN")
SEQ_M=$(jq -r '[.[]][-1].data[66:]' <<<"$SIGN")
if [ "$SIGN_M_BLOCK" = "null" ] || [ -z "$SIGN_M_BLOCK" ]; then echo "FATAL: no Sign(m) log"; exit 1; fi
SIGN_M_DEC=$((16#${SIGN_M_BLOCK#0x}))
echo "    Sign(m) sequenceWord=$SEQ_M"
echo "    Sign(m) sid=$SID_M at block $SIGN_M_DEC"
REV=$(fetch_t0 "$REVEAL_T0")
NONCE_M=$(jq -r --arg sid "$SID_M" --arg a "$PARTA" '[.[]|select(.topics[1]==$sid)|select((.data[26:66]|ascii_downcase)==($a[2:]))][0].data[66:]' <<<"$REV")
echo "    valA revealed nonce for m (d.x,d.y,e.x,e.y concatenated):"; echo "    $NONCE_M"

# ---- reorg back past Sign(m) ----
HEAD=$(cast block-number --rpc-url "$RPC"); DEPTH=$((HEAD - SIGN_M_DEC + 1))
echo "==> reorg $DEPTH blocks from head $HEAD (uncle=$((SIGN_M_DEC-1)), dropping Sign(m))"
cast rpc anvil_reorg "$DEPTH" '[]' --rpc-url "$RPC" >/dev/null
sleep 3

# ---- propose m' (TX_NONCE=2) : rebinds sequence 0 to a new message ----
echo "==> propose transaction m' (TX_NONCE=2) after reorg"
env CONSENSUS_ADDRESS="$CONSENSUS_ADDR" ORACLE_ADDRESS="$ORACLE_ADDR" TX_CHAIN_ID="$CHAIN_ID" TX_SAFE="$SENDER" TX_TO="$SENDER" TX_NONCE=2 \
  forge script --root "$REPO_ROOT/contracts" ProposeTransactionScript --rpc-url "$RPC" --unlocked --sender "$SENDER" --broadcast >/dev/null 2>&1
sleep 2
SIGS2=$(fetch_logs "$RPC" "$CONSENSUS_ADDR" 'TransactionProposed(bytes32,bytes32,address,uint64,bytes,(uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256))')
MHASH2=$(jq -er --arg e "$GENESIS_EPOCH_WORD" '[.[]|select(.data|startswith($e))][-1].topics[1]' <<<"$SIGS2")
SIGN2=$(fetch_t0 "$SIGN_T0"); echo "    [dbg] Sign logs count(after reorg)=$(jq 'length' <<<"$SIGN2")"
SID_MP=$(jq -r '[.[]][-1].data[0:66]' <<<"$SIGN2")
SEQ_MP=$(jq -r '[.[]][-1].data[66:]' <<<"$SIGN2")
echo "    m' safeTxHash=$MHASH2 sid=$SID_MP sequenceWord=$SEQ_MP"
echo "    (m != m': $([ "$MHASH" != "$MHASH2" ] && echo YES || echo NO))"

# ---- STOP validators, RESTORE backup, RESTART ----
echo "==> STOP validators"; kill "$VA_PID" 2>/dev/null; kill "$VB_PID" 2>/dev/null; sleep 2
echo "==> RESTORE backup over live DBs (un-burns nonce for seq 0)"
rm -f "$VA_DB"* "$VB_DB"*; cp "$TMPDIR/backup/"va.sqlite* "$TMPDIR/" 2>/dev/null; cp "$TMPDIR/backup/"vb.sqlite* "$TMPDIR/" 2>/dev/null
echo "==> RESTART validators (fresh index over post-reorg chain)"; start_va; start_vb

# ---- wait for valA to reveal a nonce for sid_m' ----
echo "==> waiting for valA to reveal nonce for m' after restore"
NONCE_MP=""
for i in $(seq 1 90); do
  REV2=$(fetch_t0 "$REVEAL_T0")
  NONCE_MP=$(jq -r --arg sid "$SID_MP" --arg a "$PARTA" '[.[]|select(.topics[1]==$sid)|select((.data[26:66]|ascii_downcase)==($a[2:]))][0].data[66:] // empty' <<<"$REV2")
  [ -n "$NONCE_MP" ] && break; sleep 1
done
ATT2=$(fetch_logs "$RPC" "$CONSENSUS_ADDR" 'TransactionAttested(bytes32,bytes32,address,uint64,bytes32,bytes32,((uint256,uint256),uint256))')
A2=$(jq --arg h "$MHASH2" '[.[]|select(.topics[1]==$h)]|length' <<<"$ATT2")
echo "    valA revealed nonce for m' after restore:"; echo "    ${NONCE_MP:-<none>}"
echo "    m' attested after restore = ${A2:-0}"

echo "===================== VERDICT ====================="
echo "message m  = $MHASH"
echo "message m' = $MHASH2"
echo "valA nonce (m)  = $NONCE_M"
echo "valA nonce (m') = ${NONCE_MP:-<none>}"
if [ -n "$NONCE_MP" ] && [ "$NONCE_M" = "$NONCE_MP" ]; then
  echo "RESULT: NONCE REUSED -- identical (d,e) revealed for two DIFFERENT messages"
elif [ -n "$NONCE_MP" ]; then
  echo "RESULT: valA revealed a DIFFERENT nonce for m' (no reuse)"
else
  echo "RESULT: valA did NOT reveal a nonce for m' after restore"
fi
