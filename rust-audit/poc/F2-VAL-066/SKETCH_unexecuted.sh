#!/usr/bin/env bash
# F2-VAL-066 -- UNEXECUTED PoC sketch (QA2-VAL-B). Not run: it needs a deployed local chain
# and a validator driven into an unrecoverable driver error, which no in-process harness
# reaches (main.rs:95-98 is only exercised by a full run).
#
# Procedure (local Anvil only, port 8648; the config's rpc MUST be rewritten to loopback,
# never the sample's https://rpc.gnosischain.com):
#   1. Deploy contracts and write a validator config exactly as
#      scripts/run_validator_deep_reorg_test.sh does, but with rpc = "http://127.0.0.1:8648"
#      and [observability] metrics_address = "127.0.0.1:8649".
#   2. Start the validator, wait until it processes blocks.
#   3. Reorg deeper than max_reorg_depth (default 5): the watcher returns
#      ExceededMaxReorgDepth, driver.rs:188-192 logs "unrecoverable watcher error; exiting"
#      and breaks; main.rs:96-98 then returns Ok(()).
# Expected (the defect): `echo exit=$?` prints exit=0, and every `curl /health` issued while
# the process was retrying or exiting printed "OK".
set -euo pipefail
export PATH="$HOME/.foundry/bin:$HOME/.cargo/bin:$PATH"
CFG="${CFG:?path to a loopback validator.toml (rpc = http://127.0.0.1:8648)}"
grep -n '^rpc' "$CFG"                                    # print the effective rpc; must be loopback
anvil --port 8648 --block-time 1 >/dev/null 2>&1 & ANVIL=$!
trap 'kill $ANVIL 2>/dev/null || true' EXIT
# (deployment + config steps from scripts/run_validator_deep_reorg_test.sh go here)
"${CARGO_TARGET_DIR:-target}/debug/validator" --config-file "$CFG" & VAL=$!
sleep 30
curl -s http://127.0.0.1:8649/health; echo                # expected: OK
cast rpc --rpc-url http://127.0.0.1:8648 anvil_reorg 10 '[]'
sleep 5
curl -s http://127.0.0.1:8649/health || echo "(listener gone)"; echo
set +e; wait "$VAL"; echo "validator exit=$?"            # expected: 0  <- F2-VAL-066
