# rv-xc: re-validation log (run 3, cross-cutting rows)

| Field | Value |
| --- | --- |
| Agent | rv-xc (auditor department, re-validation track) |
| Audit commit | `361aaf3` |
| Worktree | `<run folder>/wt/rv-xc` (clean at the end: `git status --short` empty); target dir `<run folder>/target-rv-xc` |
| Time | 2026-10-08, about 16:45Z to 17:55Z |
| Rows | 16 of 16 done: 1 Fixed, 2 Changed, 13 Still valid; none left unfinished |
| Output | 16 appended sections in `rust-audit/findings/`, `rust-audit/state/run3/revalidation/rv-xc.md`, `poc/<id>/run3/` for 12 PoC folders, this log, and the department report `<run folder>/reports/rv-xc.md` |

`<run folder>` is the local run folder. Every build and test ran after `source <run folder>/env/rv-xc.sh`, offline, in the worktree. Temporary test hunks were pasted with `poc/F2-XC-050/paste.py` or appended, run, and reverted with `git checkout -- <file>` after `git diff -U0` showed one added hunk.

## Rows

| ID | Verdict | What was run | Output |
| --- | --- | --- | --- |
| F2-XC-001 | Fixed (#1046) | `run.sh` (18 config variants and a control against both binaries) plus 5 key-in-the-wrong-field variants; mutation with `config.rs:37` commented out (leaks in every in-file variant), reverted and re-run; span fuzz of `parse` (213,312 documents); in-tree `config::tests`; RPC-URL residual checks | `poc/F2-XC-001/run3/` |
| F2-XC-050 | Still valid | adapted `qa2_xc_050` (3 tests) in `core/src/state/mod.rs` | `poc/F2-XC-050/run3/` |
| F-XC-006 | Still valid | adapted `qa2_xc_006` with two Anvil 1.5.1 nodes on ports 31600 (chain 100) and 31601 (chain 11155111), default timings | `poc/F2-XC-006/run3/` |
| F-XC-001 | Still valid | adapted `qa2_xc_009` in `core/src/index/blocks.rs`, dev profile and release profile (`run-bg r3-rv-xc-xc009-release`, 1 min 50 s build) | `poc/F2-XC-009/run3/` |
| F2-XC-002 | Still valid | `run.sh` with `../stub.py` on port 31602, both binaries, bare and `?mode=rwc` paths, `${NAME}` route | `poc/F2-XC-002/run3/` |
| F2-XC-005 | Changed | adapted `qa2_xc_005` (3 tests, one new: taken signing nonce) in `validator/src/secrets/store.rs` | `poc/F2-XC-005/run3/` |
| F-XC-051 | Still valid | run-1 PoC adapted to the PoP-first `verify_commitment` (6 tests, first execution ever) appended to `validator/src/frost/keygen.rs` | `poc/F-XC-051/run3/` |
| F-XC-050 | Still valid | code trace (no fixture exists for `CollectingConfirmations`); consequence executed via F-XC-051's run | none of its own |
| F-XC-003 | Still valid | run-1 tests pasted unchanged into both `config.rs` (6 pass); 3 binary variants for new keys | `poc/F-XC-003/run3/` |
| F-XC-002 | Still valid | redaction test adapted to `Effect::GenerateNonces`, `Resume::Setup`, `Resume::Nonces` in `validator/src/service/effect.rs` | `poc/F-XC-002/run3/` |
| F-XC-011 | Still valid | `tree.sh` (16 `cargo tree -i` queries, feature queries, exporter source grep) against `state/run3/logs/05-audit.log` | `poc/F2-XC-004/run3/` |
| F-XC-007 | Still valid | workflow and Justfile greps; `tree.sh` | `poc/F2-XC-004/run3/` |
| F-XC-004 | Still valid | `git log` on Dockerfiles and `docker.yml`, greps | none (E2) |
| F-XC-009 | Still valid | sample-loading test in both `config.rs` (prints the signer address of the shipped samples) | `poc/F-XC-009/run3/` |
| F-XC-008 | Still valid | redirect test (307 to another loopback host) and `HTTP_PROXY` test (port 31605) in `sentinel/src/engine.rs` | `poc/F-XC-008/run3/` |
| F2-XC-007 | Changed | code trace | none (E2) |

## Commands (representative)

- `cargo build -p validator -p sentinel --bins --locked --offline` (warm cache, 25 s; rebuilt after each mutation and revert).
- `REPO=$WT bash rust-audit/poc/F2-XC-001/run3/run.sh $CARGO_TARGET_DIR/debug <scratch>`.
- `cargo test --offline --locked -p safenet-core --lib config::tests::rv_xc_span_fuzz -- --nocapture`.
- `QA2_XC_DIR=<scratch> cargo test --offline --locked -p safenet-core --lib state::tests::qa2_xc_050 -- --nocapture --test-threads=1`.
- `bash rust-audit/poc/F2-XC-006/run3/run.sh <scratch>` (starts and kills its own Anvil nodes after an `ss -ltnp` pid check).
- `cargo test --offline --locked [--profile release] -p safenet-core --lib index::blocks::tests::qa2_xc_009 [-v] -- --nocapture`.
- `QA2_XC_DIR=<scratch> cargo test --offline --locked -p validator --bins secrets::store::tests::qa2_xc_005 -- --nocapture --test-threads=1`.
- `cargo test --offline --locked -p validator --bins frost::keygen::qa_xc_051 -- --nocapture --test-threads=1`.
- `cargo test --offline --locked -p {validator,sentinel} --bins config::tests::qa_xc_003` and `config::tests::rv_xc_009`.
- `cargo test --offline --locked -p validator --bins service::effect::rv_xc_debug_redaction -- --nocapture`.
- `[HTTP_PROXY=http://127.0.0.1:31605] cargo test --offline --locked -p sentinel --bins engine::tests::rv_xc_008::…`.
- `bash rust-audit/poc/F2-XC-004/run3/tree.sh`.

## Process notes

- No Anvil or stub is left running (`pgrep -a anvil` empty, no listener on 31600 to 31609).
- The release test build left `<run folder>/target-rv-xc/release` behind.
- A `pgrep -af stub.py` check matched its own command line once; the port check (`ss -ltn`) is the reliable one.
- Not opened: `findings/F3-*`, `state/run3/agents/R*`, `state/run3/agents/L-*`.
