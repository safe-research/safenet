# Run 2 baseline delta (Recon-Δ, post-merge re-baseline)

| Field | Value |
| --- | --- |
| New audited commit | `fe9e84cc59b65367b31d5a3121774383cc422234` — merge of `origin/main` (`8b6a75d`) into `audit/rust-services` |
| Previous audited commit | `3ec8bc57dc35d1e9e65075ae9424bff427c47833` (`baseline.md`) |
| Branch | `audit/rust-services` |
| Mode | **full** — all five baseline commands re-executed at `fe9e84c` |
| Logs | `rust-audit/state/run2/logs/`: `10-build.log`, `11-test.log`, `12-clippy.log`, `13-audit.log`, `14-tree-d.log`, plus `15-future-incompat.log` (follow-up to the build warning, mirrors `08-future-incompat.log`) |
| Toolchain | Unchanged from `baseline.md` Section 1: `cargo 1.98.1 (797e8a9bc)`, `rustc 1.98.1 (48a229cea)`, `clippy 0.1.98 (48a229ceae)` (`logs/12-clippy.log` line 4), `cargo-audit-audit 0.22.2` |
| Independence | Written without opening `rust-audit/findings/F-*.md`, `rust-audit/report/`, or `rust-audit/state/*.md` outside `state/run2/`. Run-2 finding files (`F2-*`) were opened only by `grep` for path citations (Section 3). |

Every command below was run from the repository root with:

```sh
export PATH="$HOME/.foundry/bin:$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR=/home/shebin.guest/.cache/safenet-run2/target CARGO_BUILD_JOBS=2
```

The target directory is the one built by run-2 Phase 0 and shared with other run-2 agents, so the build was **incremental**: only the four workspace crates recompiled (`logs/10-build.log`: `Compiling safenet-core`, `validator`, `sentinel-engine`, `sentinel`; 0 `Downloaded` lines). Dependency artefacts from `3ec8bc5` were reused, which is valid because `Cargo.lock` has the same 573 packages at the same versions (Section 1.3).

**Working-tree caveat.** While this delta was being written, a QA agent appended a temporary proof-of-concept module (`mod qa2_sen`, 897 inserted lines from line 2345) to `crates/sentinel/src/service.rs`; `git status` shows that file as ` M`. Its mtime is later than the end of all five commands here, and the line and test counts below were re-derived from the committed blob (`git show fe9e84c:…`) and equal the working-tree values taken before the edit. **Every number in this document reflects `fe9e84c` as committed, not the PoC edit.** Anyone re-running `cargo test -p sentinel` before the QA agent reverts will see additional `qa2_sen::*` tests.

## 1. Executed baseline at `fe9e84c` versus `3ec8bc5`

| # | Command | Exit `fe9e84c` | Exit `3ec8bc5` | Wall time | Log | Result at `fe9e84c` |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `cargo build --workspace --all-targets --locked` | **0** | 0 | `Finished` in 15.54 s (incremental) | `logs/10-build.log` | 4 `Compiling` lines, 0 `Downloaded`, 0 errors, 1 warning: the same future-incompat lint for `proc-macro-error2 v2.0.1` |
| 2 | `cargo test --workspace` | **0** | 0 | `Finished` test profile in 0.16 s; test phase < 1 s (binaries built by #1) | `logs/11-test.log` | **299 passed**, 0 failed, 0 ignored across 4 unit-test binaries + 1 empty doc-test run (was 280) |
| 3 | `cargo clippy --workspace --all-targets --locked -- -D warnings` | **0** | 0 | `Finished` in 2.14 s (4 `Checking` lines, 0 `Compiling`) | `logs/12-clippy.log` | **0 clippy diagnostics**, same future-incompat warning |
| 4 | `cargo audit` | **1** (expected) | 1 | 9 s | `logs/13-audit.log` | 573 crate dependencies scanned: **5 vulnerabilities, 11 warnings** — the identical set (Section 1.3) |
| 5 | `cargo tree -d --workspace` | **0** | 0 | — | `logs/14-tree-d.log` | the identical 14 multi-version packages (Section 1.4) |
| + | `cargo report future-incompatibilities --id 1` | 0 | 0 | — | `logs/15-future-incompat.log` | only `proc-macro-error2@2.0.1`, unchanged |

Commands #1–#3 were launched with `setsid nohup bash -c '…'` writing `# end:`/`# exit=` trailer lines into the log and were waited on in the foreground; #4, #5 and the follow-up ran directly in the foreground. No cargo lock contention was observed.

### 1.1 Tests per crate (`logs/11-test.log` versus `logs/02-test.log`)

| Test binary | Passed `fe9e84c` | Passed `3ec8bc5` | Δ | Failed | Ignored | Time |
| --- | --- | --- | --- | --- | --- | --- |
| `safenet_core` (lib unit tests) | 99 | 99 | 0 | 0 | 0 | 0.05 s |
| `sentinel` (bin unit tests) | **44** | 42 | **+2** | 0 | 0 | 0.19 s |
| `validator` (bin unit tests) | 42 | 42 | 0 | 0 | 0 | 0.33 s |
| `sentinel_engine` (bin unit tests, **out of scope**) | 114 | 97 | +17 | 0 | 0 | 0.00 s |
| Doc-tests `safenet_core` | 0 | 0 | 0 | 0 | 0 | — |

In-scope total: **185** executed tests (was 183), equal to the `#[test]`/`#[tokio::test]` attribute count in the committed tree (99 + 44 + 42). The two new sentinel tests are `flow_reveals_on_the_commit_deadline_block` (`crates/sentinel/src/service.rs:2164`) and `flow_defers_dropping_until_the_commit_deadline_block_is_indexed` (`crates/sentinel/src/service.rs:2242`); see Section 2.2. Workspace total 299 (was 280); the other +17 are engine tests and are not examined.

### 1.2 Clippy

Zero diagnostics at both commits. The new `#[allow(clippy::too_many_arguments)]` on `crates/sentinel/src/bindings.rs:3` (Section 2.2) is the only new lint-related token in scope; clippy with `-D warnings` passes with it.

### 1.3 `cargo audit` and the `Cargo.lock` delta

**Advisory output is byte-identical** between `logs/04-audit.log` and `logs/13-audit.log` apart from the advisory-database header: `Loaded 1261 security advisories` now versus `1246` before (`diff` of the two logs minus wrapper lines shows only that line). Same 573 crate dependencies scanned; same 5 vulnerabilities (RUSTSEC-2026-0204 `crossbeam-epoch 0.9.18`, RUSTSEC-2026-0258 `h2 0.4.14`, RUSTSEC-2026-0185 `quinn-proto 0.11.14`, RUSTSEC-2026-0220 `ruint 1.18.0`, RUSTSEC-2026-0285 `rustls 0.23.40`); same 4 unmaintained, 3 unsound and 4 yanked warnings, with identical versions. **No advisory added, none removed, none changed version.** The 15 advisories newly loaded into the database match nothing in this lock file.

`Cargo.lock` delta (`git diff 3ec8bc5 fe9e84c -- Cargo.lock`), complete:

```
@@ -4301,6 +4301,8 @@ dependencies = [
  "argh",
  "async-trait",
  "axum",
+ "bitflags",
+ "metrics",
  "reqwest",
  "safenet-core",
```

The hunk lies inside the `[[package]] name = "sentinel-engine"` block (`Cargo.lock:4296-4297` immediately precede it). Both lock files contain 573 `[[package]]` entries with the identical name list (`diff` of the two name lists is empty); the line count went 6,169 → 6,171. So the delta adds **two dependency edges from `sentinel-engine` and no package, no version change**. `bitflags v2.13.0` was already resolved in the old lock (`git show 3ec8bc5:Cargo.lock` has `name = "bitflags" / version = "2.13.0"`).

Reachability proof for the two edges (all commands at `fe9e84c`):

- `cargo tree --workspace -i bitflags -e normal`: `bitflags v2.13.0` has three reverse dependents: `sentinel-engine` (the new direct edge), `tower-http v0.6.11` ← `reqwest v0.13.4` ← `alloy-provider`/`alloy-rpc-client`/`alloy-transport-http` ← `alloy` ← `safenet-core`/`sentinel`/`validator`, and `tower-http v0.7.0` ← `sentinel-engine` only. The `reqwest` path is how `bitflags` already reached `safenet-core`, `sentinel` and `validator` at `3ec8bc5`; `cargo tree -p sentinel -e normal`, `-p validator -e normal` and `-p safenet-core -e normal` each show `bitflags v2.13.0` once, at depth 8, under that same path.
- `cargo tree --workspace -i metrics -e normal`: `metrics v0.24.6` was already a **direct** dependency of `safenet-core` (`crates/core/Cargo.toml:16`), `sentinel` (`crates/sentinel/Cargo.toml:10`) and `validator` (`crates/validator/Cargo.toml:13`); the only new dependent is `sentinel-engine`.
- The three in-scope crate manifests are unchanged (`git diff --stat` lists neither `crates/*/Cargo.toml` except `crates/sentinel-engine/Cargo.toml`, which gained `bitflags.workspace = true` and `metrics.workspace = true`).

**Verdict: no changed `Cargo.lock` line touches the dependency graph of `safenet-core`, `validator` or `sentinel`.** The root `Cargo.toml` change (`bitflags = "2"` added to `[workspace.dependencies]`, `Cargo.toml:10`, 26 → 27 lines) only declares a version for a crate the engine now consumes directly. The advisory reachability picture recorded for R10 at `3ec8bc5` is unchanged.

### 1.4 `cargo tree -d --workspace` (`logs/14-tree-d.log`, 945 lines)

The set of packages present in more than one distinct version is identical to `baseline.md` Section 2.3 (mechanically compared by extracting `name vX.Y.Z` pairs from both logs): `block-buffer`, `const-oid`, `cpufeatures`, `crypto-common`, `digest`, `getrandom`, `hashbrown` (3 versions), `hmac`, `rand`, `rand_chacha`, `rand_core`, `sha2`, `tower-http`, `webpki-roots` — 14 packages, same version pairs. **No new version split, none resolved.** `tower-http 0.7.0` (the out-of-scope side of that split) now additionally sits next to the new engine `bitflags` edge, but that split pre-dates this merge.

## 2. Inventory delta (in-scope files)

`git diff --stat 3ec8bc5 fe9e84c -- crates/core crates/validator crates/sentinel Cargo.toml Cargo.lock`:

```
 Cargo.lock                      |   2 +
 Cargo.toml                      |   1 +
 crates/sentinel/src/bindings.rs |   2 +
 crates/sentinel/src/service.rs  | 199 +++++++++++++++++++++++++++++++++++++++-
 4 files changed, 199 insertions(+), 5 deletions(-)
```

`git diff --quiet 3ec8bc5 fe9e84c -- crates/core crates/validator` exits 0: **`crates/core` and `crates/validator` are byte-identical**, so `baseline.md` Sections 3.1 and 3.2 stand unchanged (7,701 / 99 and 8,728 / 42). The whole-repository diff is 160 files, +4,879 / −19,245, almost entirely `crates/sentinel-engine` (out of scope), `contracts/`, `scripts/` and `docs`; none of it is in scope except the four files above.

### 2.1 Changed in-scope files

| File | Lines `3ec8bc5` | Lines `fe9e84c` | Δ | Tests `3ec8bc5` | Tests `fe9e84c` | Upstream commit |
| --- | --- | --- | --- | --- | --- | --- |
| `crates/sentinel/src/service.rs` | 2,158 | **2,347** | +189 | 16 | **18** | `8b6a75d` "Adjust deadline handling for quicker reaction (#914)" (logic + 2 tests); `a56c626` "[Oracle Audit] Fix I-05 (#944)" (1 test-helper line) |
| `crates/sentinel/src/bindings.rs` | 173 | **175** | +2 | 0 | 0 | `a56c626` "[Oracle Audit] Fix I-05 (#944)" |
| `Cargo.toml` | 26 | **27** | +1 | — | — | `d266200` "[Phase 1] Introduce Assessment, Coverage and Aspect (#921)" (engine series) |
| `Cargo.lock` | 6,169 | **6,171** | +2 | — | — | same |

Revised totals: `crates/sentinel` 10 files, **4,063** lines (was 3,872; +191), **44** tests (was 42; +2). In-scope `.rs` total **20,492** lines (was 20,301), **185** tests (was 183). File count unchanged (61 tracked `.rs`; nothing added or removed). Unchanged non-Rust in-scope files, re-measured: `crates/core/Cargo.toml` 31, `crates/validator/Cargo.toml` 28, `crates/sentinel/Cargo.toml` 24, `crates/validator/Dockerfile` 37, `crates/sentinel/Dockerfile` 38, `validator.sample.toml` 77, `sentinel.sample.toml` 61 — all equal to `baseline.md` Section 3.5.

### 2.2 Functional description of the sentinel change (`git diff 3ec8bc5 fe9e84c -- crates/sentinel/src`)

Described, not judged; the delta reviewer assesses it.

**`crates/sentinel/src/bindings.rs`** (2 insertions):

- Line 3: `#[allow(clippy::too_many_arguments)]` added on `pub mod oracle`.
- Line 31: the `NewRequest` event in the `sol!` block gains a field `uint24 daoFeeShare` between `uint96 bondTarget` and `uint96 slashAmount`. The event signature (and therefore its `topic0` and its ABI data layout) becomes `NewRequest(bytes32,address,uint96,uint96,uint24,uint96,uint64,uint64)`. This mirrors the same field added to the Solidity event (Section 2.3).

**`crates/sentinel/src/service.rs`** (`SentinelTransition::handle_block_advance`, now at lines 403–…; all logic changes are inside the `state.0.retain(…)` closure):

- Lines 390–402: a new doc-comment paragraph explaining that `block` in `Message::NewBlock(block)` is already mined, so the earliest inclusion block for any action emitted now is `block + 1`, and every deadline comparison is therefore made "one block ahead". It is labelled `STOPGAP` and references `safe-research/safenet#471` (block transitions to be run against the _pending_ block), stating that the compensation must be reverted together with that change.
- `WaitingForEngineCheck { deadline, request }`: retention test changed from `block <= …commit_deadline-or-deadline` to `block < …` (old line 396, new line 409). Effect: the entry is dropped at `block == deadline` instead of `deadline + 1`.
- `WaitingForRequest { deadline, .. }`: `block <= *deadline` → `block < *deadline` (old 400, new 413). Same one-block-earlier drop.
- `CollectingCommitments { … }`: the early `return true` guard changed from `block <= *commit_deadline` to `block < *commit_deadline` (old 410, new 423). Then, for `!*self_committed`, the former unconditional `return false;` became `return block <= *commit_deadline;` (old 414, new 432) with a new comment (lines 427–431) explaining that `NewBlock(block)` is applied before that block's own logs, so a `Committed` of ours mined in `commit_deadline` has not been tallied yet, and dropping now would forfeit a posted bond. Net timing: with `self_committed == true` the `Reveal` action is now emitted at `block == commit_deadline` (previously `commit_deadline + 1`); with `self_committed == false` the entry is still kept at `block == commit_deadline` and dropped at `commit_deadline + 1` (unchanged timing).
- Tests: `new_request_event` helper gains `daoFeeShare: Default::default()` (line 1159) to match the binding. Two new tests: `flow_reveals_on_the_commit_deadline_block` (line 2164; asserts no command at `NewBlock(19)`, a `Reveal` with `expires_at: Some(40)` and transition to `CollectingVotes` at `NewBlock(20)` for `commit_deadline == 20`) and `flow_defers_dropping_until_the_commit_deadline_block_is_indexed` (line 2242; two requests, neither self-committed at `NewBlock(20)`, both survive; a `Committed` log at block 20 for one of them; at `NewBlock(21)` that one reveals and the other is dropped).

No other in-scope Rust line changed: `action.rs`, `config.rs`, `effect.rs`, `engine.rs`, `hashing.rs`, `main.rs`, `metrics.rs`, `state.rs` are identical.

### 2.3 Functional description of the reference-contract change (`git diff 3ec8bc5 fe9e84c -- contracts/src`)

`contracts/src` is the hashing/encoding/protocol reference (PROMPT.md Section 2); Solidity receives no findings. Three files changed (+55 / −38), from the "[Oracle Audit] Fix …" series `dab8ef9`, `ecea6a9`, `e6866a3`, `4df721c`, `a56c626`, `1d89801`:

- **`contracts/src/SentinelOracle.sol`** (+27 / −6): **comment-only** — every changed line is a `//` comment (verified by filtering the diff). New documentation on `FEE_TOKEN` (must be a standard, non-rebasing ERC20 with ≤ 18 decimals and no transfer hooks; a hooked token could let a sponsor revert its own refund and freeze the request), on `GOVERNANCE_DELAY` (zero is a deliberately valid value making governance changes immediate), on the constructor config, and on `timeoutArbitration`/`markOutOfScope` (a non-revealer's bond, already slashed at `finalize()`, is not refunded by these paths).
- **`contracts/src/libraries/SentinelOracleCommitments.sol`** (+3 / −5): new error `InvalidCommitHash()`; `checkNotCommitted` removed; `add(...)` now `require(commitHash != bytes32(0), InvalidCommitHash())` and checks `vote == Vote.NONE` (instead of `commitHash == 0`) for `AlreadyCommitted()`.
- **`contracts/src/libraries/SentinelOracleRequests.sol`** (+25 / −27): `applyDaoFeeCut` gains a `uint16 winningSideCount` parameter; after deducting the DAO share it rounds the remaining fee **down to a multiple of `winningSideCount`**, adds the rounding remainder to the returned `daoCut`, and stores the rounded pot in `self.progress.fee`. Both callers pass the winning side's sentinel count: `finalize` (`approveMet ? prog.approveSentinelCount : prog.denySentinelCount`) and `resolveDispute` (same from `self.progress`). The `NewRequest` event declaration gains `uint24 daoFeeShare` after `bondTarget`, and the `emit NewRequest(...)` in `SentinelOracleRequestMap` passes `daoFeeShare` in that position. Comments on `timeoutArbitration`/`outOfScope` are reworded to state the non-revealer caveat.

Cross-reference to Rust, stated factually: of these, only the `NewRequest` ABI change has a counterpart in `crates/sentinel` (`bindings.rs:31`, `service.rs:1159`). `crates/sentinel` does not decode the fee split, `InvalidCommitHash`, or `winningSideCount` rounding; whether any of them bears on an existing or new Rust finding is for the delta reviewer.

## 3. Drift note: run-2 findings whose sentinel anchors moved

Method: `grep -oE 'crates/sentinel/src/(service|bindings)\.rs:[0-9]+(-[0-9]+)?'` over `rust-audit/findings/F2-*.md` (17 files, 61 distinct anchors), then each cited old range was mapped to `fe9e84c` with a line-level `difflib.SequenceMatcher` over the two committed blobs. "same" = identical line numbers and text; "moved +n" = same text, shifted; "**content changed**" = a cited line no longer exists verbatim. Offsets in `service.rs`: lines ≤ 389 unchanged; 390–416 are the rewritten `handle_block_advance` region (+13 growing to +18); 417–1140 shift +18; 1141–2137 shift +19; ≥ 2138 shift +189. In `bindings.rs`: lines 3–29 shift +1, ≥ 30 shift +2.

| Finding | Anchors in `service.rs` / `bindings.rs` at `3ec8bc5` → `fe9e84c` | Status |
| --- | --- | --- |
| F2-CORE-030 | service 127-139, 214-242, 264-276, 307-319 same; **service 393-399 → 406-412: content changed at old line 396** (`<= request` → `< request`) | **re-anchor and re-read**: claim row 11 ("kept until the commit deadline and then dropped") describes the pre-merge timing |
| F2-SEN-001 | service 156-171, 176-179, 307-319 same; **service 394-399 → 407-412: content changed at old line 396** | **re-anchor and re-read**: claim row 7 ("Past `commit_deadline`, an entry without `self_committed` is dropped …; `WaitingForEngineCheck` likewise") sits on the rewritten logic |
| F2-SEN-003 | service 347-362, 376-379 same; service 631-637 → 649-655 (+18) | moved; claim row 7 concerns the `CollectingCommitments → CollectingVotes` transition on the first post-warp block, whose trigger block changed (Section 2.2) — reviewer should re-check the wording |
| F2-SEN-002 | service 214-224, 264-276, 307-319, 372-375 same; 631-637 → 649-655, 639-653 → 657-671, 780-786 → 798-804 (+18) | moved only |
| F2-SEN-004 | bindings 42 → 44, 50-60 → 52-62 (+2); service 466-467 → 484-485, 928-933 → 946-951 (+18) | moved only |
| F2-SEN-005 | service 137-144, 181-183 same; 426-436 → 444-454 (+18; the `Reveal` push inside the changed arm, text unchanged) | moved only |
| F2-SEN-006 | service 226-242, 286-289 same; bindings 66 → 68 (+2) | moved only |
| F2-SEN-007 | service 226-242 same; 545-557 → 563-575, 688-694 → 706-712, 848 → 866 (+18) | moved only |
| F2-CORE-063 | service 525 → 543 (+18) | moved only |
| F2-XC-050 | service 520-526 → 538-544, 545 → 563 (+18) | moved only |
| F2-XC-004 | service 1076 → 1094 (+18) | moved only |
| F2-CORE-004 | bindings 14 → 15 (+1), 77 → 79, 165-171 → 167-173 (+2) | moved only |
| F2-CORE-010 | bindings 25-48 → 26-50 (range now spans the inserted `daoFeeShare` line 31), 167-172 → 169-174 (+2) | moved; the cited `NewRequest` event text gained one field |
| F2-CORE-032 | service 226-242 | same — no drift |
| F2-CORE-035 | service 127-139 | same — no drift |
| F2-CORE-064 | service 231 | same — no drift |
| F2-SEN-008 | service 176-179 | same — no drift |

No `F2-*` finding quotes any of the four changed comparison lines verbatim (grep for `block <= *deadline`, `<= request`, `block <= *commit_deadline`, `return false;` in `findings/F2-*.md` matches only prose rows in F2-CORE-030, F2-SEN-001, F2-SEN-003 and unrelated VAL rows). Findings citing only other sentinel files (`state.rs`, `engine.rs`, `effect.rs`, `config.rs`, `hashing.rs`, `metrics.rs`) are unaffected because those files are byte-identical.

Run-2 state files that also cite `service.rs`/`bindings.rs` line numbers and will need the same offsets applied by the Manager: `state/run2/STATE.md`, `state/run2/baseline.md` (Section 3.3 line counts 2158/173 and test count 42 are now stale), `state/run2/coverage.md`, `state/run2/reviewer-split.md`, `state/run2/agents/R7.md`, `state/run2/agents/R1.md`.

## 4. Summary for the Manager

- Exit codes at `fe9e84c`: build 0, test 0, clippy 0, audit 1 (expected), tree -d 0 — identical to `3ec8bc5`.
- Differences: sentinel unit tests 42 → 44 (in-scope 183 → 185); `service.rs` 2,158 → 2,347 lines, `bindings.rs` 173 → 175, `Cargo.toml` 26 → 27, `Cargo.lock` 6,169 → 6,171; core and validator byte-identical; advisory set, scanned-dependency count (573) and version-split set unchanged.
- Lock delta: two edges added to `sentinel-engine` only; no package or version change; no effect on the `safenet-core`/`validator`/`sentinel` graphs or on advisory reachability.
- Anchors moved in 13 findings (Section 3); two of them (F2-CORE-030, F2-SEN-001) cite a line whose text changed, and F2-SEN-003's row 7 describes timing that changed.
- Blockers: none. Caveat: a QA PoC hunk is currently present in the working copy of `crates/sentinel/src/service.rs` (not by this agent); all numbers here are from the committed blob.
