# C3-COV coverage log (run 3, Phase 2, discovery track)

| Field | Value |
| --- | --- |
| Agent | C3-COV, Coverage Critic (reviewer department), Opus 5.5 |
| Brief | the local run folder's `briefs/critic-shared.md` (C3-COV row and paragraph) |
| Audit commit | `361aaf3` (main checkout HEAD; `git diff --stat 361aaf3 -- crates Cargo.toml Cargo.lock contracts/src docs scripts` empty) |
| Date | 2026-10-08 |
| Outputs | `rust-audit/state/run3/coverage.md`, `rust-audit/findings/F3-XC-160.md`, this log, department report in the local run folder (`reports/C3-COV.md`) |
| Promotion range | `F3-XC-160` to `F3-XC-179`; used `F3-XC-160` |

## Independence

- Read: `rust-audit/PROMPT.md`, `state/run3/settled.md`, `state/run3/baseline.md` (with its section 9 correction), `state/run3/map.md` sections 1 to 7, every `findings/F3-*.md` (headers and Claim sections; Trigger and rejection sections of F3-CORE-032), the coverage logs `state/run3/agents/{R1..R8, L-crypto, L-rust}.md`, the "Not covered" sections of the matching Phase 1 department reports, the shared Phase 1 brief, and issues #961 and #973 from the run cache (`jq` select by number, so the PR #905 item was not printed).
- Not opened: `findings/F-*`, `findings/F2-*`, `report/`, `poc/`, `codebase-map.md`, `analysis/`, any `state/` path outside `state/run3/`, `state/run3/revalidation/`, the `rv-*` logs and reports, `STATE.md`, PR #905.
- Incidental exposure, names only: `ls` of the run's `scratch/` folder printed `rv-core-a.md`, `rv-core-b.md`, `rv-sen.md`, `rv-val.md`, `rv-xc.md` and `canonical.tsv`; `ls` of the run's `reports/` folder printed the `rv-*.md` report names; the session's initial `git status` listed modified `F-*` file names. None was opened.
- No memory written (parallel instances).

## Findings handled

No existing F3 finding was assigned to C3-COV, so none was critiqued and no header cell of another file was changed.

| ID | Action | Severity | Basis | Notes |
| --- | --- | --- | --- | --- |
| `F3-XC-160` | Promoted (new Draft) from R1's observation O5 | Medium | E2 mechanism, I for the RPC behaviour; self-estimate 50% | A lagging load-balanced backend makes `revalidate_last_block` (`crates/core/src/index/blocks.rs:561-564`) report an uncle for a canonical block; the rollback opens #973's live hole without a reorg and can strand a sentinel bond. Tagged `known` for the consequence. Pre-existing at `fe9e84c` |

## Commands

| # | Command | Result |
| --- | --- | --- |
| 1 | `cat` and `Read` of the brief, PROMPT.md, settled.md, baseline.md, map.md, the ten coverage logs | read |
| 2 | `for f in F3-*.md; do head -14 ...; done` and an `awk` extract of each Claim | 31 Phase 1 drafts listed with location, severity, claim |
| 3 | `git grep -c '' 361aaf3 -- Cargo.toml Cargo.lock crates/core crates/validator crates/sentinel` | 75 files; every count equals map section 7 |
| 4 | `python3 -I nontest.py` (session scratchpad; `git show 361aaf3:<path>`, lines before `#[cfg(test)] mod tests`) | every non-test count equals the map |
| 5 | `python3 -I locmap.py` over the `Location` field of every `findings/F3-*.md` | file to finding to Critic map (32 findings after F3-XC-160) |
| 6 | `python3 -I matrix.py` with the reviewer shares transcribed from the ten logs | matrix rows and counts: full readers 0 for 1 file, 1 for 12, 2 for 41, 3 for 12, 4 for 5, 5 for 2, 6 for 2; 38 files named by no finding |
| 7 | `git show 361aaf3:<path>` for the spot-reads (`index/bloom.rs`, `index/events.rs` 1-40, `tx/fees.rs`, `tx/config.rs`, validator `metrics.rs`, `consensus/{epoch,mod}.rs`, `secrets/mod.rs`, `state/secrets.rs`, `frost/serialization.rs`, sentinel `metrics.rs`, `verdicts.rs`, `hashing.rs`) and `git grep -n 'metrics::' 361aaf3 -- crates/{validator,sentinel}/src` | results in `coverage.md` section 3 |
| 8 | `git show 361aaf3:` of `crates/core/src/index/{blocks,mod,events}.rs`, `state/{mod,storage}.rs`, `driver.rs`, `tx/mod.rs`, `crates/sentinel/src/service.rs` (ranges cited in F3-XC-160) | mechanism of F3-XC-160 traced |
| 9 | `git show fe9e84c:crates/core/src/index/blocks.rs`, `git show fe9e84c:crates/core/src/index/mod.rs`, `git log -S revalidate_last_block -- crates/core/src/index` | same code at `fe9e84c` (lines 504-505, 109-113); introduced by #454 and #462 |
| 10 | `git show e69fdc8^:validator/src/watcher/{index,blocks}.ts` | TypeScript origin: `BlockNotFoundError` mapped to `null`, then uncle |
| 11 | `jq -r '.[] \| select(.number==961 or .number==973)' gh/issues-open.json` | issue texts (live-reorg hole in #973, testnet incident in #961) |
| 12 | `grep` over `contracts/src` (`Consensus.sol` epoch timing, `FROSTCoordinator.sol` `sign`, `keyGen`, `keyGenComplain`, `keyGenComplaintResponse`, `keyGenConfirm`; `FROSTParticipantMap.sol` 170-240; `SentinelOracle.sol` 224-236) | context for the R1 O6, R4 O2, R6 O6 and deadline checks |
| 13 | `grep -rn -i 'health' docs/ ...`, `grep -rn 'max_logs_per_query' docs/ crates/ scripts/`, `grep -rn -i 'load.balanc\|32001\|too soon' docs/ crates/...` | no doc promises a progress probe; `max_logs_per_query` undocumented; the handbooks' "too soon" note |
| 14 | `grep -n 'sign' logs/R6-gas-coordinator.txt` | `sign` 53,397 gas at Aegis size |
| 15 | the style check and `explorer/node_modules/.bin/prettier --check` (3.9.6) on `coverage.md`, `F3-XC-160.md`, this log; `prettier --write` on `F3-XC-160.md` once (it changed only the Basis table's separator row) | clean |

No build, test or PoC was run. No scratch copy was made under `scratch/C3-COV/`; the helper scripts live in the session scratchpad (the session scratchpad, tmpfs).

## Citations re-opened at `361aaf3`

- F3-XC-160: `crates/core/src/index/blocks.rs:281-283, 529-539, 561-564, 576-591`; `crates/core/src/index/mod.rs:85-97, 106-127, 135-142, 354-402`; `crates/core/src/index/events.rs:440-490, 494-548`; `crates/core/src/state/mod.rs:171-276, 278-290`; `crates/core/src/state/storage.rs:118-143`; `crates/core/src/driver.rs:170-298`; `crates/core/src/tx/mod.rs:164-226, 364-390`; `crates/sentinel/src/service.rs:100-256, 296-345, 396-430`; `contracts/src/SentinelOracle.sol:231-236`; `docs/validator-handbook.md:37`.
- Observation checks: `crates/validator/src/state/keygen.rs:275-430, 640-780` (complaint flow, R6 O6); `contracts/src/libraries/FROSTParticipantMap.sol:181-227`; `contracts/src/FROSTCoordinator.sol:336-348, 440-497, 509-521`; `crates/core/src/index/events.rs:60-135, 636-672` (`max_logs_per_query`); `crates/validator/src/config.rs:1-75`, both samples, `docs/configuration.md:70-125`, both handbooks' batching tips; `crates/core/src/observability/metrics.rs:1-60`.

## Promotions

`F3-XC-160` only. The other 96 observation entries are dispositioned in `coverage.md` section 6 (17 covered by an F3 finding or confirming one, 11 settled or known, 68 kept with a reason).

Considered for promotion and left as observations:

- R1 O9 (logs of an orphaned hash answered with another code than -32001 keep the event watcher on that hash while the block watcher is not polled): a permanent indexing stall if a Gnosis client behaves that way, but its answer for an orphaned hash is not known offline. Cross-referenced in F3-XC-160.
- R1 O6 (`max_logs_per_query` plus permissionless `sign` spam): opt-in, undocumented, and a 17M-gas block holds only a few hundred `Sign` events.
- R6 O6 (late complaint excludes the honest plaintiff): needs the plaintiff's own complaint delayed about 10 minutes; contradicts R4's Q30 answer, handed to C3-VAL.
- R1 O1 (warp completeness): the documented RPC failure is the "too soon" one, which warps avoid.
- `/health` always `OK` (R2 O4, R6 O3, L-rust O1 and O3): no document promises a progress probe; Informational.

## Items not settled

- Whether the Gnosis clients behind load-balanced endpoints answer -32001 for logs of an unknown block hash, and how often consecutive requests reach a backend one block behind (decides F3-XC-160's practicality). No network beyond the Cargo registry in this run.
- The same clients' answer for logs of an orphaned hash (R1 O9).
- Two Phase 1 disagreements left to C3-VAL: R4 O2 against R6 Q29 (late rollover confirmation), R4 Q30 against R6 O6 (late complaints).
