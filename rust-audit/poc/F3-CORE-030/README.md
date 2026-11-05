# F3-CORE-030 PoC (QA3-CORE)

The real sentinel binary built at `361aaf3`, a local Anvil (1 s blocks), and `rpcproxy.py` between them. With `{"lag": n}` in its control file the proxy serves the same Anvil chain as a node `n` blocks behind: `eth_blockNumber` and `latest` answer the head minus `n`, higher block numbers answer `null`. It is the same chain with the same hashes (no fork). The sentinel needs no contracts for this; the trigger depends on block numbers only, so explicit 1 s timings replace the Gnosis ones and `max_reorg_depth` stays at its default 5.

- `f030.sh <label>`: run 1 follows the chain and stops with SIGINT; the node then lags 30 blocks; runs 2 and 3 restart against it; run 4 restarts once it has caught up (with a fix applied, run 4 instead starts while lagging and the node catches up 8 s later).
- `f030-boundary.sh <label> <lag>`: mining is frozen after run 1, the restart sees a head of exactly `S - 1` (lag 6) or `S` (lag 5), then one block is mined.

## Run

```sh
./f030.sh base && ./f030-boundary.sh base 6 && ./f030-boundary.sh base 5
git apply remediation1.patch && cargo build -p sentinel && ./f030.sh fix && ./f030-boundary.sh fix 6
```

## Files

- `sentinel-restart-at-361aaf3.txt`, `sentinel-boundary-at-361aaf3.txt`, `sentinel-restart-with-remediation1.txt`
- `sentinel.toml` (run config), `procs.sh` (process helpers), `rpcproxy.py`
