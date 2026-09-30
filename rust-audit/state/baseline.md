> **Scope note (run 2):** this is the run-1 baseline. `crates/sentinel-engine` has since been placed out of scope (PROMPT.md Section 11); its toolchain, inventory and test rows below are historical. The run-2 baseline is `state/run2/baseline.md`.

# Phase 0 baseline

| Field | Value |
| --- | --- |
| Commit | `2893917757ae518ebb91154712cf3e401cb68d33` ("AI review changes", Shebin John) |
| Branch | `rust-audit` |
| Produced | by the Recon agent |
| Mode | **read-only** (assumption A9 is FALSE — no Rust toolchain on this host) |
| Map baseline | `rust-audit/codebase-map.md`, written against `82b3e0d`; line numbers verified valid at HEAD (Section 7 below) |
| Logs | `rust-audit/state/logs/` — every number in this file comes from a command run in this session and saved there |

Everything below was measured. Where a thing was not measured, this file says "not run" or "not present". No version, count, or advisory is reproduced from memory or from the map without a measurement beside it.

---

## 1. Toolchain and environment

Probe method, per tool: `command -v <tool>` and then `<tool> --version`. Raw output: [`logs/toolchain.txt`](logs/toolchain.txt). Resources: [`logs/resources.txt`](logs/resources.txt).

| Tool | Command run | Result |
| --- | --- | --- |
| `cargo` | `command -v cargo` / `cargo --version` | **not installed** — "cargo: command not found", exit 127 |
| `rustc` | `command -v rustc` / `rustc --version` | **not installed** — exit 127 |
| `rustup` | `command -v rustup` / `rustup --version` | **not installed** — exit 127 |
| `cargo-audit` | `command -v cargo-audit` / `cargo audit --version` | **not installed** (and uninstallable — no `cargo`) |
| `forge` | `command -v forge` / `forge --version` | **not installed** — exit 127 |
| `anvil` | `command -v anvil` / `anvil --version` | **not installed** — exit 127 |
| `cast` | `command -v cast` / `cast --version` | **not installed** — exit 127 |
| `just` | `command -v just` / `just --version` | **not installed** — exit 127 |
| `sqlite3` | `command -v sqlite3` / `sqlite3 --version` | **not installed** — exit 127 (the `sqlx` `sqlite` feature links the system library at build time; no CLI to open a `.sqlite` file by hand) |
| `jq` | `command -v jq` / `jq --version` | present, `/usr/bin/jq`, **jq-1.8.1** |
| `git` | `command -v git` / `git --version` | present, `/usr/bin/git`, **git version 2.51.0** |
| `python3` | `command -v python3` / `python3 --version` | present, `/usr/bin/python3`, **Python 3.13.7** |
| `node` | `command -v node` / `node --version` | present, `/home/shebin.guest/.local/bin/node`, **v22.23.2** (not required by the audit; recorded because it is the only other scripting runtime) |

Absence confirmed at the usual install locations as well (`ls -la`, all "No such file or directory"): `~/.cargo/bin`, `~/.rustup`, `/usr/local/cargo/bin`, `/opt/cargo/bin`, `~/.foundry/bin`. `/usr/local/bin` contains only `lima-guestagent`.

**Host resources** (`free -h`, `nproc`, `lscpu`, `df -h`):

| Resource | Measured | A9 requirement | Verdict |
| --- | --- | --- | --- |
| RAM | 3.8 GiB total, 1.7 GiB available, 0 B swap | at least 8 GB | **fails** |
| Disk (`/dev/vda1`, holds the repo) | 96 G size, 83 G available, 14% used | at least 15 GB free | passes |
| CPU | 4 × aarch64, vendor Apple (Lima VM, `Linux lima-default 6.17.0-41-generic`) | not specified | n/a |
| Repo size | 13 M (`du -sh`) | n/a | n/a |

**Not run, and why.** `cargo build --workspace --all-targets --locked`, `cargo test --workspace`, `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo audit` were **not run**: there is no `cargo` binary on this host and no toolchain to install one from (installing one was explicitly out of bounds for this run — no network, no `rustup`). There is therefore **no build log, no test log, no clippy log and no advisory log** under `logs/`, and none should be cited by any later agent. Likewise the Anvil integration scripts under `scripts/` and the `just` recipes were not run: `anvil`, `forge`, `cast` and `just` are all absent.

**Dependency sources are also absent** (relevant to assumption A6): there is no `vendor/`, no `target/`, no `.cargo/` in the repo and no `*/cargo/registry` directory anywhere under a 4-level scan of `/`. No dependency source (`frost-core`, `frost-secp256k1`, `alloy`, `sqlx`, `k256`, `sha2`, `hkdf`, `reqwest`) can be read in this checkout. Every claim about upstream library behaviour stays "upstream, not read" and may only be checked against the versions in `Cargo.lock` (Section 6), never against source.

---

## 2. Mode: read-only

Assumption **A9 is FALSE**. Per A9's "If false" column, the run is downgraded to a **read-only review** and this baseline records it so the report can state it.

Consequences, to be applied by every later agent:

1. **`E1` evidence is unreachable this run.** No test, PoC, fuzz case, script or tool output can be executed, so no finding can produce an `E1` basis row. The `logs/` directory holds inspection output only (`git`, `find`, `wc`, `grep`, `python3` over text files); none of it is program behaviour.
2. **The certainty ceiling is 89%.** By the Section 8 rubric, the 90–100 band requires "`E1` reproduction and Critic Confirmed". The best a finding can reach in this run is **`E2` + Critic Confirmed = the 70–89 band**. Any finding written with a certainty of 90 or above is wrong by construction and must be pushed back to at most 89 with the reason "no toolchain, read-only run".
3. **QA agents (Phase 3) do static work only.** They may (a) _author_ a PoC test or binary and save it under `poc/<finding-id>/` unexecuted, and (b) review the proposed remediation statically for soundness. Every QA section must record the verdict **"Not attempted (no toolchain)"** with a pointer to this file, never "Not reproduced" — nothing was attempted, so nothing failed. The instruction in Section 6 to "compil[e] it in a scratch copy when cheap" cannot be followed.
4. **No temporary edits to tracked files.** The only reason PROMPT.md Section 1 permits them is a QA PoC run; with no runner, there is no reason to touch a tracked file at any point in this run.
5. **Knock-on to other assumptions.** A8's "If false → Checker findings can reach `E1`" is moot: even with the `sentinel-test-vectors` corpus on disk, `just test-integration-sentinel-engine` cannot be run. A6's library-internals question cannot be settled by reading source (Section 1, last paragraph). A5's reorg behaviour and A10's timing parameters can be checked only by code tracing.
6. **Checklist item 9 is half-answerable.** "Duplicate major versions (`cargo tree -d`)" is substituted below by a lockfile parse (Section 6); "advisories (`cargo audit`)" has **no substitute** — R10 must write "cargo audit not run" rather than assert any CVE or RUSTSEC status.

---

## 3. Rust inventory verification

Method: `find crates -name '*.rs' | sort | xargs wc -l` ([`logs/wc-rs.txt`](logs/wc-rs.txt)), then a parse of the four tables in `codebase-map.md` Section 2 compared row-by-row against those measurements ([`logs/inventory-diff.txt`](logs/inventory-diff.txt), script kept in the session scratchpad).

### Per-crate totals (measured)

| Crate | Files on disk | LOC on disk | Files in map §2 | LOC in map §2 | Match |
| --- | --- | --- | --- | --- | --- |
| `core` | 23 | 7,644 | 23 | 7,644 | exact |
| `validator` | 28 | 8,098 | 28 | 8,098 | exact |
| `sentinel` | 10 | 3,348 | 10 | 3,348 | exact |
| `sentinel-engine` | 22 | 5,113 | 22 | 5,113 | exact |
| **Workspace total** | **83** | **24,203** | **83** | **24,203** | **exact** |

`ls -1 crates/` confirms exactly four crate directories, so `find crates -name '*.rs'` and "the four in-scope crates" are the same set: no in-scope `.rs` file lives outside them and no out-of-scope `.rs` file is swept in.

### The three required lists

| List                                        | Count | Contents |
| ------------------------------------------- | ----- | -------- |
| A. In the map, line count differs from disk | **0** | (none)   |
| B. On disk, missing from the map            | **0** | (none)   |
| C. In the map, missing from disk            | **0** | (none)   |

**Total inventory mismatches: 0.** Every one of the 83 files listed in `codebase-map.md` Section 2 exists at the stated path with exactly the stated `wc -l`. Combined with the drift check in Section 7, the map's `path:line` citations are usable as written.

### Resolving "81 Rust files" (map Section 10) against 83 on disk

`find crates -name '*.rs' | wc -l` reports **83**. The resolution, exactly:

- **No files account for the difference.** The map's own Section 2 tables enumerate **83 rows** — 23 (`core`) + 28 (`validator`) + 10 (`sentinel`) + 22 (`sentinel-engine`) — and those 83 rows match the 83 files on disk one-for-one with zero LOC deltas (lists A, B, C above).
- The map's four per-crate analyses agree with 83 too: `analysis-core.md:5` "7,644 LOC", `analysis-validator.md:5` "8,098 LOC", `analysis-sentinel.md:5` "all 10 `.rs` files, 3,348 LOC", `analysis-sentinel-engine.md:5` "every `.rs` file read in full, 5,113 lines". Those four totals sum to 24,203, the measured workspace total.
- The map documents **no exclusion** anywhere — no file is marked skipped, generated, or out of scope. So "81" is a **prose miscount in Section 10 of the map**, not a coverage gap and not a pair of unread files. Section 4 of PROMPT.md ("every `.rs` file") already sets the scope correctly.
- **Action for the Coverage Critic:** the coverage denominator is **83 files / 24,203 lines**, not 81.

### Secondary numeric defects found in the map (documentation only, no coverage impact)

The `Lines` column of `codebase-map.md` Section 9 (reviewer assignments) was re-added from disk ([`logs/reviewer-line-totals.txt`](logs/reviewer-line-totals.txt)). Seven of nine entries are exact; two are wrong:

| Reviewer | Map says | Measured (Rust files in the assignment) | Delta |
| --- | --- | --- | --- |
| R2 (`core` runtime, state, effects, observability) | 1,993 | **1,998** | −5 |
| R9 (`sentinel-engine` checkers) | 3,275 | **3,471** | −196 |
| R1, R3, R4, R5, R6, R7, R8 | 4,106 / 1,540 / 3,228 / 2,802 / 2,068 / 3,348 / 1,642 | identical | 0 |

Sum of the map's R1–R9 column is 24,002 against a measured 24,203 (−201 = −5 + −196). Neither delta corresponds to any file or pair of files in those assignments (no `core` file is 5 lines; no `checkers/` file or pair sums to 196), so both are arithmetic slips, not omitted files. R1+R2+R3 measured = 7,644 = the whole of `core`; R4+R5+R6 = 8,098 = the whole of `validator`; R8+R9 = 5,113 = the whole of `sentinel-engine`; R7 = 3,348 = the whole of `sentinel`. **Every one of the 83 files is assigned to exactly one reviewer**; the assignment set is complete despite the two wrong totals. R2 and R9 should read 1,998 and 3,471 lines of work respectively.

### Map Section 3 claim spot-checked

"no `unsafe` blocks anywhere": `grep -rn --include='*.rs' -w 'unsafe' crates` returns **0 matches** ([`logs/manifests.txt`](logs/manifests.txt)). Confirmed.

---

## 4. Non-Rust in-scope files

Method: `stat -c%s` and `wc -l` per path, plus a `find crates -type f ! -name '*.rs'` sweep to catch anything the map omits. Raw output: [`logs/nonrust-inventory.txt`](logs/nonrust-inventory.txt).

| File | Exists | Bytes | Lines | Map §2 says | Match |
| --- | --- | --- | --- | --- | --- |
| `Cargo.toml` | yes | 777 | 26 | 26 | exact |
| `Cargo.lock` | yes | 148,444 | 6,169 | 6,169 | exact |
| `crates/core/Cargo.toml` | yes | 723 | 31 | 31 | exact |
| `crates/validator/Cargo.toml` | yes | 633 | 28 | 28 | exact |
| `crates/sentinel/Cargo.toml` | yes | 548 | 24 | 24 | exact |
| `crates/sentinel-engine/Cargo.toml` | yes | 532 | 24 | 24 | exact |
| `crates/validator/Dockerfile` | yes | 1,376 | 37 | 37 | exact |
| `crates/sentinel/Dockerfile` | yes | 1,393 | 38 | 38 | exact |
| `crates/sentinel-engine/Dockerfile` | yes | 964 | 28 | 28 | exact |
| `crates/validator/validator.sample.toml` | yes | 3,545 | 77 | 77 | exact |
| `crates/sentinel/sentinel.sample.toml` | yes | 2,595 | 61 | 61 | exact |
| `crates/sentinel-engine/sentinel-engine.sample.toml` | yes | 1,852 | 42 | 42 | exact |
| `crates/sentinel-engine/openapi.yaml` | yes | 6,726 | 190 | 190 | exact |

All 13 non-Rust files the map lists exist with the stated line counts. **0 mismatches.**

Deltas between the map and the filesystem:

- **`crates/core/Dockerfile` does not exist.** The map does not claim it does (`core` is a library crate, not a service); PROMPT.md Section 4's glob `crates/*/Dockerfile` therefore resolves to three files, not four. Recorded so no agent reports a "missing Dockerfile" for `core`.
- **Three files exist under `crates/` that the map's non-Rust table omits**, each 42 bytes / 4 lines: `crates/validator/Dockerfile.dockerignore`, `crates/sentinel/Dockerfile.dockerignore`, `crates/sentinel-engine/Dockerfile.dockerignore`. They are build-context filters sitting beside the in-scope Dockerfiles. They are not named by PROMPT.md Section 4's scope list, so they are not findings-eligible on their own; R10 should read them alongside each Dockerfile, since what they do and do not exclude from the image build context is part of the Dockerfile's behaviour.
- No `build.rs` anywhere under `crates/`. No other `.yaml`/`.yml` under `crates/*/` besides `openapi.yaml`. The complete non-`.rs` file set under `crates/` is 14 files (the 11 crate-local in-scope ones plus the 3 `.dockerignore` files); `Cargo.toml` and `Cargo.lock` at the root make 16.

---

## 5. Test census

Method: `grep -cE '^[[:space:]]*#\[(tokio::)?test\b'` per file — this matches `#[test]`, `#[tokio::test]` and `#[tokio::test(...)]` at the start of a line. A control grep confirms **0** occurrences of either attribute anywhere except at the start of a line, so the count cannot miss an inline form. Raw output: [`logs/tests.txt`](logs/tests.txt).

| Crate | `#[test]` | `#[tokio::test*]` | Total measured | Map / task claim | Verdict |
| --- | --- | --- | --- | --- | --- |
| `core` | 19 | 78 | **97** | 97 | match |
| `validator` | 26 | 9 | **35** | 35 | match |
| `sentinel` | 27 | 10 | **37** | 37 | match |
| `sentinel-engine` | 53 | 44 | **97** | 97 | match |
| **Total** | **125** | **141** | **266** | — | — |

The map's per-file `Tests` column was also checked row by row: **all 83 per-file counts match** (0 mismatches), and each crate's column sums to the crate total above.

Supporting facts measured in the same pass:

- `#[cfg(test)]` module count: `core` 20, `validator` 17, `sentinel` 6, `sentinel-engine` 13.
- `#[should_panic]`: 1 occurrence workspace-wide. `#[ignore]`: 0.
- No `rstest`, `test_case`, `proptest`, `quickcheck` or `#[bench]` attributes anywhere in `crates/` (the `proptest` crate does appear in `Cargo.lock` at 1.11.0, but only transitively — its dependents are `alloy-primitives 1.6.0`, `const-hex 1.19.1`, `nybbles 0.4.8` and `ruint 1.18.0`, never a workspace crate; verified in `logs/lockfile-revdeps.txt`).
- **No `crates/*/tests/` directory exists.** Every test in the workspace is a unit test inside a `#[cfg(test)]` module in the file it tests; there are no integration-test targets in-tree. The integration coverage referenced by the map's Section 3 lives entirely in `scripts/` and the external `sentinel-test-vectors` corpus, neither of which can run here (Section 2).
- Zero-test files worth flagging for the Coverage Critic (measured, not inferred): the whole of `validator/src/state/` (`keygen.rs` 1,459, `sign.rs` 868, `mod.rs` 515, `preprocess.rs` 248, `transactions.rs` 101) and `validator/src/service/` (`action.rs` 381, `effect.rs` 275, `mod.rs` 129) carry **0** test attributes, as do `validator/src/frost/{keygen,marshal,error}.rs` (516 + 176 + 46), `core/src/driver.rs` (318), `core/src/provider/mod.rs` (166), `core/src/utils.rs` (97), `sentinel-engine/src/checkers/staking.rs` (183), `sentinel-engine/src/contracts/multi_send.rs` (186) and both `sentinel-engine/src/api/*.rs`.

---

## 6. Dependency facts obtainable without cargo

`cargo tree`, `cargo tree -d` and `cargo audit` were **not run** (Section 1). Everything below is a text parse of `Cargo.toml` and `Cargo.lock` in this checkout.

- Manifests dumped verbatim: [`logs/manifests.txt`](logs/manifests.txt).
- Lockfile parse and duplicate analysis: [`logs/lockfile-dupes.txt`](logs/lockfile-dupes.txt).
- Reverse dependencies for the duplicated crypto crates: [`logs/lockfile-revdeps.txt`](logs/lockfile-revdeps.txt).

**Lockfile shape.** `Cargo.lock` is format `version = 4`, 6,169 lines, **573 `[[package]]` entries over 516 distinct package names**. The four workspace members appear without a `source` key: `safenet-core 0.2.0`, `validator 0.2.0`, `sentinel 0.2.0`, `sentinel-engine 0.2.0`. Workspace is `resolver = "3"`, `members = ["crates/*"]`, every crate `edition = "2024"`, `publish = false`.

### Pinned versions of the requested packages

Requirement is the `Cargo.toml` range; version is the exact `Cargo.lock` pin. All sources are `registry+https://github.com/rust-lang/crates.io-index`.

| Package | Declared in | Requirement (`Cargo.toml`) | Locked version |
| --- | --- | --- | --- |
| `frost-core` | `crates/validator` | `"3"`, features `["internals"]` | **3.0.0** |
| `frost-secp256k1` | `crates/validator` | `"3"` | **3.0.0** |
| `k256` | workspace | `"0.13"`, features `["hash2curve", "serde"]` | **0.13.4** |
| `sha2` | `crates/core` | `"0.11"` | **0.11.0** — _and_ **0.10.9** also in the lock (see duplicates) |
| `hkdf` | `crates/core` | `"0.13"` | **0.13.0** |
| `alloy` | workspace | `"2"`, features `["full", "json-rpc"]` | **2.0.5** |
| `sqlx` | workspace | `"0.9"`, features `["sqlite", "runtime-tokio"]` (no `bundled`) | **0.9.0** |
| `axum` | workspace (used by `sentinel-engine`) | `"0.8"` | **0.8.9** |
| `reqwest` | workspace | `"0.13"`, `default-features = false`, features `["json", "rustls"]` | **0.13.4** |
| `rand` | workspace (used by `validator`) | `"0.8"` | **0.8.6** — _and_ 0.9.4, 0.10.1 in the lock |
| `rand_chacha` | workspace (used by `validator`) | `"0.3"` | **0.3.1** — _and_ 0.9.0 in the lock |
| `tokio` | workspace | `"1"`, features `["full"]` (+ `test-util` in dev) | **1.52.3** |
| `tracing-subscriber` | `crates/core` | `"0.3"`, features `["env-filter", "fmt", "json"]` | **0.3.23** |
| `toml` | workspace | `"1"` | **1.1.2+spec-1.1.0** |
| `argh` | workspace | `"0.1"` | **0.1.19** |

Other pins named by map Section 3, measured in the same pass: `metrics` (workspace `"0.24"`), `metrics-exporter-prometheus` `0.18.3`, `tokio-metrics` (`"0.5"`, feature `metrics-rs-integration`), `rayon` (`"1"`, `validator`), `tower-http` (workspace `"0.7"`, features `["trace"]`) → locked **0.7.0**, `async-trait` `0.1.89`, `regex` `"1"` (`core`), `thiserror` `"2"`, `serde` `"1"`, `url` `"2"`. Map Section 3 does not mention `metrics-exporter-prometheus` or `regex`; both are real `core` dependencies and are in scope for R10.

### Duplicate versions — substitute for `cargo tree -d`

Method (shown in [`logs/lockfile-dupes.txt`](logs/lockfile-dupes.txt), reproducible with `python3`): split `Cargo.lock` on `[[package]]`, take each block's `name` and `version`, group by name, and compare version keys. Two groupings are reported because they answer different questions:

- **Strict "different major number"** (first component differs): **6 packages** — `indexmap` (1.9.3, 2.14.0), `r-efi` (5.3.0, 6.0.0), `schemars` (0.9.0, 1.2.1), `semver` (0.11.0, 1.0.28), `syn` (1.0.109, 2.0.117), `webpki-roots` (0.26.11, 1.0.7).
- **Cargo's own compatibility rule** (a `0.x` release line is its own major, which is what `cargo tree -d` actually reports): **43 packages**. Because no package in this lock appears twice within one compatibility line, 43 is also the count of package names appearing more than once at all.

The 43, grouped by why an auditor would care:

| Group | Packages and locked versions |
| --- | --- |
| **RNG stack** (matters for A6, nonce and share generation) | `rand` 0.8.6 / 0.9.4 / 0.10.1; `rand_core` 0.6.4 / 0.9.5 / 0.10.1; `rand_chacha` 0.3.1 / 0.9.0; `getrandom` 0.2.17 / 0.3.4 / 0.4.2 |
| **Hashing / digest stack** (matters for hashing-parity claims) | `sha2` 0.10.9 / 0.11.0; `sha1` 0.10.6 / 0.11.0; `digest` 0.9.0 / 0.10.7 / 0.11.3; `hmac` 0.12.1 / 0.13.0; `block-buffer` 0.10.4 / 0.12.0; `crypto-common` 0.1.6 / 0.2.2; `const-oid` 0.9.6 / 0.10.2 |
| **Curve libraries** | `secp256k1` 0.30.0 / 0.31.1; `secp256k1-sys` 0.10.1 / 0.11.0 |
| **Arkworks (transitive, from `alloy`)** | `ark-ff`, `ark-ff-asm`, `ark-ff-macros`, `ark-serialize` each 0.3.0 / 0.4.2 / 0.5.0; `ark-std` 0.3.0 / 0.4.0 / 0.5.0 |
| **HTTP / TLS** | `tower-http` 0.6.11 / 0.7.0; `webpki-roots` 0.26.11 / 1.0.7 |
| **Build and container plumbing** | `syn` 1.0.109 / 2.0.117; `indexmap` 1.9.3 / 2.14.0; `hashbrown` 0.12.3 / 0.14.5 / 0.15.5 / 0.16.1 / 0.17.1; `itertools` 0.10.5 / 0.13.0 / 0.14.0; `semver` 0.11.0 / 1.0.28; `rustc_version` 0.3.3 / 0.4.1; `schemars` 0.9.0 / 1.2.1; `foldhash` 0.1.5 / 0.2.0; `fastrlp` 0.3.1 / 0.4.0; `embedded-io` 0.4.0 / 0.6.1; `cpufeatures` 0.2.17 / 0.3.0; `r-efi` 5.3.0 / 6.0.0; `wit-bindgen` 0.51.0 / 0.57.1; `windows-sys` 0.52.0 / 0.60.2 / 0.61.2, `windows-targets` 0.52.6 / 0.53.5 and the eight `windows_*` target shims 0.52.6 / 0.53.1 |

Reverse-dependency facts for the security-relevant duplicates, derived from each block's `dependencies` list (a dependency entry carries an explicit version only when the name is ambiguous, which is exactly the duplicated case):

- **`rand`**: `validator 0.2.0` pins **0.8.6** directly (with `rand_chacha 0.3.1`); `alloy-consensus`, `alloy-signer-local`, `ruint`, `secp256k1 0.30.0` and the `ark-std` family also use 0.8.6. `rand 0.9.4` enters via `alloy-primitives 1.6.0`, `metrics-util`, `proptest`, `quinn-proto`, `secp256k1 0.31.1`, `tungstenite`. `rand 0.10.1` enters solely via `sqlx-postgres 0.9.0`.
- **`rand_core`**: `frost-core 3.0.0`, `frost-rerandomized 3.0.0`, `frost-secp256k1 3.0.0`, `elliptic-curve 0.13.8`, `crypto-bigint`, `ff`, `group`, `signature` and `rand 0.8.6` all use **0.6.4** — i.e. the `validator`'s own RNG (`rand 0.8` / `rand_chacha 0.3`) is on the same `rand_core` line as the FROST libraries it feeds. `rand_core 0.9.5` and `0.10.1` are confined to the `rand 0.9`/`0.10` sub-trees.
- **`sha2`**: `safenet-core 0.2.0` uses **0.11.0** (over `digest 0.11.3`), while `k256 0.13.4` and `frost-secp256k1 3.0.0` use **0.10.9** (over `digest 0.10.7`). Two independent SHA-256 implementations are linked into every binary. Recorded as a fact, not a finding: reviewers checking hashing parity against Solidity should note which of the two a given call site uses.
- **`hmac`**: `hkdf 0.13.0` (used by `core/src/kdf.rs`) uses **0.13.0**/`digest 0.11`; `rfc6979 0.4.0` (deterministic ECDSA, under `k256`) uses **0.12.1**/`digest 0.10`.
- **`tower-http`**: `sentinel-engine 0.2.0` pins **0.7.0** directly; `reqwest 0.13.4` pulls **0.6.11**.
- **`secp256k1`**: 0.30.0 via `alloy-consensus 2.0.5`, 0.31.1 via `alloy-primitives 1.6.0` — both transitive, neither used directly by a workspace crate.

**Advisories: `cargo audit` not run** (no toolchain). No RUSTSEC or CVE status is asserted anywhere in this baseline, for any of the 516 packages. Any dependency-advisory finding in this run is unverifiable and must say so.

---

## 7. Drift note

Raw output: [`logs/git-drift.txt`](logs/git-drift.txt).

- `git rev-parse HEAD` → `2893917757ae518ebb91154712cf3e401cb68d33`; branch `rust-audit`; `git log -1` → "AI review changes", Shebin John.
- `codebase-map.md` and the four analyses were written against `82b3e0d`. `git diff --name-only 82b3e0d..HEAD` lists **11 files, all under `rust-audit/`** (`PROMPT.md`, `README.md`, the four `analysis/*.md`, `codebase-map.md`, four `.gitkeep` files; 2,447 insertions, 0 deletions). Filtering that list with `grep -v '^rust-audit/'` returns nothing.
- `git diff --stat 82b3e0d..HEAD -- crates Cargo.toml Cargo.lock` is **empty**: no drift in any audited source file, manifest or lockfile. Together with the zero inventory mismatches in Section 3, **every `path:line` citation in `codebase-map.md` and `analysis/` is valid at HEAD**; agents may cite them without re-deriving line numbers, subject to the map's own caveat that the citations themselves were only spot-verified by its author.
- `git status --short` at the time of writing lists four entries, all untracked and all inside `rust-audit/state/`: `STATE.md` and `reviewer-brief.md` (the Manager's), `baseline.md` and `logs/` (this agent's). **Nothing outside `rust-audit/` is modified, staged, or untracked.** No `target/` directory exists (nothing has been built), so the usual `target/` exemption is not needed.
- This agent created only `rust-audit/state/baseline.md` and `rust-audit/state/logs/*`. It ran no `git` command that writes: no commit, branch, stash, push, checkout, or config change.
- Assumption **A14** ("the tree does not change during the run") is baselined here: any later `git status --short` showing a change outside `rust-audit/`, or a `git rev-parse HEAD` other than `2893917`, means Phase 0 must be restarted.

---

## 8. Log index

| Log | Contains |
| --- | --- |
| [`logs/toolchain.txt`](logs/toolchain.txt) | `command -v` and `--version` for all 13 tools, install-location checks, PATH, `unsafe`-free check tail, dependency-source availability |
| [`logs/resources.txt`](logs/resources.txt) | `free -h`, `nproc`, `lscpu`, `df -h`, `du -sh` |
| [`logs/git-drift.txt`](logs/git-drift.txt) | commit, branch, `git status --short`, `git diff` against `82b3e0d` |
| [`logs/wc-rs.txt`](logs/wc-rs.txt) | full sorted `.rs` file list and per-file `wc -l` (83 files, 24,203 lines) |
| [`logs/inventory-diff.txt`](logs/inventory-diff.txt) | map §2 vs disk: per-crate totals and the three mismatch lists (all empty) |
| [`logs/reviewer-line-totals.txt`](logs/reviewer-line-totals.txt) | map §9 `Lines` column re-added from disk (R2 and R9 wrong) |
| [`logs/nonrust-inventory.txt`](logs/nonrust-inventory.txt) | existence, bytes and lines for every non-Rust in-scope file, plus the full non-`.rs` sweep under `crates/` |
| [`logs/tests.txt`](logs/tests.txt) | test-attribute census per crate and per file, `#[cfg(test)]` counts, per-file comparison with map §2 |
| [`logs/manifests.txt`](logs/manifests.txt) | all five `Cargo.toml` files verbatim, `unsafe` grep, lockfile format marker |
| [`logs/lockfile-dupes.txt`](logs/lockfile-dupes.txt) | locked versions of the 15 requested packages; duplicate analysis under both major-version definitions |
| [`logs/lockfile-revdeps.txt`](logs/lockfile-revdeps.txt) | reverse dependencies for each duplicated crypto crate; direct dependency list of each workspace member |

Absent by design (do not cite): `logs/build.txt`, `logs/test.txt`, `logs/clippy.txt`, `logs/audit.txt` — the four `cargo` invocations named in PROMPT.md Section 6 were **not run** (Section 1).

---

# ADDENDUM — Phase 5 baseline (toolchain installed)

The operator installed a Rust toolchain after phases 0–4 completed. Everything below was **executed**; contrast with the original sections above, where every corresponding row reads "not run".

**Toolchain is NOT on the default PATH.** It lives at `~/.cargo/bin`; every command must `export PATH="$HOME/.cargo/bin:$PATH"` first.

| Tool | Version | Note |
| --- | --- | --- |
| `cargo` | 1.98.1 (797e8a9bc 2026-08-05) |  |
| `rustc` | 1.98.1 (48a229cea 2026-09-01) |  |
| toolchain | `stable-aarch64-unknown-linux-gnu` | **aarch64**, not x86_64 |
| `just` | 1.40.0 | newly present |
| `jq` | 1.8.1 | unchanged |
| `forge`/`anvil`/`cast` | **absent** | no `~/.foundry`; Anvil integration scripts remain unrunnable |
| `cargo-llvm-cov` | **absent** | coverage not reproducible locally |
| RAM / disk | 11 GB / 94 GB free | was 3 GB / 83 GB; the README's 8 GB minimum is now met |

## Executed results

| Command | Exit | Log | Result |
| --- | --- | --- | --- |
| `cargo build --workspace --all-targets --locked` | **0** | `logs/cargo-build.txt` | Builds clean. One warning, and it is **not** Safenet's code: `proc-macro-error2 v2.0.1` contains code a future rustc will reject (a transitive dependency). |
| `cargo test --workspace` | **0** | `logs/cargo-test.txt` | **266 passed, 0 failed, 0 ignored.** |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | **0** | `logs/cargo-clippy.txt` | **Clean.** The CI lint gate passes. |
| `cargo tree -d --workspace` | 0 | `logs/cargo-tree-dupes.txt` | 76 duplicate entries; `alloy-json-abi`/`alloy-core`/`alloy-dyn-abi`/`alloy-sol-types` at **v1.6.0** beneath `alloy` **v2.0.5**. |

## Test census — the map's claims are now VERIFIED, not asserted

Per-crate counts match `codebase-map.md` Section 2 **exactly**:

| Crate             | Claimed | Executed | Test binary             |
| ----------------- | ------- | -------- | ----------------------- |
| `safenet-core`    | 97      | **97**   | `unittests src/lib.rs`  |
| `sentinel`        | 37      | **37**   | `unittests src/main.rs` |
| `sentinel-engine` | 97      | **97**   | `unittests src/main.rs` |
| `validator`       | 35      | **35**   | `unittests src/main.rs` |

The `src/main.rs` test targets independently **confirm QA's packaging finding**: `sentinel`, `sentinel-engine` and `validator` are binary-only crates. Only `safenet-core` has a `lib.rs`.

## F-XC-001 confirmed by execution

`grep` over the workspace `Cargo.toml` finds **no `[profile.*]` section of any kind**, so the release profile is stock: `overflow-checks = false` and `debug_assert!` compiled out in every shipped binary. The finding stands as written.

## What is still blocked

- **A8 remains FALSE** — the `sentinel-test-vectors` corpus is still unavailable, so engine checker findings cannot be validated against their intended oracle.
- **Foundry is still absent** — no `anvil`, so the integration scripts and any finding needing a live chain simulation stay unexecuted.

---

# ADDENDUM 2 — Foundry installed

| Tool | Version | Note |
| --- | --- | --- |
| `forge` / `anvil` / `cast` / `chisel` | **1.8.1** | at `~/.foundry/bin`, **also not on the default PATH** |
| `foundryup`, `solar` | present |  |

**Deviation from assumption A9**, which specified **Foundry 1.5.1**. The installed toolchain is **1.8.1**. Recorded rather than ignored: integration results below were produced on a newer Foundry than the audit assumed, so a behavioural difference between 1.5.1 and 1.8.1 is a possible (if unlikely) confounder for any anvil-dependent result.

Full invocation prefix for anything in this phase: `export PATH="$HOME/.foundry/bin:$HOME/.cargo/bin:$PATH"`

## Integration suites — availability

| Suite | Runnable | Note |
| --- | --- | --- |
| `just test-integration-validator-deep-reorg` | yes | single validator + anvil |
| `just test-integration-validator-reorg-nonce` | yes | anvil + two validator instances |
| `just test-integration-validator` | yes | anvil + two validator instances |
| `just test-integration-sentinel` | yes |  |
| `just test-integration-sentinel-engine <corpus>` | **NO** | requires the external `sentinel-test-vectors` checkout — **A8 remains FALSE**, so the engine checkers' intended oracle is _still_ unavailable even with a full toolchain |

## Result — deep-reorg regression: PASSES

`scripts/run_validator_deep_reorg_test.sh`, exit **0**, log `logs/it-validator-deep-reorg.txt`. Contracts deployed to anvil (chain 31337) via `forge script Deploy.s.sol`; coordinator `0xE6DCB448...471E9`, consensus `0x9aDb2f0B...C6fA2`, oracle `0x9fE46736...fa6e0`. The harness reorged **5 blocks, deeper than the configured `max_reorg_depth` of 2**, and reported:

> `SUCCESS: the validator failed loudly after a reorg exceeding the configured max_reorg_depth.`

**Scope of what this proves, and what it does not.** The harness reorgs while the validator is **running**, so it exercises the live `ExceededMaxReorgDepth` exit path — and that path works. `F-CORE-001` attacks a different case: a reorg that happens while the validator is **down**, where snapshots keyed by block number alone cannot detect that the chain changed underneath them. The two are not in conflict, and the passing test arguably _sharpens_ `F-CORE-001`: the deliberate exit demonstrably works, which is precisely why the finding's claim — that the exit-then-restart cycle is self-defeating because the retained anchor sits at exactly the fatal depth — matters.

**This reconciliation must be verified by an agent against the finding text, not accepted from the Manager's reading.** See the Phase 7 verification note in `STATE.md`.

## Integration suite results (Foundry 1.8.1)

| Suite | Exit | Verdict |
| --- | --- | --- |
| `run_validator_deep_reorg_test.sh` | **0** | **PASSES** — validator fails loudly on a 5-block reorg vs `max_reorg_depth` 2 |
| `run_validator_reorg_nonce_test.sh` | **0** | **PASSES** — see the tension note below |
| `run_validator_integration_test.sh` | **0** | **PASSES** — genesis and epoch 1 each attested an oracle-backed transaction; epoch 1 generated, staged and rolled over |
| `run_sentinel_integration_test.sh` | 1 | **CANNOT RUN on Foundry 1.8.1** — harness incompatibilities, not a code defect |
| `run_sentinel_engine_integration_test.sh` | n/a | **Still blocked** — needs the external `sentinel-test-vectors` corpus (A8 FALSE) |

### A false failure the Manager caused, and corrected

`run_validator_integration_test.sh` and `run_sentinel_integration_test.sh` first failed with `Error: failed to get latest block; latest block number: 1`. **That was not a defect.** A stray `python3 .../scratchpad/fakerpc.py` (pid 97442), left running by a Phase 5 verification agent, was squatting on **127.0.0.1:8545**, so the harnesses talked to the mock instead of anvil. After killing it, `run_validator_integration_test.sh` **passed**. Recorded because a report that listed it as a failure would have been wrong, and the cause was our own litter.

### `run_sentinel_integration_test.sh` — harness vs Foundry 1.8.1 (OUT OF SCOPE, observation only)

`scripts/` is **reference-only** under PROMPT.md Section 4, so this is an observation, not a finding. The suite hits at least three incompatibilities with Foundry **1.8.1** (A9 assumed **1.5.1**):

1. **`cast wallet new --json` output shape.** Line 110-116 parse `jq -r '.[0].address'`, but 1.8.1 emits an envelope: `{"schema_version":1,"success":true,"data":[{...}],"errors":[],"warnings":[]}`. The wallets are under `.data[N]`. Verified directly. Patching a scratchpad copy to `.data[N]` got past it.
2. **Bare contract-name resolution.** `forge script --root contracts DeployERC20Script` then fails with `No contract found with the name DeployERC20Script`, although `contracts/script/DeployERC20.s.sol:9` does define `contract DeployERC20Script is Script`. The file name and contract name differ, and 1.8.1 appears to have tightened resolution.
3. Rewriting that to the explicit `path:Name` form produced `No such file or directory`, i.e. a further `--root`-relative path interaction.

Chasing further harness repairs was stopped deliberately: it is out of scope, and **none of it is evidence about the sentinel's own code**. The suite's result for the sentinel is simply **unknown**. A `foundry.toml` warning also recurs on every invocation: `Found unknown 'optimizer' config key in section 'compilation_restrictions'`.

### Tension to reconcile — `run_validator_reorg_nonce_test.sh` PASSES

Its success message:

> `SUCCESS: the genesis group (0xf2b57b06...) attested a transaction after validator A's restart and reorg spanning the KeyGenSecretShared block, proving its nonce tree 805579abd97a1c1b93af46b58d17cbbaf3c2c5d60f2c14e54f69709950643fe3 was retained.`

That is a restart **plus** a reorg spanning a DKG block, with secrets **retained** — which sits close to `F-VAL-005` (High, 91%, `E1`: "a reorg across the key-generation block deletes the DKG secrets the store promises never to overwrite"), and near `F-VAL-030`/`F-VAL-033`. Either the test and the finding address different blocks (`KeyGenSecretShared` vs the confirmation block the finding cites), or one of them is wrong.

**The Manager must not adjudicate this.** Assigned to a Phase 7 agent to reconcile against the finding text and the harness source. If the regression test genuinely covers `F-VAL-005`'s path, that finding must be reduced or refuted.

## V-INT (Phase 7) — resolution of the reorg-nonce tension, and two new executed confirmations

The tension flagged above is **resolved against the harness, not against the findings**. Three corrections to the harness's own documentation, each verified from source and from its logs:

1. **`run_validator_reorg_nonce_test.sh` never restarts validator A.** The header comment (lines 8, 19) and the SUCCESS message say it does; the script starts it once (line 90) and contains no `kill` of it. Validator A's log has exactly one `starting validator service` line. **No suite in `scripts/` restarts a validator** — the happy-path suite does not either. Every finding whose trigger involves a restart (`F-CORE-001`, `F-CORE-067`, `F-VAL-033`, half of `F-VAL-030`) is untested by construction, and the report must not cite that SUCCESS message as restart evidence.
2. **The uncle is the `KeyGenSecretShared` block, not the `KeyGen` block.** `REORG_DEPTH = CURRENT_BLOCK - SECRET_SHARED_BLOCK + 1`, so the restored snapshot leaves the _genesis_ rollover in `CollectingShares { Participating }` — a retained arm of `handle_group_reconciliation`. That is why the genesis assertion passes, and it is an adjacent path to `F-VAL-005`, not its path.
3. **`anvil_reorg` drops the reorged transactions permanently** (verified directly on Foundry 1.8.1: the sender's nonce reverts and the transaction is never re-mined), so these harnesses cannot replay re-included logs at all.

**The passing suite exhibits `F-VAL-005` while reporting SUCCESS.** Its assertions cover the genesis group; its reorg (uncle = block 9) sits below the _epoch-1_ group's `KeyGen` block (10), which is exactly the finding's trigger. Both validators logged `failed to advance key generation, skipping to next epoch :: "unexpected FROST error: The participant's commitment is incorrect."`, and validator A's epoch-1 commitment differs before (`0343738943…`) and after (`03308eece3…`) the reorg — proving the `keygen_secrets` row was deleted and resampled. Epoch 1 was lost network-wide. The re-inclusion of the stale commitment came from the validator's **own** transaction queue (`resubmitting stale transaction`), so it does not depend on chain behaviour at all. `F-VAL-005`: 91% → **99%**.

The same run also executed `F-VAL-061`/`F-VAL-030` unforced: `failed to perform effect NonceTree … "nonce generator is unavailable"`, swallowed to `Resume::Noop` with no retry and no chunk beyond `chunk 0` ever linked. 93% → **98%** and 92% → **97%**.

**`F-CORE-001` executed by a V-INT scratchpad probe** (`logs/vint-downtime-reorg-probe.sh.txt`, a copy of the deep-reorg harness patched only to reorg during downtime; nothing was written into `scripts/`). Same parameters as the repo suite — `max_reorg_depth = 2`, `anvil_reorg 5`:

|  | reorg while running (repo suite) | reorg while down (probe) |
| --- | --- | --- |
| `ExceededMaxReorgDepth` | 1, fatal | **0** |
| process | exits | **keeps running** |
| `WARN`/`ERROR` after | the fatal error | **none** |

Run 2 re-anchored with `initializing block watcher :: {"latest":12,"safe":10,"resume":"Some(BlockStatus { latest: 9, safe: 7 })"}` onto state derived from blocks 5-9 that the reorg had replaced, silently. 96% → **99%**.

Evidence: `logs/it-validator_reorg_nonce-vint-*.txt`, `logs/vint-downtime-reorg-*.txt`.
