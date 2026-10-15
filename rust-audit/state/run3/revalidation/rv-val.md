# Run 3 re-validation: validator rows (rv-val)

Audit commit `361aaf3`. One row per canonical finding. Evidence paths are under `rust-audit/poc/<id>/run3/` (E1) or name the trace (E2).

| ID | Ledger severity | Verdict | Proposed severity | Certainty | Changed by | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| F-VAL-001 | Critical | Fixed | n/a (resolved) | E1 | `14750ea` (#1027), on `010a09f` (#1022) | `poc/F-VAL-001/run3/` (original adapted: 0/3 pass; 8 variants pass; PoP-removal mutation brings the attack back), `poc/F2-VAL-001/run3/`, `poc/F-VAL-001-onchain/run3/` (7/7, mutation 6 fail) |
| F-VAL-002 | Medium | Changed | Low (`known`) | 90 | impact narrowed by `14750ea` (#1027); pad untouched; KDF fix #1026 closed | `poc/F-VAL-002/run3/` (2/2 pass after PoP), `poc/F2-VAL-002/run3/` |
| F-VAL-003 | Medium | Still valid | Medium (unchanged) | 82 | nothing removed it; #1019 changed bookkeeping around it | `poc/F-VAL-003/run3/` (2/2: plaintext response to an unshared plaintiff; per-accused count) |
| F-VAL-004 | Medium (rollover); genesis Informational `known` | Still valid | unchanged (genesis part is settled A16) | 69 | nothing in scope | `poc/F2-VAL-063/run3/` (setup never re-issued), `poc/F-VAL-061/run3/` (KeyGenSetup failure resumes as Noop) |
| F-VAL-005 | none (High at `2893917`, fixed before run 2) | Fixed | n/a | E1 | `8b88f4a` (#909), `c87c054` (#910) | `poc/F-VAL-005/run3/RESULT-store-tests.txt` (12/12) |
| F-VAL-030 | High | Moot | n/a (code removed; one-ceremony residual is F-VAL-061) | E1 (replacement) | `a9023ac` (#1038), `392d7c8` (#1040); replacement `3154a05` (#1035), `82d5419` (#1036) | `poc/F-VAL-030/run3/` (lost GenerateNonces costs one ceremony; old harness no longer compiles) |
| F-VAL-031 | Low | Moot | n/a (code removed) | E2 | `392d7c8` (#1040), `5e694c4` (#1039) | `git log --diff-filter=D` on `secrets/nonces.rs`; no thread or generator left |
| F-VAL-032 | High (team: accepted risk) | Fixed | n/a (resolved in code) | E1 | `82d5419` (#1036), `a9023ac` (#1038) | `poc/F-VAL-032/run3/` (6 variants pass; mutation restoring the drop arm fails V1, V2, V5) |
| F-VAL-033 | (High measured), out of scope A17 | Settled | n/a | E2 | trigger unchanged (operator restore) | settled.md section 1, A17 item |
| F-VAL-034 | Low | Still valid | Low (unchanged) | 55 | survived the Direct Nonce rewrite | `poc/F-VAL-034/run3/` (stale pair reaches frost-core and is refused; control shares) |
| F-VAL-035 | Low (observation, 35) | Changed | Informational | 70 | `06e3e6e` (#1033), `392d7c8` (#1040) | E2: JSON strings at `store.rs:236,241-253,274-291`; legs (b) narrowed, (c) moot |
| F-VAL-036 | Low | Moot | n/a (code removed) | E2 | `a9023ac` (#1038) | `git log -S 'fn observe'`; sequence only logged; F-VAL-032 run-3 V4 |
| F-VAL-037 | Informational | Still valid | Informational (unchanged) | 60 | nothing in scope | `poc/F-VAL-037/run3/` (address(0) proof verifies against an odd participants root) |
| F-VAL-038 | Low | Moot | n/a (code removed) | E2 | `392d7c8` (#1040) | `git log -S 'into_par_iter'`; no `rayon` in the tree |
| F-VAL-039 | High (58) | Moot | n/a (code removed) | E2 + E1 replacement | `a9023ac` (#1038), `82d5419` (#1036) | `git log -S 'NONCE_TOPUP_THRESHOLD'`; F-VAL-032 run-3 V4 |
| F-VAL-040 | Low | Changed | Low | 70 | `57b164b` (#1008), `82d5419` (#1036) removed the reveal-round leg; share-round `last_signer` remains | `poc/F-VAL-040/run3/` (repeat commitment ignored; last sharer becomes responsible, one extra timeout at 3 of 5) |
| F-VAL-060 | High (conditional on a malicious allow-listed oracle) | Changed | High conditional (unchanged) | 90 | scenario (b) removed by `82d5419` (#1036), `a9023ac` (#1038); dispatch unchanged | `poc/F-VAL-060/run3/` (forged complaints from the oracle address: f_me(0)\*G == c[0] at 3 of 5) |
| F-VAL-061 | High | Changed | Medium | 90 | policy unchanged; NonceTree placeholder and generator race removed by `a9023ac` (#1038), `392d7c8` (#1040) | `poc/F-VAL-061/run3/` (all four effects fail to Noop; consult tests), `poc/F-VAL-030/run3/`, `poc/F2-VAL-063/run3/` |
| F-VAL-062 | Informational | Changed | Informational | 90 | Direct Nonce effects moved the key share to `GenerateNonces` | `poc/F-VAL-061/run3/` Debug test (redacted by frost-core) |
| F-VAL-063 | Medium | Still valid | Medium (unchanged) | 72 | #1062 changed defaults and docs only | `poc/F-VAL-063/run3/` (F2-VAL-067 timing PoC re-run); E2 for the other legs |
| F-VAL-064 | Low | Still valid | Low (unchanged) | 72 | nothing in scope | E2: `main.rs:96-98`, `observability/mod.rs:24-33`, `validator.sample.toml:69`, `Dockerfile` |
| F-VAL-065 | Low | Changed | Low | 70 | Preprocess leg removed (`a9023ac` #1038, `392d7c8` #1040); duplicate-Sign leg defused (`82d5419` #1036); SetValidatorStaker leg unchanged | E2: `main.rs:82-92`, `action.rs:340-349`, `tx/storage.rs:112-127` |
| F-VAL-066 | Medium | Changed | Low | 65 | `73967cf` (#1015), `24eaf8d` (#1017) changed the sequencing; the unordered spawn and prune remain | `poc/F-VAL-066/run3/` (old same-input race still loses 20/20; short-outage loss deterministic, the store's documented concession) |
| F-VAL-067 | Low | Fixed | n/a (resolved) | E1 | `97f4cd1` (#1019) | `poc/F-VAL-067/run3/` (Rust aborts at the threshold-th complaint after answered ones; contract test passes; netting mutation fails it) |
| F-VAL-068 | none (unmerged branch, split in run 2) | Superseded | n/a | E2 | branch merged; D1 removed by `392d7c8` (#1040) | D1 to F2-VAL-031 (Moot), D2 store concession, D3 F-VAL-066, D4 F2-XC-006 |
| F2-VAL-003 | Medium | Fixed | n/a (resolved) | E1 | `24eaf8d` (#1017) | `poc/F2-VAL-003/run3/` (core StateMachine + validator Transition through a warp: commitments kept, restart and deadline variants; mutation brings the drop back) |
| F2-VAL-004 | Low (genesis case Informational `known`) | Still valid | Low (unchanged) | 50 | nothing in scope | `poc/F2-VAL-004/run3/` (share deadlines 32 vs 50; early share dropped, honest peer excluded) |
| F2-VAL-007 | Low | Still valid | Low (unchanged) | 65 | nothing in scope (one feeder, F2-VAL-003, fixed) | `poc/F2-VAL-007/run3/` (active_epoch stuck; session under hash(1->3); junk Sign at timeout) |
| F2-VAL-031 | Medium | Moot | n/a (code removed) | E2 + E1 replacement | `5e694c4` (#1039), `392d7c8` (#1040) | no generator left; consult tests in `poc/F-VAL-061/run3/` |
| F2-VAL-034 | Informational `known` | Settled | n/a | E2 | trigger unchanged (old database) | settled.md section 1, migration and old databases (Aegis release rule) |
| F2-VAL-064 | Informational | Still valid | Informational (unchanged) | 95 | nothing in scope | `poc/F2-VAL-064/run3/RESULT.txt` (`=` form rejected, exit 1) |

Counts: 31 rows. Fixed 5 (F-VAL-001, F-VAL-005, F-VAL-032, F-VAL-067, F2-VAL-003), Moot 6 (F-VAL-030, F-VAL-031, F-VAL-036, F-VAL-038, F-VAL-039, F2-VAL-031), Settled 2 (F-VAL-033, F2-VAL-034), Superseded 1 (F-VAL-068), Still valid 9 (F-VAL-003, F-VAL-004, F-VAL-034, F-VAL-037, F-VAL-063, F-VAL-064, F2-VAL-004, F2-VAL-007, F2-VAL-064), Changed 8 (F-VAL-002, F-VAL-035, F-VAL-040, F-VAL-060, F-VAL-061, F-VAL-062, F-VAL-065, F-VAL-066).

For the second verifier: the High-or-above Fixed rows are F-VAL-001 (variants v1 to v8 in Rust, 7 Foundry tests, both mutations) and F-VAL-032 (variants V1 to V6, one mutation); every variant is listed in the row's run-3 section.
