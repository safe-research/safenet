# F3-VAL-062 second reproduction (QA3-2A)

Route: the warp page that replays the key-generation start carries ALL FIVE peer secret-share events before the local Resume::Setup (a different shape from the single-dropped-share case). Every share is dropped, so the restarted validator excludes the whole group at its share deadline and skips the epoch itself. A live-order control shows the drop and the deadline drift (1680 vs 1561) both vanish when the setup resumes first.

Worktree: wt/rv-core-b. Env: env/rv-core-b.sh.

## Reproduce (defect present)

```sh
source <run folder>/env/rv-core-b.sh
cd "$WT"
printf '\n' >> crates/validator/src/state/keygen.rs
cat rust-audit/poc/F3-VAL-062/repro-2/qa2a_val062_all_shares_dropped.rs >> crates/validator/src/state/keygen.rs
cargo test --offline -p validator --bins qa2a_val062 -- --nocapture --test-threads=1
git checkout -- crates/validator/src/state/keygen.rs
```

Pass: warp -> shared={}, share deadline 1680, then EpochSkipped(2); live control -> CollectingConfirmations, deadline 1561. See `baseline.out`.

## Bite (remediation option 1/3: buffer and replay)

Add `pending_shares: BTreeMap<B256, Vec<(Address, Bytes, u64)>>` to `State`; buffer a share that arrives while the group is still in CollectingCommitments for that gid; replay the buffer after `handle_key_gen_setup` finalizes into CollectingShares. With the patch the warp test reaches CollectingConfirmations (all five replayed), so the `shared == {}` assertion fails. See `with-fix.out`. Remediation is sound for the dropped-share effect; the deadline drift needs the separate "store the last commitment block" part of option 1.
