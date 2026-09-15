# Run 2 — shared brief for Reviewers

Repository root: `/home/shebin.guest/safe/safenet`. The commit you audit is recorded in `state/run2/STATE.md` (field **Commit**) — read it first and cite that hash.

## 1. Read before starting

`PROMPT.md` Sections 1 (boundaries), 2 (evidence discipline), 6 (your role), 8 (finding template, certainty rubric, severity scale) and **11 (run-2 addendum)**; `codebase-map.md` Sections 1, 5, your crate's subsection of 6, and 8; the analysis file for your crate under `analysis/`; and `state/run2/baseline.md` (executed baseline, inventory, the reviewer split you were assigned from).

## 2. Independence — the rule that makes run 2 worth doing

This is a second, independent audit. **Do not open** `findings/F-*.md`, anything under `report/`, or any `state/*.md` outside `state/run2/`. Do not grep them. If you notice a run-1 file, close it. The codebase map and analyses are leads to confirm or refute, exactly as in run 1 — never findings. Your value is an opinion formed from the code, not from the previous run's conclusions; reconciliation happens later, by a different agent, reading both.

## 3. Scope and assumptions

- In scope: `crates/core`, `crates/validator`, `crates/sentinel`, the workspace `Cargo.toml`/`Cargo.lock`, those crates' `Cargo.toml`, `Dockerfile` and `*.sample.toml`. **`crates/sentinel-engine` is out of scope** — do not read it, do not file against it, and treat the engine as an opaque HTTP oracle from the sentinel's side. Solidity under `contracts/src` is reference only (A7): a Rust/Solidity mismatch is a Rust finding.
- Assumptions A1–A17 in `PROMPT.md` Section 3 are confirmed. Two are new and shape severity:
  - **A16** — genesis need not be recoverable; the operator watches it and redeploys on abnormal participation. A defect whose **only** impact is genesis-ceremony liveness is Informational, tagged `known`. A defect reaching later epochs (rollover key generations, an epoch-1 group) keeps its measured severity.
  - **A17** — only the services touch their databases. A trigger that needs out-of-band database access (restore, copy to another chain, manual edit) is out of scope. Service-caused crash-consistency or reorg corruption stays in.
- The code has changed since the map was written: the sentinel gained verdict aggregation, meta-transactions and waiting-for-outcome states; the validator gained scheduled secret pruning (`secrets/store.rs`, `service/effect.rs`, `core/effects.rs`, `core/driver.rs`). Map line numbers will be off — re-anchor every citation to the audited commit.

## 4. Environment — executed evidence is reachable this run

Start **every** bash call with `export PATH="$HOME/.foundry/bin:$HOME/.cargo/bin:$PATH" CARGO_TARGET_DIR=/home/shebin.guest/.cache/safenet-run2/target CARGO_BUILD_JOBS=2`. cargo/rustc 1.98.1, Foundry 1.8.1, `just` 1.40.0 are present; `cargo test -p <crate>` and targeted tests are allowed and encouraged when a claim can be checked cheaply. `sentinel` and `validator` are **binary-only crates** — use `--bins`, not `--lib`; `safenet-core` has a `lib.rs`.

- **`/tmp` is a ~6 GB RAM-backed tmpfs shared with other agents. Never build or write large output there.**
- Run commands in the **foreground** with a long timeout (up to 600000 ms). Do not background a build and end your turn — nothing wakes you when it finishes.
- **Do not edit tracked files** — not even temporarily. Reviewers read and run existing tests; writing PoC tests is QA's job in Phase 3. Never commit, branch, stash, push or fetch.
- No network beyond the Cargo registry. **No live RPC.** If you run anything against a chain, it is local Anvil only, on a port from the range assigned in your launch message. All three `*.sample.toml` ship `rpc = "https://rpc.gnosischain.com"` — **live Gnosis mainnet** — so never start a service from an unmodified sample; copy it, rewrite `rpc` to loopback, and print the effective value into your log. Kill only PIDs you started.

## 5. What to produce

- One file per defect, **the moment it is drafted**: `findings/F2-<CRATE>-<nnn>.md`, `CRATE` in `CORE`, `VAL`, `SEN`, `XC`, using your assigned ID range and the exact Section 8 template. Every claim row cites `path:line-range` at the audited commit with a verbatim quote (≤15 lines) and a class: `E1` only for something you executed this session, `E2` for cited code plus a concrete trigger, `I` otherwise. A self-estimated certainty in the Trail; the Critic sets the real one.
- A coverage log `state/run2/agents/<you>.md`: every assigned file with its line count and the percentage you actually read (tests included), every command you ran, every hypothesis you considered and **rejected with the citation that refutes it**, and one observation entry per plausible-but-unproven concern. The rejected list is mined later — it is where a checker looks for things you talked yourself out of.
- Priorities, in order: consensus-critical correctness, secret handling, reorg and crash consistency, input validation at trust boundaries, panics reachable from untrusted input, resource exhaustion, dependency advisories (the `cargo audit` output is in `state/run2/logs/`; assess **reachability**, never repeat an advisory's CVSS as severity).

## 6. Reporting back

At most ten lines: paths written, counts, blockers. Never paste finding text into chat. If you could not finish a file, say which and how far you got.
