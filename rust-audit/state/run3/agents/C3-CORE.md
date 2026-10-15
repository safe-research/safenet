# C3-CORE coverage log (run 3, Phase 2 Critic, discovery track)

| Field | Value |
| --- | --- |
| Agent | C3-CORE (auditor department), Critic for core |
| Audit commit | `361aaf3` (main checkout; `git status --short -- crates contracts docs scripts Cargo.toml Cargo.lock` shows only untracked submodule folders under `contracts/lib/`, no change to scope files) |
| Date | 2026-10-08, finished after midnight (2026-10-09 local); section headings use the brief's date |
| Brief | `<run folder>/briefs/critic-shared.md` (`<run folder>` is the local run folder) |
| Assigned | `F3-CORE-001` to `005`, `F3-CORE-030`, `031`, `032`, `F3-CORE-060`, `061`, `F3-XC-130`; rejected lists of R1, R2, R3, L-rust; promotion range `F3-CORE-090` to `099` |

## Independence

- Read for setup: the brief, `rust-audit/PROMPT.md` (whole), `state/run3/settled.md`, `state/run3/STATE.md`, `state/run3/baseline.md` (whole, section 9 correction noted), `state/run3/map.md` (whole); the maintainers' project notes, the team's decisions, the safenet lessons of practicality-checker, confidence-assigner, reviewer and tester.
- Disclosed: `STATE.md` (an allowed path) carries one-line summaries of the re-validation rows, which name some run-1 IDs and their outcomes. I read it once at the start for orientation; every verdict below rests on code and logs I re-opened at `361aaf3`.
- Disclosed: my first directory check was `ls rust-audit/findings/ | grep '^F3-'`, which lists the folder but printed only `F3-*` names. Later checks used `F3-*` paths only. Listing `<run folder>/` and `<run folder>/scratch/` showed the names of `reports/rv-*.md`, `scratch/rv-*.md` and `scratch/canonical.tsv`; I opened none of them.
- Not opened: `findings/F-*`, `findings/F2-*`, `report/`, `poc/`, `codebase-map.md`, `analysis/`, `state/` outside `state/run3/`, `state/run3/revalidation/`, the `rv-*` agent logs and reports.
- Opened outside my set: the head of `findings/F3-SEN-004.md` (lines 1 to 25), to check whether it duplicates `F3-CORE-032` (it does not).

## Findings handled

| ID | Reviewer severity | Critic severity | Verdict | Certainty | Duplicate | Rows |
| --- | --- | --- | --- | --: | --- | --- |
| F3-CORE-001 | Medium | Medium | Plausible | 60 | canonical (related to F3-CORE-002) | 13 Supported |
| F3-CORE-002 | Low | Medium | Plausible | 50 | canonical (related to F3-CORE-001) | 8 Supported |
| F3-CORE-003 | Low | Medium | Confirmed | 90 | duplicate of F3-CORE-030 | 8 Supported |
| F3-CORE-004 | Low | Low | Confirmed | 72 | canonical | 6 Supported |
| F3-CORE-005 | Low | Low | Confirmed | 72 | canonical | 7 Supported |
| F3-CORE-030 | Medium | Medium | Confirmed | 90 | canonical for F3-CORE-003 | 7 Supported (row 7 re-run) |
| F3-CORE-031 | Informational | Informational | Confirmed | 90 | duplicate of F3-XC-130 | 5 Supported |
| F3-CORE-032 | Low | Low | Confirmed | 90 | canonical (related to F3-SEN-004) | 8 Supported |
| F3-CORE-060 | Low | Low | Plausible | 50 | canonical | 8 Supported |
| F3-CORE-061 | Informational | Informational | Confirmed | 85 | canonical | 6 Supported |
| F3-XC-130 | Low | Informational | Confirmed | 90 | canonical for F3-CORE-031 | 8 Supported |

No row was Unsupported (no `H`), nothing was refuted, so no rebuttal round is needed. Corrections recorded in the Critic sections: F3-CORE-032's rollover-after-warp window is about one block time of wall clock after catch-up (not "about 5 blocks"), and a live uncle loses only resumes that already landed; F3-CORE-061's `blocks_before_resubmit = 0` behaves like 1.

## Citations re-opened at `361aaf3`

- `crates/core/src/driver.rs` 1-328 (whole); `crates/core/src/index/events.rs` 1-800 and the test at 1917-1977; `index/blocks.rs` 1-619; `index/mod.rs` 1-150; `index/clock.rs` (whole); `index/bloom.rs` 15-40; `state/mod.rs` 1-330; `state/storage.rs` 1-175; `provider/mod.rs` (whole); `observability/metrics.rs` (whole); `effects.rs` 80-101 plus greps.
- `crates/core/src/tx/fees.rs` (whole), `tx/config.rs` 1-140, `tx/types.rs` (whole), `tx/mod.rs` 1-450, `tx/storage.rs` 1-430.
- Validator: `main.rs` 1-60, `service/mod.rs` 60-90, `bindings.rs` event declarations, `state/keygen.rs` 36-60, 1005-1130, 1150-1175, `state/secrets.rs` 1-30 and 70-86. Sentinel: `main.rs` 25-50, `bindings.rs` 1-176 (events and watcher set), `service.rs` 100-160, 300-345, 396-430.
- Docs: `docs/validator-handbook.md` 20-75, `docs/sentinel-handbook.md` 20-35, 57, 65, `docs/overview.md` 60-84; greps for `reorg`, `ntp|clock|synchroni` and `use_client_filtering`.
- Registry sources at the locked versions: alloy-rpc-client 2.0.5 `src/builder.rs` 125-200 and `src/builtin.rs` 1-60, 230-300; alloy-sol-types 1.6.0 `src/types/data_type.rs` 370-395 and `src/types/event/mod.rs` 155-215; metrics-exporter-prometheus 0.18.3 `src/exporter/http_listener.rs` 130-160; alloy-transport-http 2.0.5 `src/reqwest_transport.rs` 85-96.
- History: `git show e69fdc8^:validator/src/watcher/backoff.ts` and `events.ts`; `git show 86d5d0f:validator/src/watcher/index.ts` 50-115; commit messages of `1601391` (#460), `55d811e` (#1046), `39c9cb1` (#462).
- Logs: `R1-index-tests.txt` (grep), `R2-harness-main.rs.txt`, `R2-harness-Cargo.toml.txt`, `R2-stale-rpc-restart.txt`, `R2-lost-effect-on-restart.txt`, `R2-startup-config-checks.txt` (grep), `L-rust-provider-connect-url-leak.txt`, `R3-fee-config-probe.txt`.

## Commands

| Command | Result |
| --- | --- |
| `git log --oneline -3`, `git rev-parse HEAD`, `git status --short -- crates ...` | `361aaf3`; scope files unchanged |
| `sha256sum <run folder>/scratch/R2/poc/src/main.rs` | `caaf9e6d...`, equal to the hash in the R2 logs |
| `<run folder>/scratch/R2/target/debug/deps/r2_poc-0770c0a9a0e420d8 restart 999` (existing binary, in-memory SQLite, mocked RPC) | `ERROR safenet_core::driver: unrecoverable driver error; exiting err=State(BadUpdate)`, `snapshots left: [1001]`: F3-CORE-030 reproduced |
| `grep -rn 'syncing\|is_syncing\|get_block_number\|BadUpdate' crates/{core,validator,sentinel}/src` | only the state machine's own `BadUpdate` returns |
| `grep -n` over the validator and sentinel bindings | 16 and 11 watched events |
| the style check on the 11 finding files, this log and the report | 0 errors, 0 warnings |
| `explorer/node_modules/.bin/prettier --check` (3.9.6) on the 11 finding files | 8 clean; F3-CORE-030, 031 and 032 fail only on R2's Basis table separator row (`\| - \| ----- \| ...`), earlier text I may not edit |

No build of my own: nothing was created under `<run folder>/scratch/C3-CORE/`.

## Rejected lists re-checked (promotions)

None promoted; no rejected hypothesis was wrongly dismissed. Spot checks behind that:

- R1 (15 items): #3 holds, strings decode lossily (`alloy-sol-types-1.6.0/src/types/data_type.rs:382-388`) and `decode_raw_log` does not validate (`event/mod.rs:186-196`); #1 and R2-H6 hold, the 16 and 11 topics are distinct; #5, #6 and #7 hold by the queue and uncle logic (`index/mod.rs:85-97`, `blocks.rs:476-489, 540-592`, `state/mod.rs:187-190`); #15 is a transient startup error.
- R2 (19 items): H7 holds (during a warp every recent block is queued, so `rposition` finds nothing); H16 holds for live operation; H17 holds (`NonceNotFound` needs a row deleted between read and write inside one sequential update).
- R3 (15 items): #1 holds because a by-number `eth_getProof` cannot return a nonce older than a row executed at or below the safe block without a deeper reorg; #2 holds (a self-sponsored authorization for `nonce + 1` is always valid after the sender's own increment); #8 holds (bumps preserve the tip to max-fee order).
- L-rust (17 items): R-5 rests on A4 (a peer that holds a request open is an RPC liveness failure); R-7, R-10 to R-13 hold by the cited statement order.

## Items not settled

- R1 observation O9 (uncled-block recovery only for JSON-RPC `-32001`): if a Gnosis client answers `eth_getLogs` for an uncled hash with another code, the event watcher retries that hash forever and the block watcher is never polled (`index/mod.rs:85-97, 109-142`). The TypeScript watcher had the same rule (`86d5d0f:validator/src/watcher/index.ts:60-83`). Needs Nethermind and Erigon behaviour, which I could not check offline. Left for C3-COV and the head.
- F3-CORE-001's stall and F3-CORE-002's window depend on provider limits and on a Nethermind timing window that no offline source gives.
- The sentinel binary was not run for F3-XC-130 (the validator binary was, twice, by R2 and L-rust).
