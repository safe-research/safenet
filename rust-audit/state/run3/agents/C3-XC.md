# C3-XC coverage log (Critic, discovery track, cross-cutting)

| Field | Value |
| --- | --- |
| Critic | C3-XC (auditor department), run 3 Phase 2 |
| Audit commit | `361aaf3` (tests run in `<run folder>/wt/base`; main checkout unmodified outside `rust-audit/`) |
| Date | 2026-10-08 |
| Brief | `<run folder>/briefs/critic-shared.md` |
| Findings | F3-XC-003, F3-XC-004, F3-XC-005, F3-XC-007, F3-VAL-060, F3-SEN-005 |
| Mined | R8 coverage log, rejected hypotheses R-1 to R-8, R-12 to R-19, R-21 and observations O-2 to O-9 (non-sentinel parts) |
| Promotion range | F3-XC-030 to F3-XC-039; used F3-XC-030 |
| Scratch | none created; no build outside the shared `target-base` |

`<run folder>` is the local run folder.

## Independence

Read: `rust-audit/PROMPT.md`, `state/run3/settled.md`, `state/run3/baseline.md` (with its section 9 correction), `state/run3/map.md` (sections 1 to 5, R8's part of section 6, section 7), every `F3-*` title and location line, the six assigned files in full, F3-CORE-032 (claim, basis, trigger) for the relation to my promotion, the coverage logs of R8 (full), R2 (rejected list and observations, to check that my promotion is not already covered) and L-rust (Q72, Q75), and the run-3 logs cited by the assigned files. Not opened: `rust-audit/findings/F-*` and `F2-*`, `report/`, `poc/`, `codebase-map.md`, `analysis/`, any `state/` path outside `state/run3/`, `state/run3/revalidation/`, the `rv-*` logs and reports. From the run cache `gh/` I read only issue #113 (filtered by number). For each finding I read the title and location first and formed my own view from the cited code before reading the reviewer's text.

## Verdicts

| Finding | Verdict | Severity (reviewer / Critic) | Certainty | Duplicate | Note |
| --- | --- | --- | --- | --- | --- |
| F3-XC-003 | Confirmed | Informational / Informational | 95 | canonical | re-ran `cargo tree` and `ldd`; added a third remediation (`LIBSQLITE3_SYS_USE_PKG_CONFIG`) |
| F3-XC-004 | Confirmed, `known` | Informational / Informational | 90 | canonical | #1046 (`55d811e`) lists the engine URL under "Not covered" on purpose; documented deployment has no credential in the URL |
| F3-XC-005 | Confirmed | Low / Low | 95 | canonical for the sample-database group | remediation note: `create_if_missing: false` guards against a mistyped named volume; both remediation options drop that guard |
| F3-VAL-060 | Confirmed | Low / Low | 95 | duplicate of F3-XC-005 | adds the binary-level E1 and option 3 (error hint); one rejected-list statement corrected |
| F3-SEN-005 | Confirmed | Low / Low | 95 | duplicate of F3-XC-005 | adds sentinel `Config` E1; Basis 4 line corrected (`mod.rs:200`, not `:199`), not an `H` |
| F3-XC-007 | Confirmed | Informational / Informational | 90 | canonical | scope note: the CI-gate half is in `.github/` and `Justfile`, outside findings scope; remediation versions checked against the cached index |

No finding refuted, no `H` row, no severity change. Every Basis row of the six files was re-opened at `361aaf3` or in the pinned registry source.

## Commands

| # | Command (cwd) | Result |
| --- | --- | --- |
| 1 | `git show 361aaf3:<file>` for every cited path (Cargo.toml, both Dockerfiles and samples, engine.rs, sentinel config.rs and main.rs, core provider, utils, driver, state/mod.rs, state/storage.rs, index/blocks.rs, index/mod.rs, index/events.rs, tx/mod.rs, tx/storage.rs, tx/fees.rs, tx/bundle.rs, validator main.rs, frost/marshal.rs, frost/keygen.rs, frost/sign.rs, frost/ecdh.rs, frost/participants.rs, service/effect.rs, state/keygen.rs, SentinelOracle.sol, docs) | read |
| 2 | `cargo tree --offline --locked -e features -i libsqlite3-sys -p validator` (`<run folder>/wt/base`) | `libsqlite3-sys feature "bundled"` from `sqlx feature "sqlite"` |
| 3 | `ldd <run folder>/target-base/debug/{validator,sentinel} \| grep -i sqlite`; `grep -c -a '2026-03-13 10:38:09 737ae4a3'` on both | no shared SQLite; source id found once in each |
| 4 | `cargo tree --offline --locked -i pkg-config -p validator` | only `libsqlite3-sys` uses the `pkg-config` crate, and only on its linked path |
| 5 | registry reads: sqlx 0.9.0 and sqlx-sqlite 0.9.0 (features, defaults, URL parser), libsqlite3-sys 0.37.0 (`build.rs`, `sqlite3.h`), reqwest 0.13.4 (`error.rs`, `async_impl/request.rs`, `response.rs`, `client.rs` ALPN), metrics-exporter-prometheus 0.18.3 (features, `http_listener.rs`), metrics-util 0.20.4, ruint 1.18.0 (`bits.rs`), alloy-primitives 1.6.0, alloy-provider 2.0.5, alloy-sol-types 1.6.0 (lossy `string`), alloy-transport-ws 2.0.5, tungstenite 0.28.0, frost-core 3.0.0 (`Debug` redaction), hyper 1.10.1 manifest | as cited in the Critic sections |
| 6 | `~/.cargo/advisory-db` files for RUSTSEC-2026-0204, 0258, 0220, 0285, 0185, 0253, 0221 | read |
| 7 | `cargo tree --offline --locked -e features -i h2`, `-i hyper`, `-i reqwest` (`-p validator`) | hyper `http2` only via hyper-util `server-auto`; no reqwest `http2` |
| 8 | `strings` over the cached crates.io index entries for h2, ruint, rustls, crossbeam-epoch, lru, quinn-proto | patched versions present (h2 0.4.20, ruint 1.20.1, rustls 0.23.45, crossbeam-epoch 0.9.21, lru 0.18.5, quinn-proto 0.11.19) |
| 9 | `git log -S` for the Dockerfile's "does not enable the bundled feature" comment, then `git show 20cad66:Cargo.lock` | Dockerfile comment added in #640 with sqlx 0.9.0 already locked |
| 10 | `git show --stat 55d811e` | #1046 "Not covered" lists the engine URL |
| 11 | `git show fe9e84c:<file>` for the samples, `engine.rs`, `driver.rs`, `blocks.rs`; `git diff --stat fe9e84c 361aaf3` for driver, storage, utils, Dockerfiles, Cargo.toml | all assigned findings and the promotion are pre-existing |
| 12 | `cargo test --offline --locked -p safenet-core --lib` with `--exact` filters for `state::tests::warps_and_prunes_intermediate_snapshots`, `index::blocks::tests::resumes_after_a_single_pruned_snapshot_without_a_fake_reorg`, `index::blocks::tests::does_not_reemit_recent_blocks_at_or_before_the_persisted_snapshot` (`<run folder>/wt/base`) | 3 passed; saved as `logs/C3-XC-warp-restart-tests.txt` |
| 13 | Prettier 3.9.6 `--check` and the style check on every file I wrote or changed | clean |

## R8 rejected hypotheses (non-sentinel), re-checked

| # | R8's refutation | Critic check | Outcome |
| --- | --- | --- | --- |
| R-1, R-2 | h2 not reachable | metrics listener is `http1::Builder` only; reqwest without `http2` offers ALPN `http/1.1` (`reqwest-0.13.4/src/async_impl/client.rs:822-840`) | holds |
| R-3 | ruint shifts not reachable | only `<<` in scope is in `mod tests`; 1.18.0 `wrapping_sh*` take the full `usize` | holds |
| R-4 | crossbeam-epoch pointer formatting not reachable | metrics-util's only user is `storage/bucket.rs`, `Debug` without the pointer | holds |
| R-5, R-6 | lru and event-listener unsoundness not reachable | advisory conditions (`catch_unwind` plus panicking `Drop`; `!Send` tags) absent | holds |
| R-7 | no extension loading through the URL | `parse.rs:37-108` rejects unknown keys | holds |
| R-8 | `solidity_point` safe on identity | `frost/marshal.rs:72-77` returns early | holds |
| R-12 | actions not lost on queue errors; "a crash after the snapshot commit replays from the safe snapshot" | true for intermittent errors and in steady state; false during warps, where the prune leaves the page's own snapshot as the only one and the restart resumes after it (`driver.rs:261-290`, `storage.rs:151-160`, `blocks.rs:315-320`, executed tests) | promoted as F3-XC-030 |
| R-13 | `cap_priority_fee` cannot panic | `max(0.0)`, saturating cast, `>= PRECISION` guard (`tx/fees.rs:16-19`); a NaN cap silently disables priority fees, which is F3-CORE-061's territory (config, A1) | holds |
| R-14 | block watcher `expect`s cannot fire | `blocks.rs:383-390, 509-513` re-read | holds |
| R-15 | WS handshake errors carry no URL | alloy uses `tokio_tungstenite::connect_async_with_config` (`alloy-transport-ws-2.0.5/src/native.rs:147`); tungstenite's URI-bearing `UnableToConnect` is on the blocking path only (`tungstenite-0.28.0/src/client.rs:130-138`) | holds |
| R-16, R-17 | no secret in `Debug`, config never logged | frost-core `SigningShare` redacted (`keys.rs:126-133`); `KeyShare` wraps `KeyPackage`; validator and sentinel `main` log only the config path | holds |
| R-18, R-19 | batch gas sufficient; EIP-7623 floor met | settled L5 covers the missing worst-case test; constants re-read (`tx/bundle.rs:71-74, 196-209`) | holds |
| R-21 | SQLite URL errors echo no secret | only `mode`, `cache`, `immutable` values and unknown keys are echoed | holds |

R-9 was spot-checked because it touches core decoding (`decode_raw_log` is the non-validating path and `string` decodes lossily, `alloy-sol-types-1.6.0/src/types/data_type.rs:381-388`); holds. R-10, R-11 and R-20 are sentinel parts for C3-SEN.

## R8 observations (non-sentinel), not promoted

- O-2 (frost-core `^3` with `internals`): the lock pins 3.0.0 and the Dockerfiles build `--locked`; hardening.
- O-3 (Docker hardening): floating tags, root, no `HEALTHCHECK`, `!/crates/**` context. Hardening; the runtime image copies only the binary.
- O-4 (samples use private key 1): operator error under A1; a candidate Informational for C3-COV.
- O-5: A17.
- O-6 (metrics listener without auth or limits on `0.0.0.0:3555`): the handbooks' `docker run` examples publish no port; also R2's O14. Candidate for C3-COV.
- O-7, O-8, O-9: operator config, no concrete overflow, side result.

## Not settled

- The aside in F3-XC-003 that Debian trixie ships SQLite 3.46 (no network).
- The consult's claim that ruint 1.19.x introduced the shift-amount truncation (1.19.0 not in the local registry).
- F3-XC-030 end to end through the real `Driver`: left to QA with the recipe in its Trail. A second trigger, `SQLITE_BUSY` on the queue insert under the per-block reconcile writes of a warp, was not measured.
