# F3-SEN-002 PoC (QA3-SEN)

Reproduced at the FSM level, driving the real `SentinelTransition::apply_transition`. Tests in `fsm_poc_module.rs` (paste into `mod tests` in `crates/sentinel/src/service.rs`):

```
cargo test -p sentinel --bins qa3_sen_poc -- --nocapture
```

- `f3_sen_002_foreign_commit_before_verdict_strands_the_claim` (variant A): a foreign commit during the engine check is ignored (local committed_count 1 vs onchain 2); the undercount early-finalizes and drops the bonded request; the OracleResult queues no Claim.
- `f3_sen_002_silent_committer_leaves_request_unfinalized` (case 2): a silent committer in the proposal block makes our own reveal early-finalize; the Finalize reverts onchain and WaitingForOutcome never retries, so the request is stuck at revealDeadline+30.

`fsm-audit-commit.txt`: both pass on 361aaf3 (reproduce). `fsm-bite-remediation.txt`: under remediation option 1 they FAIL (bite) and `f3_sen_fix_recovers_the_claim` passes (the bond is recovered).

## Bite edits (remediation option 1), both in `crates/sentinel/src/service.rs`

1. `handle_revealed`: replace the early-finalize block

   ```
   if *revealed_count < *committed_count { return (state, Vec::new()); }
   let (update, actions) = self.finalize(entry, event.requestId);
   match update { None => { state.0.remove(&event.requestId); } Some(entry) => { state.0.insert(event.requestId, entry); } }
   (state, actions)
   ```

   with `let _ = (revealed_count, committed_count); (state, Vec::new())`.

2. `finalize()`: remove the drop guard

   ```
   let timed_out = *revealed_count == 0;
   if !*self_revealed && !timed_out { return (None, Vec::new()); }
   ```

   replacing it with `let _ = (*revealed_count, *self_revealed);`.

Rebuild and re-run; then `git checkout crates/sentinel/src/service.rs`.
