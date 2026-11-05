# Run 3 baseline (Phase 0, Recon3)

| Field | Value |
| --- | --- |
| Audit commit | `361aaf3b25e12d2e04f296184c326830968f3727` (outside `rust-audit/` byte-identical to `c355af1`: `git diff --stat 361aaf3 c355af1 -- . ':(exclude)rust-audit'` is empty) |
| Run 2 commit | `fe9e84cc59b65367b31d5a3121774383cc422234` (2026-09-22) |
| Checkout | detached worktree `wt/base` in the local run folder, at the audit commit |
| Date | 2026-10-08, commands run between 14:15 and 14:31 UTC |
| Mode | full: every A9 requirement is met, nothing ran read-only |
| Logs | trimmed copies in [`logs/`](./logs/) (progress lines dropped, local paths shortened to a run-folder placeholder and `~`); full raw logs in the local run folder's `out/recon/` |

Totals: build, unit tests, clippy 1.96 and clippy 1.99 all exit 0; 364 unit tests pass (242 in scope); `cargo audit` reports 5 vulnerabilities and 11 warnings, 9 of the 16 crates are compiled into the in-scope packages; all 7 validator and sentinel integration suites pass on the first run. Two assumption values drifted from `settled.md` section 3: `oracle_timeout` is 24 (not 12) and the sentinel's `voting_window` sample is 6 (not 100). The Aegis production oracle windows are 6 and 6 blocks with a 483,840-block arbitration timeout.

## 1. Toolchain (A9)

| Item | Value | Evidence |
| --- | --- | --- |
| Host | Linux 6.17.0-41-generic, aarch64 VM, 10 cores, 23 GiB RAM (12 GiB available at start), no swap | `uname -a`, `nproc`, `free -h` |
| Disk | 242 GB, 184 GB free at start, 178 GB free after the run | `df -h /` |
| Rust default | `rustc 1.96.0 (ac68faa20 2026-05-25)`, `cargo 1.96.0`, `clippy 0.1.96`, `rustfmt 1.9.0` | `rustc -Vv`, `cargo -V`, `cargo clippy -V` |
| Rust 1.99 | `rustc 1.99.0 (b940084d7 2026-09-28)`, `cargo 1.99.0`, `clippy 0.1.99` | `cargo +1.99.0 -V`, `cargo +1.99.0 clippy -V` |
| Toolchain file | none in the repository (`rust-toolchain*` absent), so the default 1.96 builds | `ls rust-toolchain*` |
| Foundry | `forge`, `anvil`, `cast` `1.5.1-v1.5.1` (commit `b0a9dd9`) | `--version` of each |
| solc | 0.8.30 cached in `~/.local/share/svm/0.8.30`, so forge builds offline | `ls` |
| Other | `just 1.40.0`, `jq-1.8.1`, `podman 5.4.2`, `git 2.51.0`, `bash 5.2.37`, Python 3.13.7 | `--version` of each |
| cargo-audit | missing at start; installed `cargo-audit v0.22.2` with `cargo install cargo-audit --locked` (built in a separate target directory under the local run folder's `tmp/`, so `target-base` holds only the workspace) | `logs/00-toolchain.log` |
| Advisory DB | RustSec advisory-db fetched 2026-10-08 14:20 UTC, 1,294 advisories loaded | `logs/05-audit.log` |

A9 holds: Rust stable, Foundry 1.5.1, `just`, `jq`, at least 8 GB RAM and 15 GB free disk. Network use was limited to the Cargo registry (cargo-audit install, the crates.io index for the yanked check) and the RustSec advisory DB.

## 2. Commands

All commands ran in the base worktree with `env/base.sh` sourced (target directory `target-base` in the local run folder; clippy 1.99 uses `target-clippy199`).

| Command | Exit | Counts | Duration | Log |
| --- | --: | --- | --: | --- |
| `cargo build --workspace --all-targets --locked` | 0 | 349 crates compiled, 0 warnings, no future-incompatibility notice | 75 s | `logs/01-build.log` |
| `cargo test --workspace` | 0 | 364 passed, 0 failed, 0 ignored (per-crate table below) | 14 s | `logs/02-test.log` |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` (1.96) | 0 | 0 warnings, 0 errors | 27 s | `logs/03-clippy-196.log` |
| `clippy199` (`cargo +1.99.0 clippy --workspace --all-targets --locked -- -D warnings`) | 0 | 0 warnings, 0 errors in workspace code; one future-incompatibility notice for `proc-macro-error2 v2.0.1` | 52 s | `logs/04-clippy-199.log` |
| `cargo +1.99.0 report future-incompatibilities --id 1` (against `target-clippy199`) | 0 | `proc-macro-error2 2.0.1`: `warning[E0365]: extern crate proc_macro is private and cannot be re-exported` at its `src/lib.rs:494` (`pub use proc_macro;`), rust-lang/rust#127909; it is a dependency of `alloy-sol-macro 1.6.0` | 0 s | appended to `logs/04-clippy-199.log` |
| `cargo audit` | 1 | 570 crates scanned: 5 vulnerabilities, 11 warnings (4 unmaintained, 3 unsound, 4 yanked) | 16 s | `logs/05-audit.log` |
| `cargo tree -i <crate@version> -e normal -p <pkg>` for each advisory crate and each in-scope package, host target and `--target all` | 0 | 9 of 16 crates reachable from all three in-scope packages; 1 only for non-host targets; 6 lockfile-only (section 3) |  | `logs/05-audit.log` |
| `cargo tree -d -p safenet-core -p validator -p sentinel` (default edges, then `-e normal`) | 0 | 13 crates with two or more versions, the same 13 with `-e normal` (section 4) |  | `logs/06-tree-d.log` |

The 1.96 build, test and clippy runs printed no future-incompatibility notice and `target-base` holds no report, so `cargo report future-incompatibilities` was only needed for 1.99.

Unit tests per test binary:

| Crate | Binary | Passed | Failed | Ignored | In scope |
| --- | --- | --: | --: | --: | --- |
| `safenet-core` | unittests `src/lib.rs` | 151 | 0 | 0 | yes |
| `validator` | unittests `src/main.rs` | 41 | 0 | 0 | yes |
| `sentinel` | unittests `src/main.rs` | 50 | 0 | 0 | yes |
| `sentinel-engine` | unittests `src/main.rs` | 122 | 0 | 0 | no (out of scope) |
| `safenet-core` | doc-tests | 0 | 0 | 0 | yes |
| In scope |  | 242 | 0 | 0 |  |
| Workspace |  | 364 | 0 | 0 |  |

The executed in-scope counts equal the static count of `#[test]`, `#[tokio::test]` and `#[tokio::test(...)]` attributes in the in-scope files (107 + 120 + 15 = 242). At `fe9e84c` the static counts were 99 (core), 42 (validator) and 44 (sentinel).

## 3. Advisories and reachability

Reachability is `cargo tree --locked -i <crate@version> -e normal -p <pkg>` for `safenet-core`, `validator` and `sentinel`, on the host target (aarch64 Linux) and with `--target all`; the two agree for every crate except `atomic-polyfill`. In the path column, `a < b` means `a` is a dependency of `b`. "Compiled" means the crate appears as `Compiling` in the workspace build log. Severity is not judged here; the "Kind" column is the advisory's own classification. Every one of the 16 crates has the same version at `fe9e84c`.

| Crate | Version | ID | Kind | Advisory title | `Cargo.lock` line | Reachable (normal edges) | Path into scope | Fixed in |
| --- | --- | --- | --- | --- | --: | --- | --- | --- |
| `crossbeam-epoch` | 0.9.18 | RUSTSEC-2026-0204 | vulnerability | Invalid pointer dereference in `fmt::Pointer` impl for `Atomic` and `Shared` | 1582 | core, validator, sentinel | `metrics-util 0.20.4` < `metrics-exporter-prometheus 0.18.3` < `safenet-core` | >=0.9.20 |
| `h2` | 0.4.14 | RUSTSEC-2026-0258 | vulnerability | h2 unbounded empty DATA frames | 2329 | core, validator, sentinel | `hyper 1.10.1` < `reqwest 0.13.4` (alloy HTTP transport) and `hyper-rustls` < `metrics-exporter-prometheus` | >=0.4.16 |
| `quinn-proto` | 0.11.14 | RUSTSEC-2026-0185 | vulnerability (CVSS 7.5 per advisory) | Remote memory exhaustion from unbounded out-of-order stream reassembly | 3572 | none, any target | lockfile-only: parent `quinn 0.11.9` is not activated for any workspace package; not compiled | >=0.11.15 |
| `ruint` | 1.18.0 | RUSTSEC-2026-0220 | vulnerability | Uint shift operations: incorrect overflow flags and truncated shift amounts | 3885 | core, validator, sentinel | `alloy-primitives 1.6.0` < `alloy` | >=1.20.0 |
| `rustls` | 0.23.40 | RUSTSEC-2026-0285 | vulnerability (CVSS 5.3 per advisory) | TLS 1.3 handshake messages accepted across encryption level boundaries | 3962 | core, validator, sentinel | `alloy-transport-ws 2.0.5`, `hyper-rustls 0.27.9` (< `reqwest`, `metrics-exporter-prometheus`) | >=0.23.45 |
| `atomic-polyfill` | 1.0.3 | RUSTSEC-2023-0089 | unmaintained | atomic-polyfill is unmaintained | 1090 | host: none; `--target all`: validator only | `heapless 0.7.17` < `postcard 1.1.3` < `frost-core 3.0.0`, only on targets without native atomics; not compiled |  |
| `derivative` | 2.2.0 | RUSTSEC-2024-0388 | unmaintained | `derivative` is unmaintained | 1727 | none, any target | lockfile-only (parents `ark-ff 0.3.0`, `0.4.2`); not compiled |  |
| `paste` | 1.0.15 | RUSTSEC-2024-0436 | unmaintained | paste - no longer maintained | 3328 | core, validator, sentinel | proc-macro of `alloy-primitives 1.6.0` |  |
| `proc-macro-error2` | 2.0.1 | RUSTSEC-2026-0173 | unmaintained | proc-macro-error2 is unmaintained | 3491 | core, validator, sentinel | proc-macro dependency of `alloy-sol-macro 1.6.0`; also the 1.99 future-incompatibility warning |  |
| `anyhow` | 1.0.102 | RUSTSEC-2026-0190 | unsound | Unsoundness in `Error::downcast_mut()` | 805 | none, any target | lockfile-only (parents `wit-bindgen-*`, `wasm-metadata`); not compiled |  |
| `event-listener` | 5.4.1 | RUSTSEC-2026-0221 | unsound | allows `!Send` tags to cross thread boundaries via `StackSlot` | 1963 | core, validator, sentinel | `sqlx-core 0.9.0` < `sqlx` |  |
| `lru` | 0.16.4 | RUSTSEC-2026-0253 | unsound | Potential use-after-free due to lack of panic safety in `LruCache::pop()` | 3046 | core, validator, sentinel | `alloy-provider 2.0.5` < `alloy` |  |
| `bitcoin-io` | 0.1.100 | (none) | yanked |  | 1235 | none, any target | lockfile-only (parent `bitcoin_hashes 0.14.100`); not compiled |  |
| `bitcoin_hashes` | 0.14.100 | (none) | yanked |  | 1241 | none, any target | lockfile-only (parents `secp256k1 0.30.0`, `0.31.1`); not compiled |  |
| `chacha20` | 0.10.0 | (none) | yanked |  | 1401 | none, any target | lockfile-only (parent `rand 0.10.1`); not compiled |  |
| `spin` | 0.9.8 | (none) | yanked |  | 4554 | core, validator, sentinel | `flume 0.12.0` < `sqlx-sqlite 0.9.0` < `sqlx` |  |

The six lockfile-only crates (`quinn-proto`, `derivative`, `anyhow`, `bitcoin-io`, `bitcoin_hashes`, `chacha20`) print "nothing to print" even for `cargo tree -i <crate> --workspace -e all --target all`, so no workspace package reaches them with any edge kind on any target. `Cargo.lock` changes since `fe9e84c` only removed `crossbeam-deque 0.8.7`, `rayon 1.12.0`, `rayon-core 1.13.0` and the validator's direct `rand_chacha` dependency, and gave `safenet-core` direct dependencies on `reqwest` and `toml` (both already locked); none of them touches the flagged crates.

## 4. Duplicate versions

`cargo tree -d -p safenet-core -p validator -p sentinel` lists 13 crates with two or more versions, and the same 13 with `-e normal`. A further 21 crates are listed twice with a single version (cargo lists a version again when it is built with a different feature set, for example on the host for a proc-macro); they are not version duplicates and are left out here (full list in `logs/06-tree-d.log`).

| Crate | Versions | Where each comes from (`cargo tree -i <crate@version> -e normal --depth 1`) |
| --- | --- | --- |
| `sha2` | 0.10.9, 0.11.0 | 0.11.0: `safenet-core` directly (`crates/core/Cargo.toml:21`); 0.10.9: `k256`, `frost-secp256k1`, `alloy-eips`, `sqlx-core`, `sqlx-macros-core` |
| `digest` | 0.10.7, 0.11.3 | 0.11.3: `sha2 0.11`, `hmac 0.13`, `sha3 0.11`; 0.10.7: `ecdsa`, `elliptic-curve`, `hmac 0.12`, `sha1`, `sha2 0.10`, `signature` |
| `hmac` | 0.12.1, 0.13.0 | 0.13.0: `hkdf 0.13.0` < `safenet-core`; 0.12.1: `rfc6979` < `ecdsa` |
| `block-buffer` | 0.10.4, 0.12.0 | follows `digest` 0.10 and 0.11 |
| `crypto-common` | 0.1.6, 0.2.2 | follows `digest` 0.10 and 0.11 |
| `const-oid` | 0.9.6, 0.10.2 | follows the RustCrypto 0.10 and 0.11 generations |
| `cpufeatures` | 0.2.17, 0.3.0 | follows the RustCrypto 0.10 and 0.11 generations |
| `rand` | 0.8.6, 0.9.4 | 0.8.6: `validator` directly, `alloy-signer-local`; 0.9.4: `metrics-util`, `tungstenite` |
| `rand_core` | 0.6.4, 0.9.5 | 0.6.4: `frost-core`, `frost-secp256k1`, `frost-rerandomized`, `elliptic-curve`, `ff`, `group`, `crypto-bigint`, `signature`, `rand 0.8`; 0.9.5: `rand 0.9`, `rand_chacha 0.9`, `rand_xoshiro` |
| `rand_chacha` | 0.3.1, 0.9.0 | follows `rand` 0.8 and 0.9 |
| `getrandom` | 0.2.17, 0.3.4 | 0.2.17: `rand_core 0.6`; 0.3.4: `rand_core 0.9` |
| `hashbrown` | 0.14.5, 0.16.1, 0.17.1 | several transitive users (see log) |
| `webpki-roots` | 0.26.11, 1.0.7 | 0.26.11 (`tokio-tungstenite 0.28.0`) re-exports 1.0.7 |

## 5. Inventory

Scope per PROMPT.md section 12: every `.rs` file of `crates/core`, `crates/validator` and `crates/sentinel`, the workspace `Cargo.toml` and `Cargo.lock`, and those crates' `Cargo.toml`, `Dockerfile` and `*.sample.toml` (`crates/core` has neither a Dockerfile nor a sample; `crates/*/Dockerfile.dockerignore` is outside the listed scope). Line counts are `wc -l` at `361aaf3`. Change data is `git diff --numstat --no-renames fe9e84c 361aaf3` (no renames are detected with the default `-M` either). "Test fns" counts `#[test]` and `#[tokio::test]` attributes in the file.

Per crate:

| Crate | Files now | `.rs` files | `.rs` lines now | `.rs` lines at `fe9e84c` | `.rs` added | modified | deleted | unchanged | `.rs` + | `.rs` - | Test fns |
| --- | --: | --: | --: | --: | --: | --: | --: | --: | --: | --: | --: |
| core | 27 | 26 | 10,723 | 7,701 | 3 | 13 | 0 | 10 | 3,422 | 400 | 151 |
| validator | 30 | 27 | 8,221 | 8,728 | 2 | 16 | 3 | 9 | 1,227 | 1,734 | 41 |
| sentinel | 14 | 11 | 4,375 | 4,063 | 1 | 8 | 0 | 2 | 611 | 299 | 50 |
| All in scope | 73 | 64 | 23,319 | 20,492 | 6 | 37 | 3 | 21 | 5,260 | 2,433 | 242 |

Non-Rust files in scope: `Cargo.lock` 6,141 lines (+2/-32), `Cargo.toml` 26 (+0/-1), `crates/core/Cargo.toml` 33 (+2/-0), `crates/validator/Cargo.toml` 26 (+0/-2), `crates/validator/Dockerfile` 37 (unchanged), `crates/validator/validator.sample.toml` 104 (+28/-1), `crates/sentinel/Cargo.toml` 24 (unchanged), `crates/sentinel/Dockerfile` 38 (unchanged), `crates/sentinel/sentinel.sample.toml` 89 (+31/-3). With them, the in-scope change since `fe9e84c` is 6 files added, 43 modified, 3 deleted and 24 unchanged, +5,323/-2,472 lines. 50 non-merge commits touch the scope in `fe9e84c..361aaf3` (`git log --no-merges fe9e84c..361aaf3 -- crates/core crates/validator crates/sentinel Cargo.toml Cargo.lock`).

Per file:

| File | Lines at `361aaf3` | Change since `fe9e84c` | + | - | Test fns |
| --- | --: | --- | --: | --: | --: |
| `Cargo.lock` | 6141 | modified | 2 | 32 |  |
| `Cargo.toml` | 26 | modified | 0 | 1 |  |
| `crates/core/Cargo.toml` | 33 | modified | 2 | 0 |  |
| `crates/core/src/config.rs` | 157 | added | 157 | 0 | 2 |
| `crates/core/src/driver.rs` | 328 | unchanged | 0 | 0 | 0 |
| `crates/core/src/effects.rs` | 267 | modified | 1 | 1 | 8 |
| `crates/core/src/index/blocks.rs` | 1429 | modified | 166 | 67 | 26 |
| `crates/core/src/index/bloom.rs` | 523 | unchanged | 0 | 0 | 2 |
| `crates/core/src/index/clock.rs` | 103 | unchanged | 0 | 0 | 2 |
| `crates/core/src/index/events.rs` | 2051 | modified | 619 | 84 | 24 |
| `crates/core/src/index/mod.rs` | 485 | modified | 28 | 11 | 5 |
| `crates/core/src/kdf.rs` | 81 | unchanged | 0 | 0 | 4 |
| `crates/core/src/lib.rs` | 26 | modified | 1 | 0 | 0 |
| `crates/core/src/metrics.rs` | 107 | modified | 18 | 1 | 0 |
| `crates/core/src/observability/logging.rs` | 21 | unchanged | 0 | 0 | 0 |
| `crates/core/src/observability/metrics.rs` | 80 | unchanged | 0 | 0 | 1 |
| `crates/core/src/observability/mod.rs` | 94 | unchanged | 0 | 0 | 2 |
| `crates/core/src/provider/mod.rs` | 182 | modified | 18 | 2 | 0 |
| `crates/core/src/serialization.rs` | 218 | modified | 184 | 0 | 6 |
| `crates/core/src/state/mod.rs` | 731 | modified | 118 | 31 | 6 |
| `crates/core/src/state/storage.rs` | 294 | unchanged | 0 | 0 | 7 |
| `crates/core/src/tx/bundle.rs` | 407 | added | 407 | 0 | 8 |
| `crates/core/src/tx/config.rs` | 194 | added | 194 | 0 | 2 |
| `crates/core/src/tx/fees.rs` | 109 | unchanged | 0 | 0 | 3 |
| `crates/core/src/tx/mod.rs` | 1286 | modified | 654 | 87 | 23 |
| `crates/core/src/tx/signer.rs` | 297 | modified | 203 | 24 | 5 |
| `crates/core/src/tx/storage.rs` | 990 | modified | 565 | 82 | 15 |
| `crates/core/src/tx/types.rs` | 166 | modified | 89 | 10 | 0 |
| `crates/core/src/utils.rs` | 97 | unchanged | 0 | 0 | 0 |
| `crates/validator/Cargo.toml` | 26 | modified | 0 | 2 |  |
| `crates/validator/Dockerfile` | 37 | unchanged | 0 | 0 |  |
| `crates/validator/src/bindings.rs` | 245 | modified | 8 | 10 | 0 |
| `crates/validator/src/config.rs` | 281 | modified | 11 | 20 | 4 |
| `crates/validator/src/consensus/epoch.rs` | 95 | unchanged | 0 | 0 | 1 |
| `crates/validator/src/consensus/group.rs` | 459 | unchanged | 0 | 0 | 5 |
| `crates/validator/src/consensus/hashing.rs` | 249 | unchanged | 0 | 0 | 4 |
| `crates/validator/src/consensus/mod.rs` | 5 | unchanged | 0 | 0 | 0 |
| `crates/validator/src/frost/ecdh.rs` | 316 | modified | 143 | 8 | 6 |
| `crates/validator/src/frost/error.rs` | 46 | unchanged | 0 | 0 | 0 |
| `crates/validator/src/frost/keygen.rs` | 515 | modified | 17 | 18 | 0 |
| `crates/validator/src/frost/marshal.rs` | 181 | modified | 21 | 16 | 0 |
| `crates/validator/src/frost/mod.rs` | 325 | modified | 92 | 25 | 3 |
| `crates/validator/src/frost/participants.rs` | 33 | unchanged | 0 | 0 | 1 |
| `crates/validator/src/frost/preprocess.rs` | (deleted, 189 at base) | deleted | 0 | 189 |  |
| `crates/validator/src/frost/serialization.rs` | 85 | added | 85 | 0 | 0 |
| `crates/validator/src/frost/sign.rs` | 236 | modified | 61 | 29 | 1 |
| `crates/validator/src/main.rs` | 99 | unchanged | 0 | 0 | 0 |
| `crates/validator/src/merkle.rs` | 142 | unchanged | 0 | 0 | 4 |
| `crates/validator/src/metrics.rs` | 173 | modified | 9 | 17 | 0 |
| `crates/validator/src/secrets/mod.rs` | 5 | modified | 0 | 1 | 0 |
| `crates/validator/src/secrets/nonces.rs` | (deleted, 348 at base) | deleted | 0 | 348 |  |
| `crates/validator/src/secrets/store.rs` | 1056 | modified | 330 | 243 | 12 |
| `crates/validator/src/service/action.rs` | 353 | modified | 6 | 34 | 0 |
| `crates/validator/src/service/effect.rs` | 235 | modified | 50 | 143 | 0 |
| `crates/validator/src/service/mod.rs` | 129 | unchanged | 0 | 0 | 0 |
| `crates/validator/src/state/keygen.rs` | 1447 | modified | 114 | 126 | 0 |
| `crates/validator/src/state/mod.rs` | 487 | modified | 39 | 68 | 0 |
| `crates/validator/src/state/preprocess.rs` | (deleted, 253 at base) | deleted | 0 | 253 |  |
| `crates/validator/src/state/secrets.rs` | 86 | added | 86 | 0 | 0 |
| `crates/validator/src/state/sign.rs` | 836 | modified | 154 | 186 | 0 |
| `crates/validator/src/state/transactions.rs` | 102 | modified | 1 | 0 | 0 |
| `crates/validator/validator.sample.toml` | 104 | modified | 28 | 1 |  |
| `crates/sentinel/Cargo.toml` | 24 | unchanged | 0 | 0 |  |
| `crates/sentinel/Dockerfile` | 38 | unchanged | 0 | 0 |  |
| `crates/sentinel/sentinel.sample.toml` | 89 | modified | 31 | 3 |  |
| `crates/sentinel/src/action.rs` | 46 | modified | 3 | 0 | 0 |
| `crates/sentinel/src/bindings.rs` | 176 | modified | 1 | 0 | 0 |
| `crates/sentinel/src/config.rs` | 135 | modified | 8 | 17 | 4 |
| `crates/sentinel/src/effect.rs` | 263 | modified | 168 | 39 | 3 |
| `crates/sentinel/src/engine.rs` | 430 | modified | 54 | 16 | 11 |
| `crates/sentinel/src/hashing.rs` | 224 | unchanged | 0 | 0 | 5 |
| `crates/sentinel/src/main.rs` | 93 | modified | 6 | 2 | 0 |
| `crates/sentinel/src/metrics.rs` | 143 | unchanged | 0 | 0 | 0 |
| `crates/sentinel/src/service.rs` | 2214 | modified | 91 | 224 | 16 |
| `crates/sentinel/src/state.rs` | 383 | modified | 12 | 1 | 6 |
| `crates/sentinel/src/verdicts.rs` | 268 | added | 268 | 0 | 5 |

## 6. Refreshed assumption values

### A6: pinned library versions

| Crate | Locked version | `Cargo.lock` | Manifest requirement | Since `fe9e84c` |
| --- | --- | --- | --- | --- |
| `frost-core` | 3.0.0 | `Cargo.lock:2079-2080` | `crates/validator/Cargo.toml:10` (`"3"`, feature `internals`) | unchanged |
| `frost-secp256k1` | 3.0.0 | `Cargo.lock:2114-2115` | `crates/validator/Cargo.toml:11` (`"3"`) | unchanged |
| `k256` | 0.13.4 | `Cargo.lock:2904-2905` | `Cargo.toml:12` (`"0.13"`, features `hash2curve`, `serde`) | unchanged |
| `sha2` | 0.11.0 (direct) and 0.10.9 (transitive) | `Cargo.lock:4441-4442`, `Cargo.lock:4430-4431` | `crates/core/Cargo.toml:21` (`"0.11"`) | unchanged |
| `hkdf` | 0.13.0 | `Cargo.lock:2456-2457` | `crates/core/Cargo.toml:13` (`"0.13"`) | unchanged |
| `alloy` | 2.0.5 | `Cargo.lock:21-22` | `Cargo.toml:6` (`"2"`, features `full`, `json-rpc`) | unchanged |
| `sqlx` | 0.9.0 | `Cargo.lock:4573-4574` | `Cargo.toml:19` (`"0.9"`, features `sqlite`, `runtime-tokio`; system SQLite, no `bundled`) | unchanged |
| `tokio` | 1.52.3 | `Cargo.lock:4967-4968` | `Cargo.toml:21` (`"1"`, feature `full`) | unchanged |
| `reqwest` | 0.13.4 | `Cargo.lock:3814-3815` | `Cargo.toml:15` (`"0.13"`, no default features, `json`, `rustls`); now also a direct dependency of `safenet-core` (`crates/core/Cargo.toml:18`) | unchanged version |

### A10: defaults in code, samples and docs

| Parameter | Code default | Sample config | Docs | Notes |
| --- | --- | --- | --- | --- |
| `blocks_per_epoch` | 1440 (`crates/validator/src/config.rs:61-63`, `:77-79`) | commented `1440` (`crates/validator/validator.sample.toml:48`) | 1440, about 2 hours (`docs/configuration.md:38-39`) | matches `settled.md` |
| `key_gen_timeout` | 120 blocks (`config.rs:64-67`, `:81-83`) | commented `120` (`validator.sample.toml:49`) | 120, about 10 minutes (`docs/configuration.md:40-41`) | matches |
| `signing_timeout` | 6 blocks (`config.rs:68-70`, `:85-87`) | commented `6` (`validator.sample.toml:50`) | 6, about 30 seconds (`docs/configuration.md:42-43`) | matches |
| `oracle_timeout` | 24 blocks (`config.rs:71-73`, `:89-91`) | commented `24` (`validator.sample.toml:51`) | 24, about 2 minutes (`docs/configuration.md:44-45`) | _differs_: `settled.md` and PROMPT.md A10 say 12; changed 12 to 24 by `fcc7028` (#1062, 2026-10-07) |
| Threshold | `count / 2 + 1` (`crates/validator/src/consensus/group.rs:219-222`, test `:330-335`) |  | `threshold = (group_size / 2) + 1` (`docs/overview.md:87`) | Aegis: 5 validators, threshold 3, matches |
| Minimum KeyGen participants | `max(2, count * 2 / 3 + 1)` (`group.rs:224-240`) |  | `((total_validators * 2) / 3) + 1` (`docs/overview.md:88`), no floor of two | Aegis: 4 of 5 |
| Nonce chunk 1024 (PROMPT.md A10) | removed: no preprocessing or nonce chunks remain; `frost/preprocess.rs`, `secrets/nonces.rs` and `state/preprocess.rs` were deleted by Direct Nonce (#1032 to #1044) |  | a fresh nonce pair per signing ceremony (`docs/overview.md:58`, `:65-68`) | _obsolete_; no `1024` in scope code or docs |
| `max_reorg_depth` | 5 (`crates/core/src/index/blocks.rs:135`, documented at `:86-97`) | not set in either sample | handbooks name the "reorg window" without a number (`docs/validator-handbook.md:95`) | not in `settled.md`; the deep-reorg suite sets 2, the reorg-nonce suite 10 (validator A) and 30 (validator B), the other suites use the default |
| `block_time` | `auto` (`blocks.rs:132`); Gnosis (chain 100) 5,000 ms, Sepolia 12,000 ms (`blocks.rs:52-71`); any other chain fails with `UnknownChainTimings` unless all three timings are set (`blocks.rs:30-38`, `:110-126`) | not set (`auto`) |  | 5 s blocks, matches |
| `block_propagation_delay` (#1010) | `auto`; Gnosis 500 ms, Sepolia 1,000 ms (`blocks.rs:59`, `:65`) | not set |  | new since run 2 (`96e16c8`, #1010) |
| `block_retry_delays` (#1010) | `auto`; Gnosis `[500, 500, 500, 1000, 250, 250]` ms (3,000 ms in total), Sepolia `[1000, 1000, 1000, 500, 500]` ms (`blocks.rs:60`, `:66`); after the list the watcher waits a whole `block_time` (`blocks.rs:82-85`) | not set |  | new since run 2 (#1010) |
| Event watcher | `block_page_size` 100, `block_single_query_retry_count` 3, `use_client_filtering` false, `max_logs_per_query` none, `fallible_events` empty (`crates/core/src/index/events.rs:105-115`); `start_block` none (`blocks.rs:136`) | `use_client_filtering = true` commented (`validator.sample.toml:75`, `sentinel.sample.toml:60`) | handbooks recommend it for unreliable `eth_getLogs` (`docs/validator-handbook.md:35-44`) |  |
| In-flight limit | Direct mode, `max_in_flight_transactions` 16 (`crates/core/src/tx/config.rs:7`, `:27-37`, `:110-113`); Batched mode (an `executor` is set) always 1 (`tx/config.rs:62-74`) | commented `16` (`validator.sample.toml:80`, `sentinel.sample.toml:65`) | 7702 tips (`docs/validator-handbook.md:77`, `docs/sentinel-handbook.md:69`) |  |
| `executor` | none (Direct mode); zero address rejected; cannot be combined with `max_in_flight_transactions` (`tx/config.rs:114-127`) | commented zero address (`validator.sample.toml:93`, `sentinel.sample.toml:78`) | as above |  |
| `max_batch_gas` | 2,000,000 (`tx/config.rs:10`, `:124-127`); requires `executor` (`:114`) | commented `2000000` (`validator.sample.toml:98`, `sentinel.sample.toml:83`) |  | batching gas constants: 16 gas per calldata byte, batch overhead 26,000 + 16 x 68, 5,000 per call, 34,000 per call with value (`crates/core/src/tx/bundle.rs:71-74`) |
| `blocks_before_resubmit` | 2 (`tx/config.rs:33`) | not in either sample |  | each resubmission raises both fee fields to at least 110% of the previous submission (`crates/core/src/tx/fees.rs:33-56`) |
| Fee caps | `priority_fee_cap_percentage` none, so no cap (`tx/config.rs:21-24`, `:34`); a cap of 0 or below disables priority fees, 100 or above is a no-op (`fees.rs:5-31`); there is no absolute max-fee setting in `tx::Config` (`tx/config.rs:15-25`) | commented example `95` (`validator.sample.toml:104`, `sentinel.sample.toml:89`) | 95% example (`docs/validator-handbook.md:73`, `docs/sentinel-handbook.md:65`) |  |
| Sentinel `voting_window` | required, no default (`crates/sentinel/src/config.rs:44-52`; TODO at `:37-41`) | `6` (`crates/sentinel/sentinel.sample.toml:38-41`, "set it to the oracle's commit window") | `voting_window = 6` (`docs/configuration.md:119`) | _differs_: `settled.md` says 100; sample changed 100 to 6 by `fcc7028` (#1062) |
| Engine timeout | `max(1,000 ms, (voting_window - 1) x block_time x 3 / 4)` (`crates/sentinel/src/main.rs:48-65`), sent as the reqwest timeout and the `x-request-timeout` header (`crates/sentinel/src/engine.rs:164-170`) | derived |  | with Gnosis `auto` (5,000 ms) and `voting_window` 6: 18,750 ms; with 100 it would be 371,250 ms |
| Oracle windows (production) |  |  | commit 6, reveal 6, arbitration timeout 483,840 blocks (about 28 days), governance delay 51,840 (`docs/configuration.md:60-67`; `contracts/.env.aegis.sample:18-24`) | _differs_: `settled.md` lists the testnet values 2, 3 and 50,400 (`contracts/.env.testnet.sample:19-25`) |
| Observability | `log_filter` "info", `metrics_address` `127.0.0.1:0` (`crates/core/src/observability/mod.rs:24-33`) | `log_filter = "info"`, `metrics_address` commented | ephemeral port on localhost (`docs/validator-handbook.md:48`, `docs/sentinel-handbook.md:42`) | matches |
| Driver step retry | 100 ms (`crates/core/src/driver.rs:28`) |  |  |  |

### Differences from `settled.md` section 3

1. `oracle_timeout` default is 24 blocks, not 12 (`crates/validator/src/config.rs:89-91`, `fcc7028`).
2. The sentinel sample's `voting_window` is 6, not 100 (`crates/sentinel/sentinel.sample.toml:41`); the field has no code default. With 6 the derived engine timeout on Gnosis is 18.75 s.
3. Aegis production oracle windows are commit 6 and reveal 6 blocks, arbitration timeout 483,840 blocks, governance delay 51,840 blocks (`contracts/.env.aegis.sample:18-24`). `settled.md` gives the testnet values 2, 3 and 50,400, which still match `contracts/.env.testnet.sample:19-25`.
4. PROMPT.md A10 "nonce chunk 1024" no longer exists in code or docs.
5. Not in `settled.md` but needed for triggers: `max_reorg_depth` 5; Gnosis block timings 5,000 / 500 / `[500, 500, 500, 1000, 250, 250]` ms; minimum KeyGen participation 4 of 5; `blocks_before_resubmit` 2; in-flight limit 16 (Direct) or 1 (Batched); `max_batch_gas` 2,000,000.

Unchanged and matching: chain 100 with 5 s blocks, 5 genesis validators with threshold 3, `blocks_per_epoch` 1440, `key_gen_timeout` 120, `signing_timeout` 6, fee token USDC.e with fee 0.40, bond 2000x, slashing 1x, DAO share 0.

### Docs compared with code

Checked and consistent: the timing defaults in `docs/configuration.md:38-45` and `:77`; `${NAME}` and `$$` substitution limited to `rpc`, `signer` and `database` (`crates/core/src/serialization.rs:40-41`, `crates/validator/src/config.rs:12-19`, `crates/sentinel/src/config.rs:12-19`); `metrics_address` default; the metric names `safenet_validator_secrets_total{kind="keygen"|"nonces"}` and `safenet_validator_housekeeping_total{result}` (`crates/validator/src/metrics.rs:115-155`); the log text `submission failed, will retry without bumping fees` (`crates/core/src/tx/mod.rs:358`); account nonce and code read with `eth_getProof` (`crates/core/src/tx/mod.rs:378`); fee estimates from `estimate_eip1559_fees` (`crates/core/src/tx/mod.rs:395`); `WORKDIR /usr/src/app` and `ENTRYPOINT ["./validator"]` / `["./sentinel"]` (both Dockerfiles).

Differences:

- `docs/overview.md:88` gives the minimum group size as `((total_validators * 2) / 3) + 1`; the code also applies a floor of two (`crates/validator/src/consensus/group.rs:239`).
- `docs/overview.md:56` says a selection "only needs to be larger than the group threshold"; the code accepts exactly `threshold` signers (`crates/validator/src/state/sign.rs:532`, `:580`, `:626`, `:639` compare against `group_threshold()`), as `docs/overview.md:44` states ("at least a threshold").

Not checked: behavioural claims in the docs (secret retention timing, finalisation, the RPC method mix in `docs/validator-handbook.md:25-29`); those belong to the reviewers.

## 7. Integration suites

Each recipe ran under `integ` (private network namespace via `podman unshare unshare -n`, offline cargo and forge), one at a time, after `warm-chain base` (cargo build rc 0, `cargo test --no-run` rc 0, forge build rc 0). Durations are wall time of the whole recipe, including its own `cargo build` check and `forge build --force` (about 17 s).

| Recipe | Script | Result | Duration | Runs | What the script asserted | Log |
| --- | --- | --- | --: | --: | --- | --- |
| `test-integration-sentinel` | `scripts/run_sentinel_integration_test.sh` | pass | 87 s | 1 | 7 `OK` checks: agreement with bonds returned and fee split; opposing votes freeze the request; arbitration resolves and both claim; the loser is slashed only partially; an unruled arbitration is timed out and bonds come back in full | `logs/07-integ-test-integration-sentinel.log` |
| `test-integration-validator` | `scripts/run_validator_integration_test.sh` | pass | 40 s | 1 | genesis and epoch 1 each attested an oracle-backed transaction; epoch 1 generated, staged and rolled over | `logs/07-integ-test-integration-validator.log` |
| `test-integration-validator-7702` | `scripts/run_validator_7702_integration_test.sh` | pass | 33 s | 1 | every transaction batched through the executor; three transactions proposed in one block attested; validator A undelegated after restarting without the executor | `logs/07-integ-test-integration-validator-7702.log` |
| `test-integration-validator-reorg-nonce` | `scripts/run_validator_reorg_nonce_test.sh` | pass | 43 s | 1 | attestation after a reorg spanning the `KeyGenSecretShared` block, validator A re-committing its stored nonces | `logs/07-integ-test-integration-validator-reorg-nonce.log` |
| `test-integration-validator-signing-restart` | `scripts/run_validator_signing_restart_test.sh` | pass | 40 s | 1 | attestation under the restarted signing request after validator C withheld its signature share | `logs/07-integ-test-integration-validator-signing-restart.log` |
| `test-integration-validator-nonce-commit-timeout` | `scripts/run_validator_nonce_commit_timeout_test.sh` | pass | 35 s | 1 | attestation under the original signing request with A and B after C never committed nonces | `logs/07-integ-test-integration-validator-nonce-commit-timeout.log` |
| `test-integration-validator-deep-reorg` | `scripts/run_validator_deep_reorg_test.sh` | pass | 24 s | 1 | the validator stopped after a 5-block reorg with `max_reorg_depth` 2 and logged `ExceededMaxReorgDepth` | `logs/07-integ-test-integration-validator-deep-reorg.log` |

No failure occurred, so nothing was re-run. `test-integration-sentinel-engine` was skipped (engine out of scope). `pgrep -a anvil` was empty before each suite and after the last.

The deep-reorg suite checks that the process stopped and what it logged (`scripts/run_validator_deep_reorg_test.sh:81-96`), not its exit status, so it cannot catch the known exit-status-0 item (`settled.md` section 2, first row). The reorg-nonce suite gives validator B a `max_reorg_depth` of three times validator A's, so that B's replay after its restart keeps the DKG secrets it scheduled for deletion before the reorg (`scripts/run_validator_reorg_nonce_test.sh:35-44`); it does not exercise the known replay-pruning item (`settled.md` section 2, second row).

## 8. Drift and notes

- Scope drift since run 2: 50 non-merge commits; in-scope Rust grew from 20,492 to 23,319 lines (+5,260/-2,433). Main themes: 7702 executor batching (#991 to #1005, #1023), Direct Nonce (#1032 to #1044, preprocessing removed), Event Timestamps (#1047 to #1052), New Block (#1014, #1015, #1017), per-network block timings (#1010), secrets from environment variables and out of logs (#1007, #1046), KeyGen proof of possession and complaint fixes (#1019, #1025, #1027), signing restart fixes (#1008, #1009), the sentinel verdict store (#978, #979), engine timestamps (#975), #974, and the Aegis defaults (#1062).
- Added files: `crates/core/src/config.rs` (157), `crates/core/src/tx/bundle.rs` (407), `crates/core/src/tx/config.rs` (194), `crates/validator/src/frost/serialization.rs` (85), `crates/validator/src/state/secrets.rs` (86), `crates/sentinel/src/verdicts.rs` (268). Deleted: `crates/validator/src/frost/preprocess.rs` (189 at `fe9e84c`), `crates/validator/src/secrets/nonces.rs` (348), `crates/validator/src/state/preprocess.rs` (253).
- The PROMPT.md header figure (about 24,200 lines at `82b3e0d`) included the engine. The run-3 scope is 23,319 lines of Rust plus 377 lines of manifests, Dockerfiles and samples, plus `Cargo.lock`.
- Unit-test spread: the validator has 41 unit tests for 8,221 lines. `state/keygen.rs` (1,447 lines), `state/sign.rs` (836), `state/mod.rs` (487), `frost/keygen.rs` (515) and `service/*.rs` (717) have no test attribute in the file; the six validator integration suites exercise them.
- A12: in-scope `TODO` markers are `crates/sentinel/src/config.rs:37` and `crates/sentinel/src/main.rs:48` (plus a reference at `config.rs:112`); none in core or validator. `state/run3/map.md` (Map3) owns the full known-items list.
- Checkout preparation (outside the repository's tracked files): the worktree's three contract submodule directories were empty, so I exported the pinned commits (`forge-std` `8e40513`, `openzeppelin-contracts` `c64a1ed`, `safe-smart-account` `a2e19c6`) with `git archive` from the main checkout's submodule repositories (local, no network; the nested OpenZeppelin submodules are not needed by the remappings). The integration scripts start `$REPO_ROOT/target/debug/<service>` (`scripts/lib/shared_test_scripts.sh:214`), so I linked `wt/base/target` to the local run folder's `target-base` (ignored by `/target` in `.git/info/exclude`). `warm-chain` writes to the local run folder's `out/warm/`, which did not exist; I created it. The worktree's `git status --short` stayed empty throughout.
- `forge build` (in `warm-chain` and in six of the seven suites) tries to auto-install the submodules (`git submodule update --init`) and fails at once because the directories are not empty ("destination path already exists"), then compiles normally. The attempt left an empty `.git/worktrees/base/modules/contracts/lib/` directory (12 KB), which goes away with the worktree. The five re-validation worktrees (`wt/rv-*`) have empty submodule directories and no `target` link yet; any of them that runs forge or an integration script needs the same preparation.
- The integration scripts rewrite `validator_*_logs.txt`, `sentinel_*_logs.txt` and `anvil_logs.txt` in the worktree root on every suite, so only the last suite's service logs remain; the per-suite recipe output is in `logs/07-*`.

## 9. Corrections (head, 2026-10-08)

- Section 6, row `sqlx`: "system SQLite, no `bundled`" is wrong. In `sqlx 0.9.0` the `sqlite` feature that `Cargo.toml:19` enables expands to `sqlite-bundled` (`sqlx-0.9.0/Cargo.toml:181-186`), which turns on `sqlx-sqlite/bundled` and `libsqlite3-sys/bundled`, so the build compiles SQLite 3.51.3 from `libsqlite3-sys 0.37.0` into the binaries (`ldd` shows no shared SQLite, `logs/R8-sqlite-bundled.txt`). Reported by R8 (`findings/F3-XC-003.md`), checked by the head against the registry source.
