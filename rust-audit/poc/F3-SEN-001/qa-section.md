## QA (QA3-SEN, 2026-10-08)

Result: Reproduced, end to end.

Route: the real `sentinel` binary in Direct mode (no `executor`), against a local Anvil carrying the real Gnosis USDC.e proxy+implementation code and its 14 cached storage slots (`~/.foundry/cache/rpc/xdai/48635154`, code hashes proxy `ad30d8...`, impl `aada1fd...`) and a `SentinelOracle` deployed with the Aegis production parameters (fee 400000, bond 2000x, slash 1x, commit 6, reveal 6, USDC.e fee token). One proposal is driven through a stub engine returning `secure`. The sentinel's own `approve` (gasLimit 55000) runs out of gas: receipt status 0, gasUsed 54629, allowance stays 0. The `commit` queued behind it reverts in `safeTransferFrom` (status 0, gasUsed 100202). Zero `Committed` events, `getCommitment` vote NONE, no bond in the oracle, and no reveal (a block scan of the sentinel's account found only nonce 0 `approve` and nonce 1 `commit`). A token-level probe (`probe_etch.sh`) confirms the same on the bare token: 55000 fails, 80000 succeeds with gasUsed 55437.

Command and output: `poc/F3-SEN-001/` (`e2e.sh buggy`, `out-buggy-run.txt`, `out-buggy/`; probe in `probe_approve_55000.json`/`probe_approve_80000.json`).

Bite check: raised the encoder's `ApproveToken` gas from 55_000 to 80_000 (remediation option 1), rebuilt, re-ran (`e2e.sh fixed`, `out-fixed-run.txt`). The approve now succeeds (gasUsed 55437, the finding's exact figure), the commit lands (`Committed` count 1, vote recorded), and the sentinel reveals and claims its bond plus fee. Remediation option 1 is sound. Reverted with `git checkout`. (Incidental: the fixed run's nonce 3 `finalize` reverts `RequestNotPending`, an end-to-end sighting of F3-XC-006.)

Certainty: 98 (Reproduced end to end plus Critic Confirmed; gasUsed 55437 matches the finding exactly).
