## QA (QA3-SEN, 2026-10-08)

Result: Reproduced, through the real core `StateMachine` (not hand-sequenced).

Route: a `StateMachine<State, SentinelTransition>` over an in-memory sqlite pool. Phase 1 runs live to block 12 with the proposal at P=10 and our own commit at P+2=12, pruning so the oldest retained snapshot is safe=7 (< P). Phase 2 drops the machine and recreates it (restart), then feeds `Uncle(8)` (rolls back to 7, below the proposal) and `Warp{8..=14}` with one logs page replaying the proposal (P) and our own `Committed` (P+2). The real warp ordering applies `NewBlock` before each block's logs and defers the engine-check resume to after the page, so the replayed own `Committed` lands in `WaitingForEngineCheck` and is ignored (not counted as participation). The deferred resume then re-commits with `self_committed` false and queues a second `Commit` (reverts `AlreadyCommitted` onchain). Crossing the commit deadline with live blocks, no `Reveal` is ever queued for the bond that is onchain. Test `f3_sen_004_restart_replay_ignores_own_commit_and_drops_without_reveal`.

Command and output: `poc/F3-SEN-004/` (`cargo test -p sentinel --bins qa3_sen_004 -- --nocapture`; `statemachine-audit-commit.txt`, module `statemachine_poc_module.rs`).

Bite check: removed the drop guard `if !*self_committed { return false; }` in `handle_block_advance` (the line Basis row 5 cites). The test then FAILS ("any Reveal queued = true"), confirming it is coupled to the drop. That raw not-drop is unsound in general (it would reveal when no bond exists); the sound fix is remediation option 1 (record `self_committed` in `WaitingForEngineCheck`) or option 2/3 (read the oracle before re-committing/dropping). Output: `statemachine-bite-mutation.txt`. Reverted with `git checkout`.

Certainty: 95 (Reproduced through the real StateMachine warp path plus Critic Confirmed; tagged `known`, the restart/warp mechanics the Critic traced as E2 are now executed).
