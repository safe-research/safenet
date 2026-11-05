# F3-XC-100 second reproduction (QA3-2A)

Route: the last committer cancels TWO honest verifying shares at once (threshold 3 leaves c[1] and c[2] free), a different attacker position from the one-victim case. Drives the real validator state machine to RolloverState::EpochSkipped.

Worktree: wt/rv-core-b. Env: env/rv-core-b.sh.

## Reproduce (defect present)

```sh
source <run folder>/env/rv-core-b.sh
cd "$WT"
printf '\n' >> crates/validator/src/state/keygen.rs
cat rust-audit/poc/F3-XC-100/repro-2/qa2a_xc100_two_victims.rs >> crates/validator/src/state/keygen.rs
cargo test --offline -p validator --bins qa2a_xc100 -- --nocapture --test-threads=1
git checkout -- crates/validator/src/state/keygen.rs
```

Pass: both victims publish y = (0,0), the two survivors publish real points, and the commitment round completes into CollectingShares for the poisoned group, then times out to EpochSkipped(2). See `baseline.out`.

## Bite (remediation option 1)

Before the `finalize_key_gen_commitments` call in `handle_key_gen_committed`, detect an identity verifying share (a zero share that `verify_secret_share` accepts) and restart excluding the last committer (`event.participant`). With the patch the validator restarts into a fresh group (excluding the attacker, 4 of 5 remain >= min_participants 4), so the poisoned group never reaches share collection and the test's assertion fails. See `with-fix.out`. Remediation is sound.
