## QA (QA3-SEN, 2026-10-08)

Result: Reproduced (FSM level, driving the real `SentinelTransition::apply_transition`).

Route: Aegis windows (commit 6, reveal 6; bond 800 USDC.e). Variant A (`f3_sen_002_foreign_commit_before_verdict_strands_the_claim`): a foreign `Committed` seen while the request is in `WaitingForEngineCheck` is ignored (warn path), so after the verdict `commit_vote` starts the tally at zero; the local `committed_count` is 1 while the oracle's is 2. The foreign `Revealed` meets the local tally, the early finalize runs with `self_revealed` false, and the bonded request is dropped; the later `OracleResult` queues no `Claim`. Case 2 (`f3_sen_002_silent_committer_leaves_request_unfinalized`): a silent committer in the proposal block makes our own reveal early-finalize; the premature `Finalize` reverts onchain (`FinalizeTooEarly`) and `WaitingForOutcome` never retries, so the request is stuck at revealDeadline+30.

Command and output: `poc/F3-SEN-002/` (`cargo test -p sentinel --bins qa3_sen_poc -- --nocapture`; `fsm-audit-commit.txt`, module `fsm_poc_module.rs`).

Bite check: applied remediation option 1 (remove the early finalize in `handle_revealed`; keep a committed request in `finalize()` instead of dropping it). Both reproduction tests then FAIL (the request is kept and no premature finalize fires), and the soundness test `f3_sen_fix_recovers_the_claim` PASSES ("Claim queued after OracleResult = true"), so the bond is recovered. Remediation option 1 is sound. Output: `fsm-bite-remediation.txt`. Reverted with `git checkout`.

Certainty: 95 (Reproduced plus Critic Confirmed; the mechanism is executed deterministically, the honest one-block engine-latency gap is modeled rather than measured on a live chain, and the adversarial variant needs none).
