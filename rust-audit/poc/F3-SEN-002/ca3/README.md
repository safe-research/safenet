# F3-SEN-002: CA3 reruns (2026-10-09)

Worktree `wt/rv-sen` at `361aaf3`. Real-binary runs use QA3-2B's harness (`../repro-2/h.py`) copied to the local run folder's `scratch/CA3/qa2b/` with ports moved to 31500-31509 and the sentinel binary built at `361aaf3`.

| Run | Command | Result |
| --- | --- | --- |
| QA3-SEN FSM tests | `fsm_poc_module.rs` pasted into `mod tests` of `crates/sentinel/src/service.rs`, filter `qa3_sen_poc` | `qa3-sen-fsm.txt`: the three reproduction tests pass (local `committed_count` 1 against 2 onchain; dropped, no `Claim`; stuck in `WaitingForOutcome` at revealDeadline+30; F3-SEN-003's drop), and `f3_sen_fix_recovers_the_claim` fails as expected without the fix |
| QA3-2B adversarial route | `python3 -I sen002.py <bin> ca3` | `sen002-ca3.txt`: both honest sentinels ignore M's commit, send a `finalize` that reverts `FinalizeTooEarly`, request `PENDING` at revealDeadline+20, no outcome event, three bonds locked |
| Honest variant (a), PC3's settling run | `python3 -I sen002a.py <bin> runN 7` (engine S answers 7 s after its request) | `sen002a-run1..3.txt`: S ignored A's commit in 3 of 3 (the undercount), but S's own reveal was mined first in the reveal block each time, so S took the recoverable path and claimed (3 of 3) |
| Honest variant, accounts swapped | `python3 -I sen002a.py <bin> swap1 7 swap` | `sen002a-swap1.txt`: same; S's reveal first again, S claimed |

So the honest path's bond loss (case 1 of the claim, the faster sentinel's reveal processed first) did not occur in 4 real-binary runs at the Aegis windows; it is executed only at the FSM level. The adversarial path (case 2) reproduces on the real binaries.
