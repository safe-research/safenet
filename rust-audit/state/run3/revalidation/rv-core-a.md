# Run 3 re-validation: rv-core-a (core indexing and runtime rows)

Audit commit `361aaf3`. One row per canonical finding; the full reasoning is the `## Run 3 re-validation (rv-core-a, 2026-10-08)` section at the end of each finding file. A certainty with "was" is a proposal backed by a run-3 execution that did not exist before; the verdict is unaffected.

| ID | Ledger severity | Verdict | Proposed severity | Certainty | Changed by | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| F-CORE-001 | High | Still valid | High (unchanged) | 99 | nothing in scope (`blocks.rs` moved with #1010, #1048; `state/storage.rs`, `driver.rs` identical) | E1 `poc/F-CORE-001/run3/`: run-1 unit PoC 3 of 3 reproduce; real binary exits 0 on the deep reorg, then resumes silently from the orphaned anchor |
| F-CORE-002 | High | Still valid | High (unchanged) | 99 | nothing changed the mechanism (#1047 to #1052 restructured `events.rs`) | E1 `poc/F-CORE-002/run3/` (run-1 PoC plus a partial-set variant), `poc/F2-CORE-002/run3/` |
| F-CORE-003 | Medium | Still valid | Medium (unchanged) | 80 (was 70) | nothing in scope | E1 `poc/F-CORE-003/run3/` (new: `-32001` then `null` gives `Uncle`, then the same block again) |
| F-CORE-004 | Medium | Still valid | Medium (unchanged) | 90 | #1049, #1050 add a `MissingBlock` route to the same unbounded loop | E1 `poc/F2-CORE-005/run3/`, `poc/F-CORE-004/run3/` (new header-fetch variant) |
| F-CORE-005 | Low | Still valid | Low (unchanged) | 85 (was 75) | nothing in scope | E1 `poc/F-CORE-005/run3/` (new: depth-0 `-32001` spin, no loud failure) |
| F-CORE-006 | Low | Still valid | Low (unchanged) | 70 | nothing in scope | E2, plus E1 `poc/F2-CORE-004/run3/` (decode permissiveness) |
| F-CORE-007 | Low | Still valid | Low (unchanged) | 70 | nothing in scope | E2 |
| F-CORE-008 | Low | Still valid | Low (unchanged) | 70 | nothing in scope (`clock.rs` identical) | E2 |
| F-CORE-009 | Low | Still valid | Low (unchanged) | 85 (was 78) | #1010 `Timing<T>`, no validation added | E1 `poc/F-CORE-009/run3/` (new: cases 1 and 3) |
| F-CORE-010 | Low | Still valid | Low (unchanged) | 45 | nothing in scope | E2 (no trigger) |
| F-CORE-011 | Low | Still valid | Low (unchanged) | 80 | #1046 `without_url` only; pins unchanged | E2 |
| F-CORE-012 | Medium | Still valid | Medium (unchanged) | 85 (was 70) | nothing changed the check; example event changed (#1040, #1042) | E1 `poc/F-CORE-012/run3/` (new: repeated-shape loss, no `check_logs_limit`) |
| F-CORE-030 | Medium | Still valid | Medium (unchanged, `known` per `settled.md` section 2) | 95 (was 85) | nothing in scope | E1 `poc/F2-VAL-066/run3/` (real binary: `ExceededMaxReorgDepth`, exit status 0, `/health` OK) |
| F-CORE-031 | High | Still valid | High (unchanged) | 92 | #1015, #1017 change which block transitions replay, not the loss | E1 `poc/F2-CORE-030/run3/` (4 tests), `poc/F-SEN-001/run3/` (2 tests) |
| F-CORE-032 | Low | Still valid | Low (unchanged) | 45 | nothing in scope | E1 mechanism (existing test), `poc/F-CORE-032/run3/`; no trigger |
| F-CORE-033 | Medium | Still valid | Medium (unchanged) | 70 | #1017 corrects a run-2 sub-claim only | E2 |
| F-CORE-034 | Medium | Changed | Medium (band unchanged) | 80 | #1062 removed the handbook "exponential backoff" sentence | E2; escalation horn E1 via `poc/F2-CORE-005/run3/`, `poc/F-CORE-004/run3/` |
| F-CORE-035 | Medium | Changed | Medium (band unchanged) | 78 | #999 (`eth_getProof`; its unsupported case overlaps settled L2), #1003 (batch histograms) | E2 |
| F-CORE-036 | Low | Still valid | Low (unchanged) | 85 | payloads changed (#1027, #1032 to #1044); all render redacted | E1 `poc/F-XC-002/run3/` (executed for the first time, plus two Direct Nonce variants) |
| F-CORE-037 | Low | Settled | n/a | n/a | settled item: migration, backward compatibility, old snapshots (Aegis release rule) | E2 `settled.md` section 1 |
| F-CORE-038 | Informational | Still valid | Informational (unchanged) | 90 (was 85) | nothing in scope | E1 `poc/F-CORE-038/run3/` (`kdf::tests`, Python HKDF and salt variants) |
| F-CORE-039 | Low | Still valid | Low (unchanged) | 72 | nothing in scope | E2 |
| F-CORE-040 | Low | Still valid | Low (unchanged) | 65 | nothing in scope (#1049 adds header work lost on cancellation) | E2 |
| F2-CORE-003 | Informational | Still valid | Informational (unchanged) | 80 (was 70) | nothing in scope | E1 `poc/F2-CORE-003/run3/` (new) |
| F2-CORE-007 | Low | Still valid | Low (unchanged) | 85 (was 75) | #1015 `BlockApplied` keeps the same guard | E1 `poc/F2-CORE-007/run3/` (new: `New{995} -> Err(BadUpdate)`) |
| F2-CORE-011 | Low | Still valid | Low (unchanged) | 92 | nothing in scope | E1 `poc/F2-CORE-011/run3/` (sequences A and B) |
| F2-CORE-067 | Informational | Changed | Informational (band unchanged) | 85 | #1003 batch histograms narrow the gap | E2 |

Counts: 27 rows; Still valid 23, Changed 3, Settled 1, Fixed 0, Moot 0, Superseded 0.
