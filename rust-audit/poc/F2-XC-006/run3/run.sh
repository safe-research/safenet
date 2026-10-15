#!/usr/bin/env bash
# rv-xc run 3 re-run of the F2-XC-006 PoC (canonical row F-XC-006): a store written on chain A is
# resumed on chain B after an `rpc` edit, and the request queued on A is signed for and mined on B.
# Prerequisite: poc/F2-XC-050/run3/qa2_xc_state_tests_run3.rs pasted into the `mod tests` block of
# crates/core/src/state/mod.rs in the checkout this runs from (reverted afterwards by the caller).
#
# usage (from the worktree, after sourcing the rv-xc env file): run.sh <scratch-dir>
# Two local Anvil nodes on POC_ANVIL_PORT and POC_ANVIL_PORT+1, loopback only, started and killed here.
set -u
SCRATCH=${1:?scratch dir}
PA=${POC_ANVIL_PORT:?source the env file}
PB=$((PA + 1))
ME=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 # Anvil default account 0 (public test key)
mkdir -p "$SCRATCH"

anvil --host 127.0.0.1 --port "$PA" --chain-id 100 --quiet > "$SCRATCH/anvil-a.log" 2>&1 &
A=$!
anvil --host 127.0.0.1 --port "$PB" --chain-id 11155111 --timestamp 1600000000 --quiet > "$SCRATCH/anvil-b.log" 2>&1 &
B=$!
trap 'kill "$A" "$B" 2>/dev/null; wait "$A" "$B" 2>/dev/null' EXIT

for _ in $(seq 1 50); do
  cast block-number --rpc-url "http://127.0.0.1:$PA" > /dev/null 2>&1 &&
    cast block-number --rpc-url "http://127.0.0.1:$PB" > /dev/null 2>&1 && break
  sleep 0.2
done
echo "# anvil pids: A=$A (port $PA) B=$B (port $PB); listeners:"
ss -ltnp 2>/dev/null | grep -E ":($PA|$PB) " | sed 's/^/#   /'
ss -ltnp 2>/dev/null | grep -E ":$PA " | grep -q "pid=$A," || { echo "!! port $PA is not owned by pid $A"; exit 3; }
ss -ltnp 2>/dev/null | grep -E ":$PB " | grep -q "pid=$B," || { echo "!! port $PB is not owned by pid $B"; exit 3; }

cast rpc --rpc-url "http://127.0.0.1:$PA" anvil_mine 10 > /dev/null
cast rpc --rpc-url "http://127.0.0.1:$PB" anvil_mine 20 > /dev/null
echo "# chain A: chain id $(cast chain-id --rpc-url "http://127.0.0.1:$PA"), block $(cast block-number --rpc-url "http://127.0.0.1:$PA")"
echo "# chain B: chain id $(cast chain-id --rpc-url "http://127.0.0.1:$PB"), block $(cast block-number --rpc-url "http://127.0.0.1:$PB")"
echo "# command: QA2_XC_DIR=<scratch> QA2_XC_RPC_A=http://127.0.0.1:$PA QA2_XC_RPC_B=http://127.0.0.1:$PB cargo test --offline --locked -p safenet-core --lib state::tests::qa2_xc_006 -- --nocapture"
QA2_XC_DIR="$SCRATCH" QA2_XC_RPC_A="http://127.0.0.1:$PA" QA2_XC_RPC_B="http://127.0.0.1:$PB" \
  cargo test --offline --locked -p safenet-core --lib state::tests::qa2_xc_006 -- --nocapture 2>&1 |
  grep -v '^\s*Compiling'
echo "# exit=${PIPESTATUS[0]}"

echo "=================== chain B ($PB) after the test: block $(cast block-number --rpc-url "http://127.0.0.1:$PB"); signer nonce $(cast nonce "$ME" --rpc-url "http://127.0.0.1:$PB")"
echo "=================== the latest block on chain B, full transactions: block / chainId / nonce / from / to / input"
cast rpc --rpc-url "http://127.0.0.1:$PB" eth_getBlockByNumber latest true |
  jq -c '.transactions[] | {block: .blockNumber, chainId, nonce, from, to, input}'
echo "=================== chain A ($PA): block $(cast block-number --rpc-url "http://127.0.0.1:$PA"); signer nonce $(cast nonce "$ME" --rpc-url "http://127.0.0.1:$PA") (the request was never broadcast on the chain it was queued for)"
