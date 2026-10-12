# F3-SEN-003 PoC (QA3-SEN)

Reproduced at the FSM level, driving the real `SentinelTransition::apply_transition`. The test `f3_sen_003_missed_own_reveal_drops_without_claim` lives in the shared module `fsm_poc_module.rs` (same module as F3-SEN-002); paste it into `mod tests` in `crates/sentinel/src/service.rs` and run:

```
cargo test -p sentinel --bins qa3_sen_poc -- --nocapture
```

- `fsm-audit-commit.txt`: on 361aaf3 the test passes (reproduces): at revealDeadline+1 the bonded request is dropped (tracked=false, commands=0) and the OracleResult queues no Claim, so the `RevealMissed` recovery never runs and 799.60 USDC.e is never claimed.
- `fsm-bite-remediation.txt`: with remediation option 1 applied (remove the early finalize in `handle_revealed`; keep a committed request in `finalize()` instead of dropping it), the test FAILS (bites) because the request is kept and a Finalize is queued, and `f3_sen_fix_recovers_the_claim` passes (a Claim is queued, the bond is recovered). The remediation is sound.

See `../F3-SEN-002/README.md` for the exact bite edits.
