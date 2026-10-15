# C3-SEN coverage log (run 3, Critic, discovery track)

| Field | Value |
| --- | --- |
| Critic | C3-SEN, auditor department (Opus 5.5, max effort) |
| Assignment | `F3-SEN-001` to `004`, `F3-XC-001`, `F3-XC-002`, `F3-XC-006`; rejected lists of R7 and R8 (sentinel parts); promotion range `F3-SEN-050` to `059` |
| Audit commit | `361aaf3` (main checkout HEAD, `crates/` clean; shared worktree `<run folder>/wt/base` at the same commit) |
| Date | 2026-10-08 |
| Brief | `<run folder>/briefs/critic-shared.md` |

`<run folder>` is the local run folder.

## Independence

- Read: the brief; `rust-audit/PROMPT.md` (all); `state/run3/settled.md`; `state/run3/baseline.md` (with its section 9 correction); `state/run3/map.md` by grep for the sentinel known items; my seven assigned `F3-*` files; `state/run3/agents/R7.md` and `R8.md`; `state/run3/logs/R7-usdc-approve-gas.txt`, `R7-commit-undercount.txt`, `R7-restart-replay.txt`, `R8-sentinel-bond-poc.txt`; from `<run folder>/gh/issues-open.json` the list of titles and the bodies of #961, #973 and #549 (filtered by number, so the PR #905 item was not printed) and `issue-549-comments.json`.
- Project context: the maintainers' project notes; the team's decisions (headings and selected entries); the maintainers' review notes by a narrow grep for the known "revealDeadline + 1" item (it is from the review of #1017).
- Not opened: `rust-audit/findings/F-*` and `F2-*`, `report/`, `poc/`, `codebase-map.md`, `analysis/`, any `state/` path outside `state/run3/`, `state/run3/revalidation/`, the `rv-*` agent logs and the `rv-*` reports, `<run folder>/scratch/canonical.tsv`, `<run folder>/scratch/rv-*.md`.
- Incidental exposure, names only: the session's starting `git status` listed modified `F-*` file names; a listing of `<run folder>/scratch/` showed other agents' folder and file names; a listing of `<run folder>/scratch/R7/` showed R7's PoC file names. Nothing of these was opened.

## Findings handled

| ID | Verdict | Severity (reviewer / critic) | Certainty | Duplicate | Main point |
| --- | --- | --- | --: | --- | --- |
| F3-SEN-001 | Confirmed | High / High | 90 | canonical | Gas threshold re-measured by a second method (isolated transactions): 55,000 fails, 55,819 needed; Direct mode is the documented Aegis path (`docs/configuration.md:106-121`, no `[transactions]` table). Conditional escalation to Critical noted (a malicious Batched-mode sentinel votes unopposed). |
| F3-SEN-002 | Confirmed | Medium / Medium | 80 | canonical | Commits applied before the engine resume are dropped; undercount leads to drop without claim or no finalize. New relative to #961 (other sentinels' commits, live path). |
| F3-XC-001 | Confirmed | High / Medium | 80 | duplicate of F3-SEN-002 | Same branch and outcomes; adds the address-set fix, the no-receipt note and the self-proposed attacker route. |
| F3-XC-006 | Confirmed | Informational / Informational | 85 | canonical | The early `Finalize` always reverts; Batched-mode reservation 263,065 gas per call. |
| F3-SEN-003 | Confirmed | Medium / Medium | 85 | canonical | Known from the review of #1017, with three real additions (unreachable `RevealMissed` recovery, amount, routine triggers). |
| F3-XC-002 | Confirmed | Medium / Medium | 85 | duplicate of F3-SEN-003 | Same guard; should carry `known`; adds the dispute path spelled out. |
| F3-SEN-004 | Confirmed | Medium / Medium | 80 | canonical | Known #961 symptom with a routine trigger (restart with about four blocks of downtime); #973's "recovered" case and its proposed fix do not cover it. |

No finding refuted; no `H` rows. Duplicate groups settled: F3-SEN-002 / F3-XC-001 (canonical F3-SEN-002, F3-XC-006 kept separate as the redundant-call record) and F3-SEN-003 / F3-XC-002 (canonical F3-SEN-003).

## Commands run

| # | Command (cwd) | Key output |
| --- | --- | --- |
| 1 | `git rev-parse HEAD`; `git status --short -- crates contracts docs scripts` (main checkout) | `361aaf3...`; only untracked contract submodule folders, `crates/` clean |
| 2 | `python3 -I -c ...` over `~/.foundry/cache/rpc/xdai/48635154` | block 48635154; proxy (1,852 bytes) and implementation (23,464 bytes) code with code hashes; 14 proxy slots (admin, implementation, pauser and paused, blacklister, others zero) |
| 3 | `python3 -I scripts/extract.py && forge test -vv --offline --root . --match-test "test_a\|test_b"` (`<run folder>/scratch/C3-SEN/usdc-gas/`, env sourced) | approval reads 3 proxy slots (admin, implementation, slot 1) and writes the allowance slot; callee gas 33,841 for the first call in a test (fully cold), 31,341 after `vm.cool` (implementation account stays warm), 25,341 warm |
| 4 | `forge test -vv --offline --root . --isolate --match-test "test_c\|test_d\|test_e"` (same folder) | no-op with 100 gas forwarded succeeds (isolated gas limit is forwarded gas plus 21,000); gas limit 55,000 (forwarded 34,000): approval fails; minimum forwarded 34,819, so minimum gas limit 55,819; a callee budget of exactly 55,000 through a forwarder succeeds |
| 5 | `cargo test -p sentinel --bins --offline -- service::tests::flow_unanimous_approve_finalizes_via_early_reveal_and_claims service::tests::oracle_result_recovers_a_slashed_bond_when_our_own_reveal_was_missed` (shared worktree) | 2 passed: today's behaviour (redundant early `Finalize`, recovery test with an impossible event order) |
| 6 | `python3 -I -c` reimplementing `call_gas` (`crates/core/src/tx/bundle.rs:196-209`) | `Finalize` 263,065; `ApproveToken` 65,482; batch base 27,088 |
| 7 | `node <npx cache>/prettier/bin/prettier.cjs --check` and the style check on the seven finding files and this log | clean (two code-span hyphen hits rephrased) |
| 8 | Greps: `git log -S'55_000' -- crates/sentinel/`, `git log -- contracts/.env.aegis.sample`, `grep receipt crates/core/src/tx/`, `grep sqlite scripts/run_sentinel_integration_test.sh` | 55,000 from #504/#540; USDC.e from #1062; no receipt handling in the queue; integration suite uses `sqlite::memory:` (`:209`) |

## Citations re-opened at `361aaf3`

- Sentinel: `crates/sentinel/src/service.rs` (read in full, 1-2214), `state.rs` (full), `effect.rs` (full), `verdicts.rs:1-140`, `hashing.rs:42-53`, `sentinel.sample.toml` (full).
- Core: `driver.rs` (full), `effects.rs:1-102`, `state/mod.rs:1-325`, `state/storage.rs:1-185`, `index/mod.rs:1-142`, `index/blocks.rs:1-420`, `tx/types.rs` (full), `tx/config.rs` (full), `tx/bundle.rs:1-210`, `tx/storage.rs:140-240`, `tx/mod.rs:55-110, 140-200`.
- Contracts (reference): `SentinelOracle.sol` (full), `libraries/SentinelOracleRequests.sol` (full), `libraries/SentinelOracleCommitments.sol` (full), `Consensus.sol:245-275`, `Safenet7702Executor.sol:85-111`, `.env.aegis.sample` (full), `script/DeployERC20.s.sol:10-20`.
- Docs: `docs/configuration.md:1-140`, `docs/sentinel-handbook.md` (grep for executor, gas, approve), `scripts/run_sentinel_integration_test.sh:81, 190, 209`.

## Rejected hypotheses re-checked (R7, R8 sentinel parts)

All stand. Spot-checked against code:

- R7 R1 (a reverted batched `Finalize` takes the `Claim` down): the executor emits `CallFailed` and continues (`contracts/src/Safenet7702Executor.sol:96-110`).
- R7 R12 (salt leaks before reveal): the salt is a key derivation from the signer key (`crates/sentinel/src/hashing.rs:49-53`).
- R7 R7 (verdict pruned while replayable) and R8 R-12 (actions lost on queue errors): `queue()` stores rows before submitting (`crates/core/src/tx/mod.rs:150-160`); verdicts and snapshots prune at the same `safe`. One narrow race remains, recorded as an observation below.
- R7 R5, R8 R-20 (allowance ordering): queue order is `ORDER BY id ASC` and Direct nonces execute in order.
- R7 R9 (`WaitingForRequest` deadline): `NewRequest` is in the proposal's transaction (`contracts/src/Consensus.sol:264-266`), and a block's logs are never split across updates (`crates/core/src/state/mod.rs:212-223`).
- R8 R-11 (overcount on re-delivered logs): reorgs restore snapshots; out-of-range updates are `BadUpdate`.
- R7 R2, R3, R4, R6, R8, R10, R11, R13 and R8 R-9, R-10, R-18, R-19, R-21: agree with the refutations as written; R7 R10's commit gas and R8 R-18's batch margin are consistent with my own numbers.

Observations reviewed and not promoted: R7 O1 (filed as F3-XC-006), O2 and O3 (A3 engine, #113), O4 (engine burst; the engine is out of scope and no bond is at risk), O5 (unreachable states), O6 and O7 (operator config, known TODO), O8 (same outcome class as F3-SEN-004, needs a 5 s SQLite stall), O9 (part of F3-SEN-002), O10 (metric), O11 (Docker hardening, R8 owns); R8 O-1 (undercount frequency, a QA question).

## Promotions

None. No rejected hypothesis or observation in R7's or R8's sentinel parts was wrongly dismissed.

## Own observations (not filed)

- Verdict prune race in a restart warp: the replayed `EngineCheck` for a proposal below the new watcher `safe` reads its verdict in a spawned task, while the next `New` update's housekeeping prunes verdicts below `safe` (`crates/sentinel/src/effect.rs:139-163`, `crates/core/src/driver.rs:292-294`). If the prune ran first, the engine would be asked again and could answer differently from the committed vote. The driver does RPC work (`update_block_status`) before that prune, so the read almost always wins. Below 40 %; QA could force it with a delayed verdict read.
- F3-SEN-001 masks F3-SEN-002, F3-SEN-003 and F3-SEN-004 on Aegis Direct-mode sentinels (no commit ever lands); they are reachable in Batched mode today and everywhere once F3-SEN-001 is fixed.

## Not settled

- How often the honest trigger of F3-SEN-002 fires depends on engine latency differences between sentinels; not measurable without a deployment. QA recipe in the finding.
- F3-SEN-001 end to end (approve receipt status 0, commit revert, drop) is code-traced; the Anvil recipe is in the finding. Both gas measurements assume Gnosis charges the Ethereum gas schedule for these opcodes (no live RPC was used to compare against a real transaction).

## Artifacts left

- `<run folder>/scratch/C3-SEN/usdc-gas/` (1.8 MB: `foundry.toml`, `scripts/extract.py`, generated `src/UsdcCode.sol`, `test/ApproveGasCheck.t.sol`, forge `out/` and `cache/`). No Rust build of my own; no tracked file touched.

## Appendix: key test functions (`test/ApproveGasCheck.t.sol`)

```solidity
// setUp: vm.etch the cached proxy and implementation code; vm.store the 14 cached proxy slots.
// Every attempt approves from a fresh owner, so the allowance starts at zero.

function test_c_isolationCalibration() public {
    Noop noop = new Noop();
    (bool ok,) = address(noop).call{gas: 100}("");
    console2.log("isolated no-op with 100 gas forwarded succeeded", ok); // true
}

function test_d_isolatedApprove() public {
    bool at55k = _approve(_fresh(), APPROVE_GAS - 21_000); // tx gas limit 55,000 under --isolate
    console2.log("isolated tx, forwarded 34,000 (tx gas limit 55,000 if +21,000) succeeded", at55k); // false
    uint256 lo = 20_000;
    uint256 hi = 120_000;
    while (lo < hi) {
        uint256 mid = (lo + hi) / 2;
        if (_approve(_fresh(), mid)) hi = mid;
        else lo = mid + 1;
    }
    console2.log("isolated: minimum forwarded gas", lo); // 34819
    console2.log("isolated: minimum tx gas limit if +21,000", lo + 21_000); // 55819
}

function test_e_isolatedBatchedBudget() public {
    Forwarder fwd = new Forwarder(); // token.call{gas: gasLimit}(data), as the executor does
    bool ok = fwd.forward(UsdcCode.PROXY, _data(), APPROVE_GAS);
    console2.log("isolated, token called with exactly 55,000 callee gas, succeeded", ok); // true
}
```
