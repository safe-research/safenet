#!/bin/bash
# CA3 (2026-10-09) variant of reorg_oracle_poc.sh: the oracle is a real SentinelOracle with the
# Aegis windows (commit 6, reveal 6, bond 2000x, slash 1x, governance delay 51840, arbitration
# timeout 483840) instead of SimpleOracle, and the validators run with the Aegis default
# max_reorg_depth 5 (signing_timeout 6 and oracle_timeout 24 are the code defaults).
#
# Branch A: sentinel S1 commits approve at about T+1, nobody else commits by the commit deadline
# T+6, S1 reveals at T+7 and its reveal (the last one) finalizes the request: OracleResult(true).
# The validators commit nonces and broadcast their signShareWithCallback shares.
# Branch B (anvil_reorg back to T+5): S2's deny commit lands in block T+6, the last block of the
# commit window, so on B both sentinels reveal at T+7 and the request freezes (DisputeTriggered,
# no OracleResult). The validators' already-broadcast shares are resent by their transaction
# queue, complete the group signature on B and attest the frozen transaction.
#
# The two sentinels are scripted with cast (their votes are inputs, inside A2); the validators,
# FROSTCoordinator, Consensus and SentinelOracle are the real code. No TMPDIR removal: logs stay.
set -euo pipefail

ANVIL_PORT="${POC_ANVIL_PORT:-31400}"
ANVIL_RPC_URL="http://127.0.0.1:$ANVIL_PORT"
CHAIN_ID=31337
BLOCK_TIME="${BLOCK_TIME:-2}"
TIMEOUT="${TIMEOUT:-180}"
MAX_REORG_DEPTH=5
BLOCKS_PER_EPOCH=1000000
COMMIT_WINDOW=6
REVEAL_WINDOW=6
OUT_DIR="${OUT_DIR:?set OUT_DIR}"

PARTICIPANTS=(
    0x70997970C51812dc3A010C7d01b50e0d17dc79C8
    0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC
)
PRIVATE_KEYS=(
    0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d
    0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a
)
SENDER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
SENDER_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
# Sentinels: Anvil accounts 3 and 4.
S1=0x90F79bf6EB2c4f870365E785982E1f101E93b906
S1_KEY=0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6
S2=0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65
S2_KEY=0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a

source "$WT/scripts/lib/shared_test_scripts.sh"
mkdir -p "$OUT_DIR"
TMPDIR="$OUT_DIR"
PIDS=()
EXIT_MESSAGE="FAILURE: interrupted"
cleanup() {
    local pid
    for pid in "${PIDS[@]:-}"; do kill "$pid" 2>/dev/null || true; done
    echo "$EXIT_MESSAGE"
}
trap cleanup EXIT
require_commands anvil cast forge jq
test -x "$REPO_ROOT/target/debug/validator"
R="$ANVIL_RPC_URL"
blk() { cast block-number --rpc-url "$R"; }
H2N='def h2n: ltrimstr("0x") | explode | reduce .[] as $c (0; . * 16 + (if $c >= 97 then $c - 87 elif $c >= 65 then $c - 55 else $c - 48 end));'
logs_from() { cast logs --json --rpc-url "$R" --from-block "$1" --to-block latest --address "$2" "$3"; }

echo "==> Anvil on $ANVIL_PORT, ${BLOCK_TIME}s blocks"
anvil --block-time "$BLOCK_TIME" --port "$ANVIL_PORT" --prune-history 2000 > "$TMPDIR/anvil.log" 2>&1 &
PIDS+=("$!")
for _ in $(seq 1 40); do cast block-number --rpc-url "$R" >/dev/null 2>&1 && break; sleep 0.25; done

PARTICIPANTS_CSV=$(IFS=,; echo "${PARTICIPANTS[*]}")
deploy_validator_contracts "$R" "$SENDER" "$PARTICIPANTS_CSV" "$CHAIN_ID" > "$TMPDIR/deploy_validator.log" 2>&1
echo "    coordinator $COORDINATOR_ADDR consensus $CONSENSUS_ADDR"

echo "==> Fee token (MyToken) and SentinelOracle with the Aegis parameters"
env FACTORY=2 forge script --root "$REPO_ROOT/contracts" DeployERC20Script --rpc-url "$R" \
    --unlocked --sender "$SENDER" --broadcast > "$TMPDIR/deploy_erc20.log" 2>&1
FEE_TOKEN=$(jq -er '.returns.erc20.value' "$REPO_ROOT/contracts/build/broadcast/DeployERC20.s.sol/$CHAIN_ID/run-latest.json")
env FACTORY=2 \
    SENTINEL_ARBITRATOR="$SENDER" SENTINEL_GOVERNANCE="$SENDER" \
    SENTINEL_PROTOCOL_FUNDS_RECEIVER="$SENDER" SENTINEL_CONSENSUS="$CONSENSUS_ADDR" \
    SENTINEL_FEE_TOKEN="$FEE_TOKEN" \
    SENTINEL_REQUEST_FEE=400000 SENTINEL_COMMIT_WINDOW=$COMMIT_WINDOW SENTINEL_REVEAL_WINDOW=$REVEAL_WINDOW \
    SENTINEL_GOVERNANCE_DELAY=51840 SENTINEL_BOND_MULTIPLIER=2000 \
    SENTINEL_INITIAL_SLASHING_MULTIPLIER=1 SENTINEL_INITIAL_DAO_FEE_SHARE=0 \
    SENTINEL_ARBITRATION_TIMEOUT=483840 SENTINEL_CHARTER_ENS=charter.safenet-gov.eth \
    forge script --root "$REPO_ROOT/contracts" DeploySentinelOracleScript --rpc-url "$R" \
    --unlocked --sender "$SENDER" --broadcast > "$TMPDIR/deploy_oracle.log" 2>&1
ORACLE=$(jq -er '.returns.sentinelOracle.value' "$REPO_ROOT/contracts/build/broadcast/DeploySentinelOracle.s.sol/$CHAIN_ID/run-latest.json")
echo "    fee token $FEE_TOKEN"
echo "    SentinelOracle $ORACLE fee=$(cast call --rpc-url "$R" "$ORACLE" 'fee()(uint96)' | awk '{print $1}') bondMultiplier=$(cast call --rpc-url "$R" "$ORACLE" 'bondMultiplier()(uint32)' | awk '{print $1}')"

for s in "$S1" "$S2"; do
    cast send --rpc-url "$R" --unlocked --from "$SENDER" "$ORACLE" 'addSentinel(address)' "$s" >/dev/null
    cast send --rpc-url "$R" --unlocked --from "$SENDER" "$FEE_TOKEN" 'transfer(address,uint256)' "$s" 1000000000000000000000 >/dev/null
done
cast send --rpc-url "$R" --private-key "$S1_KEY" "$FEE_TOKEN" 'approve(address,uint256)' "$ORACLE" 1000000000000000000000 >/dev/null
cast send --rpc-url "$R" --private-key "$S2_KEY" "$FEE_TOKEN" 'approve(address,uint256)' "$ORACLE" 1000000000000000000000 >/dev/null
cast send --rpc-url "$R" --unlocked --from "$SENDER" "$FEE_TOKEN" 'approve(address,uint256)' "$ORACLE" 1000000000000000000000 >/dev/null
echo "==> Mining the 51840-block governance delay"
cast rpc --rpc-url "$R" anvil_mine 0xca80 >/dev/null
START_BLOCK=$(blk)
echo "    head $START_BLOCK, S1 active at $(cast call --rpc-url "$R" "$ORACLE" 'sentinelActiveAt(address)(uint256)' "$S1" | awk '{print $1}')"

validator_config() {
    print_validator_config_base \
        "$R" "$1" "$2" "$CONSENSUS_ADDR" "$ORACLE" \
        "$BLOCKS_PER_EPOCH" "$((BLOCK_TIME * 1000))" PARTICIPANTS | sed "s/^start_block = 0\$/start_block = $START_BLOCK/"
    echo "max_reorg_depth = $MAX_REORG_DEPTH"
}
validator_config "${PRIVATE_KEYS[0]}" "$TMPDIR/a.sqlite" > "$TMPDIR/validator_a.toml"
validator_config "${PRIVATE_KEYS[1]}" "$TMPDIR/b.sqlite" > "$TMPDIR/validator_b.toml"
grep -q "start_block = $START_BLOCK" "$TMPDIR/validator_a.toml"
echo "==> Starting validators (max_reorg_depth $MAX_REORG_DEPTH, default signing_timeout/oracle_timeout)"
run_rust_process validator "$TMPDIR/validator_a.toml" "$TMPDIR/va.log"
run_rust_process validator "$TMPDIR/validator_b.toml" "$TMPDIR/vb.log"
sleep 1
assert_processes_alive "FAILURE: a validator exited during startup." "${PIDS[@]}"
trigger_genesis_keygen "$R" "$SENDER" "$PARTICIPANTS_CSV" "$COORDINATOR_ADDR" > "$TMPDIR/genesis.log" 2>&1

TRUE_WORD=0000000000000000000000000000000000000000000000000000000000000001
echo "==> Waiting for genesis confirmation"
DEADLINE=$((SECONDS + TIMEOUT)); CONFIRMED=0; GENESIS_GROUP=""
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    CONF=$(logs_from "$START_BLOCK" "$COORDINATOR_ADDR" 'KeyGenConfirmed(bytes32,address,bool)')
    GENESIS_GROUP=$(jq -r '.[0].topics[1] // empty' <<< "$CONF")
    if [ -n "$GENESIS_GROUP" ]; then
        CONFIRMED=$(jq --arg g "$GENESIS_GROUP" --arg t "$TRUE_WORD" '[.[]|select(.topics[1]==$g and (.data|endswith($t)))]|length' <<< "$CONF")
        [ "$CONFIRMED" -ge 1 ] && break
    fi
    assert_processes_alive "FAILURE: a validator exited before genesis." "${PIDS[@]}"
    sleep 1
done
[ "$CONFIRMED" -ge 1 ] || { EXIT_MESSAGE="TIMEOUT: genesis not confirmed."; exit 1; }
echo "    genesis group $GENESIS_GROUP"

echo "==> Proposing an oracle-backed transaction (oracle = SentinelOracle)"
P0=$(blk)
env CONSENSUS_ADDRESS="$CONSENSUS_ADDR" ORACLE_ADDRESS="$ORACLE" \
    TX_CHAIN_ID="$CHAIN_ID" TX_SAFE="$SENDER" TX_TO="$SENDER" TX_NONCE=1 \
    forge script --root "$REPO_ROOT/contracts" ProposeTransactionScript \
    --rpc-url "$R" --unlocked --sender "$SENDER" --broadcast > "$TMPDIR/propose.log" 2>&1
NR=$(logs_from "$P0" "$ORACLE" 'NewRequest(bytes32,address,uint96,uint96,uint24,uint96,uint64,uint64)')
REQ=$(jq -r '.[-1].topics[1]' <<< "$NR")
T=$(( $(jq -r '.[-1].blockNumber' <<< "$NR") ))
CD=$(( T + COMMIT_WINDOW )); RD=$(( CD + REVEAL_WINDOW )); F=$(( T + COMMIT_WINDOW - 1 ))
SIGN=$(logs_from "$P0" "$COORDINATOR_ADDR" 'Sign(address,bytes32,bytes32,bytes32,uint64)')
# Pick the Sign event of this proposal by its message (topic 3), which is the oracle request id.
MESSAGE=$(jq -r --arg r "$REQ" '[.[]|select(.topics[3]==$r)][0].topics[3] // empty' <<< "$SIGN")
SID=$(jq -r --arg r "$REQ" '[.[]|select(.topics[3]==$r)][0].data[2:66] // empty' <<< "$SIGN")
[ -n "$SID" ] && SID="0x$SID"
echo "    Sign events since the proposal start: $(jq length <<< "$SIGN")"
[ "$MESSAGE" = "$REQ" ] || { EXIT_MESSAGE="FAILURE: no Sign event for request $REQ"; exit 1; }
echo "    request $REQ proposed in block T=$T; commitDeadline $CD, revealDeadline $RD; fork block T+5=$F"
echo "    signature id $SID"

SALT1=0x$(openssl rand -hex 32); SALT2=0x$(openssl rand -hex 32); REASON2="R-2.1"
H1=$(cast call --rpc-url "$R" "$ORACLE" 'hashCommitment(address,bytes32,bool,bytes32,string)(bytes32)' "$S1" "$REQ" true "$SALT1" "")
H2=$(cast call --rpc-url "$R" "$ORACLE" 'hashCommitment(address,bytes32,bool,bytes32,string)(bytes32)' "$S2" "$REQ" false "$SALT2" "$REASON2")
echo "==> S1 commits approve"
S1C=$(cast send --rpc-url "$R" --private-key "$S1_KEY" "$ORACLE" 'commit(bytes32,bytes32)' "$REQ" "$H1" --json)
echo "    S1 commit block $(( $(jq -r .blockNumber <<< "$S1C") )) (T+$(( $(jq -r .blockNumber <<< "$S1C") - T ))) status $(jq -r .status <<< "$S1C")"
N1=$(cast nonce --rpc-url "$R" "$S1"); N2=$(cast nonce --rpc-url "$R" "$S2")
GAS=(--gas-limit 400000 --gas-price 20gwei --priority-gas-price 1gwei)
RAW_S1_REVEAL=$(cast mktx --rpc-url "$R" --private-key "$S1_KEY" --nonce "$N1" "${GAS[@]}" "$ORACLE" 'reveal(bytes32,bool,bytes32,string)' "$REQ" true "$SALT1" "")
RAW_S2_COMMIT=$(cast mktx --rpc-url "$R" --private-key "$S2_KEY" --nonce "$N2" "${GAS[@]}" "$ORACLE" 'commit(bytes32,bytes32)' "$REQ" "$H2")
RAW_S2_REVEAL=$(cast mktx --rpc-url "$R" --private-key "$S2_KEY" --nonce "$((N2 + 1))" "${GAS[@]}" "$ORACLE" 'reveal(bytes32,bool,bytes32,string)' "$REQ" false "$SALT2" "$REASON2")

echo "==> Branch A: waiting for the commit deadline block $CD (mined without S2's commit), then S1 reveals"
while [ "$(blk)" -lt "$CD" ]; do sleep 0.2; done
cast publish --rpc-url "$R" "$RAW_S1_REVEAL" >/dev/null
DEADLINE=$((SECONDS + TIMEOUT)); O=""
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    ORA=$(logs_from "$T" "$ORACLE" 'OracleResult(bytes32,address,bytes,bool)' | jq --arg r "$REQ" '[.[]|select(.topics[1]==$r)]')
    if [ "$(jq length <<< "$ORA")" -ge 1 ]; then O=$(( $(jq -r '.[0].blockNumber' <<< "$ORA") )); break; fi
    sleep 0.3
done
[ -n "$O" ] || { EXIT_MESSAGE="TIMEOUT: no OracleResult on branch A."; exit 1; }
echo "    branch A: OracleResult(approved=$(jq -r '.[0].data[66:130]|test("1$")' <<< "$ORA")) in block o=$O (T+$((O - T)))"

echo "==> Waiting for both validators' SignShared on branch A"
DEADLINE=$((SECONDS + TIMEOUT)); SHARES=0
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    SS=$(logs_from "$T" "$COORDINATOR_ADDR" 'SignShared(bytes32,bytes32,address,uint256)')
    SHARES=$(jq --arg s "$SID" '[.[]|select(.topics[1]==$s)]|length' <<< "$SS")
    [ "$SHARES" -ge 2 ] && break
    assert_processes_alive "FAILURE: a validator exited on branch A." "${PIDS[@]}"
    sleep 0.2
done
[ "$SHARES" -ge 2 ] || { EXIT_MESSAGE="TIMEOUT: shares not broadcast on branch A."; exit 1; }
cast rpc --rpc-url "$R" evm_setIntervalMining 0 >/dev/null
H=$(blk)
SHARE_BLOCKS=$(jq -r --arg s "$SID" "$H2N"'[.[]|select(.topics[1]==$s)|.blockNumber|h2n]|map(tostring)|join(",")' <<< "$SS")
BRANCH_A_ATTESTED=$(cast call --rpc-url "$R" "$CONSENSUS_ADDR" 'getAttestationSignatureId(bytes32)(bytes32)' "$MESSAGE")
jq --arg s "$SID" "$H2N"'[.[]|select(.topics[1]==$s)|{root:.topics[2],participant:("0x"+.data[26:66]),z:("0x"+.data[66:130]),block:(.blockNumber|h2n)}]' <<< "$SS" > "$TMPDIR/branchA_shares.json"
DEPTH=$(( H - F ))
echo "    branch A: shares mined in blocks $SHARE_BLOCKS; head $H; attestation id on A $BRANCH_A_ATTESTED"
if [ "$DEPTH" -gt "$MAX_REORG_DEPTH" ]; then
    EXIT_MESSAGE="INCONCLUSIVE: the reorg would need depth $DEPTH > max_reorg_depth $MAX_REORG_DEPTH (timing slipped)"; exit 2
fi

echo "==> Reorg of depth $DEPTH (blocks $((F + 1))..$H replaced): S2's deny commit in block T+6=$((F + 1)), both reveals in T+7"
cast rpc --rpc-url "$R" anvil_reorg "$DEPTH" "[[\"$RAW_S2_COMMIT\", 0], [\"$RAW_S1_REVEAL\", 1], [\"$RAW_S2_REVEAL\", 1]]" >/dev/null
cast rpc --rpc-url "$R" evm_setIntervalMining "$BLOCK_TIME" >/dev/null
echo "    head after reorg $(blk)"

echo "==> Waiting for the validators' resent shares to attest on the canonical branch"
DEADLINE=$((SECONDS + TIMEOUT)); ZERO=0x0000000000000000000000000000000000000000000000000000000000000000; CANON_ATTEST=$ZERO
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    CANON_ATTEST=$(cast call --rpc-url "$R" "$CONSENSUS_ADDR" 'getAttestationSignatureId(bytes32)(bytes32)' "$MESSAGE")
    [ "$CANON_ATTEST" != "$ZERO" ] && break
    assert_processes_alive "FAILURE: a validator exited after the reorg." "${PIDS[@]}"
    sleep 1
done
# Let a few more blocks pass so late events show up.
sleep $((BLOCK_TIME * 2))

C_COMMITS=$(logs_from "$T" "$ORACLE" 'Committed(bytes32,address,uint96)' | jq --arg r "$REQ" '[.[]|select(.topics[1]==$r)]')
C_REVEALS=$(logs_from "$T" "$ORACLE" 'Revealed(bytes32,address,bool,uint96,string)' | jq --arg r "$REQ" '[.[]|select(.topics[1]==$r)]')
C_DISPUTE=$(logs_from "$T" "$ORACLE" 'DisputeTriggered(bytes32,uint64)' | jq --arg r "$REQ" '[.[]|select(.topics[1]==$r)]')
C_RESULT=$(logs_from "$T" "$ORACLE" 'OracleResult(bytes32,address,bytes,bool)' | jq --arg r "$REQ" '[.[]|select(.topics[1]==$r)]')
C_ATTESTED=$(logs_from "$T" "$CONSENSUS_ADDR" 'TransactionAttested(bytes32,bytes32,address,uint64,bytes32,bytes32,((uint256,uint256),uint256))')
REQ_STATE=$(cast call --rpc-url "$R" "$ORACLE" 'getRequest(bytes32)(((uint64,uint24,uint64,uint96,uint8,address,uint96),(uint8,uint96,uint64,uint16,uint16,uint16,uint16,uint24)))' "$REQ")
STATE_NUM=$(sed -E 's/.*\), \(([0-9]+),.*/\1/' <<< "$REQ_STATE")
COMPLETED=$(logs_from "$T" "$COORDINATOR_ADDR" 'SignCompleted(bytes32,bytes32,((uint256,uint256),uint256))' | jq --arg s "$SID" '[.[]|select(.topics[1]==$s)][-1]')
COMPLETED_ROOT=$(jq -r '.topics[2]' <<< "$COMPLETED"); COMPLETED_Z=$(jq -r '"0x"+.data[130:194]' <<< "$COMPLETED")
SUM_Z=$(python3 -I -c '
import json, sys
N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
s = sum(int(x["z"], 16) for x in json.load(open(sys.argv[1]))) % N
c = int(sys.argv[2], 16) if sys.argv[2] not in ("", "0x", "null") else -1
print(hex(s), "MATCH" if s == c else "MISMATCH")' "$TMPDIR/branchA_shares.json" "$COMPLETED_Z")

echo
echo "=== CANONICAL CHAIN (branch B) ==="
echo "Committed for request         : $(jq length <<< "$C_COMMITS") ($(jq -r "$H2N"'[.[]|("0x"+.topics[2][26:66])+"@"+((.blockNumber|h2n)|tostring)]|join(" ")' <<< "$C_COMMITS"))"
echo "Revealed for request          : $(jq length <<< "$C_REVEALS") ($(jq -r "$H2N"'[.[]|("0x"+.topics[2][26:66])+"@"+((.blockNumber|h2n)|tostring)]|join(" ")' <<< "$C_REVEALS"))"
echo "DisputeTriggered for request  : $(jq length <<< "$C_DISPUTE") (block $(jq -r "$H2N"'[.[]|.blockNumber|h2n]|map(tostring)|join(",")' <<< "$C_DISPUTE"))"
echo "OracleResult for request      : $(jq length <<< "$C_RESULT")"
echo "request state (2 = FROZEN)    : $STATE_NUM"
echo "TransactionAttested events    : $(jq length <<< "$C_ATTESTED") (block $(jq -r "$H2N"'[.[]|.blockNumber|h2n]|map(tostring)|join(",")' <<< "$C_ATTESTED"))"
echo "attestation id for message    : $CANON_ATTEST"
echo "branch-A shares               : $(jq -r '[.[]|.participant+"@"+(.block|tostring)]|join(" ")' "$TMPDIR/branchA_shares.json")"
echo "SignCompleted root (canonical): $COMPLETED_ROOT"
echo "sum(branch-A z_i) vs canonical z: $SUM_Z"
echo "validator log lines:"
for v in va vb; do
    echo "  [$v] $(grep -c 'unexpected signing state' "$TMPDIR/$v.log" || true) x 'unexpected signing state'; $(grep -ci 'uncle\|reorg' "$TMPDIR/$v.log" || true) uncle/reorg lines; $(grep -ci 'exceeded\|deep reorg' "$TMPDIR/$v.log" || true) deep-reorg lines"
done
assert_processes_alive "FAILURE: a validator exited by the end." "${PIDS[@]}"

if [ "$CANON_ATTEST" != "$ZERO" ] && [ "$(jq length <<< "$C_DISPUTE")" -ge 1 ] && [ "$(jq length <<< "$C_RESULT")" -eq 0 ] && [ "$STATE_NUM" = "2" ]; then
    EXIT_MESSAGE="REPRODUCED: the canonical chain attested a transaction whose SentinelOracle request is FROZEN (DisputeTriggered, no OracleResult); reorg depth $DEPTH within max_reorg_depth $MAX_REORG_DEPTH."
    exit 0
fi
EXIT_MESSAGE="NOT REPRODUCED: attest=$CANON_ATTEST dispute=$(jq length <<< "$C_DISPUTE") results=$(jq length <<< "$C_RESULT") state=$STATE_NUM"
exit 1
