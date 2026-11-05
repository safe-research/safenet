# F3-SEN-001 PoC (QA3-SEN)

End-to-end: the real `sentinel` binary in Direct mode, against a local Anvil carrying the real Gnosis USDC.e code+storage and a `SentinelOracle` deployed with the Aegis production parameters.

## Files

- `extract_usdc.py` writes `proxy.code`, `impl.code`, `slots.txt` from `~/.foundry/cache/rpc/xdai/48635154` (code hashes: proxy `ad30d8...`, impl `aada1fd...`, matching the finding).
- `engine_stub.py` a stub sentinel engine that always returns `{"verdict":"secure"}` (the engine is a true system boundary; it does not touch the defect).
- `scan_txs.py` lists the sentinel's own transactions with receipt status and gas.
- `probe_etch.sh` a token-level probe (no sentinel): approve from zero allowance at gas 55000 vs 80000.
- `e2e.sh <label>` the full harness; outputs in `out-<label>/`.

## Commands

```
source .../env/rv-sen.sh
# token-level probe
bash rust-audit/poc/F3-SEN-001/probe_etch.sh
# buggy end-to-end (binary at 55_000)
bash rust-audit/poc/F3-SEN-001/e2e.sh buggy
# bite: edit crates/sentinel/src/service.rs gas 55_000 -> 80_000; cargo build -p sentinel
bash rust-audit/poc/F3-SEN-001/e2e.sh fixed
# revert: git checkout crates/sentinel/src/service.rs
```

## Result (buggy, `out-buggy-run.txt`)

The sentinel sent `approve` to USDC.e with gasLimit 55000: status FAILED, gasUsed 54629, allowance stayed 0. The `commit` behind it FAILED (gasUsed 100202, the `safeTransferFrom` revert). Zero `Committed` events, `getCommitment` vote NONE, no bond, no reveal (the scan found only nonce 0 and 1). The request is dropped at commitDeadline+1 (silent in logs by design).

## Bite (fixed, `out-fixed-run.txt`)

At gas 80000 the approve succeeds (gasUsed 55437, the finding's exact figure), the commit lands (`Committed` count 1), and the sentinel reveals and claims. Remediation option 1 (raise the gas limit) is sound. Incidental: nonce 3 `finalize` returns status 0 (`RequestNotPending`), an end-to-end sighting of F3-XC-006.
