# PC3 practicality check (run 3, Phase 4)

| Field | Value |
| --- | --- |
| Agent | PC3 (practicality-checker), parallel with CA3-FIX; CA3 runs after |
| Audit commit | `361aaf3` |
| Worktree | `<run folder>/wt/rv-core-a` with `env/rv-core-a.sh`, read-only use of its forge artifacts (`contracts/build/out`, built 2026-10-08 22:55); no Anvil started, no tracked file edited |
| Date | 2026-10-09 |
| Output | `## Practicality (PC3, 2026-10-09)` in every scoped finding file, `state/run3/phase4/PC3.md`, `poc/F-VAL-060/pc3/`, this log, report `<run folder>/reports/PC3.md` |

`<run folder>` is the local run folder.

## Scope

- Canonical run-3 findings: `F3-CORE-001`, `002`, `004`, `005`, `030`, `032`, `060`, `061`; `F3-VAL-030`, `031`, `032`, `061`, `062`, `100`; `F3-SEN-001` to `004`, `060`; `F3-XC-003` to `007`, `030`, `040`, `100`, `130`, `160`. `F3-VAL-100` and `F3-XC-040` were filed by LT3 during this run and added at the head's request.
- Duplicates, each given a one-line pointer section: `F3-CORE-003` (to `030`), `F3-CORE-031` (to `F3-XC-130`), `F3-VAL-001` (to `F3-XC-100`), `F3-VAL-060` and `F3-SEN-005` (to `F3-XC-005`), `F3-XC-001` (to `F3-SEN-002`), `F3-XC-002` (to `F3-SEN-003`).
- Re-validation rows with verdict Changed and a proposed severity of High or a move up: `F-SEN-008`, `F-VAL-060`, `F-SEN-002`, `F-SEN-015` (CA3-FIX Not fixed). The five `revalidation/*.md` ledgers hold no other Changed row that is High or moved up (checked row by row: `F-CORE-034`, `F-CORE-035`, `F2-CORE-067`, `F-CORE-068`, `F-SEN-004`, `F-SEN-005`, `F-SEN-014`, `F2-SEN-003`, `F2-SEN-011`, `F-VAL-002`, `F-VAL-035`, `F-VAL-040`, `F-VAL-061`, `F-VAL-062`, `F-VAL-065`, `F-VAL-066`, `F2-XC-005`, `F2-XC-007` keep or lower their band).

## Read

- Brief `<run folder>/briefs/phase4-shared.md`; `rust-audit/PROMPT.md` sections 1 to 12; `state/run3/STATE.md`, `settled.md`, the five `revalidation/*.md` ledgers, `phase4/CA3-FIX.md`, `agents/LT3.md`, `agents/CA3-FIX.md`.
- The maintainers' project notes (Current state), the team's decisions, the maintainers' review notes (sections 1, 5, 8), the practicality-checker lessons.
- Every scoped finding file in full, including the QA3-2B sections appended during this run (re-read before appending).
- Deployment facts: `contracts/.env.aegis.sample`, `docs/configuration.md`, `docs/validator-handbook.md:15-60, 60-75, 115-150`, `docs/sentinel-handbook.md:75-110`, both sample configs, and from a local review of the Aegis configuration (2026-10-07): `reports/onchain-analyst.md`, `deploy-check/a.txt`, `RESUME.md`, `slack-addendum.md`.
- Code checked for the calls: `contracts/src/SentinelOracle.sol:1-120, 215-300, 344-409`, `contracts/src/libraries/SentinelOracleRequests.sol:160-215`, `contracts/src/Consensus.sol:245-275`, `crates/sentinel/src/service.rs:395-520`, `crates/validator/src/bindings.rs:113-243`, the run-2 twin `F2-CORE-007`, the run-1 twins `F-CORE-031`, `F-SEN-002` (Phase 8 section), `F-SEN-008`, `F-VAL-060`, `F-SEN-015`.

## Commands

| Step | Command | Result |
| --- | --- | --- |
| F-VAL-060 topics | `source env/rv-core-a.sh && python3 -I rust-audit/poc/F-VAL-060/pc3/topics.py $WT/contracts/build/out` | `SentinelOracle` 16 events, `FROSTCoordinator` 10, `Consensus` 6, overlap none (`poc/F-VAL-060/pc3/topics-at-361aaf3.txt`) |
| F-VAL-060 bytecode | `cast disassemble <SentinelOracle deployedBytecode>` and `python3 -I rust-audit/poc/F-VAL-060/pc3/bytecode_check.py $WT/contracts/build/out` | no `DELEGATECALL`, `CALLCODE`, `SELFDESTRUCT`, `CREATE`, `CREATE2`; 1 `CALL`; 13 `LOG` sites; no foreign `topic0` in the runtime code, also with leading zeros trimmed (`poc/F-VAL-060/pc3/bytecode-at-361aaf3.txt`) |
| source history | `git log -- contracts/src/SentinelOracle.sol` and its libraries | last change `0be9636` (2026-09-23), before the 2026-10-07 deployment |
| reorg arithmetic | `python3 -I -c` on the consult's counts | 5.09M slots with no verified reorg of 3 or more: 95 % bound 0.59 per million slots, 3.7 a year; two timely 2-block forks, one per about 147 days; 1-block orphans 0.025 to 0.076 % of slots |
| appends | `python3 -I $SCRATCH/pc3/append.py rust-audit/findings $SCRATCH/pc3/sections <IDs>` (asserts one PC3 header per file and a verified tail) | 40 files, one PC3 section each; one pointer text that quoted the header literally was reworded by hand in `F3-CORE-003.md` |
| format | the style check on every PC3 section (line-filtered for files with older em dashes), `explorer/node_modules/.bin/prettier --check` (3.9.6, the Justfile version) | no style error inside a PC3 section; no new Prettier failure (the 10 files that fail did so before PC3, from earlier sections) |

## Consults

Two researcher consults, no RPC of any kind, raw pages kept in the session scratchpad (`gnosis-reorg/`, `rpc-consult/`):

- Gnosis reorgs since the merge: daily 1-block orphans (Dora 10 to 13 a day, beaconcha.in 4.35), six 2-block orphan chains in 294.8 usable days of which two were timely forks at the tip, no verified reorg of 3 or more; finality about 32 slots (2.7 min); no public finality failure since the merge; GIP-153 moves the chain to 2 s blocks around 2027-01. Data before 2025-07 is not readable.
- RPC behaviour: unknown hash in `eth_getLogs` gives -32001 only on Nethermind up to 1.35.8 and Reth, -32000 on Nethermind 1.36+ and Erigon; Nethermind 1.39+ answers a processed but not yet canonical hash with `[]` and success; every client answers `null` above head; dRPC's balancer keeps Gnosis upstreams up to 10 blocks behind in rotation; no free or public tier publishes the 133 `eth_getLogs`/s the per-event fallback needs. Backends and skew of `rpc.gnosischain.com` are not public.

## Not done

- No new end-to-end runs; every practicality call rests on the executed QA, QA3-2A, QA3-2B, CA3-FIX and LT3 evidence plus the F-VAL-060 bytecode check above.
- No live RPC (brief): the deployed `SentinelOracle` bytecode was not fetched; its match to the source rests on the unchanged source history and the overview's Sourcify record. Endpoint client versions, head skew and the Nethermind 1.39+ not-yet-canonical window were not measured.
- The `SentinelOracle` variant of the `F3-VAL-030` reorg PoC (named as the cheapest settling change) was not run.
- The department report file was not written: the harness blocks report files from subagents, so the report went to the head in the handback.
