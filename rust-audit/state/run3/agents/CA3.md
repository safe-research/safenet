# CA3 coverage log (phase 4, final confidence)

Agent: CA3 (confidence-assigner), 2026-10-09, after PC3. Audit commit `361aaf3`. Worktrees `wt/rv-sen` (env `rv-sen.sh`, Anvil 31500 to 31509) and `wt/rv-val` (env `rv-val.sh`, Anvil 31400 to 31409). The only agent running. Both worktrees ended with no tracked changes; no Anvil, sentinel, validator, engine or proxy of mine is left running (checked with a script-file process list, since `pgrep -f` in the calling shell matched and killed the shell once).

## Read

Brief `briefs/phase4-shared.md`; `PROMPT.md` sections 1 to 12; `state/run3/settled.md`; `state/run3/phase4/PC3.md`, `PC3` report, `state/run3/phase4/CA3-FIX.md`; QA logs `QA3-SEN`, `QA3-VAL`, `QA3-CORE`, `QA3-2A`, `QA3-2B`; the 33 in-scope finding files in full or by section (claim, trigger, Critic, QA, second reproduction, triage, PC3) and the 7 duplicate files' headers; `report/RECONCILIATION.md` (known-tag precedent); `docs/configuration.md:80-121`, `docs/validator-handbook.md:118-128`, `docs/sentinel-handbook.md:80-90`; `contracts/src/SentinelOracle.sol:200-300, 344-350, 62-67`, `contracts/src/libraries/SentinelOracleCommitments.sol:45-124`, `SentinelOracleRequests.sol` structs, `Consensus.sol:250-268`, `FROSTParticipantMap.sol:155-166`, `Secp256k1.sol:194-195`; `crates/validator/src/config.rs:85-91`, `crates/core/src/index/blocks.rs:129-136`, `crates/sentinel/src/service.rs:477-507`.

## Builds

- `cargo build -p sentinel -p validator --locked` in both worktrees before any binary run (rv-sen rebuilt `sentinel`; rv-val rebuilt `validator` and `sentinel`, "1 input file differs": QA3-VAL's patched `sign.rs` build was still in the target directory). `cargo build -p sentinel -p sentinel-engine` before `f160.sh`.
- Pristine sentinel copied to the local run folder's `scratch/CA3/qa2b/bin/sentinel-361aaf3` for the QA3-2B harness runs.

## Commands and results (all after sourcing the worktree's env file)

| Finding | Command | Result |
| --- | --- | --- |
| F3-SEN-001 | `bash poc/F3-SEN-001/ca3/e2e.sh buggy1`, `buggy2`; `e2e-batched.sh batched1`; C3-SEN `forge test -vv --offline --isolate` | 2 of 2 out of gas and reverted commit; Batched commits and claims; 55,819 threshold |
| F-SEN-008 | `bash poc/F-SEN-008/ca3/f008_run.sh <dir> 31502 <cache> prague` | status 0 at 55,000 (54,629), status 1 at 200,000 (55,437); first attempt failed on the hardfork name `latest` |
| F3-VAL-030 | `bash poc/F3-VAL-030/ca3/reorg_oracle_poc.sh`; `OUT_DIR=… bash poc/F3-VAL-030/ca3/reorg_sentinel_oracle_poc.sh` (4 attempts, 2 aborted on harness bugs before any vote, 2 reproduced); QA3-2A test appended to `sign.rs` | 1 of 1, 2 of 2, 1 of 1 |
| F3-XC-100 | `git apply poc/F3-XC-100/poc.diff`; QA3-2A module appended to `keygen.rs`; L-crypto `forge test` in the local run folder's `scratch/L-crypto/forge-identity` | 3 of 3 |
| F3-VAL-062 | QA3-2A module (with the group above); then `git apply poc/F3-VAL-062/poc.diff`, test `warp_order_drops_peer_share_and_delays_deadline` | 2 of 2 |
| F-VAL-060 | `forged_complaints.rs` wired by `#[path]` into `keygen.rs`; `python3 -I poc/F-VAL-060/pc3/bytecode_check.py <out>`, `topics.py`, `cast disassemble`, ABI `anonymous` check | 1 of 1; no foreign topic0, no delegatecall or create, no anonymous events |
| F3-VAL-100 | `lt3_l2.rs` wired by `#[path]` into `keygen.rs` | 1 of 1 |
| F3-SEN-002, 003, 004, F-SEN-002, F-SEN-015 (handler), F3-SEN-060 | QA3-SEN modules, rv-sen run-3 modules, LT3's lead-13 test and CA3-FIX's `ca3fix_sen` pasted into `mod tests` of `crates/sentinel/src/service.rs`; one test binary, filters `qa3_sen_poc`, `qa3_sen_004`, `poc_f_sen_002`, `f_sen_002_run3_mirror`, `lead13`, `f_sen_015_run3`, `ca3fix_sen_f_sen_015` | as recorded by QA; emulation 278 of 300, 217 of 300, 471 of 1,200 |
| F3-SEN-002, 003, 004 (real binaries) | QA3-2B harness re-ported to 31500 to 31509: `sen002.py`, `sen003.py`, `sen004.py`; new `sen002a.py` (honest variant) x3 plus one swapped run | 1 of 1 each; honest loss 0 of 4 |
| F3-CORE-001, 002, 030 (real binary) | `core001.py`, `core002.py`, `core030.py` | 1 of 1 each |
| F3-CORE-001, 002, F3-XC-160 (unit and driver) | `git apply` of `driver-rate-limit-test.patch`, `unit-test.patch`, `watcher-test.patch`, QA3-2B XC-160 tests inserted into `index/mod.rs`; then `driver-too-soon-test.patch` alone | all reproduce |
| F3-CORE-030 | QA3-CORE `f030.sh` re-ported to 31500 to 31509 | `BadUpdate`, exit 0, three restarts |
| F3-XC-160 | QA3-CORE `f160.sh ca3inject 1` and `ca3control 0` | inject: no reveal, 799.60 left; control: both claim |
| F-SEN-015 | `integ python3 -I poc/F-SEN-015/ca3-fix/ca3fix_f_sen_015_e2e.py <wt> <out> 10 down 4`, `6 flip 4`, `4 down 3` | 10 of 10, 6 of 6, control 0 of 4 |
| F3-SEN-060 contract half | LT3's fork test copied to scratch, remapped, `integ env FOUNDRY_OFFLINE=true forge test --offline` | not run: `createSelectFork` tried to reach `rpc.gnosischain.com` and failed in the namespace |
| CA3-FIX spot-check | `ca3fix_sen.rs` pasted again, filter `ca3fix_sen_f` skipping `f_sen_015` | 3 of 3 pass (`F2-SEN-010`, `F-SEN-003` Fixed hold) |

Every pasted or applied hunk was reverted with `git checkout -- <files>`; `git status --short` showed no tracked change afterwards.

## Outside facts read (public web, generic User-Agent)

- `https://raw.githubusercontent.com/NethermindEth/nethermind/1.35.8/src/Nethermind/Nethermind.JsonRpc/ErrorCodes.cs` (`ResourceNotFound = -32001`, line 43) and the same path at `1.36.0` (`-32000`, line 48).
- `https://raw.githubusercontent.com/NethermindEth/nethermind/2.1.0/src/Nethermind/Nethermind.Facade/Find/LogFinder.cs` (an empty list for a missing header, lines 205-208; "incomplete results" log, line 356).

## Hypotheses considered

- Rejected: "the binaries in the shared target directories are at `361aaf3`". Both held a patched bite build; rebuilt before use.
- Rejected: "the honest variant of `F3-SEN-002` loses the bond in practice" (PC3's practical (a)). Undercount 4 of 4, loss 0 of 4: the slow sentinel's reveal led the block every time, in either account role.
- Kept: `F3-CORE-032`'s live one-block route reaches the same lost reveal as `F3-XC-160`; PC3 weighed only restarts.
- Kept: `F3-VAL-030` with a real `SentinelOracle` at the Aegis windows (PC3's remaining doubt).
- Noted: tracing JSON lines with a field named `message` overwrite the log message when parsed; grep the raw text.

## Not covered

- `F3-VAL-030` with five validators at 3 of 5 end to end (only QA3-2A's state-machine run covers 3 of 5); a natural Gnosis reorg cannot be produced offline.
- `F3-SEN-060`'s contract half (needs an RPC handshake for the fork); `F3-VAL-062` and `F3-SEN-003` process-level restarts beyond what QA3-2B and the core warp tests cover.
- Real provider limits and empty-answer windows (`F3-CORE-001`, `F3-CORE-002`, `F3-XC-160`): no RPC allowed.
- Low and Informational rows were checked against their evidence files, not re-run.

## Cleanup candidate

The local run folder's `scratch/CA3/` (223 MB): a copy of the `sentinel` binary (about 190 MB), Anvil logs and SQLite files from the reruns. Nothing in `rust-audit/` points into it except as the place the re-ported harnesses live; safe to remove once the report is final.
