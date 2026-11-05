# Run 3 re-validation: rv-xc (cross-cutting rows)

Audit commit `361aaf3`. One row per canonical finding; the full reasoning is the `## Run 3 re-validation (rv-xc, 2026-10-08)` section at the end of each finding file. Verdicts: 1 Fixed, 2 Changed, 13 Still valid.

| ID | Ledger severity | Verdict | Proposed severity | Certainty | Changed by | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| F-XC-001 | Informational | Still valid | Informational (unchanged) | 93 | nothing added a profile; #1038 removed one of the two `debug_assert`s; #1010 renamed the block time field | E1 `poc/F2-XC-009/run3/` (dev panic at `blocks.rs:598`, release wrap to 2384) |
| F-XC-002 | Low (union, not counted) | Still valid | Low (unchanged; halves `F-VAL-062`, `F-CORE-036`) | 88 | Direct Nonce moved the key share to `Effect::GenerateNonces`; `Secrets`/`KeyShare` still derive `Debug` | E1 `poc/F-XC-002/run3/` (redaction holds; hygiene gap stands) |
| F-XC-003 | Informational | Still valid | Informational (unchanged) | 96 | nothing; still no unknown-field test, `driver::Config` still without `deny_unknown_fields` | E1 `poc/F-XC-003/run3/` (6 run-1 tests pass unchanged, 3 new-key variants rejected) |
| F-XC-004 | Low | Still valid | Low (unchanged) | 85 | nothing; Dockerfiles and `docker.yml` untouched since `fe9e84c` | E2 greps |
| F-XC-006 | Low | Still valid | Low (unchanged) | 90 | nothing binds a store to its chain; new tables (#993, #1033, #978) carry no chain id | E1 `poc/F2-XC-006/run3/` (two Anvil chains, default timings) |
| F-XC-007 | Informational | Still valid | Informational (unchanged) | 92 | no gate added; workflows moved to `safe-research/actions` (#976) | E1 run-3 `cargo audit` log (exit 1) + `poc/F2-XC-004/run3/tree-361aaf3.txt`; E2 greps |
| F-XC-008 | Informational | Still valid | Informational (unchanged) | 90 (was 80) | nothing; engine client still `reqwest::Client::new()` | E1 `poc/F-XC-008/run3/` (307 redirect re-posts the transaction; `HTTP_PROXY` honoured) |
| F-XC-009 | Low | Still valid | Low (unchanged) | 75 | #1007 added a `${SIGNER_PRIVATE_KEY}` comment but kept the active key `1`; #1062 changed `voting_window` | E1 `poc/F-XC-009/run3/` (both samples load, signer `0x7E5F…5Bdf`) |
| F-XC-011 | Informational | Still valid | Informational (unchanged) | 90 | nothing bumped a flagged crate; same 5 vulnerabilities and 11 warnings as run 2 | E1 run-3 `cargo audit` log + `poc/F2-XC-004/run3/tree-361aaf3.txt` |
| F-XC-050 | Medium (rollover) / Informational (genesis) | Still valid | unchanged | 48 | #1038 removed the nonce-tree half of the silent `None` finalisation; precondition `F-VAL-060` unchanged | E2 trace; commitment-round consequence executed at the FROST layer in `poc/F-XC-051/run3/` |
| F-XC-051 | Informational | Still valid | Informational (unchanged) | 90 (was 85) | #1027 (PoP checked first), #1025 (identity points persistable); no length or identity check on `c` | E1 `poc/F-XC-051/run3/` (first run of the run-1 PoC; short `c` accepted, group commitment silently truncated when sorted first) |
| F2-XC-001 | High | Fixed | n/a (resolved) | 95 (history) | `55d811e` #1046 (`1a292af` #1007 adds the env route) | E1 `poc/F2-XC-001/run3/`: 23 variants plus a control, mutation check, 213,312-document span fuzz |
| F2-XC-002 | Low | Still valid | Low (unchanged) | 92 | nothing; no `mode=rwc` or `create_if_missing` added | E1 `poc/F2-XC-002/run3/` (both binaries, plus the `${NAME}` route) |
| F2-XC-005 | Informational | Changed | Informational (unchanged, A17) | 90 | Direct Nonce: per-ceremony `signing_nonces`, cleared by `UPDATE … SET nonces = NULL`; bundled SQLite still without `SECURE_DELETE` | E1 `poc/F2-XC-005/run3/` (pruned secret, journal pre-image, and new: taken nonce stays in the file) |
| F2-XC-007 | Informational | Changed | Informational (unchanged, A1) | 75 | #1046 removed the error-retention leg; #1007 added a zeroized `${NAME}` route; the file buffer is still a plain `String` | E2 trace |
| F2-XC-050 | Low | Still valid | Low (unchanged) | 92 | nothing in scope changed the ordering (`driver.rs`, `state/storage.rs` untouched); #1017 widens what a warp page loses | E1 `poc/F2-XC-050/run3/` (3 tests, adapted) |
