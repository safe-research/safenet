## QA (QA3-SEN, 2026-10-08)

Result: Reproduced (FSM level, driving the real `SentinelTransition::apply_transition`).

Route: Aegis windows (commit 6, reveal 6; bond 800 USDC.e, slash 0.40 USDC.e). This sentinel and one other commit; our own `Reveal` never lands; the other sentinel reveals. At `NewBlock(revealDeadline + 1)` the reveal-deadline branch calls `finalize()`, which returns `None` (`self_revealed` false, `revealed_count` 1), so the bonded request is dropped with no `Finalize` and no `Claim`. The `OracleResult` in the same block finds no tracked request, so the `RevealMissed` recovery in `handle_oracle_result` never runs and 799.60 USDC.e is never claimed. Test `f3_sen_003_missed_own_reveal_drops_without_claim`.

Command and output: `poc/F3-SEN-003/` (`cargo test -p sentinel --bins qa3_sen_poc -- --nocapture`; `fsm-audit-commit.txt`, module `fsm_poc_module.rs` shared with F3-SEN-002).

Bite check: applied remediation option 1 (keep a committed request in `finalize()`; send `Finalize` and move to `WaitingForOutcome` instead of dropping). The test then FAILS (request kept, a `Finalize` queued), and `f3_sen_fix_recovers_the_claim` PASSES (a `Claim` is queued and `RevealMissed` becomes reachable). Remediation option 1 is sound. Output: `fsm-bite-remediation.txt`. Reverted with `git checkout`.

Certainty: 95 (Reproduced plus Critic Confirmed; tagged `known`, with the unreachable recovery and the 799.60 vs 0.40 amount shown).
