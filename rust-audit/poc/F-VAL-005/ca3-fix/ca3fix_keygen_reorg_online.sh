#!/bin/bash
# CA3-FIX (2026-10-09): F-VAL-005 trail, all validators online. Two Rust validators run genesis;
# key generation for epoch 1 starts on the final genesis KeyGenConfirmed log. Once both epoch-1
# commitments are mined, the chain is reorged back to before the final genesis confirmation
# (`anvil_reorg` drops the reorged transactions; each validator's queue re-sends its own). The
# epoch-1 group must still be confirmed, and no validator may log a mismatch or a skipped epoch.
# Run inside `integ`. Usage: ca3fix_keygen_reorg_online.sh <worktree> <runs>
set -euo pipefail
WT=$1
RUNS=${2:-3}
ANVIL_PORT=8547
ANVIL_RPC_URL="http://127.0.0.1:$ANVIL_PORT"
CHAIN_ID=31337
BLOCK_TIME=1
MAX_REORG_DEPTH=10
PARTICIPANTS=(0x70997970C51812dc3A010C7d01b50e0d17dc79C8 0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC)
PRIVATE_KEYS=(0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d 0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a)
SENDER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
TRUE_WORD=0000000000000000000000000000000000000000000000000000000000000001
source "$WT/scripts/lib/shared_test_scripts.sh"
require_commands anvil cast forge jq cargo
PARTICIPANTS_CSV=$(IFS=,; echo "${PARTICIPANTS[*]}")
OUTDIR=${OUTDIR:-$WT}

one_run() {
    local run=$1
    TMPDIR="$(mktemp -d)"
    PIDS=()
    start_anvil "$BLOCK_TIME" "$ANVIL_PORT" "$OUTDIR/ca3fix_online_anvil_$run.txt" "$ANVIL_RPC_URL"
    deploy_validator_contracts "$ANVIL_RPC_URL" "$SENDER" "$PARTICIPANTS_CSV" "$CHAIN_ID" > "$TMPDIR/deploy.log" 2>&1
    local i
    for i in 0 1; do
        { print_validator_config_base "$ANVIL_RPC_URL" "${PRIVATE_KEYS[$i]}" "$TMPDIR/v$i.sqlite" "$CONSENSUS_ADDR" "$ORACLE_ADDR" \
            1000000 "$((BLOCK_TIME * 1000))" PARTICIPANTS; echo "max_reorg_depth = $MAX_REORG_DEPTH"; } > "$TMPDIR/v$i.toml"
        run_rust_process validator "$TMPDIR/v$i.toml" "$OUTDIR/ca3fix_online_v${i}_$run.txt"
    done
    sleep 0.5
    trigger_genesis_keygen "$ANVIL_RPC_URL" "$SENDER" "$PARTICIPANTS_CSV" "$COORDINATOR_ADDR" > "$TMPDIR/genesis.log" 2>&1

    local deadline=$((SECONDS + 90)) genesis="" confirmed="" e1gid="" e1count=0
    while [ "$SECONDS" -lt "$deadline" ]; do
        confirmed=$(fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" 'KeyGenConfirmed(bytes32,address,bool)')
        genesis=$(jq -r --arg t "$TRUE_WORD" '[.[] | select(.data | endswith($t))][0].topics[1] // empty' <<< "$confirmed")
        if [ -n "$genesis" ]; then
            local commits
            commits=$(fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" \
                'KeyGenCommitted(bytes32,address,((uint256,uint256),((uint256,uint256),uint256),(uint256,uint256)[],((uint256,uint256),uint256)),bool)')
            e1gid=$(jq -r --arg g "$genesis" '[.[] | select(.topics[1] != $g)][0].topics[1] // empty' <<< "$commits")
            e1count=$(jq --arg g "$genesis" '[.[] | select(.topics[1] != $g)] | length' <<< "$commits")
            [ "$e1count" -ge 2 ] && break
        fi
        sleep 0.5
    done
    if [ "$e1count" -lt 2 ]; then echo "run $run: setup timeout"; return 1; fi
    local final_confirm current depth
    final_confirm=$(jq -r --arg g "$genesis" '[.[] | select(.topics[1] == $g)] | map(.blockNumber) | .[]' <<< "$confirmed" | max_block)
    current=$(cast block-number --rpc-url "$ANVIL_RPC_URL")
    depth=$((current - final_confirm + 1))
    echo "run $run: genesis $genesis final confirmation at $final_confirm; epoch-1 group $e1gid has $e1count commitments; head $current; reorg depth $depth"
    if [ "$depth" -gt "$MAX_REORG_DEPTH" ]; then echo "run $run: depth too large"; return 1; fi
    cast rpc anvil_reorg "$depth" '[]' --rpc-url "$ANVIL_RPC_URL" > /dev/null

    local ok=0
    deadline=$((SECONDS + 150))
    while [ "$SECONDS" -lt "$deadline" ]; do
        local n
        n=$(fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" 'KeyGenConfirmed(bytes32,address,bool)' |
            jq --arg g "$e1gid" --arg t "$TRUE_WORD" '[.[] | select(.topics[1] == $g) | select(.data | endswith($t))] | length')
        if [ "$n" -gt 0 ]; then ok=1; break; fi
        sleep 1
    done
    local order
    order=$(fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" 'KeyGenConfirmed(bytes32,address,bool)' |
        jq -r --arg g "$genesis" '[.[] | select(.topics[1] == $g)] | map("confirm@" + .blockNumber + "/" + .logIndex) | join(" ")')
    local c2
    c2=$(fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" \
        'KeyGenCommitted(bytes32,address,((uint256,uint256),((uint256,uint256),uint256),(uint256,uint256)[],((uint256,uint256),uint256)),bool)' |
        jq -r --arg g "$e1gid" '[.[] | select(.topics[1] == $g)] | map("commit(" + .data[26:34] + ")@" + .blockNumber + "/" + .logIndex) | join(" ")')
    echo "run $run: after the reorg, genesis $order; epoch-1 $c2"
    for i in 0 1; do
        echo "run $run: validator $i timed-out/skip lines: $(grep -c 'timed out\|skipping to next epoch\|commitment is incorrect' "$OUTDIR/ca3fix_online_v${i}_$run.txt" || true)"
    done
    echo "run $run: epoch-1 group confirmed after the reorg = $ok"
    for pid in "${PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
    wait 2>/dev/null || true
}

for r in $(seq 1 "$RUNS"); do
    one_run "$r" || true
done
EXIT_MESSAGE="done"
