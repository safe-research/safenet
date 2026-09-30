> **Scope note (run 2):** `crates/sentinel-engine` is out of scope (PROMPT.md Section 11). Questions that exist only for the engine are withdrawn and kept for history: **Q3, Q4, Q10, Q19, Q20, Q21, Q23**. `Q-ENG-A` was removed with the engine PoCs.

# Unresolved questions — what a Rust toolchain would settle, in order

> **Merged canonical file.** Assembled by the Manager on from the four QA agents' restored parts, after the original shared file was overwritten mid-run (see `rust-audit/state/STATE.md`, "Data loss and recovery"). Every entry below was restored by its **own author from that agent's working context** — nothing here was reconstructed by guesswork, and no author wrote another author's entry.
>
> Structure: questions **0–23** are the main value-per-minute list (QA-XC), with original numbering preserved so existing `question N` citations in the finding files still resolve. **Q2 carries an addendum** from QA-CORE-SEN, inserted in place below. **Q-ENG-A**, the **not-answerable-by-reading-a-dependency** pair (2a/2b), and **VAL-Q1…VAL-Q9** follow the main list as their own sections.
>
> Source parts, kept for provenance: `-XC.md`, `-ENG.md`, `-CORE-SEN.md`, `-VAL.md`.

> **Recovery copy (QA-XC).** This is the original `UNRESOLVED-DEPENDENCY-QUESTIONS.md` as written by QA-XC, restored verbatim from the authoring agent's working context after the shared file was overwritten by a concurrent write. Numbering is unchanged, so every `question N of rust-audit/poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md` citation in the finding files still resolves. Two additions by other agents (`Q-ENG-A`, and QA-CORE-SEN's extension of Q2) were lost in the same event and were restored by their own authors and merged in below — I have not attempted to reconstruct anyone else's text.

Commit `2893917`. Compiled by **QA-XC** from the Coverage Critic's twenty-two toolchain-blocked questions (`rust-audit/state/coverage.md` §7) plus every open dependency question raised in a `## Critic` section, de-duplicated and re-ordered.

**Why this file exists.** No Rust toolchain exists on the machine this audit ran on: no `cargo`, `rustc`, `forge`, `anvil` or `just`, and `~/.cargo/registry` does not exist, so no dependency source is on disk (`rust-audit/state/baseline.md` §1-2; assumption A9 is FALSE). `E1` was reached **zero times**, the whole audit is capped at **89%** certainty, and every finding below is short of its final number by exactly one of these answers.

**How to use it.** It is ordered by value-per-minute, not by finding ID or by severity. Work top-down and stop when the returns fall off — everything above the line marked _"diminishing returns begin here"_ is worth doing on the first afternoon somebody has a toolchain; everything below it is worth doing before the next release.

**Two standing disclaimers, stated once.**

1. **No advisory status is asserted anywhere in this audit, and none may be.** `cargo audit` was never run and no dependency source was ever read. Questions 9 and 21 below are "unknown, here is how to find out" — they are _not_ suspicions about any package, and nothing in this audit should be quoted as a clean bill of health for the 516 locked packages either. Nobody looked, because nobody could.
2. **Nothing in this file was executed**, with the single exception of Q22, which needed only `python3` and is marked **ANSWERED** with its output saved.

Certainty percentages are the values in the finding headers at the time this file was written; several findings were still being critiqued in parallel, so re-read the finding before quoting a number.

---

## Summary

| # | Question | Effort | Unblocks | Current |
| --- | --- | --- | --- | --- |
| **0** | Does the workspace build and does `cargo test --workspace` pass? | 10 min (mostly waiting) | everything below; Phase 0's empty `E1` slot | not run |
| **1** | Does `frost-core` redact secrets in `Debug`? | **5 min** | `F-XC-002` 74%, `F-VAL-062` 60%, `F-CORE-036` 50% | `I` |
| **2** | Does `alloy-sol-types` reject non-UTF-8 in a `string` field? | 15 min | `F-SEN-013` 65% (the whole finding) | `I` |
| **3** | Does `deny_unknown_fields` work alongside `#[serde(flatten)]`? | 5 min | `F-XC-003` 58%, `F-VAL-063` 72% | `I` |
| **5** | What does `verify_proof_of_knowledge` do with an empty commitment vector? | 10 min | `F-XC-051` 42%, R4's O4 | `I` |
| **6** | Does alloy's ABI decoder pre-allocate from a declared array length? | 20 min | `F-XC-051` 42%, `F-VAL-001` 88%, `F-VAL-003` 80% | `I` |
| **7** | What are Cargo's stock `release` profile defaults on the pinned toolchain? | 2 min | `F-XC-001` 66% | `I` |
| **8** | Does `cargo tree -d` match the lockfile-derived duplicate list? | 2 min | `baseline.md` §6 | text parse only |
| **9** | Any advisory status for the 516 locked packages? | 5 min | `F-XC-007` 84% | **unknown, unasserted** |
| **11** | What does `estimate_eip1559_fees` return when `reward` is empty? | 20 min | `F-CORE-060` 82% | mock only |
| **12** | `sqlx` 0.9 defaults: `journal_mode`, `synchronous`, `busy_timeout`, `foreign_keys`, pool size | 20 min | `F-VAL-035` 45%, R2's O-6/O-7 | `I` |
| **13** | Does `frost-core` reject a signing package whose commitments do not match the nonces? | 15 min | `F-VAL-034` 40% | `I` |
| **14** | Can `round1::SigningNonces::new` panic on anything `NonceChunk::with_size` passes it? | 15 min | `F-VAL-031` 42% | `I` |
| **15** | Do `PrivateKeySigner` / k256 `SigningKey` zeroize on drop? | 30 min (source read) | R3's O4; every secret-at-rest severity | `I` |
| **16** | Five `alloy` 2.0.5 specifics R1 could not read | 45 min (source read) | `F-CORE-002` 85%, `F-CORE-010` 45%, `F-CORE-012` 70%, `F-CORE-065` 55% | `I` |
| **17** | Which TLS root source does `reqwest` select under the pinned features? | 15 min | `F-XC-004` 85%, R10's O3 | `I` |
| **18** | `reqwest`'s default redirect and proxy policy | 15 min | `F-XC-008` 80% (items 2-3) | `I` |
| **19** | Does the linker strip the unused `sqlx-mysql`/`sqlx-postgres` code? | 30 min | `F-XC-007` 84% item 2 | `I` |
| — | _diminishing returns begin here — the rest need something other than a toolchain_ |  |  |  |
| **22** | Is the HKDF reference vector reproducible? | — | `F-CORE-038` | ✅ **ANSWERED — yes** |
| **23** | Can the `sentinel-test-vectors` corpus be cloned and run? | hours + a team decision | every `F-ENG-*` | A8 unresolved |

**If you have ten minutes:** do #1. **If you have an hour:** #0, #1, #3, #5, #7, #8, #9 — seven answers, four findings re-scored, and the run's `E1` slot filled.

---

## 0. Does the workspace build, and does the existing suite pass?

**Do this first.** Every experiment below assumes a green baseline; a red one changes what a failing PoC means.

```sh
cargo build --workspace --all-targets --locked
cargo test --workspace
cargo clippy --workspace --all-targets --locked -- -D warnings
```

The third is what `Justfile:22-30` and `.github/workflows/ci.yml:37-39` require of every PR, and nothing in this audit observed it. Save all three logs under `rust-audit/state/logs/`.

- **All green** → the run's single unfilled Phase 0 `E1` slot is filled, and every PoC result below is trustworthy.
- **Anything red** → say so in the report before anything else. A finding derived from code that does not compile as shipped is a different finding.

**Effort:** 10 minutes, nearly all of it a cold `cargo build` of 516 packages.

---

## 1. Does `frost-core` 3.0.0 redact secrets in `Debug`?

**Unblocks:** `F-XC-002` (74%, Medium), `F-VAL-062` (60%), `F-CORE-036` (50%). Also VAL-H10. **Effort: 5 minutes.** The cheapest real evidence in this audit.

This is **not** a source-reading exercise, and treating it as one is the mistake to avoid. C-VAL-B established that `KeyShare::dummy` already exists at `crates/validator/src/frost/keygen.rs:443-453`, so a one-line `format!("{:?}", …)` assertion decides it locally, without opening upstream at all — and, unlike a source read, the assertion keeps deciding it after every future dependency bump.

**Run:** the ready test is at `rust-audit/poc/F-XC-002/append-to-crates-validator-src-frost-keygen.rs`. Append it to `crates/validator/src/frost/keygen.rs` and run:

```sh
cargo test -p validator qa_xc_002 -- --nocapture
```

Or, if you want the answer in thirty seconds rather than five minutes, paste this into any `#[test]` in that file:

```rust
println!("{:?}", KeyShare::dummy());   // dummy's signing share is the scalar 1
```

**What each answer implies:**

| Answer | Consequence |
| --- | --- |
| Redacted | `F-XC-002` and `F-VAL-062` → **Informational**. `F-CORE-036`'s separate workspace-contract half (core requires `Debug` on `Effect`/`Resume` and logs them at five sites, imposing no constraint on downstream types) stays **Low** on its own merits. |
| Printed | `F-XC-002` / `F-VAL-062` → **Critical**. `crates/validator/src/service/effect.rs:249` formats the whole `Effect` with `tracing::warn!(?effect, %err, …)`, which the shipped `log_filter = "info"` emits, on any `try_perform_effect` error — and `Effect::ReconcileGroupSecrets` carries every live `Arc<KeyShare>`. A single SQLite write failure (a locked file during the backup `docs/validator-handbook.md:75` tells operators to take) writes FROST key shares to the log. That is PROMPT.md §8's first Critical bullet. |

**The report must not present the current Medium as a measured midpoint.** It is a placeholder for an unanswered question that spans three severity bands, and the question costs five minutes.

If you want the upstream read as well (you do not need it): after `cargo fetch`, look at `~/.cargo/registry/src/index.crates.io-*/frost-core-3.0.0/src/` for the `Debug` impl on `keys::SigningShare` (the value that must not print), `keys::KeyPackage` (reached by `KeyShare`'s newtype derive), `keys::dkg::round1::SecretPackage` (reached by `Secrets`' derive — check its `coefficients` field), and `keys::SecretShare`. Checksum `81ef2787af391c7e…`, `Cargo.lock:2089-2092`. Check whether the `internals` feature, which `crates/validator/Cargo.toml:9` enables, changes any of them.

---

## 2. Does `alloy-sol-types` 1.6.0 reject invalid UTF-8 in a `string` field?

**Unblocks:** `F-SEN-013` (65%), whose severity is currently written as the conditional _"High if basis 8 holds, else Informational"_. **Effort: 15 minutes.**

> *(QA-CORE-SEN appended to this question the source-read steps C-SEN asked for by name, plus a warning that an `Ok` answer does **not** close `F-CORE-004`. That addendum was lost in the same overwrite. QA-CORE-SEN restored its own text, which is merged in directly below this line.

The single biggest answer in this list. If `decode_raw_log` returns `Err` on non-UTF-8, then one `reveal` call by any active sentinel — an actor inside A2's fault bound — stalls **every** sentinel's _and_ every validator's indexer permanently, not just the attacker's own:

`E::decode_log(...).ok` discards the error (`crates/core/src/index/events.rs:546-554`), one `None` aborts the whole batch with `Error::DecodeLog` (`:495-516`), and the driver classifies every watcher error except `ExceededMaxReorgDepth` as transient and retries the same range every 100 ms forever (`crates/core/src/driver.rs:206-225`). The log is a permanent feature of the canonical chain, so restarting does not help. Every sentinel with an outstanding commitment then fails to reveal and is slashed.

**Run** — append to a `#[cfg(test)] mod tests` in `crates/sentinel/src/bindings.rs`:

```rust
#[test]
fn qa_non_utf8_reason_decode {
    use alloy::{primitives::{Address, B256, b256}, sol_types::{SolEvent as _, SolEventInterface as _}};
    use crate::bindings::oracle::SentinelOracle::{Revealed, SentinelOracleEvents};

    // Revealed(bytes32 indexed requestId, address indexed sentinel,
    //          bool approved, uint96 bondAmount, string reason)
    let topics = [
        Revealed::SIGNATURE_HASH,
        b256!("0x00000000000000000000000000000000000000000000000000000000000000aa"), // requestId
        b256!("0x000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266"), // sentinel
    ];
    // approved = true; bondAmount = 0; offset = 0x60; len = 1; byte = 0x80
    // (a lone UTF-8 continuation byte, which is never valid on its own)
    let data = alloy::hex::decode(concat!(
        "0000000000000000000000000000000000000000000000000000000000000001",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000060",
        "0000000000000000000000000000000000000000000000000000000000000001",
        "8000000000000000000000000000000000000000000000000000000000000000",
    )).unwrap();

    let decoded = SentinelOracleEvents::decode_raw_log(&topics, &data);
    println!("{decoded:?}");
    assert!(decoded.is_ok, "non-UTF-8 reason is REJECTED — F-SEN-013 is High");
}
```

```sh
cargo test -p sentinel qa_non_utf8_reason_decode -- --nocapture
```

**What each answer implies:**

| Answer | Consequence |
| --- | --- |
| `Ok` (lossy, e.g. `String::from_utf8_lossy`) | `F-SEN-013` → **Informational**. The FSM never reads `event.reason` (`crates/sentinel/src/service.rs:335-385`), so replacement characters are harmless. The residual note — that one undecodable log of _any_ kind poisons a whole batch and is then retried forever — stays worth recording. |
| `Err` | `F-SEN-013` → **High, Confirmed**, and it is the most serious liveness finding in the audit: a fleet-wide, unrecoverable stall for the price of one bond and one `reveal`. Remediation 2 (skip undecodable logs with a `warn` and a counter, rather than failing the batch) becomes urgent, and remediation 5 (alert on `safenet_core_block_number{status="processed"}` going flat) should ship with it. |

**Do the same check for the other three `string` fields** in the same watched set — `DisputeResolved.context` and `DisputeOutOfScope.context` (`crates/sentinel/src/bindings.rs:41-43`) — since they come from the arbitrator rather than from a sentinel and would widen or narrow the trigger.

---

### Addendum from QA-CORE-SEN (two points that version does not cover)

**1. Do the source read as well as the run.** C-SEN asked for it by name in F-SEN-013's `### Exactly what QA must read to settle basis 8`, and it earns its five minutes for a reason the assertion alone does not give you: the read explains _why_ the answer is what it is, so it keeps being informative after a dependency bump that a bare assertion would only flag after the fact. Two items:

1. `~/.cargo/registry/src/*/alloy-sol-types-1.6.0/src/types/data_type.rs` — the `impl SolType for sol_data::String` block. Read `detokenize`, and `valid_token` / `type_check` if present. The whole question is one line: does it go through `String::from_utf8` (**checked** → returns `Err`), or `String::from_utf8_lossy` / `from_utf8_unchecked` (**lossy** → returns `Ok`)?
2. `~/.cargo/registry/src/*/alloy-sol-types-1.6.0/src/types/event.rs` — `SolEvent::decode_raw_log` and `SolEventInterface::decode_raw_log`. Confirm a `detokenize` error propagates as `Err` rather than being swallowed.

Alloy has historically used the lossy path, which would refute the finding. **Do not assume it.** C-SEN explicitly refused to upgrade basis 8 on that recollection and so do I — under PROMPT.md §2 an assertion about a pinned dependency's internals with no source on disk is class `I` at best, and no amount of protocol reasoning about what a decoder "should" do substitutes for reading it.

**2. An `Ok` answer does not close `F-CORE-004`.** F-CORE-004 is canonical for the _mechanism_ — a deterministic, content-dependent decode failure retried every 100 ms forever with no terminal state (`crates/core/src/index/events.rs:495-516`, `crates/core/src/driver.rs:206-225`) — and that mechanism survives a lossy decoder completely untouched. Only the cheap, attacker-chosen path into it goes away. A green run on Q2 must **not** be read as "the batch-poisoning design is fine".

Relatedly: **F-SEN-013's remediation option 2** (skip undecodable logs in `decode_and_sort` with a `warn` and a counter) must not be taken as a blanket `core` change, whatever Q2 answers. `decode_and_sort` is shared by every service, and silently dropping a log a **validator** needed — a `Sign`, a `KeyGenSecretShared`, a `Preprocess` — is precisely the F-CORE-002 failure mode of committing an incomplete batch as complete, which this audit rates High. F-CORE-004 option 2 and F-SEN-013 option 2 propose the same skip and will look like consensus; F-CORE-002 option 1 points the opposite way on the same code path. The sound pair is **option 3** (make `DecodeLog` terminal rather than transient — which depends on F-CORE-030, since `Driver::run` currently discards its outcome and the process exits with status 0) plus **option 5** (alert on a flat `safenet_core_block_number{status="processed"}`, `driver.rs:313-317`). If skipping is adopted at all, the boundary belongs in F-CORE-004: only for a log that _cannot_ be a valid protocol message, only paired with F-CORE-006's per-address topic sets, and never in a way that lets an empty or short result pass a completeness gate.

---

## 3. Does `#[serde(deny_unknown_fields)]` do anything on a container that also has `#[serde(flatten)]`?

**Unblocks:** `F-XC-003` (58%), `F-VAL-063` (72%, its top-level-key leg). **Effort: 5 minutes.**

A known library subtlety, and the repository is split on it: the engine has a `rejects_unknown_field` test (`crates/sentinel-engine/src/config.rs:129-144`) and is also the one config with **no** flattened field, so its green test proves nothing about the interaction. The validator and the sentinel both flatten and neither has such a test.

One fact here needs no library knowledge at all: `core::driver::Config` — the struct everything flattened is routed into — is `#[serde(default)]` with **no** `deny_unknown_fields` (`crates/core/src/driver.rs:29-37`), unlike all five of its siblings in `core` (`tx/mod.rs:70`, `index/blocks.rs:48`, `index/mod.rs:21`, `index/events.rs:72`, `observability/mod.rs:17`).

**Run:** the six ready tests are in `rust-audit/poc/F-XC-003/`.

```sh
cargo test -p validator config::tests::qa_xc_003
cargo test -p sentinel  config::tests::qa_xc_003
```

**What each answer implies:**

| Answer | Consequence |
| --- | --- |
| All reject | `F-XC-003` → **Informational** (a test gap, now filled). Keep the tests. |
| Top-level key accepted | `F-XC-003` → **Confirmed**, and **Low → Medium** is defensible: the failure is undetectable at runtime, since `crates/validator/src/main.rs:43` logs only the config file path and no metric or startup echo exists. Confirms the same leg of `F-VAL-063`. |
| A typo inside `[index]` accepted | The sharpest form. `use_client_filtering` is the A4 integrity switch that makes the watcher verify `eth_getLogs` results client-side; `max_reorg_depth` is the A5 knob. Either degrades in silence. Call it out by name in the report. |
| The two crates disagree | The validator flattens with `#[serde(default, flatten)]`, the sentinel with a bare `#[serde(flatten)]`. The difference is itself the answer, and the fix must cover both. |

---

## 4. Can an un-timed `reqwest` request actually hang indefinitely?

**Unblocks:** `F-ENG-005` (80%), `F-ENG-043` (74%), `F-CORE-011` (60%), `F-CORE-039` (55%) — four findings on one answer. **Effort: 10 minutes** (two `#[ignore]`d tests, 5 s each).

This is the premise that turns a mechanism into a stall. `CowChecker::new` builds a bare `reqwest::Client::new` with no request or connect timeout (`crates/sentinel-engine/src/checkers/cow.rs:228-231`); whether that can park forever is currently class `I` in four files.

**Run:** `rust-audit/poc/F-XC-008/append-to-crates-sentinel-engine-src-checkers-cow.rs`, which drives the real private `ReqwestOrderApi` against a loopback listener that accepts and never answers, and includes the control case proving the proposed fix bounds it.

```sh
cargo test -p sentinel-engine qa_xc_008 -- --ignored --nocapture
```

**What each answer implies:**

| Answer | Consequence |
| --- | --- |
| Returns within a cap | Record the observed duration — it re-scores four findings downwards at once, from "stall" to "delay". |
| The control also hangs | The proposed `.timeout`/`.connect_timeout` fix does not work; fall back to a `tokio::time::timeout` wrapper at the call site. |

---

## 5. What does `frost_core::keys::dkg::verify_proof_of_knowledge` do with an empty commitment vector?

**Unblocks:** `F-XC-051` (42%, Plausible), R4's Observation O4. **Effort: 10 minutes.**

C-VAL-A's verdict on that finding ends with the same request in its own words: run `verify_commitment` against `c = []` and `c[0] = (0,0)` and record what `frost-core` actually does — "that single unreadable fact is doing all the work", and it decides between Low and High in one run.

`verify_commitment` carries this comment (`crates/validator/src/frost/keygen.rs:83-86`):

> `// Note that we do not check the length of the commitments, this is enforced by the smart` `// contract and any issues will be caught later and produce an unexpected FROST error.`

The second clause is an unverified assertion about an upstream crate sitting inside a security-critical decoder, and `marshal::frost_commitment` maps over `commitment.c` with no length check (`crates/validator/src/frost/marshal.rs:105-121`). A panic on that path is caught nowhere — there is no `catch_unwind` in the driver, and the call site is a state-machine event handler (`crates/validator/src/state/keygen.rs:173`).

**Run:** `rust-audit/poc/F-XC-051/append-to-crates-validator-src-frost-keygen.rs` — three cases: `c = []`, `c = [identity, identity]`, and `c` shorter than the threshold.

```sh
cargo test -p validator qa_xc_051 -- --nocapture
```

**What each answer implies:**

| Answer | Consequence |
| --- | --- |
| `Err` on all three | The comment is right. `F-XC-051` → **Informational**; keep the tests so the claim is pinned rather than asserted. |
| Panic on `c = []` | A malformed `KeyGenCommitted` event aborts the validator. Severity then depends on `F-VAL-060` (50%): if a non-coordinator can emit the event, this is a remote panic from attacker-controlled chain data under A2 and `F-XC-051` is **High**. |
| `Ok` on `c = []` or on identity coefficients | No structural validation happens at all, and the Rust is relying entirely on a contract that `F-VAL-060` argues is not necessarily in the event path. Take remediation 1 (validate `c` in `frost_commitment`) regardless of `F-VAL-060`'s outcome. |

---

## 6. Does alloy's ABI decoder pre-allocate a `Vec` from a declared array length before validating it?

**Unblocks:** `F-XC-051` (42%), `F-VAL-001` (88%), `F-VAL-003` (80%). **Effort: 20 minutes.**

A memory-exhaustion question under A2. `KeyGenCommitment.c` is a `Point[]` and `KeyGenSecretShare.f` is a `uint256[]` (`crates/validator/src/bindings.rs:63-76`), both arriving from event data that `F-VAL-060` argues is injectable. If the decoder allocates from the declared length prefix before checking it against the actual payload size, a log whose length prefix is `2^32` with a two-word payload is a cheap OOM against every validator that indexes it.

**Run:**

```rust
#[test]
fn qa_array_length_preallocation {
    use alloy::sol_types::SolValue;
    // uint256[] with a declared length of 2^32 and no elements at all.
    let data = alloy::hex::decode(concat!(
        "0000000000000000000000000000000000000000000000000000000000000020", // offset
        "0000000000000000000000000000000000000000000000100000000000000000", // length = 2^68
    )).unwrap();
    let decoded = <Vec<alloy::primitives::U256>>::abi_decode(&data);
    println!("{:?}", decoded.map(|v| v.len()));
}
```

Watch RSS while it runs (`/usr/bin/time -v`, or run it under a `ulimit -v`). Escalate the declared length gradually — `2^20`, `2^32`, `2^68` — and record where, if anywhere, memory tracks it.

**What each answer implies:**

| Answer | Consequence |
| --- | --- |
| Errors immediately, allocation flat | No finding. Record it in the report's rejected list so nobody re-raises it. |
| Allocates proportional to the prefix | A new **High** finding in its own right (a single log OOMs every indexing validator and sentinel), independent of `F-VAL-001`/`F-VAL-003`, and it needs its own file. |

---

## 7. What are Cargo's stock `release` profile defaults on the pinned toolchain?

**Unblocks:** `F-XC-001` (66%, Informational). **Effort: 2 minutes.**

The workspace has no `[profile.release]` anywhere (`grep -rn 'profile' Cargo.toml crates/*/Cargo.toml` → no output; no `.cargo/`, no `rust-toolchain*`), and all three Dockerfiles build `--release`. The finding's remaining `I` leg is simply what that implies.

```sh
cargo build --release --workspace -v 2>&1 | grep -o "\-C debug-assertions=[a-z]*\|\-C overflow-checks=[a-z]*" | sort -u
```

- **Confirms `overflow-checks = false` / `debug-assertions = false`** → `F-XC-001` stays **Informational**: the two `debug_assert`s in the tree are dead in production, and the sweep in its Critic section found no reachable arithmetic consequence. Remediation 1 (add `overflow-checks = true`) remains worth doing as cheap insurance.
- **Shows either enabled** → the finding is void; say so.

Note the Critic's caution on remediation 3: `crates/validator/src/consensus/group.rs:236` sits on the epoch-rollover path, so promoting that `debug_assert!` to `assert!` would convert a theoretically-impossible condition into a validator crash. An `if`-guarded `tracing::error!` is the safe form.

---

## 8. Does `cargo tree -d` match the lockfile-derived duplicate list?

**Unblocks:** `rust-audit/state/baseline.md` §6. **Effort: 2 minutes.**

```sh
cargo tree -d --workspace --locked
```

The baseline's duplicate list was produced by a text parse of `Cargo.lock`, which cannot see feature-gated edges. C-XC re-read ten lockfile blocks by eye and found **zero** discrepancies, so the expectation is that this confirms the list — but it is two minutes and it either closes the caveat or finds the thing the parse could not see. The two facts that matter downstream are that the validator's `rand 0.8.6` and all three FROST crates sit on `rand_core 0.6.4` (so `keygen::setup` handing its `&mut R` to `dkg::part1` is type-compatible for the right reason), and that core's `sha2 0.11` reaches only `kdf.rs`.

---

## 9. Is there any advisory affecting any of the 516 locked packages?

**Unblocks:** `F-XC-007` (84%, Informational). **Effort: 5 minutes.**

**Read the standing disclaimer at the top of this file before quoting this section.** No CVE, no RUSTSEC identifier and no claim about any pinned version appears anywhere in this audit. `F-XC-007`'s claim is about the _pipeline_ — there is no advisory gate in `.github/workflows/ci.yml` or the `Justfile` — not about any package.

I verified that independently, as instructed, because this is the file most likely to attract an advisory claim: `grep -n -i -E "RUSTSEC|CVE-|vulnerab|advisor|exploit|patched|yanked|unmaintained|outdated"` over `rust-audit/findings/F-XC-007.md` returns twelve lines, and every one is a process claim, an explicit disclaimer, a remediation proposal, or a subject-less hypothetical. **Confirmed clean.** The nearest approach is "rand 0.8 is a superseded line", which is a version-currency fact read off `Cargo.lock` and is immediately labelled "a maintenance fact, not a defect".

```sh
cargo install cargo-audit --locked && cargo audit
# or, preferred, since it also covers bans and licences:
cargo install cargo-deny --locked && cargo deny check
```

- **Clean** → record the date and the advisory-DB revision. That is the first advisory statement this project can honestly make.
- **Not clean** → a new finding per advisory, scored on reachability, not on the CVSS number.

Either way, `F-XC-007` remediation 1 (add the gate to CI, scheduled daily as well as on PR, so it fires between releases) is the highest-value line in that Informational finding.

---

## 10. Does axum's default `JsonRejection` echo a fragment of the request body?

**Unblocks:** `F-ENG-008` (80%), R10's Observation 5. **Effort: 10 minutes.**

The engine's two extractor rejections are `(StatusCode, &'static str)` (`crates/sentinel-engine/src/api/extractors.rs:22-25`, `:43-46`), but the statuses axum itself contributes (`415`, `422`, `413`, `405`, `404`) and their bodies are library behaviour. So is whether a panic in a handler yields _no response at all_, since `Cargo.toml:24` takes `tower-http` with only `["trace"]` and not `catch-panic`.

```sh
cargo run -p sentinel-engine -- --config-file crates/sentinel-engine/sentinel-engine.sample.toml &
curl -sS -i -X POST localhost:5473/v1/security-check -H 'content-type: application/json' \
     -d '{"chainId": "not-a-number", "safe": "0x00"}'
curl -sS -i -X POST localhost:5473/v1/security-check -d 'not json at all'
curl -sS -i -X GET  localhost:5473/v1/security-check
curl -sS -i -X POST localhost:5473/v1/nonexistent
```

Record the exact status and body of each. Under A3 the caller is the trusted co-deployed sentinel, so a leaked value fragment is Informational — but the documented API contract in `docs/sentinel-engine.md` should list every status the service can actually return, which is the substance of `F-ENG-008`.

---

## 11. What does alloy's `estimate_eip1559_fees` issue and return when `reward` is empty?

**Unblocks:** `F-CORE-060` (82%). **Effort: 20 minutes.**

`F-CORE-060`'s starting fee level currently comes from the tests' mocked `fee_history`, not from the real estimator, so the finding's arithmetic begins from a number nobody has observed. Point a test at a local `anvil` (Foundry 1.5.1 per A9) with an empty reward history and record both the `eth_feeHistory` request the estimator issues and the `max_fee_per_gas`/`max_priority_fee_per_gas` it returns.

- A high or unbounded starting level makes `F-CORE-060`'s cap-and-bump interaction worse than filed; a conservative one makes it milder. Either way the finding's numbers should be re-derived from the observed value rather than from the mock.

---

## 12. `sqlx` 0.9 SQLite defaults

**Unblocks:** `F-VAL-035` (45%, the `ON DELETE CASCADE` leg / M6), R2's Observations O-6 and O-7. **Effort: 20 minutes.**

`connect_sqlite` sets only the two recycling knobs (`crates/core/src/utils.rs:56-62`). Everything else — `journal_mode`, `synchronous`, `busy_timeout`, `foreign_keys`, `create_if_missing`, pool size — is a library default, and **`foreign_keys` is the one that decides `F-VAL-035`**: SQLite does not enforce foreign keys unless `PRAGMA foreign_keys = ON` is issued per connection, so an `ON DELETE CASCADE` may be decorative.

```rust
#[tokio::test]
async fn qa_sqlite_pragmas {
    let pool = safenet_core::utils::connect_sqlite(/* the same options main.rs uses */).await.unwrap();
    for pragma in ["foreign_keys", "journal_mode", "synchronous", "busy_timeout"] {
        let v: (String,) = sqlx::query_as(&format!("PRAGMA {pragma}")).fetch_one(&pool).await.unwrap();
        println!("{pragma} = {}", v.0);
    }
}
```

- **`foreign_keys = 0`** → `F-VAL-035`'s cascade leg is Confirmed and the fix is a one-line connect option, applied to every service.
- **`foreign_keys = 1`** → that leg closes; the rest of the finding stands or falls on its own.
- `busy_timeout = 0` would additionally mean concurrent writers get `SQLITE_BUSY` immediately rather than waiting, which is the mechanism behind several "a transient SQLite error" triggers elsewhere in this audit — including `F-XC-002`'s.

---

## 13. Does `frost-core` reject a signing package whose commitments do not match the nonces?

**Unblocks:** `F-VAL-034` (40%). **Effort: 15 minutes.** _Raised by C-VAL-B, not in the original twenty-two._

`F-VAL-034` step 6 turns entirely on this: after a signing timeout rewrites a session, a late `Resume` can call `signature_share(key_share, nonces_from_session_1, revealed_from_session_2, m)`, and "`frost-core`'s own-commitment check is the only thing between this and a published share".

Build two independent signing sessions in a unit test (the crate's own `crates/validator/src/frost/mod.rs:24+` ceremony test is the template), then cross them: pass session 1's nonces with session 2's revealed commitments.

- **`Err`** → `F-VAL-034` stays Low: the effect logs a warning and the ceremony is unaffected.
- **`Ok`** → the validator queues an `Action::SignShare` whose `z` cannot satisfy onchain verification, wasting a transaction and, more importantly, revealing that nothing local guards the pairing. Re-score upward and take the finding's own remediation.

---

## 14. Can `round1::SigningNonces::new` panic on anything `NonceChunk::with_size` passes it?

**Unblocks:** `F-VAL-031` (42%). **Effort: 15 minutes.** _Raised by C-VAL-B._

C-VAL-B eliminated every other panic source in that path by inspection — the only `expect` is `offset.checked_add(1).expect("chunk too large")` with `offset < 1024` (`crates/validator/src/frost/preprocess.rs:117`), and `MerkleTree::build`/`proof` are total (`crates/validator/src/merkle.rs:21-22`, `:55`) — leaving `SigningNonces::new` as the sole unknown, class `I` under A6.

Note that no untrusted input reaches this path, so the answer caps `F-VAL-031` at Low either way. Its real value is the _other_ half of C-VAL-B's QA note, which needs no upstream knowledge and is the cheapest test in the validator crate: make a `Sampler::Custom` that panics (`crates/validator/src/secrets/nonces.rs:84-86` already exists for this), then assert that a subsequent `start` **replaces** the dead entry and that `next` succeeds. If it does not, the validator stops producing nonce chunks for the rest of the process lifetime after one worker death, with no metric distinguishing it.

---

## 15. Do `PrivateKeySigner` and k256's `SigningKey` zeroize on drop?

**Unblocks:** R3's Observation O4; raises or lowers **every** secret-at-rest severity in the report. **Effort: 30 minutes, source reading.**

Read, after `cargo fetch`: k256's `SigningKey` (does it hold a `SecretKey` with a `ZeroizeOnDrop` impl?), and alloy's `PrivateKeySigner`. Specifically whether `to_bytes` leaves an unzeroized intermediate — this codebase calls it, and a copy that outlives the source is what turns "the key is in memory" (unavoidable) into "the key is in freed memory" (avoidable).

Under A1 the operator and host are trusted, so no finding here becomes High on its own; the answer calibrates the severity language used across the secret-handling findings, which is why it is worth thirty minutes but not worth doing before anything above it.

---

## 16. Five `alloy` 2.0.5 specifics R1 could not read

**Unblocks:** `F-CORE-002` (85%), `F-CORE-010` (45%), `F-CORE-012` (70%), `F-CORE-065` (55%). **Effort: 45 minutes, source reading.**

1. How `Filter::at_block_hash` encodes — does it send `blockHash` or a `fromBlock`/`toBlock` pair?
2. Is `EthRpcErrorCode::ResourceNotFound` really `-32001`?
3. `logs_bloom` accumulation semantics on a reorged range.
4. Exactly when `SolEventInterface::decode_raw_log` fails (this overlaps question 2 and should be answered together with it).
5. Whether `ProviderCall::Ready` short-circuits every internal chain-id fetch, or only some.

Four findings share the 45 minutes, but none of them is bimodal the way questions 1 and 2 are — each answer moves a certainty by ten or twenty points rather than across severity bands, which is why this sits here rather than higher.

---

## 17. Which TLS root source does `reqwest` select under `default-features = false, features = ["json", "rustls"]`?

**Unblocks:** `F-XC-004` (85%), R10's Observation 3. **Effort: 15 minutes.**

`Cargo.toml:15` pins exactly those features. The question is whether the images' `ca-certificates` package is load-bearing or dead weight — i.e. whether `reqwest` uses the OS trust store or a bundled `webpki-roots`.

```sh
cargo tree -p reqwest -e features --locked | grep -i -E "rustls|webpki|native-tls|ca-cert"
```

Then confirm empirically: run the engine image with `ca-certificates` removed and see whether an HTTPS RPC still connects.

- **OS store** → `ca-certificates` is required, and removing it would be an outage. Document it.
- **`webpki-roots`** → the package is dead weight _and_ certificate roots are pinned at build time, which is a supply-chain property the operator should know about (roots update only on rebuild).

---

## 18. `reqwest` 0.13.4's default redirect and proxy policy

**Unblocks:** `F-XC-008` (80%) items 2 and 3. **Effort: 15 minutes.**

Two behaviours, both currently `I`: how many redirects a default client follows, and whether it honours `HTTP_PROXY`/`HTTPS_PROXY` from the environment. Test against a local listener that returns `301` in a loop, and against a client started with the proxy variables set to a listener that logs connections.

Under A1 and A3 both are Low — they need an environment or configuration change by the trusted operator — which is why this is below question 4 despite touching the same finding. Note the Critic's caution: do **not** hard-code `no_proxy` as the fix; an operator legitimately behind a corporate proxy would lose all CoW lookups.

---

## 19. Does the linker strip the unused `sqlx-mysql` / `sqlx-postgres` code from the release binaries?

**Unblocks:** `F-XC-007` (84%) item 2. **Effort: 30 minutes.**

`Cargo.toml:19` takes `sqlx` without `default-features = false`, so `Cargo.lock:4598-4609` pulls `sqlx-mysql` and `sqlx-postgres` into the build. Whether they reach the shipped binary decides whether remediation 2 is a real surface reduction or only a build-time saving.

```sh
cargo build --release --workspace --locked
nm -C target/release/validator | grep -c -i "sqlx_mysql\|sqlx_postgres"
ls -l target/release/{validator,sentinel,sentinel-engine}
# then apply remediation 2 and compare both numbers
```

C-XC already cleared the prerequisite: `grep -rn "FromRow\|derive(sqlx" crates/ --include='*.rs'` returns no match, so nothing uses the `macros` feature for derives and the change has no known code dependency. It still needs a build to confirm.

---

## _Diminishing returns begin here._ The remaining questions need something a toolchain does not provide.

### 20. Are the eight MultiSend deployment addresses correct and complete?

**Unblocks:** `F-ENG-006` (80%), `F-ENG-035` (80%), `F-ENG-037` (84%), `F-XC-052` (85%). **Effort: 30 minutes and network access. Needs no toolchain — and the contracts are not in this checkout** (R9: "`Safe.sol` is not in this checkout").

`crates/sentinel-engine/src/contracts/multi_send.rs:27-68` hard-codes eight canonical deployments with a `Legacy`/`V150Plus` wire-format tag and an `allows_delegate_calls` flag each. **A wrong or missing address means a batch is silently not recognised as a batch and every sub-call check is skipped** — `decode_multi_send_call` returns `None` and `sub_transactions` falls back to `vec![tx.clone]` (`multi_send.rs:165-171`), so the transaction is checked as an opaque delegatecall rather than as its constituent calls. That is a security-relevant miss, and it is silent by construction.

Check each of the eight against Safe's own deployment registry (`safe-global/safe-deployments`) for the target chain, and check the list for **omissions** as carefully as for errors:

| Address                                      | Tagged   | Delegate calls |
| -------------------------------------------- | -------- | -------------- |
| `0x218543288004CD07832472D464648173c77D7eB7` | V150Plus | allowed        |
| `0xA83c336B20401Af773B6219BA5027174338D1836` | V150Plus | not allowed    |
| `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` | Legacy   | allowed        |
| `0x9641d764fc13c8B624c04430C7356C1C7C8102e2` | Legacy   | not allowed    |
| `0xA238CBeb142c10Ef7Ad8442C6D1f9E89e07e7761` | Legacy   | allowed        |
| `0x40A2aCCbd92BCA938b02010E17A5b8929b49130D` | Legacy   | not allowed    |
| `0x998739BFdAAdde7C933B942a68053933098f9EDa` | Legacy   | allowed        |
| `0xA1dabEF33b3B82c7814B6D82A79e50F4AC44102B` | Legacy   | not allowed    |

Three things to verify per row: that the address is a real MultiSend deployment on the deployment chain; that the `MultiSendCallOnly` variants are the ones tagged `allows_delegate_calls: false` and no other; and that the `Legacy` / `V150Plus` split matches where `to == address(0)` began to mean a self-call. A missing deployment (a chain-specific or newer release) is the failure mode that produces no error anywhere.

### 21. CoW and Safe contract semantics

**Unblocks:** `F-ENG-031` (85%), `F-ENG-037` (84%), `F-ENG-038` (78%). **Effort: hours; needs the contracts, which are not in this checkout.**

`GPv2Signing.setPreSignature`, `GPv2VaultRelayer`, `ComposableCoW.createWithContext`, and Safe's `handlePayment` arithmetic. Under A7 the Solidity is the reference, so a Rust/Solidity mismatch is a Rust finding — but the Solidity has to be obtainable before the comparison can be made. Fetch the deployed sources for the target chain and re-derive each of the three findings' encoding claims against them.

### 22. Is the HKDF reference vector reproducible? — ✅ **ANSWERED: yes**

**This one needed no toolchain and has been settled.** C-CORE-B flagged that the reviewer declined to re-derive the vector at `crates/core/src/kdf.rs:36-46`; `python3` is present, and the vector's own doc comment says Python produced it originally.

```sh
python3 rust-audit/poc/F-CORE-038/hkdf_reference_vector.py
# de66ad87d39718318f7ec36177e9e2286b5c0ade3dc0de22b65e9ee55ccaab0d
# True
```

RFC 5869 HKDF-SHA256 with `salt = b"safenet-sentinel-reveal-salt"`, `ikm = b"top secret key material"`, `info = b"request-1"`, `L = 32` reproduces the literal exactly. Script and output are saved under `rust-audit/poc/F-CORE-038/`.

**What it proves:** the vector is arithmetically correct and independently derived, so it is not a value copied from a previous run of the code under test. C-CORE-B's concern is closed. **What it does not prove:** that the Rust produces it — that still needs `cargo test -p safenet-core kdf::tests::derive_key_matches_reference_vector`, which is an ordinary green-suite check rather than an open question.

**Do not repeat this one.** It is answered; the artefact is on disk.

### 23. Can the `sentinel-test-vectors` corpus be cloned and run?

**Unblocks:** every `F-ENG-*` finding. **Effort: hours, and it needs a decision, not a command.**

Assumption **A8** is still "TEAM TO CONFIRM": the corpus is not available locally and nobody decided whether QA may clone it. This is the only route by which any checker finding reaches `E1`, so it is the single largest structural gap in the audit — but it is a permissions question first and a technical one second.

```sh
just test-integration-sentinel-engine <path-to-corpus>
```

### Q-ENG-A. Does `alloy::transports::mock::Asserter` expose a queue-emptiness accessor?

Restored by its author, QA-ENG. **Full entry is in the "Additional sections" part below** — see `## Q-ENG-A.` there for the reasoning, the two-minute check, the exact three lines to delete if the accessor does not exist, and why nothing is lost if it goes.

## What is _not_ on this list, and why

- **Anything about a specific package's security posture.** See the disclaimer at the top.
- **`F-XC-004` (images run as root, no digest pin, discarded provenance argument), `F-XC-006` (nothing binds a deployment to a chain), `F-XC-009` (sample-config placeholders).** These are Confirmed on direct reads of files in this checkout and are not blocked on anything. They need a decision, not an experiment. The suggested CI gate for `F-XC-004` — `docker inspect` asserting a non-root `User` and a non-empty revision label — is the right shape and needs no answer from this list.

---

# Additional sections

---

## Standing note on all ten engine PoCs

`rust-audit/poc/F-ENG-{002,030,031,032,033,034,035,036,037,044}/` contain Rust that has **never been compiled or run**. Each README says so at the top and states, per test, what a pass and a fail mean.

They are written as in-crate `#[cfg(test)]` modules appended to the file they exercise, because `sentinel-engine` is a **binary-only crate** — `src/main.rs`, no `src/lib.rs`, no `[lib]` section in `crates/sentinel-engine/Cargo.toml` — and therefore has **no library for a `tests/*.rs` integration test to import**. This is not a dependency question, but it is the single most useful fact for anyone planning engine test infrastructure, and it was discovered while writing these PoCs: adding a thin `src/lib.rs` re-exporting `checkers`, `engine` and `contracts` would let all ten become ordinary integration tests.

---

## Questions that are **not** answerable by reading a dependency

> **Merge instruction:** add as its own section near the end of the shared file, below the _"diminishing returns"_ line. These are recorded here specifically so they are not mistaken for crate questions and handed to whoever is doing the registry reads — they need a **node** or a **deployed engine**, and no amount of `~/.cargo/registry` will settle either.
>
> Two finding QA sections cite this section by name; after the merge they should point at wherever it lands. Until then they cite `rust-audit/../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md` §2.

### 2a. `F-CORE-061` — what does a node say when it rejects a **first** submission on fee grounds?

`is_transaction_underpriced` (`crates/core/src/tx/mod.rs:359-365`) matches two patterns, both of which require the rejection to be about a _replacement_: `"replacement transaction"` **and** `"underpriced"` together, or the single vendor sentence `"INTERNAL_ERROR: could not replace existing tx"`. A node that rejects a **first** submission because its fee is below the pool's own floor produces neither, so the rejection falls into the generic branch, which deliberately records no fee floor — and the row is then rebuilt with `bump(fresh, None)`, i.e. the same fee, forever, holding an allocated nonce that blocks every later one.

The pivotal fact is a **string**, and it is a client's string, not a crate's. Settle it against:

- geth's `core/txpool` error set (`ErrUnderpriced`, `ErrReplaceUnderpriced`, `ErrTxGasPriceTooLow`) and how `internal/ethapi` renders each over JSON-RPC;
- Nethermind's `AcceptTxResult` / `TxPool` rejection reasons;
- Erigon's equivalents;
- or, fastest and most trustworthy: a testnet node with a deliberately underpriced first submission, capturing the literal `error.message` and `error.code`.

**Record which client and version produced each string**, because the answer is version-specific and the current two regexes are already vendor-shaped.

C-CORE's section on F-CORE-061 explicitly declined to upgrade the finding on a recollection of geth's `ErrUnderpriced` wording, which was the right call and which this entry preserves.

**What each answer implies.** If a common client's first-submission rejection is unmatched, the finding's trigger becomes `E2` and its certainty rises from 58% toward the `E2` band. If every client in scope happens to include the word "replacement", the finding weakens sharply. **Either way, F-CORE-061's remediation options 1 and 3 must not ship before F-CORE-060's ceiling** — both widen the set of rejections that feed an unbounded fee ratchet.

### 2b. `F-SEN-015` variant 1 — does the sentinel engine re-decide a proposal the same way?

Variant 1 of F-SEN-015 (a replayed engine check returns a _different_ verdict, so the stored `reason` no longer matches the onchain `commitHash` and the reveal reverts `InvalidReveal`) rests on the engine being non-deterministic across a restart — a rule list updated during a deploy, a checker reading chain state at head rather than at `Effect::EngineCheck.block`, or simply a different rule id for the same violation. That is a property of the **deployed engine**, not of any crate, and it cannot be assessed from the `sentinel` crate at all.

**Do not spend registry time on this.** Instead run **variant 2** of the PoC at `rust-audit/poc/F-SEN-015/poc_service.rs` (`poc_f_sen_015_unknown_verdict_on_replay_drops_an_already_committed_request`), which needs no determinism assumption whatsoever: it turns on `handle_engine_check_result` removing the entry at `crates/sentinel/src/service.rs:156` _before_ inspecting the outcome and never re-inserting it on `CheckOutcome::Unknown` (`:176-179`) — the overwhelmingly likely case on a restart under A3, where the engine is co-deployed and still booting. Variant 2 alone establishes the loss.

If someone does want variant 1 settled, the check is: run the same proposal through the deployed engine twice across a restart and diff `(approve, reason)`. That is an engine-team question and belongs on their list, not on the dependency list.

---

---

# Validator-specific questions (QA-VAL)

## VAL-Q1 — `frost_core::Identifier`: how does one get the scalar out?

**Blocks:** `poc/F-VAL-001/poc.rs` (`identifier_scalar`), and any future Lagrange work.

**Question.** `poc.rs` converts an `Identifier` to a `k256::Scalar` via `Identifier::serialize`, written against `AsRef<[u8]>` so it compiles whether that returns `Vec<u8>` or the ciphersuite `Serialization` type. Which is it in 3.0.0, and is `Identifier::to_scalar` public (the validator enables `frost-core`'s `internals` feature, `crates/validator/Cargo.toml:11`)?

**Exact check.** `frost-core-3.0.0/src/identifier.rs` — the signature of `pub fn serialize`, and whether `to_scalar` is `pub` or `pub(crate)`. Thirty seconds.

**If `to_scalar` is public**, replace `identifier_scalar`'s body with `frost_core::Identifier::to_scalar(id)` and delete the length assertion.

---

## VAL-Q2 — the FROST(secp256k1, SHA-256) challenge `H2`

**Blocks:** `poc/F-VAL-033/nonce_reuse.rs` (`challenge`), i.e. the key-recovery arithmetic.

**Question.** The PoC recomputes `c = H2(SerializeElement(R) ‖ SerializeElement(PK) ‖ msg)` as `hash_to_field::<ExpandMsgXmd<Sha256>, Scalar>` with DST `"FROST-secp256k1-SHA256-v1" ‖ "chal"`, mirroring `frost::ecdh::hash_to_scalar`'s use of the same split DST with `"enc"` (`crates/validator/src/frost/ecdh.rs:123-132`) and `participants.rs`'s documented `"id"`. Is that the ciphersuite's actual `H2`?

**Exact check.** `frost-secp256k1-3.0.0/src/lib.rs` — the `H2` implementation on the `Secp256K1Sha256` ciphersuite; confirm the context string and the discriminant. Alternatively check whether `frost_core::challenge` and `Challenge::to_scalar` are public under `internals`, in which case use them directly.

**Self-answering.** `signing_round` asserts `z_aggregate·G == R + c·PK` on public data before using `c`, with a failure message naming this question. So the PoC tells you the answer when you run it — this entry only tells you where to look if it says no.

---

## VAL-Q3 — does `sqlx` 0.9 enable `PRAGMA foreign_keys` by default?

**Blocks:** **F-VAL-035** claim (c) (class `I` there), and `poc/F-VAL-005-066/secrets_reconciliation.rs::reconciliation_cascades_away_a_committed_nonce_chunk`.

**Question.** `nonces` rows are removed only by the `ON DELETE CASCADE` from `nonces_chunks` (`crates/validator/src/secrets/store.rs:80-87`). SQLite honours that only with `PRAGMA foreign_keys = ON`, which is **off** in SQLite's own default. `safenet_core::utils::connect_sqlite` sets only pool timeouts (`crates/core/src/utils.rs:56-62`) and the options come from a TOML URL (`crates/validator/src/config.rs:29`). No pragma is set anywhere in the workspace.

**Exact check, no source needed** — this is the one C-VAL-B already identified as answerable by a test, and it is now written:

```rust
// crates/validator/src/secrets/store.rs, tests module
#[tokio::test]
async fn retain_nonces_cascades {
    let store = store.await;
    let root = store.register_nonces_chunk(GROUP, ME, nonce_chunk(4)).await.unwrap();
    store.retain_nonces([]).await.unwrap();
    assert_eq!(count_root_nonces(&store, root).await, 0);
}
```

**If it fails**, retired groups leave their complete nonce inventory on disk with no chunk row pointing at it, so no later `retain_nonces` can ever find it — F-VAL-035 (c) becomes a live defect rather than an upgrade hazard, and F-VAL-066's nonce-cascade consequence changes shape (orphaned rather than deleted).

**Either way, set the pragma explicitly.** The value of the answer is bounded; the value of not depending on it is not.

---

## VAL-Q4 — does `sqlx`'s SQLite pool have a busy timeout, and what is it?

**Blocks:** **F-VAL-038** (the step from "contention" to "the validator exits" is explicitly an inference), **F-VAL-004** trigger A (how likely is an `SQLITE_BUSY` from `store_keygen_secrets`), **F-VAL-066** (whether the delete/insert interleaving is a retry race or a queue).

**Exact check.** `sqlx-sqlite-0.9.*/src/options/mod.rs` — the `Default` impl for `SqliteConnectOptions`: the `busy_timeout` field's default, and whether `journal_mode` defaults to WAL. Then `SqlitePoolOptions`' default `max_connections`.

**Or measure it.** With a toolchain, the honest answer is a benchmark, not a source read:

```rust
// time `register_nonces_chunk` with a real 1024-nonce chunk while a second
// task commits snapshots in a loop on the same pool
```

F-VAL-038's whole claim is about duration; a source read gives the timeout but not the transaction length, and it is the ratio that matters.

---

## VAL-Q5 — does `frost-core`'s `Debug` redact `SigningShare`, `SecretPackage` and `KeyPackage`?

**Blocks:** **F-VAL-062**'s central claim (class `I` today).

**Exact check — no source needed, and it is the cheapest check in the audit.** Run `poc/F-VAL-062/debug_redaction.rs`. `KeyShare::dummy`'s signing share is `k256::Scalar::ONE` (`crates/validator/src/frost/keygen.rs:446-452`), so a `format!("{:?}", …)` answers it directly. See that directory's README for how to read the result — **a pass partly refutes the finding**, and that is a result worth recording.

If you want the source anyway: `frost-core-3.0.0/src/keys.rs`, the `Debug` impls for `SigningShare` and `SecretShare`, and `frost-core-3.0.0/src/keys/dkg.rs` for `round1::SecretPackage`.

---

## VAL-Q6 — does `round2::sign` verify that the supplied `SigningNonces` match the signing package?

**Blocks:** **F-VAL-034** (the whole _outcome_; the missing local check is `E2`, the consequence is `I`).

**Question.** `handle_nonces` applies a nonce resume to whatever session currently holds the message, without checking the signature id (`crates/validator/src/state/sign.rs:359-404`). Because `core::state` states that resume ordering is undefined and effects may run more than once (`crates/core/src/state/mod.rs:44-64`), a resume from a ceremony that has since restarted can land on the restarted one. The only thing preventing a share computed from a stale nonce is `frost-core`'s own commitment check.

**Exact check.** `frost-core-3.0.0/src/round2.rs` — does `sign` (or `SigningPackage`'s accessors it calls) compare `signer_nonces.commitments` against the package's entry for this signer, and what error does it return? If it does, F-VAL-034's outcome is "a warning and no share", which is the benign branch and the reason C-VAL-B floored it at 40.

**Do not leave it as a dependency.** Whatever the answer, `handle_nonces` should check the signature id itself; that is F-VAL-034's remediation and it removes the question.

---

## VAL-Q7 — `k256::Scalar` inherent items

**Blocks:** nothing substantive; a possible mechanical fix in three PoC files.

**Question.** The PoCs use `Scalar::ZERO`, `Scalar::ONE`, `Scalar::invert` and `Scalar::from_repr`, importing `elliptic_curve::{Field, PrimeField}` under `#[allow(unused_imports)]` in case they are inherent. `crates/validator/src/frost/ecdh.rs:124` uses `Scalar::ZERO` with no `Field` import, so at least `ZERO` is inherent.

**Exact check.** `k256-0.13.4/src/arithmetic/scalar.rs` — the inherent `impl Scalar` block. If the imports are redundant, drop the `allow` and the `use`; if `invert` is not inherent, keep them.

---

## VAL-Q8 — is the x-coordinate of an ECDH point close to uniform?

**Blocks:** **F-VAL-002** basis row 9 (class `I`).

**Question.** The pad is `x(sk · Q)` used directly as a one-time pad (`crates/validator/src/frost/ecdh.rs:110-121`). About half of all 256-bit values are valid secp256k1 x-coordinates, so the pad is measurably non-uniform.

**Not a dependency question at all** — it is a property of the curve, and the fix (F-VAL-002's KDF) removes it regardless. Listed here only so it is not mistaken for one. The check, if anyone wants a number, is a histogram over `hash_to_scalar`-derived keys, but it will not change the remediation: option 1 of F-VAL-001 (`HKDF-SHA256` over the shared secret, bound to `(gid, sender, recipient)`) fixes the bias, the two-time pad and the possession gap in one change.

---

## VAL-Q9 — `rayon`'s global pool sizing under the shipped deployment

**Blocks:** **F-VAL-038** basis row 2's escalation.

**Question.** `NonceChunk::with_size` fans 1024 nonce generations across `rayon`'s global pool (`crates/validator/src/frost/preprocess.rs:112-131`), from inside a dedicated worker thread that is itself one per group. How many cores does that occupy, and does it starve the tokio runtime?

**Exact check.** Not a source read — a measurement, on the single-core configuration `docs/validator-handbook.md` describes. Run the validator's nonce generation under `taskset -c 0` and watch block-processing latency. `RAYON_NUM_THREADS` is the mitigation to test against.

---

## Answers appended by V-VAL (Phase 5) — questions 1, 12 and 13

Scoped detail, test sources and full output: `../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md` (VAL-Q1 through VAL-Q9) and `V-VAL-dependency-questions/`. Summarised here so the shared list is not stale.

**Question 1 — does `frost-core` 3.0.0 redact secrets in `Debug`? — ✅ ANSWERED: yes, it redacts.** `SigningShare` → `SigningShare("<redacted>")` (`frost-core-3.0.0/src/keys.rs:126-133`); `dkg::round1::SecretPackage` → `coefficients: "<redacted>"` (`src/keys/dkg.rs:191-204`); `dkg::round2::SecretPackage` → `secret_share: "<redacted>"` (`:337-350`). Confirmed by executing `poc/F-VAL-062/debug_redaction.rs`, whose verbatim output is quoted in F-VAL-062's verification section. **No secret reaches any log sink today.** This refutes the leak claim in **F-VAL-062**, **F-XC-002** and **F-CORE-036**, all three of which correctly wrote it as class `I`; all three are reduced, and the surviving hygiene claim (the safety is an upstream detail no test here pins) keeps its remediation. _Trap for anyone re-checking:_ `KeyShare::dummy` sets the signing share to `Scalar::ONE` and the identifier to `Identifier::try_from(1)`, so `0000…0001` appears in the rendering as the public **identifier**; a naive substring search reports a leak that is not there.

**Question 12 — `sqlx` 0.9 SQLite defaults — ✅ ANSWERED.** Against a pool built exactly as `main.rs:46` builds one: `foreign_keys = 1`, `journal_mode = delete`, `synchronous = 2`, `busy_timeout = 5000`, `page_size = 4096`, `locking_mode = normal`, pool `max_connections = 10`. `sqlx-sqlite` sets `foreign_keys = ON` itself in `SqliteConnectOptions::new` (`sqlx-sqlite-0.9.0/src/options/mod.rs:185-187`), against SQLite's own default. So per the entry's own decision rule, **`F-VAL-035`'s cascade leg closes** — and the cascade was additionally observed firing on the shipped schema. Two riders: `busy_timeout = 5000` means a competing writer waits five seconds rather than erroring immediately, which _raises_ the bar for every "a transient SQLite error" trigger in this audit (**F-VAL-004** trigger A, **F-XC-002**, **F-VAL-066**); and WAL is **not** enabled, so a writer blocks readers outright, which lowers the bar for **F-VAL-038**'s contention mechanism.

**Question 13 — does `frost-core` reject a signing package whose commitments do not match the nonces? — ✅ ANSWERED: yes.** `round2.rs:140-143` returns `Error::IncorrectCommitment` before any use of the nonce. Executed: `stale-nonce signature_share -> Err(Unexpected(IncorrectCommitment))`. **F-VAL-034**'s outcome is therefore a warning and no share, never a share over a reused nonce; it cannot escalate and must not be conflated with **F-VAL-033**.

**Also closed in passing:** `frost_core::Identifier::serialize` returns `Vec<u8>` (`identifier.rs:65`); `k256::Scalar::{ZERO, ONE, invert}` are inherent, `from_repr` is `PrimeField`.

**Correction that applied to every PoC README in this audit (now fixed in place):** the commands originally said `cargo test -p validator --lib …`, which fails with `error: no library targets found in package 'validator'` because `validator`, `sentinel` and `sentinel-engine` are binary-only crates. They now use **`--bins`**. `safenet-core` does have a `lib.rs`, so `-p safenet-core --lib` is correct and was left alone.
