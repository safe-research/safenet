# Run 2 baseline (Phase 0, Recon)

| Field | Value |
| --- | --- |
| Commit | `3ec8bc57dc35d1e9e65075ae9424bff427c47833` — merge of `origin/main` (`5cc096e`) into `audit/rust-services`; parents `57044af`, `5cc096e` |
| Branch | `audit/rust-services` |
| Date | 2026-09-15 (toolchain probe 10:29 UTC; build 10:34–10:36 UTC; clippy finished 10:47 UTC) |
| Mode | **full** — toolchain present, all five baseline commands executed |
| Scope | `crates/core`, `crates/validator`, `crates/sentinel`, `Cargo.toml`, `Cargo.lock`, those three crates' `Cargo.toml`, `Dockerfile` and `*.sample.toml`. `crates/sentinel-engine` is excluded (PROMPT.md Section 11). |
| Logs | `rust-audit/state/run2/logs/`: `00-toolchain.log`, `01-build.log`, `02-test.log`, `03-clippy.log`, `04-audit.log`, `05-tree-d.log`, `06-inventory.txt`, `07-drift.txt`, `08-future-incompat.log` |
| Independence | Written without opening `rust-audit/findings/`, `rust-audit/report/`, `rust-audit/state/*.md` outside `state/run2/`, or `rust-audit/state/logs/`. Leads come only from `codebase-map.md`. |

Every command below was run from the repository root with:

```sh
export PATH="$HOME/.foundry/bin:$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR=/home/shebin.guest/.cache/safenet-run2/target CARGO_BUILD_JOBS=2
```

## 1. Toolchain (assumption A9)

Full output with exit codes: `logs/00-toolchain.log`. Only observed values are recorded.

| Tool | Command | Observed | Exit |
| --- | --- | --- | --- |
| cargo | `cargo --version` | `cargo 1.98.1 (797e8a9bc 2026-08-05)` | 0 |
| rustc | `rustc --version` | `rustc 1.98.1 (48a229cea 2026-09-01)` | 0 |
| rustup toolchain | `rustup show active-toolchain` | `stable-aarch64-unknown-linux-gnu (default)`; `rustup toolchain list` shows it is the only installed toolchain; the repo has no `rust-toolchain.toml` | 0 |
| cargo-audit | `cargo audit --version` | `cargo-audit-audit 0.22.2` | 0 |
| forge | `forge --version` | `forge Version: 1.8.1`, commit `982849d3140c01fd3b72905759581a132df7aa98`, built 2026-08-28 | 0 |
| anvil | `anvil --version` | `anvil Version: 1.8.1`, same commit and build | 0 |
| cast | `cast --version` | `cast Version: 1.8.1`, same commit and build | 0 |
| just | `just --version` | `just 1.40.0` | 0 |
| jq | `jq --version` | `jq-1.8.1` | 0 |
| git | `git --version` | `git version 2.51.0` | 0 |
| python3 | `python3 --version` | `Python 3.13.7` | 0 |
| sqlite3 CLI | `sqlite3 --version` | not installed (`command not found`). Not required by PROMPT.md; the system SQLite _library_ is present because `libsqlite3-sys 0.37.0` (non-bundled) linked | 127 |
| clippy | `cargo clippy --version` | `clippy 0.1.98 (48a229ceae 2026-09-01)` (`logs/03-clippy.log` line 5) | 0 |

| Resource | Command | Observed |
| --- | --- | --- |
| CPU | `nproc` | 4 (`uname -srm`: `Linux 6.17.0-41-generic aarch64`; host `lima-default`) |
| RAM | `free -h` | 11 GiB total, 7.0 GiB available at probe time, no swap |
| Disk `/` | `df -h /` | 145 G, 75 G free before the build; 72 G free after build + test + clippy |
| Disk `/tmp` | `df -h /tmp` | tmpfs 5.9 G, 3.5 G free — **never used for builds or logs** |
| Cargo target | — | `/home/shebin.guest/.cache/safenet-run2/target`, created fresh for run 2; 3.9 G after build + test + clippy |
| Cargo config | — | no `~/.cargo/config(.toml)`, no `.cargo/` in the repo, no `RUSTC_WRAPPER`/`sccache`, no `RUSTFLAGS`; `~/.cargo/registry` was already populated (596 MB) |

**A9 verdict: TRUE, with versions.** Rust stable 1.98.1 (unpinned), Foundry 1.8.1 (PROMPT.md names 1.5.1; the installed release is newer), `just` 1.40.0, `jq` 1.8.1, 11 GiB RAM (≥ 8), 75 G free disk (≥ 15). Network: `cargo audit` fetched the RustSec advisory database from `https://github.com/RustSec/advisory-db.git` and ran `Updating crates.io index` for its yanked-crate check (`logs/04-audit.log` lines 5–7), which the Recon brief allows; the build downloaded nothing (0 `Downloaded` lines) and no other network use was made by this agent. Prettier was **not** run on the run-2 files because the repo's `Justfile` invokes it via `npm exec -y -- prettier@3.9.6`, which would download from npm; the Manager should run `just fmt`/prettier before committing.

## 2. Executed baseline

| # | Command | Exit | Wall time | Log | Result |
| --- | --- | --- | --- | --- | --- |
| 1 | `cargo build --workspace --all-targets --locked` | **0** | `Finished` in 1 m 36 s (10:34:49–10:36:26 UTC) | `logs/01-build.log` | 352 `Compiling` lines, 0 `Downloaded` lines (registry already populated), 0 errors, 1 warning: future-incompat lint in `proc-macro-error2 v2.0.1` |
| 2 | `cargo test --workspace` | **0** | test phase ≈ 1 s (binaries already built by #1) | `logs/02-test.log` | 280 passed, 0 failed, 0 ignored across 4 unit-test binaries + 1 empty doc-test run |
| 3 | `cargo clippy --workspace --all-targets --locked -- -D warnings` | **0** | 25 s (10:47:11–10:47:36 UTC) | `logs/03-clippy.log` | 268 `Checking` + 28 `Compiling` lines, **0 clippy diagnostics**, same future-incompat warning |
| 4 | `cargo audit` | **1** (expected) | 5 s (10:46:25–10:46:30 UTC) | `logs/04-audit.log` | 573 crate dependencies scanned against 1246 advisories: **5 vulnerabilities**, 11 warnings (4 unmaintained, 3 unsound, 4 yanked) |
| 5 | `cargo tree -d --workspace` | **0** | — | `logs/05-tree-d.log` | 14 packages present in more than one distinct version (listed below) |
| + | `cargo report future-incompatibilities --id 1` (follow-up to #1) | 0 | — | `logs/08-future-incompat.log` | only `proc-macro-error2@2.0.1` (registry source `src/lib.rs:494`) |

Commands #1 and #2 were launched detached (`setsid nohup … >> log 2>&1`) so the log survives the shell call, and were waited on in the foreground; each log ends with `# end: <time>` and `# exit=<code>` written by the wrapper. Commands #3–#5 ran directly in the foreground.

### 2.1 Tests per crate (`logs/02-test.log`)

| Test binary | Passed | Failed | Ignored | Time |
| --- | --- | --- | --- | --- |
| `safenet_core` (lib unit tests) | 99 | 0 | 0 | 0.08 s |
| `sentinel` (bin unit tests) | 42 | 0 | 0 | 0.20 s |
| `validator` (bin unit tests) | 42 | 0 | 0 | 0.32 s |
| `sentinel_engine` (bin unit tests, **out of scope**) | 97 | 0 | 0 | 0.01 s |
| Doc-tests `safenet_core` | 0 | 0 | 0 | — |

In-scope total: **183** executed tests, equal to the `#[test]`/`#[tokio::test]` attribute count in Section 3 (99 + 42 + 42). Workspace total 280. There are no `tests/` integration targets, benches or examples in the three crates (`find … -not -path '*/src/*'` is empty), and the bin crates have no doc-tests. One `#[should_panic]` test exists: `crates/core/src/kdf.rs:77` (`derive_key_rejects_empty_domain`). No `#[ignore]`, `sqlx::test`, `rstest` or `test_case` attributes in scope. No `unsafe` token anywhere in the three crates.

### 2.2 `cargo audit` advisories, verbatim (`logs/04-audit.log`)

Reachability and impact are **not assessed here**; that is reviewer R10's task. The log contains the full inverse dependency tree for each vulnerability.

| Crate | Version | ID | Kind | Title (verbatim) | Solution (verbatim) |
| --- | --- | --- | --- | --- | --- |
| `crossbeam-epoch` | 0.9.18 | RUSTSEC-2026-0204 | vulnerability | Invalid pointer dereference in `fmt::Pointer` impl for `Atomic` and `Shared` when the underlying pointer is invalid | Upgrade to >=0.9.20 |
| `h2` | 0.4.14 | RUSTSEC-2026-0258 | vulnerability | h2 unbounded empty DATA frames | Upgrade to >=0.4.16 |
| `quinn-proto` | 0.11.14 | RUSTSEC-2026-0185 | vulnerability, `7.5 (high)` | Remote memory exhaustion in quinn-proto from unbounded out-of-order stream reassembly | Upgrade to >=0.11.15 |
| `ruint` | 1.18.0 | RUSTSEC-2026-0220 | vulnerability | Uint shift operations: incorrect overflow flags and truncated shift amounts | Upgrade to >=1.20.0 |
| `rustls` | 0.23.40 | RUSTSEC-2026-0285 | vulnerability, `5.3 (medium)` | TLS 1.3 handshake messages incorrectly accepted across encryption level boundaries | Upgrade to >=0.23.45 |
| `atomic-polyfill` | 1.0.3 | RUSTSEC-2023-0089 | warning: unmaintained | atomic-polyfill is unmaintained | — |
| `derivative` | 2.2.0 | RUSTSEC-2024-0388 | warning: unmaintained | `derivative` is unmaintained; consider using an alternative | — |
| `paste` | 1.0.15 | RUSTSEC-2024-0436 | warning: unmaintained | paste - no longer maintained | — |
| `proc-macro-error2` | 2.0.1 | RUSTSEC-2026-0173 | warning: unmaintained | proc-macro-error2 is unmaintained | — |
| `anyhow` | 1.0.102 | RUSTSEC-2026-0190 | warning: unsound | Unsoundness in `Error::downcast_mut()` | — |
| `event-listener` | 5.4.1 | RUSTSEC-2026-0221 | warning: unsound | `event-listener` allows `!Send` tags to cross thread boundaries via `StackSlot` | — |
| `lru` | 0.16.4 | RUSTSEC-2026-0253 | warning: unsound | Potential use-after-free due to lack of panic safety in `LruCache::pop()` | — |
| `bitcoin-io` | 0.1.100 | — | warning: yanked | — | — |
| `bitcoin_hashes` | 0.14.100 | — | warning: yanked | — | — |
| `chacha20` | 0.10.0 | — | warning: yanked | — | — |
| `spin` | 0.9.8 | — | warning: yanked | — | — |

The advisory set is date-dependent: RUSTSEC-2026-0285 (`rustls`) is dated 2026-09-14, the day before this run. `cargo audit` is not part of CI (`codebase-map.md` Section 3), so none of this is gated upstream.

### 2.3 `cargo tree -d --workspace` (`logs/05-tree-d.log`, 947 lines)

Packages present in more than one **distinct version**:

| Package         | Versions               |
| --------------- | ---------------------- |
| `block-buffer`  | 0.10.4, 0.12.0         |
| `const-oid`     | 0.9.6, 0.10.2          |
| `cpufeatures`   | 0.2.17, 0.3.0          |
| `crypto-common` | 0.1.6, 0.2.2           |
| `digest`        | 0.10.7, 0.11.3         |
| `getrandom`     | 0.2.17, 0.3.4          |
| `hashbrown`     | 0.14.5, 0.16.1, 0.17.1 |
| `hmac`          | 0.12.1, 0.13.0         |
| `rand`          | 0.8.6, 0.9.4           |
| `rand_chacha`   | 0.3.1, 0.9.0           |
| `rand_core`     | 0.6.4, 0.9.5           |
| `sha2`          | 0.10.9, 0.11.0         |
| `tower-http`    | 0.6.11, 0.7.0          |
| `webpki-roots`  | 0.26.11, 1.0.7         |

Two families are split across incompatible majors: RustCrypto 0.10/0.11 (`digest`, `sha2`, `hmac`, `block-buffer`, `crypto-common`, `cpufeatures`) and `rand` 0.8/0.9 (`rand`, `rand_chacha`, `rand_core`, `getrandom`). The workspace pins `sha2 0.11`, `hkdf 0.13`, `rand 0.8`, `rand_chacha 0.3` (`Cargo.toml`, map Section 3), so the other side of each split is pulled by dependencies — which dependency, and whether it matters, is for R10. A further 22 packages appear as two roots at a single version (e.g. `tokio v1.52.3`, `libc v0.2.186`, `alloy-primitives v1.6.0`); these are not version splits and are listed only in the log.

## 3. Inventory (run-2 scope)

Method: `wc -l` on every tracked `.rs` under `crates/core`, `crates/validator`, `crates/sentinel` at `3ec8bc5` (working tree identical to HEAD for `crates/`); tests = `grep -cE '#\[(tokio::)?test'`. Raw output: `logs/06-inventory.txt`. "Map" columns are `codebase-map.md` Section 2. The map also lists 22 `crates/sentinel-engine` files (5,113 lines, 97 tests at this commit) plus `crates/sentinel-engine/{Cargo.toml,Dockerfile,sentinel-engine.sample.toml,openapi.yaml}`; **all of these are excluded from run 2** and do not appear below.

### 3.1 `crates/core` — 23 files, 7,701 lines (map 7,644; +57), 99 tests (map 97; +2)

| File                                       | LOC  | Map  | Δ   | Tests | Map |
| ------------------------------------------ | ---- | ---- | --- | ----- | --- |
| `crates/core/src/driver.rs`                | 328  | 318  | +10 | 0     | 0   |
| `crates/core/src/effects.rs`               | 267  | 220  | +47 | 8     | 6   |
| `crates/core/src/index/blocks.rs`          | 1330 | 1330 | 0   | 23    | 23  |
| `crates/core/src/index/bloom.rs`           | 523  | 523  | 0   | 2     | 2   |
| `crates/core/src/index/clock.rs`           | 103  | 103  | 0   | 2     | 2   |
| `crates/core/src/index/events.rs`          | 1516 | 1516 | 0   | 19    | 19  |
| `crates/core/src/index/mod.rs`             | 468  | 468  | 0   | 5     | 5   |
| `crates/core/src/kdf.rs`                   | 81   | 81   | 0   | 4     | 4   |
| `crates/core/src/lib.rs`                   | 25   | 25   | 0   | 0     | 0   |
| `crates/core/src/metrics.rs`               | 90   | 90   | 0   | 0     | 0   |
| `crates/core/src/observability/logging.rs` | 21   | 21   | 0   | 0     | 0   |
| `crates/core/src/observability/metrics.rs` | 80   | 80   | 0   | 1     | 1   |
| `crates/core/src/observability/mod.rs`     | 94   | 94   | 0   | 2     | 2   |
| `crates/core/src/provider/mod.rs`          | 166  | 166  | 0   | 0     | 0   |
| `crates/core/src/serialization.rs`         | 34   | 34   | 0   | 0     | 0   |
| `crates/core/src/state/mod.rs`             | 644  | 644  | 0   | 6     | 6   |
| `crates/core/src/state/storage.rs`         | 294  | 294  | 0   | 7     | 7   |
| `crates/core/src/tx/fees.rs`               | 109  | 109  | 0   | 3     | 3   |
| `crates/core/src/tx/mod.rs`                | 719  | 719  | 0   | 9     | 9   |
| `crates/core/src/tx/signer.rs`             | 118  | 118  | 0   | 1     | 1   |
| `crates/core/src/tx/storage.rs`            | 507  | 507  | 0   | 7     | 7   |
| `crates/core/src/tx/types.rs`              | 87   | 87   | 0   | 0     | 0   |
| `crates/core/src/utils.rs`                 | 97   | 97   | 0   | 0     | 0   |

### 3.2 `crates/validator` — 28 files, 8,728 lines (map 8,098; +630), 42 tests (map 35; +7)

| File | LOC | Map | Δ | Tests | Map |
| --- | --- | --- | --- | --- | --- |
| `crates/validator/src/bindings.rs` | 247 | 247 | 0 | 0 | 0 |
| `crates/validator/src/config.rs` | 290 | 290 | 0 | 4 | 4 |
| `crates/validator/src/consensus/epoch.rs` | 95 | 95 | 0 | 1 | 1 |
| `crates/validator/src/consensus/group.rs` | 459 | 459 | 0 | 5 | 5 |
| `crates/validator/src/consensus/hashing.rs` | 249 | 249 | 0 | 4 | 4 |
| `crates/validator/src/consensus/mod.rs` | 5 | 5 | 0 | 0 | 0 |
| `crates/validator/src/frost/ecdh.rs` | 181 | 181 | 0 | 4 | 4 |
| `crates/validator/src/frost/error.rs` | 46 | 46 | 0 | 0 | 0 |
| `crates/validator/src/frost/keygen.rs` | 516 | 516 | 0 | 0 | 0 |
| `crates/validator/src/frost/marshal.rs` | 176 | 176 | 0 | 0 | 0 |
| `crates/validator/src/frost/mod.rs` | 258 | 258 | 0 | 1 | 1 |
| `crates/validator/src/frost/participants.rs` | 33 | 33 | 0 | 1 | 1 |
| `crates/validator/src/frost/preprocess.rs` | 189 | 189 | 0 | 1 | 1 |
| `crates/validator/src/frost/sign.rs` | 204 | 204 | 0 | 1 | 1 |
| `crates/validator/src/main.rs` | 99 | 99 | 0 | 0 | 0 |
| `crates/validator/src/merkle.rs` | 142 | 142 | 0 | 4 | 4 |
| `crates/validator/src/metrics.rs` | 181 | 132 | +49 | 0 | 0 |
| `crates/validator/src/secrets/mod.rs` | 6 | 6 | 0 | 0 | 0 |
| `crates/validator/src/secrets/nonces.rs` | 348 | 348 | 0 | 3 | 3 |
| `crates/validator/src/secrets/store.rs` | 969 | 447 | +522 | 13 | 6 |
| `crates/validator/src/service/action.rs` | 381 | 381 | 0 | 0 | 0 |
| `crates/validator/src/service/effect.rs` | 328 | 275 | +53 | 0 | 0 |
| `crates/validator/src/service/mod.rs` | 129 | 129 | 0 | 0 | 0 |
| `crates/validator/src/state/keygen.rs` | 1459 | 1459 | 0 | 0 | 0 |
| `crates/validator/src/state/mod.rs` | 516 | 515 | +1 | 0 | 0 |
| `crates/validator/src/state/preprocess.rs` | 253 | 248 | +5 | 0 | 0 |
| `crates/validator/src/state/sign.rs` | 868 | 868 | 0 | 0 | 0 |
| `crates/validator/src/state/transactions.rs` | 101 | 101 | 0 | 0 | 0 |

### 3.3 `crates/sentinel` — 10 files, 3,872 lines (map 3,348; +524), 42 tests (map 37; +5)

| File                              | LOC  | Map  | Δ    | Tests | Map |
| --------------------------------- | ---- | ---- | ---- | ----- | --- |
| `crates/sentinel/src/action.rs`   | 43   | 43   | 0    | 0     | 0   |
| `crates/sentinel/src/bindings.rs` | 173  | 170  | +3   | 0     | 0   |
| `crates/sentinel/src/config.rs`   | 144  | 144  | 0    | 4     | 4   |
| `crates/sentinel/src/effect.rs`   | 134  | 134  | 0    | 1     | 1   |
| `crates/sentinel/src/engine.rs`   | 392  | 392  | 0    | 10    | 10  |
| `crates/sentinel/src/hashing.rs`  | 224  | 224  | 0    | 5     | 5   |
| `crates/sentinel/src/main.rs`     | 89   | 89   | 0    | 0     | 0   |
| `crates/sentinel/src/metrics.rs`  | 143  | 134  | +9   | 0     | 0   |
| `crates/sentinel/src/service.rs`  | 2158 | 1851 | +307 | 16    | 15  |
| `crates/sentinel/src/state.rs`    | 372  | 167  | +205 | 6     | 2   |

### 3.4 Totals and mismatches

| Crate | Files | Lines now | Lines map | Δ | Tests now | Tests map | Δ |
| --- | --- | --- | --- | --- | --- | --- | --- |
| core | 23 | 7,701 | 7,644 | +57 | 99 | 97 | +2 |
| validator | 28 | 8,728 | 8,098 | +630 | 42 | 35 | +7 |
| sentinel | 10 | 3,872 | 3,348 | +524 | 42 | 37 | +5 |
| **Total** | **61** | **20,301** | **19,090** | **+1,211** | **183** | **169** | **+14** |

- No file was added or removed relative to the map: 61 tracked `.rs` files in both (`git ls-files` count 61; no untracked `.rs`).
- Exactly 11 files differ from the map, and they are exactly the 11 files in `git diff --stat 2893917 3ec8bc5 -- crates/` (Section 5): the pruning series changed `core/{driver,effects}.rs` and `validator/{metrics,secrets/store,service/effect,state/mod,state/preprocess}.rs`; the oracle-events/SEF-veto series changed `sentinel/{bindings,metrics,service,state}.rs`. **Every map line number in those 11 files is stale**; the other 50 files match the map exactly.
- The sentinel difference expected by the brief is confirmed: 42 tests, not 37.
- Executed test counts (Section 2.1) equal the attribute counts per crate, so no test is compiled out or filtered.

### 3.5 Non-Rust in-scope files

| File                                     | Lines | Owner (Section 4) |
| ---------------------------------------- | ----- | ----------------- |
| `Cargo.toml`                             | 26    | R10               |
| `Cargo.lock`                             | 6,169 | R10               |
| `crates/core/Cargo.toml`                 | 31    | R10               |
| `crates/validator/Cargo.toml`            | 28    | R10               |
| `crates/sentinel/Cargo.toml`             | 24    | R10               |
| `crates/validator/Dockerfile`            | 37    | R6                |
| `crates/sentinel/Dockerfile`             | 38    | R7                |
| `crates/validator/validator.sample.toml` | 77    | R6                |
| `crates/sentinel/sentinel.sample.toml`   | 61    | R7                |

All nine match the map's line counts. `crates/core` has no Dockerfile or sample config. Two further tracked files exist under the three crates but fall outside the Section 4 pattern: `crates/validator/Dockerfile.dockerignore` and `crates/sentinel/Dockerfile.dockerignore` (4 lines each) — context for R6/R7, not inventory.

## 4. Reviewer split for run 2

Modelled on `codebase-map.md` Section 9 with R8 and R9 (engine) removed and R10 kept minus its engine items; line counts re-measured at `3ec8bc5`. Every in-scope file appears under exactly one primary owner (verified mechanically: 70 of 70 files assigned once, none twice, none missing). "Start from" lists the map's seeded leads; note that leads in the 11 drifted files cite pre-merge line numbers and that the merged code (pruning/housekeeping in validator secrets and effects; oracle events and SEF veto in the sentinel) has **no seeded leads at all** and must be read fresh.

| Reviewer | Scope | Files | Lines (`.rs` / all) | Map lines | Start from (map Section 9) |
| --- | --- | --- | --- | --- | --- |
| R1 | core indexing and reorgs | 6 | 4,106 / 4,106 | 4,106 | CORE-H1, H2, H4, H8, H9, H14; core checklist 1, 3, 4, 8, 9 |
| R2 | core runtime, state, effects, observability | 12 | 2,055 / 2,055 | 1,993 | CORE-H3, H5, H10, H12, H13, M9, M10; core checklist 2, 10, 11, 12, 14 |
| R3 | core transaction queue | 5 | 1,540 / 1,540 | 1,540 | CORE-H6, H7, H11; core checklist 5, 6, 7 |
| R4 | validator DKG path | 10 | 3,228 / 3,228 | 3,228 | VAL-H1, H4, H5, H7, H9, M1, M3; validator checklist 1, 3, 7, 8, 12 |
| R5 | validator signing path and secrets | 10 | 3,329 / 3,329 | 2,802 | VAL-H3, H6, H8, H11, M4, M5, M6, M7; validator checklist 3, 4, 5, 10 |
| R6 | validator service, wiring, config | 10 | 2,171 / 2,285 | 2,068 | VAL-H2, H7, H10, CORE-H4; validator checklist 2, 6, 9, 11, 13 |
| R7 | sentinel | 12 | 3,872 / 3,971 | 3,348 | SEN-H1 to H15, M8; sentinel checklist 1 to 13 |
| R10 | cross-cutting (manifests, deps, CI, censuses) | 5 | 0 / 6,278 | n/a | VAL-H10, SEN-H14, SEN-H15, CORE-H17 (ENG-H14 dropped) |

R8 and R9 are not launched in run 2. Critics per the map: `C-CORE` (R1–R3), `C-VAL` (R4–R6), `C-SEN` (R7), one for R10, and the Coverage Critic; no `C-ENG`.

### R1 — core indexing and reorgs (4,106 lines)

| LOC  | File                              |
| ---- | --------------------------------- |
| 468  | `crates/core/src/index/mod.rs`    |
| 1330 | `crates/core/src/index/blocks.rs` |
| 1516 | `crates/core/src/index/events.rs` |
| 523  | `crates/core/src/index/bloom.rs`  |
| 103  | `crates/core/src/index/clock.rs`  |
| 166  | `crates/core/src/provider/mod.rs` |

### R2 — core runtime, state, effects, observability (2,055 lines)

| LOC | File                                        |
| --- | ------------------------------------------- |
| 328 | `crates/core/src/driver.rs` (drifted, +10)  |
| 267 | `crates/core/src/effects.rs` (drifted, +47) |
| 81  | `crates/core/src/kdf.rs`                    |
| 97  | `crates/core/src/utils.rs`                  |
| 34  | `crates/core/src/serialization.rs`          |
| 90  | `crates/core/src/metrics.rs`                |
| 25  | `crates/core/src/lib.rs`                    |
| 644 | `crates/core/src/state/mod.rs`              |
| 294 | `crates/core/src/state/storage.rs`          |
| 21  | `crates/core/src/observability/logging.rs`  |
| 80  | `crates/core/src/observability/metrics.rs`  |
| 94  | `crates/core/src/observability/mod.rs`      |

### R3 — core transaction queue (1,540 lines)

| LOC | File                            |
| --- | ------------------------------- |
| 719 | `crates/core/src/tx/mod.rs`     |
| 507 | `crates/core/src/tx/storage.rs` |
| 109 | `crates/core/src/tx/fees.rs`    |
| 118 | `crates/core/src/tx/signer.rs`  |
| 87  | `crates/core/src/tx/types.rs`   |

### R4 — validator DKG path (3,228 lines)

| LOC  | File                                         |
| ---- | -------------------------------------------- |
| 258  | `crates/validator/src/frost/mod.rs`          |
| 516  | `crates/validator/src/frost/keygen.rs`       |
| 181  | `crates/validator/src/frost/ecdh.rs`         |
| 33   | `crates/validator/src/frost/participants.rs` |
| 176  | `crates/validator/src/frost/marshal.rs`      |
| 46   | `crates/validator/src/frost/error.rs`        |
| 1459 | `crates/validator/src/state/keygen.rs`       |
| 5    | `crates/validator/src/consensus/mod.rs`      |
| 459  | `crates/validator/src/consensus/group.rs`    |
| 95   | `crates/validator/src/consensus/epoch.rs`    |

### R5 — validator signing path and secrets (3,329 lines)

| LOC | File |
| --- | --- |
| 189 | `crates/validator/src/frost/preprocess.rs` |
| 204 | `crates/validator/src/frost/sign.rs` |
| 142 | `crates/validator/src/merkle.rs` |
| 6 | `crates/validator/src/secrets/mod.rs` |
| 348 | `crates/validator/src/secrets/nonces.rs` |
| 969 | `crates/validator/src/secrets/store.rs` (drifted, +522: pruning/retention series #906–#913) |
| 253 | `crates/validator/src/state/preprocess.rs` (drifted, +5) |
| 868 | `crates/validator/src/state/sign.rs` |
| 101 | `crates/validator/src/state/transactions.rs` |
| 249 | `crates/validator/src/consensus/hashing.rs` |

### R6 — validator service, wiring, config (2,171 `.rs` + 114 = 2,285 lines)

| LOC | File                                                    |
| --- | ------------------------------------------------------- |
| 516 | `crates/validator/src/state/mod.rs` (drifted, +1)       |
| 381 | `crates/validator/src/service/action.rs`                |
| 328 | `crates/validator/src/service/effect.rs` (drifted, +53) |
| 129 | `crates/validator/src/service/mod.rs`                   |
| 247 | `crates/validator/src/bindings.rs`                      |
| 290 | `crates/validator/src/config.rs`                        |
| 99  | `crates/validator/src/main.rs`                          |
| 181 | `crates/validator/src/metrics.rs` (drifted, +49)        |
| 77  | `crates/validator/validator.sample.toml`                |
| 37  | `crates/validator/Dockerfile`                           |

### R7 — sentinel (3,872 `.rs` + 99 = 3,971 lines)

| LOC | File |
| --- | --- |
| 43 | `crates/sentinel/src/action.rs` |
| 173 | `crates/sentinel/src/bindings.rs` (drifted, +3) |
| 144 | `crates/sentinel/src/config.rs` |
| 134 | `crates/sentinel/src/effect.rs` |
| 392 | `crates/sentinel/src/engine.rs` |
| 224 | `crates/sentinel/src/hashing.rs` |
| 89 | `crates/sentinel/src/main.rs` |
| 143 | `crates/sentinel/src/metrics.rs` (drifted, +9) |
| 2158 | `crates/sentinel/src/service.rs` (drifted, +307: oracle events / SEF veto, #884–#891, #917) |
| 372 | `crates/sentinel/src/state.rs` (drifted, +205) |
| 61 | `crates/sentinel/sentinel.sample.toml` |
| 38 | `crates/sentinel/Dockerfile` |

### R10 — cross-cutting (6,278 lines of manifests)

| LOC  | File                          |
| ---- | ----------------------------- |
| 26   | `Cargo.toml`                  |
| 6169 | `Cargo.lock`                  |
| 31   | `crates/core/Cargo.toml`      |
| 28   | `crates/validator/Cargo.toml` |
| 24   | `crates/sentinel/Cargo.toml`  |

R10 additionally reads, as secondary (non-owning) coverage, the two Dockerfiles and two sample configs for cross-crate consistency, and performs the map's remaining R10 duties restricted to the three crates: the repo-wide sweep for secrets in `Debug`/`Display` impls, logs and metrics; the panic and `as`-cast censuses from the analyses; the `cargo audit` and `cargo tree -d` output in Section 2 (reachability of the 5 vulnerabilities and 11 warnings, and the RustCrypto/`rand` version splits); and CI gaps (`codebase-map.md` Section 3: no `cargo audit`/`cargo deny`, no Miri, no fuzzing).

Suggestions for the Manager (not decisions): R7 now carries 3,971 lines including ~520 lines of unmapped new code, and R5 carries 3,329 with ~530 unmapped lines in `secrets/store.rs`; both are candidates for the map's "split at the same scope boundary" rule if their observation counts balloon.

## 5. Drift note

Raw output: `logs/07-drift.txt`.

| Fact | Value |
| --- | --- |
| Audited HEAD | `3ec8bc57dc35d1e9e65075ae9424bff427c47833`, 2026-09-15 16:02:35 +0530, "Merge remote-tracking branch 'origin/main' into audit/rust-services" |
| `origin/main` | `5cc096e085ab1fe51afebd673a93fc12d0d59015` (second parent of HEAD) |
| Previous branch tip | `57044afee403da56c666b321cc20271153616576` (first parent) |
| Run-1 audited commit | `2893917757ae518ebb91154712cf3e401cb68d33`, 2026-09-08 |
| Old `origin/main` used in the brief | `49d7e398ac35c315da2b54d44a998b265855b43e`, 2026-09-09 (#893) |
| Identity with `origin/main` | `git diff --stat 3ec8bc5 origin/main -- crates/ Cargo.toml Cargo.lock` is empty; tree hash of `crates/` is `9f6038f3…` on both, `Cargo.lock` blob `28307b7e…` on both |
| Manifests since run 1 | `git diff --stat 2893917 3ec8bc5 -- Cargo.toml Cargo.lock` is empty — **dependency set unchanged since run 1** |

`git diff --stat 49d7e39 3ec8bc5 -- crates/ Cargo.toml Cargo.lock` (the pruning series, 7 files, +789/−102):

```text
 crates/core/src/driver.rs                |  16 +-
 crates/core/src/effects.rs               |  49 ++-
 crates/validator/src/metrics.rs          |  67 +++-
 crates/validator/src/secrets/store.rs    | 668 +++++++++++++++++++++++++++----
 crates/validator/src/service/effect.rs   |  79 +++-
 crates/validator/src/state/mod.rs        |   3 +-
 crates/validator/src/state/preprocess.rs |   9 +-
 7 files changed, 789 insertions(+), 102 deletions(-)
```

`git diff --stat 2893917 3ec8bc5 -- crates/` (everything since run 1, 11 files, +1348/−137):

```text
 crates/core/src/driver.rs                |  16 +-
 crates/core/src/effects.rs               |  49 ++-
 crates/sentinel/src/bindings.rs          |   3 +
 crates/sentinel/src/metrics.rs           |  13 +-
 crates/sentinel/src/service.rs           | 371 +++++++++++++++--
 crates/sentinel/src/state.rs             | 207 +++++++++-
 crates/validator/src/metrics.rs          |  67 +++-
 crates/validator/src/secrets/store.rs    | 668 +++++++++++++++++++++++++++----
 crates/validator/src/service/effect.rs   |  79 +++-
 crates/validator/src/state/mod.rs        |   3 +-
 crates/validator/src/state/preprocess.rs |   9 +-
 11 files changed, 1348 insertions(+), 137 deletions(-)
```

Commits on `main` between `49d7e39` and `5cc096e` (first-parent): `246d28e` [Epic] SEF Veto for Reality Module Proposals (#917); `224845b` Pruning Plan (#906); `4a74b41` Prune 1: Extend Effect Handler for Housekeeping (#907); `545c890` Phase 2: Update Database Structure for Pruning (#908); `8b88f4a` Prune 3: Schedule Group Secret Deletion Through Reconciliation (#909); `c87c054` Prune 4: Collect Scheduled Secrets On Housekeeping (#910); `f0fdc40` Prune 5: Document Secret Retention (#912); `80951a0` Prune End (#913); `5cc096e` CI fix (#923). Between `2893917` and `49d7e39`, the `crates/` changes come from the sentinel oracle-event series #884, #885, #888, #889, #890, #891.

**Branch HEAD moved during Phase 0, audited tree did not.** Two commits landed on `audit/rust-services` at 16:07:58 +0530 while the baseline was running: `1f3fe52` "audit: run-2 scope -- engine out, A16/A17 in, engine artefacts removed" and `4ac6838` "audit: record reviewer threads on PR #905 and their dispositions". Together they touch 105 paths, **all under `rust-audit/`**; `git diff --stat 3ec8bc5 4ac6838 -- crates/ Cargo.toml Cargo.lock` is empty and the object hashes are identical on both commits (`crates/` tree `9f6038f3…`, `Cargo.lock` blob `28307b7e…`, `Cargo.toml` blob `c1d67bcf…`); `3ec8bc5` is an ancestor of `4ac6838`. Consequently the log headers differ: `01-build.log` and `02-test.log` record `# commit: 3ec8bc5…`, while `03-clippy.log`, `04-audit.log`, `05-tree-d.log` and `08-future-incompat.log` record `# commit: 4ac6838…` — every one of them ran against the same audited code. This baseline keeps **`3ec8bc5`** as the audited commit, as the Manager instructed; if the Manager prefers to name the branch tip instead, only the hash label changes, not the evidence. A14 is unaffected because the tree under audit is unchanged.

Working tree: `git status --short` lists **nothing outside `rust-audit/`**; `git stash list` is empty; `target/` is git-ignored (`.gitignore:11`) and the run-2 target directory is outside the repository anyway. Entries inside `rust-audit/` (103 at the time of the check) are the Manager's in-progress, intentional edits and were not read.

## 6. Notes for the Manager

- Assumption A14 (tree unchanged during the run) now binds to `3ec8bc5`; a further merge of `origin/main` would require restarting Phase 0.
- No blocker. Mode is `full`; Anvil-based scripts are available for QA (Foundry 1.8.1).
- `sqlite3` CLI is absent; if a QA agent needs to inspect a SQLite file it must use `sqlx` from a test or install the CLI (not done here).
- The harness shell snapshot exports a third-party API key into every command environment; it surfaced in a process listing during this run and was deliberately not written to any file.
